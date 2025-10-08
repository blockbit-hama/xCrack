use std::collections::HashMap;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use ethers::types::{Address, U256, H256};

use crate::types::Transaction;

/// Flashbots 번들 - 여러 트랜잭션을 하나의 블록에 순차적으로 포함
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashbotsBundle {
    /// 번들 고유 ID
    pub id: String,
    
    /// 번들에 포함된 트랜잭션들 (실행 순서대로)
    pub transactions: Vec<BundleTransaction>,
    
    /// 타겟 블록 번호
    pub target_block: u64,
    
    /// 번들 전체 예상 수익
    pub expected_profit: U256,
    
    /// 번들 전체 가스 사용량 추정
    pub total_gas_estimate: u64,
    
    /// 번들 우선순위 팁 (MEV 수익의 일부를 miner에게 지불)
    pub priority_tip: U256,
    
    /// 번들 유형 (Sandwich, Liquidation, etc.)
    pub bundle_type: BundleType,
    
    /// 번들 메타데이터
    pub metadata: BundleMetadata,
}

/// 번들 내 개별 트랜잭션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleTransaction {
    /// 트랜잭션 해시 (서명 후)
    pub hash: H256,
    
    /// 발신자 주소
    pub from: Address,
    
    /// 수신자 주소
    pub to: Option<Address>,
    
    /// 전송할 ETH 양
    pub value: U256,
    
    /// 가스 가격
    pub gas_price: U256,
    
    /// 가스 한도
    pub gas_limit: u64,
    
    /// 트랜잭션 데이터 (스마트 컨트랙트 호출)
    pub data: Vec<u8>,
    
    /// 논스
    pub nonce: u64,
    
    /// 트랜잭션 유형
    pub tx_type: TransactionType,
    
    /// 트랜잭션 역할 (Front-run, Victim, Back-run)
    pub role: TransactionRole,
}

/// 번들 유형
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BundleType {
    /// 샌드위치 공격 (Front-run + Victim + Back-run)
    Sandwich,
    /// 청산 프론트런
    Liquidation,
    /// 아비트러지
    Arbitrage,
    /// 일반 MEV
    GeneralMev,
}

/// 트랜잭션 유형
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    /// DEX 스왑
    DexSwap,
    /// 토큰 전송
    TokenTransfer,
    /// 청산 호출
    Liquidation,
    /// 아비트러지
    Arbitrage,
    /// 기타
    Other,
}

/// 번들 내 트랜잭션 역할
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionRole {
    /// 프론트런 트랜잭션 (피해자 트랜잭션 앞에 실행)
    FrontRun,
    /// 피해자 트랜잭션 (우리가 감시하는 대상)
    Victim,
    /// 백런 트랜잭션 (피해자 트랜잭션 뒤에 실행)
    BackRun,
    /// 단독 트랜잭션 (청산 등)
    Standalone,
}

/// 번들 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleMetadata {
    /// 생성 시간
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// 번들 생성 전략
    pub strategy: String,
    
    /// 예상 성공 확률
    pub success_probability: f64,
    
    /// 경쟁 번들 수 (동일한 기회를 노리는 다른 봇들)
    pub competition_level: u32,
    
    /// 추가 메타데이터
    pub extra: HashMap<String, String>,
}

/// 번들 시뮬레이션 결과
#[derive(Debug, Clone)]
pub struct BundleSimulation {
    /// 시뮬레이션 성공 여부
    pub success: bool,
    
    /// 실제 수익
    pub actual_profit: U256,
    
    /// 실제 가스 사용량
    pub actual_gas_used: u64,
    
    /// 가스 비용
    pub gas_cost: U256,
    
    /// 순 수익
    pub net_profit: U256,
    
    /// 각 트랜잭션별 실행 결과
    pub transaction_results: Vec<TransactionResult>,
    
    /// 시뮬레이션 오류 (있는 경우)
    pub error: Option<String>,
}

/// 개별 트랜잭션 실행 결과
#[derive(Debug, Clone)]
pub struct TransactionResult {
    /// 트랜잭션 해시
    pub hash: H256,
    
    /// 실행 성공 여부
    pub success: bool,
    
    /// 가스 사용량
    pub gas_used: u64,
    
