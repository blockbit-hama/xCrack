// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/**
 * @title xCrack Sandwich Strategy (FlashLoan + Front/Back swaps)
 * @notice Executes frontrun/backrun swaps atomically using Aave v3 FlashLoanSimple.
 *         Router calls are provided as raw calldata to support multiple DEXes (UniV2 compatible).
 */

import "@aave/core-v3/contracts/interfaces/IPoolAddressesProvider.sol";
import "@aave/core-v3/contracts/interfaces/IPool.sol";
import "@aave/core-v3/contracts/flashloan/base/FlashLoanSimpleReceiverBase.sol";

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract SandwichStrategy is FlashLoanSimpleReceiverBase, ReentrancyGuard {
    using SafeERC20 for IERC20;

    address private owner;

    struct SandwichParams {
        address router;
        bytes   frontCalldata;
        bytes   backCalldata;
        address asset;
        uint256 amount;
    }

    event FlashLoanRequested(address indexed asset, uint256 amount);
    event SwapExecuted(address indexed router, uint256 direction); // 1=front,2=back
    event Repaid(address indexed asset, uint256 amountOwed);
    event Rescue(address indexed token, uint256 amount, address to);

    error NotAuthorized();
    error InvalidCaller();
    error BadAsset();
    error DexCallFailed(address dex, bytes data, bytes reason);
    error InsufficientRepay(uint256 have, uint256 need);

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotAuthorized();
        _;
    }

    constructor(IPoolAddressesProvider provider) FlashLoanSimpleReceiverBase(provider) {
        owner = msg.sender;
    }

    function executeSandwich(address asset, uint256 amount, SandwichParams calldata params)
        external
        onlyOwner
        nonReentrant
    {
        if (asset != params.asset) revert BadAsset();
        if (amount != params.amount) revert BadAsset();
        _assertIsContract(params.router);

        bytes memory data = abi.encode(params);
        emit FlashLoanRequested(asset, amount);
        POOL.flashLoanSimple(address(this), asset, amount, data, 0);
    }

    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        if (msg.sender != address(POOL)) revert InvalidCaller();
        if (initiator != address(this)) revert InvalidCaller();

        SandwichParams memory p = abi.decode(params, (SandwichParams));
        if (asset != p.asset) revert BadAsset();

        // Approve router to spend asset and potential intermediate token from back path
        IERC20(asset).safeApprove(p.router, 0);
        IERC20(asset).safeApprove(p.router, amount);

        // Execute front run swap
        (bool ok, bytes memory ret) = p.router.call(p.frontCalldata);
        if (!ok) revert DexCallFailed(p.router, p.frontCalldata, ret);
        emit SwapExecuted(p.router, 1);

        // Execute back run swap
        (ok, ret) = p.router.call(p.backCalldata);
        if (!ok) revert DexCallFailed(p.router, p.backCalldata, ret);
        emit SwapExecuted(p.router, 2);

        // Repay
        uint256 owe = amount + premium;
        uint256 bal = IERC20(asset).balanceOf(address(this));
        if (bal < owe) revert InsufficientRepay(bal, owe);
        IERC20(asset).safeApprove(address(POOL), 0);
        IERC20(asset).safeApprove(address(POOL), owe);
        emit Repaid(asset, owe);
        return true;
    }

    function setOwner(address newOwner) external onlyOwner {
        owner = newOwner;
    }

    function rescue(address token, uint256 amount, address to) external onlyOwner {
        IERC20(token).safeTransfer(to, amount);
        emit Rescue(token, amount, to);
    }

    function _assertIsContract(address account) internal view {
        uint256 size;
        assembly { size := extcodesize(account) }
        require(size > 0, "not a contract");
    }
}
