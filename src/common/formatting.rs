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