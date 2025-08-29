// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

/**
 * @title LiquidationStrategy
 * @dev Advanced liquidation bot with FlashLoan integration
 * @notice Supports Aave v3 and Compound v2/v3 protocols
 */

// Aave v3 Pool interface
interface IAavePool {
    function liquidationCall(
        address collateralAsset,
        address debtAsset,
        address user,
        uint256 debtToCover,
        bool receiveAToken
    ) external;
    
    function getUserAccountData(address user) external view returns (
        uint256 totalCollateralBase,
        uint256 totalDebtBase,
        uint256 availableBorrowsBase,
        uint256 currentLiquidationThreshold,
        uint256 ltv,
        uint256 healthFactor
    );
}

// Compound v2 cToken interface
interface ICToken {
    function liquidateBorrow(
        address borrower,
        uint256 repayAmount,
        address cTokenCollateral
    ) external returns (uint256);
    
    function redeem(uint256 redeemTokens) external returns (uint256);
    function redeemUnderlying(uint256 redeemAmount) external returns (uint256);
    function balanceOf(address owner) external view returns (uint256);
    function underlying() external view returns (address);
    function exchangeRateStored() external view returns (uint256);
}

// Compound v3 Comet interface
interface IComet {
    function absorb(address absorber, address[] calldata accounts) external;
    function getAssetInfo(uint8 i) external view returns (AssetInfo memory);
    function numAssets() external view returns (uint8);
    
    struct AssetInfo {
        uint8 offset;
        address asset;
        address priceFeed;
        uint64 scale;
        uint64 borrowCollateralFactor;
        uint64 liquidateCollateralFactor;
        uint64 liquidationFactor;
        uint128 supplyCap;
    }
}

