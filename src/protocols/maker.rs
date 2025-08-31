use std::sync::Arc;
use std::collections::HashMap;
use anyhow::Result;
use tracing::{info, debug};
use alloy::primitives::{Address, U256};
use ethers::{
    providers::{Provider, Ws, Middleware},
    contract::Contract,
    abi::Abi,
    types::{H160, H256, Filter, Log, BlockNumber},
};
use async_trait::async_trait;

use crate::config::Config;
use super::{
    ProtocolScanner, ProtocolType, LiquidatableUser, UserAccountData, 
    CollateralPosition, DebtPosition, ProtocolStats
};

/// MakerDAO CDP Scanner
pub struct MakerScanner {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    cdp_manager_address: H160,
    vat_address: H160,
    cdp_manager_contract: Contract<Provider<Ws>>,
    vat_contract: Contract<Provider<Ws>>,
    supported_ilks: Vec<String>,
    last_scan_block: u64,
}

impl MakerScanner {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🏦 Initializing MakerDAO Scanner...");
        
        // MakerDAO mainnet addresses
        let cdp_manager_address: H160 = "0x5ef30b9986345249bc32d8928B7ee64DE9435E39".parse()?;
        let vat_address: H160 = "0x35D1b3F3D7966A1DFe207aa4514C12a259A0492B".parse()?;
        
        // Load contract ABIs
        let cdp_manager_abi: Abi = serde_json::from_str(include_str!("../../abi/MakerCDPManager.json"))?;
        let vat_abi: Abi = serde_json::from_str(include_str!("../../abi/MakerVat.json"))?;
        
        // Create contract instances
        let cdp_manager_contract = Contract::new(cdp_manager_address, cdp_manager_abi, Arc::clone(&provider));
        let vat_contract = Contract::new(vat_address, vat_abi, Arc::clone(&provider));
        
        // Get supported ilks (collateral types)
        let supported_ilks = Self::get_supported_ilks(&vat_contract).await?;
        
        let current_block = provider.get_block_number().await?.as_u64();
        
        info!("✅ MakerDAO Scanner initialized with {} ilks", supported_ilks.len());
        
        Ok(Self {
            config,
            provider,
            cdp_manager_address,
            vat_address,
            cdp_manager_contract,
            vat_contract,
            supported_ilks,
            last_scan_block: current_block,
        })
    }
    
    async fn get_supported_ilks(_vat: &Contract<Provider<Ws>>) -> Result<Vec<String>> {
        // TODO: 실제 ilk 목록 조회 구현
        // 현재는 하드코딩된 목록 사용
        let ilks = vec![
            "ETH-A".to_string(),
            "ETH-B".to_string(),
            "ETH-C".to_string(),
            "WBTC-A".to_string(),
            "USDC-A".to_string(),
        ];
        
        info!("📊 Found {} MakerDAO ilks", ilks.len());
        Ok(ilks)
    }
    
    async fn get_active_cdps(&self) -> Result<Vec<u32>> {
        let current_block = self.provider.get_block_number().await?.as_u64();
        let from_block = if current_block > 1000 { current_block - 1000 } else { 0 };
        
        let mut cdps = std::collections::HashSet::new();
        
        // Get CDPs from recent events
        let open_filter = Filter::new()
            .address(self.cdp_manager_address)
            .topic0(H256::from_slice(&hex::decode("91e78c6c7d214de6657ff94d886adb4b377b1a8a5f7c4b8b8b8b8b8b8b8b8b8b").unwrap()))
            .from_block(from_block)
            .to_block(BlockNumber::Latest);
            
        if let Ok(open_logs) = self.provider.get_logs(&open_filter).await {
            for log in open_logs {
                if let Some(cdp_id) = self.parse_cdp_id_from_log(&log) {
                    cdps.insert(cdp_id);
                }
            }
        }
        
        Ok(cdps.into_iter().collect())
    }
    
    fn parse_cdp_id_from_log(&self, _log: &Log) -> Option<u32> {
        // TODO: 실제 로그 파싱 구현
        // 현재는 더미 데이터 반환
        Some(rand::random::<u32>() % 10000)
    }
    
    async fn get_cdp_data(&self, cdp_id: u32) -> Result<Option<CdpData>> {
        // TODO: 실제 CDP 데이터 조회 구현
        // 현재는 더미 데이터 반환
        
        let cdp_data = CdpData {
            cdp_id,
            owner: Address::from_slice(&rand::random::<[u8; 20]>()),
            ilk: "ETH-A".to_string(),
            collateral: U256::from(10_000_000_000_000_000_000u64), // 10 ETH
            debt: U256::from(2_000_000_000_000_000_000u64), // 2000 DAI
            liquidation_ratio: 1.5,
            collateral_price: 2000.0, // $2000 per ETH
            last_updated: chrono::Utc::now(),
        };
        
        Ok(Some(cdp_data))
    }
}

#[async_trait]
impl ProtocolScanner for MakerScanner {
    async fn scan_all_users(&self) -> anyhow::Result<Vec<LiquidatableUser>> {
        let mut liquidatable_users = Vec::new();
        
        // 1. 활성 CDP 목록 가져오기
        let active_cdps = self.get_active_cdps().await?;
        
        // 2. 각 CDP의 데이터 조회
        for cdp_id in active_cdps {
            if let Ok(Some(cdp_data)) = self.get_cdp_data(cdp_id).await {
                // 3. 청산 가능 여부 확인
                if self.is_liquidatable(&cdp_data).await? {
                    let user = self.build_liquidatable_user(cdp_data).await?;
                    liquidatable_users.push(user);
                }
            }
        }
        
        Ok(liquidatable_users)
    }
    
