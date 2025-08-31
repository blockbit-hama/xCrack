use std::collections::HashMap;
use anyhow::Result;
use ethers::abi::{Abi, Function, FunctionExt};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tracing::info;

/// ABI 매니저
pub struct AbiManager {
    /// 로드된 ABI들
    abis: HashMap<String, Abi>,
    /// 함수 시그니처 캐시
    function_signatures: HashMap<[u8; 4], (String, Function)>,
}

impl AbiManager {
    /// 새로운 ABI 매니저 생성
    pub fn new() -> Self {
        Self {
            abis: HashMap::new(),
            function_signatures: HashMap::new(),
        }
    }
    
    /// ABI 파일 로드
    pub fn load_from_file(&mut self, name: &str, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)?;
        let abi: Abi = serde_json::from_str(&content)?;
        
        // 함수 시그니처 캐싱
        for function in abi.functions() {
            let selector = function.selector();
            self.function_signatures.insert(
                selector,
                (name.to_string(), function.clone())
            );
        }
        
        self.abis.insert(name.to_string(), abi);
        info!("✅ ABI 로드: {}", name);
        
        Ok(())
    }
    
    /// 기본 ABI들 로드
    pub fn load_default_abis(&mut self) -> Result<()> {
        // ERC20
        self.load_from_file("ERC20", Path::new("abi/erc20.json"))?;
        
        // Uniswap V2
        self.load_from_file("UniswapV2Router", Path::new("abi/uniswap_v2_router.json"))?;
        self.load_from_file("UniswapV2Pair", Path::new("abi/uniswap_v2_pair.json"))?;
        
        // Aave
        self.load_from_file("AaveLendingPool", Path::new("abi/aave_lending_pool.json"))?;
        
        info!("✅ 기본 ABI 로드 완료");
        Ok(())
    }
    
    /// ABI 가져오기
    pub fn get_abi(&self, name: &str) -> Option<&Abi> {
        self.abis.get(name)
    }
    
    /// 함수 시그니처로 함수 찾기
    pub fn get_function_by_selector(&self, selector: &[u8; 4]) -> Option<&(String, Function)> {
        self.function_signatures.get(selector)
    }
    
    /// 알려진 함수 시그니처들
    pub fn known_signatures() -> HashMap<[u8; 4], &'static str> {
        let mut signatures = HashMap::new();
        
        // ERC20
        signatures.insert([0xa9, 0x05, 0x9c, 0xbb], "transfer");
        signatures.insert([0x23, 0xb8, 0x72, 0xdd], "transferFrom");
        signatures.insert([0x09, 0x5e, 0xa7, 0xb3], "approve");
        signatures.insert([0x70, 0xa0, 0x82, 0x31], "balanceOf");
        
        // Uniswap V2
        signatures.insert([0x38, 0xed, 0x17, 0x39], "swapExactTokensForTokens");
        signatures.insert([0x7f, 0xf3, 0x6a, 0xb5], "swapExactETHForTokens");
        signatures.insert([0x18, 0xcb, 0xaf, 0x05], "swapExactTokensForETH");
        signatures.insert([0x8b, 0x03, 0xc4, 0x96], "swapTokensForExactTokens");
        signatures.insert([0xfb, 0x3b, 0xdb, 0x41], "swapTokensForExactETH");
        signatures.insert([0x4a, 0x25, 0xd9, 0x4a], "swapETHForExactTokens");
        
        // Uniswap V3
        signatures.insert([0x41, 0x4b, 0xf3, 0x89], "exactInputSingle");
        signatures.insert([0xdb, 0x3e, 0x21, 0x98], "exactOutputSingle");
        signatures.insert([0xc0, 0x4b, 0x8d, 0x59], "exactInput");
        signatures.insert([0xf2, 0x8c, 0x61, 0x45], "exactOutput");
        
        // Aave
        signatures.insert([0xe8, 0xed, 0xa9, 0xdf], "liquidationCall");
        signatures.insert([0xe8, 0xac, 0xa4, 0x71], "deposit");
        signatures.insert([0x69, 0x32, 0x8d, 0xec], "withdraw");
        signatures.insert([0xa4, 0x15, 0xbc, 0xcc], "borrow");
        signatures.insert([0x57, 0x3a, 0xde, 0x61], "repay");
        
        // Compound
        signatures.insert([0x4c, 0x0b, 0x5b, 0x3e], "liquidate");
        signatures.insert([0xa0, 0x71, 0x2d, 0x68], "mint");
        signatures.insert([0xdb, 0x00, 0x6a, 0x75], "redeem");
        signatures.insert([0xc5, 0xeb, 0xbe, 0xc5], "borrow");
        signatures.insert([0x0e, 0x75, 0x27, 0x02], "repayBorrow");
        
        signatures
    }
    
    /// 함수 시그니처 식별
    pub fn identify_function(&self, data: &[u8]) -> Option<String> {
        if data.len() < 4 {
            return None;
        }
        
        let mut selector = [0u8; 4];
        selector.copy_from_slice(&data[0..4]);
        
        // 먼저 로드된 ABI에서 찾기
        if let Some((abi_name, function)) = self.function_signatures.get(&selector) {
            return Some(format!("{}.{}", abi_name, function.name));
        }
        
        // 알려진 시그니처에서 찾기
        if let Some(name) = Self::known_signatures().get(&selector) {
            return Some(name.to_string());
        }
        
        None
    }
}

/// 함수 시그니처 데이터베이스
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureDatabase {
    /// 함수 시그니처
    pub functions: HashMap<String, FunctionSignature>,
    /// 이벤트 시그니처
    pub events: HashMap<String, EventSignature>,
}

/// 함수 시그니처
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSignature {
    /// 함수 이름
    pub name: String,
    /// 4바이트 셀렉터
    pub selector: String,
    /// 입력 타입들
    pub inputs: Vec<String>,
    /// 출력 타입들
    pub outputs: Vec<String>,
    /// 상태 변경 여부
    pub state_mutability: String,
}

/// 이벤트 시그니처
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSignature {
    /// 이벤트 이름
    pub name: String,
    /// 토픽 해시
    pub topic: String,
    /// 입력 타입들
    pub inputs: Vec<EventInput>,
}

/// 이벤트 입력
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInput {
    /// 이름
    pub name: String,
    /// 타입
    pub type_: String,
    /// 인덱싱 여부
    pub indexed: bool,
}