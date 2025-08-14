use super::traits::{Bridge, BridgeQuote, BridgeError, BridgeResult};
use super::{StargateBridge, HopBridge, RubicBridge, SynapseBridge};
use crate::types::{ChainId, CrossChainToken, BridgeProtocol};
use alloy::primitives::U256;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, debug, warn};
use tokio::time::{timeout, Duration};

/// Route optimization strategy
#[derive(Debug, Clone)]
pub enum RouteStrategy {
    /// Minimize total cost (fees + gas)
    LowestCost,
    /// Minimize completion time
    FastestTime,
    /// Highest success rate
    MostReliable,
    /// Best overall score (balanced)
    Balanced,
}

/// Bridge performance metrics
#[derive(Debug, Clone)]
pub struct BridgeMetrics {
    pub success_rate: f64,
    pub avg_completion_time: u64,
    pub avg_cost: U256,
    pub liquidity: U256,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Bridge manager for routing and optimization
#[derive(Debug)]
pub struct BridgeManager {
    /// Available bridges
    bridges: HashMap<BridgeProtocol, Arc<dyn Bridge>>,
    
    /// Bridge performance cache
    metrics_cache: tokio::sync::RwLock<HashMap<BridgeProtocol, BridgeMetrics>>,
    
    /// Default route strategy
    default_strategy: RouteStrategy,
    
    /// Request timeout
    timeout_duration: Duration,
}

impl BridgeManager {
    /// Create new bridge manager with all available bridges
    pub fn new() -> Self {
        let mut bridges: HashMap<BridgeProtocol, Arc<dyn Bridge>> = HashMap::new();
        
        // Initialize all bridge implementations
        bridges.insert(BridgeProtocol::Stargate, Arc::new(StargateBridge::new()));
        bridges.insert(BridgeProtocol::Hop, Arc::new(HopBridge::new()));
        bridges.insert(BridgeProtocol::Rubic, Arc::new(RubicBridge::new()));
        bridges.insert(BridgeProtocol::Synapse, Arc::new(SynapseBridge::new()));
        
        Self {
            bridges,
            metrics_cache: tokio::sync::RwLock::new(HashMap::new()),
            default_strategy: RouteStrategy::Balanced,
            timeout_duration: Duration::from_secs(30),
        }
    }
    
    /// Set default route optimization strategy
    pub fn set_default_strategy(&mut self, strategy: RouteStrategy) {
        self.default_strategy = strategy;
    }
    
    /// Get best quote for a cross-chain transfer
    pub async fn get_best_quote(
        &self,
        from: ChainId,
        to: ChainId,
        token: &CrossChainToken,
        amount: U256,
        slippage: f64,
        strategy: Option<RouteStrategy>,
    ) -> BridgeResult<BridgeQuote> {
        let strategy = strategy.unwrap_or_else(|| self.default_strategy.clone());
        
        debug!("🔍 Getting quotes from all bridges for {} {} -> {}", 
               token.symbol, from.name(), to.name());
        
        // Get quotes from all supporting bridges in parallel
        let mut quote_futures = Vec::new();
        
        for (protocol, bridge) in &self.bridges {
            let protocol = protocol.clone();
            let bridge = Arc::clone(bridge);
            let token = token.clone();
            
            let future = async move {
                // Check if bridge supports this route
                match timeout(Duration::from_secs(5), bridge.supports_route(from, to, &token)).await {
                    Ok(Ok(supports)) if supports => {
                        // Get quote with timeout
                        match timeout(Duration::from_secs(10), bridge.get_quote(from, to, &token, amount, slippage)).await {
                            Ok(Ok(quote)) => Some((protocol, quote)),
                            Ok(Err(e)) => {
                                debug!("❌ {} quote failed: {}", bridge.name(), e);
                                None
                            }
                            Err(_) => {
                                warn!("⏰ {} quote timeout", bridge.name());
                                None
                            }
                        }
                    }
                    _ => None,
                }
            };
            
            quote_futures.push(future);
        }
        
        // Execute all quote requests concurrently
        let results = futures::future::join_all(quote_futures).await;
        let mut valid_quotes: Vec<(BridgeProtocol, BridgeQuote)> = results.into_iter()
            .filter_map(|result| result)
            .collect();
        
        if valid_quotes.is_empty() {
            return Err(BridgeError::UnsupportedRoute { from, to });
        }
        
        info!("📊 Got {} valid quotes", valid_quotes.len());
        
        // Sort quotes based on strategy
        self.sort_quotes_by_strategy(&mut valid_quotes, &strategy).await;
        
        // Return the best quote
        let (best_protocol, best_quote) = valid_quotes.into_iter().next().unwrap();
        
        info!("🏆 Best quote: {} with {} cost and {}s completion", 
               best_protocol.name(),
               best_quote.total_cost().to::<u64>() as f64 / 1_000_000.0,
               best_quote.estimated_time);
        
        Ok(best_quote)
    }
    
    /// Get quotes from all supporting bridges for comparison
    pub async fn get_all_quotes(
        &self,
        from: ChainId,
        to: ChainId,
        token: &CrossChainToken,
        amount: U256,
        slippage: f64,
    ) -> BridgeResult<Vec<(BridgeProtocol, BridgeQuote)>> {
        let mut all_quotes = Vec::new();
        
        for (protocol, bridge) in &self.bridges {
            // Check if bridge supports this route
            if let Ok(true) = bridge.supports_route(from, to, token).await {
                match timeout(self.timeout_duration, bridge.get_quote(from, to, token, amount, slippage)).await {
                    Ok(Ok(quote)) => {
                        all_quotes.push((protocol.clone(), quote));
                    }
                    Ok(Err(e)) => {
                        debug!("Bridge {} quote error: {}", protocol.name(), e);
                    }
                    Err(_) => {
                        warn!("Bridge {} timeout", protocol.name());
                    }
                }
            }
        }
        
        Ok(all_quotes)
    }
    
