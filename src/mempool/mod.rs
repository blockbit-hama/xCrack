pub mod monitor;
pub mod filters;

// Re-exports
pub use monitor::MempoolMonitor;

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::{U256, Transaction as EthersTransaction};

    #[test]
    fn test_dex_router_filtering() {
        let routers = filters::get_dex_routers();
        assert!(routers.contains(&"0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()));
        assert!(routers.contains(&"0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap()));
    }

    #[test]
    fn test_dex_swap_detection() {
        // Mock swap transaction
        let mut tx = EthersTransaction::default();
        tx.to = Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap());
        tx.input = vec![0x38, 0xed, 0x17, 0x39, 0x00, 0x00, 0x00, 0x00].into(); // swap function
        
        assert!(filters::is_dex_swap(&tx));
        
        // Non-swap transaction
        tx.to = Some("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6".parse().unwrap());
        assert!(!filters::is_dex_swap(&tx));
    }

    #[test]
    fn test_liquidation_call_detection() {
        // Mock liquidation transaction
        let mut tx = EthersTransaction::default();
        tx.to = Some("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap());
        tx.input = vec![0x2f, 0x54, 0xbf, 0x6a, 0x00, 0x00, 0x00, 0x00].into(); // liquidationCall function
        
        assert!(filters::is_liquidation_call(&tx));
        
        // Non-liquidation transaction
        tx.to = Some("0x742d35Cc6634C0532925a3b8D4C9db96C4b4d8b6".parse().unwrap());
        assert!(!filters::is_liquidation_call(&tx));
    }

    #[test]
    fn test_significant_value_filtering() {
        let mut tx = EthersTransaction::default();
        tx.value = U256::from(1000000000000000000u128); // 1 ETH
        
        assert!(filters::has_significant_value(&tx, 0.5)); // 0.5 ETH minimum
        assert!(!filters::has_significant_value(&tx, 2.0)); // 2 ETH minimum
    }
}
