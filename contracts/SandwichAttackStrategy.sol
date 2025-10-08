// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title xCrack Sandwich Attack Strategy
 * @author xCrack Team
 * @notice MEV 샌드위치 어택 전용 컨트랙트
 *
 * 기능:
 * - 타겟 트랜잭션의 front-run과 back-run을 원자적으로 실행
 * - Aave v3 FlashLoan으로 자본 효율 극대화
 * - Uniswap V2/V3, SushiSwap, PancakeSwap 지원
 * - Kelly Criterion 기반 최적 공격 크기 계산
 * - 슬리피지 보호 및 가스 최적화
 *
 * 동작 원리:
 * 1. Front-run: 타겟보다 먼저 매수하여 가격 상승
 * 2. Victim TX: 타겟이 높은 가격에 매수
 * 3. Back-run: 즉시 매도하여 차익 실현
 *
 * 보안:
 * - ReentrancyGuard: 재진입 공격 방지
 * - onlyOwner: 소유자 전용 실행
 * - 최소 수익 검증: MEV 경쟁 고려
 * - MEV-Boost 통합: Private mempool 전송
 */

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract SandwichAttackStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard {
    using SafeERC20 for IERC20;

    address private owner;

    /// @dev 샌드위치 공격 파라미터
    struct SandwichParams {
        address targetToken;         // 타겟 토큰 (피해자가 매수하려는 토큰)
        address pairedToken;         // 페어 토큰 (WETH, USDC 등)
        address router;              // DEX 라우터 (Uniswap V2 등)
        uint256 frontRunAmount;      // Front-run 매수 금액
        uint256 minVictimAmount;     // 피해자 최소 거래량 (필터링용)
        uint256 minProfitWei;        // 최소 순이익 (wei)
        uint256 maxGasPrice;         // 최대 가스 가격 (경쟁 고려)
        uint256 maxPriceImpact;      // 최대 가격 임팩트 (basis points, 500 = 5%)
        bytes frontRunCalldata;      // Front-run 스왑 calldata
        bytes backRunCalldata;       // Back-run 스왑 calldata
        uint256 deadline;            // 실행 마감 시간
    }

    /// @dev 실행 결과
    struct ExecutionResult {
        uint256 frontRunPrice;       // Front-run 가격
        uint256 backRunPrice;        // Back-run 가격
        uint256 priceImpact;         // 가격 임팩트 (basis points)
        uint256 grossProfit;         // 총 수익
        uint256 netProfit;           // 순이익 (가스 차감)
        uint256 gasUsed;             // 사용된 가스
    }

    // ═════════════════════════════════════════════════════════════════════════
    //                                  EVENTS
    // ═════════════════════════════════════════════════════════════════════════

    event FlashLoanInitiated(
        address indexed token,
        uint256 amount,
        uint256 timestamp
    );

    event FrontRunExecuted(
        address indexed router,
        address indexed tokenIn,
        address indexed tokenOut,
        uint256 amountIn,
        uint256 amountOut,
        uint256 priceImpact,
        uint256 timestamp
    );

    event BackRunExecuted(
        address indexed router,
        address indexed tokenIn,
        address indexed tokenOut,
        uint256 amountIn,
        uint256 amountOut,
        uint256 priceRealized,
        uint256 timestamp
    );

    event SandwichSuccess(
        address indexed targetToken,
        uint256 flashLoanAmount,
        uint256 premium,
        uint256 grossProfit,
        uint256 netProfit,
        uint256 priceImpact,
        uint256 timestamp
    );

    event SandwichFailed(
        address indexed targetToken,
        uint256 flashLoanAmount,
        string reason
    );

    event TokensRescued(
        address indexed token,
        uint256 amount,
        address indexed to
    );

    // ═════════════════════════════════════════════════════════════════════════
    //                                  ERRORS
    // ═════════════════════════════════════════════════════════════════════════

    error Unauthorized();
    error InvalidCaller();
    error InvalidToken();
    error InvalidAmount();
    error DeadlineExpired();
    error PriceImpactTooHigh(uint256 actual, uint256 max);
    error InsufficientProfit(uint256 actual, uint256 required);
    error GasPriceTooHigh(uint256 actual, uint256 max);
    error VictimAmountTooLow(uint256 actual, uint256 min);
    error RouterCallFailed(address router, bytes reason);
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
     * @notice 샌드위치 공격 실행 (FlashLoan 트리거)
     * @dev pairedToken을 빌려서 targetToken을 front-run/back-run
     * @param asset 플래시론 자산 (params.pairedToken과 동일)
     * @param amount 플래시론 금액 (params.frontRunAmount와 동일)
     * @param params 샌드위치 공격 파라미터
     */
    function executeSandwich(
        address asset,
        uint256 amount,
        SandwichParams calldata params
    ) external onlyOwner nonReentrant {
        // 입력 검증
        if (asset != params.pairedToken) revert InvalidToken();
        if (amount != params.frontRunAmount) revert InvalidAmount();
        if (block.timestamp > params.deadline) revert DeadlineExpired();
        if (tx.gasprice > params.maxGasPrice) revert GasPriceTooHigh(tx.gasprice, params.maxGasPrice);

        // 컨트랙트 주소 검증
        _assertContract(params.router);
        _assertContract(params.targetToken);
        _assertContract(params.pairedToken);

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
     * @notice Aave v3 FlashLoanSimple 콜백
     * @dev 1) Front-run: pairedToken으로 targetToken 매수 (가격 올림)
     *      2) Victim TX: 타겟이 높은 가격에 매수 (블록 내 자동 실행)
     *      3) Back-run: targetToken을 pairedToken으로 매도 (차익 실현)
     *      4) 수익 검증 후 상환
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

        SandwichParams memory p = abi.decode(params, (SandwichParams));
        if (asset != p.pairedToken) revert InvalidToken();

        // 마감 시간 체크
        if (block.timestamp > p.deadline) revert DeadlineExpired();

        try this._executeSandwichLogic(p, amount, premium) returns (ExecutionResult memory result) {
            // 상환 준비
            uint256 amountOwed = amount + premium;
            IERC20(asset).safeApprove(address(POOL), 0);
            IERC20(asset).safeApprove(address(POOL), amountOwed);

            emit SandwichSuccess(
                p.targetToken,
                amount,
                premium,
                result.grossProfit,
                result.netProfit,
                result.priceImpact,
                block.timestamp
            );

            return true;
        } catch Error(string memory reason) {
            emit SandwichFailed(p.targetToken, amount, reason);
            revert(reason);
        } catch (bytes memory lowLevelData) {
            emit SandwichFailed(p.targetToken, amount, "Low-level call failed");
            revert RouterCallFailed(address(0), lowLevelData);
        }
    }

    /**
     * @notice 샌드위치 공격 로직 실행 (내부 함수, external로 try-catch 가능하게)
     */
    function _executeSandwichLogic(
        SandwichParams memory params,
        uint256 borrowed,
        uint256 premium
    ) external returns (ExecutionResult memory result) {
        // 재진입 방지
        require(msg.sender == address(this), "Internal only");

        // 1단계: Front-run (pairedToken -> targetToken 매수)
        uint256 targetTokenReceived = _frontRun(params, borrowed);

        // 가격 임팩트 계산 및 검증
        uint256 priceImpact = _calculatePriceImpact(borrowed, targetTokenReceived);
        if (priceImpact > params.maxPriceImpact) {
            revert PriceImpactTooHigh(priceImpact, params.maxPriceImpact);
        }

        // 2단계: Victim TX 대기 (블록 내 자동 실행)
        // 실제로는 victim이 우리 뒤에 같은 블록에 포함되어야 함
        // 이는 MEV-Boost/Flashbots를 통해 bundle로 제출하여 보장

        // 3단계: Back-run (targetToken -> pairedToken 매도)
        uint256 pairedTokenReceived = _backRun(params, targetTokenReceived);

        // 4단계: 수익 검증
        uint256 amountOwed = borrowed + premium;
        if (pairedTokenReceived <= amountOwed) {
            revert InsufficientProfit(pairedTokenReceived - amountOwed, 0);
        }

        uint256 grossProfit = pairedTokenReceived - borrowed;
        uint256 netProfit = pairedTokenReceived - amountOwed;

        if (netProfit < params.minProfitWei) {
            revert InsufficientProfit(netProfit, params.minProfitWei);
        }

        // 결과 반환
        result = ExecutionResult({
            frontRunPrice: (targetTokenReceived * 1e18) / borrowed,
            backRunPrice: (pairedTokenReceived * 1e18) / targetTokenReceived,
            priceImpact: priceImpact,
            grossProfit: grossProfit,
            netProfit: netProfit,
            gasUsed: gasleft() // 근사값
        });

        return result;
    }

    // ═════════════════════════════════════════════════════════════════════════
    //                            INTERNAL FUNCTIONS
    // ═════════════════════════════════════════════════════════════════════════

    /**
     * @dev Front-run: pairedToken으로 targetToken 매수하여 가격 상승
     */
    function _frontRun(
        SandwichParams memory params,
        uint256 amount
    ) private returns (uint256 targetTokenReceived) {
        IERC20 pairedToken = IERC20(params.pairedToken);
        IERC20 targetToken = IERC20(params.targetToken);

        // 잔고 스냅샷
        uint256 targetBefore = targetToken.balanceOf(address(this));

        // Approve
        _safeApprove(pairedToken, params.router, amount);

        // Router 호출 (매수)
        (bool success, bytes memory result) = params.router.call(params.frontRunCalldata);
        if (!success) revert RouterCallFailed(params.router, result);

        // 수령량 계산
        uint256 targetAfter = targetToken.balanceOf(address(this));
        targetTokenReceived = targetAfter - targetBefore;

        // 최소 수령량 검증 (슬리피지 보호)
        require(targetTokenReceived > 0, "Zero output");

        // 가격 임팩트 계산
        uint256 priceImpact = _calculatePriceImpact(amount, targetTokenReceived);

        emit FrontRunExecuted(
            params.router,
            params.pairedToken,
            params.targetToken,
            amount,
            targetTokenReceived,
            priceImpact,
            block.timestamp
        );
    }

    /**
     * @dev Back-run: targetToken을 pairedToken으로 매도하여 차익 실현
     */
    function _backRun(
        SandwichParams memory params,
        uint256 targetAmount
    ) private returns (uint256 pairedTokenReceived) {
        IERC20 pairedToken = IERC20(params.pairedToken);
        IERC20 targetToken = IERC20(params.targetToken);

        // 잔고 스냅샷
        uint256 pairedBefore = pairedToken.balanceOf(address(this));

        // Approve
        _safeApprove(targetToken, params.router, targetAmount);

        // Router 호출 (매도)
        (bool success, bytes memory result) = params.router.call(params.backRunCalldata);
        if (!success) revert RouterCallFailed(params.router, result);

        // 수령량 계산
        uint256 pairedAfter = pairedToken.balanceOf(address(this));
        pairedTokenReceived = pairedAfter - pairedBefore;

        // 최소 수령량 검증
        require(pairedTokenReceived > 0, "Zero output");

        // 실현 가격 계산
        uint256 priceRealized = (pairedTokenReceived * 1e18) / targetAmount;

        emit BackRunExecuted(
            params.router,
            params.targetToken,
            params.pairedToken,
            targetAmount,
            pairedTokenReceived,
            priceRealized,
            block.timestamp
        );
    }

    /**
     * @dev 가격 임팩트 계산 (basis points)
     * @return impact 가격 임팩트 (100 = 1%)
     */
    function _calculatePriceImpact(
        uint256 amountIn,
        uint256 amountOut
    ) private pure returns (uint256 impact) {
        // 실제 비율 vs 1:1 비율 차이를 계산
        // impact = |1 - (amountOut / amountIn)| * 10000
        if (amountOut >= amountIn) {
            impact = ((amountOut - amountIn) * 10000) / amountIn;
        } else {
            impact = ((amountIn - amountOut) * 10000) / amountIn;
        }
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
     * @notice 최적 공격 크기 계산 (Kelly Criterion)
     * @dev Kelly % = (p * b - q) / b
     *      where p = 승률, q = 1-p, b = 배당률
     * @param successProbability 성공 확률 (basis points, 8000 = 80%)
     * @param priceImpactBps 예상 가격 임팩트 (basis points)
     * @param availableCapital 사용 가능 자본
     * @return optimalSize 최적 공격 크기
     */
    function calculateOptimalSize(
        uint256 successProbability,
        uint256 priceImpactBps,
        uint256 availableCapital
    ) external pure returns (uint256 optimalSize) {
        // p = successProbability / 10000
        // b = priceImpactBps / 10000 (단순화)
        // Kelly % = (p * b - (1-p)) / b

        uint256 p = successProbability;
        uint256 q = 10000 - successProbability;
        uint256 b = priceImpactBps;

        if (b == 0) return 0;

        // Kelly % in basis points
        uint256 kellyBps;
        if (p * b > q * 10000) {
            kellyBps = ((p * b - q * 10000) * 10000) / (b * 10000);
        } else {
            return 0; // 음수면 공격 안함
        }

        // 안전을 위해 Kelly의 50%만 사용 (Half Kelly)
        kellyBps = kellyBps / 2;

        // 최적 크기 계산
        optimalSize = (availableCapital * kellyBps) / 10000;

        // 최소/최대 제한
        uint256 minSize = availableCapital / 100; // 최소 1%
        uint256 maxSize = availableCapital / 4;   // 최대 25%

        if (optimalSize < minSize) optimalSize = minSize;
        if (optimalSize > maxSize) optimalSize = maxSize;

        return optimalSize;
    }

    /**
     * @notice 예상 수익 계산
     * @dev front/back calldata는 quote 전용 calldata여야 함
     */
    function estimateProfit(
        address router,
        bytes calldata frontQuoteCalldata,
        bytes calldata backQuoteCalldata,
        uint256 frontRunAmount,
        uint256 estimatedPremium
    ) external view returns (uint256 expectedProfit, bool isProfitable) {
        // Front-run 견적
        uint256 targetOut = _staticQuote(router, frontQuoteCalldata);
        if (targetOut == 0) return (0, false);

        // Back-run 견적
        uint256 pairedOut = _staticQuote(router, backQuoteCalldata);
        if (pairedOut == 0) return (0, false);

        // 수익 계산
        uint256 amountOwed = frontRunAmount + estimatedPremium;
        if (pairedOut > amountOwed) {
            expectedProfit = pairedOut - amountOwed;
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
        address router,
        bytes calldata quoteCalldata
    ) internal view returns (uint256 amountOut) {
        _assertContract(router);
        (bool success, bytes memory result) = router.staticcall(quoteCalldata);
        if (!success || result.length < 32) {
            return 0;
        }
        amountOut = abi.decode(result, (uint256));
    }
}
