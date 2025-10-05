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