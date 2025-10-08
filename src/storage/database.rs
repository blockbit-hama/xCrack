use anyhow::{Result, Context};
use sqlx::{PgPool, postgres::PgPoolOptions, Row};
use tracing::{info, error, debug};
use std::sync::Arc;
use ethers::types::Address;
use chrono::{DateTime, Utc};

use crate::protocols::{LiquidatableUser, UserAccountData, CollateralPosition, DebtPosition};

/// PostgreSQL 데이터베이스 클라이언트
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// 데이터베이스 연결 생성
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("🔌 Connecting to PostgreSQL database...");

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .context("Failed to connect to PostgreSQL")?;

        info!("✅ PostgreSQL connection established");

        Ok(Self { pool })
    }

    /// 기본 환경변수에서 연결
    pub async fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://xcrack:xcrack_password@localhost:5432/xcrack_liquidation".to_string());

        Self::new(&database_url).await
    }

    /// 사용자 정보 업데이트 (upsert)
    pub async fn upsert_user(&self, user: &LiquidatableUser) -> Result<()> {
        let address_str = format!("{:#x}", user.address);
        let protocol_str = format!("{:?}", user.protocol);

        sqlx::query!(
            r#"
            INSERT INTO users (
                address, protocol, health_factor, total_collateral_usd, total_debt_usd,
                available_borrows_usd, liquidation_threshold, ltv, is_liquidatable, priority_score
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (address)
            DO UPDATE SET
                protocol = $2,
                health_factor = $3,
                total_collateral_usd = $4,
                total_debt_usd = $5,
                available_borrows_usd = $6,
                liquidation_threshold = $7,
                ltv = $8,
                is_liquidatable = $9,
                priority_score = $10,
                scan_count = users.scan_count + 1
            "#,
            address_str,
            protocol_str,
            user.account_data.health_factor,
            user.account_data.total_collateral_usd,
            user.account_data.total_debt_usd,
            user.account_data.available_borrows_usd,
            user.account_data.current_liquidation_threshold,
            user.account_data.ltv,
            user.account_data.health_factor < 1.0,
            user.priority_score
        )
        .execute(&self.pool)
        .await
        .context("Failed to upsert user")?;

        // 담보 포지션 업데이트
        self.update_collateral_positions(&user.address, &user.collateral_positions).await?;

        // 부채 포지션 업데이트
        self.update_debt_positions(&user.address, &user.debt_positions).await?;

        Ok(())
    }

    /// 담보 포지션 업데이트
    async fn update_collateral_positions(&self, user_address: &Address, positions: &[CollateralPosition]) -> Result<()> {
        let user_addr_str = format!("{:#x}", user_address);

        // 기존 포지션 삭제
        sqlx::query!(
            "DELETE FROM collateral_positions WHERE user_address = $1",
            user_addr_str
        )
        .execute(&self.pool)
        .await?;

        // 새 포지션 삽입
        for pos in positions {
            let asset_str = format!("{:#x}", pos.asset);
            let amount_str = pos.amount.to_string();

            sqlx::query!(
                r#"
                INSERT INTO collateral_positions (
                    user_address, asset_address, amount, usd_value, liquidation_threshold, price_usd
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                user_addr_str,
                asset_str,
                amount_str,
                pos.usd_value,
                pos.liquidation_threshold,
                pos.price_usd
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// 부채 포지션 업데이트
    async fn update_debt_positions(&self, user_address: &Address, positions: &[DebtPosition]) -> Result<()> {
        let user_addr_str = format!("{:#x}", user_address);

        // 기존 포지션 삭제
        sqlx::query!(
            "DELETE FROM debt_positions WHERE user_address = $1",
            user_addr_str
        )
        .execute(&self.pool)
        .await?;

        // 새 포지션 삽입
        for pos in positions {
            let asset_str = format!("{:#x}", pos.asset);
            let amount_str = pos.amount.to_string();

            sqlx::query!(
                r#"
                INSERT INTO debt_positions (
                    user_address, asset_address, amount, usd_value, borrow_rate, price_usd
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                user_addr_str,
                asset_str,
                amount_str,
                pos.usd_value,
                pos.borrow_rate,
                pos.price_usd
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    /// 고위험 사용자 조회 (health_factor < 1.5)
    pub async fn get_high_risk_users(&self, limit: i64) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            r#"
            SELECT address
            FROM users
            WHERE health_factor < 1.5 AND health_factor > 0
            ORDER BY health_factor ASC, priority_score DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get high-risk users")?;

        Ok(rows.into_iter().map(|r| r.address).collect())
    }

    /// 청산 가능한 사용자 조회 (health_factor < 1.0)
    pub async fn get_liquidatable_users(&self, limit: i64) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            r#"
            SELECT address
            FROM users
            WHERE is_liquidatable = true
            ORDER BY priority_score DESC, health_factor ASC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get liquidatable users")?;

        Ok(rows.into_iter().map(|r| r.address).collect())
    }

    /// 특정 프로토콜의 청산 가능 사용자 조회
    pub async fn get_liquidatable_users_by_protocol(&self, protocol: &str, limit: i64) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            r#"
            SELECT address
            FROM users
            WHERE is_liquidatable = true AND protocol = $1
            ORDER BY priority_score DESC, health_factor ASC
            LIMIT $2
            "#,
            protocol,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get liquidatable users by protocol")?;

        Ok(rows.into_iter().map(|r| r.address).collect())
    }

    /// 청산 기회 기록
    pub async fn insert_liquidation_opportunity(
        &self,
        user_address: &Address,
        protocol: &str,
        health_factor: f64,
        estimated_profit_usd: f64,
        priority_score: f64,
    ) -> Result<uuid::Uuid> {
        let user_addr_str = format!("{:#x}", user_address);

        let row = sqlx::query!(
            r#"
            INSERT INTO liquidation_opportunities (
                user_address, protocol, health_factor, estimated_profit_usd, priority_score
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            "#,
            user_addr_str,
            protocol,
            health_factor,
            estimated_profit_usd,
            priority_score
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to insert liquidation opportunity")?;

        Ok(row.id)
    }

    /// 청산 기회 상태 업데이트
    pub async fn update_liquidation_opportunity_status(
        &self,
        opportunity_id: uuid::Uuid,
        status: &str,
        tx_hash: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE liquidation_opportunities
            SET status = $1,
                tx_hash = $2,
                error_message = $3,
                executed_at = CASE WHEN $1 = 'executing' THEN CURRENT_TIMESTAMP ELSE executed_at END,
                completed_at = CASE WHEN $1 IN ('completed', 'failed') THEN CURRENT_TIMESTAMP ELSE completed_at END
            WHERE id = $4
            "#,
            status,
            tx_hash,
            error_message,
            opportunity_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update opportunity status")?;

        Ok(())
    }

    /// 청산 히스토리 기록
    pub async fn insert_liquidation_history(
        &self,
        opportunity_id: Option<uuid::Uuid>,
        user_address: &Address,
        protocol: &str,
        collateral_asset: &Address,
        debt_asset: &Address,
        liquidated_debt: &str,
        received_collateral: &str,
        profit_usd: f64,
        gas_used: i64,
        gas_price_gwei: f64,
        tx_hash: &str,
        block_number: i64,
        success: bool,
    ) -> Result<()> {
        let user_addr_str = format!("{:#x}", user_address);
        let collateral_str = format!("{:#x}", collateral_asset);
        let debt_str = format!("{:#x}", debt_asset);

        sqlx::query!(
            r#"
            INSERT INTO liquidation_history (
                opportunity_id, user_address, protocol, collateral_asset, debt_asset,
                liquidated_debt, received_collateral, profit_usd, gas_used, gas_price_gwei,
                tx_hash, block_number, success
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
            opportunity_id,
            user_addr_str,
            protocol,
            collateral_str,
            debt_str,
            liquidated_debt,
            received_collateral,
            profit_usd,
            gas_used,
            gas_price_gwei,
            tx_hash,
            block_number,
            success
        )
        .execute(&self.pool)
        .await
        .context("Failed to insert liquidation history")?;

        Ok(())
    }

    /// 통계 조회
    pub async fn get_statistics(&self) -> Result<DatabaseStatistics> {
        let row = sqlx::query!(
            r#"
            SELECT
                (SELECT COUNT(*) FROM users) as total_users,
                (SELECT COUNT(*) FROM users WHERE is_liquidatable = true) as liquidatable_users,
                (SELECT COUNT(*) FROM liquidation_opportunities WHERE status = 'completed') as completed_liquidations,
                (SELECT COALESCE(SUM(profit_usd), 0) FROM liquidation_history WHERE success = true) as total_profit_usd,
                (SELECT COALESCE(AVG(health_factor), 1.0) FROM users WHERE health_factor > 0) as avg_health_factor
            "#
        )
        .fetch_one(&self.pool)
        .await
        .context("Failed to get statistics")?;

        Ok(DatabaseStatistics {
            total_users: row.total_users.unwrap_or(0),
            liquidatable_users: row.liquidatable_users.unwrap_or(0),
            completed_liquidations: row.completed_liquidations.unwrap_or(0),
            total_profit_usd: row.total_profit_usd.unwrap_or(0.0),
            avg_health_factor: row.avg_health_factor.unwrap_or(1.0),
        })
    }

    /// 데이터베이스 연결 테스트
    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .context("Database health check failed")?;

        Ok(true)
    }
}

/// 데이터베이스 통계
#[derive(Debug, Clone)]
pub struct DatabaseStatistics {
    pub total_users: i64,
    pub liquidatable_users: i64,
    pub completed_liquidations: i64,
    pub total_profit_usd: f64,
    pub avg_health_factor: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Docker가 실행 중일 때만 테스트
    async fn test_database_connection() {
        let db = Database::from_env().await.unwrap();
        assert!(db.health_check().await.unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_statistics() {
        let db = Database::from_env().await.unwrap();
        let stats = db.get_statistics().await.unwrap();
        println!("Database Stats: {:?}", stats);
    }
}
