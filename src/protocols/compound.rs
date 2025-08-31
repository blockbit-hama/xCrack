use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use tracing::{info, debug, warn, error};
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

/// Compound v2 Protocol Scanner
pub struct CompoundV2Scanner {
    config: Arc<Config>,
    provider: Arc<Provider<Ws>>,
    comptroller_address: H160,
    comptroller_contract: Contract<Provider<Ws>>,
    ctoken_contracts: HashMap<H160, Contract<Provider<Ws>>>,
    supported_ctokens: Vec<H160>,
    oracle_address: H160,
    oracle_contract: Contract<Provider<Ws>>,
}

impl CompoundV2Scanner {
    pub async fn new(config: Arc<Config>, provider: Arc<Provider<Ws>>) -> Result<Self> {
        info!("🏦 Initializing Compound v2 Scanner...");
        
        // Compound v2 mainnet addresses
        let comptroller_address: H160 = "0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B".parse()?;
        let oracle_address: H160 = "0x046728da7cb8272284238bD3e47909823d63A58D".parse()?;
        
        // Load contract ABIs
        let comptroller_abi: Abi = serde_json::from_str(include_str!("../../abi/CompoundComptroller.json"))?;
        let ctoken_abi: Abi = serde_json::from_str(include_str!("../../abi/CompoundCToken.json"))?;
        let oracle_abi: Abi = serde_json::from_str(include_str!("../../abi/CompoundOracle.json"))?;
        
        // Create contract instances
        let comptroller_contract = Contract::new(comptroller_address, comptroller_abi, Arc::clone(&provider));
        let oracle_contract = Contract::new(oracle_address, oracle_abi, Arc::clone(&provider));
        
        // Get all supported cTokens
        let supported_ctokens = Self::get_supported_ctokens(&comptroller_contract).await?;
        
        // Create cToken contract instances
        let mut ctoken_contracts = HashMap::new();
        for &ctoken in &supported_ctokens {
            let contract = Contract::new(ctoken, ctoken_abi.clone(), Arc::clone(&provider));
            ctoken_contracts.insert(ctoken, contract);
        }
        
        info!("✅ Compound v2 Scanner initialized with {} cTokens", supported_ctokens.len());
        
        Ok(Self {
            config,
            provider,
            comptroller_address,
            comptroller_contract,
            ctoken_contracts,
            supported_ctokens,
            oracle_address,
            oracle_contract,
        })
    }
    
    async fn get_supported_ctokens(comptroller: &Contract<Provider<Ws>>) -> Result<Vec<H160>> {
        // Get all markets from Comptroller
        let all_markets: Vec<H160> = comptroller
            .method::<_, Vec<H160>>("getAllMarkets", ())?
            .call()
            .await
            .map_err(|e| anyhow!("Failed to get markets: {}", e))?;
            
        info!("📊 Found {} Compound markets", all_markets.len());
        Ok(all_markets)
    }
    
