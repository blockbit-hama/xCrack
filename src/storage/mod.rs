pub mod database;

use std::sync::Arc;
use anyhow::Result;
use redis::{AsyncCommands, Client as RedisClient};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;
use chrono::{Utc, DateTime};
use ethers::types::{Address, U256};

pub use database::Database;

#[derive(Clone)]
pub struct Storage {
    client: RedisClient,
    connection: Arc<Mutex<redis::aio::Connection>>,
}

impl Storage {
    pub async fn new(redis_url: Option<&str>) -> Result<Self> {
        let url = redis_url.unwrap_or("redis://127.0.0.1:6379");
        let client = RedisClient::open(url)?;
        let conn = client.get_async_connection().await?;
        info!("📦 Redis storage connected: {}", url);
        Ok(Self { client, connection: Arc::new(Mutex::new(conn)) })
    }

    async fn conn(&self) -> redis::aio::Connection {
        // Note: redis::aio::Connection is not Clone; reopen per call for simplicity
        // Keep original connection for basic health; open a fresh one for ops
        self.client.get_async_connection().await.expect("redis connection")
    }

    pub async fn save_user_position(&self, position: &UserPositionRecord) -> Result<()> {
        let key = format!("positions:{}:{}", to_hex(&position.user), to_hex(&position.protocol));
        let value = serde_json::to_string(position)?;
        let mut conn = self.conn().await;
        conn.set_ex::<_, _, ()>(key, value, 60 * 10).await?; // 10 minutes TTL
        Ok(())
    }

    pub async fn save_price_history(&self, record: &PriceHistoryRecord) -> Result<()> {
        let key = format!("price_history:{}", to_hex(&record.token));
        let value = serde_json::to_string(record)?;
        let mut conn = self.conn().await;
        let _: () = conn.rpush(&key, value).await?;
        let _: () = conn.ltrim(&key, -1000, -1).await?; // keep last 1000
        Ok(())
    }

    pub async fn get_recent_price_history(&self, token: Address, n: usize) -> Result<Vec<PriceHistoryRecord>> {
        let key = format!("price_history:{}", to_hex(&token));
        let mut conn = self.conn().await;
        let raw: Vec<String> = conn.lrange(key, -((n as isize).max(1)), -1).await.unwrap_or_default();
        let mut out = Vec::new();
        for s in raw {
            if let Ok(rec) = serde_json::from_str::<PriceHistoryRecord>(&s) {
                out.push(rec);
            }
        }
        Ok(out)
    }

    pub async fn save_liquidation_event(&self, event: &LiquidationEvent) -> Result<()> {
        let key = "liquidation_events";
        let value = serde_json::to_string(event)?;
        let mut conn = self.conn().await;
        let _: () = conn.rpush(key, value).await?;
        let _: () = conn.ltrim(key, -5000, -1).await?;
        Ok(())
    }

    pub async fn save_competitor_profile(&self, profile: &CompetitorProfile) -> Result<()> {
        let key = format!("competitor:{}", to_hex(&profile.address));
        let value = serde_json::to_string(profile)?;
        let mut conn = self.conn().await;
        conn.set_ex::<_, _, ()>(key, value, 60 * 60 * 24).await?; // 1 day TTL
        Ok(())
    }

        /// Get a specific user position snapshot
        pub async fn get_user_position(&self, user: Address, protocol: Address) -> Result<Option<UserPositionRecord>> {
            let key = format!("positions:{}:{}", to_hex(&user), to_hex(&protocol));
            let mut conn = self.conn().await;
            let raw: Option<String> = conn.get(key).await.ok();
            if let Some(s) = raw {
                match serde_json::from_str::<UserPositionRecord>(&s) {
                    Ok(rec) => Ok(Some(rec)),
                    Err(_) => Ok(None),
                }
            } else {
                Ok(None)
            }
        }

        /// Get the most recent N liquidation events (from the tail)
        pub async fn get_last_liquidation_events(&self, n: usize) -> Result<Vec<LiquidationEvent>> {
            let mut conn = self.conn().await;
            let raw: Vec<String> = conn.lrange("liquidation_events", -((n as isize).max(1)), -1).await.unwrap_or_default();
            let mut out = Vec::new();
            for s in raw {
                if let Ok(rec) = serde_json::from_str::<LiquidationEvent>(&s) {
                    out.push(rec);
                }
            }
            Ok(out)
        }
}

fn to_hex(addr: &Address) -> String {
    format!("0x{}", hex::encode(addr.as_bytes()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPositionRecord {
    pub user: Address,
    pub protocol: Address,
    pub health_factor: f64,
    pub total_collateral_usd: f64,
    pub total_debt_usd: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceHistoryRecord {
    pub token: Address,
    pub price_usd: f64,
    pub price_eth: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationEvent {
    pub protocol: String,
    pub user: Address,
    pub collateral_asset: Address,
    pub debt_asset: Address,
    pub debt_repaid: U256,
    pub collateral_received: U256,
    pub expected_profit: U256,
    pub gas_cost: U256,
    pub net_profit: U256,
    pub success: bool,
    pub reason: Option<String>,
    pub block_number: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitorProfile {
    pub address: Address,
    pub last_seen_block: Option<u64>,
    pub liquidation_txs_last_hour: u32,
    pub avg_gas_price_gwei: f64,
    pub win_rate_estimate: f64,
    pub notes: Option<String>,
}