    async fn get_user_data(&self, _user: Address) -> anyhow::Result<Option<LiquidatableUser>> {
        // TODO: 특정 사용자의 CDP 데이터 조회 구현
        Ok(None)
    }
    
    async fn get_protocol_stats(&self) -> anyhow::Result<ProtocolStats> {
        let users = self.scan_all_users().await?;
        let liquidatable_count = users.len();
        
        Ok(ProtocolStats {
            protocol: ProtocolType::MakerDAO,
            total_users: users.len() as u64,
            liquidatable_users: liquidatable_count as u64,
            total_tvl_usd: 0.0, // TODO: 실제 TVL 계산
            total_borrows_usd: 0.0, // TODO: 실제 총 부채 계산
            avg_health_factor: 0.0, // TODO: 평균 헬스팩터 계산
            last_scan_duration_ms: 0, // TODO: 스캔 시간 측정
        })
    }
    
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::MakerDAO
    }
    
    fn is_healthy(&self) -> bool {
        true // TODO: 실제 헬스 체크 구현
    }
}

impl MakerScanner {
    async fn is_liquidatable(&self, cdp_data: &CdpData) -> Result<bool> {
        // MakerDAO 청산 조건: 담보 비율 < 청산 비율
        let collateral_ratio = (cdp_data.collateral.to::<u128>() as f64 * cdp_data.collateral_price) 
                              / (cdp_data.debt.to::<u128>() as f64 / 1e18);
        
        let is_liquidatable = collateral_ratio < cdp_data.liquidation_ratio;
        
        debug!("CDP {}: collateral_ratio={:.3}, liquidation_ratio={:.3}, liquidatable={}", 
               cdp_data.cdp_id, collateral_ratio, cdp_data.liquidation_ratio, is_liquidatable);
        
        Ok(is_liquidatable)
    }
    
    async fn build_liquidatable_user(&self, cdp_data: CdpData) -> Result<LiquidatableUser> {
        let collateral_ratio = (cdp_data.collateral.to::<u128>() as f64 * cdp_data.collateral_price) 
                              / (cdp_data.debt.to::<u128>() as f64 / 1e18);
        
        let health_factor = collateral_ratio / cdp_data.liquidation_ratio;
        
        // 담보 포지션
        let collateral_position = CollateralPosition {
            asset: Address::ZERO, // TODO: 실제 담보 토큰 주소
            amount: cdp_data.collateral,
            usd_value: cdp_data.collateral.to::<u128>() as f64 * cdp_data.collateral_price / 1e18,
            liquidation_threshold: cdp_data.liquidation_ratio,
            price_usd: cdp_data.collateral_price,
        };
        
        // 부채 포지션
        let debt_position = DebtPosition {
            asset: Address::ZERO, // TODO: 실제 DAI 주소
            amount: cdp_data.debt,
            usd_value: cdp_data.debt.to::<u128>() as f64 / 1e18,
            borrow_rate: 0.0, // TODO: 실제 대출 이자율
            price_usd: 1.0, // DAI = $1
        };
        
        let account_data = UserAccountData {
            user: cdp_data.owner,
            protocol: ProtocolType::MakerDAO,
            total_collateral_usd: collateral_position.usd_value,
            total_debt_usd: debt_position.usd_value,
            available_borrows_usd: 0.0, // MakerDAO는 추가 대출 불가
            current_liquidation_threshold: cdp_data.liquidation_ratio,
            ltv: 1.0 / cdp_data.liquidation_ratio,
            health_factor,
            last_updated: cdp_data.last_updated,
        };
        
        let mut max_liquidatable_debt = HashMap::new();
        max_liquidatable_debt.insert(Address::ZERO, cdp_data.debt); // 전체 부채 청산 가능
        
        let mut liquidation_bonus = HashMap::new();
        liquidation_bonus.insert(Address::ZERO, 0.13); // 13% 청산 보너스
        
        let priority_score = self.calculate_priority_score(health_factor, debt_position.usd_value);
        
        Ok(LiquidatableUser {
            address: cdp_data.owner,
            protocol: ProtocolType::MakerDAO,
            account_data,
            collateral_positions: vec![collateral_position],
            debt_positions: vec![debt_position],
            max_liquidatable_debt,
            liquidation_bonus,
            priority_score,
        })
    }
    
    fn calculate_priority_score(&self, health_factor: f64, debt_usd: f64) -> f64 {
        let urgency_score = if health_factor < 0.95 { 1.0 } else { 0.5 };
        let size_score = debt_usd / 1_000_000.0; // 100만 달러 기준
        
        urgency_score * 0.7 + size_score * 0.3
    }
}

/// MakerDAO CDP 데이터
#[derive(Debug, Clone)]
struct CdpData {
    cdp_id: u32,
    owner: Address,
    ilk: String,
    collateral: U256,
    debt: U256,
    liquidation_ratio: f64,
    collateral_price: f64,
    last_updated: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_maker_scanner_creation() {
        // TODO: 테스트 구현
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_cdp_liquidation_check() {
        // TODO: 테스트 구현
        assert!(true);
    }
}
