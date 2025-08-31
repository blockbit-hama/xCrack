use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{info, debug, error};
use alloy::primitives::{Address, U256};
use ethers::{
    providers::{Provider, Ws, Middleware},
    contract::Contract,
    abi::Abi,
    types::{H160, H256, U256 as EthersU256, Filter, Log, BlockNumber},
};
use async_trait::async_trait;

use crate::config::Config;
use super::{
    ProtocolScanner, ProtocolType, LiquidatableUser, UserAccountData, 
    CollateralPosition, DebtPosition, ProtocolStats
};

/// Aave v3 Protocol Scanner
pub struct AaveScanner {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    pool_address: H160,
    data_provider_address: H160,
    oracle_address: H160,
    pool_contract: Contract<Provider<Ws>>,
    data_provider_contract: Contract<Provider<Ws>>,
    oracle_contract: Contract<Provider<Ws>>,
    supported_assets: Vec<H160>,
    last_scan_block: u64,
}

impl AaveScanner {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🏦 Initializing Aave v3 Scanner...");
        
        // Aave v3 mainnet addresses
        let pool_address: H160 = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".parse()?;
        let data_provider_address: H160 = "0x7B4EB56E7CD4b454BA8ff71E4518426369a138a3".parse()?;
        let oracle_address: H160 = "0x54586bE62E3c3580375aE3723C145253060Ca0C2".parse()?;
        
        // Load contract ABIs
        let pool_abi: Abi = serde_json::from_str(include_str!("../../abi/AavePool.json"))?;
        let data_provider_abi: Abi = serde_json::from_str(include_str!("../../abi/AaveDataProvider.json"))?;
        let oracle_abi: Abi = serde_json::from_str(include_str!("../../abi/AaveOracle.json"))?;
        
        // Create contract instances
        let pool_contract = Contract::new(pool_address, pool_abi, Arc::clone(&provider));
        let data_provider_contract = Contract::new(data_provider_address, data_provider_abi, Arc::clone(&provider));
        let oracle_contract = Contract::new(oracle_address, oracle_abi, Arc::clone(&provider));
        
        // Get supported assets
        let supported_assets = Self::get_supported_assets(&data_provider_contract).await?;
        
        let current_block = 0u64;
        
        info!("✅ Aave v3 Scanner initialized with {} assets", supported_assets.len());
        
