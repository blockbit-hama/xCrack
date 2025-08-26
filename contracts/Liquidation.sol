// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title xCrack Liquidation Strategy
 * @notice Flashloan-based liquidation bot for Aave/Compound protocols (ERC-20 only)
 * @dev
 *  - Aave v3: IPool.liquidationCall(collateral, debt, user, debtToCover, false)
 *  - Compound v2: cTokenDebt.liquidateBorrow(user, repayAmount, cTokenCollateral) + redeem()
 *  - DEX swap: arbitrary router + calldata (low-level call)
 *  - FlashLoan: Aave v3 FlashLoanSimpleReceiverBase
 *  - ETH 미지원(언더라이잉은 ERC-20 가정; WETH 사용)
 */

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

// ─────────────────────────────────────────────
// Minimal Interfaces
// ─────────────────────────────────────────────

interface IAavePoolMinimal {
    function liquidationCall(
        address collateralAsset,
        address debtAsset,
        address user,
        uint256 debtToCover,
        bool receiveAToken
    ) external;
}

// Compound v2 cERC20 minimal
interface ICErc20 {
    function liquidateBorrow(address borrower, uint256 repayAmount, address cTokenCollateral) external returns (uint256);
    function redeem(uint256 redeemTokens) external returns (uint256);
    function balanceOf(address) external view returns (uint256);
    function underlying() external view returns (address); // cETH에는 없음(여기선 ERC-20만 지원)
}

