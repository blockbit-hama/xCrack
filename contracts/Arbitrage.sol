// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title xCrack Arbitrage Strategy (FlashLoan + Cross-DEX)
 * @author
 * @notice
 *  - Aave v3 FlashLoanSimpleReceiverBase를 이용해 단일 트랜잭션 내에서
 *    [대출] -> [DEX A 매수] -> [DEX B 매도] -> [상환]을 수행합니다.
 *  - DEX 호출은 임의 라우터 주소 + calldata로 저수준 호출하여
 *    Uniswap v2/v3, 1inch/Aggregator 등 다양한 경로를 지원합니다.
 *  - ETH는 지원하지 않고, WETH 등 ERC-20 기준으로만 동작합니다.
 *
 * SECURITY NOTES
 *  - 외부 호출(DEX) 전 approve는 필요한 양만 최소화합니다(필요 시 0 -> amt).
 *  - 재진입 방지(ReentrancyGuard), 소유자 제약(onlyOwner).
 *  - 이익 미달/상환 불가 시 require로 전체 트랜잭션 revert.
 */

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";


contract ArbitrageStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard {
    using SafeERC20 for IERC20;

    address private owner;

    /// @dev 아비트래지 시나리오 파라미터
    struct ArbitrageParams {
        address tokenA;          // 대출/기준 토큰 (예: USDC)
        address tokenB;          // 중간 토큰 (예: WETH)
        address dexA;            // 첫 번째 DEX 라우터(매수)
        address dexB;            // 두 번째 DEX 라우터(매도)
        uint256 amountIn;        // 대출받아 첫 스왑에 투입할 tokenA 양
        uint256 expectedProfitMin; // 최소 기대 이익(tokenA 단위). 미만이면 revert.
        bytes   swapCallDataA;   // DEX A에 보낼 실행용 calldata (예: swapExactTokensForTokens 등)
        bytes   swapCallDataB;   // DEX B에 보낼 실행용 calldata
    }

    // ─────────────────────────────
    //             Events
    // ─────────────────────────────
    event FlashLoanRequested(address indexed asset, uint256 amount);
    event SwapExecuted(
        address indexed dex,
        address indexed inToken,
        address indexed outToken,
        uint256 amountIn,
        uint256 amountOut
    );
    event ArbitrageSucceeded(
        address indexed asset,
        uint256 amountBorrowed,
        uint256 premium,
        uint256 profit
    );
    event Rescue(address indexed token, uint256 amount, address to);

    // ─────────────────────────────
    //            Errors
    // ─────────────────────────────
    error NotAuthorized();
    error InvalidCaller();
    error BadToken();
    error DexCallFailed(address dex, bytes data, bytes reason);
    error NoProfit();

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotAuthorized();
        _;
    }

    constructor(IPoolAddressesProvider provider)
        FlashLoanSimpleReceiverBase(provider)
    {
        owner = msg.sender;
    }

    // ─────────────────────────────
    //        External Interface
    // ─────────────────────────────

    /**
     * @notice 아비트래지 실행(FlashLoan 트리거)
     * @dev
     *  - asset == params.tokenA 이어야 합니다.
     *  - amount == params.amountIn 이어야 합니다.
     *  - 성공/실패 여부는 단일 트랜잭션에서 결정됩니다(atomic).
     */
    function executeArbitrage(
        address asset,
        uint256 amount,
        ArbitrageParams calldata params
    ) external onlyOwner nonReentrant {
        if (asset != params.tokenA) revert BadToken();
        if (amount != params.amountIn) revert BadToken();
        // 간단한 방어: DEX 주소는 컨트랙트여야
        _assertIsContract(params.dexA);
        _assertIsContract(params.dexB);

        bytes memory data = abi.encode(params);
        emit FlashLoanRequested(asset, amount);
        // interestRateMode=0 (FlashLoanSimple)
        POOL.flashLoanSimple(address(this), asset, amount, data, 0);
    }

    /**
     * @notice Aave v3 FlashLoanSimple 콜백
     * @dev
     *  1) tokenA로 DEX A에서 tokenB 매수
     *  2) tokenB로 DEX B에서 tokenA 매도
     *  3) 이익/상환 체크 후 승인 + true 반환
     */
    function executeOperation(
        address asset,               // 대출 토큰(tokenA)
        uint256 amount,              // 대출 원금
        uint256 premium,             // 이자(수수료)
        address initiator,           // flashLoanSimple 호출자(본 컨트랙트)
        bytes calldata params        // ArbitrageParams
    ) external override returns (bool) {
        if (msg.sender != address(POOL)) revert InvalidCaller();
        if (initiator != address(this)) revert InvalidCaller();

        ArbitrageParams memory p = abi.decode(params, (ArbitrageParams));
        if (asset != p.tokenA) revert BadToken();

        // (1) DEX A: tokenA -> tokenB
        uint256 tokenBOut = _buyOnDexA(p, amount);

        // (2) DEX B: tokenB -> tokenA
        uint256 tokenAOut = _sellOnDexB(p, tokenBOut);

        // (3) 상환/이익 확인
        uint256 owe = amount + premium;
        if (tokenAOut <= owe + p.expectedProfitMin) revert NoProfit();

        // Aave가 pull 하므로 approve 필요
        IERC20(asset).safeApprove(address(POOL), 0);
        IERC20(asset).safeApprove(address(POOL), owe);

        emit ArbitrageSucceeded(asset, amount, premium, tokenAOut - owe);
        return true;
    }

    // ─────────────────────────────
    //     Internal Swap Helpers
    // ─────────────────────────────

    /**
     * @dev DEX A에 swapCallDataA로 저수준 호출하여 tokenA -> tokenB 스왑
     * @return tokenBReceived 수령한 tokenB 양
     */
    function _buyOnDexA(ArbitrageParams memory p, uint256 amount)
        private
        returns (uint256 tokenBReceived)
    {
        IERC20 tokenA = IERC20(p.tokenA);
        IERC20 tokenB = IERC20(p.tokenB);

        // 잔고 스냅샷
        uint256 beforeBal = tokenB.balanceOf(address(this));

        // 필요량만 승인 (0 → amount)
        _safeApprove(tokenA, p.dexA, 0);
        _safeApprove(tokenA, p.dexA, amount);

        // 저수준 호출: 라우터가 pull 하거나 내부에서 transferFrom/permit2 등 사용
        (bool ok, bytes memory ret) = p.dexA.call(p.swapCallDataA);
        if (!ok) revert DexCallFailed(p.dexA, p.swapCallDataA, ret);

        // 실제 수령량은 잔고 차이로 계산
        uint256 afterBal = tokenB.balanceOf(address(this));
        tokenBReceived = afterBal - beforeBal;

        emit SwapExecuted(p.dexA, p.tokenA, p.tokenB, amount, tokenBReceived);
    }

    /**
     * @dev DEX B에 swapCallDataB로 저수준 호출하여 tokenB -> tokenA 스왑
     * @return tokenAReceived 수령한 tokenA 양
     */
    function _sellOnDexB(ArbitrageParams memory p, uint256 tokenBAmount)
        private
        returns (uint256 tokenAReceived)
    {
        IERC20 tokenA = IERC20(p.tokenA);
        IERC20 tokenB = IERC20(p.tokenB);

        // 잔고 스냅샷
        uint256 beforeBal = tokenA.balanceOf(address(this));

        // 필요량만 승인
        _safeApprove(tokenB, p.dexB, 0);
        _safeApprove(tokenB, p.dexB, tokenBAmount);

        (bool ok, bytes memory ret) = p.dexB.call(p.swapCallDataB);
        if (!ok) revert DexCallFailed(p.dexB, p.swapCallDataB, ret);

        uint256 afterBal = tokenA.balanceOf(address(this));
        tokenAReceived = afterBal - beforeBal;

        emit SwapExecuted(p.dexB, p.tokenB, p.tokenA, tokenBAmount, tokenAReceived);
    }

    // ─────────────────────────────
    //     View: Profit Estimation
    // ─────────────────────────────

    /**
     * @notice 예상 이익 계산(견적 전용). **중요**: 이 함수 호출 시
     *  - `swapCallDataA/B`는 실제 swap용이 아닌, 각 DEX의 **견적(quote)용** calldata여야 합니다.
     *    * 예: Uniswap v2 라우터의 `getAmountsOut(amountIn, path)` 인코딩
     *          Uniswap v3 Quoter의 `quoteExactInput(...)` 인코딩 등
     *  - 반환값은 tokenA 기준의 예상 이익입니다(수수료/프리미엄 차감).
     */
    function calculateProfitability(ArbitrageParams calldata params)
        external
        view
        returns (uint256 expectedProfit)
    {
        // 1) DEX A 견적: tokenA -> tokenB
        uint256 quoteB = _staticQuote(params.dexA, params.swapCallDataA);

        // 2) DEX B 견적: tokenB(quoteB) -> tokenA
        //    (대부분의 Quoter는 amountIn이 calldata에 포함되어 있으므로,
        //     params.swapCallDataB를 만들 때 quoteB를 반영해 인코딩해야 함)
        uint256 quoteBackA = _staticQuote(params.dexB, params.swapCallDataB);

        // 플래시론 프리미엄은 체인/자산/상황에 따라 달라 실행 시점에만 확정됨.
        // 여기서는 최소 기대 이익과 비교하는 용도로 사용자가 적용.
        // expectedProfit = (quoteBackA - params.amountIn) - estPremium;
        if (quoteBackA > params.amountIn) {
            expectedProfit = quoteBackA - params.amountIn;
        } else {
            expectedProfit = 0;
        }
    }

    /// @dev staticcall로 DEX에 견적용 calldata를 던지고, 첫 번째 uint256을 파싱해 반환
    function _staticQuote(address dex, bytes calldata quoteCalldata)
        internal
        view
        returns (uint256 amountOut)
    {
        _assertIsContract(dex);
        (bool ok, bytes memory ret) = dex.staticcall(quoteCalldata);
        if (!ok || ret.length < 32) {
            // 일부 Quoter는 revert reason으로 금액을 encode하기도 하므로,
            // 단순화: 실패 시 0 반환
            return 0;
        }
        // 가장 단순한 케이스: 반환값의 첫 번째 uint256만 사용
        amountOut = abi.decode(ret, (uint256));
    }

    // ─────────────────────────────
    //          Owner Utils
    // ─────────────────────────────

    function setOwner(address newOwner) external onlyOwner {
        owner = newOwner;
    }

    /// @notice 잘못 들어온 토큰 회수
    function rescue(address token, uint256 amount, address to) external onlyOwner {
        IERC20(token).safeTransfer(to, amount);
        emit Rescue(token, amount, to);
    }

    // ─────────────────────────────
    //          Internals
    // ─────────────────────────────

    function _safeApprove(IERC20 token, address spender, uint256 amount) internal {
        // 일부 토큰은 non-zero → non-zero 갱신을 거부하므로 0으로 초기화 후 설정
        token.safeApprove(spender, 0);
        token.safeApprove(spender, amount);
    }

    function _assertIsContract(address account) internal view {
        uint256 size;
        assembly { size := extcodesize(account) }
        require(size > 0, "not a contract");
    }
}
