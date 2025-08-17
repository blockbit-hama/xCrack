use std::sync::Arc;
use anyhow::{Result, anyhow};
use ethers::{
    abi::{Abi, Token},
    contract::{Contract, ContractFactory as EthersContractFactory},
    providers::{Provider, Http, Middleware},
    types::{Address, U256, H256, Bytes, TransactionRequest},
    signers::{LocalWallet, Signer},
    middleware::SignerMiddleware,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, warn};

/// 스마트 컨트랙트 인터페이스
pub trait ContractInterface: Send + Sync {
    /// 컨트랙트 주소 반환
    fn address(&self) -> Address;
    
    /// ABI 반환
    fn abi(&self) -> &Abi;
    
    /// 읽기 전용 함수 호출
    fn call(&self, function: &str, args: Vec<Token>) -> Box<dyn std::future::Future<Output = Result<Vec<Token>>> + Send + '_>;
    
    /// 트랜잭션 전송
    fn send(&self, function: &str, args: Vec<Token>) -> Box<dyn std::future::Future<Output = Result<H256>> + Send + '_>;
}

/// DEX 라우터 컨트랙트 인터페이스 (Uniswap V2 호환)
pub struct DexRouterContract {
    contract: Contract<Provider<Http>>,
    address: Address,
    abi: Abi,
}

impl DexRouterContract {
    pub fn new(address: Address, provider: Arc<Provider<Http>>) -> Result<Self> {
        let abi_json = include_str!("../../abi/uniswap_v2_router.json");
        let abi: Abi = serde_json::from_str(abi_json)?;
        
        let contract = Contract::new(address, abi.clone(), provider);
        
        Ok(Self {
            contract,
            address,
            abi,
        })
    }
    
