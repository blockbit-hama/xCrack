use std::collections::HashMap;
use anyhow::{Result, anyhow};
use ethers::{
    abi::Token,
    types::{Transaction, H256, U256, Address, Bytes},
};
use tracing::debug;
use crate::blockchain::abi::AbiManager;

/// 트랜잭션 디코더
pub struct TransactionDecoder {
    abi_manager: AbiManager,
}

impl TransactionDecoder {
    /// 새로운 트랜잭션 디코더 생성
    pub fn new() -> Result<Self> {
        let mut abi_manager = AbiManager::new();
        abi_manager.load_default_abis()?;
        
        Ok(Self { abi_manager })
    }
    
    /// 트랜잭션 디코딩
    pub fn decode_transaction(&self, tx: &Transaction) -> Result<DecodedTransaction> {
        let mut decoded = DecodedTransaction {
            hash: tx.hash,
            from: tx.from,
            to: tx.to,
            value: tx.value,
            gas_price: tx.gas_price.unwrap_or_default(),
            gas_limit: tx.gas,
            function_name: None,
            parameters: HashMap::new(),
            is_dex_swap: false,
            is_liquidation: false,
            raw_data: tx.input.clone(),
        };
        
        // 입력 데이터가 있는 경우에만 디코딩
        if !tx.input.is_empty() && tx.input.len() >= 4 {
            // 함수 식별
            if let Some(function_name) = self.abi_manager.identify_function(&tx.input) {
                decoded.function_name = Some(function_name.clone());
                
                // 특정 함수 타입별 처리
                if function_name.contains("swap") {
                    decoded.is_dex_swap = true;
                    self.decode_swap_parameters(&mut decoded, &tx.input)?;
                } else if function_name.contains("liquidation") || function_name.contains("liquidate") {
                    decoded.is_liquidation = true;
                    self.decode_liquidation_parameters(&mut decoded, &tx.input)?;
                }
            }
        }
        
        Ok(decoded)
    }
    
    /// 스왑 파라미터 디코딩
    fn decode_swap_parameters(&self, decoded: &mut DecodedTransaction, data: &Bytes) -> Result<()> {
        let selector = &data[0..4];
        
        match selector {
            // swapExactTokensForTokens
            [0x38, 0xed, 0x17, 0x39] => {
                if data.len() >= 164 {
                    let amount_in = U256::from_big_endian(&data[4..36]);
                    let amount_out_min = U256::from_big_endian(&data[36..68]);
                    
                    decoded.parameters.insert("amountIn".to_string(), Token::Uint(amount_in));
                    decoded.parameters.insert("amountOutMin".to_string(), Token::Uint(amount_out_min));
                    
                    // path 파싱 (복잡한 동적 배열)
                    if let Ok(path) = self.decode_address_array(&data[68..]) {
                        decoded.parameters.insert("path".to_string(), Token::Array(
                            path.into_iter().map(Token::Address).collect()
                        ));
                    }
                }
            }
            // swapExactETHForTokens
            [0x7f, 0xf3, 0x6a, 0xb5] => {
                if data.len() >= 100 {
                    let amount_out_min = U256::from_big_endian(&data[4..36]);
                    decoded.parameters.insert("amountOutMin".to_string(), Token::Uint(amount_out_min));
                    
                    if let Ok(path) = self.decode_address_array(&data[36..]) {
                        decoded.parameters.insert("path".to_string(), Token::Array(
                            path.into_iter().map(Token::Address).collect()
                        ));
                    }
                }
            }
            // swapExactTokensForETH
            [0x18, 0xcb, 0xaf, 0x05] => {
                if data.len() >= 164 {
                    let amount_in = U256::from_big_endian(&data[4..36]);
                    let amount_out_min = U256::from_big_endian(&data[36..68]);
                    
                    decoded.parameters.insert("amountIn".to_string(), Token::Uint(amount_in));
                    decoded.parameters.insert("amountOutMin".to_string(), Token::Uint(amount_out_min));
                }
            }
            _ => {
                debug!("알 수 없는 스왑 함수: {:02x?}", selector);
            }
        }
        
        Ok(())
    }
    
    /// 청산 파라미터 디코딩
    fn decode_liquidation_parameters(&self, decoded: &mut DecodedTransaction, data: &Bytes) -> Result<()> {
        let selector = &data[0..4];
        
        match selector {
            // Aave liquidationCall
            [0xe8, 0xed, 0xa9, 0xdf] => {
                if data.len() >= 164 {
                    let collateral_asset = Address::from_slice(&data[16..36]);
                    let debt_asset = Address::from_slice(&data[48..68]);
                    let user = Address::from_slice(&data[80..100]);
                    let debt_to_cover = U256::from_big_endian(&data[100..132]);
                    let receive_a_token = data[163] != 0;
                    
                    decoded.parameters.insert("collateralAsset".to_string(), Token::Address(collateral_asset));
                    decoded.parameters.insert("debtAsset".to_string(), Token::Address(debt_asset));
                    decoded.parameters.insert("user".to_string(), Token::Address(user));
                    decoded.parameters.insert("debtToCover".to_string(), Token::Uint(debt_to_cover));
                    decoded.parameters.insert("receiveAToken".to_string(), Token::Bool(receive_a_token));
                }
            }
            // Compound liquidate
            [0x4c, 0x0b, 0x5b, 0x3e] => {
                if data.len() >= 100 {
                    let borrower = Address::from_slice(&data[16..36]);
                    let c_token_collateral = Address::from_slice(&data[48..68]);
                    let repay_amount = U256::from_big_endian(&data[68..100]);
                    
                    decoded.parameters.insert("borrower".to_string(), Token::Address(borrower));
                    decoded.parameters.insert("cTokenCollateral".to_string(), Token::Address(c_token_collateral));
                    decoded.parameters.insert("repayAmount".to_string(), Token::Uint(repay_amount));
                }
            }
            _ => {
                debug!("알 수 없는 청산 함수: {:02x?}", selector);
            }
        }
        
        Ok(())
    }
    
