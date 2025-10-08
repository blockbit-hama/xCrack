use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{info, debug, error};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use ethers::types::Address;

use super::{ProtocolType, LiquidatableUser, UserAccountData, CollateralPosition, DebtPosition};

/// The Graph API 클라이언트
pub struct TheGraphClient {
    http_client: Client,
    aave_endpoint: String,
    compound_endpoint: String,
}

/// The Graph Subgraph 응답 구조
#[derive(Debug, Deserialize)]
struct SubgraphResponse {
    data: SubgraphData,
}

#[derive(Debug, Deserialize)]
struct SubgraphData {
    users: Vec<SubgraphUser>,
}

/// Subgraph User 데이터
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubgraphUser {
    id: String,
    health_factor: String,
    total_collateral_usd: String,
    total_debt_usd: String,
    borrowed_reserves_count: i32,
    #[serde(default)]
    reserves: Vec<SubgraphReserve>,
}

/// Subgraph Reserve 데이터
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubgraphReserve {
    current_atoken_balance: String,
    current_stable_debt: String,
    current_variable_debt: String,
    reserve: SubgraphReserveAsset,
}

#[derive(Debug, Deserialize)]
struct SubgraphReserveAsset {
    id: String,
    symbol: String,
    #[serde(rename = "underlyingAsset")]
    underlying_asset: String,
}

