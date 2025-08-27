# Liquidation Strategy (MEV + Flashloan)

Aave/Compound/MakerDAO 포지션을 스캔하여 청산 기회를 포착하고, 전용 `Liquidation.sol`을 호출해 플래시론으로 자금을 조달한 뒤 즉시 상환/이익 실현합니다. 메인 제출 경로는 MEV 번들입니다.

## 정책 요약
- MEV 사용: Yes
- Flashloan 사용: Yes (Aave V3 flashLoanSimple, Liquidation.sol)

## 데이터 소스
- 프로토콜 상태: Aave V3, Compound V3(Comet), MakerDAO
- 가격: Chainlink, Uniswap TWAP
- DEX 견적: 0x / 1inch (담보 매도 경로)

## 처리 흐름 (요약)

```mermaid
flowchart LR
  A[Protocol Scan] --> B{HF < Threshold?}
  B -- No --> X[Skip]
  B -- Yes --> C[Quote Sell Path (0x/1inch)]
  C --> D[Encode params for Liquidation.sol]
  D --> E[Assemble Flashbots Bundle]
  E --> F[Submit Bundle]
```

## 실행 단계
1) 트리거 탐지: Health Factor(또는 담보/부채 비율) 기준으로 후보 도출
2) 담보 매각 경로 산출: 0x/1inch로 allowanceTarget/Calldata 확보(필요시 승인)
3) 컨트랙트 호출 데이터 생성:
   - `executeLiquidation(asset=debtAsset, amount=debtToCover, params)`
   - params: protocol, user, collateralAsset, debtAsset, debtToCover, dexRouter, swapCalldata
4) 번들 생성/제출: 가스 전략(urgency/competition) 반영
5) 수익 계산/기록: 플래시론 수수료(기본 9bps) 차감 반영

## 구성/환경
- `blockchain.primary_network.liquidation_contract` 필수 설정
- 1inch API 키(선택), 0x는 퍼블릭 가능하지만 rate-limit 고려
- Flashbots private key, relay url

## 실패/리스크 처리
- 슬리피지/견적 타임아웃 시 보수적 폴백
- 승인/스왑 대상 유효성 점검
- 번들 미포함/경쟁 과열 시 가스 상향/재시도 제한