        Ok(Self {
            config,
            provider,
            pool_address,
            data_provider_address,
            oracle_address,
            pool_contract,
            data_provider_contract,
            oracle_contract,
            supported_assets,
            last_scan_block: current_block,
        })
    }
    
    async fn get_supported_assets(data_provider: &Contract<Provider<Ws>>) -> Result<Vec<H160>> {
        // Get all reserves from Aave data provider
        let all_reserves: Vec<H160> = data_provider
            .method::<_, Vec<H160>>("getAllReservesTokens", ())?
            .call()
            .await
            .map_err(|e| anyhow!("Failed to get reserves: {}", e))?
            .into_iter()
            .map(|addr| addr.into())
            .collect();
            
        info!("📊 Found {} Aave reserves", all_reserves.len());
        Ok(all_reserves)
    }
    
    async fn get_active_users(&self) -> Result<Vec<H160>> {
        let current_block = 0u64;
        let from_block = if current_block > 1000 { current_block - 1000 } else { 0 };
        
        let mut users = std::collections::HashSet::new();
        
        // Get users from recent Deposit events
        let deposit_filter = Filter::new()
            .address(self.pool_address)
            .topic0(H256::from_slice(&hex::decode("2b627736bca15cd5381dcf80b85eaae9c6d54c5fc5d0b6b3e6b39e6c3c00ea7").unwrap()))
            .from_block(from_block)
            .to_block(BlockNumber::Latest);
            
        let deposit_logs: Vec<Log> = self.provider.get_logs(&deposit_filter).await?;
        
        for log in deposit_logs {
            if let Some(user) = log.topics.get(2) {
                users.insert(H160::from_slice(&user.0[12..]));
            }
        }
        
        // Get users from recent Borrow events
        let borrow_filter = Filter::new()
            .address(self.pool_address)
            .topic0(H256::from_slice(&hex::decode("13ed6866d4e1ee6da46f845c46d7e6aa3a3f7b92e5a6a8b7a2b8b0a7a0a7a0a7").unwrap()))
            .from_block(from_block)
            .to_block(BlockNumber::Latest);
            
        let borrow_logs: Vec<Log> = self.provider.get_logs(&borrow_filter).await?;
        
        for log in borrow_logs {
            if let Some(user) = log.topics.get(1) {
                users.insert(H160::from_slice(&user.0[12..]));
            }
        }
        
        // self.last_scan_block = current_block; // TODO: 내부 가변성 패턴 적용 필요
        let user_list: Vec<H160> = users.into_iter().collect();
        
        debug!("👥 Found {} active Aave users", user_list.len());
        Ok(user_list)
    }
    
    async fn get_user_account_data_detailed(&self, user: H160) -> Result<Option<LiquidatableUser>> {
        // Get user account data from Aave Pool
        let account_data: (EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, EthersU256) = self.pool_contract
            .method::<_, (EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, EthersU256)>(
                "getUserAccountData", 
                user
            )?
            .call()
            .await
            .map_err(|e| anyhow!("Failed to get user account data for {}: {}", user, e))?;
        
        let (
            total_collateral_base,
            total_debt_base,
            available_borrows_base,
            current_liquidation_threshold,
            ltv,
            health_factor
        ) = account_data;
        
        // Convert health factor (1e18 = 1.0)
        let health_factor_f64 = health_factor.as_u128() as f64 / 1e18;
        
        // Skip if user has no debt or is healthy
        if total_debt_base.is_zero() || health_factor_f64 >= 1.0 {
            return Ok(None);
        }
        
        // Get detailed position data
        let mut collateral_positions = Vec::new();
        let mut debt_positions = Vec::new();
        let mut max_liquidatable_debt = HashMap::new();
        let mut liquidation_bonus = HashMap::new();
        
        for &asset in &self.supported_assets {
            // Get user reserve data
            let reserve_data: (EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, u32, bool) = self.data_provider_contract
                .method::<_, (EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, EthersU256, u32, bool)>(
                    "getUserReserveData",
                    (asset, user)
                )?
                .call()
                .await
                .unwrap_or_default();
            
            let (
                current_atoken_balance,
                current_stable_debt,
                current_variable_debt,
                _principal_stable_debt,
                _scaled_variable_debt,
                _stable_borrow_rate,
                _liquidity_rate,
                _stable_rate_last_updated,
                usage_as_collateral_enabled
            ) = reserve_data;
            
            // Get asset price
            let asset_price: EthersU256 = self.oracle_contract
                .method::<_, EthersU256>("getAssetPrice", asset)?
                .call()
                .await
                .unwrap_or_default();
            
            let price_usd = asset_price.as_u128() as f64 / 1e8; // Chainlink 8 decimals
            
            // Add collateral position if exists
            if !current_atoken_balance.is_zero() && usage_as_collateral_enabled {
                let collateral_usd = current_atoken_balance.as_u128() as f64 * price_usd / 1e18;
                
                collateral_positions.push(CollateralPosition {
                    asset: Address::from_slice(&asset.0),
                    amount: U256::from_limbs_slice(&current_atoken_balance.0),
                    usd_value: collateral_usd,
                    liquidation_threshold: current_liquidation_threshold.as_u128() as f64 / 1e4,
                    price_usd,
                });
            }
            
            // Add debt position if exists
            let total_debt = current_stable_debt + current_variable_debt;
            if !total_debt.is_zero() {
                let debt_usd = total_debt.as_u128() as f64 * price_usd / 1e18;
                
                debt_positions.push(DebtPosition {
                    asset: Address::from_slice(&asset.0),
                    amount: U256::from_limbs_slice(&total_debt.0),
                    usd_value: debt_usd,
                    borrow_rate: 0.0, // TODO: Get from reserve data
                    price_usd,
                });
                
                // Calculate max liquidatable amount (50% of debt for most assets)
                let close_factor = 0.5;
                let max_liquidatable = U256::from_limbs_slice(&(total_debt * EthersU256::from((close_factor * 1e18) as u128) / EthersU256::from(1e18 as u128)).0);
                max_liquidatable_debt.insert(Address::from_slice(&asset.0), max_liquidatable);
                
                // Get liquidation bonus (typically 5-10%)
                liquidation_bonus.insert(Address::from_slice(&asset.0), 0.05); // 5% default
            }
        }
        
        // Skip if no positions
        if collateral_positions.is_empty() || debt_positions.is_empty() {
            return Ok(None);
        }
        
        let total_collateral_usd = total_collateral_base.as_u128() as f64 / 1e8;
        let total_debt_usd = total_debt_base.as_u128() as f64 / 1e8;
        
        let user_account_data = UserAccountData {
            user: Address::from_slice(&user.0),
            protocol: ProtocolType::Aave,
            total_collateral_usd,
            total_debt_usd,
            available_borrows_usd: available_borrows_base.as_u128() as f64 / 1e8,
            current_liquidation_threshold: current_liquidation_threshold.as_u128() as f64 / 1e4,
            ltv: ltv.as_u128() as f64 / 1e4,
            health_factor: health_factor_f64,
            last_updated: chrono::Utc::now(),
        };
        
        // Calculate priority score (lower health factor = higher priority)
        let priority_score = if health_factor_f64 > 0.0 {
            total_debt_usd * (1.0 - health_factor_f64) / health_factor_f64
        } else {
            total_debt_usd * 1000.0 // Very high priority for HF=0
        };
        
        Ok(Some(LiquidatableUser {
            address: Address::from_slice(&user.0),
            protocol: ProtocolType::Aave,
            account_data: user_account_data,
            collateral_positions,
            debt_positions,
            max_liquidatable_debt,
            liquidation_bonus,
            priority_score,
        }))
    }
}