impl TheGraphClient {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
            // Aave V3 Mainnet Subgraph
            aave_endpoint: "https://api.thegraph.com/subgraphs/name/aave/protocol-v3".to_string(),
            // Compound V3 Mainnet Subgraph
            compound_endpoint: "https://api.thegraph.com/subgraphs/name/messari/compound-v3-ethereum".to_string(),
        }
    }

    /// Aave에서 청산 가능한 사용자 조회
    pub async fn get_aave_liquidatable_users(&self, limit: i32) -> Result<Vec<LiquidatableUser>> {
        info!("🔍 The Graph: Querying Aave liquidatable users (limit: {})", limit);

        // GraphQL 쿼리
        let query = serde_json::json!({
            "query": format!(r#"
                query {{
                    users(
                        where: {{ healthFactor_lt: "1.0" }}
                        orderBy: healthFactor
                        orderDirection: asc
                        first: {}
                    ) {{
                        id
                        healthFactor
                        totalCollateralUSD
                        totalDebtUSD
                        borrowedReservesCount
                        reserves(where: {{ currentATokenBalance_gt: "0" }}) {{
                            currentATokenBalance
                            currentStableDebt
                            currentVariableDebt
                            reserve {{
                                id
                                symbol
                                underlyingAsset
                            }}
                        }}
                    }}
                }}
            "#, limit)
        });

        // API 호출
        let response = self.http_client
            .post(&self.aave_endpoint)
            .json(&query)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("The Graph API request failed: {}", response.status()));
        }

        let result: SubgraphResponse = response.json().await?;

        // 파싱
        let mut liquidatable_users = Vec::new();
        for user in result.data.users {
            match self.parse_aave_user(user).await {
                Ok(liquidatable) => liquidatable_users.push(liquidatable),
                Err(e) => error!("Failed to parse user: {}", e),
            }
        }

        info!("✅ The Graph: Found {} liquidatable Aave users", liquidatable_users.len());
        Ok(liquidatable_users)
    }

    /// Aave 고위험 사용자 조회 (HF < 1.5)
    pub async fn get_aave_high_risk_users(&self, limit: i32) -> Result<Vec<LiquidatableUser>> {
        info!("🔍 The Graph: Querying Aave high-risk users (HF < 1.5, limit: {})", limit);

        let query = serde_json::json!({
            "query": format!(r#"
                query {{
                    users(
                        where: {{
                            healthFactor_lt: "1.5"
                            healthFactor_gt: "0"
                        }}
                        orderBy: healthFactor
                        orderDirection: asc
                        first: {}
                    ) {{
                        id
                        healthFactor
                        totalCollateralUSD
                        totalDebtUSD
                        borrowedReservesCount
                        reserves(where: {{ currentATokenBalance_gt: "0" }}) {{
                            currentATokenBalance
                            currentStableDebt
                            currentVariableDebt
                            reserve {{
                                id
                                symbol
                                underlyingAsset
                            }}
                        }}
                    }}
                }}
            "#, limit)
        });

        let response = self.http_client
            .post(&self.aave_endpoint)
            .json(&query)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("The Graph API request failed: {}", response.status()));
        }

        let result: SubgraphResponse = response.json().await?;

        let mut high_risk_users = Vec::new();
        for user in result.data.users {
            match self.parse_aave_user(user).await {
                Ok(liquidatable) => high_risk_users.push(liquidatable),
                Err(e) => error!("Failed to parse user: {}", e),
            }
        }

        info!("✅ The Graph: Found {} high-risk Aave users", high_risk_users.len());
        Ok(high_risk_users)
    }

    /// SubgraphUser를 LiquidatableUser로 변환
    async fn parse_aave_user(&self, user: SubgraphUser) -> Result<LiquidatableUser> {
        // 주소 파싱
        let address: Address = user.id.parse()?;

        // health_factor 파싱 (1e18 decimals)
        let health_factor = user.health_factor.parse::<f64>()
            .unwrap_or(1.0);

        // USD 가치 파싱
        let total_collateral_usd = user.total_collateral_usd.parse::<f64>()
            .unwrap_or(0.0);
        let total_debt_usd = user.total_debt_usd.parse::<f64>()
            .unwrap_or(0.0);

        // 담보/부채 포지션 파싱
        let mut collateral_positions = Vec::new();
        let mut debt_positions = Vec::new();
        let mut max_liquidatable_debt = std::collections::HashMap::new();
        let mut liquidation_bonus = std::collections::HashMap::new();

        for reserve in user.reserves {
            let asset: Address = reserve.reserve.underlying_asset.parse()?;

            // aToken 잔액 (담보)
            let atoken_balance = reserve.current_atoken_balance.parse::<f64>()
                .unwrap_or(0.0);

            if atoken_balance > 0.0 {
                collateral_positions.push(CollateralPosition {
                    asset,
                    amount: crate::common::abi::u256_from_f64(atoken_balance),
                    usd_value: atoken_balance, // 간단화
                    liquidation_threshold: 0.8, // Aave 기본값
                    price_usd: 1.0,
                });
            }

            // 부채
            let stable_debt = reserve.current_stable_debt.parse::<f64>()
                .unwrap_or(0.0);
            let variable_debt = reserve.current_variable_debt.parse::<f64>()
                .unwrap_or(0.0);
            let total_debt = stable_debt + variable_debt;

            if total_debt > 0.0 {
                debt_positions.push(DebtPosition {
                    asset,
                    amount: crate::common::abi::u256_from_f64(total_debt),
                    usd_value: total_debt,
                    borrow_rate: 0.0,
                    price_usd: 1.0,
                });

                // 최대 청산 가능 금액 (Aave는 50%)
                let max_liquidatable = crate::common::abi::u256_from_f64(total_debt * 0.5);
                max_liquidatable_debt.insert(asset, max_liquidatable);

                // 청산 보너스 (Aave는 5%)
                liquidation_bonus.insert(asset, 0.05);
            }
        }

        // 우선순위 점수 계산
        let priority_score = if health_factor > 0.0 && health_factor < 1.0 {
            total_debt_usd * (1.0 - health_factor) / health_factor
        } else if health_factor >= 1.0 {
            0.0 // 건강함
        } else {
            total_debt_usd * 1000.0 // HF = 0
        };

        Ok(LiquidatableUser {
            address,
            protocol: ProtocolType::Aave,
            account_data: UserAccountData {
                user: address,
                protocol: ProtocolType::Aave,
                total_collateral_usd,
                total_debt_usd,
                available_borrows_usd: 0.0,
                current_liquidation_threshold: 0.8,
                ltv: 0.75,
                health_factor,
                last_updated: chrono::Utc::now(),
            },
            collateral_positions,
            debt_positions,
            max_liquidatable_debt,
            liquidation_bonus,
            priority_score,
        })
    }

    /// 특정 사용자의 상세 정보 조회
    pub async fn get_user_details(&self, user_address: Address, protocol: ProtocolType) -> Result<Option<LiquidatableUser>> {
        debug!("🔍 The Graph: Querying user details for {:?}", user_address);

        let endpoint = match protocol {
            ProtocolType::Aave => &self.aave_endpoint,
            ProtocolType::CompoundV2 | ProtocolType::CompoundV3 => &self.compound_endpoint,
            _ => return Ok(None),
        };

        let user_id = format!("{:#x}", user_address).to_lowercase();

        let query = serde_json::json!({
            "query": format!(r#"
                query {{
                    user(id: "{}") {{
                        id
                        healthFactor
                        totalCollateralUSD
                        totalDebtUSD
                        borrowedReservesCount
                        reserves(where: {{ currentATokenBalance_gt: "0" }}) {{
                            currentATokenBalance
                            currentStableDebt
                            currentVariableDebt
                            reserve {{
                                id
                                symbol
                                underlyingAsset
                            }}
                        }}
                    }}
                }}
            "#, user_id)
        });

        let response = self.http_client
            .post(endpoint)
            .json(&query)
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        #[derive(Debug, Deserialize)]
        struct UserResponse {
            data: UserData,
        }

        #[derive(Debug, Deserialize)]
        struct UserData {
            user: Option<SubgraphUser>,
        }

        let result: UserResponse = response.json().await?;

        if let Some(user) = result.data.user {
            match self.parse_aave_user(user).await {
                Ok(liquidatable) => Ok(Some(liquidatable)),
                Err(e) => {
                    error!("Failed to parse user: {}", e);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
}

impl Default for TheGraphClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_aave_liquidatable_users() {
        let client = TheGraphClient::new();
        let result = client.get_aave_liquidatable_users(10).await;

        // 네트워크 오류 등으로 실패할 수 있음
        if let Ok(users) = result {
            println!("Found {} liquidatable users", users.len());
            for user in users.iter().take(3) {
                println!("  User: {:?}, HF: {:.4}", user.address, user.account_data.health_factor);
            }
        }
    }

    #[tokio::test]
    async fn test_get_aave_high_risk_users() {
        let client = TheGraphClient::new();
        let result = client.get_aave_high_risk_users(20).await;

        if let Ok(users) = result {
            println!("Found {} high-risk users", users.len());
            for user in users.iter().take(5) {
                println!("  User: {:?}, HF: {:.4}, Debt: ${:.2}",
                         user.address,
                         user.account_data.health_factor,
                         user.account_data.total_debt_usd);
            }
        }
    }
}
