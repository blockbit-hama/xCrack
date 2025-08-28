// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

interface IERC20 {
    function approve(address spender, uint256 amount) external returns (bool);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);
    function decimals() external view returns (uint8);
    function symbol() external view returns (string memory);
}

interface IAaveV3Pool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

/// Multi-strategy executor for Aave V3 flashLoanSimple that executes:
/// - Liquidation + optional sell (executeLiquidation params)
/// - Sandwich front/back swaps (executeSandwich params)
/// - Arbitrage buy/sell swaps (executeArbitrage params)
/// Then repays the flashloan.
contract FlashLoanStrategyExecutor {
    address public owner;
    address public immutable AAVE_POOL;

    error NotOwner();
    error NotPool();
    error InvalidAsset();
    error DecodeFailed();
    error ExternalCallFailed(bytes data);
    error InsufficientRepay(uint256 have, uint256 need);

    bytes4 constant EXECUTE_LIQ_SELECTOR = bytes4(keccak256(
        "executeLiquidation(address,bytes,address,bytes,address,address,uint256,address,uint256)"
    ));
    bytes4 constant EXECUTE_SANDWICH_SELECTOR = bytes4(keccak256(
        "executeSandwich(address,bytes,bytes,address,uint256)"
    ));
    bytes4 constant EXECUTE_ARBITRAGE_SELECTOR = bytes4(keccak256(
        "executeArbitrage(address,bytes,address,bytes,address,uint256)"
    ));

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    constructor(address aavePool, address initialOwner) {
        require(aavePool != address(0), "pool=0");
        require(initialOwner != address(0), "owner=0");
        AAVE_POOL = aavePool;
        owner = initialOwner;
    }

    function setOwner(address newOwner) external onlyOwner {
        require(newOwner != address(0), "owner=0");
        owner = newOwner;
    }

    /// Aave V3 will call this on the receiver during flashLoanSimple.
    /// The function signature matches IFlashLoanSimpleReceiver, but we avoid importing to keep it minimal.
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address /*initiator*/,
        bytes calldata params
    ) external returns (bool) {
        if (msg.sender != AAVE_POOL) revert NotPool();

        // Params were encoded as a full function call. First 4 bytes are selector.
        if (params.length < 4) revert DecodeFailed();
        bytes4 sel = bytes4(params[:4]);

        if (sel == EXECUTE_LIQ_SELECTOR) {
            (
                address liquidationTarget,
                bytes memory liquidationCalldata,
                address sellTarget,
                bytes memory sellCalldata,
                address sellSpender,
                address debtAsset,
                uint256 debtAmount,
                address collateralAsset,
                uint256 minOut
            ) = abi.decode(params[4:], (address, bytes, address, bytes, address, address, uint256, address, uint256));

            if (asset != debtAsset) revert InvalidAsset();

            // Approve the liquidation target to pull debt (covers Aave Pool.liquidationCall or other targets)
            _safeApprove(debtAsset, liquidationTarget, type(uint256).max);

            // Execute liquidation
            (bool ok, bytes memory data) = liquidationTarget.call(liquidationCalldata);
            if (!ok) revert ExternalCallFailed(data);

            // Optional sell path
            if (sellTarget != address(0) && sellCalldata.length > 0) {
                // Approve spender (0x allowanceTarget) if provided; otherwise approve router/target
                address spender = sellSpender != address(0) ? sellSpender : sellTarget;
                _safeApprove(collateralAsset, spender, type(uint256).max);

                (ok, data) = sellTarget.call(sellCalldata);
                if (!ok) revert ExternalCallFailed(data);
            }

            // Ensure we can repay flashloan
            uint256 need = debtAmount + premium;
            uint256 have = IERC20(debtAsset).balanceOf(address(this));
            if (have < need || have < minOut) revert InsufficientRepay(have, need);

            // Approve pool to pull repayment
            _safeApprove(debtAsset, AAVE_POOL, need);
            return true;
        } else if (sel == EXECUTE_SANDWICH_SELECTOR) {
            (
                address router,
                bytes memory frontCalldata,
                bytes memory backCalldata,
                address assetParam,
                uint256 amountParam
            ) = abi.decode(params[4:], (address, bytes, bytes, address, uint256));

            if (asset != assetParam || amountParam != amount) revert InvalidAsset();

            // Approve router for the borrowed asset
            _safeApprove(asset, router, type(uint256).max);

            // Decode back path[0] to approve its input token as well
            address backInput = _decodeUniswapV2PathInput(backCalldata);
            if (backInput != address(0) && backInput != asset) {
                _safeApprove(backInput, router, type(uint256).max);
            }

            // Execute front run swap
            (bool ok, bytes memory data) = router.call(frontCalldata);
            if (!ok) revert ExternalCallFailed(data);

            // Execute back run swap
            (ok, data) = router.call(backCalldata);
            if (!ok) revert ExternalCallFailed(data);

            // Ensure we can repay flashloan (asset balance >= amount+premium)
            uint256 need = amount + premium;
            uint256 have = IERC20(asset).balanceOf(address(this));
            if (have < need) revert InsufficientRepay(have, need);
            _safeApprove(asset, AAVE_POOL, need);
            return true;
        } else if (sel == EXECUTE_ARBITRAGE_SELECTOR) {
            (
                address routerBuy,
                bytes memory buyCalldata,
                address routerSell,
                bytes memory sellCalldata,
                address assetParam,
                uint256 amountParam
            ) = abi.decode(params[4:], (address, bytes, address, bytes, address, uint256));

            if (asset != assetParam || amountParam != amount) revert InvalidAsset();

            // Approve routers
            _safeApprove(asset, routerBuy, type(uint256).max);
            address sellInput = _decodeUniswapV2PathInput(sellCalldata);
            if (sellInput != address(0) && sellInput != asset) {
                _safeApprove(sellInput, routerSell, type(uint256).max);
            } else {
                _safeApprove(asset, routerSell, type(uint256).max);
            }

            // Execute buy on routerBuy
            (bool ok, bytes memory data) = routerBuy.call(buyCalldata);
            if (!ok) revert ExternalCallFailed(data);

            // Execute sell on routerSell
            (ok, data) = routerSell.call(sellCalldata);
            if (!ok) revert ExternalCallFailed(data);

            // Repay
            uint256 need = amount + premium;
            uint256 have = IERC20(asset).balanceOf(address(this));
            if (have < need) revert InsufficientRepay(have, need);
            _safeApprove(asset, AAVE_POOL, need);
            return true;
        } else {
            revert DecodeFailed();
        }
    }

    function rescue(address token, uint256 amount, address to) external onlyOwner {
        require(to != address(0), "to=0");
        IERC20(token).transfer(to, amount);
    }

    function _safeApprove(address token, address spender, uint256 amount) internal {
        // Reset to 0 first for USDT-like tokens
        IERC20 erc = IERC20(token);
        uint256 curr = erc.allowance(address(this), spender);
        if (curr != 0) {
            require(erc.approve(spender, 0));
        }
        require(erc.approve(spender, amount));
    }

    /// Decode UniswapV2 swapExactTokensForTokens calldata and return input token (path[0])
    function _decodeUniswapV2PathInput(bytes memory callData) internal pure returns (address tokenIn) {
        if (callData.length < 4) return address(0);
        // selector 0x38ed1739 for swapExactTokensForTokens
        bytes4 sel = bytes4(callData[:4]);
        if (sel != 0x38ed1739) return address(0);
        (uint256 /*amountIn*/, uint256 /*amountOutMin*/, address[] memory path, address /*to*/, uint256 /*deadline*/) =
            abi.decode(callData[4:], (uint256, uint256, address[], address, uint256));
        if (path.length == 0) return address(0);
        return path[0];
    }
}
