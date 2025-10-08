//! Utility functions and helpers

use anyhow::Result;
use ethers::types::{Address, U256, H256};
use std::collections::HashMap;

/// Convert U256 to big-endian bytes
pub fn u256_to_be_bytes(value: U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    value.to_big_endian(&mut bytes);
    bytes
}

/// Convert H256 to bytes
pub fn h256_to_bytes(hash: H256) -> [u8; 32] {
    hash.0
}

/// Convert bytes to H256
pub fn bytes_to_h256(bytes: [u8; 32]) -> H256 {
    H256(bytes)
}

/// Calculate gas price with priority fee
pub fn calculate_gas_price(base_fee: U256, priority_fee: U256) -> U256 {
    base_fee + priority_fee
}

/// Format address for display
pub fn format_address(addr: Address) -> String {
    format!("0x{:x}", addr)
}

/// Parse address from string
pub fn parse_address(s: &str) -> Result<Address> {
    s.parse()
        .map_err(|e| anyhow::anyhow!("Invalid address: {}", e))
}

/// Calculate percentage
pub fn calculate_percentage(part: U256, total: U256) -> f64 {
    if total.is_zero() {
        0.0
    } else {
        (part.as_u128() as f64 / total.as_u128() as f64) * 100.0
    }
}

/// Safe division with zero check
pub fn safe_divide(numerator: U256, denominator: U256) -> Result<U256> {
    if denominator.is_zero() {
        Err(anyhow::anyhow!("Division by zero"))
    } else {
        Ok(numerator / denominator)
    }
}

/// Convert wei to ether
pub fn wei_to_ether(wei: U256) -> f64 {
    wei.as_u128() as f64 / 1e18
}

/// Convert ether to wei
pub fn ether_to_wei(ether: f64) -> U256 {
    U256::from((ether * 1e18) as u128)
}

/// Generate random nonce
pub fn generate_nonce() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// Check if address is zero
pub fn is_zero_address(addr: Address) -> bool {
    addr == Address::zero()
}

/// Create a map from key-value pairs
pub fn create_map<K, V>(pairs: Vec<(K, V)>) -> HashMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    pairs.into_iter().collect()
}

/// Time utilities
pub mod time {
    use chrono::{DateTime, Utc};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Get current timestamp
    pub fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Get current datetime
    pub fn now_datetime() -> DateTime<Utc> {
        Utc::now()
    }

    /// Convert timestamp to datetime
    pub fn timestamp_to_datetime(timestamp: u64) -> DateTime<Utc> {
        DateTime::from_timestamp(timestamp as i64, 0).unwrap_or_else(Utc::now)
    }
}

/// Math utilities
pub mod math {
    use ethers::types::U256;

    /// Calculate square root (approximation)
    pub fn sqrt(value: U256) -> U256 {
        if value.is_zero() {
            return U256::zero();
        }
        
        let mut x = value;
        let mut y = (x + U256::one()) / U256::from(2);
        
        while y < x {
            x = y;
            y = (x + value / x) / U256::from(2);
        }
        
        x
    }

    /// Calculate power
    pub fn pow(base: U256, exponent: u32) -> U256 {
        base.pow(exponent)
    }

    /// Calculate percentage of value
    pub fn percentage_of(value: U256, percentage: u64) -> U256 {
        value * U256::from(percentage) / U256::from(100)
    }
}