// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title Protocol Interfaces
 * @dev Comprehensive interfaces for supported lending protocols
 */

// ===== AAVE V3 INTERFACES =====

interface IAavePoolV3 {
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

    function getReserveData(address asset) external view returns (ReserveData memory);
}

struct ReserveData {
    ReserveConfigurationMap configuration;
    uint128 liquidityIndex;
    uint128 currentLiquidityRate;
    uint128 variableBorrowIndex;
    uint128 currentVariableBorrowRate;
    uint128 currentStableBorrowRate;
    uint40 lastUpdateTimestamp;
    uint16 id;
    address aTokenAddress;
    address stableDebtTokenAddress;
    address variableDebtTokenAddress;
    address interestRateStrategyAddress;
    uint128 accruedToTreasury;
    uint128 unbacked;
    uint128 isolationModeTotalDebt;
}

struct ReserveConfigurationMap {
    uint256 data;
}

interface IAaveProtocolDataProvider {
    function getUserReserveData(address asset, address user) external view returns (
        uint256 currentATokenBalance,
        uint256 currentStableDebt,
        uint256 currentVariableDebt,
        uint256 principalStableDebt,
        uint256 scaledVariableDebt,
        uint256 stableBorrowRate,
        uint256 liquidityRate,
        uint40 stableRateLastUpdated,
        bool usageAsCollateralEnabled
    );

    function getReserveConfigurationData(address asset) external view returns (
        uint256 decimals,
        uint256 ltv,
        uint256 liquidationThreshold,
        uint256 liquidationBonus,
        uint256 reserveFactor,
        bool usageAsCollateralEnabled,
        bool borrowingEnabled,
        bool stableBorrowRateEnabled,
        bool isActive,
        bool isFrozen
    );

    function getReserveEModeCategory(address asset) external view returns (uint256);
}

interface IAavePriceOracle {
    function getAssetPrice(address asset) external view returns (uint256);
    function getAssetsPrices(address[] calldata assets) external view returns (uint256[] memory);
}

// ===== COMPOUND V2 INTERFACES =====

interface IComptroller {
    function getAccountLiquidity(address account) external view returns (
        uint256,  // error code
        uint256,  // liquidity
        uint256   // shortfall
    );

    function liquidationIncentiveMantissa() external view returns (uint256);
    function closeFactorMantissa() external view returns (uint256);
    
    function markets(address cToken) external view returns (
        bool isListed,
        uint256 collateralFactorMantissa,
        bool isComped
    );

    function oracle() external view returns (address);
}

interface ICTokenV2 {
    // Core functions
    function liquidateBorrow(
        address borrower,
        uint256 repayAmount,
        address cTokenCollateral
    ) external returns (uint256);

    function redeem(uint256 redeemTokens) external returns (uint256);
    function redeemUnderlying(uint256 redeemAmount) external returns (uint256);

    // View functions
    function balanceOf(address owner) external view returns (uint256);
    function underlying() external view returns (address);
    function exchangeRateStored() external view returns (uint256);
    function borrowBalanceStored(address account) external view returns (uint256);
    function getAccountSnapshot(address account) external view returns (
        uint256,  // error code
        uint256,  // cToken balance
        uint256,  // borrow balance
        uint256   // exchange rate
    );

    // Metadata
    function symbol() external view returns (string memory);
    function decimals() external view returns (uint8);
}

interface ICompoundPriceOracle {
    function getUnderlyingPrice(address cToken) external view returns (uint256);
}

// ===== COMPOUND V3 INTERFACES =====

interface ICometV3 {
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

    struct UserBasic {
        int104 principal;
        uint64 baseTrackingIndex;
        uint64 baseTrackingAccrued;
        uint16 assetsIn;
        uint8 _reserved;
    }

    struct UserCollateral {
        uint128 balance;
        uint128 _reserved;
    }

    // Core functions
    function absorb(address absorber, address[] calldata accounts) external;
    
    // View functions
    function userBasic(address user) external view returns (UserBasic memory);
    function userCollateral(address user, address asset) external view returns (UserCollateral memory);
    
    function getAssetInfo(uint8 i) external view returns (AssetInfo memory);
    function numAssets() external view returns (uint8);
    function baseToken() external view returns (address);
    
    function isLiquidatable(address account) external view returns (bool);
    function liquidatorPoints(
        address liquidator,
        address borrower,
        address baseToken,
        address collateralAsset,
        uint128 repayAmount,
        uint128 minCollateralAmount
    ) external view returns (uint256);

    // Price and rates
    function getPrice(address priceFeed) external view returns (uint256);
    function getUtilization() external view returns (uint256);
}

// ===== MAKERDAO INTERFACES =====

interface IMakerManager {
    function cdpCan(address, uint256, address) external view returns (uint256);
    function ilks(uint256) external view returns (bytes32);
    function owns(uint256) external view returns (address);
    function urns(uint256) external view returns (address);
    function vat() external view returns (address);
}

interface IMakerVat {
    struct Urn {
        uint256 ink;   // Locked Collateral  [wad]
        uint256 art;   // Normalised Debt    [wad]
    }

    struct Ilk {
        uint256 Art;   // Total Normalised Debt     [wad]
        uint256 rate;  // Accumulated Rates         [ray]
        uint256 spot;  // Price with Safety Margin  [ray]
        uint256 line;  // Debt Ceiling              [rad]
        uint256 dust;  // Urn Debt Floor            [rad]
    }

    function urns(bytes32, address) external view returns (Urn memory);
    function ilks(bytes32) external view returns (Ilk memory);
    function gem(bytes32, address) external view returns (uint256);
}

interface IMakerDog {
    function bark(bytes32, address, address) external returns (uint256);
}

interface IMakerClipper {
    function kick(uint256, uint256, address, address) external returns (uint256);
    function take(uint256, uint256, uint256, address, bytes calldata) external;
}

// ===== DEX AGGREGATOR INTERFACES =====

interface I0xExchangeProxy {
    function transformERC20(
        address inputToken,
        address outputToken,
        uint256 inputTokenAmount,
        uint256 minOutputTokenAmount,
        bytes calldata transformations
    ) external payable returns (uint256 outputTokenAmount);
}

interface I1inchRouter {
    struct SwapDescription {
        address srcToken;
        address dstToken;
        address srcReceiver;
        address dstReceiver;
        uint256 amount;
        uint256 minReturnAmount;
        uint256 flags;
        bytes permit;
    }

    function swap(
        address executor,
        SwapDescription calldata desc,
        bytes calldata permit,
        bytes calldata data
    ) external payable returns (uint256 returnAmount, uint256 spentAmount);
}

// ===== UNISWAP INTERFACES =====

interface IUniswapV3Router {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }

    function exactInputSingle(ExactInputSingleParams calldata params) external returns (uint256 amountOut);
}

interface IUniswapV2Router {
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);

    function getAmountsOut(uint256 amountIn, address[] calldata path) external view returns (uint256[] memory amounts);
}