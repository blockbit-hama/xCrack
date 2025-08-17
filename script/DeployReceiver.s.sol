// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Script.sol";

interface IReceiver {
    function setOwner(address) external;
}

contract DeployReceiver is Script {
    // Mainnet Aave V3 Pool
    address constant AAVE_POOL_MAINNET = 0x87870bca3f3fd633543545f15f8073b8a42ad6f8;

    function run() external {
        uint256 pk = vm.envUint("DEPLOYER_PK");
        address deployer = vm.addr(pk);
        vm.startBroadcast(pk);

        address pool = vm.envOr("AAVE_POOL", AAVE_POOL_MAINNET);
        address owner = vm.envOr("OWNER", deployer);

        bytes memory bytecode = abi.encodePacked(
            type(FlashLoanLiquidationReceiver).creationCode,
            abi.encode(pool, owner)
        );
        address receiver;
        assembly {
            receiver := create(0, add(bytecode, 0x20), mload(bytecode))
        }
        require(receiver != address(0), "deploy failed");

        console2.log("Receiver:", receiver);
        vm.stopBroadcast();
    }
}