    /// Execute bridge transaction using specified bridge
    pub async fn execute_bridge(
        &self,
        protocol: BridgeProtocol,
        quote: &BridgeQuote,
    ) -> BridgeResult<super::traits::BridgeExecution> {
        let bridge = self.bridges.get(&protocol)
            .ok_or_else(|| BridgeError::BridgeUnavailable)?;
        
        info!("🚀 Executing bridge via {}: {} {} -> {}", 
               protocol.name(),
               quote.token.symbol,
               quote.source_chain.name(),
               quote.destination_chain.name());
        
        match timeout(self.timeout_duration, bridge.execute_bridge(quote)).await {
            Ok(result) => result,
            Err(_) => Err(BridgeError::ApiError {
                message: "Bridge execution timeout".to_string(),
            }),
        }
    }
    
    /// Update bridge metrics cache
    pub async fn update_metrics(&self) {
        let mut cache = self.metrics_cache.write().await;
        
        for (protocol, bridge) in &self.bridges {
            let metrics = BridgeMetrics {
                success_rate: bridge.get_success_rate().await.unwrap_or(0.0),
                avg_completion_time: bridge.get_avg_completion_time(ChainId::Ethereum, ChainId::Polygon).await.unwrap_or(600),
                avg_cost: U256::from(10_000u64), // Mock average cost
                liquidity: bridge.get_liquidity(ChainId::Ethereum, ChainId::Polygon, 
                    &CrossChainToken {
                        symbol: "USDC".to_string(),
                        addresses: HashMap::new(),
                        decimals: 6,
                    }).await.unwrap_or(U256::ZERO),
                last_updated: chrono::Utc::now(),
            };
            
            cache.insert(protocol.clone(), metrics);
        }
        
        info!("📊 Updated metrics for {} bridges", cache.len());
    }
    
    /// Get bridge metrics
    pub async fn get_bridge_metrics(&self, protocol: BridgeProtocol) -> Option<BridgeMetrics> {
        let cache = self.metrics_cache.read().await;
        cache.get(&protocol).cloned()
    }
    
    /// Get all available bridges
    pub fn get_available_bridges(&self) -> Vec<BridgeProtocol> {
        self.bridges.keys().cloned().collect()
    }
    
    /// Check if a specific route is supported by any bridge
    pub async fn is_route_supported(
        &self,
        from: ChainId,
        to: ChainId,
        token: &CrossChainToken,
    ) -> bool {
        for bridge in self.bridges.values() {
            if let Ok(true) = bridge.supports_route(from, to, token).await {
                return true;
            }
        }
        false
    }
    
    /// Sort quotes based on optimization strategy
    async fn sort_quotes_by_strategy(
        &self,
        quotes: &mut Vec<(BridgeProtocol, BridgeQuote)>,
        strategy: &RouteStrategy,
    ) {
        match strategy {
            RouteStrategy::LowestCost => {
                quotes.sort_by(|a, b| a.1.total_cost().cmp(&b.1.total_cost()));
            }
            RouteStrategy::FastestTime => {
                quotes.sort_by(|a, b| a.1.estimated_time.cmp(&b.1.estimated_time));
            }
            RouteStrategy::MostReliable => {
                // Sort by success rate (descending)
                let cache = self.metrics_cache.read().await;
                quotes.sort_by(|a, b| {
                    let rate_a = cache.get(&a.0).map(|m| m.success_rate).unwrap_or(0.0);
                    let rate_b = cache.get(&b.0).map(|m| m.success_rate).unwrap_or(0.0);
                    rate_b.partial_cmp(&rate_a).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            RouteStrategy::Balanced => {
                // Balanced scoring: (cost_score * 0.4 + time_score * 0.3 + reliability_score * 0.3)
                let cache = self.metrics_cache.read().await;
                
                quotes.sort_by(|a, b| {
                    let cost_a = a.1.total_cost().to::<u128>() as f64;
                    let cost_b = b.1.total_cost().to::<u128>() as f64;
                    let time_a = a.1.estimated_time as f64;
                    let time_b = b.1.estimated_time as f64;
                    let rate_a = cache.get(&a.0).map(|m| m.success_rate).unwrap_or(0.5);
                    let rate_b = cache.get(&b.0).map(|m| m.success_rate).unwrap_or(0.5);
                    
                    // Normalize scores (lower is better for cost and time, higher for reliability)
                    let cost_score_a = cost_a / (cost_a + cost_b);
                    let cost_score_b = cost_b / (cost_a + cost_b);
                    let time_score_a = time_a / (time_a + time_b);
                    let time_score_b = time_b / (time_a + time_b);
                    let reliability_score_a = rate_a;
                    let reliability_score_b = rate_b;
                    
                    let score_a = cost_score_a * 0.4 + time_score_a * 0.3 + (1.0 - reliability_score_a) * 0.3;
                    let score_b = cost_score_b * 0.4 + time_score_b * 0.3 + (1.0 - reliability_score_b) * 0.3;
                    
                    score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
    }
}

impl Default for BridgeManager {
    fn default() -> Self {
        Self::new()
    }
}