//! Aave V3 다중자산 FlashLoan 실행자
//!
//! Aave V3의 다중자산 flashLoan 기능을 사용하여
//! 여러 토큰을 동시에 빌려서 복잡한 아비트리지를 수행합니다.

use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::prelude::*;
use ethers::types::{Address, U256, Bytes, H256};
use tracing::{info, debug, warn, error};

use super::types::*;

// Aave V3 Pool ABI (다중자산 플래시론)
abigen!(
    AaveV3PoolMulti,
    r#"[
        function flashLoan(address receiverAddress, address[] calldata assets, uint256[] calldata amounts, uint256[] calldata interestRateModes, address onBehalfOf, bytes calldata params, uint16 referralCode) external
        function getReserveData(address asset) external view returns (uint256 configuration, uint128 liquidityIndex, uint128 currentLiquidityRate, uint128 variableBorrowIndex, uint128 currentVariableBorrowRate, uint128 currentStableBorrowRate, uint40 lastUpdateTimestamp, uint16 id, address aTokenAddress, address stableDebtTokenAddress, address variableDebtTokenAddress, address interestRateStrategyAddress, uint128 accruedToTreasury, uint128 unbacked, uint128 isolationModeTotalDebt)
        function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128)
    ]"#
);

/// Aave V3 다중자산 FlashLoan 실행자
pub struct AaveFlashLoanExecutor {
    provider: Arc<Provider<Ws>>,
    wallet: LocalWallet,
    pool_address: Address,
    receiver_address: Address,
}

impl AaveFlashLoanExecutor {
    /// 새로운 FlashLoan 실행자 생성
    pub fn new(
        provider: Arc<Provider<Ws>>,
        wallet: LocalWallet,
    ) -> Result<Self> {
        // Aave V3 Pool 주소 (Ethereum Mainnet)
        let pool_address = std::env::var("AAVE_V3_POOL_ADDRESS")
            .unwrap_or_else(|_| "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".to_string())
            .parse::<Address>()?;

        // FlashLoan Receiver 컨트랙트 주소
        let receiver_address = std::env::var("MULTI_ASSET_FLASHLOAN_RECEIVER")
            .map_err(|_| anyhow!("MULTI_ASSET_FLASHLOAN_RECEIVER not set in environment"))?
            .parse::<Address>()?;

        info!("✅ Aave V3 다중자산 FlashLoan 실행자 초기화");
        info!("   Pool: {:?}", pool_address);
        info!("   Receiver: {:?}", receiver_address);

        Ok(Self {
            provider,
            wallet,
            pool_address,
            receiver_address,
        })
    }

