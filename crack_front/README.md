# xCrack Frontend Dashboard

xCrack MEV Searcher의 실시간 모니터링 및 관리 대시보드입니다.

## 기술 스택

- **Next.js 14** (App Router)
- **TypeScript**
- **Tailwind CSS**
- **React 18**

## 주요 기능

### 📊 대시보드
- MEV 기회 실시간 모니터링
- 수익성 분석 및 통계
- 시스템 상태 모니터링

### 🔧 전략 관리

#### 청산 v2.0 (`/liquidation`)
- **프로토콜 기반 스캔**: Aave/Compound 실시간 포지션 모니터링
- **지능형 펀딩**: 자동/플래시론/지갑 모드 최적화
- **DEX 통합**: 0x/1inch 어그리게이터 지원
- **실시간 대시보드**: 청산 기회, 성공률, 수익 통계

#### 마이크로 아비트라지 v2.0 (`/micro-v2`)
- **다층 스케줄러**: 10ms 가격, 50ms 오더북, 100ms 기회 스캔
- **실시간 모니터링**: 가격 차이, 거래량, 유동성 추적
- **지능형 실행**: 최적 경로 선택 및 슬리피지 최적화
- **성과 분석**: 거래 성공률, 평균 수익, 실행 지연시간

### 📡 모니터링

#### 프로토콜 모니터링 (`/protocols`)
- **Aave/Compound 스캐너 상태**: 연결 상태, 스캔 성능
- **포지션 분석**: 총 담보, 부채, 건강도 분포
- **청산 위험 추적**: 실시간 위험 포지션 모니터링
- **성능 메트릭스**: 스캔 지연시간, 처리량 통계

#### 시스템 모니터링
- **API 상태**: 백엔드 서버 연결 및 응답 시간
- **실행 통계**: 전략별 성공률 및 오류율
- **리소스 사용량**: 메모리, CPU 사용률

### ⚙️ 설정 관리

#### 청산 v2.0 설정
- **펀딩 모드**: 자동 선택, 플래시론, 지갑 모드
- **수익성 임계값**: 최소 수익률, 가스 버퍼, 수수료 한도
- **동시 실행**: 최대 동시 청산 수, 큐 관리
- **DEX 통합**: 선호 어그리게이터, 슬리피지 허용치

#### 마이크로 아비트라지 v2.0 설정
- **스케줄러 설정**: 각 레이어별 업데이트 간격
- **기회 검색**: 최소 차익률, 최대 포지션 크기
- **실행 파라미터**: 지연시간 임계값, 재시도 로직
- **리스크 관리**: 최대 손실 한도, 포지션 제한

## 프로젝트 구조

```
crack_front/
├── app/                    # Next.js App Router 페이지
│   ├── page.tsx           # 메인 대시보드
│   ├── liquidation/       # 청산 v2.0 대시보드
│   ├── micro-v2/         # 마이크로 아비트라지 v2.0
│   ├── protocols/        # 프로토콜 모니터링
│   ├── settings/         # 설정 관리
│   └── layout.tsx        # 전체 레이아웃 및 네비게이션
├── components/           # 공유 UI 컴포넌트
│   ├── ApiHealth.tsx     # API 상태 표시기
│   └── [기타 컴포넌트들]
├── lib/                  # 유틸리티 및 API
│   └── api.ts           # 백엔드 API 인터페이스
└── globals.css          # 전역 스타일
```

## API 인터페이스

### 주요 API 엔드포인트

```typescript
// 전략 상태 및 제어
GET  /api/strategies/status
POST /api/strategies/start/:strategy
POST /api/strategies/stop/:strategy

// 청산 v2.0
GET  /api/liquidation/dashboard      # 청산 대시보드 데이터
GET  /api/liquidation/opportunities  # 실시간 청산 기회
GET  /api/protocols/status          # Aave/Compound 스캐너 상태

// 마이크로 아비트라지 v2.0
GET  /api/micro-arbitrage/dashboard  # 마이크로 대시보드
GET  /api/scheduler/metrics         # 스케줄러 성능 메트릭스

// 설정 관리
GET  /api/strategies/params         # 모든 전략 파라미터
PUT  /api/strategies/:strategy/params # 전략별 파라미터 업데이트
```

### 타입 정의

주요 TypeScript 타입들이 `lib/api.ts`에 정의되어 있습니다:

- `LiquidationParams`: 청산 v2.0 설정 파라미터
- `MicroArbitrageParams`: 마이크로 아비트라지 v2.0 설정
- `LiquidationOpportunity`: 청산 기회 데이터
- `ProtocolStatus`: Aave/Compound 프로토콜 상태
- `SchedulerMetrics`: 실시간 스케줄러 메트릭스

## 설치 및 실행

### 전제 조건
- Node.js 18+ 
- npm 또는 yarn
- xCrack 백엔드 서버 실행 중 (포트 8080)

### 설치
```bash
npm install
```

### 개발 모드 실행
```bash
npm run dev
```

브라우저에서 `http://localhost:3000` 접속

### 프로덕션 빌드
```bash
npm run build
npm start
```

## 실시간 데이터 업데이트

대시보드는 다음 간격으로 데이터를 자동 업데이트합니다:

- **전략 상태**: 2초마다
- **청산 기회**: 3초마다  
- **프로토콜 상태**: 5초마다
- **성능 메트릭스**: 1초마다 (활성 모니터링 시)

## 주요 개선사항 (v2.0)

### 청산 전략
- 멤풀 기반 → 프로토콜 상태 기반 스캔으로 전환
- 지능형 펀딩 모드로 수익성 최적화
- DEX 어그리게이터 통합으로 더 나은 스와핑

### 마이크로 아비트라지
- 단일 스케줄러 → 다층 실시간 스케줄러
- 향상된 기회 감지 알고리즘
- 지연시간 최적화 및 동시 실행

### 사용자 경험
- 실시간 모니터링 대시보드
- 상세한 성능 분석 및 통계
- 직관적인 설정 관리 인터페이스

## 문제 해결

### 일반적인 문제

**백엔드 연결 오류**
- xCrack 백엔드 서버가 실행 중인지 확인 (`localhost:8080`)
- CORS 설정이 올바른지 확인

**데이터 로딩 오류**  
- API Health 컴포넌트에서 연결 상태 확인
- 브라우저 개발자 도구에서 네트워크 에러 확인

**설정 저장 실패**
- 파라미터 형식이 올바른지 확인
- 백엔드 로그에서 에러 메시지 확인

## 기여 가이드

1. TypeScript 엄격 모드 준수
2. Tailwind CSS 클래스 사용
3. 컴포넌트 재사용성 고려
4. API 타입 안정성 유지
5. 실시간 업데이트 성능 최적화

## 라이선스

이 프로젝트는 xCrack MEV Searcher의 일부입니다.
