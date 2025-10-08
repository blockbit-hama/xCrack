use super::types::SandwichStats;
use ethers::types::U256;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// 샌드위치 통계 매니저
pub struct SandwichStatsManager {
    stats: Arc<RwLock<SandwichStats>>,
}

impl SandwichStatsManager {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(SandwichStats::default())),
        }
    }

    pub async fn record_opportunity_detected(&self) {
        let mut stats = self.stats.write().await;
        stats.total_opportunities_detected += 1;
    }

    pub async fn record_opportunity_analyzed(&self) {
        let mut stats = self.stats.write().await;
        stats.total_opportunities_analyzed += 1;
    }

    pub async fn record_bundle_submitted(&self) {
        let mut stats = self.stats.write().await;
        stats.total_bundles_submitted += 1;
    }

    pub async fn record_bundle_included(&self) {
        let mut stats = self.stats.write().await;
        stats.total_bundles_included += 1;
    }

    pub async fn record_successful_sandwich(&self, profit: U256, gas_cost: U256) {
        let mut stats = self.stats.write().await;
        stats.update_success(profit, gas_cost);
    }

    pub async fn record_failed_sandwich(&self) {
        let mut stats = self.stats.write().await;
        stats.update_failure();
    }

    pub async fn get_stats(&self) -> SandwichStats {
        self.stats.read().await.clone()
    }

    pub async fn print_stats(&self) {
        let stats = self.stats.read().await;

        info!("📊 ===== 샌드위치 전략 통계 =====");
        info!("🔍 기회 탐지: {}", stats.total_opportunities_detected);
        info!("📈 기회 분석: {}", stats.total_opportunities_analyzed);
        info!("📤 번들 제출: {}", stats.total_bundles_submitted);
        info!("✅ 번들 포함: {}", stats.total_bundles_included);
        info!("🎯 성공한 샌드위치: {}", stats.total_successful_sandwiches);
        info!("❌ 실패한 샌드위치: {}", stats.total_failed_sandwiches);
        info!("💰 총 수익: {} ETH", format_eth(stats.total_profit));
        info!("⛽ 총 가스 비용: {} ETH", format_eth(stats.total_gas_cost));
        info!("💵 순이익: {} ETH", format_eth(stats.net_profit));
        info!("📊 평균 수익/샌드위치: {} ETH", format_eth(stats.avg_profit_per_sandwich));
        info!("📈 성공률: {:.2}%", stats.success_rate * 100.0);
        info!("⏱️ 가동 시간: {}초", stats.uptime_seconds());
        info!("=====================================");
    }

    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = SandwichStats::default();
        info!("🔄 샌드위치 통계 초기화 완료");
    }
}

impl Default for SandwichStatsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// ETH 금액 포맷팅
fn format_eth(wei: U256) -> String {
    let eth = wei.as_u128() as f64 / 1e18;
    format!("{:.6}", eth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stats_manager() {
        let manager = SandwichStatsManager::new();

        manager.record_opportunity_detected().await;
        manager.record_opportunity_analyzed().await;
        manager.record_bundle_submitted().await;
        manager.record_successful_sandwich(
            U256::from(1_000_000_000_000_000_000u64), // 1 ETH
            U256::from(100_000_000_000_000_000u64)    // 0.1 ETH
        ).await;

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_opportunities_detected, 1);
        assert_eq!(stats.total_opportunities_analyzed, 1);
        assert_eq!(stats.total_bundles_submitted, 1);
        assert_eq!(stats.total_successful_sandwiches, 1);
        assert_eq!(stats.success_rate, 1.0);
    }
}
