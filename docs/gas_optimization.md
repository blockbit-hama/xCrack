# 가스 최적화 모듈 (Gas Optimization Module)

## 개요

이 모듈은 MEV 샌드위치 전략에서 가스 가격을 최적화하는 고급 기능을 제공합니다. 기존의 고정된 150k 가스 추정 대신 실제 시뮬레이션을 통해 정확한 가스 사용량을 계산하고, EIP-1559 지원과 언더플로우 보호 기능을 포함합니다.

## 주요 개선사항

### 1. 사전 시뮬레이션 기반 가스 추정
- **기존**: 고정된 150k 가스 사용
- **개선**: `eth_estimateGas`와 Flashbots 시뮬레이션을 통한 정확한 가스 추정
- **장점**: 실제 가스 사용량에 근거한 정확한 비용 계산

### 2. EIP-1559 지원
- **Base Fee**: 네트워크 상태에 따른 동적 base fee 조회
- **Priority Fee**: 최적의 priority fee 계산
- **Max Fee**: base fee + priority fee + 여유분으로 계산
- **장점**: EIP-1559 네트워크에서 더 효율적인 가스 가격 설정

### 3. 언더플로우 보호
- **기존**: `victim_gas_price - 1 gwei` (언더플로우 위험)
- **개선**: `checked_sub()` 사용으로 안전한 감산
- **대안값**: 언더플로우 시 절반값 사용
- **장점**: 가스 가격이 매우 낮을 때도 안전한 처리

### 4. MEV 번들 순서 고정
- **기존**: 가스 가격으로만 순서 결정
- **개선**: 번들 내 트랜잭션 순서 고정
- **장점**: 더 예측 가능하고 안전한 실행

## 사용법

### 기본 사용법

```rust
use xcrack::strategies::gas_optimization::{GasOptimizer, SandwichOpportunity};

// 1. 가스 최적화기 생성
let gas_optimizer = GasOptimizer::new(
    provider,
    bundle_simulator,
    flashbots_client,
    max_gas_price,
    eip1559_enabled,
);

// 2. 샌드위치 기회 생성
let sandwich_opportunity = SandwichOpportunity {
    target_tx: target_transaction,
    expected_profit: expected_profit,
    pool_address: pool_address,
    token_in: token_in,
    token_out: token_out,
    amount_in: amount_in,
};

// 3. 가스 가격 최적화
let gas_strategy = gas_optimizer.optimize_gas_prices(&sandwich_opportunity).await?;
```

### EIP-1559 사용법

```rust
// EIP-1559 활성화
let eip1559_enabled = true;

// 가스 전략에서 EIP-1559 정보 확인
if gas_strategy.eip1559_enabled {
    println!("Max Fee per Gas: {} gwei", 
        gas_strategy.max_fee_per_gas.unwrap() / U256::from(1_000_000_000u64));
    println!("Max Priority Fee: {} gwei", 
        gas_strategy.max_priority_fee_per_gas.unwrap() / U256::from(1_000_000_000u64));
}
```

## 구조체 설명

### GasStrategy
```rust
pub struct GasStrategy {
    pub frontrun_gas_price: U256,           // 프론트런 가스 가격
    pub backrun_gas_price: U256,            // 백런 가스 가격
    pub total_gas_cost: U256,               // 총 가스 비용
    pub frontrun_gas_limit: u64,            // 프론트런 가스 한도
    pub backrun_gas_limit: u64,             // 백런 가스 한도
    pub eip1559_enabled: bool,              // EIP-1559 활성화 여부
    pub max_fee_per_gas: Option<U256>,      // EIP-1559 max fee
    pub max_priority_fee_per_gas: Option<U256>, // EIP-1559 priority fee
    pub bundle_order_fixed: bool,           // 번들 순서 고정 여부
}
```

### SandwichOpportunity
```rust
pub struct SandwichOpportunity {
    pub target_tx: TargetTransaction,       // 대상 트랜잭션
    pub expected_profit: U256,              // 예상 수익
    pub pool_address: Address,              // 풀 주소
    pub token_in: Address,                  // 입력 토큰
    pub token_out: Address,                 // 출력 토큰
    pub amount_in: U256,                    // 입력 금액
}
```

## 시뮬레이션 프로세스

### 1. 개별 트랜잭션 가스 추정
```rust
// eth_estimateGas를 사용한 개별 추정
let frontrun_gas = self.estimate_frontrun_gas(sandwich_opp).await?;
let backrun_gas = self.estimate_backrun_gas(sandwich_opp).await?;
```

