pub mod math;
pub mod formatting;
pub mod validation;
pub mod crypto;
pub mod time;
pub mod network;
pub mod abi;
pub mod profitability;

// Re-exports
// pub use math::*;
// pub use formatting::*;
// pub use validation::*;
// pub use crypto::*;
// pub use time::*;
// pub use network::*;

#[cfg(test)]
mod tests {
    // use super::*;
    use crate::utils::formatting::format_eth;
    use crate::utils::validation::{is_valid_address, is_reasonable_gas_price};

    #[test]
    fn test_format_eth() {
        let wei = "1000000000000000000".to_string(); // 1 ETH
        let formatted = format_eth(wei, 4);
        assert_eq!(formatted, "1.0000");
    }

    #[test]
    fn test_address_validation() {
        assert!(is_valid_address("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6"));
        assert!(!is_valid_address("invalid"));
    }

    #[test]
    fn test_gas_price_validation() {
        assert!(is_reasonable_gas_price("20000000000".to_string())); // 20 gwei
        assert!(!is_reasonable_gas_price("1000000000000000000".to_string())); // Too high
    }
}