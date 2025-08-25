### xCrack 프론트엔드 튜토리얼 (Next.js UI)

이 문서는 xCrack의 웹 UI만을 대상으로, 실행 방법과 각 화면/인터랙션/동작을 간단히 설명합니다.

---

### 1) 개요
- 기술 스택: Next.js 14(App Router), React 18, TypeScript
- 위치: `crack_front/`
- 백엔드 주소: `NEXT_PUBLIC_BACKEND_URL` (기본 `http://localhost:8080`)
- 내비게이션: Dashboard, Strategies, Bundles, Logs, Settings
- API 헬스 표시: 헤더 우측(약 5초마다 `/api/health` 폴링)

---

### 2) 사전 준비물
- Node.js 18+ (권장 20+)
- npm
- 백엔드 API가 실행 중이면 모든 데이터가 표시됩니다. 백엔드가 꺼져 있어도 UI는 안전한 기본값으로 렌더링됩니다.

---

### 3) 설치 및 실행
- 개발 모드:
```bash
cd crack_front
npm install
npm run dev
# → http://localhost:7777
```
- 프로덕션 빌드/실행:
```bash
npm run build
npm start
# → http://localhost:7777
```
- 백엔드 URL 설정(옵션):
```bash
# crack_front/.env.local
NEXT_PUBLIC_BACKEND_URL=http://localhost:8080
```

참고
- UI는 `${NEXT_PUBLIC_BACKEND_URL}`에서 데이터를 가져옵니다. 설정하지 않으면 `http://localhost:8080`을 사용합니다.
- 백엔드가 꺼져 있어도 주요 페이지는 안전한 기본값으로 렌더링됩니다.

---

### 4) 내비게이션 & 헤더
- 전역 헤더: Dashboard, Strategies, Bundles, Logs, Settings 링크 제공
- API 헬스 표시 색상:
  - 초록: API 정상
  - 빨강: API 오류
  - 회색: 확인 중/미확인

---

### 5) 페이지와 인터랙션

#### 5.1 대시보드(`/`)
- 사용하는 API:
  - `GET /api/status`
  - `GET /api/bundles`
  - `GET /api/report`
- 보여주는 정보:
  - 실행 상태 카드(실행 여부, 제출 번들 수, 업타임, 성공률, 총 이익)
  - 번들 요약 및 최근 5개 번들
  - 성능 리포트 요약과 권고사항
- 백엔드 다운 시: 안전한 기본값으로 렌더링

#### 5.2 전략(`/strategies`)
- 사용하는 API: `GET /api/strategies`, `GET /api/strategies/stats`
- 보여주는 정보:
  - 전략별 카드(샌드위치/청산/마이크로/크로스), ON/OFF 토글
  - 전략별 메트릭(분석 수, 기회 수, 평균 분석 시간)
  - 상세 페이지 링크
- 자동 새로고침: 약 10초 간격
- 토글 동작: `POST /api/strategies/toggle` 호출(실패 시 다음 갱신에서 되돌아옴)

#### 5.3 전략 상세(`/strategies/[key]`)
- 사용하는 API: `GET /api/strategies/stats`, `GET /api/bundles`
- 보여주는 정보:
  - 선택된 전략의 세부 통계
  - 최근 수익 추이 스파크라인
  - 최근 번들 테이블(최대 ~20개)

#### 5.4 번들(`/bundles`)
- 사용하는 API: `GET /api/bundles`
- 보여주는 정보:
  - 번들 요약(제출/포함 등)
  - 최근 번들 목록(ID, 전략, 예상 이익, 가스 추정, 시간, 상태)
  - 각 ID는 상세로 링크

#### 5.5 번들 상세(`/bundles/[id]`)
- 사용하는 API: `GET /api/bundles/:id`
- 보여주는 정보:
  - 핵심 속성(전략, 예상 이익, 가스 추정, 타임스탬프)
  - 번들 전체 JSON(읽기 전용)

#### 5.6 로그(`/logs`)
- 사용하는 API: `GET /api/stream/logs` (SSE)
- 보여주는 정보:
  - 실시간 알림 리스트(시간/레벨/메시지)
  - 컨트롤: 일시정지/재개, 레벨 필터, 텍스트 검색, 클리어
  - 퀵 액션:
    - “알림 전체 확인” → `POST /api/settings` `{ action: "ack_all_alerts" }`
    - “통계 초기화” → `POST /api/settings` `{ action: "reset_stats" }`

#### 5.7 설정(`/settings`)
- 사용하는 API:
  - `GET /api/settings`(API/메트릭 포트, 활성 맵)
  - `GET /api/strategies/params`(전략 파라미터 조회)
  - `POST /api/settings`(액션 실행)
  - `POST /api/strategies/params`(파라미터 저장)
- 보여주는 정보/동작:
  - 읽기 전용 정보: API 포트, 메트릭 포트, 전략 활성 상태
  - 액션: 통계 초기화, 알림 전체 확인
  - 간단 파라미터 편집: Sandwich `min_profit_eth`, Liquidation `min_profit_eth`, Micro `min_profit_usd`
- 저장 동작:
  - 설정 파일(`config/default.toml`)에 쓰기
  - 응답에 `restart_required: true` 포함(재시작 시 적용)

---

### 6) 백엔드 불가 상황 동작
- Dashboard/Strategies/Bundles는 폴백으로 렌더링 유지
- API 헬스 인디케이터가 오류 상태를 표시
- Logs/Settings의 액션은 실패 시 알림 메시지로 안내

---

### 7) 자주 겪는 이슈 & 팁
- API 주소 불일치: `NEXT_PUBLIC_BACKEND_URL`을 백엔드 주소로 설정
- 개발 CORS: 개발용 CORS는 완화되어 있으나, 운영에서는 화이트리스트로 제한 권장
- SSE 차단: 리버스 프록시/방화벽이 스트리밍 응답을 허용하는지 확인
- 번들 상세 404: `/api/bundles`에 해당 ID가 존재하는지 확인

---

### 8) 로드맵(UI)
- 그래프: 성공률/평균 시간 추이 시각화
- 번들 상세 액션: 시뮬레이션/재제출(백엔드 엔드포인트 필요)
- 파라미터 에디터: 더 많은 필드, 검증, 낙관적 UI
- 역할 기반 접근 제어 및 API 토큰 입력 UI
