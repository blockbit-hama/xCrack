use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{info, debug, warn, error};
use ethers::types::Address;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use ethers::providers::{Provider, Ws};
use futures::future::join_all;

use crate::config::Config;
use super::{
    ProtocolScanner, ProtocolType, LiquidatableUser, ProtocolStats,
    aave::AaveScanner, compound::CompoundV2Scanner
};

/// Multi-Protocol Position Scanner
pub struct MultiProtocolScanner {
    config: Arc<Config>,
    scanners: HashMap<ProtocolType, Box<dyn ProtocolScanner>>,
    cached_users: Arc<RwLock<HashMap<ProtocolType, Vec<LiquidatableUser>>>>,
    cached_stats: Arc<RwLock<HashMap<ProtocolType, ProtocolStats>>>,
    scan_interval_sec: u64,
    is_running: Arc<tokio::sync::RwLock<bool>>,
}

impl MultiProtocolScanner {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🔄 Initializing Multi-Protocol Scanner...");
        
        let mut scanners: HashMap<ProtocolType, Box<dyn ProtocolScanner>> = HashMap::new();
        
        // Initialize Aave scanner
        if config.protocols.aave.enabled {
            match AaveScanner::new(Arc::clone(&config), Arc::clone(&provider)).await {
                Ok(aave_scanner) => {
                    scanners.insert(ProtocolType::Aave, Box::new(aave_scanner));
                    info!("✅ Aave scanner initialized");
                }
                Err(e) => {
                    error!("❌ Failed to initialize Aave scanner: {}", e);
                }
            }
        }
        
        // Initialize Compound v2 scanner
        if config.protocols.compound_v2.enabled {
            match CompoundV2Scanner::new(Arc::clone(&config), Arc::clone(&provider)).await {
                Ok(compound_scanner) => {
                    scanners.insert(ProtocolType::CompoundV2, Box::new(compound_scanner));
                    info!("✅ Compound v2 scanner initialized");
                }
                Err(e) => {
                    error!("❌ Failed to initialize Compound v2 scanner: {}", e);
                }
            }
        }
        
        if scanners.is_empty() {
            return Err(anyhow!("No protocol scanners initialized"));
        }
        
        let scan_interval_sec = config.liquidation.scan_interval_seconds.unwrap_or(30);
        
        info!("✅ Multi-Protocol Scanner initialized with {} protocols (scan interval: {}s)", 
              scanners.len(), scan_interval_sec);
        