### 2. Flashbots 시뮬레이션
```rust
// 더 정확한 번들 시뮬레이션
let flashbots_result = self.simulate_with_flashbots(
    sandwich_opp, 
    frontrun_gas, 
    backrun_gas
).await?;
```

### 3. 가스 가격 계산
```rust
// EIP-1559 또는 Legacy 방식으로 계산
let (frontrun_gas_price, backrun_gas_price) = if let Some(eip1559) = &eip1559_info {
    self.calculate_eip1559_gas_prices(victim_gas_price, eip1559)?
} else {
    self.calculate_legacy_gas_prices(victim_gas_price)?
};
```

## 언더플로우 보호 예시

### Legacy 가스 가격
```rust
// 안전한 감산 (언더플로우 보호)
let backrun_gas_price = victim_gas_price.checked_sub(U256::from(1_000_000_000u64))
    .unwrap_or(victim_gas_price / U256::from(2)); // 대안값: 절반
```

### EIP-1559 가스 가격
```rust
// Back-run: 피해자보다 낮은 maxFeePerGas 설정 (언더플로우 체크)
let backrun_max_fee = victim_gas_price.checked_sub(U256::from(1_000_000_000u64))
    .unwrap_or(victim_gas_price / U256::from(2)); // 언더플로우 시 절반으로
```

## 테스트

### 언더플로우 보호 테스트
```rust
#[test]
fn test_underflow_protection() {
    let victim_gas = U256::from(1_000_000_000u64); // 1 gwei
    let result = victim_gas.checked_sub(U256::from(2_000_000_000u64)); // 2 gwei 빼기
    
    assert!(result.is_none()); // 언더플로우 발생
}
```

### EIP-1559 계산 테스트
```rust
#[test]
fn test_eip1559_calculation() {
    let base_fee = U256::from(15_000_000_000u64);
    let priority_fee = U256::from(2_000_000_000u64);
    let max_fee = base_fee + priority_fee;
    
    // Front-run 계산
    let frontrun_max_fee = max_fee + U256::from(2_000_000_000u64);
    assert_eq!(frontrun_max_fee, U256::from(19_000_000_000u64));
    
    // Back-run 계산 (언더플로우 보호)
    let backrun_max_fee = max_fee.checked_sub(U256::from(1_000_000_000u64))
        .unwrap_or(max_fee / U256::from(2));
    assert_eq!(backrun_max_fee, U256::from(16_000_000_000u64));
}
```

## 실행 예시

```bash
# 예시 실행
cargo run --example gas_optimization_example

# 테스트 실행
cargo test gas_optimization
```

## 성능 최적화

### 1. 캐싱
- 시뮬레이션 결과 캐싱으로 반복 계산 방지
- Base fee 캐싱으로 네트워크 호출 최소화

### 2. 병렬 처리
- 프론트런/백런 가스 추정을 병렬로 실행
- 여러 시뮬레이션을 동시에 처리

### 3. 조기 종료
- 시뮬레이션 실패 시 즉시 종료
- 수익성 임계값 미달 시 조기 종료

## 보안 고려사항

### 1. 가스 가격 제한
- 최대 가스 가격 설정으로 과도한 비용 방지
- 동적 임계값으로 네트워크 상태 반영

### 2. 시뮬레이션 검증
- Flashbots 시뮬레이션으로 실행 가능성 확인
- 다중 시나리오 테스트로 안정성 확보

### 3. 오류 처리
- 모든 가스 계산에서 오류 처리
- 대안값 제공으로 안정성 확보

## 향후 개선 계획

1. **머신러닝 기반 가스 예측**: 과거 데이터를 활용한 더 정확한 가스 예측
2. **다중 네트워크 지원**: Polygon, BSC 등 다른 네트워크 지원
3. **실시간 가스 오라클**: 실시간 네트워크 상태 모니터링
4. **가스 최적화 전략**: 더 정교한 가스 최적화 알고리즘

## 참고 자료

- [EIP-1559: Fee market change for ETH 1.0 chain](https://eips.ethereum.org/EIPS/eip-1559)
- [Flashbots: MEV-Boost](https://docs.flashbots.net/flashbots-mev-boost/overview)
- [Ethereum Gas Optimization](https://ethereum.org/en/developers/docs/gas/)