    /// 실행 오류 (있는 경우)
    pub error: Option<String>,
    
    /// 트랜잭션 로그
    pub logs: Vec<String>,
}

impl FlashbotsBundle {
    /// 새로운 번들 생성
    pub fn new(
        bundle_type: BundleType,
        target_block: u64,
        strategy: String,
    ) -> Self {
        let id = format!("{}_{}", 
            chrono::Utc::now().timestamp_millis(), 
            rand::random::<u32>()
        );
        
        Self {
            id,
            transactions: Vec::new(),
            target_block,
            expected_profit: U256::zero(),
            total_gas_estimate: 0,
            priority_tip: U256::zero(),
            bundle_type,
            metadata: BundleMetadata {
                created_at: chrono::Utc::now(),
                strategy,
                success_probability: 0.0,
                competition_level: 0,
                extra: HashMap::new(),
            },
        }
    }
    
    /// 샌드위치 번들 생성 (프론트런 + 피해자 + 백런)
    pub fn create_sandwich_bundle(
        front_run_tx: Transaction,
        victim_tx: Transaction,
        back_run_tx: Transaction,
        target_block: u64,
        expected_profit: U256,
    ) -> Self {
        let mut bundle = Self::new(BundleType::Sandwich, target_block, "sandwich".to_string());
        
        // 프론트런 트랜잭션 추가
        bundle.add_transaction(BundleTransaction::from_transaction(
            front_run_tx,
            TransactionRole::FrontRun,
        ));
        
        // 피해자 트랜잭션 추가
        bundle.add_transaction(BundleTransaction::from_transaction(
            victim_tx,
            TransactionRole::Victim,
        ));
        
        // 백런 트랜잭션 추가
        bundle.add_transaction(BundleTransaction::from_transaction(
            back_run_tx,
            TransactionRole::BackRun,
        ));
        
        bundle.expected_profit = expected_profit;
        bundle.calculate_priority_tip();
        bundle.estimate_total_gas();
        
        bundle
    }
    
    /// 청산 번들 생성
    pub fn create_liquidation_bundle(
        liquidation_tx: Transaction,
        target_block: u64,
        expected_profit: U256,
    ) -> Self {
        let mut bundle = Self::new(BundleType::Liquidation, target_block, "liquidation".to_string());
        
        bundle.add_transaction(BundleTransaction::from_transaction(
            liquidation_tx,
            TransactionRole::Standalone,
        ));
        
        bundle.expected_profit = expected_profit;
        bundle.calculate_priority_tip();
        bundle.estimate_total_gas();
        
        bundle
    }
    
    /// 번들에 트랜잭션 추가
    pub fn add_transaction(&mut self, tx: BundleTransaction) {
        self.transactions.push(tx);
    }
    
    /// 우선순위 팁 계산 (수익의 10%를 miner에게)
    pub fn calculate_priority_tip(&mut self) {
        self.priority_tip = self.expected_profit / U256::from(10); // 10%
    }
    
    /// 전체 가스 사용량 추정
    pub fn estimate_total_gas(&mut self) {
        self.total_gas_estimate = self.transactions
            .iter()
            .map(|tx| tx.gas_limit)
            .sum();
    }
    
    /// 번들 검증
    pub fn validate(&self) -> Result<()> {
        if self.transactions.is_empty() {
            return Err(anyhow!("번들이 비어있습니다"));
        }
        
        if self.target_block == 0 {
            return Err(anyhow!("유효하지 않은 타겟 블록"));
        }
        
        // 샌드위치 번들의 경우 특별 검증
        if self.bundle_type == BundleType::Sandwich {
            self.validate_sandwich_bundle()?;
        }
        
        // 가스 한도 검증 (블록 가스 한도의 50% 미만)
        let block_gas_limit = 30_000_000u64; // 이더리움 블록 가스 한도
        if self.total_gas_estimate > block_gas_limit / 2 {
            return Err(anyhow!("번들 가스 사용량이 너무 큽니다: {}", self.total_gas_estimate));
        }
        
        Ok(())
    }
    
