use anyhow::Result;
use chrono::Utc;

// Constants are now in crate::constants - removed to avoid duplication

/// Utility functions for mathematical operations
pub mod math {
    use super::*;

    /// Calculate optimal trade size for arbitrage
    pub fn calculate_optimal_trade_size(
        _reserve_a1: String, // Reserve of token A on DEX 1
        _reserve_b1: String, // Reserve of token B on DEX 1
        _reserve_a2: String, // Reserve of token A on DEX 2
        _reserve_b2: String, // Reserve of token B on DEX 2
        _fee1: u32,        // Fee on DEX 1 (in basis points)
        _fee2: u32,        // Fee on DEX 2 (in basis points)
    ) -> String {
        // Simplified calculation - in practice you'd solve the optimization problem
        let optimal_size = 1000000000000000000u128; // 1 ETH in wei
        optimal_size.to_string()
    }

    /// Calculate AMM output amount using x*y=k formula
    pub fn calculate_amm_output(
        amount_in: String,
        reserve_in: String,
        reserve_out: String,
        fee_rate: u32, // in basis points (300 = 0.3%)
    ) -> String {
        let amount_in_u128 = amount_in.parse::<u128>().unwrap_or(0);
        let reserve_in_u128 = reserve_in.parse::<u128>().unwrap_or(0);
        let reserve_out_u128 = reserve_out.parse::<u128>().unwrap_or(0);
        
        if amount_in_u128 == 0 || reserve_in_u128 == 0 || reserve_out_u128 == 0 {
            return "0".to_string();
        }
        
        let fee_denominator = 10000u128;
        let fee_numerator = fee_denominator - fee_rate as u128;
        
        let amount_in_with_fee = amount_in_u128 * fee_numerator;
        let numerator = amount_in_with_fee * reserve_out_u128;
        let denominator = reserve_in_u128 * fee_denominator + amount_in_with_fee;
        
        (numerator / denominator).to_string()
    }

    /// Calculate price impact for a trade
    pub fn calculate_price_impact(
        amount_in: String,
        reserve_in: String,
        reserve_out: String,
    ) -> f64 {
        let amount_in_u128 = amount_in.parse::<u128>().unwrap_or(0);
        let reserve_in_u128 = reserve_in.parse::<u128>().unwrap_or(0);
        let reserve_out_u128 = reserve_out.parse::<u128>().unwrap_or(0);
        
        if reserve_in_u128 == 0 || reserve_out_u128 == 0 {
            return 0.0;
        }
        
        let original_price = reserve_out_u128 as f64 / reserve_in_u128 as f64;
        let new_reserve_in = reserve_in_u128 + amount_in_u128;
        let amount_out = calculate_amm_output(amount_in, reserve_in, reserve_out, 300);
        let amount_out_u128 = amount_out.parse::<u128>().unwrap_or(0);
        let new_reserve_out = reserve_out_u128 - amount_out_u128;
        
        if new_reserve_in == 0 {
            return 1.0; // 100% impact
        }
        
        let new_price = new_reserve_out as f64 / new_reserve_in as f64;
        
        ((original_price - new_price) / original_price).abs()
    }
}

/// Utility functions for formatting
pub mod formatting {
    use super::*;

    /// Format wei amount to ETH with specified decimals
    pub fn format_eth(wei: String, decimals: usize) -> String {
        let wei_u128 = wei.parse::<u128>().unwrap_or(0);
        let eth = wei_u128 as f64 / 10_f64.powi(18);
        format!("{:.1$}", eth, decimals)
    }

    /// Format gas price from wei to gwei
    pub fn format_gas_price_gwei(gas_price_wei: String) -> String {
        let gas_price_u128 = gas_price_wei.parse::<u128>().unwrap_or(0);
        let gwei = gas_price_u128 as f64 / 1_000_000_000.0;
        format!("{:.2} gwei", gwei)
    }

    /// Format percentage
    pub fn format_percentage(value: f64) -> String {
        format!("{:.2}%", value * 100.0)
    }
}

