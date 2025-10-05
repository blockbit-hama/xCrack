
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

/// Calculate Volume Weighted Average Price (VWAP)
pub fn calculate_vwap(prices: &[f64], volumes: &[f64]) -> f64 {
    if prices.is_empty() || volumes.is_empty() || prices.len() != volumes.len() {
        return 0.0;
    }
    
    let mut total_volume = 0.0;
    let mut total_price_volume = 0.0;
    
    for (price, volume) in prices.iter().zip(volumes.iter()) {
        total_price_volume += price * volume;
        total_volume += volume;
    }
    
    if total_volume == 0.0 {
        return 0.0;
    }
    
    total_price_volume / total_volume
}

/// Calculate Time Weighted Average Price (TWAP)
pub fn calculate_twap(prices: &[f64], time_weights: &[f64]) -> f64 {
    if prices.is_empty() || time_weights.is_empty() || prices.len() != time_weights.len() {
        return 0.0;
    }
    
    let mut total_weight = 0.0;
    let mut total_weighted_price = 0.0;
    
    for (price, weight) in prices.iter().zip(time_weights.iter()) {
        total_weighted_price += price * weight;
        total_weight += weight;
    }
    
    if total_weight == 0.0 {
        return 0.0;
    }
    
    total_weighted_price / total_weight
}

/// Calculate order slicing for iceberg orders
pub fn calculate_iceberg_slices(total_amount: u128, max_slice_size: u128) -> Vec<u128> {
    if total_amount == 0 || max_slice_size == 0 {
        return vec![];
    }
    
    let mut slices = vec![];
    let mut remaining = total_amount;
    
    while remaining > 0 {
        let slice_size = if remaining > max_slice_size {
            max_slice_size
        } else {
            remaining
        };
        
        slices.push(slice_size);
        remaining -= slice_size;
    }
    
    slices
} 