#[async_trait]
impl ProtocolScanner for AaveScanner {
    async fn scan_all_users(&self) -> Result<Vec<LiquidatableUser>> {
        info!("🔍 Starting Aave full user scan...");
        let start_time = std::time::Instant::now();
        
        let active_users = self.get_active_users().await?;
        let mut liquidatable_users = Vec::new();
        
        // Process users in batches to avoid rate limiting
        const BATCH_SIZE: usize = 10;
        for chunk in active_users.chunks(BATCH_SIZE) {
            let mut batch_futures = Vec::new();
            
            for &user in chunk {
                batch_futures.push(self.get_user_account_data_detailed(user));
            }
            
            let results = futures::future::join_all(batch_futures).await;
            
            for result in results {
                match result {
                    Ok(Some(liquidatable_user)) => {
                        liquidatable_users.push(liquidatable_user);
                    }
                    Ok(None) => {
                        // User not liquidatable, skip
                    }
                    Err(e) => {
                        error!("❌ Failed to get user data: {}", e);
                    }
                }
            }
            
            // Small delay to avoid rate limiting
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        
        // Sort by priority score (highest first)
        liquidatable_users.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap_or(std::cmp::Ordering::Equal));
        
        let duration = start_time.elapsed();
        info!("✅ Aave scan complete: {} liquidatable users found in {}ms", 
              liquidatable_users.len(), duration.as_millis());
        
        Ok(liquidatable_users)
    }
    
    async fn get_user_data(&self, user: Address) -> Result<Option<LiquidatableUser>> {
        let h160_user = H160::from_slice(user.as_slice());
        self.get_user_account_data_detailed(h160_user).await
    }
    
    async fn get_protocol_stats(&self) -> Result<ProtocolStats> {
        let active_users = self.get_active_users().await?;
        let liquidatable_users = self.scan_all_users().await?;
        
        let total_health_factor: f64 = liquidatable_users
            .iter()
            .map(|u| u.account_data.health_factor)
            .sum();
        
        let avg_health_factor = if liquidatable_users.is_empty() {
            1.0
        } else {
            total_health_factor / liquidatable_users.len() as f64
        };
        
        let total_tvl: f64 = liquidatable_users
            .iter()
            .map(|u| u.account_data.total_collateral_usd)
            .sum();
            
        let total_borrows: f64 = liquidatable_users
            .iter()
            .map(|u| u.account_data.total_debt_usd)
            .sum();
        
        Ok(ProtocolStats {
            protocol: ProtocolType::Aave,
            total_users: active_users.len() as u64,
            liquidatable_users: liquidatable_users.len() as u64,
            total_tvl_usd: total_tvl,
            total_borrows_usd: total_borrows,
            avg_health_factor,
            last_scan_duration_ms: 0, // TODO: Track this
        })
    }
    
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Aave
    }
    
    fn is_healthy(&self) -> bool {
        // Check if we can make RPC calls
        true // TODO: Implement health check
    }
}