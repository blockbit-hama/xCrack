// 실시간 멤풀 모니터링 시스템 - 대형 트랜잭션 및 MEV 기회 포착

use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::time::{Instant, Duration};
use anyhow::Result;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, debug, error, warn};
use ethers::providers::{Provider, Ws, Middleware, StreamExt};
use ethers::types::{Transaction as EthersTransaction, BlockNumber, H256, Address as EthersAddress};
use serde::Serialize;

use crate::config::Config;
use crate::types::Transaction;
use crate::utils::abi::ABICodec;

/// 멤풀 트랜잭션 분류
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionCategory {
    /// DEX 스왑 트랜잭션
    DexSwap {
        dex_name: String,
        token_in: String,
        token_out: String,
        amount_in: String,
    },
    /// 대형 토큰 전송
    LargeTransfer {
        token: String,
        amount: String,
    },
    /// 청산 대상 트랜잭션
    LiquidationCandidate {
        protocol: String,
        user: String,
        health_factor: f64,
    },
    /// NFT 거래
    NftTrade {
        collection: String,
        token_id: String,
        price: String,
    },
    /// 기타
    Other,
}

/// 트랜잭션 메트릭
#[derive(Debug, Clone)]
pub struct TransactionMetrics {
    /// 발견 시간
    pub discovered_at: Instant,
    /// 가스 가격 (gwei)
    pub gas_price_gwei: f64,
    /// 트랜잭션 가치 (ETH)
    pub value_eth: f64,
    /// 예상 MEV 수익 (ETH)
    pub estimated_mev_profit: f64,
    /// 경쟁자 수 (동일한 기회를 노리는 봇들)
    pub competitors: u32,
}

/// 고급 멤풀 필터
#[derive(Debug, Clone)]
struct AdvancedFilter {
    /// 최소 가스 가격 (gwei)
    min_gas_price: u64,
    /// 최대 가스 가격 (gwei) - 너무 높으면 경쟁이 치열
    max_gas_price: u64,
    /// 최소 트랜잭션 가치 (ETH)
    min_value_eth: f64,
    /// 감시할 DEX 컨트랙트 주소들
    target_dex_addresses: Vec<EthersAddress>,
    /// 감시할 토큰 주소들
    target_tokens: Vec<EthersAddress>,
    /// 함수 셀렉터 필터
    target_function_selectors: Vec<[u8; 4]>,
}

/// 실시간 멤풀 모니터링 시스템
#[derive(Clone)]
pub struct MempoolMonitor {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    is_running: Arc<RwLock<bool>>,
    
    /// ABI 코덱 (트랜잭션 파싱용)
    abi_codec: Arc<ABICodec>,
    
    /// 고급 필터
    filter: AdvancedFilter,
    
    /// 최근 트랜잭션 캐시 (해시 -> 메트릭)
    transaction_cache: Arc<RwLock<HashMap<H256, TransactionMetrics>>>,
    
    /// 트랜잭션 발견 통계
    stats: Arc<RwLock<MempoolStats>>,
    
    /// 실시간 가스 가격 추적
    gas_price_tracker: Arc<RwLock<GasPriceTracker>>,
}

/// 멤풀 모니터링 통계
#[derive(Debug)]
struct MempoolStats {
    /// 총 스캔된 트랜잭션 수
    total_scanned: u64,
    /// 필터 통과한 트랜잭션 수
    filtered_transactions: u64,
    /// MEV 기회 발견 수
    mev_opportunities_found: u64,
    /// 평균 처리 시간 (ms)
    avg_processing_time_ms: f64,
    /// 마지막 업데이트 시간
    last_update: Instant,
}

impl Default for MempoolStats {
    fn default() -> Self {
        Self {
            total_scanned: 0,
            filtered_transactions: 0,
            mev_opportunities_found: 0,
            avg_processing_time_ms: 0.0,
            last_update: Instant::now(),
        }
    }
}