contract LiquidationStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard, Ownable {
    using SafeERC20 for IERC20;

    // Protocol types
    enum ProtocolType { AAVE, COMPOUND_V2, COMPOUND_V3 }

    // Liquidation parameters
    struct LiquidationParams {
        ProtocolType protocolType;
        address protocol;          // Pool address or cToken address
        address user;              // Target user
        address collateralAsset;   // Collateral token address
        address debtAsset;         // Debt token address
        uint256 debtToCover;       // Amount to repay
        address dexRouter;         // DEX router/aggregator
        bytes swapCalldata;        // Swap execution data
        uint256 minCollateralOut;  // Minimum collateral to receive
        uint256 flashLoanPremium;  // Expected premium (for validation)
    }

    // Events
    event FlashLoanTriggered(
        address indexed asset, 
        uint256 amount, 
        address indexed user,
        ProtocolType protocolType
    );
    
    event AaveLiquidated(
        address indexed user,
        address indexed collateralAsset,
        address indexed debtAsset,
        uint256 debtToCover,
        uint256 liquidationBonus
    );
    
    event CompoundV2Liquidated(
        address indexed user,
        address indexed cTokenBorrowed,
        address indexed cTokenCollateral,
        uint256 repayAmount,
        uint256 seizeTokens
    );
    
    event CompoundV3Absorbed(
        address indexed user,
        address indexed comet,
        uint256 assetsAbsorbed
    );
    
    event CollateralRedeemed(
        address indexed token,
        uint256 amount,
        address indexed underlying,
        uint256 underlyingReceived
    );
    
    event CollateralSwapped(
        address indexed router,
        address indexed tokenIn,
        address indexed tokenOut,
        uint256 amountIn,
        uint256 amountOut,
        uint256 minAmountOut
    );
    
    event FlashLoanRepaid(
        address indexed asset,
        uint256 amount,
        uint256 premium,
        uint256 totalRepaid
    );
    
    event ProfitRealized(
        address indexed asset,
        uint256 profit,
        address indexed user
    );

    // Error messages
    error InsufficientCollateral();
    error SwapFailed();
    error LiquidationFailed();
    error InsufficientProfit();
    error InvalidProtocol();
    error FlashLoanCallbackFailed();

    constructor(IPoolAddressesProvider provider) 
        FlashLoanSimpleReceiverBase(provider) 
        Ownable(msg.sender) 
    {}

    /**
     * @dev Execute liquidation with flash loan
     * @param asset Flash loan asset (debt token)
     * @param amount Flash loan amount
     * @param params Liquidation parameters
     */
    function executeLiquidation(
        address asset,
        uint256 amount,
        LiquidationParams calldata params
    ) external onlyOwner nonReentrant {
        require(amount >= params.debtToCover, "Insufficient flash loan amount");
        require(params.user != address(0), "Invalid user address");
        require(params.protocol != address(0), "Invalid protocol address");
        
        emit FlashLoanTriggered(asset, amount, params.user, params.protocolType);
        
        // Trigger flash loan
        POOL.flashLoanSimple(
            address(this),
            asset,
            amount,
            abi.encode(params),
            0
        );
    }

    /**
     * @dev Flash loan callback - executes liquidation strategy
     */
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        // Validate caller
        require(msg.sender == address(POOL), "Invalid callback caller");
        require(initiator == address(this), "Invalid initiator");
        
        LiquidationParams memory p = abi.decode(params, (LiquidationParams));
        
        // Validate premium matches expectation (±10% tolerance)
        uint256 expectedPremium = p.flashLoanPremium;
        require(
            premium <= expectedPremium * 110 / 100 && 
            premium >= expectedPremium * 90 / 100,
            "Premium outside tolerance"
        );

        try this._executeLiquidationLogic(asset, amount, premium, p) {
            // Success - repay flash loan
            uint256 amountOwed = amount + premium;
            IERC20(asset).safeApprove(address(POOL), amountOwed);
            
            emit FlashLoanRepaid(asset, amount, premium, amountOwed);
            return true;
        } catch Error(string memory reason) {
            // Log error and revert
            revert FlashLoanCallbackFailed();
        }
    }

    /**
     * @dev Internal liquidation logic (external for try/catch)
     */
    function _executeLiquidationLogic(
        address asset,
        uint256 amount,
        uint256 premium,
        LiquidationParams memory params
    ) external {
        require(msg.sender == address(this), "Internal function only");
        
        // Step 1: Execute liquidation based on protocol
        uint256 collateralReceived = _executeLiquidation(params);
        
        // Step 2: Swap collateral for debt asset
        uint256 debtTokensReceived = _executeSwap(
            params.dexRouter,
            params.swapCalldata,
            params.collateralAsset,
            asset,
            collateralReceived,
            params.minCollateralOut
        );
        
        // Step 3: Validate profitability
        uint256 totalOwed = amount + premium;
        require(debtTokensReceived >= totalOwed, "Insufficient profit");
        
        uint256 profit = debtTokensReceived - totalOwed;
        emit ProfitRealized(asset, profit, params.user);
    }

    /**
     * @dev Execute liquidation based on protocol type
     */
    function _executeLiquidation(
        LiquidationParams memory params
    ) internal returns (uint256 collateralReceived) {
        if (params.protocolType == ProtocolType.AAVE) {
            return _executeAaveLiquidation(params);
        } else if (params.protocolType == ProtocolType.COMPOUND_V2) {
            return _executeCompoundV2Liquidation(params);
        } else if (params.protocolType == ProtocolType.COMPOUND_V3) {
            return _executeCompoundV3Liquidation(params);
        } else {
            revert InvalidProtocol();
        }
    }

    /**
     * @dev Execute Aave liquidation
     */
    function _executeAaveLiquidation(
        LiquidationParams memory params
    ) internal returns (uint256 collateralReceived) {
        // Approve debt repayment
        IERC20(params.debtAsset).safeApprove(params.protocol, params.debtToCover);
        
        uint256 collateralBefore = IERC20(params.collateralAsset).balanceOf(address(this));
        
        // Execute liquidation
        IAavePool(params.protocol).liquidationCall(
            params.collateralAsset,
            params.debtAsset,
            params.user,
            params.debtToCover,
            false // Don't receive aTokens
        );
        
        collateralReceived = IERC20(params.collateralAsset).balanceOf(address(this)) - collateralBefore;
        require(collateralReceived > 0, "No collateral received");
        
        emit AaveLiquidated(
            params.user,
            params.collateralAsset,
            params.debtAsset,
            params.debtToCover,
            collateralReceived
        );
    }

    /**
     * @dev Execute Compound v2 liquidation
     */
    function _executeCompoundV2Liquidation(
        LiquidationParams memory params
    ) internal returns (uint256 collateralReceived) {
        address cTokenBorrowed = params.debtAsset;
        address cTokenCollateral = params.collateralAsset;
        
        // Approve repayment to cToken
        address underlying = ICToken(cTokenBorrowed).underlying();
        IERC20(underlying).safeApprove(cTokenBorrowed, params.debtToCover);
        
        // Execute liquidation
        uint256 result = ICToken(cTokenBorrowed).liquidateBorrow(
            params.user,
            params.debtToCover,
            cTokenCollateral
        );
        require(result == 0, "Compound liquidation failed");
        
        // Redeem cTokens for underlying
        uint256 cTokenBalance = ICToken(cTokenCollateral).balanceOf(address(this));
        require(cTokenBalance > 0, "No cTokens received");
        
        uint256 underlyingBefore = IERC20(ICToken(cTokenCollateral).underlying()).balanceOf(address(this));
        
        uint256 redeemResult = ICToken(cTokenCollateral).redeem(cTokenBalance);
        require(redeemResult == 0, "Compound redeem failed");
        
        collateralReceived = IERC20(ICToken(cTokenCollateral).underlying()).balanceOf(address(this)) - underlyingBefore;
        
        emit CompoundV2Liquidated(
            params.user,
            cTokenBorrowed,
            cTokenCollateral,
            params.debtToCover,
            cTokenBalance
        );
        
        emit CollateralRedeemed(
            cTokenCollateral,
            cTokenBalance,
            ICToken(cTokenCollateral).underlying(),
            collateralReceived
        );
    }

    /**
     * @dev Execute Compound v3 liquidation (absorption)
     */
    function _executeCompoundV3Liquidation(
        LiquidationParams memory params
    ) internal returns (uint256 collateralReceived) {
        address[] memory accounts = new address[](1);
        accounts[0] = params.user;
        
        uint256 collateralBefore = IERC20(params.collateralAsset).balanceOf(address(this));
        
        // Absorb underwater account
        IComet(params.protocol).absorb(address(this), accounts);
        
        collateralReceived = IERC20(params.collateralAsset).balanceOf(address(this)) - collateralBefore;
        require(collateralReceived > 0, "No collateral absorbed");
        
        emit CompoundV3Absorbed(params.user, params.protocol, collateralReceived);
    }

    /**
     * @dev Execute swap via DEX aggregator
     */
    function _executeSwap(
        address router,
        bytes memory swapData,
        address tokenIn,
        address tokenOut,
        uint256 amountIn,
        uint256 minAmountOut
    ) internal returns (uint256 amountOut) {
        require(router != address(0), "Invalid router");
        require(_isContract(router), "Router is not a contract");
        require(amountIn > 0, "No tokens to swap");
        
        IERC20 tokenInContract = IERC20(tokenIn);
        IERC20 tokenOutContract = IERC20(tokenOut);
        
        // Reset and approve tokens for swap
        tokenInContract.safeApprove(router, 0);
        tokenInContract.safeApprove(router, amountIn);
        
        uint256 balanceBefore = tokenOutContract.balanceOf(address(this));
        
        // Execute swap
        (bool success, bytes memory returnData) = router.call(swapData);
        require(success, "Swap execution failed");
        
        amountOut = tokenOutContract.balanceOf(address(this)) - balanceBefore;
        require(amountOut >= minAmountOut, "Insufficient swap output");
        
        emit CollateralSwapped(
            router,
            tokenIn,
            tokenOut,
            amountIn,
            amountOut,
            minAmountOut
        );
    }

    /**
     * @dev Emergency functions
     */
    function rescueToken(address token, uint256 amount) external onlyOwner {
        IERC20(token).safeTransfer(owner(), amount);
    }
    
    function rescueETH() external onlyOwner {
        payable(owner()).transfer(address(this).balance);
    }

    /**
     * @dev Utility functions
     */
    function _isContract(address account) internal view returns (bool) {
        uint256 size;
        assembly { size := extcodesize(account) }
        return size > 0;
    }

    // Receive ETH
    receive() external payable {}
}