    /// 샌드위치 번들 특별 검증
    fn validate_sandwich_bundle(&self) -> Result<()> {
        let front_runs = self.transactions.iter()
            .filter(|tx| tx.role == TransactionRole::FrontRun)
            .count();
        let victims = self.transactions.iter()
            .filter(|tx| tx.role == TransactionRole::Victim)
            .count();
        let back_runs = self.transactions.iter()
            .filter(|tx| tx.role == TransactionRole::BackRun)
            .count();
        
        if front_runs == 0 {
            return Err(anyhow!("샌드위치 번들에 프론트런 트랜잭션이 없습니다"));
        }
        
        if victims == 0 {
            return Err(anyhow!("샌드위치 번들에 피해자 트랜잭션이 없습니다"));
        }
        
        if back_runs == 0 {
            return Err(anyhow!("샌드위치 번들에 백런 트랜잭션이 없습니다"));
        }
        
        // 순서 검증: 프론트런 -> 피해자 -> 백런 순으로 배치되어야 함
        let mut expecting_role = TransactionRole::FrontRun;
        let mut victim_found = false;
        
        for tx in &self.transactions {
            match (&expecting_role, &tx.role) {
                (TransactionRole::FrontRun, TransactionRole::FrontRun) => {
                    // 프론트런이 여러 개일 수 있음
                }
                (TransactionRole::FrontRun, TransactionRole::Victim) => {
                    expecting_role = TransactionRole::Victim;
                    victim_found = true;
                }
                (TransactionRole::Victim, TransactionRole::Victim) => {
                    // 피해자가 여러 개일 수 있음
                }
                (TransactionRole::Victim, TransactionRole::BackRun) => {
                    if victim_found {
                        expecting_role = TransactionRole::BackRun;
                    } else {
                        return Err(anyhow!("피해자 트랜잭션 없이 백런이 나타났습니다"));
                    }
                }
                (TransactionRole::BackRun, TransactionRole::BackRun) => {
                    // 백런이 여러 개일 수 있음
                }
                _ => {
                    return Err(anyhow!("샌드위치 번들의 트랜잭션 순서가 잘못되었습니다"));
                }
            }
        }
        
        Ok(())
    }
    
    /// 번들을 Flashbots 형식으로 직렬화
    pub fn to_flashbots_format(&self) -> Result<FlashbotsBundleRequest> {
        let transactions = self.transactions
            .iter()
            .map(|tx| tx.to_flashbots_transaction())
            .collect::<Result<Vec<_>>>()?;
        
        Ok(FlashbotsBundleRequest {
            txs: transactions,
            block_number: format!("0x{:x}", self.target_block),
            min_timestamp: None,
            max_timestamp: None,
        })
    }
    
    /// 번들 통계 정보
    pub fn get_stats(&self) -> BundleStats {
        let front_runs = self.transactions.iter()
            .filter(|tx| tx.role == TransactionRole::FrontRun)
            .count();
        let victims = self.transactions.iter()
            .filter(|tx| tx.role == TransactionRole::Victim)
            .count();
        let back_runs = self.transactions.iter()
            .filter(|tx| tx.role == TransactionRole::BackRun)
            .count();
        
        BundleStats {
            transaction_count: self.transactions.len(),
            front_run_count: front_runs,
            victim_count: victims,
            back_run_count: back_runs,
            estimated_gas: self.total_gas_estimate,
            expected_profit_eth: format!("{:.6}", self.expected_profit.as_u128() as f64 / 1e18),
            priority_tip_eth: format!("{:.6}", self.priority_tip.as_u128() as f64 / 1e18),
        }
    }
}

impl BundleTransaction {
    /// Transaction에서 BundleTransaction으로 변환
    pub fn from_transaction(tx: Transaction, role: TransactionRole) -> Self {
        let tx_type = match tx.data.get(0..4) {
            Some([0x38, 0xed, 0x17, 0x39]) => TransactionType::DexSwap, // swapExactTokensForTokens
            Some([0xa9, 0x05, 0x9c, 0xbb]) => TransactionType::TokenTransfer, // transfer
            Some([0x00, 0xa7, 0x18, 0xa9]) => TransactionType::Liquidation, // liquidationCall
            _ => TransactionType::Other,
        };
        
        Self {
            hash: tx.hash,
            from: tx.from,
            to: tx.to,
            value: tx.value,
            gas_price: tx.gas_price,
            gas_limit: tx.gas_limit.try_into().unwrap_or(300_000),
            data: tx.data,
            nonce: tx.nonce,
            tx_type,
            role,
        }
    }
    
