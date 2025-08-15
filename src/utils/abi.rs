use anyhow::{Result, anyhow};
use alloy::primitives::{Address, U256, Bytes, FixedBytes, B256, Uint};
use alloy::sol_types::{SolCall, SolEvent, SolValue};
use alloy::sol;
use std::collections::HashMap;
use tracing::{debug, warn};

// Define Solidity interfaces using alloy's sol! macro

// Uniswap V2 Router interface
sol! {
    interface IUniswapV2Router {
        function swapExactTokensForTokens(
            uint amountIn,
            uint amountOutMin,
            address[] calldata path,
            address to,
            uint deadline
        ) external returns (uint[] memory amounts);

        function swapTokensForExactTokens(
            uint amountOut,
            uint amountInMax,
            address[] calldata path,
            address to,
            uint deadline
        ) external returns (uint[] memory amounts);

        function swapExactETHForTokens(
            uint amountOutMin,
            address[] calldata path,
            address to,
            uint deadline
        ) external payable returns (uint[] memory amounts);

        function swapTokensForExactETH(
            uint amountOut,
            uint amountInMax,
            address[] calldata path,
            address to,
            uint deadline
        ) external returns (uint[] memory amounts);

        function getAmountsOut(uint amountIn, address[] calldata path)
            external view returns (uint[] memory amounts);

        function getAmountsIn(uint amountOut, address[] calldata path)
            external view returns (uint[] memory amounts);
    }
}

// Uniswap V3 Router interface
sol! {
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

        struct ExactOutputSingleParams {
            address tokenIn;
            address tokenOut;
            uint24 fee;
            address recipient;
            uint256 deadline;
            uint256 amountOut;
            uint256 amountInMaximum;
            uint160 sqrtPriceLimitX96;
        }

        function exactInputSingle(ExactInputSingleParams calldata params)
            external payable returns (uint256 amountOut);

        function exactOutputSingle(ExactOutputSingleParams calldata params)
            external payable returns (uint256 amountIn);
    }
}

// Aave V3 Pool interface for liquidations
sol! {
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
}

