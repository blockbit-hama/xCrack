# 📚 xCrack MEV Searcher API 참조

xCrack MEV Searcher의 모든 API, 구조체, 함수에 대한 상세 참조 문서입니다.

## 📋 목차

1. [Core API](#core-api)
2. [Strategy API](#strategy-api)
3. [Types & Structures](#types--structures)
4. [Configuration API](#configuration-api)
5. [Monitoring API](#monitoring-api)
6. [Utility Functions](#utility-functions)

---

## Core API

### SearcherCore

```rust
pub struct SearcherCore {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    is_running: Arc<AtomicBool>,
    strategy_manager: Arc<StrategyManager>,
    bundle_manager: Arc<BundleManager>,
    mempool_monitor: Arc<CoreMempoolMonitor>,
    performance_tracker: Arc<PerformanceTracker>,
}
```

#### 주요 메서드

##### `new(config: Config) -> Result<Self>`
새로운 SearcherCore 인스턴스를 생성합니다.

**매개변수:**
- `config: Config` - 시스템 구성 설정

**반환값:**
- `Result<SearcherCore>` - 성공시 SearcherCore 인스턴스, 실패시 에러

**예시:**
```rust
let config = Config::load("config/default.toml").await?;
let core = SearcherCore::new(config)?;
```

##### `start(&self) -> Result<()>`
시스템을 시작하고 모든 컴포넌트를 초기화합니다.

**반환값:**
- `Result<()>` - 성공시 (), 실패시 에러

**예시:**
```rust
core.start().await?;
```

##### `stop(&self) -> Result<()>`
시스템을 안전하게 중지합니다.

**반환값:**
- `Result<()>` - 성공시 (), 실패시 에러

**예시:**
```rust
core.stop().await?;
```

##### `get_performance_metrics(&self) -> PerformanceMetrics`
현재 성능 메트릭을 반환합니다.

**반환값:**
- `PerformanceMetrics` - 성능 통계 구조체

---

### BundleManager

```rust
pub struct BundleManager {
    flashbots_client: Option<Arc<FlashbotsClient>>,
    mock_flashbots_client: Option<Arc<MockFlashbotsClient>>,
    pending_bundles: Arc<Mutex<HashMap<String, Bundle>>>,
    submitted_bundles: Arc<Mutex<HashMap<String, Bundle>>>,
}
```

#### 주요 메서드

##### `create_bundle(&self, opportunities: Vec<Opportunity>) -> Result<Bundle>`
기회들을 기반으로 번들을 생성합니다.

**매개변수:**
- `opportunities: Vec<Opportunity>` - MEV 기회 목록

**반환값:**
- `Result<Bundle>` - 생성된 번들 또는 에러

##### `submit_bundle(&self, bundle: Bundle) -> Result<BundleResult>`
생성된 번들을 Flashbots에 제출합니다.

**매개변수:**
- `bundle: Bundle` - 제출할 번들

**반환값:**
- `Result<BundleResult>` - 제출 결과

---

### MicroArbitrageOrchestrator

```rust
pub struct MicroArbitrageOrchestrator {
    config: Arc<Config>,
    exchange_monitor: Arc<ExchangeMonitor>,
    price_feed_manager: Arc<PriceFeedManager>,
    strategy: Arc<MicroArbitrageStrategy>,
    order_executor: Arc<OrderExecutor>,
}
```

#### 주요 메서드

##### `start(&self) -> Result<()>`
마이크로 아비트래지 시스템을 시작합니다.

##### `scan_and_execute(&self) -> Result<Vec<MicroArbitrageStats>>`
아비트래지 기회를 스캔하고 실행합니다.

**반환값:**
- `Result<Vec<MicroArbitrageStats>>` - 실행 통계 목록

---

## Strategy API

### Strategy Trait

```rust
pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn strategy_type(&self) -> StrategyType;
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>>;
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    
    fn is_enabled(&self) -> bool;
    fn get_stats(&self) -> StrategyStats;
}
```

### SandwichStrategy

```rust
impl Strategy for SandwichStrategy {
    fn name(&self) -> &str { "Sandwich" }
    fn strategy_type(&self) -> StrategyType { StrategyType::Sandwich }
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        // 샌드위치 공격 기회 분석 로직
    }
}
```

### LiquidationStrategy

```rust
impl Strategy for LiquidationStrategy {
    fn name(&self) -> &str { "Liquidation" }
    fn strategy_type(&self) -> StrategyType { StrategyType::Liquidation }
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        // 청산 기회 분석 로직
    }
}
```

### MicroArbitrageStrategy

```rust
impl Strategy for MicroArbitrageStrategy {
    fn name(&self) -> &str { "MicroArbitrage" }
    fn strategy_type(&self) -> StrategyType { StrategyType::MicroArbitrage }
    
    async fn analyze(&self, transaction: &Transaction) -> Result<Vec<Opportunity>> {
        // 마이크로 아비트래지 기회 분석 로직
    }
}
```

---

## Types & Structures

### Core Types

#### Transaction
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub hash: B256,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub data: Vec<u8>,
    pub nonce: u64,
    pub timestamp: DateTime<Utc>,
    pub block_number: Option<u64>,
}
```

#### Opportunity
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Opportunity {
    pub id: String,
    pub opportunity_type: OpportunityType,
    pub strategy: StrategyType,
    pub expected_profit: U256,
    pub confidence: f64,
    pub gas_estimate: u64,
    pub priority: Priority,
    pub timestamp: DateTime<Utc>,
    pub expiry_block: u64,
    pub details: OpportunityDetails,
}
```

#### Bundle
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bundle {
    pub id: String,
    pub transactions: Vec<Transaction>,
    pub target_block: u64,
    pub expected_profit: U256,
    pub gas_estimate: u64,
    pub priority: Priority,
    pub strategy: StrategyType,
    pub timestamp: DateTime<Utc>,
    pub expiry_time: DateTime<Utc>,
}
```

### Micro-Arbitrage Types

#### MicroArbitrageDetails
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicroArbitrageDetails {
    pub token_symbol: String,
    pub buy_exchange: ExchangeInfo,
    pub sell_exchange: ExchangeInfo,
    pub amount: U256,
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub price_diff: Decimal,
    pub profit_percentage: f64,
    pub execution_time_ms: u64,
    pub order_books: Vec<OrderBookSnapshot>,
}
```

#### PriceData
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceData {
    pub symbol: String,
    pub exchange: String,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last_price: Decimal,
    pub volume_24h: U256,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}
```

#### OrderBookSnapshot
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderBookSnapshot {
    pub exchange: String,
    pub symbol: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}
```

#### OrderExecutionResult
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderExecutionResult {
    pub order_id: String,
    pub exchange: String,
    pub symbol: String,
    pub side: OrderSide,
    pub amount: U256,
    pub price: Decimal,
    pub filled_amount: U256,
    pub filled_price: Decimal,
    pub status: OrderStatus,
    pub execution_time: DateTime<Utc>,
    pub latency_ms: u64,
    pub fees: U256,
}
```

### Enums

#### StrategyType
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StrategyType {
    Sandwich,
    Liquidation,
    MicroArbitrage,
}
```

#### OpportunityType
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpportunityType {
    Sandwich,
    Liquidation,
    MicroArbitrage,
    MevBoost,
}
```

#### OrderSide
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}
```

#### OrderStatus
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}
```

---

## Configuration API

### Config Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub strategies: StrategyConfig,
    pub flashbots: FlashbotsConfig,
    pub safety: SafetyConfig,
    pub monitoring: MonitoringConfig,
    pub performance: PerformanceConfig,
    pub dexes: Vec<DexConfig>,
    pub tokens: HashMap<String, String>,
}
```

### Config Methods

#### `load(path: &str) -> Result<Config>`
TOML 파일에서 설정을 로드합니다.

**예시:**
```rust
let config = Config::load("config/default.toml").await?;
```

#### `validate(&self) -> Result<()>`
설정의 유효성을 검사합니다.

**예시:**
```rust
config.validate()?;
```

#### `save(&self, path: &str) -> Result<()>`
설정을 파일로 저장합니다.

**예시:**
```rust
config.save("config/backup.toml").await?;
```

---

## Monitoring API

### PerformanceMetrics
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceMetrics {
    pub transactions_processed: u64,
    pub opportunities_found: u64,
    pub bundles_submitted: u64,
    pub bundles_included: u64,
    pub total_profit: U256,
    pub total_gas_spent: U256,
    pub avg_analysis_time: f64,
    pub avg_submission_time: f64,
    pub success_rate: f64,
    pub uptime: u64,
}
```

### MicroArbitrageStats
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MicroArbitrageStats {
    pub total_opportunities: u64,
    pub executed_trades: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub total_volume: U256,
    pub total_profit: U256,
    pub total_fees: U256,
    pub avg_profit_per_trade: U256,
    pub avg_execution_time_ms: f64,
    pub success_rate: f64,
    pub profit_rate: f64,
    pub uptime_percentage: f64,
    pub exchanges_monitored: u32,
    pub pairs_monitored: u32,
}
```

### Alert System
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Alert {
    pub id: String,
    pub alert_type: AlertType,
    pub title: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertType {
    Profit,
    Error,
    Warning,
    Emergency,
}
```

---

## Utility Functions

### Priority Utils
```rust
impl Priority {
    pub fn to_u8(&self) -> u8 {
        match self {
            Priority::Low => 0,
            Priority::Medium => 1,
            Priority::High => 2,
            Priority::Urgent => 3,
        }
    }
    
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Priority::Low,
            1 => Priority::Medium,
            2 => Priority::High,
            3 => Priority::Urgent,
            _ => Priority::Medium,
        }
    }
}
```

### Opportunity Utils
```rust
impl Opportunity {
    pub fn new(
        opportunity_type: OpportunityType,
        strategy: StrategyType,
        expected_profit: U256,
        confidence: f64,
        gas_estimate: u64,
        expiry_block: u64,
        details: OpportunityDetails,
    ) -> Self {
        // 구현 로직
    }
    
    pub fn is_expired(&self, current_block: u64) -> bool {
        current_block >= self.expiry_block
    }
    
    pub fn profit_per_gas(&self) -> f64 {
        if self.gas_estimate == 0 {
            return 0.0;
        }
        self.expected_profit.to::<u128>() as f64 / self.gas_estimate as f64
    }
}
```

### Bundle Utils
```rust
impl Bundle {
    pub fn new(
        transactions: Vec<Transaction>,
        target_block: u64,
        expected_profit: U256,
        gas_estimate: u64,
        strategy: StrategyType,
    ) -> Self {
        // 구현 로직
    }
    
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expiry_time
    }
}
```

---

## Exchange Client API

### ExchangeClient Trait
```rust
pub trait ExchangeClient: Send + Sync {
    async fn place_order(&self, order: &Order) -> Result<OrderExecutionResult>;
    async fn cancel_order(&self, order_id: &str) -> Result<()>;
    async fn get_balance(&self, asset: &str) -> Result<U256>;
    async fn get_current_price(&self, symbol: &str) -> Result<PriceData>;
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus>;
    async fn get_order_fills(&self, order_id: &str) -> Result<Vec<OrderFill>>;
    fn get_exchange_name(&self) -> &str;
    fn get_average_latency(&self) -> Duration;
    fn is_connected(&self) -> bool;
}
```

### Mock Exchange Clients

#### MockDexClient
```rust
pub struct MockDexClient {
    name: String,
    config: ExchangeConfig,
    latency: Duration,
    connected: AtomicBool,
    balances: Arc<RwLock<HashMap<String, U256>>>,
    orders: Arc<RwLock<HashMap<String, Order>>>,
}

impl ExchangeClient for MockDexClient {
    // DEX 특성을 시뮬레이션하는 구현
    // - 높은 지연시간 (100-200ms)
    // - 가스비 포함
    // - 슬리피지 시뮬레이션
}
```

#### MockCexClient
```rust
pub struct MockCexClient {
    name: String,
    config: ExchangeConfig,
    latency: Duration,
    connected: AtomicBool,
    balances: Arc<RwLock<HashMap<String, U256>>>,
    orders: Arc<RwLock<HashMap<String, Order>>>,
}

impl ExchangeClient for MockCexClient {
    // CEX 특성을 시뮬레이션하는 구현
    // - 낮은 지연시간 (30-60ms)
    // - 고정 수수료
    // - 높은 유동성
}
```

---

## Error Types

### MevError
```rust
#[derive(thiserror::Error, Debug)]
pub enum MevError {
    #[error("Strategy error: {message}")]
    Strategy { message: String, strategy: StrategyType },

    #[error("Bundle error: {message}")]
    Bundle { message: String, bundle_id: String },

    #[error("Simulation error: {0}")]
    Simulation(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),
}

pub type MevResult<T> = Result<T, MevError>;
```

---

## Constants

### Network Constants
```rust
pub const MAINNET_CHAIN_ID: u64 = 1;
pub const BLOCK_TIME: u64 = 12; // seconds
pub const DEFAULT_GAS_LIMIT: u64 = 300_000;
pub const MAX_GAS_LIMIT: u64 = 30_000_000;
pub const MAX_BUNDLE_LIFETIME: u64 = 300; // seconds
```

### Token Addresses
```rust
pub const WETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
pub const USDC: &str = "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46";
pub const USDT: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
pub const DAI: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";
pub const WBTC: &str = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599";
```

### Profit Thresholds
```rust
pub const MIN_PROFIT_WEI: u128 = 10_000_000_000_000_000; // 0.01 ETH
pub const MIN_PROFIT_RATIO: f64 = 0.01; // 1%
pub const MAX_GAS_PRICE_GWEI: u64 = 500;
pub const MAX_PRIORITY_FEE_GWEI: u64 = 50;
```

---

## HTTP API Endpoints

### Metrics Endpoint
```
GET /metrics
Content-Type: text/plain

# HELP xcrack_transactions_total Total number of transactions processed
# TYPE xcrack_transactions_total counter
xcrack_transactions_total 1234

# HELP xcrack_opportunities_total Total number of opportunities found
# TYPE xcrack_opportunities_total counter
xcrack_opportunities_total 56

# HELP xcrack_bundles_submitted_total Total number of bundles submitted
# TYPE xcrack_bundles_submitted_total counter
xcrack_bundles_submitted_total 23
```

### Performance Endpoint
```
GET /performance
Content-Type: application/json

{
  "transactions_processed": 1234,
  "opportunities_found": 56,
  "bundles_submitted": 23,
  "bundles_included": 12,
  "total_profit": "1500000000000000000",
  "success_rate": 0.95,
  "avg_analysis_time": 45.6,
  "uptime": 86400
}
```

### Health Check
```
GET /health
Content-Type: application/json

{
  "status": "healthy",
  "timestamp": "2025-01-09T10:30:00Z",
  "version": "1.2.0",
  "uptime_seconds": 86400,
  "components": {
    "searcher_core": "healthy",
    "mempool_monitor": "healthy",
    "strategy_manager": "healthy",
    "bundle_manager": "healthy"
  }
}
```

---

## CLI Commands

### 기본 명령어
```bash
# 기본 실행
./xcrack

# 설정 파일 지정
./xcrack --config config/production.toml

# 특정 전략만 실행
./xcrack --strategies sandwich,micro_arbitrage

# 설정 검증
./xcrack --validate-config

# 버전 정보
./xcrack --version

# 도움말
./xcrack --help
```

### 환경 변수
```bash
export API_MODE=mock          # mock 또는 real
export RUST_LOG=info          # 로그 레벨
export ALCHEMY_API_KEY=...    # API 키
export FLASHBOTS_PRIVATE_KEY=... # 프라이빗 키
```

---

## 예제 코드

### 기본 사용법
```rust
use xcrack::{Config, SearcherCore, StrategyType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 설정 로드
    let config = Config::load("config/default.toml").await?;
    
    // SearcherCore 생성
    let mut core = SearcherCore::new(config)?;
    
    // 시스템 시작
    core.start().await?;
    
    // 성능 메트릭 확인
    let metrics = core.get_performance_metrics();
    println!("처리된 트랜잭션: {}", metrics.transactions_processed);
    
    // 시스템 중지
    core.stop().await?;
    
    Ok(())
}
```

### 커스텀 전략 구현
```rust
use xcrack::{Strategy, StrategyType, Transaction, Opportunity, MevResult};

pub struct CustomStrategy {
    name: String,
    enabled: AtomicBool,
}

impl Strategy for CustomStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn strategy_type(&self) -> StrategyType {
        StrategyType::MicroArbitrage
    }
    
    async fn analyze(&self, transaction: &Transaction) -> MevResult<Vec<Opportunity>> {
        // 커스텀 분석 로직
        let mut opportunities = Vec::new();
        
        if self.is_profitable_transaction(transaction) {
            let opportunity = self.create_opportunity(transaction)?;
            opportunities.push(opportunity);
        }
        
        Ok(opportunities)
    }
    
    async fn start(&self) -> MevResult<()> {
        self.enabled.store(true, Ordering::SeqCst);
        Ok(())
    }
    
    async fn stop(&self) -> MevResult<()> {
        self.enabled.store(false, Ordering::SeqCst);
        Ok(())
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}
```

---

이 API 참조 문서는 xCrack MEV Searcher의 모든 공개 인터페이스와 사용법을 다룹니다. 각 섹션은 실제 코드 구현과 일치하며, 개발자들이 시스템을 이해하고 확장할 수 있도록 구성되었습니다.