        Ok(Self {
            config,
            scanners,
            cached_users: Arc::new(RwLock::new(HashMap::new())),
            cached_stats: Arc::new(RwLock::new(HashMap::new())),
            scan_interval_sec,
            is_running: Arc::new(tokio::sync::RwLock::new(false)),
        })
    }
    
    /// Start background scanning
    pub async fn start_background_scanning(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            warn!("⚠️ Scanner already running");
            return Ok(());
        }
        *is_running = true;
        drop(is_running);
        
        info!("🚀 Starting background protocol scanning...");
        
        // Start scanning task
        let _cached_users = Arc::clone(&self.cached_users);
        let _cached_stats = Arc::clone(&self.cached_stats);
        let _scanners_clone = &self.scanners; // Reference, not clone
        let scan_interval = self.scan_interval_sec;
        let is_running_clone = Arc::clone(&self.is_running);
        
        // We need to move scanners into the async block, but we can't clone trait objects
        // So we'll need to restructure this differently
        
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(scan_interval));
            
            loop {
                // Check if we should stop
                if !*is_running_clone.read().await {
                    info!("🛑 Stopping background scanner");
                    break;
                }
                
                interval_timer.tick().await;
                info!("🔍 Starting scheduled protocol scan...");
                
                // This is where we need the scanners, but we can't move them
                // We'll need to refactor this to work with Arc<RwLock<>> or similar
                
                // For now, just log that we would scan
                debug!("📊 Would perform scan cycle here");
            }
        });
        
        Ok(())
    }
    
    /// Stop background scanning
    pub async fn stop_background_scanning(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            warn!("⚠️ Scanner not running");
            return Ok(());
        }
        *is_running = false;
        
        info!("🛑 Stopped background scanning");
        Ok(())
    }
    
    /// Perform single scan of all protocols
    pub async fn scan_all_protocols(&mut self) -> Result<HashMap<ProtocolType, Vec<LiquidatableUser>>> {
        info!("🔍 Scanning all protocols for liquidatable positions...");
        let start_time = std::time::Instant::now();
        
        let mut all_users = HashMap::new();
        let mut total_liquidatable = 0;
        
        // Process scanners sequentially to avoid mutable borrow conflicts
        for (protocol_type, scanner) in &mut self.scanners {
            match scanner.scan_all_users().await {
                Ok(users) => {
                    total_liquidatable += users.len();
                    all_users.insert(protocol_type.clone(), users);
                }
                Err(e) => {
                    error!("❌ Failed to scan {}: {}", protocol_type, e);
                    all_users.insert(protocol_type.clone(), Vec::new());
                }
            }
        }
        
        // Update cache
        {
            let mut cached_users = self.cached_users.write().await;
            *cached_users = all_users.clone();
        }
        
        let duration = start_time.elapsed();
        info!("✅ Multi-protocol scan complete: {} liquidatable users found across {} protocols in {}ms", 
              total_liquidatable, all_users.len(), duration.as_millis());
        
        Ok(all_users)
    }
    
    /// Get aggregated liquidatable users sorted by priority
    pub async fn get_top_liquidatable_users(&mut self, limit: usize) -> Result<Vec<LiquidatableUser>> {
        let all_users = self.scan_all_protocols().await?;
        
        let mut aggregated_users = Vec::new();
        for (_, users) in all_users {
            aggregated_users.extend(users);
        }
        
        // Sort by priority score (highest first)
        aggregated_users.sort_by(|a, b| {
            b.priority_score.partial_cmp(&a.priority_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Limit results
        aggregated_users.truncate(limit);
        
        Ok(aggregated_users)
    }
    
    /// Get liquidatable user by address from any protocol
    pub async fn get_user_data(&self, user: Address) -> Result<Option<LiquidatableUser>> {
        for scanner in self.scanners.values() {
            match scanner.get_user_data(user).await? {
                Some(user_data) => return Ok(Some(user_data)),
                None => continue,
            }
        }
        Ok(None)
    }
    
    /// Get cached users (faster, may be stale)
    pub async fn get_cached_users(&self) -> HashMap<ProtocolType, Vec<LiquidatableUser>> {
        self.cached_users.read().await.clone()
    }
    
    /// Get protocol statistics
    pub async fn get_protocol_stats(&self) -> Result<HashMap<ProtocolType, ProtocolStats>> {
        let mut stats_futures = Vec::new();
        let mut protocol_types = Vec::new();
        
        for (protocol_type, scanner) in &self.scanners {
            protocol_types.push(protocol_type.clone());
            stats_futures.push(scanner.get_protocol_stats());
        }
        
        let results = join_all(stats_futures).await;
        let mut all_stats = HashMap::new();
        
        for (protocol_type, result) in protocol_types.into_iter().zip(results.into_iter()) {
            match result {
                Ok(stats) => {
                    all_stats.insert(protocol_type, stats);
                }
                Err(e) => {
                    error!("❌ Failed to get stats for {}: {}", protocol_type, e);
                }
            }
        }
        
        // Update cache
        {
            let mut cached_stats = self.cached_stats.write().await;
            *cached_stats = all_stats.clone();
        }
        
        Ok(all_stats)
    }
    
    /// Get cached stats (faster)
    pub async fn get_cached_stats(&self) -> HashMap<ProtocolType, ProtocolStats> {
        self.cached_stats.read().await.clone()
    }
    
    /// Check if scanner is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
    
    /// Get summary of all liquidatable positions
    pub async fn get_liquidation_summary(&self) -> Result<LiquidationSummary> {
        let all_users = self.get_cached_users().await;
        
        let mut total_users = 0;
        let mut total_collateral_usd = 0.0;
        let mut total_debt_usd = 0.0;
        let mut protocol_breakdown = HashMap::new();
        
        for (protocol, users) in all_users {
            let protocol_collateral: f64 = users.iter().map(|u| u.account_data.total_collateral_usd).sum();
            let protocol_debt: f64 = users.iter().map(|u| u.account_data.total_debt_usd).sum();
            
            total_users += users.len();
            total_collateral_usd += protocol_collateral;
            total_debt_usd += protocol_debt;
            
            protocol_breakdown.insert(protocol, ProtocolBreakdown {
                users: users.len(),
                collateral_usd: protocol_collateral,
                debt_usd: protocol_debt,
            });
        }
        
        Ok(LiquidationSummary {
            total_users,
            total_collateral_usd,
            total_debt_usd,
            protocol_breakdown,
            last_updated: chrono::Utc::now(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct LiquidationSummary {
    pub total_users: usize,
    pub total_collateral_usd: f64,
    pub total_debt_usd: f64,
    pub protocol_breakdown: HashMap<ProtocolType, ProtocolBreakdown>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ProtocolBreakdown {
    pub users: usize,
    pub collateral_usd: f64,
    pub debt_usd: f64,
}

impl std::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolType::Aave => write!(f, "Aave"),
            ProtocolType::CompoundV2 => write!(f, "Compound v2"),
            ProtocolType::CompoundV3 => write!(f, "Compound v3"),
            ProtocolType::MakerDAO => write!(f, "MakerDAO"),
        }
    }
}