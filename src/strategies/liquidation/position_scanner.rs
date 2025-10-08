/// 포지션 스캔 모듈
///
/// 역할: 대출 프로토콜에서 청산 가능한 포지션을 스캔하고 발견
/// - Aave, Compound, MakerDAO 프로토콜별 스캔
/// - 고위험 사용자 탐지
/// - 청산 기회 수집 및 정렬

use std::sync::Arc;
use anyhow::Result;
use tokio::sync::Mutex;
use tracing::{info, debug, warn};
use ethers::types::Address;
use std::collections::HashMap;

use crate::strategies::liquidation::types::{LendingProtocolInfo, OnChainLiquidationOpportunity, UserPosition};
use crate::strategies::liquidation::stats::OnChainLiquidationStats;
use crate::blockchain::{BlockchainClient, ContractFactory, LendingPoolContract};
use crate::storage::Storage;
use crate::protocols::TheGraphClient;

pub struct PositionScanner {
    blockchain_client: Arc<BlockchainClient>,
    contract_factory: Arc<ContractFactory>,
    lending_protocols: HashMap<Address, LendingProtocolInfo>,
    stats: Arc<Mutex<OnChainLiquidationStats>>,
    storage: Arc<Storage>,
    thegraph_client: Arc<TheGraphClient>,
}

impl PositionScanner {
    pub fn new(
        blockchain_client: Arc<BlockchainClient>,
        contract_factory: Arc<ContractFactory>,
        lending_protocols: HashMap<Address, LendingProtocolInfo>,
        stats: Arc<Mutex<OnChainLiquidationStats>>,
        storage: Arc<Storage>,
    ) -> Self {
        Self {
            blockchain_client,
            contract_factory,
            lending_protocols,
            stats,
            storage,
            thegraph_client: Arc::new(TheGraphClient::new()),
        }
    }

    /// 모든 프로토콜에서 청산 가능한 포지션 스캔
    pub async fn scan_liquidatable_positions(&self) -> Result<Vec<OnChainLiquidationOpportunity>> {
        debug!("🔍 청산 가능 포지션 스캔 시작");

        let mut opportunities = Vec::new();

        // 각 프로토콜에서 청산 가능한 포지션 탐색
        for protocol in self.lending_protocols.values() {
            match self.scan_protocol_positions(protocol).await {
                Ok(mut protocol_opportunities) => {
                    opportunities.append(&mut protocol_opportunities);
                }
                Err(e) => {
                    warn!("프로토콜 {} 스캔 실패: {}", protocol.name, e);
                }
            }
        }

        // 수익성 순으로 정렬
        opportunities.sort_by(|a, b| b.net_profit.cmp(&a.net_profit));

        // 상위 10개만 반환
        opportunities.truncate(10);

        info!("🎯 청산 기회 발견: {} 개", opportunities.len());

        Ok(opportunities)
    }

    /// 특정 프로토콜의 포지션 스캔
    async fn scan_protocol_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        let mut opportunities = Vec::new();

        match protocol.protocol_type {
            crate::strategies::liquidation::types::ProtocolType::Aave => {
                opportunities.extend(self.scan_aave_positions(protocol).await?);
            }
            crate::strategies::liquidation::types::ProtocolType::Compound => {
                opportunities.extend(self.scan_compound_positions(protocol).await?);
            }
            crate::strategies::liquidation::types::ProtocolType::MakerDAO => {
                opportunities.extend(self.scan_maker_positions(protocol).await?);
            }
        }

        Ok(opportunities)
    }

    /// Aave 포지션 스캔
    async fn scan_aave_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        let mut opportunities = Vec::new();

        // 알려진 고위험 사용자들 조회
        let high_risk_users = self.get_high_risk_users(protocol).await?;

        for user in high_risk_users {
            // 개별 포지션 분석은 position_analyzer에서 처리
            debug!("사용자 {} Aave 포지션 분석", user);
        }

        Ok(opportunities)
    }

    /// Compound 포지션 스캔
    async fn scan_compound_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        let mut opportunities = Vec::new();
        let users = self.get_high_risk_users(protocol).await?;

        for user in users {
            debug!("사용자 {} Compound 포지션 분석", user);
        }

        Ok(opportunities)
    }

    /// MakerDAO 포지션 스캔
    async fn scan_maker_positions(&self, protocol: &LendingProtocolInfo) -> Result<Vec<OnChainLiquidationOpportunity>> {
        let mut opportunities = Vec::new();
        let users = self.get_high_risk_users(protocol).await?;

        for user in users {
            debug!("사용자 {} MakerDAO 포지션 분석", user);
        }

        Ok(opportunities)
    }

    /// 고위험 사용자 목록 조회
    async fn get_high_risk_users(&self, protocol: &LendingProtocolInfo) -> Result<Vec<Address>> {
        info!("🔍 Fetching high-risk users for protocol: {}", protocol.name);

        // The Graph API로 청산 가능한 사용자 조회
        match protocol.protocol_type {
            crate::strategies::liquidation::types::ProtocolType::Aave => {
                match self.thegraph_client.get_aave_liquidatable_users(100).await {
                    Ok(users) => {
                        let addresses: Vec<Address> = users.iter()
                            .map(|u| u.address)
                            .collect();

                        info!("✅ The Graph: Found {} Aave users", addresses.len());
                        Ok(addresses)
                    }
                    Err(e) => {
                        warn!("⚠️ The Graph API failed: {}, using fallback", e);

                        // Fallback: 하드코딩된 테스트 주소
                        Ok(vec![
                            "0x742d35Cc6570000000000000000000000000001".parse()?,
                            "0x742d35Cc6570000000000000000000000000002".parse()?,
                            "0x742d35Cc6570000000000000000000000000003".parse()?,
                        ])
                    }
                }
            }
            _ => {
                // 다른 프로토콜은 아직 The Graph 미지원
                warn!("⚠️ The Graph not implemented for {:?}, using fallback", protocol.protocol_type);
                Ok(vec![
                    "0x742d35Cc6570000000000000000000000000001".parse()?,
                    "0x742d35Cc6570000000000000000000000000002".parse()?,
                ])
            }
        }
    }
}