    /// 토큰 스왑 정보 조회
    pub async fn get_amounts_out(&self, amount_in: U256, path: Vec<Address>) -> Result<Vec<U256>> {
        let result: Vec<U256> = self.contract
            .method("getAmountsOut", (amount_in, path))?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 토큰 스왑 정보 조회 (역방향)
    pub async fn get_amounts_in(&self, amount_out: U256, path: Vec<Address>) -> Result<Vec<U256>> {
        let result: Vec<U256> = self.contract
            .method("getAmountsIn", (amount_out, path))?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 팩토리 주소 조회
    pub async fn factory(&self) -> Result<Address> {
        let result: Address = self.contract
            .method("factory", ())?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// WETH 주소 조회
    pub async fn weth(&self) -> Result<Address> {
        let result: Address = self.contract
            .method("WETH", ())?
            .call()
            .await?;
        
        Ok(result)
    }
}

/// 대출 프로토콜 컨트랙트 인터페이스 (Aave V2)
pub struct LendingPoolContract {
    contract: Contract<Provider<Http>>,
    address: Address,
    abi: Abi,
}

impl LendingPoolContract {
    pub fn new(address: Address, provider: Arc<Provider<Http>>) -> Result<Self> {
        let abi_json = include_str!("../../abi/aave_lending_pool.json");
        let abi: Abi = serde_json::from_str(abi_json)?;
        
        let contract = Contract::new(address, abi.clone(), provider);
        
        Ok(Self {
            contract,
            address,
            abi,
        })
    }
    
    /// 사용자 계정 데이터 조회
    pub async fn get_user_account_data(&self, user: Address) -> Result<UserAccountData> {
        let result: (U256, U256, U256, U256, U256, U256) = self.contract
            .method("getUserAccountData", user)?
            .call()
            .await?;
        
        Ok(UserAccountData {
            total_collateral_eth: result.0,
            total_debt_eth: result.1,
            available_borrow_eth: result.2,
            current_liquidation_threshold: result.3,
            ltv: result.4,
            health_factor: result.5,
        })
    }
    
    /// 예약 데이터 조회
    pub async fn get_reserve_data(&self, asset: Address) -> Result<ReserveData> {
        let result: (U256, U256, U256, U256, U256, U256, U256, Address, Address, Address, Address, u8) = 
            self.contract
                .method("getReserveData", asset)?
                .call()
                .await?;
        
        Ok(ReserveData {
            liquidity_rate: result.1,
            variable_borrow_rate: result.2,
            stable_borrow_rate: result.3,
            liquidity_index: result.5,
            variable_borrow_index: result.6,
            a_token_address: result.7,
            stable_debt_token_address: result.8,
            variable_debt_token_address: result.9,
        })
    }
}

/// 사용자 계정 데이터
#[derive(Debug, Clone)]
pub struct UserAccountData {
    pub total_collateral_eth: U256,
    pub total_debt_eth: U256,
    pub available_borrow_eth: U256,
    pub current_liquidation_threshold: U256,
    pub ltv: U256,
    pub health_factor: U256,
}

/// 예약 데이터
#[derive(Debug, Clone)]
pub struct ReserveData {
    pub liquidity_rate: U256,
    pub variable_borrow_rate: U256,
    pub stable_borrow_rate: U256,
    pub liquidity_index: U256,
    pub variable_borrow_index: U256,
    pub a_token_address: Address,
    pub stable_debt_token_address: Address,
    pub variable_debt_token_address: Address,
}

/// AMM 풀 컨트랙트 인터페이스 (Uniswap V2 Pair)
pub struct AmmPoolContract {
    contract: Contract<Provider<Http>>,
    address: Address,
    abi: Abi,
}

impl AmmPoolContract {
    pub fn new(address: Address, provider: Arc<Provider<Http>>) -> Result<Self> {
        let abi_json = include_str!("../../abi/uniswap_v2_pair.json");
        let abi: Abi = serde_json::from_str(abi_json)?;
        
        let contract = Contract::new(address, abi.clone(), provider);
        
        Ok(Self {
            contract,
            address,
            abi,
        })
    }
    
    /// 리저브 조회
    pub async fn get_reserves(&self) -> Result<(U256, U256, u32)> {
        let result: (U256, U256, u32) = self.contract
            .method("getReserves", ())?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 토큰 0 주소
    pub async fn token0(&self) -> Result<Address> {
        let result: Address = self.contract
            .method("token0", ())?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 토큰 1 주소
    pub async fn token1(&self) -> Result<Address> {
        let result: Address = self.contract
            .method("token1", ())?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 가격 계산 (토큰0 대비 토큰1)
    pub async fn get_price(&self) -> Result<f64> {
        let (reserve0, reserve1, _) = self.get_reserves().await?;
        
        if reserve0 == U256::zero() {
            return Err(anyhow!("Reserve0이 0입니다"));
        }
        
        let price = reserve1.as_u128() as f64 / reserve0.as_u128() as f64;
        Ok(price)
    }
    
    /// 가격 영향 계산
    pub async fn calculate_price_impact(&self, amount_in: U256, is_token0: bool) -> Result<f64> {
        let (reserve0, reserve1, _) = self.get_reserves().await?;
        
        let (reserve_in, reserve_out) = if is_token0 {
            (reserve0, reserve1)
        } else {
            (reserve1, reserve0)
        };
        
        // x * y = k 공식 사용
        let amount_in_with_fee = amount_in * U256::from(997); // 0.3% 수수료
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in * U256::from(1000) + amount_in_with_fee;
        let amount_out = numerator / denominator;
        
        // 가격 영향 계산
        let price_before = reserve_out.as_u128() as f64 / reserve_in.as_u128() as f64;
        let new_reserve_in = reserve_in + amount_in;
        let new_reserve_out = reserve_out - amount_out;
        let price_after = new_reserve_out.as_u128() as f64 / new_reserve_in.as_u128() as f64;
        
        let price_impact = ((price_before - price_after) / price_before).abs() * 100.0;
        
        Ok(price_impact)
    }
}

/// ERC20 토큰 컨트랙트
pub struct ERC20Contract {
    contract: Contract<Provider<Http>>,
    address: Address,
    abi: Abi,
}

impl ERC20Contract {
    pub fn new(address: Address, provider: Arc<Provider<Http>>) -> Result<Self> {
        let abi_json = include_str!("../../abi/erc20.json");
        let abi: Abi = serde_json::from_str(abi_json)?;
        
        let contract = Contract::new(address, abi.clone(), provider);
        
        Ok(Self {
            contract,
            address,
            abi,
        })
    }
    
    /// 잔액 조회
    pub async fn balance_of(&self, account: Address) -> Result<U256> {
        let result: U256 = self.contract
            .method("balanceOf", account)?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 토탈 서플라이
    pub async fn total_supply(&self) -> Result<U256> {
        let result: U256 = self.contract
            .method("totalSupply", ())?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// 심볼
    pub async fn symbol(&self) -> Result<String> {
        let result: String = self.contract
            .method("symbol", ())?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// Decimals
    pub async fn decimals(&self) -> Result<u8> {
        let result: u8 = self.contract
            .method("decimals", ())?
            .call()
            .await?;
        
        Ok(result)
    }
    
    /// Allowance 조회
    pub async fn allowance(&self, owner: Address, spender: Address) -> Result<U256> {
        let result: U256 = self.contract
            .method("allowance", (owner, spender))?
            .call()
            .await?;
        
        Ok(result)
    }
}

/// 컨트랙트 팩토리
pub struct ContractFactory {
    provider: Arc<Provider<Http>>,
    contracts_cache: HashMap<Address, Arc<dyn ContractInterface>>,
}

impl ContractFactory {
    pub fn new(provider: Arc<Provider<Http>>) -> Self {
        Self {
            provider,
            contracts_cache: HashMap::new(),
        }
    }
    
    /// DEX 라우터 컨트랙트 생성
    pub fn create_dex_router(&self, address: Address) -> Result<Arc<DexRouterContract>> {
        Ok(Arc::new(DexRouterContract::new(address, self.provider.clone())?))
    }
    
    /// 대출 풀 컨트랙트 생성
    pub fn create_lending_pool(&self, address: Address) -> Result<Arc<LendingPoolContract>> {
        Ok(Arc::new(LendingPoolContract::new(address, self.provider.clone())?))
    }
    
    /// AMM 풀 컨트랙트 생성
    pub fn create_amm_pool(&self, address: Address) -> Result<Arc<AmmPoolContract>> {
        Ok(Arc::new(AmmPoolContract::new(address, self.provider.clone())?))
    }
    
    /// ERC20 토큰 컨트랙트 생성
    pub fn create_erc20(&self, address: Address) -> Result<Arc<ERC20Contract>> {
        Ok(Arc::new(ERC20Contract::new(address, self.provider.clone())?))
    }
}

/// 범용 컨트랙트 인터페이스 구현
impl ContractInterface for DexRouterContract {
    fn address(&self) -> Address {
        self.address
    }
    
    fn abi(&self) -> &Abi {
        &self.abi
    }
    
    fn call(&self, _function: &str, _args: Vec<Token>) -> Box<dyn std::future::Future<Output = Result<Vec<Token>>> + Send + '_> {
        Box::new(async move {
            // 구현 필요
            Ok(vec![])
        })
    }
    
    fn send(&self, _function: &str, _args: Vec<Token>) -> Box<dyn std::future::Future<Output = Result<H256>> + Send + '_> {
        Box::new(async move {
            // 구현 필요
            Ok(H256::zero())
        })
    }
}