/// Utility functions for validation
pub mod validation {
    use super::*;

    /// Validate Ethereum address format
    pub fn is_valid_address(address: &str) -> bool {
        if !address.starts_with("0x") {
            return false;
        }
        
        if address.len() != 42 {
            return false;
        }
        
        // Check that the remaining characters are valid hex
        let hex_part = &address[2..];
        hex_part.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Validate transaction hash format
    pub fn is_valid_tx_hash(hash: &str) -> bool {
        hash.len() == 66 && hash.starts_with("0x")
    }

    /// Check if amount is reasonable (not too large)
    pub fn is_reasonable_amount(amount: String) -> bool {
        let amount_u128 = amount.parse::<u128>().unwrap_or(0);
        let max_amount = 1000u128 * 10u128.pow(18); // 1000 ETH
        amount_u128 <= max_amount
    }

    /// Check if gas price is reasonable
    pub fn is_reasonable_gas_price(gas_price: String) -> bool {
        let gas_price_u128 = gas_price.parse::<u128>().unwrap_or(0);
        let min_gas_price = 1_000_000_000u128; // 1 gwei
        let max_gas_price = 1_000_000_000_000u128; // 1000 gwei
        gas_price_u128 >= min_gas_price && gas_price_u128 <= max_gas_price
    }
}

/// Utility functions for cryptographic operations
pub mod crypto {
    use super::*;
    use sha3::{Digest, Keccak256};

    /// Calculate Keccak256 hash
    pub fn keccak256(data: &[u8]) -> String {
        let mut hasher = Keccak256::new();
        hasher.update(data);
        let result = hasher.finalize();
        format!("0x{}", hex::encode(result))
    }

    /// Sign message with private key (placeholder)
    pub fn sign_message(_message: &[u8], _private_key: &str) -> Result<String> {
        // This would implement actual signing logic
        Ok("0x".to_string())
    }
}

/// Utility functions for time operations
pub mod time {
    use super::*;

    /// Get current timestamp
    pub fn current_timestamp() -> u64 {
        Utc::now().timestamp() as u64
    }

    /// Calculate time difference in seconds
    pub fn time_diff(timestamp1: u64, timestamp2: u64) -> u64 {
        if timestamp1 > timestamp2 {
            timestamp1 - timestamp2
        } else {
            timestamp2 - timestamp1
        }
    }
}

/// Utility functions for network operations
pub mod network {
    use super::*;

    /// Check if we're on mainnet (placeholder)
    pub async fn is_mainnet(_chain_id: u64) -> Result<bool> {
        // This would check the actual chain ID
        Ok(true)
    }

    /// Get gas price in gwei (placeholder)
    pub async fn get_gas_price_gwei(_rpc_url: &str) -> Result<u64> {
        // This would make an RPC call to get gas price
        Ok(20) // Default 20 gwei
    }

    /// Estimate time to next block (placeholder)
    pub async fn estimate_time_to_next_block(_block_time: u64) -> Result<u64> {
        // This would calculate based on current block and block time
        let current_time = Utc::now().timestamp() as u64;
        let block_time = 12; // Ethereum block time
        Ok(block_time - (current_time % block_time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_eth() {
        let wei = "1000000000000000000"; // 1 ETH
        assert_eq!(formatting::format_eth(wei.to_string(), 2), "1.00");
        
        let wei = "500000000000000000"; // 0.5 ETH
        assert_eq!(formatting::format_eth(wei.to_string(), 3), "0.500");
    }

    #[test]
    fn test_address_validation() {
        assert!(validation::is_valid_address("0x742d35Cc65700000000000000000000000000004"));
        assert!(!validation::is_valid_address("invalid_address"));
    }

    #[test]
    fn test_gas_price_validation() {
        let reasonable_gas = "20000000000"; // 20 gwei
        assert!(validation::is_reasonable_gas_price(reasonable_gas.to_string()));
        
        let too_high_gas = "2000000000000"; // 2000 gwei
        assert!(!validation::is_reasonable_gas_price(too_high_gas.to_string()));
    }
}