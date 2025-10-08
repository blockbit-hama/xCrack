use super::types::{DexType, DexRouterInfo};
use anyhow::{Result, anyhow};
use ethers::types::Address;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{info, debug};

/// DEX 라우터 관리자
pub struct DexRouterManager {
    routers: HashMap<Address, DexRouterInfo>,
    router_by_type: HashMap<DexType, Vec<Address>>,
}

impl DexRouterManager {
    pub fn new() -> Result<Self> {
        info!("🔧 DEX 라우터 관리자 초기화 중...");

        let mut routers = HashMap::new();
        let mut router_by_type = HashMap::new();

        // Uniswap V2
        let uniswap_v2_router = DexRouterInfo {
            dex_type: DexType::UniswapV2,
            router_address: Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D")?,
            factory_address: Address::from_str("0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f")?,
            swap_exact_tokens_selector: [0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            swap_tokens_for_exact_selector: [0x8803, 0xdb, 0xee], // swapTokensForExactTokens
            fee_bps: 30, // 0.3%
        };
        routers.insert(uniswap_v2_router.router_address, uniswap_v2_router.clone());
        router_by_type.entry(DexType::UniswapV2).or_insert_with(Vec::new).push(uniswap_v2_router.router_address);

        // SushiSwap
        let sushiswap_router = DexRouterInfo {
            dex_type: DexType::SushiSwap,
            router_address: Address::from_str("0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F")?,
            factory_address: Address::from_str("0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac")?,
            swap_exact_tokens_selector: [0x38, 0xed, 0x17, 0x39],
            swap_tokens_for_exact_selector: [0x88, 0x03, 0xdb, 0xee],
            fee_bps: 30,
        };
        routers.insert(sushiswap_router.router_address, sushiswap_router.clone());
        router_by_type.entry(DexType::SushiSwap).or_insert_with(Vec::new).push(sushiswap_router.router_address);

        // Uniswap V3
        let uniswap_v3_router = DexRouterInfo {
            dex_type: DexType::UniswapV3,
            router_address: Address::from_str("0xE592427A0AEce92De3Edee1F18E0157C05861564")?,
            factory_address: Address::from_str("0x1F98431c8aD98523631AE4a59f267346ea31F984")?,
            swap_exact_tokens_selector: [0xc0, 0x4b, 0x8d, 0x59], // exactInputSingle
            swap_tokens_for_exact_selector: [0xdb, 0x3e, 0x21, 0x98], // exactOutputSingle
            fee_bps: 30, // default 0.3%, can be 0.05%, 1%
        };
        routers.insert(uniswap_v3_router.router_address, uniswap_v3_router.clone());
        router_by_type.entry(DexType::UniswapV3).or_insert_with(Vec::new).push(uniswap_v3_router.router_address);

        info!("✅ {} DEX 라우터 로드 완료", routers.len());

        Ok(Self {
            routers,
            router_by_type,
        })
    }

    /// 라우터 주소로 DEX 정보 조회
    pub fn get_router_info(&self, router: &Address) -> Option<&DexRouterInfo> {
        self.routers.get(router)
    }

    /// DEX 타입으로 라우터 목록 조회
    pub fn get_routers_by_type(&self, dex_type: DexType) -> Vec<&DexRouterInfo> {
        self.router_by_type
            .get(&dex_type)
            .map(|addrs| {
                addrs.iter()
                    .filter_map(|addr| self.routers.get(addr))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 트랜잭션 데이터에서 DEX 스왑 감지
    pub fn detect_swap(&self, to: &Address, data: &[u8]) -> Option<SwapDetection> {
        if data.len() < 4 {
            return None;
        }

        let selector = &data[0..4];
        let router_info = self.routers.get(to)?;

        // swapExactTokensForTokens 또는 exactInputSingle
        if selector == router_info.swap_exact_tokens_selector {
            debug!("✅ {} 스왑 감지: swapExactTokens", router_info.dex_type.name());
            return Some(SwapDetection {
                dex_type: router_info.dex_type,
                router_address: *to,
                is_exact_input: true,
                function_selector: selector.try_into().unwrap(),
            });
        }

        // swapTokensForExactTokens 또는 exactOutputSingle
        if selector == router_info.swap_tokens_for_exact_selector {
            debug!("✅ {} 스왑 감지: swapForExactTokens", router_info.dex_type.name());
            return Some(SwapDetection {
                dex_type: router_info.dex_type,
                router_address: *to,
                is_exact_input: false,
                function_selector: selector.try_into().unwrap(),
            });
        }

        None
    }

    /// 지원하는 모든 라우터 주소 목록
    pub fn all_router_addresses(&self) -> Vec<Address> {
        self.routers.keys().copied().collect()
    }

    /// DEX 타입별 수수료 조회
    pub fn get_fee_bps(&self, dex_type: DexType) -> u32 {
        dex_type.default_fee_bps()
    }
}

impl Default for DexRouterManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize DexRouterManager")
    }
}

/// 스왑 감지 결과
#[derive(Debug, Clone)]
pub struct SwapDetection {
    pub dex_type: DexType,
    pub router_address: Address,
    pub is_exact_input: bool,
    pub function_selector: [u8; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dex_router_manager() {
        let manager = DexRouterManager::new().unwrap();

        let uniswap_addr = Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap();
        let router_info = manager.get_router_info(&uniswap_addr);
        assert!(router_info.is_some());
        assert_eq!(router_info.unwrap().dex_type, DexType::UniswapV2);

        let all_routers = manager.all_router_addresses();
        assert!(!all_routers.is_empty());
    }

    #[test]
    fn test_swap_detection() {
        let manager = DexRouterManager::new().unwrap();
        let uniswap_addr = Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap();

        // swapExactTokensForTokens selector
        let data = vec![0x38, 0xed, 0x17, 0x39, 0x00, 0x00];
        let detection = manager.detect_swap(&uniswap_addr, &data);

        assert!(detection.is_some());
        let det = detection.unwrap();
        assert_eq!(det.dex_type, DexType::UniswapV2);
        assert!(det.is_exact_input);
    }
}
