use std::collections::HashSet;
use ethers::types::{H160, Transaction as EthersTransaction};

/// DEX 라우터 주소들을 반환합니다
pub fn get_dex_routers() -> HashSet<H160> {
    let mut routers = HashSet::new();
    
    // Uniswap V2 Router
    routers.insert("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap());
    
    // Uniswap V3 Router
    routers.insert("0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap());
    
    // SushiSwap Router
    routers.insert("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap());
    
    // PancakeSwap Router
    routers.insert("0x10ED43C718714eb63d5aA57B78B54704E256024E".parse().unwrap());
    
    routers
}

/// 대출 풀 주소들을 반환합니다
pub fn get_lending_pools() -> HashSet<H160> {
    let mut pools = HashSet::new();
    
    // Aave Lending Pool
    pools.insert("0x7d2768dE32b0b80b7a3454c06BdAc94A69DDc7A9".parse().unwrap());
    
    // Compound Comptroller
    pools.insert("0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B".parse().unwrap());
    
    // MakerDAO Vault Manager
    pools.insert("0x5ef30b9986345249bc32d8928B7ee64DE9435E39".parse().unwrap());
    
    pools
}

/// 트랜잭션이 DEX 스왑인지 확인합니다
pub fn is_dex_swap(tx: &EthersTransaction) -> bool {
    let routers = get_dex_routers();
    
    // 라우터 주소로 전송되는지 확인
    if let Some(to) = tx.to {
        if routers.contains(&to) {
            // 스왑 함수 시그니처 확인 (swap, swapExactTokensForTokens 등)
            let data = &tx.input;
            if data.len() >= 4 {
                let function_selector = &data[0..4];
                // swap 함수 시그니처들
                let swap_selectors = [
                    [0x38, 0xed, 0x17, 0x39], // swap
                    [0x88, 0x03, 0xdb, 0xee], // swapExactTokensForTokens
                    [0x4a, 0x25, 0xd9, 0x4d], // swapExactETHForTokens
                    [0x18, 0xcb, 0x50, 0x15], // swapExactTokensForETH
                ];
                
                return swap_selectors.iter().any(|selector| function_selector == selector);
            }
        }
    }
    
    false
}

/// 트랜잭션이 청산 호출인지 확인합니다
pub fn is_liquidation_call(tx: &EthersTransaction) -> bool {
    let pools = get_lending_pools();
    
    // 대출 풀 주소로 전송되는지 확인
    if let Some(to) = tx.to {
        if pools.contains(&to) {
            // 청산 함수 시그니처 확인
            let data = &tx.input;
            if data.len() >= 4 {
                let function_selector = &data[0..4];
                // liquidationCall 함수 시그니처
                let liquidation_selector = [0x2f, 0x54, 0xbf, 0x6a]; // liquidationCall
                
                return function_selector == liquidation_selector;
            }
        }
    }
    
    false
}

/// 트랜잭션이 상당한 가치를 가지고 있는지 확인합니다
pub fn has_significant_value(tx: &EthersTransaction, min_eth: f64) -> bool {
    let min_value_wei = (min_eth * 1e18) as u128;
    tx.value.as_u128() >= min_value_wei
} 