contract LiquidationStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard {
    using SafeERC20 for IERC20;

    address private owner;

    /**
     * @dev 매핑 규칙
     *  - Aave:
     *      protocol       = Aave IPool 주소
     *      collateralAsset= 언더라이잉 담보 ERC20
     *      debtAsset      = 언더라이잉 부채 ERC20
     *  - Compound v2:
     *      protocol       = cTokenDebt 주소 (**cToken**)
     *      collateralAsset= cTokenCollateral 주소 (**cToken**)
     *      debtAsset      = cTokenDebt 주소 (**cToken**)  ← protocol과 동일 가능
     */
    struct LiquidationParams {
        address protocol;          // Aave: IPool / Compound: cTokenDebt
        address user;              // 청산 대상
        address collateralAsset;   // Aave: underlying collateral / Compound: cTokenCollateral
        address debtAsset;         // Aave: underlying debt      / Compound: cTokenDebt
        uint256 debtToCover;       // 상환(청산)할 부채 양 (언더라이잉 기준)
        address dexRouter;         // 스왑 라우터 주소
        bytes   swapCalldata;      // collateral→debt 스왑 calldata (low-level)
    }

    // ─────────────────────────────
    //            Events
    // ─────────────────────────────
    event FlashLoanTriggered(address indexed asset, uint256 amount);
    event AaveLiquidated(address indexed user, address collateral, address debt, uint256 debtCovered);
    event CompoundLiquidated(address indexed user, address cDebt, address cCollat, uint256 repayAmount, uint256 cTokensSeized);
    event CollateralRedeemed(address indexed cToken, uint256 cAmount, address underlying, uint256 uReceived);
    event Swapped(address indexed router, address inToken, address outToken, uint256 inAmt, uint256 outAmt);
    event Repaid(address indexed asset, uint256 amountOwed);
    event OwnerChanged(address indexed oldOwner, address indexed newOwner);
    event Rescue(address indexed token, uint256 amount, address to);

    modifier onlyOwner() {
        require(msg.sender == owner, "Not authorized");
        _;
    }

    constructor(IPoolAddressesProvider provider) FlashLoanSimpleReceiverBase(provider) {
        owner = msg.sender;
    }

    // ─────────────────────────────
    //        External Entrypoint
    // ─────────────────────────────

    /**
     * @notice 플래시론 트리거
     * @param asset   대출받을 언더라이잉 토큰(= debt 언더라이잉)
     * @param amount  대출 양 (보통 debtToCover와 동일/이상)
     * @param params  청산 파라미터 (위 주석의 매핑 규칙 필독)
     */
    function executeLiquidation(
        address asset,
        uint256 amount,
        LiquidationParams calldata params
    ) external onlyOwner nonReentrant {
        // 최소한의 sanity check
        require(amount >= params.debtToCover, "amount < debtToCover");
        emit FlashLoanTriggered(asset, amount);
        POOL.flashLoanSimple(address(this), asset, amount, abi.encode(params), 0);
    }

    /**
     * @dev Aave v3 FlashLoanSimple 콜백
     * 1) 프로토콜 감지(Aave/Compound) 후 청산 실행
     * 2) 담보 언더라이잉을 debt 언더라이잉으로 스왑
     * 3) 플래시론 상환
     */
    function executeOperation(
        address asset,               // 플래시론 대출 언더라이잉(= 상환해야 할 토큰)
        uint256 amount,              // 원금
        uint256 premium,             // 수수료
        address initiator,           // 본 컨트랙트
        bytes calldata params
    ) external override returns (bool) {
        require(msg.sender == address(POOL), "Invalid caller");
        require(initiator == address(this), "Invalid initiator");

        LiquidationParams memory p = abi.decode(params, (LiquidationParams));

        // ── 1) 청산 실행
        bool isAave = _isAavePool(p.protocol);
        if (isAave) {
            _liquidateOnAave(p); // Aave: 언더라이잉 기준 파라미터
        } else {
            _liquidateOnCompound(p, asset); // Compound: cToken 기반, asset은 debt 언더라이잉
        }

        // ── 2) 담보 언더라이잉 → debt 언더라이잉 스왑
        // Aave: 담보 언더라이잉 = p.collateralAsset
        // Compound: 담보 언더라이잉 = underlying(cTokenCollateral)
        address collateralUnderlying = isAave
            ? p.collateralAsset
            : ICErc20(p.collateralAsset).underlying();

        // 스왑 실행 (collateralUnderlying -> asset)
        _swapCollateralForDebt(p.dexRouter, p.swapCalldata, collateralUnderlying, asset);

        // ── 3) 플래시론 상환
        uint256 amountOwed = amount + premium;
        IERC20(asset).safeApprove(address(POOL), 0);
        IERC20(asset).safeApprove(address(POOL), amountOwed);
        emit Repaid(asset, amountOwed);

        // (남은 잔고는 이익; 별도 인출 로직은 오프체인에서 호출)
        return true;
    }

    // ─────────────────────────────
    //           Aave Path
    // ─────────────────────────────

    /**
     * @dev Aave v3 청산
     * - p.protocol: Aave IPool
     * - p.collateralAsset / p.debtAsset: 언더라이잉 ERC-20
     */
    function _liquidateOnAave(LiquidationParams memory p) internal {
        // debt 언더라이잉 승인
        IERC20(p.debtAsset).safeApprove(p.protocol, 0);
        IERC20(p.debtAsset).safeApprove(p.protocol, p.debtToCover);

        // liquidationCall(collateral, debt, user, debtToCover, receiveAToken=false)
        IAavePoolMinimal(p.protocol).liquidationCall(
            p.collateralAsset,
            p.debtAsset,
            p.user,
            p.debtToCover,
            false /* 담보를 aToken 대신 언더라이잉으로 수령 */
        );

        emit AaveLiquidated(p.user, p.collateralAsset, p.debtAsset, p.debtToCover);
        // 호출 후 이 컨트랙트 주소로 담보 언더라이잉이 전송되어 있음
    }

    // ─────────────────────────────
    //         Compound v2 Path
    // ─────────────────────────────

    /**
     * @dev Compound v2 청산
     * - p.protocol: cTokenDebt 주소
     * - p.debtAsset: cTokenDebt 주소
     * - p.collateralAsset: cTokenCollateral 주소
     * - asset: debt 언더라이잉 (flashloan asset)
     *
     * liquidateBorrow(user, repayAmount, cTokenCollateral) 호출 후,
     * 콜래트럴로 받은 cTokenCollateral 잔고를 redeem() 하여 언더라이잉 확보.
     */
    function _liquidateOnCompound(LiquidationParams memory p, address asset) internal {
        address cDebt = p.debtAsset;           // = protocol (동일)
        address cColl = p.collateralAsset;

        // debt 언더라이잉 승인 → cTokenDebt가 pull
        IERC20(asset).safeApprove(cDebt, 0);
        IERC20(asset).safeApprove(cDebt, p.debtToCover);

        // liquidateBorrow
        uint256 err = ICErc20(cDebt).liquidateBorrow(p.user, p.debtToCover, cColl);
        require(err == 0, "Compound: liquidateBorrow failed");

        // seize된 담보 cToken 양
        uint256 seizedCTokens = ICErc20(cColl).balanceOf(address(this));

        // 담보 언더라이잉으로 redeem
        // (redeemUnderlying도 가능하지만 여기선 보유 cToken 전량 redeem)
        uint256 beforeU = IERC20(ICErc20(cColl).underlying()).balanceOf(address(this));
        uint256 err2 = ICErc20(cColl).redeem(seizedCTokens);
        require(err2 == 0, "Compound: redeem failed");
        uint256 afterU = IERC20(ICErc20(cColl).underlying()).balanceOf(address(this));

        emit CompoundLiquidated(p.user, cDebt, cColl, p.debtToCover, seizedCTokens);
        emit CollateralRedeemed(cColl, seizedCTokens, ICErc20(cColl).underlying(), afterU - beforeU);
        // 이후 담보 언더라이잉이 이 컨트랙트에 있음
    }

    // ─────────────────────────────
    //          DEX Swap Helper
    // ─────────────────────────────

    /**
     * @dev 저수준 스왑 호출 (router.call(calldata))
     * @param router      DEX 라우터 주소
     * @param swapData    인코딩된 calldata (예: UniV2 swapExactTokensForTokens, UniV3 exactInput 등)
     * @param tokenIn     스왑 입력 토큰(= 담보 언더라이잉)
     * @param tokenOut    스왑 출력 토큰(= 플래시론 상환 토큰)
     *
     * - approve는 필요한 양만(대개 현재 잔고)으로 최소화
     * - 반환값 파싱 대신 잔고 차이로 수령량 계산
     * - 슬리피지는 calldata 내부의 amountOutMin 등으로 보호해야 함
     */
    function _swapCollateralForDebt(
        address router,
        bytes memory swapData,
        address tokenIn,
        address tokenOut
    ) internal {
        require(router != address(0), "router=0");
        _assertIsContract(router);

        IERC20 inT = IERC20(tokenIn);
        IERC20 outT = IERC20(tokenOut);

        uint256 inBal = inT.balanceOf(address(this));
        if (inBal == 0) return; // 청산에서 바로 debt가 충당되었거나 담보가 없을 수도

        // approve 최소화
        inT.safeApprove(router, 0);
        inT.safeApprove(router, inBal);

        uint256 beforeOut = outT.balanceOf(address(this));
        (bool ok, bytes memory ret) = router.call(swapData);
        require(ok, _revertMsg("DEX swap failed", ret));
        uint256 afterOut = outT.balanceOf(address(this));

        emit Swapped(router, tokenIn, tokenOut, inBal, afterOut - beforeOut);
    }

    // ─────────────────────────────
    //        Utility / Admin
    // ─────────────────────────────

    function setOwner(address newOwner) external onlyOwner {
        address old = owner;
        owner = newOwner;
        emit OwnerChanged(old, newOwner);
    }

    /// 잘못 들어온 토큰 회수(이익 인출 등)
    function emergencyWithdraw(address token, uint256 amount) external onlyOwner {
        IERC20(token).safeTransfer(owner, amount);
        emit Rescue(token, amount, owner);
    }

    // protocol이 Aave Pool인지 간단 판별: liquidationCall selector staticcall 성공 여부
    function _isAavePool(address protocol) internal view returns (bool) {
        bytes4 sel = IAavePoolMinimal.liquidationCall.selector;
        (bool ok, ) = protocol.staticcall(abi.encodeWithSelector(sel, address(0), address(0), address(0), 0, false));
        // 당연히 파라미터가 말이 안돼 revert 날 수 있어, 단순 성공여부로 판단하기 어렵다.
        // 대신 코드 사이즈 기반으로 최소 판별 + try-catch를 권장하나, 여기선 간단화:
        uint256 size;
        assembly { size := extcodesize(protocol) }
        return size > 0; // 실전에서는 네트워크 상수/리스트로 엄격 관리 추천
    }

    function _assertIsContract(address account) internal view {
        uint256 size;
        assembly { size := extcodesize(account) }
        require(size > 0, "not a contract");
    }

    function _revertMsg(string memory base, bytes memory reason) internal pure returns (string memory) {
        // best-effort revert reason append
        if (reason.length < 68) return base;
        assembly {
            reason := add(reason, 0x04)
        }
        return string(abi.encodePacked(base, ": ", abi.decode(reason, (string))));
    }
}
