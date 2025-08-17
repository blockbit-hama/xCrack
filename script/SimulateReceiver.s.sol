// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Script.sol";
import {MockERC20} from "contracts/test/MockERC20.sol";

interface IReceiver {
    function executeOperation(address asset, uint256 amount, uint256 premium, address initiator, bytes calldata params) external returns (bool);
}

contract SimulateReceiver is Script {
    function run() external {
        uint256 pk = vm.envUint("DEPLOYER_PK");
        vm.startBroadcast(pk);

        // Deploy mocks
        MockERC20 debt = new MockERC20("USDC", "USDC", 6);
        MockERC20 coll = new MockERC20("WETH", "WETH", 18);

        // Mint debt to receiver to simulate post-sell balance >= amount+premium
        address receiver = vm.envAddress("RECEIVER");
        debt.mint(receiver, 2_000_000e6);

        // Build params payload matching Rust encoder signature
        address liquidationTarget = vm.envAddress("LIQ_TARGET");
        bytes memory liquidationCalldata = hex""; // supply real calldata when integrating
        address sellTarget = address(0);
        bytes memory sellCalldata = hex"";
        address sellSpender = address(0);
        address debtAsset = address(debt);
        uint256 amount = 1_000_000e6;
        address collateralAsset = address(coll);
        uint256 minOut = amount + (amount * 9 / 10000);

        bytes memory encoded = abi.encodeWithSelector(
            bytes4(keccak256("executeLiquidation(address,bytes,address,bytes,address,address,uint256,address,uint256)")),
            liquidationTarget,
            liquidationCalldata,
            sellTarget,
            sellCalldata,
            sellSpender,
            debtAsset,
            amount,
            collateralAsset,
            minOut
        );

        // Call receiver's executeOperation as if from pool
        vm.prank(vm.envAddress("POOL"));
        bool ok = IReceiver(receiver).executeOperation(debtAsset, amount, amount * 9 / 10000, address(this), encoded);
        require(ok, "executeOperation failed");

        vm.stopBroadcast();
    }
}