    /// 주소 배열 디코딩 (동적 배열)
    fn decode_address_array(&self, data: &[u8]) -> Result<Vec<Address>> {
        if data.len() < 32 {
            return Err(anyhow!("데이터가 너무 짧습니다"));
        }
        
        // 첫 32바이트는 배열 오프셋
        let offset = U256::from_big_endian(&data[0..32]).as_usize();
        
        if data.len() < offset + 32 {
            return Err(anyhow!("오프셋이 잘못되었습니다"));
        }
        
        // 다음 32바이트는 배열 길이
        let length = U256::from_big_endian(&data[offset..offset + 32]).as_usize();
        
        if data.len() < offset + 32 + (length * 32) {
            return Err(anyhow!("배열 데이터가 충분하지 않습니다"));
        }
        
        let mut addresses = Vec::new();
        for i in 0..length {
            let start = offset + 32 + (i * 32) + 12; // 주소는 32바이트 중 마지막 20바이트
            let end = start + 20;
            
            if end <= data.len() {
                let address = Address::from_slice(&data[start..end]);
                addresses.push(address);
            }
        }
        
        Ok(addresses)
    }
    
    /// MEV 트랜잭션 식별
    pub fn identify_mev_type(&self, decoded: &DecodedTransaction) -> MevType {
        if decoded.is_dex_swap {
            // 스왅 트랜잭션에서 가스 가격이 높으면 샌드위치 공격 가능성
            if decoded.gas_price > U256::from(100_000_000_000u64) { // 100 gwei 이상
                return MevType::PotentialSandwich;
            }
            return MevType::DexSwap;
        }
        
        if decoded.is_liquidation {
            return MevType::Liquidation;
        }
        
        // 아비트래지 패턴 감지
        if let Some(function_name) = &decoded.function_name {
            if function_name.contains("multicall") || function_name.contains("batch") {
                return MevType::PotentialArbitrage;
            }
        }
        
        MevType::Regular
    }
    
    /// 트랜잭션 가치 평가
    pub fn estimate_transaction_value(&self, decoded: &DecodedTransaction) -> U256 {
        let mut value = decoded.value;
        
        // 스왑 트랜잭션의 경우 스왑 금액 추가
        if decoded.is_dex_swap {
            if let Some(Token::Uint(amount)) = decoded.parameters.get("amountIn") {
                value += *amount;
            }
        }
        
        // 청산 트랜잭션의 경우 청산 금액 추가
        if decoded.is_liquidation {
            if let Some(Token::Uint(amount)) = decoded.parameters.get("debtToCover") {
                value += *amount;
            }
        }
        
        value
    }
}

/// 디코딩된 트랜잭션
#[derive(Debug, Clone)]
pub struct DecodedTransaction {
    /// 트랜잭션 해시
    pub hash: H256,
    /// 발신자
    pub from: Address,
    /// 수신자
    pub to: Option<Address>,
    /// 전송 금액
    pub value: U256,
    /// 가스 가격
    pub gas_price: U256,
    /// 가스 한도
    pub gas_limit: U256,
    /// 함수 이름
    pub function_name: Option<String>,
    /// 파라미터들
    pub parameters: HashMap<String, Token>,
    /// DEX 스왑 여부
    pub is_dex_swap: bool,
    /// 청산 여부
    pub is_liquidation: bool,
    /// 원본 데이터
    pub raw_data: Bytes,
}

/// MEV 타입
#[derive(Debug, Clone, PartialEq)]
pub enum MevType {
    /// 일반 트랜잭션
    Regular,
    /// DEX 스왑
    DexSwap,
    /// 청산
    Liquidation,
    /// 잠재적 샌드위치 공격
    PotentialSandwich,
    /// 잠재적 아비트래지
    PotentialArbitrage,
    /// 플래시론
    FlashLoan,
}

impl DecodedTransaction {
    /// 트랜잭션이 MEV 기회인지 확인
    pub fn is_mev_opportunity(&self) -> bool {
        self.is_dex_swap || self.is_liquidation || 
        self.gas_price > U256::from(50_000_000_000u64) // 50 gwei 이상
    }
    
    /// 샌드위치 공격 대상인지 확인
    pub fn is_sandwich_target(&self) -> bool {
        if !self.is_dex_swap {
            return false;
        }
        
        // 큰 스왑 금액 (1 ETH 이상)
        if let Some(Token::Uint(amount)) = self.parameters.get("amountIn") {
            if *amount > U256::from_str_radix("1000000000000000000", 10).unwrap() {
                return true;
            }
        }
        
        // ETH 값이 큰 경우
        if self.value > U256::from_str_radix("1000000000000000000", 10).unwrap() {
            return true;
        }
        
        false
    }
    
    /// 청산 기회인지 확인
    pub fn is_liquidation_opportunity(&self) -> bool {
        self.is_liquidation && self.gas_price < U256::from(100_000_000_000u64) // 100 gwei 미만
    }
}