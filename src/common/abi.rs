use ethers::types::{Address, U256, Bytes, H256};
use ethers::abi::{encode, Token, Param, ParamType};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use tracing::warn;

/// Helper function to convert U256 to 32-byte big-endian array
pub fn u256_to_be_bytes(value: U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    value.to_big_endian(&mut bytes);
    bytes
}

/// Helper function to convert U256 to 32-byte little-endian array
pub fn u256_to_le_bytes(value: U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    value.to_little_endian(&mut bytes);
    bytes
}

/// Helper function to convert ethers U256 internal representation to U256
/// Note: ethers U256 is stored as [u64; 4] in little-endian order
pub fn u256_from_ethers_internal(internal: [u64; 4]) -> U256 {
    U256(internal)
}

/// Helper function to convert f64 to U256 (assuming 18 decimals)
pub fn u256_from_f64(value: f64) -> U256 {
    let value_scaled = (value * 1e18) as u128;
    U256::from(value_scaled)
}

/// ABI encoder/decoder for smart contract interactions using ethers
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

        // Compound liquidate selector
        function_selectors.insert(
            "liquidate".to_string(),
            [0x4c, 0x0b, 0x5b, 0x3e],
        );

        // Maker bark selector
        function_selectors.insert(
            "bark".to_string(),
            [0x1d, 0x26, 0x3b, 0x3c],
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
        let selector = self.function_selectors.get("swapExactTokensForTokens").unwrap();

        let tokens = vec![
            Token::Uint(amount_in),
            Token::Uint(amount_out_min),
            Token::Array(path.into_iter().map(Token::Address).collect()),
            Token::Address(to),
            Token::Uint(deadline),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode Uniswap V2 swap ETH for tokens call
    pub fn encode_uniswap_v2_swap_eth_for_tokens(
        &self,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    ) -> Result<Bytes> {
        let selector = self.function_selectors.get("swapExactETHForTokens").unwrap();

        let tokens = vec![
            Token::Uint(amount_out_min),
            Token::Array(path.into_iter().map(Token::Address).collect()),
            Token::Address(to),
            Token::Uint(deadline),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode Uniswap V2 swap tokens for exact tokens call
    pub fn encode_uniswap_v2_swap_tokens_for_exact_tokens(
        &self,
        amount_out: U256,
        amount_in_max: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    ) -> Result<Bytes> {
        let selector = self.function_selectors.get("swapTokensForExactTokens").unwrap();

        let tokens = vec![
            Token::Uint(amount_out),
            Token::Uint(amount_in_max),
            Token::Array(path.into_iter().map(Token::Address).collect()),
            Token::Address(to),
            Token::Uint(deadline),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode ERC20 transfer call
    pub fn encode_erc20_transfer(&self, to: Address, amount: U256) -> Result<Bytes> {
        let selector = self.function_selectors.get("transfer").unwrap();

        let tokens = vec![
            Token::Address(to),
            Token::Uint(amount),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode ERC20 approve call
    pub fn encode_erc20_approve(&self, spender: Address, amount: U256) -> Result<Bytes> {
        let selector = self.function_selectors.get("approve").unwrap();

        let tokens = vec![
            Token::Address(spender),
            Token::Uint(amount),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
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
        let selector = self.function_selectors.get("liquidationCall").unwrap();

        let tokens = vec![
            Token::Address(collateral_asset),
            Token::Address(debt_asset),
            Token::Address(user),
            Token::Uint(debt_to_cover),
            Token::Bool(receive_a_token),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode Aave flashLoanSimple call
    pub fn encode_aave_flashloan_simple(
        &self,
        receiver: Address,
        asset: Address,
        amount: U256,
        params: Bytes,
        referral_code: u16,
    ) -> Result<Bytes> {
        // flashLoanSimple selector: 0xab9c4b5d
        let selector = [0xab, 0x9c, 0x4b, 0x5d];

        let tokens = vec![
            Token::Address(receiver),
            Token::Address(asset),
            Token::Uint(amount),
            Token::Bytes(params.to_vec()),
            Token::Uint(U256::from(referral_code)),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode Compound liquidation call
    pub fn encode_compound_liquidation(
        &self,
        borrower: Address,
        collateral_asset: Address,
        base_amount: U256,
    ) -> Result<Bytes> {
        let selector = self.function_selectors.get("liquidate").unwrap();

        let tokens = vec![
            Token::Address(borrower),
            Token::Address(collateral_asset),
            Token::Uint(base_amount),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode Maker bark (liquidation)
    pub fn encode_maker_bark(
        &self,
        ilk: [u8; 32],
        urn: Address,
        keeper: Address,
    ) -> Result<Bytes> {
        let selector = self.function_selectors.get("bark").unwrap();

        let tokens = vec![
            Token::FixedBytes(ilk.to_vec()),
            Token::Address(urn),
            Token::Address(keeper),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
    }

    /// Encode arbitrage contract parameters
    pub fn encode_arbitrage_contract_params(
        &self,
        token_in: Address,
        token_out: Address,
        dex_a: Address,
        dex_b: Address,
        spender_a: Option<Address>,
        spender_b: Option<Address>,
        amount_in: U256,
        expected_min: U256,
        data_a: Bytes,
        data_b: Bytes,
    ) -> Result<Bytes> {
        let tokens = vec![
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Address(dex_a),
            Token::Address(dex_b),
            Token::Address(spender_a.unwrap_or_else(Address::zero)),
            Token::Address(spender_b.unwrap_or_else(Address::zero)),
            Token::Uint(amount_in),
            Token::Uint(expected_min),
            Token::Bytes(data_a.to_vec()),
            Token::Bytes(data_b.to_vec()),
        ];

        let encoded = encode(&tokens);
        Ok(Bytes::from(encoded))
    }

    /// Encode arbitrage execute call
    pub fn encode_arbitrage_execute_call(
        &self,
        token_in: Address,
        amount_in: U256,
        params: Bytes,
    ) -> Result<Bytes> {
        // executeArbitrage selector: 0x12345678 (placeholder - should be actual selector)
        let selector = [0x12, 0x34, 0x56, 0x78];

        let tokens = vec![
            Token::Address(token_in),
            Token::Uint(amount_in),
            Token::Bytes(params.to_vec()),
        ];

        let encoded = encode(&tokens);
        let mut calldata = selector.to_vec();
        calldata.extend_from_slice(&encoded);

        Ok(Bytes::from(calldata))
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

    /// Decode Transfer event
    pub fn decode_transfer_event(&self, log_data: &[u8], topics: &[H256]) -> Result<TransferEvent> {
        if topics.len() < 3 {
            return Err(anyhow!("Not enough topics for Transfer event"));
        }

        // Transfer event signature: Transfer(address indexed from, address indexed to, uint256 value)
        let transfer_signature = H256::from_slice(&[
            0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa,
            0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x1e, 0x28, 0xec, 0x3b, 0x85, 0xd2, 0x61, 0xd6, 0x9c,
        ]);

        if topics[0] != transfer_signature {
            return Err(anyhow!("Not a Transfer event"));
        }

        let from = Address::from_slice(&topics[1].as_bytes()[12..]);
        let to = Address::from_slice(&topics[2].as_bytes()[12..]);

        if log_data.len() < 32 {
            return Err(anyhow!("Invalid Transfer event data"));
        }

        let value = U256::from_big_endian(&log_data[0..32]);

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
    use ethers::types::Address;
    use std::collections::HashMap;
    use once_cell::sync::Lazy;

    pub static UNISWAP_V2_ROUTER: Lazy<Address> = Lazy::new(||
        "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap());

    pub static UNISWAP_V3_ROUTER: Lazy<Address> = Lazy::new(||
        "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap());

    pub static SUSHISWAP_ROUTER: Lazy<Address> = Lazy::new(||
        "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap());

    pub static WETH_ADDRESS: Lazy<Address> = Lazy::new(||
        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap());

    pub static USDC_ADDRESS: Lazy<Address> = Lazy::new(||
        "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".parse().unwrap());

    pub static USDT_ADDRESS: Lazy<Address> = Lazy::new(||
        "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap());

    pub static AAVE_V3_POOL: Lazy<Address> = Lazy::new(||
        "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".parse().unwrap());

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
        let to = Address::zero();
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

        let to = Address::zero();
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