    /// Flashbots 트랜잭션 형식으로 변환
    pub fn to_flashbots_transaction(&self) -> Result<FlashbotsTransaction> {
        Ok(FlashbotsTransaction {
            to: self.to.map(|addr| format!("0x{:x}", addr)),
            value: format!("0x{:x}", self.value),
            gas: format!("0x{:x}", self.gas_limit),
            gas_price: format!("0x{:x}", self.gas_price),
            data: format!("0x{}", hex::encode(&self.data)),
        })
    }
}

/// Flashbots API 요청 형식
#[derive(Debug, Serialize)]
pub struct FlashbotsBundleRequest {
    pub txs: Vec<FlashbotsTransaction>,
    #[serde(rename = "blockNumber")]
    pub block_number: String,
    #[serde(rename = "minTimestamp", skip_serializing_if = "Option::is_none")]
    pub min_timestamp: Option<u64>,
    #[serde(rename = "maxTimestamp", skip_serializing_if = "Option::is_none")]
    pub max_timestamp: Option<u64>,
}

/// Flashbots 트랜잭션 형식
#[derive(Debug, Serialize)]
pub struct FlashbotsTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub value: String,
    pub gas: String,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    pub data: String,
}

/// 번들 통계 정보
#[derive(Debug)]
pub struct BundleStats {
    pub transaction_count: usize,
    pub front_run_count: usize,
    pub victim_count: usize,
    pub back_run_count: usize,
    pub estimated_gas: u64,
    pub expected_profit_eth: String,
    pub priority_tip_eth: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::Address;
    
    #[test]
    fn test_sandwich_bundle_creation() {
        let front_run = Transaction {
            hash: H256::zero(),
            from: Address::zero(),
            to: Some(Address::zero()),
            value: U256::from_str_radix("1000000000000000000", 10).unwrap(), // 1 ETH
            gas_price: U256::from(20_000_000_000u64), // 20 gwei
            gas_limit: U256::from(300_000u64),
            data: vec![0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            nonce: 0,
            timestamp: chrono::Utc::now(),
            block_number: Some(1000),
        };
        
        let victim = front_run.clone();
        let back_run = front_run.clone();
        
        let bundle = FlashbotsBundle::create_sandwich_bundle(
            front_run,
            victim,
            back_run,
            1001,
            U256::from_str_radix("100000000000000000", 10).unwrap(), // 0.1 ETH profit
        );
        
        assert_eq!(bundle.bundle_type, BundleType::Sandwich);
        assert_eq!(bundle.transactions.len(), 3);
        assert!(bundle.validate().is_ok());
        
        let stats = bundle.get_stats();
        assert_eq!(stats.front_run_count, 1);
        assert_eq!(stats.victim_count, 1);
        assert_eq!(stats.back_run_count, 1);
    }
    
    #[test]
    fn test_bundle_validation() {
        let mut bundle = FlashbotsBundle::new(BundleType::Sandwich, 1000, "test".to_string());
        
        // 빈 번들은 유효하지 않음
        assert!(bundle.validate().is_err());
        
        // 트랜잭션 추가
        bundle.add_transaction(BundleTransaction {
            hash: H256::zero(),
            from: Address::zero(),
            to: Some(Address::zero()),
            value: U256::from(1000000000000000000u64),
            gas_price: U256::from(20_000_000_000u64),
            gas_limit: 300_000,
            data: vec![],
            nonce: 0,
            tx_type: TransactionType::Other,
            role: TransactionRole::FrontRun,
        });
        
        // 샌드위치 번들이지만 불완전함 (프론트런만 있음)
        assert!(bundle.validate().is_err());
    }
    
    #[test]
    fn test_flashbots_format_conversion() {
        let bundle = FlashbotsBundle::new(BundleType::GeneralMev, 1000, "test".to_string());
        let flashbots_format = bundle.to_flashbots_format().unwrap();
        
        assert_eq!(flashbots_format.block_number, "0x3e8"); // 1000 in hex
        assert_eq!(flashbots_format.txs.len(), 0);
    }
}