    /// 다중자산 FlashLoan 실행
    pub async fn execute_multi_asset_flashloan(
        &self,
        opportunity: &MultiAssetArbitrageOpportunity,
    ) -> Result<H256> {
        info!("⚡ Aave V3 다중자산 FlashLoan 실행 시작");
        info!("   기회 ID: {}", opportunity.id);

        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.wallet.clone(),
        ));

        let pool = AaveV3PoolMulti::new(self.pool_address, client.clone());

        // FlashLoan 파라미터 준비
        let assets: Vec<Address> = opportunity.flashloan_amounts
            .iter()
            .map(|fl| fl.asset)
            .collect();

        let amounts: Vec<U256> = opportunity.flashloan_amounts
            .iter()
            .map(|fl| fl.amount)
            .collect();

        let interest_rate_modes: Vec<U256> = vec![U256::zero(); assets.len()]; // 0 = no debt

        // 아비트리지 파라미터 인코딩
        let params = self.encode_arbitrage_params(opportunity)?;

        info!("   자산 개수: {}", assets.len());
        for (i, (asset, amount)) in assets.iter().zip(amounts.iter()).enumerate() {
            info!("   자산 {}: {:?} / {} wei", i + 1, asset, amount);
        }

        // FlashLoan 실행
        let tx = pool
            .flash_loan(
                self.receiver_address,
                assets,
                amounts,
                interest_rate_modes,
                self.wallet.address(),
                params,
                0, // referral code
            )
            .send()
            .await?;

        let tx_hash = tx.tx_hash();
        info!("✅ FlashLoan 트랜잭션 전송: {:?}", tx_hash);

        // 트랜잭션 확인 대기
        match tx.await? {
            Some(receipt) => {
                if receipt.status == Some(1.into()) {
                    info!("✅ 다중자산 FlashLoan 성공 - Block: {:?}", receipt.block_number);
                    Ok(tx_hash)
                } else {
                    error!("❌ FlashLoan 실패 - 트랜잭션 revert");
                    Err(anyhow!("FlashLoan transaction reverted"))
                }
            }
            None => {
                error!("❌ FlashLoan 실패 - 영수증 없음");
                Err(anyhow!("No transaction receipt"))
            }
        }
    }

    /// 아비트리지 파라미터 인코딩
    fn encode_arbitrage_params(&self, opportunity: &MultiAssetArbitrageOpportunity) -> Result<Bytes> {
        use ethers::abi::{encode, Token};

        // 전략 타입에 따라 다른 인코딩
        let tokens = match &opportunity.strategy_type {
            MultiAssetStrategyType::TriangularArbitrage { token_a, token_b, token_c, amount_a, amount_b } => {
                vec![
                    Token::Uint(U256::from(1)), // 전략 타입: 삼각 아비트리지
                    Token::Address(*token_a),
                    Token::Address(*token_b),
                    Token::Address(*token_c),
                    Token::Uint(*amount_a),
                    Token::Uint(*amount_b),
                ]
            }
            MultiAssetStrategyType::PositionMigration { from_protocol, to_protocol, assets, amounts } => {
                vec![
                    Token::Uint(U256::from(2)), // 전략 타입: 포지션 마이그레이션
                    Token::String(from_protocol.clone()),
                    Token::String(to_protocol.clone()),
                    Token::Array(assets.iter().map(|a| Token::Address(*a)).collect()),
                    Token::Array(amounts.iter().map(|a| Token::Uint(*a)).collect()),
                ]
            }
            MultiAssetStrategyType::ComplexArbitrage { route, total_hops } => {
                vec![
                    Token::Uint(U256::from(3)), // 전략 타입: 복합 아비트리지
                    Token::Uint(U256::from(*total_hops)),
                    // 경로 정보 인코딩 (간단화)
                    Token::Array(vec![]),
                ]
            }
        };

        Ok(Bytes::from(encode(&tokens)))
    }

    /// FlashLoan 프리미엄 조회
    pub async fn get_flashloan_premium(&self) -> Result<U256> {
        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.wallet.clone(),
        ));

        let pool = AaveV3PoolMulti::new(self.pool_address, client);
        let premium = pool.flashloan_premium_total().call().await?;

        Ok(U256::from(premium))
    }

    /// 특정 자산의 유동성 확인
    pub async fn check_asset_liquidity(&self, asset: Address) -> Result<U256> {
        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.wallet.clone(),
        ));

        let pool = AaveV3PoolMulti::new(self.pool_address, client.clone());

        // Reserve 데이터 조회
        let reserve_data = pool.get_reserve_data(asset).call().await?;

        // aToken 주소 추출 (튜플의 8번째 요소)
        let a_token_address = reserve_data.8;

        // ERC20 인터페이스로 잔고 조회
        abigen!(
            IERC20,
            r#"[
                function balanceOf(address account) external view returns (uint256)
            ]"#
        );

        let a_token = IERC20::new(a_token_address, client);
        let liquidity = a_token.balance_of(asset).call().await?;

        debug!("   자산 {:?} 유동성: {} wei", asset, liquidity);
        Ok(liquidity)
    }

    /// 여러 자산의 유동성 동시 확인
    pub async fn check_multi_asset_liquidity(&self, assets: &[Address]) -> Result<Vec<U256>> {
        let mut liquidities = Vec::new();

        for asset in assets {
            let liquidity = self.check_asset_liquidity(*asset).await?;
            liquidities.push(liquidity);
        }

        Ok(liquidities)
    }

    /// 모든 자산이 충분한 유동성을 가지고 있는지 확인
    pub async fn validate_liquidity(
        &self,
        opportunity: &MultiAssetArbitrageOpportunity,
    ) -> Result<bool> {
        for fl in &opportunity.flashloan_amounts {
            let liquidity = self.check_asset_liquidity(fl.asset).await?;

            // 필요 금액이 사용 가능한 유동성의 90% 이하인지 확인
            let max_safe_amount = liquidity * U256::from(90) / U256::from(100);

            if fl.amount > max_safe_amount {
                warn!("⚠️ 자산 {:?}의 유동성 부족", fl.asset);
                warn!("   필요: {} wei", fl.amount);
                warn!("   사용 가능: {} wei (90% = {} wei)", liquidity, max_safe_amount);
                return Ok(false);
            }
        }

        Ok(true)
    }
}
