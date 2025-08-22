### xCrack Frontend Tutorial (Next.js UI)

This guide explains how to run and use the xCrack web UI only, focusing on screens, interactions, and expected behaviors.

---

### 1) Overview
- Tech stack: Next.js 14 (App Router), React 18, TypeScript
- Location: `crack_front/`
- Backend base URL: `NEXT_PUBLIC_BACKEND_URL` (default `http://localhost:8080`)
- UI navigation: Dashboard, Strategies, Bundles, Logs, Settings
- API health indicator: Header right side (checks `/api/health` every ~5s)

---

### 2) Prerequisites
- Node.js 18+ (recommended 20+)
- npm
- A running backend API (for full data). UI still renders with safe fallbacks if the backend is down.

---

### 3) Install & Run
- Development:
```bash
cd crack_front
npm install
npm run dev
# Open http://localhost:3000
```
- Production build:
```bash
npm run build
npm start
# Open http://localhost:3000
```
- Configure backend URL (optional):
```bash
# crack_front/.env.local
NEXT_PUBLIC_BACKEND_URL=http://localhost:8080
```

Notes
- UI fetches data from `${NEXT_PUBLIC_BACKEND_URL}`. If not set, it uses `http://localhost:8080`.
- When the backend is not reachable, key pages show safe default values instead of failing the build or rendering.

---

### 4) Navigation & Header
- Global header includes links: Dashboard, Strategies, Bundles, Logs, Settings
- API Health indicator:
  - Green: API reachable
  - Red: API error
  - Gray: Checking / unknown

---

### 5) Pages and Interactions

#### 5.1 Dashboard (`/`)
- Data sources used:
  - `GET /api/status` (core stats)
  - `GET /api/bundles` (summary + recent)
  - `GET /api/report` (performance summary/recommendations)
- What you see:
  - Overall status cards (is running, submitted bundle count, uptime, success rate, total profit)
  - Bundle summary and up to 5 recent bundles
  - Performance report summary and simple recommendations
- Behavior when backend down:
  - Renders with zeroed safe defaults; a failed fetch does not break the page.

#### 5.2 Strategies (`/strategies`)
- Data sources used:
  - `GET /api/strategies` (enabled map)
  - `GET /api/strategies/stats` (per-strategy metrics)
- What you see:
  - Each strategy tile (Sandwich/Liquidation/Micro/Cross) with ON/OFF toggle
  - Per-strategy metrics: transactions analyzed, opportunities found, average analysis time
  - Link to strategy detail
- Auto refresh: ~10s interval
- Toggle interaction:
  - Calls `POST /api/strategies/toggle` with `{ strategy, enabled }`
  - If backend rejects, state reverts on next refresh

#### 5.3 Strategy Detail (`/strategies/[key]`)
- Data sources used:
  - `GET /api/strategies/stats`
  - `GET /api/bundles` (recent list filtered by strategy)
- What you see:
  - Detailed stats of the selected strategy
  - Sparkline of recent profit trend (lightweight inline SVG)
  - Recent bundles table (up to ~20 rows)

#### 5.4 Bundles (`/bundles`)
- Data sources used:
  - `GET /api/bundles`
- What you see:
  - Bundle summary (total submitted/included, etc.)
  - Recent bundle list (ID, strategy, expected profit, gas est., time, state)
  - Each ID links to Bundle Detail

#### 5.5 Bundle Detail (`/bundles/[id]`)
- Data source used:
  - `GET /api/bundles/:id`
- What you see:
  - Key attributes (strategy, expected profit, gas estimate, timestamp)
  - Full bundle JSON dump (readonly)

#### 5.6 Logs (`/logs`)
- Data source used:
  - `GET /api/stream/logs` (SSE)
- What you see:
  - Live alerts list with timestamp/level/message
  - Controls: pause/resume, level filter, text search, clear
  - Quick actions:
    - “알림 전체 확인” → `POST /api/settings` with `{ action: "ack_all_alerts" }`
    - “통계 초기화” → `POST /api/settings` with `{ action: "reset_stats" }`

#### 5.7 Settings (`/settings`)
- Data sources used:
  - `GET /api/settings` (API/metrics port + enabled map)
  - `GET /api/strategies/params` (current strategy params)
  - `POST /api/settings` (actions)
  - `POST /api/strategies/params` (save params)
- What you see:
  - Read-only info: API port, metrics port, enabled map
  - Actions: reset stats, ack all alerts
  - Quick param editors (minimal):
    - Sandwich: `min_profit_eth`
    - Liquidation: `min_profit_eth`
    - Micro: `min_profit_usd`
- Save behavior:
  - Writes to config file (default `config/default.toml`)
  - Response includes `restart_required: true` (apply on restart)

---

### 6) Behavior with Unavailable Backend
- Dashboard/Strategies/Bundles have fallbacks to avoid build/runtime crashes
- API Health indicator shows error state
- Logs/Settings that depend on actions will surface error notifications when calls fail

---

### 7) Common Issues & Tips
- API base URL mismatch → set `NEXT_PUBLIC_BACKEND_URL` to backend origin
- CORS in dev: backend uses permissive CORS for development; in production, restrict origins
- SSE blocked by proxies/firewalls: confirm reverse proxy allows streaming responses
- 404 on Bundle Detail: ensure the bundle ID exists in `/api/bundles` or backend stores it

---

### 8) Roadmap (UI)
- Charts for success rate / average times
- Bundle detail actions: simulate/resubmit (requires backend endpoints)
- Parameter editor: more fields + validation + optimistic UI
- Role-based access & API token input UI