    async fn get_active_users(&self) -> Result<Vec<H160>> {
        let current_block = 0u64;
        let from_block = if current_block > 2000 { current_block - 2000 } else { 0 };
        
        let mut users = std::collections::HashSet::new();
        
        // Get users from recent Mint/Redeem/Borrow/Repay events across all cTokens
        for &ctoken in &self.supported_ctokens {
            // Mint events (supply)
            let mint_filter = Filter::new()
                .address(ctoken)
                .topic0(H256::from_slice(&hex::decode("4c209b5fc8ad50758f13e2e1088ba56a560dff690a1c6fef26394f4c03821c4f").unwrap()))
                .from_block(from_block)
                .to_block(BlockNumber::Latest);
                
            if let Ok(mint_logs) = self.provider.get_logs(&mint_filter).await {
                for log in mint_logs {
                    if let Some(user) = log.topics.get(1) {
                        users.insert(H160::from_slice(&user.0[12..]));
                    }
                }
            }
            
            // Borrow events
            let borrow_filter = Filter::new()
                .address(ctoken)
                .topic0(H256::from_slice(&hex::decode("13ed6866d4e1ee6da46f845c46d7e6aa3a3f7b92c2b8a7a7a0a6f2f1f5b9a8b7").unwrap()))
                .from_block(from_block)
                .to_block(BlockNumber::Latest);
                
            if let Ok(borrow_logs) = self.provider.get_logs(&borrow_filter).await {
                for log in borrow_logs {
                    if let Some(user) = log.topics.get(1) {
                        users.insert(H160::from_slice(&user.0[12..]));
                    }
                }
            }
            
            // Small delay to avoid rate limiting
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        
        let user_list: Vec<H160> = users.into_iter().collect();
        debug!("👥 Found {} active Compound users", user_list.len());
        Ok(user_list)
    }
    
    async fn get_user_account_data_detailed(&self, user: H160) -> Result<Option<LiquidatableUser>> {
        // Get account liquidity from Comptroller
        let account_liquidity: (EthersU256, EthersU256, EthersU256) = self.comptroller_contract
            .method::<_, (EthersU256, EthersU256, EthersU256)>("getAccountLiquidity", user)?
            .call()
            .await
            .map_err(|e| anyhow!("Failed to get account liquidity for {}: {}", user, e))?;
        
        let (error_code, liquidity, shortfall) = account_liquidity;
        
        // Skip if error or no shortfall (not liquidatable)
        if error_code != EthersU256::zero() || shortfall.is_zero() {
            return Ok(None);
        }
        
        let mut collateral_positions = Vec::new();
        let mut debt_positions = Vec::new();
        let mut max_liquidatable_debt = HashMap::new();
        let mut liquidation_bonus = HashMap::new();
        
        let mut total_collateral_usd = 0.0;
        let mut total_debt_usd = 0.0;
        
        // Get liquidation incentive from comptroller
        let liquidation_incentive: EthersU256 = self.comptroller_contract
            .method::<_, EthersU256>("liquidationIncentiveMantissa", ())?
            .call()
            .await
            .unwrap_or_else(|_| EthersU256::from(1.08e18 as u128)); // Default 8% bonus
            
        let liquidation_bonus_rate = (liquidation_incentive.as_u128() as f64 / 1e18) - 1.0;
        
        // Get close factor
        let close_factor: EthersU256 = self.comptroller_contract
            .method::<_, EthersU256>("closeFactorMantissa", ())?
            .call()
            .await
            .unwrap_or_else(|_| EthersU256::from(0.5e18 as u128)); // Default 50%
            
        let close_factor_rate = close_factor.as_u128() as f64 / 1e18;
        
        // Check each cToken for user positions
        for (&ctoken, contract) in &self.ctoken_contracts {
            // Get user's cToken balance
            let ctoken_balance: EthersU256 = contract
                .method::<_, EthersU256>("balanceOf", user)?
                .call()
                .await
                .unwrap_or_default();
            
            // Get user's borrow balance
            let borrow_balance: EthersU256 = contract
                .method::<_, EthersU256>("borrowBalanceStored", user)?
                .call()
                .await
                .unwrap_or_default();
            
            // Skip if no position in this market
            if ctoken_balance.is_zero() && borrow_balance.is_zero() {
                continue;
            }
            
            // Get underlying asset
            let underlying: H160 = if ctoken == "0x4Ddc2D193948926D02f9B1fE9e1daa0718270ED5".parse()? {
                // cEther special case
                H160::zero() // ETH
            } else {
                contract
                    .method::<_, H160>("underlying", ())?
                    .call()
                    .await
                    .unwrap_or_default()
            };
            
            // Get exchange rate
            let exchange_rate: EthersU256 = contract
                .method::<_, EthersU256>("exchangeRateStored", ())?
                .call()
                .await
                .unwrap_or_default();
            
            // Get price from oracle
            let underlying_price: EthersU256 = self.oracle_contract
                .method::<_, EthersU256>("getUnderlyingPrice", ctoken)?
                .call()
                .await
                .unwrap_or_default();
                
            let price_usd = underlying_price.as_u128() as f64 / 1e18;
            
            // Add supply position if exists
            if !ctoken_balance.is_zero() {
                let underlying_amount = ctoken_balance * exchange_rate / EthersU256::from(1e18 as u128);
                let supply_usd = underlying_amount.as_u128() as f64 * price_usd / 1e18;
                total_collateral_usd += supply_usd;
                
                collateral_positions.push(CollateralPosition {
                    asset: Address::from_slice(&underlying.0),
                    amount: U256::from_limbs_slice(&underlying_amount.0),
                    usd_value: supply_usd,
                    liquidation_threshold: 0.75, // Default for Compound
                    price_usd,
                });
            }
            
            // Add borrow position if exists
            if !borrow_balance.is_zero() {
                let debt_usd = borrow_balance.as_u128() as f64 * price_usd / 1e18;
                total_debt_usd += debt_usd;
                
                debt_positions.push(DebtPosition {
                    asset: Address::from_slice(&underlying.0),
                    amount: U256::from_limbs_slice(&borrow_balance.0),
                    usd_value: debt_usd,
                    borrow_rate: 0.0, // TODO: Get from cToken
                    price_usd,
                });
                
                // Calculate max liquidatable amount
                let max_liquidatable = borrow_balance * EthersU256::from((close_factor_rate * 1e18) as u128) / EthersU256::from(1e18 as u128);
                max_liquidatable_debt.insert(
                    Address::from_slice(&underlying.0),
                    U256::from_limbs_slice(&max_liquidatable.0)
                );
                
                liquidation_bonus.insert(Address::from_slice(&underlying.0), liquidation_bonus_rate);
            }
        }
        
        // Skip if no positions
        if collateral_positions.is_empty() || debt_positions.is_empty() {
            return Ok(None);
        }
        
        // Calculate health factor
        let health_factor = if total_debt_usd > 0.0 {
            total_collateral_usd * 0.75 / total_debt_usd // Assuming 75% liquidation threshold
        } else {
            f64::INFINITY
        };
        
        let user_account_data = UserAccountData {
            user: Address::from_slice(&user.0),
            protocol: ProtocolType::CompoundV2,
            total_collateral_usd,
            total_debt_usd,
            available_borrows_usd: liquidity.as_u128() as f64 / 1e18,
            current_liquidation_threshold: 0.75,
            ltv: 0.75,
            health_factor,
            last_updated: chrono::Utc::now(),
        };
        
        // Calculate priority score
        let shortfall_usd = shortfall.as_u128() as f64 / 1e18;
        let priority_score = total_debt_usd * shortfall_usd;
        
        Ok(Some(LiquidatableUser {
            address: Address::from_slice(&user.0),
            protocol: ProtocolType::CompoundV2,
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
impl ProtocolScanner for CompoundV2Scanner {
    async fn scan_all_users(&self) -> Result<Vec<LiquidatableUser>> {
        info!("🔍 Starting Compound v2 full user scan...");
        let start_time = std::time::Instant::now();
        
        let active_users = self.get_active_users().await?;
        let mut liquidatable_users = Vec::new();
        
        // Process users in batches
        const BATCH_SIZE: usize = 5; // Smaller batches for Compound due to more RPC calls
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
                        // User not liquidatable
                    }
                    Err(e) => {
                        error!("❌ Failed to get Compound user data: {}", e);
                    }
                }
            }
            
            // Delay to avoid rate limiting
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        
        // Sort by priority score
        liquidatable_users.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap_or(std::cmp::Ordering::Equal));
        
        let duration = start_time.elapsed();
        info!("✅ Compound v2 scan complete: {} liquidatable users found in {}ms", 
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
        
        let avg_health_factor = if liquidatable_users.is_empty() {
            1.0
        } else {
            let total: f64 = liquidatable_users.iter().map(|u| u.account_data.health_factor).sum();
            total / liquidatable_users.len() as f64
        };
        
        let total_tvl: f64 = liquidatable_users.iter().map(|u| u.account_data.total_collateral_usd).sum();
        let total_borrows: f64 = liquidatable_users.iter().map(|u| u.account_data.total_debt_usd).sum();
        
        Ok(ProtocolStats {
            protocol: ProtocolType::CompoundV2,
            total_users: active_users.len() as u64,
            liquidatable_users: liquidatable_users.len() as u64,
            total_tvl_usd: total_tvl,
            total_borrows_usd: total_borrows,
            avg_health_factor,
            last_scan_duration_ms: 0,
        })
    }
    
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::CompoundV2
    }
    
    fn is_healthy(&self) -> bool {
        true
    }
}