// ERC20 Token interface
sol! {
    interface IERC20 {
        function transfer(address to, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
        function totalSupply() external view returns (uint256);
        function decimals() external view returns (uint8);
        function symbol() external view returns (string);
        function name() external view returns (string);
    }

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
}

// WETH interface
sol! {
    interface IWETH {
        function deposit() external payable;
        function withdraw(uint256 wad) external;
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

/// ABI encoder/decoder for smart contract interactions
pub struct ABICodec {
    /// Pre-computed function selectors for common functions
    function_selectors: HashMap<String, [u8; 4]>,
}

impl ABICodec {
    pub fn new() -> Self {
        let mut function_selectors = HashMap::new();
        
        // Uniswap V2 Router function selectors
        function_selectors.insert(
            "swapExactTokensForTokens".to_string(),
            [0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens(uint256,uint256,address[],address,uint256)
        );
        function_selectors.insert(
            "swapTokensForExactTokens".to_string(),
            [0x88, 0x03, 0xdb, 0xee], // swapTokensForExactTokens(uint256,uint256,address[],address,uint256)
        );
        function_selectors.insert(
            "swapExactETHForTokens".to_string(),
            [0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens(uint256,address[],address,uint256)
        );

        // ERC20 function selectors
        function_selectors.insert(
            "transfer".to_string(),
            [0xa9, 0x05, 0x9c, 0xbb], // transfer(address,uint256)
        );
        function_selectors.insert(
            "approve".to_string(),
            [0x09, 0x5e, 0xa7, 0xb3], // approve(address,uint256)
        );
        function_selectors.insert(
            "balanceOf".to_string(),
            [0x70, 0xa0, 0x82, 0x31], // balanceOf(address)
        );

        // Aave liquidation function selector
        function_selectors.insert(
            "liquidationCall".to_string(),
            [0x00, 0xa7, 0x18, 0xa9], // liquidationCall(address,address,address,uint256,bool)
        );

        Self {
            function_selectors,
        }
    }

    /// Encode Uniswap V2 swap exact tokens for tokens call
    pub fn encode_uniswap_v2_swap_exact_tokens(
        &self,
        amount_in: U256,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    ) -> Result<Bytes> {
        let call = IUniswapV2Router::swapExactTokensForTokensCall {
            amountIn: amount_in,
            amountOutMin: amount_out_min,
            path,
            to,
            deadline,
        };
        
        Ok(call.abi_encode().into())
    }

    /// Encode Uniswap V2 swap ETH for tokens call
    pub fn encode_uniswap_v2_swap_eth_for_tokens(
        &self,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    ) -> Result<Bytes> {
        let call = IUniswapV2Router::swapExactETHForTokensCall {
            amountOutMin: amount_out_min,
            path,
            to,
            deadline,
        };
        
        Ok(call.abi_encode().into())
    }

    /// Encode Uniswap V3 exact input single swap
    pub fn encode_uniswap_v3_exact_input_single(
        &self,
        token_in: Address,
        token_out: Address,
        fee: u32,
        recipient: Address,
        deadline: U256,
        amount_in: U256,
        amount_out_minimum: U256,
        sqrt_price_limit_x96: U256,
    ) -> Result<Bytes> {
        let params = IUniswapV3Router::ExactInputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee: Uint::<24, 1>::from(if fee <= u32::MAX as u32 { fee } else { 3000 }),
            recipient,
            deadline,
            amountIn: amount_in,
            amountOutMinimum: amount_out_minimum,
            sqrtPriceLimitX96: Uint::<160, 3>::from({
                let val = sqrt_price_limit_x96.to::<u128>();
                if val <= u128::from(u64::MAX) {
                    val as u128
                } else {
                    0u128
                }
            }),
        };

        let call = IUniswapV3Router::exactInputSingleCall { params };
        Ok(call.abi_encode().into())
    }

    /// Encode ERC20 transfer call
    pub fn encode_erc20_transfer(&self, to: Address, amount: U256) -> Result<Bytes> {
        let call = IERC20::transferCall { to, amount };
        Ok(call.abi_encode().into())
    }

    /// Encode ERC20 approve call
    pub fn encode_erc20_approve(&self, spender: Address, amount: U256) -> Result<Bytes> {
        let call = IERC20::approveCall { spender, amount };
        Ok(call.abi_encode().into())
    }

    /// Encode Aave liquidation call
    pub fn encode_aave_liquidation(
        &self,
        collateral_asset: Address,
        debt_asset: Address,
        user: Address,
        debt_to_cover: U256,
        receive_a_token: bool,
    ) -> Result<Bytes> {
        let call = IAavePool::liquidationCallCall {
            collateralAsset: collateral_asset,
            debtAsset: debt_asset,
            user,
            debtToCover: debt_to_cover,
            receiveAToken: receive_a_token,
        };
        Ok(call.abi_encode().into())
    }

    /// Decode Uniswap V2 swap transaction
    pub fn decode_uniswap_v2_swap(&self, calldata: &[u8]) -> Result<SwapTransaction> {
        if calldata.len() < 4 {
            return Err(anyhow!("Calldata too short"));
        }

        let function_selector = &calldata[0..4];
        
        if function_selector == self.function_selectors.get("swapExactTokensForTokens").unwrap() {
            // Decode swapExactTokensForTokens
            match IUniswapV2Router::swapExactTokensForTokensCall::abi_decode(&calldata[4..]) {
                Ok(decoded) => {
                    return Ok(SwapTransaction {
                        function_name: "swapExactTokensForTokens".to_string(),
                        amount_in: decoded.amountIn,
                        amount_out_min: decoded.amountOutMin,
                        path: decoded.path,
                        to: decoded.to,
                        deadline: decoded.deadline,
                        is_exact_input: true,
                    });
                }
                Err(e) => {
                    warn!("Failed to decode swapExactTokensForTokens: {}", e);
                }
            }
        } else if function_selector == self.function_selectors.get("swapExactETHForTokens").unwrap() {
            // Decode swapExactETHForTokens
            match IUniswapV2Router::swapExactETHForTokensCall::abi_decode(&calldata[4..]) {
                Ok(decoded) => {
                    return Ok(SwapTransaction {
                        function_name: "swapExactETHForTokens".to_string(),
                        amount_in: U256::ZERO, // Will be set from transaction value
                        amount_out_min: decoded.amountOutMin,
                        path: decoded.path,
                        to: decoded.to,
                        deadline: decoded.deadline,
                        is_exact_input: true,
                    });
                }
                Err(e) => {
                    warn!("Failed to decode swapExactETHForTokens: {}", e);
                }
            }
        }

        Err(anyhow!("Unknown or unsupported function selector"))
    }

    /// Decode ERC20 transfer transaction
    pub fn decode_erc20_transfer(&self, calldata: &[u8]) -> Result<TransferTransaction> {
        if calldata.len() < 4 {
            return Err(anyhow!("Calldata too short"));
        }

        let function_selector = &calldata[0..4];
        
        if function_selector == self.function_selectors.get("transfer").unwrap() {
            match IERC20::transferCall::abi_decode(&calldata[4..]) {
                Ok(decoded) => {
                    return Ok(TransferTransaction {
                        to: decoded.to,
                        amount: decoded.amount,
                    });
                }
                Err(e) => {
                    warn!("Failed to decode transfer: {}", e);
                }
            }
        }

        Err(anyhow!("Not a transfer transaction"))
    }

    /// Decode liquidation transaction
    pub fn decode_aave_liquidation(&self, calldata: &[u8]) -> Result<LiquidationTransaction> {
        if calldata.len() < 4 {
            return Err(anyhow!("Calldata too short"));
        }

        let function_selector = &calldata[0..4];
        
        if function_selector == self.function_selectors.get("liquidationCall").unwrap() {
            match IAavePool::liquidationCallCall::abi_decode(&calldata[4..]) {
                Ok(decoded) => {
                    return Ok(LiquidationTransaction {
                        collateral_asset: decoded.collateralAsset,
                        debt_asset: decoded.debtAsset,
                        user: decoded.user,
                        debt_to_cover: decoded.debtToCover,
                        receive_a_token: decoded.receiveAToken,
                    });
                }
                Err(e) => {
                    warn!("Failed to decode liquidationCall: {}", e);
                }
            }
        }

        Err(anyhow!("Not a liquidation transaction"))
    }

    /// Get function selector for a function name
    pub fn get_function_selector(&self, function_name: &str) -> Option<[u8; 4]> {
        self.function_selectors.get(function_name).copied()
    }

    /// Check if calldata matches a function selector
    pub fn matches_function(&self, calldata: &[u8], function_name: &str) -> bool {
        if calldata.len() < 4 {
            return false;
        }
        
        if let Some(selector) = self.get_function_selector(function_name) {
            &calldata[0..4] == selector
        } else {
            false
        }
    }

    /// Decode event logs
    pub fn decode_transfer_event(&self, log_data: &[u8], topics: &[B256]) -> Result<TransferEvent> {
        if topics.len() < 3 {
            return Err(anyhow!("Not enough topics for Transfer event"));
        }

        // Transfer event signature: Transfer(address indexed from, address indexed to, uint256 value)
        let transfer_signature = B256::from_slice(&[
            0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
            0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x1e, 0x28, 0xec, 0x3b, 0x85, 0xd2, 0x61, 0xd6, 0x9c,
        ]);

        if topics[0] != transfer_signature {
            return Err(anyhow!("Not a Transfer event"));
        }

        let from = Address::from_slice(&topics[1][12..]);
        let to = Address::from_slice(&topics[2][12..]);
        
        if log_data.len() < 32 {
            return Err(anyhow!("Invalid Transfer event data"));
        }
        
        let value = U256::from_be_slice(&log_data[0..32]);

        Ok(TransferEvent { from, to, value })
    }
}

impl Default for ABICodec {
    fn default() -> Self {
        Self::new()
    }
}

/// Decoded swap transaction data
#[derive(Debug, Clone)]
pub struct SwapTransaction {
    pub function_name: String,
    pub amount_in: U256,
    pub amount_out_min: U256,
    pub path: Vec<Address>,
    pub to: Address,
    pub deadline: U256,
    pub is_exact_input: bool,
}

/// Decoded transfer transaction data
#[derive(Debug, Clone)]
pub struct TransferTransaction {
    pub to: Address,
    pub amount: U256,
}

/// Decoded liquidation transaction data
#[derive(Debug, Clone)]
pub struct LiquidationTransaction {
    pub collateral_asset: Address,
    pub debt_asset: Address,
    pub user: Address,
    pub debt_to_cover: U256,
    pub receive_a_token: bool,
}

/// Decoded Transfer event data
#[derive(Debug, Clone)]
pub struct TransferEvent {
    pub from: Address,
    pub to: Address,
    pub value: U256,
}

/// Contract addresses for common protocols
pub mod contracts {
    use alloy::primitives::Address;
    use std::collections::HashMap;
    use once_cell::sync::Lazy;

    pub static UNISWAP_V2_ROUTER: Lazy<Address> = Lazy::new(|| 
        Address::from_slice(&[0x7a, 0x25, 0x0d, 0x56, 0x30, 0xb4, 0xcf, 0x53, 0x97, 0x39, 0xdf, 0x2c, 0x5d, 0xac, 0xb4, 0xc6, 0x59, 0xf2, 0x48, 0x8d]));
    
    pub static UNISWAP_V3_ROUTER: Lazy<Address> = Lazy::new(|| 
        Address::from_slice(&[0xe5, 0x92, 0x42, 0x7a, 0x0a, 0xec, 0xe9, 0x2d, 0xe3, 0xed, 0xee, 0x1f, 0x18, 0xe0, 0x15, 0x7c, 0x05, 0x86, 0x15, 0x64]));
    
    pub static SUSHISWAP_ROUTER: Lazy<Address> = Lazy::new(|| 
        Address::from_slice(&[0xd9, 0xe1, 0xce, 0x17, 0xf2, 0x64, 0x1f, 0x24, 0xae, 0x83, 0x63, 0x7a, 0xb6, 0x6a, 0x2c, 0xca, 0x9c, 0x37, 0x8b, 0x9f]));
    
    pub static WETH_ADDRESS: Lazy<Address> = Lazy::new(|| 
        Address::from_slice(&[0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e, 0x5c, 0x4f, 0x27, 0xea, 0xd9, 0x08, 0x3c, 0x75, 0x6c, 0xc2]));
    
    pub static USDC_ADDRESS: Lazy<Address> = Lazy::new(|| 
        Address::from_slice(&[0xa0, 0xb8, 0x69, 0x91, 0xc6, 0x21, 0x8b, 0x36, 0xc1, 0xd1, 0x9d, 0x4a, 0x2e, 0x9e, 0xb0, 0xce, 0x36, 0x06, 0xeb, 0x48]));
    
    pub static USDT_ADDRESS: Lazy<Address> = Lazy::new(|| 
        Address::from_slice(&[0xda, 0xc1, 0x7f, 0x95, 0x8d, 0x2e, 0xe5, 0x23, 0xa2, 0x20, 0x62, 0x06, 0x99, 0x45, 0x97, 0xc1, 0x3d, 0x83, 0x1e, 0xc7]));
    
    pub static AAVE_V3_POOL: Lazy<Address> = Lazy::new(|| 
        Address::from_slice(&[0x87, 0x87, 0x0b, 0xca, 0x3f, 0x3f, 0xd6, 0x33, 0x54, 0x35, 0x45, 0xf1, 0x5f, 0x80, 0x73, 0xb8, 0xa4, 0x2a, 0xd6, 0xf8]));

    /// Get token address by symbol
    pub static TOKEN_ADDRESSES: Lazy<HashMap<&'static str, Address>> = Lazy::new(|| {
        let mut m = HashMap::new();
        m.insert("WETH", *WETH_ADDRESS);
        m.insert("USDC", *USDC_ADDRESS);
        m.insert("USDT", *USDT_ADDRESS);
        m
    });

    /// Get router address by DEX name
    pub static DEX_ROUTERS: Lazy<HashMap<&'static str, Address>> = Lazy::new(|| {
        let mut m = HashMap::new();
        m.insert("uniswap_v2", *UNISWAP_V2_ROUTER);
        m.insert("uniswap_v3", *UNISWAP_V3_ROUTER);
        m.insert("sushiswap", *SUSHISWAP_ROUTER);
        m
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_uniswap_v2_swap() {
        let codec = ABICodec::new();
        
        let amount_in = U256::from(1000000000000000000u64); // 1 ETH
        let amount_out_min = U256::from(2000000000u64); // 2000 USDC (6 decimals)
        let path = vec![*contracts::WETH_ADDRESS, *contracts::USDC_ADDRESS];
        let to = Address::ZERO;
        let deadline = U256::from(1700000000u64);
        
        let encoded = codec.encode_uniswap_v2_swap_exact_tokens(
            amount_in,
            amount_out_min,
            path,
            to,
            deadline,
        ).unwrap();
        
        assert!(!encoded.is_empty());
        assert_eq!(&encoded[0..4], &[0x38, 0xed, 0x17, 0x39]); // Function selector
    }

    #[test]
    fn test_encode_erc20_transfer() {
        let codec = ABICodec::new();
        
        let to = Address::ZERO;
        let amount = U256::from(1000000000000000000u64); // 1 token
        
        let encoded = codec.encode_erc20_transfer(to, amount).unwrap();
        
        assert!(!encoded.is_empty());
        assert_eq!(&encoded[0..4], &[0xa9, 0x05, 0x9c, 0xbb]); // transfer function selector
    }

    #[test]
    fn test_function_selector_matching() {
        let codec = ABICodec::new();
        
        let transfer_data = [0xa9, 0x05, 0x9c, 0xbb, 0x00, 0x00]; // transfer + dummy data
        
        assert!(codec.matches_function(&transfer_data, "transfer"));
        assert!(!codec.matches_function(&transfer_data, "approve"));
    }
}