/// 가스 가격 추적기
#[derive(Debug)]
struct GasPriceTracker {
    /// 최근 가스 가격들 (gwei)
    recent_prices: VecDeque<f64>,
    /// 현재 평균 가스 가격
    current_average: f64,
    /// 최고가 (최근 1분)
    peak_price_1m: f64,
    /// 최저가 (최근 1분) 
    low_price_1m: f64,
    /// 마지막 업데이트
    last_update: Instant,
}

impl MempoolMonitor {
    /// 새로운 멤풀 모니터 생성
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🔍 고급 멤풀 모니터 초기화 중...");
        
        let abi_codec = Arc::new(ABICodec::new());
        
        // 고급 필터 설정
        let filter = AdvancedFilter {
            min_gas_price: config.performance.mempool_filter_min_gas_price.parse().unwrap_or(5), // 5 gwei
            max_gas_price: config.performance.mempool_filter_max_gas_price.parse().unwrap_or(500), // 500 gwei
            min_value_eth: config.performance.mempool_filter_min_value.parse().unwrap_or(0.1), // 0.1 ETH
            target_dex_addresses: vec![
                "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap(), // Uniswap V2
                "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap(), // Uniswap V3
                "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap(), // SushiSwap
                "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506".parse().unwrap(), // SushiSwap V2
            ],
            target_tokens: vec![
                "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".parse().unwrap(), // USDC
                "0xdAC17F958D2ee523a2206206994597C13D831ec7".parse().unwrap(), // USDT
                "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse().unwrap(), // WETH
            ],
            target_function_selectors: vec![
                [0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
                [0x7f, 0xf3, 0x6a, 0xb5], // swapExactETHForTokens
                [0x18, 0xcb, 0xa5, 0xe5], // swapExactTokensForETH
                [0x00, 0xa7, 0x18, 0xa9], // liquidationCall (Aave)
            ],
        };
        
        info!("✅ 멤풀 모니터 초기화 완료");
        info!("  📊 감시 DEX: {}개", filter.target_dex_addresses.len());
        info!("  🪙 감시 토큰: {}개", filter.target_tokens.len());
        info!("  ⛽ 가스 필터: {}-{} gwei", filter.min_gas_price, filter.max_gas_price);
        info!("  💰 최소 가치: {} ETH", filter.min_value_eth);
        
        Ok(Self {
            config,
            provider,
            is_running: Arc::new(RwLock::new(false)),
            abi_codec,
            filter,
            transaction_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(MempoolStats::default())),
            gas_price_tracker: Arc::new(RwLock::new(GasPriceTracker {
                recent_prices: VecDeque::with_capacity(100),
                current_average: 20.0, // 기본값 20 gwei
                peak_price_1m: 20.0,
                low_price_1m: 20.0,
                last_update: Instant::now(),
            })),
        })
    }
    
    /// 실시간 멤풀 모니터링 시작
    pub async fn start_advanced_monitoring(&self, tx_sender: mpsc::UnboundedSender<(Transaction, TransactionCategory, TransactionMetrics)>) -> Result<()> {
        info!("🚀 고급 멤풀 모니터링 시작...");
        *self.is_running.write().await = true;
        
        // WebSocket 구독을 통한 실시간 트랜잭션 모니터링
        let provider = Arc::clone(&self.provider);
        let filter = self.filter.clone();
        let abi_codec = Arc::clone(&self.abi_codec);
        let transaction_cache = Arc::clone(&self.transaction_cache);
        let stats = Arc::clone(&self.stats);
        let gas_tracker = Arc::clone(&self.gas_price_tracker);
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            // 실시간 pending 트랜잭션 스트림 구독
            let provider_for_fallback = Arc::clone(&provider);
            match provider.subscribe_pending_txs().await {
                Ok(mut stream) => {
                    info!("✅ 실시간 멤풀 스트림 연결 성공");
                    
                    while *is_running.read().await {
                        if let Some(tx_hash) = stream.next().await {
                            let start_time = Instant::now();
                            
                            // 트랜잭션 세부사항 가져오기
                            if let Ok(Some(tx)) = provider.get_transaction(tx_hash).await {
                                // 고급 필터링 적용
                                if Self::passes_advanced_filter(&tx, &filter) {
                                    // 트랜잭션 분류
                                    let category = Self::classify_transaction(&tx, &abi_codec).await;
                                    
                                    // 메트릭 계산
                                    let metrics = Self::calculate_metrics(&tx, &category).await;
                                    
                                    // 캐시에 저장
                                    transaction_cache.write().await.insert(tx_hash, metrics.clone());
                                    
                                    // 가스 가격 추적 업데이트
                                    Self::update_gas_tracker(&gas_tracker, &tx).await;
                                    
                                    // 내부 Transaction 형식으로 변환
                                    if let Ok(converted_tx) = Self::convert_ethers_transaction_advanced(tx).await {
                                        // MEV 기회가 있는 트랜잭션만 전송
                                        if metrics.estimated_mev_profit > 0.001 { // 0.001 ETH 이상
                                            if let Err(e) = tx_sender.send((converted_tx, category, metrics)) {
                                                error!("❌ MEV 기회 트랜잭션 전송 실패: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                    
                                    // 통계 업데이트
                                    let processing_time = start_time.elapsed().as_millis() as f64;
                                    Self::update_stats(&stats, processing_time, true).await;
                                } else {
                                    // 통계 업데이트 (필터링됨)
                                    let processing_time = start_time.elapsed().as_millis() as f64;
                                    Self::update_stats(&stats, processing_time, false).await;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("❌ 멤풀 스트림 연결 실패: {}", e);
                    
                    // 폴백: 폴링 방식으로 전환
                    Self::fallback_polling_monitor(provider_for_fallback, filter, abi_codec, transaction_cache, stats, gas_tracker, is_running, tx_sender).await;
                }
            }
        });
        
        // 정기적인 통계 리포트
        self.start_stats_reporter().await;
        
        // 캐시 정리
        self.start_cache_cleaner().await;
        
        Ok(())
    }
    
    /// 고급 필터를 통과하는지 확인
    fn passes_advanced_filter(tx: &EthersTransaction, filter: &AdvancedFilter) -> bool {
        // 가스 가격 필터
        let gas_price_gwei = tx.gas_price.unwrap_or_default().as_u64() / 1_000_000_000;
        if gas_price_gwei < filter.min_gas_price || gas_price_gwei > filter.max_gas_price {
            return false;
        }
        
        // 트랜잭션 가치 필터
        let value_eth = tx.value.as_u128() as f64 / 1e18;
        if value_eth < filter.min_value_eth {
            // 가치가 낮아도 DEX 호출이면 통과
            if let Some(to) = tx.to {
                if !filter.target_dex_addresses.contains(&to) {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        // 함수 셀렉터 필터
        if tx.input.len() >= 4 {
            let function_selector = &tx.input[0..4];
            let is_target_function = filter.target_function_selectors.iter()
                .any(|selector| selector == function_selector);
            
            if !is_target_function {
                return false;
            }
        } else {
            // 데이터가 없는 단순 전송은 대형 전송만 관심
            if value_eth < filter.min_value_eth * 10.0 {
                return false;
            }
        }
        
        true
    }
    
    /// 트랜잭션 분류
    async fn classify_transaction(tx: &EthersTransaction, abi_codec: &ABICodec) -> TransactionCategory {
        if tx.input.len() < 4 {
            // 단순 ETH 전송
            let value_eth = tx.value.as_u128() as f64 / 1e18;
            if value_eth > 10.0 { // 10 ETH 이상 대형 전송
                return TransactionCategory::LargeTransfer {
                    token: "ETH".to_string(),
                    amount: format!("{:.6} ETH", value_eth),
                };
            }
            return TransactionCategory::Other;
        }
        
        let _function_selector = &tx.input[0..4];
        
        // Uniswap 스왑 감지
        if abi_codec.matches_function(&tx.input, "swapExactTokensForTokens") ||
           abi_codec.matches_function(&tx.input, "swapExactETHForTokens") {
            // 스왑 세부사항 파싱 (간단한 구현)
            let dex_name = match tx.to {
                Some(addr) if addr == "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap() => "Uniswap V2",
                Some(addr) if addr == "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap() => "SushiSwap",
                _ => "Unknown DEX",
            };
            
            return TransactionCategory::DexSwap {
                dex_name: dex_name.to_string(),
                token_in: "Unknown".to_string(), // 실제로는 ABI 디코딩 필요
                token_out: "Unknown".to_string(),
                amount_in: format!("{:.6} ETH", tx.value.as_u128() as f64 / 1e18),
            };
        }
        
        // Aave 청산 감지
        if abi_codec.matches_function(&tx.input, "liquidationCall") {
            return TransactionCategory::LiquidationCandidate {
                protocol: "Aave".to_string(),
                user: "0x...".to_string(), // 실제로는 ABI 디코딩 필요
                health_factor: 0.98, // 실제로는 계산 필요
            };
        }
        
        TransactionCategory::Other
    }
    
    /// 트랜잭션 메트릭 계산
    async fn calculate_metrics(tx: &EthersTransaction, category: &TransactionCategory) -> TransactionMetrics {
        let gas_price_gwei = tx.gas_price.unwrap_or_default().as_u64() as f64 / 1e9;
        let value_eth = tx.value.as_u128() as f64 / 1e18;
        
        // MEV 수익 추정 (매우 간단한 휴리스틱)
        let estimated_mev_profit = match category {
            TransactionCategory::DexSwap { amount_in, .. } => {
                // 스왑 크기에 따른 MEV 수익 추정
                let swap_value = amount_in.replace(" ETH", "").parse::<f64>().unwrap_or(0.0);
                if swap_value > 5.0 {
                    swap_value * 0.001 // 0.1% 수익률 가정
                } else {
                    0.0
                }
            },
            TransactionCategory::LargeTransfer { .. } => {
                // 대형 전송은 가스 조정을 통한 프론트런 가능
                if value_eth > 100.0 {
                    0.01 // 0.01 ETH 추정
                } else {
                    0.0
                }
            },
            TransactionCategory::LiquidationCandidate { .. } => {
                // 청산에서 5-10% 보너스 기대
                value_eth * 0.05
            },
            _ => 0.0,
        };
        
        // 경쟁자 수 추정 (가스 가격 기반)
        let competitors = if gas_price_gwei > 100.0 {
            10 // 높은 가스 = 많은 경쟁
        } else if gas_price_gwei > 50.0 {
            5
        } else {
            1
        };
        
        TransactionMetrics {
            discovered_at: Instant::now(),
            gas_price_gwei,
            value_eth,
            estimated_mev_profit,
            competitors,
        }
    }
    
    /// 가스 가격 추적기 업데이트
    async fn update_gas_tracker(gas_tracker: &Arc<RwLock<GasPriceTracker>>, tx: &EthersTransaction) {
        let gas_price_gwei = tx.gas_price.unwrap_or_default().as_u64() as f64 / 1e9;
        let mut tracker = gas_tracker.write().await;
        
        // 최근 가격들에 추가
        tracker.recent_prices.push_back(gas_price_gwei);
        
        // 최대 100개만 유지
        if tracker.recent_prices.len() > 100 {
            tracker.recent_prices.pop_front();
        }
        
        // 평균 계산
        tracker.current_average = tracker.recent_prices.iter().sum::<f64>() / tracker.recent_prices.len() as f64;
        
        // 1분 내 최고가/최저가 업데이트
        if tracker.last_update.elapsed() > Duration::from_secs(60) {
            tracker.peak_price_1m = tracker.recent_prices.iter().fold(0.0, |a, &b| a.max(b));
            tracker.low_price_1m = tracker.recent_prices.iter().fold(f64::MAX, |a, &b| a.min(b));
            tracker.last_update = Instant::now();
        }
    }
    
    /// 통계 업데이트
    async fn update_stats(stats: &Arc<RwLock<MempoolStats>>, processing_time: f64, passed_filter: bool) {
        let mut s = stats.write().await;
        s.total_scanned += 1;
        
        if passed_filter {
            s.filtered_transactions += 1;
        }
        
        // 이동평균으로 처리시간 업데이트
        s.avg_processing_time_ms = (s.avg_processing_time_ms * 0.9) + (processing_time * 0.1);
        s.last_update = Instant::now();
    }
    
    /// 폴백 폴링 모니터
    async fn fallback_polling_monitor(
        provider: Arc<Provider<Ws>>,
        filter: AdvancedFilter,
        abi_codec: Arc<ABICodec>,
        transaction_cache: Arc<RwLock<HashMap<H256, TransactionMetrics>>>,
        stats: Arc<RwLock<MempoolStats>>,
        gas_tracker: Arc<RwLock<GasPriceTracker>>,
        is_running: Arc<RwLock<bool>>,
        tx_sender: mpsc::UnboundedSender<(Transaction, TransactionCategory, TransactionMetrics)>,
    ) {
        warn!("🔄 실시간 스트림 실패, 폴링 모드로 전환");
        
        let mut interval = tokio::time::interval(Duration::from_millis(500)); // 500ms 간격
        
        while *is_running.read().await {
            interval.tick().await;
            
            // 최근 블록들에서 트랜잭션 가져오기
            if let Ok(current_block) = provider.get_block_number().await {
                for offset in 0..3 {
                    let block_num = current_block.as_u64().saturating_sub(offset);
                    
                    if let Ok(Some(block)) = provider.get_block_with_txs(BlockNumber::Number(block_num.into())).await {
                        for tx in block.transactions {
                            let start_time = Instant::now();
                            
                            if Self::passes_advanced_filter(&tx, &filter) {
                                let category = Self::classify_transaction(&tx, &abi_codec).await;
                                let metrics = Self::calculate_metrics(&tx, &category).await;
                                
                                transaction_cache.write().await.insert(tx.hash(), metrics.clone());
                                Self::update_gas_tracker(&gas_tracker, &tx).await;
                                
                                if let Ok(converted_tx) = Self::convert_ethers_transaction_advanced(tx).await {
                                    if metrics.estimated_mev_profit > 0.001 {
                                        if tx_sender.send((converted_tx, category, metrics)).is_err() {
                                            return;
                                        }
                                    }
                                }
                                
                                let processing_time = start_time.elapsed().as_millis() as f64;
                                Self::update_stats(&stats, processing_time, true).await;
                            }
                        }
                    }
                }
            }
        }
    }
    
    /// 고급 트랜잭션 변환
    async fn convert_ethers_transaction_advanced(tx: EthersTransaction) -> Result<Transaction> {
        let timestamp = chrono::Utc::now();
        
        Ok(Transaction {
            hash: alloy::primitives::B256::from_slice(&tx.hash.0),
            from: alloy::primitives::Address::from_slice(&tx.from.0),
            to: tx.to.map(|addr| alloy::primitives::Address::from_slice(&addr.0)),
            value: {
                let mut bytes = [0u8; 32];
                tx.value.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            gas_price: {
                let gas_price = tx.gas_price.unwrap_or_default();
                let mut bytes = [0u8; 32];
                gas_price.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            gas_limit: {
                let gas = tx.gas;
                let mut bytes = [0u8; 32];
                gas.to_big_endian(&mut bytes);
                alloy::primitives::U256::from_be_bytes(bytes)
            },
            data: tx.input.to_vec(),
            nonce: tx.nonce.as_u64(),
            timestamp,
            block_number: tx.block_number.map(|bn| bn.as_u64()),
        })
    }
    
    /// 통계 리포터 시작
    async fn start_stats_reporter(&self) {
        let stats = Arc::clone(&self.stats);
        let gas_tracker = Arc::clone(&self.gas_price_tracker);
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // 1분마다
            
            while *is_running.read().await {
                interval.tick().await;
                
                let s = stats.read().await;
                let gas = gas_tracker.read().await;
                
                let filter_rate = if s.total_scanned > 0 {
                    (s.filtered_transactions as f64 / s.total_scanned as f64) * 100.0
                } else {
                    0.0
                };
                
                info!("📊 멤풀 모니터 통계 (1분간)");
                info!("  🔍 스캔: {}개 트랜잭션", s.total_scanned);
                info!("  ✅ 필터 통과: {}개 ({:.1}%)", s.filtered_transactions, filter_rate);
                info!("  🎯 MEV 기회: {}개", s.mev_opportunities_found);
                info!("  ⚡ 평균 처리시간: {:.2}ms", s.avg_processing_time_ms);
                info!("  ⛽ 현재 가스: {:.1} gwei (범위: {:.1}-{:.1})", 
                      gas.current_average, gas.low_price_1m, gas.peak_price_1m);
            }
        });
    }
    
    /// 캐시 정리 시작
    async fn start_cache_cleaner(&self) {
        let cache = Arc::clone(&self.transaction_cache);
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5분마다
            
            while *is_running.read().await {
                interval.tick().await;
                
                let mut cache_guard = cache.write().await;
                let before_count = cache_guard.len();
                
                // 10분 이상 된 캐시 엔트리 제거
                cache_guard.retain(|_, metrics| {
                    metrics.discovered_at.elapsed() < Duration::from_secs(600)
                });
                
                let after_count = cache_guard.len();
                
                if before_count > after_count {
                    debug!("🧹 캐시 정리: {}개 -> {}개 엔트리", before_count, after_count);
                }
            }
        });
    }
    
    /// 레거시 모니터링 인터페이스 (하위 호환성)
    pub async fn start_monitoring(&self, tx_sender: mpsc::UnboundedSender<Transaction>) -> Result<()> {
        warn!("⚠️  레거시 모니터링 사용 중. 고급 모니터링으로 업그레이드 권장");
        
        // 고급 모니터링을 래핑
        let (advanced_sender, mut advanced_receiver) = mpsc::unbounded_channel();
        
        // 고급 모니터링 시작
        self.start_advanced_monitoring(advanced_sender).await?;
        
        // 고급 결과를 레거시 형식으로 변환
        tokio::spawn(async move {
            while let Some((transaction, _category, _metrics)) = advanced_receiver.recv().await {
                if let Err(e) = tx_sender.send(transaction) {
                    error!("❌ 레거시 트랜잭션 전송 실패: {}", e);
                    break;
                }
            }
        });
        
        Ok(())
    }
    
    /// 모니터링을 중지합니다
    pub async fn stop(&self) {
        *self.is_running.write().await = false;
        info!("⏹️ 멤풀 모니터링 중지됨");
    }

    /// 모니터링 상태를 확인합니다
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
    
    /// 현재 가스 가격 정보 조회
    pub async fn get_gas_price_info(&self) -> GasPriceInfo {
        let tracker = self.gas_price_tracker.read().await;
        
        GasPriceInfo {
            current_average: tracker.current_average,
            peak_1m: tracker.peak_price_1m,
            low_1m: tracker.low_price_1m,
            last_update: tracker.last_update.elapsed().as_secs(),
        }
    }
    
    /// 멤풀 통계 조회
    pub async fn get_stats(&self) -> MempoolMonitorStats {
        let stats = self.stats.read().await;
        let cache_size = self.transaction_cache.read().await.len();
        
        MempoolMonitorStats {
            total_scanned: stats.total_scanned,
            filtered_transactions: stats.filtered_transactions,
            mev_opportunities_found: stats.mev_opportunities_found,
            avg_processing_time_ms: stats.avg_processing_time_ms,
            cache_size,
            last_update: stats.last_update.elapsed().as_secs(),
        }
    }
    
    /// 특정 해시의 트랜잭션 메트릭 조회
    pub async fn get_transaction_metrics(&self, hash: &H256) -> Option<TransactionMetrics> {
        self.transaction_cache.read().await.get(hash).cloned()
    }
    
    /// 특정 트랜잭션 해시로 트랜잭션을 가져옵니다 (레거시)
    pub async fn get_transaction_by_hash(&self, hash: H256) -> Result<Option<Transaction>> {
        if let Ok(Some(tx)) = self.provider.get_transaction(hash).await {
            if Self::passes_advanced_filter(&tx, &self.filter) {
                return Ok(Some(Self::convert_ethers_transaction_advanced(tx).await?));
            }
        }
        Ok(None)
    }
}

/// 가스 가격 정보
#[derive(Debug, Clone, Serialize)]
pub struct GasPriceInfo {
    pub current_average: f64,
    pub peak_1m: f64,
    pub low_1m: f64,
    pub last_update: u64, // timestamp in seconds
}

/// 멤풀 모니터 통계
#[derive(Debug, Clone, Serialize)]
pub struct MempoolMonitorStats {
    pub total_scanned: u64,
    pub filtered_transactions: u64,
    pub mev_opportunities_found: u64,
    pub avg_processing_time_ms: f64,
    pub cache_size: usize,
    pub last_update: u64, // timestamp in seconds
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::{Address as EthersAddress, U256 as EthersU256};
    
    #[test]
    fn test_advanced_filter() {
        let filter = AdvancedFilter {
            min_gas_price: 20,
            max_gas_price: 500,
            min_value_eth: 0.1,
            target_dex_addresses: vec!["0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()],
            target_tokens: vec![],
            target_function_selectors: vec![[0x38, 0xed, 0x17, 0x39]],
        };
        
        // 높은 가스 가격 트랜잭션
        let tx = EthersTransaction {
            hash: H256::zero(),
            nonce: EthersU256::zero(),
            block_hash: None,
            block_number: None,
            transaction_index: None,
            from: EthersAddress::zero(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()),
            value: EthersU256::from_dec_str("100000000000000000").unwrap(), // 0.1 ETH
            gas_price: Some(EthersU256::from(50_000_000_000u64)), // 50 gwei
            gas: EthersU256::from(300_000u64),
            input: vec![0x38, 0xed, 0x17, 0x39, 0x00, 0x00, 0x00, 0x00].into(), // swapExactTokensForTokens
            v: None,
            r: None,
            s: None,
            transaction_type: None,
            access_list: None,
            max_priority_fee_per_gas: None,
            max_fee_per_gas: None,
        };
        
        assert!(MempoolMonitor::passes_advanced_filter(&tx, &filter));
    }
    
    #[tokio::test]
    async fn test_transaction_classification() {
        let abi_codec = ABICodec::new();
        
        // DEX 스왑 트랜잭션
        let swap_tx = EthersTransaction {
            hash: H256::zero(),
            nonce: EthersU256::zero(),
            block_hash: None,
            block_number: None,
            transaction_index: None,
            from: EthersAddress::zero(),
            to: Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap()),
            value: EthersU256::from_dec_str("1000000000000000000").unwrap(), // 1 ETH
            gas_price: Some(EthersU256::from(20_000_000_000u64)),
            gas: EthersU256::from(300_000u64),
            input: vec![0x38, 0xed, 0x17, 0x39, 0x00, 0x00, 0x00, 0x00].into(),
            v: None,
            r: None,
            s: None,
            transaction_type: None,
            access_list: None,
            max_priority_fee_per_gas: None,
            max_fee_per_gas: None,
        };
        
        let category = MempoolMonitor::classify_transaction(&swap_tx, &abi_codec).await;
        
        match category {
            TransactionCategory::DexSwap { dex_name, .. } => {
                assert_eq!(dex_name, "Uniswap V2");
            },
            _ => panic!("예상하지 못한 트랜잭션 카테고리"),
        }
    }
}