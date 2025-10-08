//! Aave V3 FlashLoan 실제 구현
//!
//! Aave V3 Pool 컨트랙트와 통신하여 실제 플래시론을 실행합니다.

use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::prelude::*;
use ethers::types::{Address, U256, Bytes, H256};
use tracing::{info, debug, warn, error};

use super::types::MicroArbitrageOpportunity;

// Aave V3 Pool ABI 일부
abigen!(
    AaveV3Pool,
    r#"[
        function flashLoan(address receiverAddress, address[] calldata assets, uint256[] calldata amounts, uint256[] calldata interestRateModes, address onBehalfOf, bytes calldata params, uint16 referralCode) external
        function getReserveData(address asset) external view returns (uint256 configuration, uint128 liquidityIndex, uint128 currentLiquidityRate, uint128 variableBorrowIndex, uint128 currentVariableBorrowRate, uint128 currentStableBorrowRate, uint40 lastUpdateTimestamp, uint16 id, address aTokenAddress, address stableDebtTokenAddress, address variableDebtTokenAddress, address interestRateStrategyAddress, uint128 accruedToTreasury, uint128 unbacked, uint128 isolationModeTotalDebt)
        function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128)
    ]"#
);

// FlashLoan Receiver ABI
abigen!(
    FlashLoanReceiver,
    r#"[
        function executeOperation(address[] calldata assets, uint256[] calldata amounts, uint256[] calldata premiums, address initiator, bytes calldata params) external returns (bool)
    ]"#
);

