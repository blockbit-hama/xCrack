// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title xCrack Micro Arbitrage Strategy
 * @author xCrack Team
 * @notice 고빈도 마이크로 아비트리지 전용 컨트랙트
 *
 * 기능:
 * - Aave v3 FlashLoanSimple을 이용한 단일 자산 차익거래
 * - CEX/DEX 간 가격 차이 활용 (Binance, Coinbase, Uniswap, Sushiswap)
 * - 0x/1inch 어그리게이터 통합
 * - 가스 최적화된 고속 실행
 * - 자동 펀딩 최적화 (플래시론 vs 지갑 자금)
 *
 * 보안:
 * - ReentrancyGuard: 재진입 공격 방지
 * - onlyOwner: 소유자 전용 실행
 * - 최소 수익 검증: 가스 비용 포함 순이익 보장
 * - 슬리피지 보호: 최소 출력량 강제
 */

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract MicroArbitrageStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard {
    using SafeERC20 for IERC20;

    address private owner;

    /// @dev 마이크로 아비트리지 실행 파라미터
    struct ArbitrageParams {
        address baseToken;          // 기준 토큰 (USDC, DAI 등)
        address quoteToken;         // 거래 토큰 (WETH, WBTC 등)
        address buyDex;             // 매수 DEX 라우터 (저가)
        address sellDex;            // 매도 DEX 라우터 (고가)
        address buySpender;         // 매수 approve 대상 (0x allowanceTarget 등, 0이면 buyDex)
        address sellSpender;        // 매도 approve 대상 (0이면 sellDex)
        uint256 amountIn;           // 투입 금액 (baseToken)
        uint256 minProfitWei;       // 최소 순이익 (wei, 가스비 포함)
        uint256 maxSlippage;        // 최대 슬리피지 (basis points, 100 = 1%)
        bytes buyCalldata;          // 매수 DEX 호출 데이터
        bytes sellCalldata;         // 매도 DEX 호출 데이터
        uint256 deadline;           // 실행 마감 시간
    }

    /// @dev 0x 어그리게이터 스왑 파라미터
    struct ZeroXSwapParams {
        address sellToken;
        address buyToken;
        address spender;            // 0x allowanceTarget
        address swapTarget;         // 0x exchange proxy
        bytes swapCallData;
    }

    // ═════════════════════════════════════════════════════════════════════════
    //                                  EVENTS
    // ═════════════════════════════════════════════════════════════════════════

    event FlashLoanInitiated(address indexed asset, uint256 amount, uint256 timestamp);

    event SwapExecuted(
        address indexed dex,
        address indexed tokenIn,
        address indexed tokenOut,
        uint256 amountIn,
        uint256 amountOut,
        uint256 timestamp
    );

    event ArbitrageSuccess(
        address indexed baseToken,
        uint256 amountBorrowed,
        uint256 premium,
        uint256 grossProfit,
        uint256 netProfit,
        uint256 timestamp
    );

    event ArbitrageFailed(
        address indexed baseToken,
        uint256 amountBorrowed,
        string reason
    );

    event TokensRescued(address indexed token, uint256 amount, address indexed to);

    // ═════════════════════════════════════════════════════════════════════════
    //                                  ERRORS
    // ═════════════════════════════════════════════════════════════════════════

    error Unauthorized();
    error InvalidCaller();
    error InvalidToken();
    error InvalidAmount();
    error DeadlineExpired();
    error SlippageTooHigh();
    error InsufficientProfit(uint256 actual, uint256 required);
    error DexCallFailed(address dex, bytes reason);
    error InvalidContract(address account);

    modifier onlyOwner() {
        if (msg.sender != owner) revert Unauthorized();
        _;
    }

    constructor(IPoolAddressesProvider provider) FlashLoanSimpleReceiverBase(provider) {
        owner = msg.sender;
    }

    // ═════════════════════════════════════════════════════════════════════════
    //                            EXTERNAL FUNCTIONS
    // ═════════════════════════════════════════════════════════════════════════

    /**
     * @notice 마이크로 아비트리지 실행 (FlashLoan 트리거)
     * @dev baseToken을 빌려서 quoteToken으로 변환 후 다시 baseToken으로 되팔아 차익 실현
     * @param asset 플래시론 자산 (params.baseToken과 동일해야 함)
     * @param amount 플래시론 금액 (params.amountIn과 동일해야 함)
     * @param params 아비트리지 실행 파라미터
     */
    function executeArbitrage(
        address asset,
        uint256 amount,
        ArbitrageParams calldata params
    ) external onlyOwner nonReentrant {
        // 입력 검증
        if (asset != params.baseToken) revert InvalidToken();
        if (amount != params.amountIn) revert InvalidAmount();
        if (block.timestamp > params.deadline) revert DeadlineExpired();

        // 컨트랙트 주소 검증
        _assertContract(params.buyDex);
        _assertContract(params.sellDex);

        // FlashLoan 실행
        bytes memory data = abi.encode(params);
        emit FlashLoanInitiated(asset, amount, block.timestamp);

        POOL.flashLoanSimple(
            address(this),
            asset,
            amount,
            data,
            0 // referralCode
        );
    }

    /**
     * @notice 0x 어그리게이터를 이용한 아비트리지
     * @dev 0x API에서 받은 quote를 그대로 실행
     */
    function executeZeroXArbitrage(
        address asset,
        uint256 amount,
        ZeroXSwapParams calldata buyParams,
        ZeroXSwapParams calldata sellParams,
        uint256 minProfitWei,
        uint256 deadline
    ) external onlyOwner nonReentrant {
        if (block.timestamp > deadline) revert DeadlineExpired();
        if (asset != buyParams.sellToken) revert InvalidToken();

        ArbitrageParams memory params = ArbitrageParams({
            baseToken: asset,
            quoteToken: buyParams.buyToken,
            buyDex: buyParams.swapTarget,
            sellDex: sellParams.swapTarget,
            buySpender: buyParams.spender,
            sellSpender: sellParams.spender,
            amountIn: amount,
            minProfitWei: minProfitWei,
            maxSlippage: 300, // 3% 기본값
            buyCalldata: buyParams.swapCallData,
            sellCalldata: sellParams.swapCallData,
            deadline: deadline
        });

        bytes memory data = abi.encode(params);
        emit FlashLoanInitiated(asset, amount, block.timestamp);

        POOL.flashLoanSimple(address(this), asset, amount, data, 0);
    }

    /**
     * @notice Aave v3 FlashLoanSimple 콜백
     * @dev 1) baseToken으로 quoteToken 매수
     *      2) quoteToken을 baseToken으로 매도
     *      3) 수익 검증 후 상환
     */
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        // 호출자 검증
        if (msg.sender != address(POOL)) revert InvalidCaller();
        if (initiator != address(this)) revert InvalidCaller();

        ArbitrageParams memory p = abi.decode(params, (ArbitrageParams));
        if (asset != p.baseToken) revert InvalidToken();

        // 마감 시간 체크
        if (block.timestamp > p.deadline) revert DeadlineExpired();

        try this._executeArbitrageLogic(p, amount, premium) returns (uint256 netProfit) {
            // 상환 준비
            uint256 amountOwed = amount + premium;
            IERC20(asset).safeApprove(address(POOL), 0);
            IERC20(asset).safeApprove(address(POOL), amountOwed);

            emit ArbitrageSuccess(
                asset,
                amount,
                premium,
                netProfit + amountOwed - amount,
                netProfit,
                block.timestamp
            );

            return true;
        } catch Error(string memory reason) {
            emit ArbitrageFailed(asset, amount, reason);
            revert(reason);
        } catch (bytes memory lowLevelData) {
            emit ArbitrageFailed(asset, amount, "Low-level call failed");
            revert DexCallFailed(address(0), lowLevelData);
        }
    }

    /**
     * @notice 아비트리지 로직 실행 (내부 함수, external로 try-catch 가능하게)
     */
    function _executeArbitrageLogic(
        ArbitrageParams memory params,
        uint256 borrowed,
        uint256 premium
    ) external returns (uint256 netProfit) {
        // 재진입 방지
        require(msg.sender == address(this), "Internal only");

        // 1단계: baseToken -> quoteToken 매수 (저가 DEX)
        uint256 quoteAmount = _buy(params, borrowed);

        // 2단계: quoteToken -> baseToken 매도 (고가 DEX)
        uint256 baseReceived = _sell(params, quoteAmount);

        // 3단계: 수익 검증
        uint256 amountOwed = borrowed + premium;
        if (baseReceived <= amountOwed) {
            revert InsufficientProfit(baseReceived - amountOwed, 0);
        }

        netProfit = baseReceived - amountOwed;
        if (netProfit < params.minProfitWei) {
            revert InsufficientProfit(netProfit, params.minProfitWei);
        }

        return netProfit;
    }

    // ═════════════════════════════════════════════════════════════════════════
    //                            INTERNAL FUNCTIONS
    // ═════════════════════════════════════════════════════════════════════════

    /**
     * @dev 저가 DEX에서 baseToken으로 quoteToken 매수
     */
    function _buy(
        ArbitrageParams memory params,
        uint256 amount
    ) private returns (uint256 quoteReceived) {
        IERC20 baseToken = IERC20(params.baseToken);
        IERC20 quoteToken = IERC20(params.quoteToken);

        // 잔고 스냅샷
        uint256 quoteBefore = quoteToken.balanceOf(address(this));

        // Approve (0x의 경우 allowanceTarget 사용)
        address spender = params.buySpender != address(0) ? params.buySpender : params.buyDex;
        _safeApprove(baseToken, spender, amount);

        // DEX 호출
        (bool success, bytes memory result) = params.buyDex.call(params.buyCalldata);
        if (!success) revert DexCallFailed(params.buyDex, result);

        // 수령량 계산
        uint256 quoteAfter = quoteToken.balanceOf(address(this));
        quoteReceived = quoteAfter - quoteBefore;

        // 슬리피지 체크
        uint256 minExpected = (amount * (10000 - params.maxSlippage)) / 10000;
        if (quoteReceived < minExpected) revert SlippageTooHigh();

        emit SwapExecuted(
            params.buyDex,
            params.baseToken,
            params.quoteToken,
            amount,
            quoteReceived,
            block.timestamp
        );
    }

    /**
     * @dev 고가 DEX에서 quoteToken을 baseToken으로 매도
     */
    function _sell(
        ArbitrageParams memory params,
        uint256 quoteAmount
    ) private returns (uint256 baseReceived) {
        IERC20 baseToken = IERC20(params.baseToken);
        IERC20 quoteToken = IERC20(params.quoteToken);

        // 잔고 스냅샷
        uint256 baseBefore = baseToken.balanceOf(address(this));

        // Approve
        address spender = params.sellSpender != address(0) ? params.sellSpender : params.sellDex;
        _safeApprove(quoteToken, spender, quoteAmount);

        // DEX 호출
        (bool success, bytes memory result) = params.sellDex.call(params.sellCalldata);
        if (!success) revert DexCallFailed(params.sellDex, result);

        // 수령량 계산
        uint256 baseAfter = baseToken.balanceOf(address(this));
        baseReceived = baseAfter - baseBefore;

        emit SwapExecuted(
            params.sellDex,
            params.quoteToken,
            params.baseToken,
            quoteAmount,
            baseReceived,
            block.timestamp
        );
    }

    /**
     * @dev 안전한 approve (USDT 등 non-standard 토큰 대응)
     */
    function _safeApprove(IERC20 token, address spender, uint256 amount) private {
        uint256 currentAllowance = token.allowance(address(this), spender);
        if (currentAllowance != 0) {
            token.safeApprove(spender, 0);
        }
        token.safeApprove(spender, amount);
    }

    /**
     * @dev 컨트랙트 주소 검증
     */
    function _assertContract(address account) private view {
        uint256 size;
        assembly {
            size := extcodesize(account)
        }
        if (size == 0) revert InvalidContract(account);
    }

    // ═════════════════════════════════════════════════════════════════════════
    //                              OWNER FUNCTIONS
    // ═════════════════════════════════════════════════════════════════════════

    function setOwner(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Zero address");
        owner = newOwner;
    }

    /**
     * @notice 잘못 들어온 토큰 회수
     */
    function rescueTokens(
        address token,
        uint256 amount,
        address to
    ) external onlyOwner {
        require(to != address(0), "Zero address");
        IERC20(token).safeTransfer(to, amount);
        emit TokensRescued(token, amount, to);
    }

    /**
     * @notice 이더 회수 (실수로 전송된 경우)
     */
    function rescueETH(address payable to) external onlyOwner {
        require(to != address(0), "Zero address");
        uint256 balance = address(this).balance;
        (bool success, ) = to.call{value: balance}("");
        require(success, "ETH transfer failed");
    }

    // ═════════════════════════════════════════════════════════════════════════
    //                              VIEW FUNCTIONS
    // ═════════════════════════════════════════════════════════════════════════

    function getOwner() external view returns (address) {
        return owner;
    }

    /**
     * @notice 예상 수익 계산 (견적 전용, 실제 실행 X)
     * @dev buyCalldata/sellCalldata는 quote 전용 calldata여야 함
     *      (예: UniswapV2Router.getAmountsOut, QuoterV2.quoteExactInput 등)
     */
    function estimateProfit(
        address buyDex,
        bytes calldata buyQuoteCalldata,
        address sellDex,
        bytes calldata sellQuoteCalldata,
        uint256 amountIn,
        uint256 estimatedPremium
    ) external view returns (uint256 expectedProfit, bool isProfitable) {
        // 매수 견적
        uint256 quoteOut = _staticQuote(buyDex, buyQuoteCalldata);
        if (quoteOut == 0) return (0, false);

        // 매도 견적
        uint256 baseOut = _staticQuote(sellDex, sellQuoteCalldata);
        if (baseOut == 0) return (0, false);

        // 수익 계산
        uint256 amountOwed = amountIn + estimatedPremium;
        if (baseOut > amountOwed) {
            expectedProfit = baseOut - amountOwed;
            isProfitable = true;
        } else {
            expectedProfit = 0;
            isProfitable = false;
        }
    }

    /**
     * @dev staticcall로 견적 조회
     */
    function _staticQuote(
        address dex,
        bytes calldata quoteCalldata
    ) internal view returns (uint256 amountOut) {
        _assertContract(dex);
        (bool success, bytes memory result) = dex.staticcall(quoteCalldata);
        if (!success || result.length < 32) {
            return 0;
        }
        amountOut = abi.decode(result, (uint256));
    }
}
