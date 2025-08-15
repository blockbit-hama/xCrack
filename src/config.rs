use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use ethers::types::{U256, H160};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub chain_id: u64,
    pub name: String,
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub block_time: u64,
    pub base_fee: Option<U256>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    pub name: String,
    pub router: H160,
    pub factory: H160,
    pub fee: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub network: NetworkConfig,
    pub strategies: StrategyConfig,
    pub flashbots: FlashbotsConfig,
    pub safety: SafetyConfig,
    pub monitoring: MonitoringConfig,
    pub performance: PerformanceConfig,
    pub dexes: Vec<DexConfig>,
    pub tokens: HashMap<String, String>, // symbol -> address
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub sandwich: SandwichConfig,
    pub liquidation: LiquidationConfig,
    pub micro_arbitrage: MicroArbitrageConfig,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandwichConfig {
    pub enabled: bool,
    pub min_target_value: String, // ETH amount
    pub max_slippage: f64,        // 0.03 = 3%
    pub max_frontrun_size: String, // ETH amount
    pub min_profit_eth: String, // minimum profit in ETH
    pub min_profit_percentage: f64, // minimum profit percentage
    pub gas_multiplier: f64, // gas price multiplier
    pub max_gas_price_gwei: String, // max gas price in gwei
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationConfig {
    pub enabled: bool,
    pub protocols: Vec<String>,
    pub min_health_factor: f64, // Below this, liquidation is considered
    pub max_liquidation_amount: String, // ETH amount
    pub min_profit_eth: String, // minimum profit in ETH
    pub min_liquidation_amount: String, // minimum liquidation amount
    pub gas_multiplier: f64, // gas price multiplier
    pub max_gas_price_gwei: String, // max gas price in gwei
    pub health_factor_threshold: f64, // health factor threshold
    pub max_liquidation_size: String, // max liquidation size
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroArbitrageConfig {
    pub enabled: bool,
    pub exchanges: Vec<ExchangeConfig>, // 모니터링할 거래소들
    pub trading_pairs: Vec<String>, // 거래할 토큰 페어들
    pub min_profit_percentage: f64, // 최소 수익률 (0.1% = 0.001)
    pub min_profit_usd: String, // 최소 수익 달러 금액
    pub max_position_size: String, // 최대 포지션 크기
    pub max_concurrent_trades: usize, // 최대 동시 거래 수
    pub execution_timeout_ms: u64, // 실행 타임아웃 (밀리초)
    pub latency_threshold_ms: u64, // 지연 임계값 (밀리초)
    pub price_update_interval_ms: u64, // 가격 업데이트 간격
    pub order_book_depth: u32, // 오더북 깊이
    pub slippage_tolerance: f64, // 슬리피지 허용치
    pub fee_tolerance: f64, // 수수료 허용치
    pub risk_limit_per_trade: String, // 거래당 위험 한도
    pub daily_volume_limit: String, // 일일 거래량 한도
    pub enable_cex_trading: bool, // CEX 거래 활성화
    pub enable_dex_trading: bool, // DEX 거래 활성화
    pub blacklist_tokens: Vec<String>, // 거래 금지 토큰들
    pub priority_tokens: Vec<String>, // 우선순위 토큰들
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub name: String,
    pub exchange_type: ExchangeType, // DEX or CEX
    pub enabled: bool,
    pub api_endpoint: String,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub trading_pairs: Vec<String>,
    pub fee_percentage: f64,
    pub min_order_size: String,
    pub max_order_size: String,
    pub rate_limit_per_second: u32,
    pub websocket_endpoint: Option<String>,
    pub supports_fast_execution: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExchangeType {
    DEX, // 탈중앙화 거래소 (Uniswap, SushiSwap 등)
    CEX, // 중앙화 거래소 (Binance, Coinbase 등)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashbotsConfig {
    pub relay_url: String,
    pub builder_urls: Vec<String>,
    pub max_priority_fee_per_gas: String, // gwei
    pub max_fee_per_gas: String,          // gwei
    pub private_key: String,
    pub network: String,
    #[serde(default)]
    pub simulation_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub max_concurrent_bundles: usize,
    pub max_daily_gas_spend: String,    // ETH amount
    pub emergency_stop_loss: String,    // ETH amount
    pub max_position_size: String,      // ETH amount
    pub enable_emergency_stop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_discord_alerts: bool,
    pub discord_webhook_url: String,
    pub enable_telegram_alerts: bool,
    pub telegram_bot_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub profit_report_interval: String, // cron expression
    pub log_level: String,
    pub metrics_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_analysis: usize,
    pub batch_processing_interval: u64, // milliseconds
    pub mempool_filter_min_value: String, // ETH amount
    pub mempool_filter_min_gas_price: String, // gwei
    pub mempool_filter_max_gas_price: String, // gwei
    pub enable_metrics: bool,
    pub cache_size: usize,
}

impl Config {
    pub async fn load(path: &str) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Self {
            network: NetworkConfig {
                chain_id: 1,
                name: "mainnet".to_string(),
                rpc_url: "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY".to_string(),
                ws_url: Some("wss://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY".to_string()),
                block_time: 12,
                base_fee: None,
            },
            strategies: StrategyConfig {
                sandwich: SandwichConfig {
                    enabled: true,
                    min_target_value: "0.5".to_string(),
                    max_slippage: 0.03,
                    max_frontrun_size: "5.0".to_string(),
                    min_profit_eth: "0.05".to_string(),
                    min_profit_percentage: 0.01,
                    gas_multiplier: 1.5,
                    max_gas_price_gwei: "200".to_string(),
                },
                liquidation: LiquidationConfig {
                    enabled: true,
                    protocols: vec![
                        "aave".to_string(),
                        "compound".to_string(),
                        "makerdao".to_string(),
                    ],
                    min_health_factor: 1.05,
                    max_liquidation_amount: "50.0".to_string(),
                    min_profit_eth: "0.05".to_string(),
                    min_liquidation_amount: "1.0".to_string(),
                    gas_multiplier: 1.5,
                    max_gas_price_gwei: "200".to_string(),
                    health_factor_threshold: 1.0,
                    max_liquidation_size: "10.0".to_string(),
                },
                micro_arbitrage: MicroArbitrageConfig {
                    enabled: true,
                    exchanges: vec![
                        // DEX 거래소들
                        ExchangeConfig {
                            name: "uniswap_v2".to_string(),
                            exchange_type: ExchangeType::DEX,
                            enabled: true,
                            api_endpoint: "https://api.uniswap.org/v1".to_string(),
                            api_key: None,
                            api_secret: None,
                            trading_pairs: vec!["WETH/USDC".to_string(), "WETH/USDT".to_string(), "WETH/DAI".to_string()],
                            fee_percentage: 0.003, // 0.3%
                            min_order_size: "10".to_string(), // 10 USDC
                            max_order_size: "50000".to_string(), // 50K USDC
                            rate_limit_per_second: 10,
                            websocket_endpoint: Some("wss://api.uniswap.org/ws".to_string()),
                            supports_fast_execution: true,
                        },
                        ExchangeConfig {
                            name: "sushiswap".to_string(),
                            exchange_type: ExchangeType::DEX,
                            enabled: true,
                            api_endpoint: "https://api.sushi.com/v1".to_string(),
                            api_key: None,
                            api_secret: None,
                            trading_pairs: vec!["WETH/USDC".to_string(), "WETH/USDT".to_string(), "WETH/DAI".to_string()],
                            fee_percentage: 0.003, // 0.3%
                            min_order_size: "10".to_string(),
                            max_order_size: "50000".to_string(),
                            rate_limit_per_second: 10,
                            websocket_endpoint: Some("wss://api.sushi.com/ws".to_string()),
                            supports_fast_execution: true,
                        },
                        // CEX 거래소들 (Mock용)
                        ExchangeConfig {
                            name: "mock_binance".to_string(),
                            exchange_type: ExchangeType::CEX,
                            enabled: true,
                            api_endpoint: "https://api.binance.com/api/v3".to_string(),
                            api_key: Some("mock_api_key".to_string()),
                            api_secret: Some("mock_api_secret".to_string()),
                            trading_pairs: vec!["ETHUSDC".to_string(), "ETHUSDT".to_string(), "ETHDAI".to_string()],
                            fee_percentage: 0.001, // 0.1%
                            min_order_size: "10".to_string(),
                            max_order_size: "100000".to_string(),
                            rate_limit_per_second: 20,
                            websocket_endpoint: Some("wss://stream.binance.com:9443/ws".to_string()),
                            supports_fast_execution: true,
                        },
                    ],
                    trading_pairs: vec![
                        "WETH/USDC".to_string(),
                        "WETH/USDT".to_string(),
                        "WETH/DAI".to_string(),
                        "WBTC/USDC".to_string(),
                        "WBTC/USDT".to_string(),
                    ],
                    min_profit_percentage: 0.001, // 0.1%
                    min_profit_usd: "5".to_string(), // 최소 5달러 수익
                    max_position_size: "10000".to_string(), // 최대 10K USDC
                    max_concurrent_trades: 50,
                    execution_timeout_ms: 100, // 100ms 타임아웃
                    latency_threshold_ms: 50, // 50ms 지연 임계값
                    price_update_interval_ms: 10, // 10ms마다 가격 업데이트
                    order_book_depth: 10, // 상위 10개 레벨
                    slippage_tolerance: 0.002, // 0.2% 슬리피지 허용
                    fee_tolerance: 0.001, // 0.1% 수수료 허용
                    risk_limit_per_trade: "1000".to_string(), // 거래당 1K USDC 위험 한도
                    daily_volume_limit: "500000".to_string(), // 일일 500K USDC 볼륨 한도
                    enable_cex_trading: true,
                    enable_dex_trading: true,
                    blacklist_tokens: vec!["SHIB".to_string(), "DOGE".to_string()], // 고변동성 토큰 제외
                    priority_tokens: vec!["WETH".to_string(), "WBTC".to_string(), "USDC".to_string(), "USDT".to_string()],
                },
            },
            flashbots: FlashbotsConfig {
                relay_url: "https://relay.flashbots.net".to_string(),
                builder_urls: vec![
                    "https://relay.flashbots.net".to_string(),
                    "https://builder0x69.io".to_string(),
                    "https://rpc.beaverbuild.org".to_string(),
                ],
                max_priority_fee_per_gas: "2".to_string(),
                max_fee_per_gas: "50".to_string(),
                private_key: "your_private_key_here".to_string(),
                network: "mainnet".to_string(),
                simulation_mode: false,
            },
            safety: SafetyConfig {
                max_concurrent_bundles: 5,
                max_daily_gas_spend: "1.0".to_string(),
                emergency_stop_loss: "0.1".to_string(),
                max_position_size: "10.0".to_string(),
                enable_emergency_stop: true,
            },
            monitoring: MonitoringConfig {
                enable_discord_alerts: false,
                discord_webhook_url: "".to_string(),
                enable_telegram_alerts: false,
                telegram_bot_token: None,
                telegram_chat_id: None,
                profit_report_interval: "0 8 * * *".to_string(),
                log_level: "info".to_string(),
                metrics_port: 9090,
            },
            performance: PerformanceConfig {
                max_concurrent_analysis: 10,
                batch_processing_interval: 100,
                mempool_filter_min_value: "0.1".to_string(),
                mempool_filter_min_gas_price: "10".to_string(),
                mempool_filter_max_gas_price: "200".to_string(),
                enable_metrics: true,
                cache_size: 10000,
            },
            dexes: vec![
                DexConfig {
                    name: "uniswap_v2".to_string(),
                    router: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".parse().unwrap(),
                    factory: "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".parse().unwrap(),
                    fee: 300, // 0.3%
                    enabled: true,
                },
                DexConfig {
                    name: "sushiswap".to_string(),
                    router: "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F".parse().unwrap(),
                    factory: "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac".parse().unwrap(),
                    fee: 300, // 0.3%
                    enabled: true,
                },
                DexConfig {
                    name: "uniswap_v3".to_string(),
                    router: "0xE592427A0AEce92De3Edee1F18E0157C05861564".parse().unwrap(),
                    factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984".parse().unwrap(),
                    fee: 500, // Variable fees
                    enabled: true,
                },
            ],
            tokens: {
                let mut tokens = HashMap::new();
                tokens.insert("WETH".to_string(), "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string());
                tokens.insert("USDC".to_string(), "0xA0b86a33E6417f8C681A1fFE6954e127c9cd8e46".to_string());
                tokens.insert("USDT".to_string(), "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string());
                tokens.insert("DAI".to_string(), "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string());
                tokens.insert("WBTC".to_string(), "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string());
                tokens
            },
        }
    }

    pub async fn save(&self, path: &str) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    pub fn get_dex_by_name(&self, name: &str) -> Option<&DexConfig> {
        self.dexes.iter().find(|dex| dex.name == name && dex.enabled)
    }

    pub fn get_enabled_dexes(&self) -> Vec<&DexConfig> {
        self.dexes.iter().filter(|dex| dex.enabled).collect()
    }

    pub fn get_token_address(&self, symbol: &str) -> Option<H160> {
        self.tokens.get(symbol)
            .and_then(|addr| addr.parse().ok())
    }

    pub fn validate(&self) -> Result<()> {
        // Validate network configuration
        if self.network.rpc_url.is_empty() {
            return Err(anyhow::anyhow!("Network RPC URL cannot be empty"));
        }

        // Validate flashbots configuration
        if self.flashbots.private_key.is_empty() || self.flashbots.private_key == "your_private_key_here" {
            return Err(anyhow::anyhow!("Flashbots private key must be configured"));
        }

        // Validate strategy thresholds
        if self.strategies.sandwich.enabled {
            let min_profit: f64 = self.strategies.sandwich.min_profit_eth.parse()
                .map_err(|_| anyhow::anyhow!("Invalid sandwich min profit threshold"))?;
            if min_profit <= 0.0 {
                return Err(anyhow::anyhow!("Sandwich min profit threshold must be positive"));
            }
        }

        // Validate micro arbitrage configuration
        if self.strategies.micro_arbitrage.enabled {
            let min_profit_usd: f64 = self.strategies.micro_arbitrage.min_profit_usd.parse()
                .map_err(|_| anyhow::anyhow!("Invalid micro arbitrage min profit USD"))?;
            if min_profit_usd <= 0.0 {
                return Err(anyhow::anyhow!("Micro arbitrage min profit USD must be positive"));
            }

            if self.strategies.micro_arbitrage.min_profit_percentage <= 0.0 {
                return Err(anyhow::anyhow!("Micro arbitrage min profit percentage must be positive"));
            }

            if self.strategies.micro_arbitrage.exchanges.is_empty() {
                return Err(anyhow::anyhow!("At least one exchange must be configured for micro arbitrage"));
            }

            if self.strategies.micro_arbitrage.trading_pairs.is_empty() {
                return Err(anyhow::anyhow!("At least one trading pair must be configured for micro arbitrage"));
            }

            if self.strategies.micro_arbitrage.execution_timeout_ms == 0 {
                return Err(anyhow::anyhow!("Execution timeout must be greater than 0"));
            }

            if self.strategies.micro_arbitrage.max_concurrent_trades == 0 {
                return Err(anyhow::anyhow!("Max concurrent trades must be greater than 0"));
            }
        }

        // Validate safety limits
        let emergency_stop: f64 = self.safety.emergency_stop_loss.parse()
            .map_err(|_| anyhow::anyhow!("Invalid emergency stop loss amount"))?;
        if emergency_stop <= 0.0 {
            return Err(anyhow::anyhow!("Emergency stop loss must be positive"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        
        // Test basic fields
        assert_eq!(config.network.chain_id, 1);
        assert_eq!(config.network.name, "mainnet");
        assert!(config.network.rpc_url.contains("alchemy"));
        assert!(config.network.ws_url.is_some());
        
        // Test strategies
        assert!(config.strategies.sandwich.enabled);
        assert!(config.strategies.sandwich.enabled);
        assert!(config.strategies.liquidation.enabled);
        
        // Test dexes
        assert_eq!(config.dexes.len(), 3);
        assert!(config.dexes.iter().any(|dex| dex.name == "uniswap_v2"));
        assert!(config.dexes.iter().any(|dex| dex.name == "sushiswap"));
        assert!(config.dexes.iter().any(|dex| dex.name == "uniswap_v3"));
        
        // Test tokens
        assert_eq!(config.tokens.len(), 5);
        assert!(config.tokens.contains_key("WETH"));
        assert!(config.tokens.contains_key("USDC"));
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        
        // Set valid values for the default config to pass validation
        config.flashbots.private_key = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string();
        
        // Valid config should pass
        assert!(config.validate().is_ok());
        
        // Empty RPC URL should fail
        config.network.rpc_url = "".to_string();
        assert!(config.validate().is_err());
        
        // Reset and test private key
        config = Config::default();
        config.flashbots.private_key = "".to_string();
        assert!(config.validate().is_err());
        
        // Reset and test invalid profit threshold
        config = Config::default();
        config.strategies.sandwich.min_profit_eth = "invalid".to_string();
        assert!(config.validate().is_err());
        
        // Reset and test negative emergency stop
        config = Config::default();
        config.safety.emergency_stop_loss = "-1.0".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_dex_by_name() {
        let config = Config::default();
        
        // Test existing DEX
        let uniswap = config.get_dex_by_name("uniswap_v2");
        assert!(uniswap.is_some());
        assert_eq!(uniswap.unwrap().name, "uniswap_v2");
        assert!(uniswap.unwrap().enabled);
        
        // Test non-existent DEX
        let fake_dex = config.get_dex_by_name("fake_dex");
        assert!(fake_dex.is_none());
    }

    #[test]
    fn test_get_enabled_dexes() {
        let config = Config::default();
        let enabled_dexes = config.get_enabled_dexes();
        
        // All default DEXes should be enabled
        assert_eq!(enabled_dexes.len(), 3);
        assert!(enabled_dexes.iter().all(|dex| dex.enabled));
    }

    #[test]
    fn test_get_token_address() {
        let config = Config::default();
        
        // Test existing token
        let weth_address = config.get_token_address("WETH");
        assert!(weth_address.is_some());
        
        // Test non-existent token
        let fake_token = config.get_token_address("FAKE");
        assert!(fake_token.is_none());
    }

    #[tokio::test]
    async fn test_config_serialization() {
        let config = Config::default();
        
        // Test TOML serialization
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        
        // Basic checks
        assert_eq!(config.network.chain_id, deserialized.network.chain_id);
        assert_eq!(config.strategies.sandwich.enabled, deserialized.strategies.sandwich.enabled);
        assert_eq!(config.dexes.len(), deserialized.dexes.len());
        assert_eq!(config.tokens.len(), deserialized.tokens.len());
    }
}