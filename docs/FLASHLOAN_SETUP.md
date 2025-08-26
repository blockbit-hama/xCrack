# Flashloan Setup (Aave V3 + Receiver)

This guide explains how to deploy the unified flashloan receiver and connect strategies (Sandwich, Liquidation, Micro Arbitrage) to use flashLoanSimple.

## 1) Prerequisites
- Foundry installed (forge, cast)
- Deployer EOA with ETH on the target network
- Aave V3 Pool address (default mainnet provided)

## 2) Deploy receiver

```bash
export DEPLOYER_PK=0x... # your private key (use a .env in practice)
# optional overrides
export AAVE_POOL=0x87870bca3f3fd633543545f15f8073b8a42ad6f8
export OWNER=0xYourEOA

forge script script/DeployReceiver.s.sol:DeployReceiver \
  --rpc-url $RPC_URL \
  --broadcast \
  -vvvv
```
Copy the printed `Receiver` address and set it to config.

## 3) Configure backend

Edit `config/default.toml`:

```toml
[blockchain.primary_network]
flashloan_receiver = "0xReceiverAddressHere"
```

Optionally enable flashloan per strategy:

```toml
[strategies.sandwich]
use_flashloan = true

[strategies.micro_arbitrage]
use_flashloan = true

[strategies.cross_chain_arbitrage]
use_flashloan = false # to be enabled when wired
```

Restart the backend after saving params via UI or editing the file.

## 4) Strategy specifics

- Sandwich: builds a single Aave flashLoanSimple tx; the receiver executes frontrun/backrun and repays.
- Liquidation: already uses liquidation path with optional 0x/1inch sell inside the receiver.
- Micro Arbitrage (DEX-DEX): when enabled and both venues are UniswapV2/Sushiswap, encodes buy/sell into the receiver (scaffold). Submission path reuses the common bundle flow.
- Cross-Chain Arbitrage: planned to use source-side flashloan for bridge + dest swap.

## 5) Safety and slippage
- Set conservative `min_out` / `amountOutMin` (TODO hooks are placed in code).
- Keep `flash_loan_amount` small for first runs and monitor via `/logs`.
- Prefer simulation mode before enabling real submission.

## 6) Troubleshooting
- If UI shows external API cards empty, refresh and verify env.
- If `flashloan_receiver` is zero, strategies will fall back to legacy paths.
- Receiver revert: check calldata encoding and token approvals; try smaller amounts.