/// Aave V3 FlashLoan 실행자
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
        // 환경변수에서 Aave Pool 주소 로드
        let pool_address = std::env::var("AAVE_V3_POOL_ADDRESS")
            .unwrap_or_else(|_| "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2".to_string())
            .parse::<Address>()?;

        // 환경변수에서 FlashLoan Receiver 컨트랙트 주소 로드
        let receiver_address = std::env::var("FLASHLOAN_RECEIVER_ADDRESS")
            .map_err(|_| anyhow!("FLASHLOAN_RECEIVER_ADDRESS not set in environment"))?
            .parse::<Address>()?;

        Ok(Self {
            provider,
            wallet,
            pool_address,
            receiver_address,
        })
    }

    /// FlashLoan 실행
    pub async fn execute_flashloan(
        &self,
        opportunity: &MicroArbitrageOpportunity,
    ) -> Result<H256> {
        info!("⚡ Aave V3 FlashLoan 실행 시작");

        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.wallet.clone(),
        ));

        let pool = AaveV3Pool::new(self.pool_address, client.clone());

        // 차입할 자산 주소
        let asset_address = self.get_asset_address(&opportunity.base_asset)?;

        // FlashLoan 파라미터
        let assets = vec![asset_address];
        let amounts = vec![opportunity.buy_amount];
        let interest_rate_modes = vec![U256::zero()]; // 0 = no debt
        let on_behalf_of = self.wallet.address();
        let referral_code: u16 = 0;

        // 아비트리지 파라미터 인코딩
        let params = self.encode_arbitrage_params(opportunity)?;

        info!("   자산: {:?}", asset_address);
        info!("   수량: {} wei", opportunity.buy_amount);
        info!("   Receiver: {:?}", self.receiver_address);

        // FlashLoan 실행
        let tx = pool
            .flash_loan(
                self.receiver_address,
                assets,
                amounts,
                interest_rate_modes,
                on_behalf_of,
                params,
                referral_code,
            )
            .send()
            .await?;

        let tx_hash = tx.tx_hash();
        info!("✅ FlashLoan 트랜잭션 전송: {:?}", tx_hash);

        // 트랜잭션 확인 대기
        match tx.await? {
            Some(receipt) => {
                if receipt.status == Some(1.into()) {
                    info!("✅ FlashLoan 성공 - Block: {:?}", receipt.block_number);
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

    /// 자산 주소 조회
    fn get_asset_address(&self, asset_symbol: &str) -> Result<Address> {
        // 환경변수 또는 하드코딩된 주소 매핑
        let address = match asset_symbol {
            "ETH" | "WETH" => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
            "USDC" => "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            "USDT" => "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            "DAI" => "0x6B175474E89094C44Da98b954EedeAC495271d0F",
            "WBTC" => "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
            _ => {
                // 환경변수에서 찾기
                let var_name = format!("{}_TOKEN_ADDRESS", asset_symbol.to_uppercase());
                return std::env::var(&var_name)
                    .map_err(|_| anyhow!("Unknown asset: {}", asset_symbol))?
                    .parse::<Address>()
                    .map_err(|e| anyhow!("Invalid address for {}: {}", asset_symbol, e));
            }
        };

        address.parse::<Address>()
            .map_err(|e| anyhow!("Failed to parse address: {}", e))
    }

    /// 아비트리지 파라미터 인코딩
    fn encode_arbitrage_params(&self, opportunity: &MicroArbitrageOpportunity) -> Result<Bytes> {
        // 아비트리지 실행에 필요한 파라미터를 ABI 인코딩
        use ethers::abi::{encode, Token};

        let tokens = vec![
            Token::String(opportunity.token_symbol.clone()),
            Token::Address(self.get_asset_address(&opportunity.base_asset)?),
            Token::Uint(opportunity.buy_amount),
            Token::String(opportunity.buy_exchange.clone()),
            Token::String(opportunity.sell_exchange.clone()),
            Token::Uint(U256::from((opportunity.buy_price.to_f64().unwrap_or(0.0) * 1e18) as u64)),
            Token::Uint(U256::from((opportunity.sell_price.to_f64().unwrap_or(0.0) * 1e18) as u64)),
        ];

        Ok(Bytes::from(encode(&tokens)))
    }

    /// FlashLoan 수수료 조회
    pub async fn get_flashloan_premium(&self) -> Result<U256> {
        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.wallet.clone(),
        ));

        let pool = AaveV3Pool::new(self.pool_address, client);

        let premium = pool.flashloan_premium_total().call().await?;
        Ok(U256::from(premium))
    }

    /// Reserve 데이터 조회 (유동성 확인)
    pub async fn get_reserve_data(&self, asset: &str) -> Result<ReserveData> {
        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.wallet.clone(),
        ));

        let pool = AaveV3Pool::new(self.pool_address, client);
        let asset_address = self.get_asset_address(asset)?;

        let (
            configuration,
            liquidity_index,
            current_liquidity_rate,
            variable_borrow_index,
            current_variable_borrow_rate,
            current_stable_borrow_rate,
            last_update_timestamp,
            id,
            a_token_address,
            stable_debt_token_address,
            variable_debt_token_address,
            interest_rate_strategy_address,
            accrued_to_treasury,
            unbacked,
            isolation_mode_total_debt,
        ) = pool.get_reserve_data(asset_address).call().await?;

        Ok(ReserveData {
            configuration,
            liquidity_index,
            current_liquidity_rate,
            variable_borrow_index,
            current_variable_borrow_rate,
            current_stable_borrow_rate,
            last_update_timestamp,
            id,
            a_token_address,
            stable_debt_token_address,
            variable_debt_token_address,
            interest_rate_strategy_address,
            accrued_to_treasury,
            unbacked,
            isolation_mode_total_debt,
        })
    }

    /// 실제 사용 가능한 유동성 계산
    pub async fn get_available_liquidity(&self, asset: &str) -> Result<U256> {
        let client = Arc::new(SignerMiddleware::new(
            self.provider.clone(),
            self.wallet.clone(),
        ));

        let asset_address = self.get_asset_address(asset)?;

        // ERC20 토큰 인터페이스로 aToken의 잔고 조회
        abigen!(
            IERC20,
            r#"[
                function balanceOf(address account) external view returns (uint256)
            ]"#
        );

        let reserve_data = self.get_reserve_data(asset).await?;
        let a_token = IERC20::new(reserve_data.a_token_address, client);

        // Pool의 유동성 = aToken이 보유한 자산 잔고
        let liquidity = a_token.balance_of(asset_address).call().await?;

        info!("   {} 유동성: {} wei", asset, liquidity);
        Ok(liquidity)
    }
}

/// Reserve 데이터 구조체
#[derive(Debug, Clone)]
pub struct ReserveData {
    pub configuration: U256,
    pub liquidity_index: u128,
    pub current_liquidity_rate: u128,
    pub variable_borrow_index: u128,
    pub current_variable_borrow_rate: u128,
    pub current_stable_borrow_rate: u128,
    pub last_update_timestamp: u64,
    pub id: u16,
    pub a_token_address: Address,
    pub stable_debt_token_address: Address,
    pub variable_debt_token_address: Address,
    pub interest_rate_strategy_address: Address,
    pub accrued_to_treasury: u128,
    pub unbacked: u128,
    pub isolation_mode_total_debt: u128,
}
