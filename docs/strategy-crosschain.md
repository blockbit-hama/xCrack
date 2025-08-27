# Cross-Chain Arbitrage Strategy (No MEV, No Flashloan)

브리지 수수료/시간을 포함하여 체인 간 가격 차이를 탐지하고, 공개 트랜잭션으로 실행합니다. 원자성 제약으로 플래시론은 사용하지 않습니다.

## 정책 요약
- MEV 사용: No
- Flashloan 사용: No

## 데이터 소스
- Bridge Aggregator (Li.Fi) 및 개별 브리지 API (Stargate, Hop 등)
- 체인별 DEX 견적/가스비

## 처리 흐름 (요약)

```mermaid
flowchart LR
  A[Periodic Scan] --> B[Fetch Quotes (+timeouts)]
  B --> C[Profitability Check (fees,time)]
  C --> D{Above Threshold?}
  D -- No --> X[Skip]
  D -- Yes --> E[Execute Source TX]
  E --> F[Bridge]
  F --> G[Execute Dest TX]
```

## 실행 단계
1) 주기 스캔(30s): 토큰/체인 조합에 대해 지원 라우트와 견적 수집
2) 수익성/시간 제약: 브리지 수수료/도착 가스 등을 포함하여 임계값 비교
3) 실행: 소스체인 매수 → 브리지 → 도착체인 매도
4) 안전장치: 견적/실행 타임아웃, 재시도, 만료 처리

## 구성/환경
- `LIFI_API_KEY` (선택)
- 체인별 RPC, 도착 체인 가스 예치

## 실패/리스크 처리
- 브리지 만료/실패 시 재시도 정책
- 상대 체인 혼잡/가격 급변 시 중단
