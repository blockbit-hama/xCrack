use alloy::primitives::{Address, U256};
use std::str::FromStr;

// Gas limits
pub const DEFAULT_GAS_LIMIT: u64 = 300_000;
pub const MAX_GAS_LIMIT: u64 = 30_000_000;

// Time constants (in seconds)
pub const BLOCK_TIME: u64 = 12;
pub const MAX_BUNDLE_LIFETIME: u64 = 300; // 5 minutes

// Profit thresholds
pub const MIN_PROFIT_WEI: u64 = 10_000_000_000_000_000; // 0.01 ETH
pub const MIN_PROFIT_RATIO: f64 = 0.01; // 1%

// Gas price limits (in gwei)
pub const MAX_GAS_PRICE_GWEI: u64 = 500;
pub const MAX_PRIORITY_FEE_GWEI: u64 = 50;

// Common token addresses (mainnet)
pub const WETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
pub const USDC: &str = "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46";
pub const USDT: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
pub const DAI: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
pub const WBTC: &str = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599";

// Helper function to get token addresses
pub fn get_token_address(symbol: &str) -> Option<Address> {
    match symbol.to_uppercase().as_str() {
        "WETH" => Some(Address::from_str(WETH).unwrap()),
        "USDC" => Some(Address::from_str(USDC).unwrap()),
        "USDT" => Some(Address::from_str(USDT).unwrap()),
        "DAI" => Some(Address::from_str(DAI).unwrap()),
        "WBTC" => Some(Address::from_str(WBTC).unwrap()),
        _ => None,
    }
}

// DEX Router addresses
pub const UNISWAP_V2_ROUTER: &str = "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D";
pub const UNISWAP_V3_ROUTER: &str = "0xE592427A0AEce92De3Edee1F18E0157C05861564";
pub const SUSHISWAP_ROUTER: &str = "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F";
pub const ONEINCH_ROUTER: &str = "0x1111111254EEB25477B68fb85Ed929f73A960582";

// Factory addresses
pub const UNISWAP_V2_FACTORY: &str = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f";
pub const UNISWAP_V3_FACTORY: &str = "0x1F98431c8aD98523631AE4a59f267346ea31F984";
pub const SUSHISWAP_FACTORY: &str = "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac";

// Protocol addresses
pub const AAVE_LENDING_POOL: &str = "0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9";
pub const AAVE_DATA_PROVIDER: &str = "0x057835Ad21a177dbdd3090bB1CAE03EaCF78Fc6d";
pub const COMPOUND_COMPTROLLER: &str = "0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B";
pub const MAKERDAO_DOG: &str = "0x135954d155898D42C90D2a57824C690e0c7BEf1B";

// Function selectors
pub const SWAP_EXACT_ETH_FOR_TOKENS: &str = "0x7ff36ab5";
pub const SWAP_EXACT_TOKENS_FOR_ETH: &str = "0x18cbafe5";
pub const SWAP_EXACT_TOKENS_FOR_TOKENS: &str = "0x38ed1739";
pub const SWAP_TOKENS_FOR_EXACT_TOKENS: &str = "0x8803dbee";

pub const LIQUIDATION_CALL_AAVE: &str = "0xe8eda9df";
pub const LIQUIDATE_BORROW_COMPOUND: &str = "0xf5e3c462";
pub const BITE_MAKERDAO: &str = "0x7c025200";

// Utility functions
pub fn is_known_dex_router(address: Address) -> bool {
    let known_routers = [
        UNISWAP_V2_ROUTER,
        UNISWAP_V3_ROUTER,
        SUSHISWAP_ROUTER,
        ONEINCH_ROUTER,
    ];
    
    known_routers.iter().any(|&router| {
        router.parse::<Address>().unwrap_or_default() == address
    })
}

pub fn is_swap_function(selector: &str) -> bool {
    let swap_selectors = [
        SWAP_EXACT_ETH_FOR_TOKENS,
        SWAP_EXACT_TOKENS_FOR_ETH,
        SWAP_EXACT_TOKENS_FOR_TOKENS,
        SWAP_TOKENS_FOR_EXACT_TOKENS,
    ];
    
    swap_selectors.contains(&selector)
}

pub fn is_liquidation_function(selector: &str) -> bool {
    let liquidation_selectors = [
        LIQUIDATION_CALL_AAVE,
        LIQUIDATE_BORROW_COMPOUND,
        BITE_MAKERDAO,
    ];
    
    liquidation_selectors.contains(&selector)
}

// Helper to format ETH amounts
pub fn format_eth_amount(wei: U256) -> String {
    let eth = wei.to::<u128>() as f64 / 1e18;
    format!("{:.6} ETH", eth)
}

// Helper to parse ETH amounts
pub fn parse_eth_amount(eth_str: &str) -> Result<U256, String> {
    let eth: f64 = eth_str.parse().map_err(|_| "Invalid ETH amount")?;
    let wei = (eth * 1e18) as u128;
    Ok(U256::from(wei))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_token_addresses() {
        assert!(get_token_address("WETH").is_some());
        assert!(get_token_address("USDC").is_some());
        assert!(get_token_address("USDT").is_some());
        assert!(get_token_address("DAI").is_some());
        assert!(get_token_address("WBTC").is_some());
        assert!(get_token_address("NONEXISTENT").is_none());
        
        // Test case insensitivity
        assert_eq!(get_token_address("weth"), get_token_address("WETH"));
        assert_eq!(get_token_address("usdc"), get_token_address("USDC"));
    }
    
    #[test]
    fn test_dex_router_detection() {
        let uniswap_v2 = Address::from_str(UNISWAP_V2_ROUTER).unwrap();
        let uniswap_v3 = Address::from_str(UNISWAP_V3_ROUTER).unwrap();
        let sushiswap = Address::from_str(SUSHISWAP_ROUTER).unwrap();
        let unknown = Address::from_str("0x1234567890123456789012345678901234567890").unwrap();
        
        assert!(is_known_dex_router(uniswap_v2));
        assert!(is_known_dex_router(uniswap_v3));
        assert!(is_known_dex_router(sushiswap));
        assert!(!is_known_dex_router(unknown));
    }
    
    #[test]
    fn test_function_selectors() {
        assert!(is_swap_function(SWAP_EXACT_ETH_FOR_TOKENS));
        assert!(is_swap_function(SWAP_EXACT_TOKENS_FOR_ETH));
        assert!(is_swap_function(SWAP_EXACT_TOKENS_FOR_TOKENS));
        assert!(!is_swap_function("0x12345678"));
        
        assert!(is_liquidation_function(LIQUIDATION_CALL_AAVE));
        assert!(is_liquidation_function(LIQUIDATE_BORROW_COMPOUND));
        assert!(is_liquidation_function(BITE_MAKERDAO));
        assert!(!is_liquidation_function("0x12345678"));
    }
    
    #[test]
    fn test_eth_formatting() {
        let one_eth = U256::from(1000000000000000000u64);
        let formatted = format_eth_amount(one_eth);
        assert!(formatted.contains("1.000000"));
        
        let half_eth = U256::from(500000000000000000u64);
        let formatted = format_eth_amount(half_eth);
        assert!(formatted.contains("0.500000"));
    }
    
    #[test]
    fn test_eth_parsing() {
        let parsed = parse_eth_amount("1.0").unwrap();
        assert_eq!(parsed, U256::from(1000000000000000000u64));
        
        let parsed = parse_eth_amount("0.5").unwrap();
        assert_eq!(parsed, U256::from(500000000000000000u64));
        
        let invalid = parse_eth_amount("invalid");
        assert!(invalid.is_err());
    }
}
