// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title xCrack Multi-Asset Arbitrage Strategy (FlashLoan + Cross-DEX)
 * @author
 * @notice
 *  - Aave v3 FlashLoanReceiverBase를 이용해 한 번의 트랜잭션에서
 *    여러 토큰을 동시에 빌려 복합 아비트래지/포지션 마이그레이션을 수행합니다.
 *  - 삼각 아비트래지: A,B → C → A,B (두 자산을 동시에 확보하여 시작)
 *  - 포지션 마이그레이션: 부채 토큰 + 담보 토큰을 함께 빌려 원자적으로 갈아끼우기
 *  - 청산/차익거래 복합 경로: 여러 토큰을 같은 블록에서 사용하고 되돌리기
 *  - DEX 호출은 임의 라우터 주소 + calldata로 저수준 호출하여
 *    Uniswap v2/v3, 1inch/Aggregator 등 다양한 경로를 지원합니다.
 *
 * SECURITY NOTES
 *  - 외부 호출(DEX) 전 approve는 필요한 양만 최소화합니다(필요 시 0 -> amt).
 *  - 재진입 방지(ReentrancyGuard), 소유자 제약(onlyOwner).
 *  - 이익 미달/상환 불가 시 require로 전체 트랜잭션 revert.
 *  - modes[i] = 0 (진짜 플래시론, 부채 남기지 않음)
 */

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanReceiverBase.sol";

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract MultiAssetArbitrageStrategy is FlashLoanReceiverBase, ReentrancyGuard {
    using SafeERC20 for IERC20;

    address private owner;

    /// @dev 다중자산 아비트래지 시나리오 파라미터
    struct MultiAssetArbitrageParams {
        address[] borrowAssets;     // 대출받을 토큰들 (예: [USDC, WETH])
        uint256[] borrowAmounts;    // 각 토큰별 대출량
        address[] targetAssets;     // 최종 목표 토큰들 (예: [USDC, WETH])
        address[] dexes;            // 사용할 DEX 라우터들
        address[] spenders;         // (옵션) approve 대상들. 0이면 해당 dex 사용
        uint256 expectedProfitMin;  // 최소 기대 이익(기준 토큰 단위)
        bytes[] swapCallData;       // 각 DEX에 보낼 실행용 calldata 배열
        uint256[] swapSequences;    // 스왑 실행 순서 (dexes 인덱스 배열)
    }

    /// @dev 삼각 아비트래지 전용 파라미터
    struct TriangularArbitrageParams {
        address tokenA;             // 첫 번째 대출 토큰 (예: USDC)
        address tokenB;             // 두 번째 대출 토큰 (예: WETH)
        address tokenC;             // 중간 토큰 (예: DAI)
        uint256 amountA;            // tokenA 대출량
        uint256 amountB;            // tokenB 대출량
        address dexAB;              // A→C 스왑용 DEX
        address dexBC;              // B→C 스왑용 DEX
        address dexCA;              // C→A 스왑용 DEX
        address dexCB;              // C→B 스왑용 DEX
        uint256 expectedProfitMin;  // 최소 기대 이익
        bytes swapCallDataAB;       // A→C 스왑 calldata
        bytes swapCallDataBC;       // B→C 스왑 calldata
        bytes swapCallDataCA;       // C→A 스왑 calldata
        bytes swapCallDataCB;       // C→B 스왑 calldata
    }

    /// @dev 포지션 마이그레이션 파라미터
    struct PositionMigrationParams {
        address[] debtAssets;       // 기존 부채 토큰들
        uint256[] debtAmounts;      // 기존 부채량들
        address[] collateralAssets; // 기존 담보 토큰들
        uint256[] collateralAmounts; // 기존 담보량들
        address[] newDebtAssets;    // 새로운 부채 토큰들
        address[] newCollateralAssets; // 새로운 담보 토큰들
        address[] migrationDexes;   // 마이그레이션용 DEX들
        bytes[] migrationCallData;  // 마이그레이션 calldata들
        uint256 expectedSavingMin;  // 최소 기대 절약액
    }

    // ─────────────────────────────
    //             Events
    // ─────────────────────────────
    event MultiAssetFlashLoanRequested(address[] assets, uint256[] amounts);
    event SwapExecuted(
        address indexed dex,
        address indexed inToken,
        address indexed outToken,
        uint256 amountIn,
        uint256 amountOut
    );
    event MultiAssetArbitrageSucceeded(
        address[] assets,
        uint256[] amounts,
        uint256[] premiums,
        uint256 totalProfit
    );
    event TriangularArbitrageSucceeded(
        address tokenA,
        address tokenB,
        address tokenC,
        uint256 amountA,
        uint256 amountB,
        uint256 totalProfit
    );
    event PositionMigrationSucceeded(
        address[] oldDebtAssets,
        address[] newDebtAssets,
        uint256 totalSaving
    );
    event Rescue(address indexed token, uint256 amount, address to);

    // ─────────────────────────────
    //            Errors
    // ─────────────────────────────
    error NotAuthorized();
    error InvalidCaller();
    error BadToken();
    error InvalidArrayLength();
    error DexCallFailed(address dex, bytes data, bytes reason);
    error NoProfit();
    error InsufficientSaving();

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotAuthorized();
        _;
    }

    constructor(IPoolAddressesProvider provider)
        FlashLoanReceiverBase(provider)
    {
        owner = msg.sender;
    }

    // ─────────────────────────────
    //        External Interface
    // ─────────────────────────────

    /**
     * @notice 다중자산 아비트래지 실행(FlashLoan 트리거)
     * @dev 복잡한 다중자산 시나리오를 위한 범용 함수
     */
    function executeMultiAssetArbitrage(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata modes,
        MultiAssetArbitrageParams calldata params
    ) external onlyOwner nonReentrant {
        if (assets.length != amounts.length || assets.length != modes.length) {
            revert InvalidArrayLength();
        }
        if (assets.length != params.borrowAssets.length) {
            revert InvalidArrayLength();
        }
        
        // 모든 mode는 0 (진짜 플래시론)이어야 함
        for (uint256 i = 0; i < modes.length; i++) {
            require(modes[i] == 0, "Only flash loan mode 0 supported");
        }

        bytes memory data = abi.encode(params);
        emit MultiAssetFlashLoanRequested(assets, amounts);
        
        POOL.flashLoan(
            address(this),
            assets,
            amounts,
            modes,
            address(this), // onBehalfOf
            data,
            0 // referralCode
        );
    }

    /**
     * @notice 삼각 아비트래지 실행
     * @dev A,B → C → A,B 패턴의 삼각 아비트래지
     */
    function executeTriangularArbitrage(
        TriangularArbitrageParams calldata params
    ) external onlyOwner nonReentrant {
        address[] memory assets = new address[](2);
        uint256[] memory amounts = new uint256[](2);
        uint256[] memory modes = new uint256[](2);
        
        assets[0] = params.tokenA;
        assets[1] = params.tokenB;
        amounts[0] = params.amountA;
        amounts[1] = params.amountB;
        modes[0] = 0; // flash loan
        modes[1] = 0; // flash loan

        bytes memory data = abi.encode(params);
        emit MultiAssetFlashLoanRequested(assets, amounts);
        
        POOL.flashLoan(
            address(this),
            assets,
            amounts,
            modes,
            address(this),
            data,
            0
        );
    }

    /**
     * @notice 포지션 마이그레이션 실행
     * @dev 부채 + 담보를 함께 빌려 원자적으로 갈아끼우기
     */
    function executePositionMigration(
        PositionMigrationParams calldata params
    ) external onlyOwner nonReentrant {
        // 부채 + 담보 자산들을 합쳐서 플래시론 요청
        uint256 totalAssets = params.debtAssets.length + params.collateralAssets.length;
        address[] memory assets = new address[](totalAssets);
        uint256[] memory amounts = new uint256[](totalAssets);
        uint256[] memory modes = new uint256[](totalAssets);
        
        uint256 idx = 0;
        // 부채 자산들 추가
        for (uint256 i = 0; i < params.debtAssets.length; i++) {
            assets[idx] = params.debtAssets[i];
            amounts[idx] = params.debtAmounts[i];
            modes[idx] = 0;
            idx++;
        }
        // 담보 자산들 추가
        for (uint256 i = 0; i < params.collateralAssets.length; i++) {
            assets[idx] = params.collateralAssets[i];
            amounts[idx] = params.collateralAmounts[i];
            modes[idx] = 0;
            idx++;
        }

        bytes memory data = abi.encode(params);
        emit MultiAssetFlashLoanRequested(assets, amounts);
        
        POOL.flashLoan(
            address(this),
            assets,
            amounts,
            modes,
            address(this),
            data,
            0
        );
    }

    /**
     * @notice Aave v3 FlashLoan 콜백 (다중자산)
     * @dev assets[], amounts[], premiums[] 배열을 처리
     */
    function executeOperation(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        if (msg.sender != address(POOL)) revert InvalidCaller();
        if (initiator != address(this)) revert InvalidCaller();

        // 파라미터 타입에 따라 다른 로직 실행
        if (params.length > 0) {
            // 첫 번째 바이트로 파라미터 타입 구분
            uint8 paramType = uint8(params[0]);
            
            if (paramType == 0x01) {
                // MultiAssetArbitrageParams
                MultiAssetArbitrageParams memory p = abi.decode(params[1:], (MultiAssetArbitrageParams));
                _executeMultiAssetArbitrage(assets, amounts, premiums, p);
            } else if (paramType == 0x02) {
                // TriangularArbitrageParams
                TriangularArbitrageParams memory p = abi.decode(params[1:], (TriangularArbitrageParams));
                _executeTriangularArbitrage(assets, amounts, premiums, p);
            } else if (paramType == 0x03) {
                // PositionMigrationParams
                PositionMigrationParams memory p = abi.decode(params[1:], (PositionMigrationParams));
                _executePositionMigration(assets, amounts, premiums, p);
            }
        }

        // 모든 대출 상환
        for (uint256 i = 0; i < assets.length; i++) {
            uint256 owe = amounts[i] + premiums[i];
            IERC20(assets[i]).safeApprove(address(POOL), 0);
            IERC20(assets[i]).safeApprove(address(POOL), owe);
        }

        return true;
    }

    // ─────────────────────────────
    //     Internal Execution Logic
    // ─────────────────────────────

    function _executeMultiAssetArbitrage(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        MultiAssetArbitrageParams memory params
    ) internal {
        // 복잡한 다중자산 시나리오 실행
        // swapSequences에 따라 순차적으로 스왑 실행
        for (uint256 i = 0; i < params.swapSequences.length; i++) {
            uint256 dexIndex = params.swapSequences[i];
            require(dexIndex < params.dexes.length, "Invalid dex index");
            
            address dex = params.dexes[dexIndex];
            bytes memory callData = params.swapCallData[dexIndex];
            
            (bool success, bytes memory ret) = dex.call(callData);
            if (!success) {
                revert DexCallFailed(dex, callData, ret);
            }
        }

        // 이익 계산 및 검증
        uint256 totalProfit = _calculateTotalProfit(assets, amounts, premiums, params.targetAssets);
        if (totalProfit < params.expectedProfitMin) revert NoProfit();

        emit MultiAssetArbitrageSucceeded(assets, amounts, premiums, totalProfit);
    }

    function _executeTriangularArbitrage(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        TriangularArbitrageParams memory params
    ) internal {
        // 1) A → C 스왑
        uint256 tokenCFromA = _executeSwap(
            params.dexAB,
            params.tokenA,
            params.tokenC,
            amounts[0],
            params.swapCallDataAB
        );

        // 2) B → C 스왑
        uint256 tokenCFromB = _executeSwap(
            params.dexBC,
            params.tokenB,
            params.tokenC,
            amounts[1],
            params.swapCallDataBC
        );

        uint256 totalTokenC = tokenCFromA + tokenCFromB;

        // 3) C → A 스왑 (일부)
        uint256 tokenAFromC = _executeSwap(
            params.dexCA,
            params.tokenC,
            params.tokenA,
            totalTokenC / 2, // 절반을 A로
            params.swapCallDataCA
        );

        // 4) C → B 스왑 (나머지)
        uint256 tokenBFromC = _executeSwap(
            params.dexCB,
            params.tokenC,
            params.tokenB,
            totalTokenC - (totalTokenC / 2), // 나머지를 B로
            params.swapCallDataCB
        );

        // 이익 계산
        uint256 totalProfit = (tokenAFromC - amounts[0] - premiums[0]) + 
                             (tokenBFromC - amounts[1] - premiums[1]);
        
        if (totalProfit < params.expectedProfitMin) revert NoProfit();

        emit TriangularArbitrageSucceeded(
            params.tokenA,
            params.tokenB,
            params.tokenC,
            amounts[0],
            amounts[1],
            totalProfit
        );
    }

    function _executePositionMigration(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        PositionMigrationParams memory params
    ) internal {
        // 포지션 마이그레이션 로직 실행
        for (uint256 i = 0; i < params.migrationDexes.length; i++) {
            address dex = params.migrationDexes[i];
            bytes memory callData = params.migrationCallData[i];
            
            (bool success, bytes memory ret) = dex.call(callData);
            if (!success) {
                revert DexCallFailed(dex, callData, ret);
            }
        }

        // 절약액 계산
        uint256 totalSaving = _calculateMigrationSaving(assets, amounts, premiums, params);
        if (totalSaving < params.expectedSavingMin) revert InsufficientSaving();

        emit PositionMigrationSucceeded(params.debtAssets, params.newDebtAssets, totalSaving);
    }

    // ─────────────────────────────
    //     Internal Swap Helpers
    // ─────────────────────────────

    function _executeSwap(
        address dex,
        address inToken,
        address outToken,
        uint256 amountIn,
        bytes memory callData
    ) internal returns (uint256 amountOut) {
        IERC20 tokenIn = IERC20(inToken);
        IERC20 tokenOut = IERC20(outToken);

        uint256 beforeBal = tokenOut.balanceOf(address(this));

        // approve
        _safeApprove(tokenIn, dex, amountIn);

        // swap
        (bool success, bytes memory ret) = dex.call(callData);
        if (!success) {
            revert DexCallFailed(dex, callData, ret);
        }

        uint256 afterBal = tokenOut.balanceOf(address(this));
        amountOut = afterBal - beforeBal;

        emit SwapExecuted(dex, inToken, outToken, amountIn, amountOut);
    }

    function _calculateTotalProfit(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        address[] memory targetAssets
    ) internal view returns (uint256) {
        uint256 totalProfit = 0;
        
        for (uint256 i = 0; i < targetAssets.length; i++) {
            uint256 currentBalance = IERC20(targetAssets[i]).balanceOf(address(this));
            uint256 borrowedAmount = 0;
            uint256 premium = 0;
            
            // 해당 자산의 대출량과 프리미엄 찾기
            for (uint256 j = 0; j < assets.length; j++) {
                if (assets[j] == targetAssets[i]) {
                    borrowedAmount = amounts[j];
                    premium = premiums[j];
                    break;
                }
            }
            
            if (currentBalance > borrowedAmount + premium) {
                totalProfit += currentBalance - borrowedAmount - premium;
            }
        }
        
        return totalProfit;
    }

    function _calculateMigrationSaving(
        address[] calldata assets,
        uint256[] calldata amounts,
        uint256[] calldata premiums,
        PositionMigrationParams memory params
    ) internal view returns (uint256) {
        // 포지션 마이그레이션 절약액 계산 로직
        // 실제 구현에서는 이전 포지션과 새 포지션의 비용을 비교
        uint256 totalSaving = 0;
        
        // 간단한 예시: 모든 자산의 현재 잔고에서 대출량과 프리미엄을 뺀 값
        for (uint256 i = 0; i < assets.length; i++) {
            uint256 currentBalance = IERC20(assets[i]).balanceOf(address(this));
            uint256 owe = amounts[i] + premiums[i];
            if (currentBalance > owe) {
                totalSaving += currentBalance - owe;
            }
        }
        
        return totalSaving;
    }

    // ─────────────────────────────
    //     View: Profit Estimation
    // ─────────────────────────────

    /**
     * @notice 삼각 아비트래지 수익성 계산
     */
    function calculateTriangularProfitability(TriangularArbitrageParams calldata params)
        external
        view
        returns (uint256 expectedProfit)
    {
        // 1) A → C 견적
        uint256 quoteCFromA = _staticQuote(params.dexAB, params.swapCallDataAB);
        
        // 2) B → C 견적
        uint256 quoteCFromB = _staticQuote(params.dexBC, params.swapCallDataBC);
        
        uint256 totalC = quoteCFromA + quoteCFromB;
        
        // 3) C → A 견적 (절반)
        uint256 quoteAFromC = _staticQuote(params.dexCA, params.swapCallDataCA);
        
        // 4) C → B 견적 (나머지)
        uint256 quoteBFromC = _staticQuote(params.dexCB, params.swapCallDataCB);
        
        // 이익 계산
        if (quoteAFromC > params.amountA && quoteBFromC > params.amountB) {
            expectedProfit = (quoteAFromC - params.amountA) + (quoteBFromC - params.amountB);
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
            return 0;
        }
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
        token.safeApprove(spender, 0);
        token.safeApprove(spender, amount);
    }

    function _assertIsContract(address account) internal view {
        uint256 size;
        assembly { size := extcodesize(account) }
        require(size > 0, "not a contract");
    }
}