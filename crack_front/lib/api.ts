type Status = {
  is_running: boolean;
  active_opportunities: number;
  submitted_bundles: number;
  total_profit_eth: string;
  success_rate: number;
  uptime_seconds: number;
};

export type StrategyKey = 'sandwich' | 'liquidation' | 'micro' | 'cross';
export type Strategies = Record<StrategyKey, boolean>;

export type BundleStatsInfo = {
  total_created: number;
  total_submitted: number;
  total_included: number;
  total_failed: number;
  total_profit: unknown;
  total_gas_spent: unknown;
  avg_submission_time_ms: number;
  success_rate: number;
};

export type BundlesSummary = {
  stats: BundleStatsInfo;
  submitted_count: number;
  pending_count: number;
};

export type BundleRow = {
  id: string;
  strategy: string;
  expected_profit: string;
  gas_estimate: number;
  timestamp: string;
  state: 'submitted' | 'pending';
};

const BASE = process.env.NEXT_PUBLIC_BACKEND_URL || 'http://localhost:8080';

export async function getStatus(): Promise<Status> {
  const res = await fetch(`${BASE}/api/status`, { cache: 'no-cache' });
  if (!res.ok) {
    // fallback to metrics server status
    const res2 = await fetch(`${BASE.replace(':8080', ':9090')}/status`, { cache: 'no-cache' });
    if (!res2.ok) throw new Error('status fetch failed');
    return res2.json();
  }
  return res.json();
}

export function defaultStatus(): Status {
  return {
    is_running: false,
    active_opportunities: 0,
    submitted_bundles: 0,
    total_profit_eth: '0.0',
    success_rate: 0,
    uptime_seconds: 0,
  };
}

// ---- Strategies API ----
function normalizeStrategiesMap(input: Record<string, boolean>): Strategies {
  // Backend enum keys: Sandwich, Liquidation, MicroArbitrage, CrossChainArbitrage
  const map: Strategies = { sandwich: false, liquidation: false, micro: false, cross: false };
  Object.entries(input || {}).forEach(([k, v]) => {
    switch (k) {
      case 'Sandwich':
        map.sandwich = v; break;
      case 'Liquidation':
        map.liquidation = v; break;
      case 'MicroArbitrage':
        map.micro = v; break;
      case 'CrossChainArbitrage':
        map.cross = v; break;
      default:
        break;
    }
  });
  return map;
}

export async function getStrategies(): Promise<Strategies> {
  const res = await fetch(`${BASE}/api/strategies`, { cache: 'no-cache' });
  if (!res.ok) return { sandwich: false, liquidation: false, micro: false, cross: false };
  const json = await res.json();
  return normalizeStrategiesMap(json.enabled || {});
}

export async function toggleStrategy(key: StrategyKey, enabled: boolean): Promise<boolean> {
  const res = await fetch(`${BASE}/api/strategies/toggle`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ strategy: key, enabled }),
  });
  if (!res.ok) return false;
  const json = await res.json();
  return !!json.ok;
}

// ---- Bundles API ----
export async function getBundlesSummary(): Promise<BundlesSummary> {
  const res = await fetch(`${BASE}/api/bundles`, { cache: 'no-cache' });
  if (!res.ok) {
    return {
      stats: {
        total_created: 0,
        total_submitted: 0,
        total_included: 0,
        total_failed: 0,
        total_profit: 0,
        total_gas_spent: 0,
        avg_submission_time_ms: 0,
        success_rate: 0,
      },
      submitted_count: 0,
      pending_count: 0,
    };
  }
  const json = await res.json();
  const submitted_count = Array.isArray(json.submitted) ? json.submitted.length : 0;
  const pending_count = Array.isArray(json.pending) ? json.pending.length : 0;
  return { stats: json.stats || {}, submitted_count, pending_count } as BundlesSummary;
}

export async function getBundlesRecent(limit = 5): Promise<BundleRow[]> {
  const res = await fetch(`${BASE}/api/bundles`, { cache: 'no-cache' });
  if (!res.ok) return [];
  const json = await res.json();
  const mapList = (arr: any[], state: 'submitted' | 'pending'): BundleRow[] =>
    (arr || []).slice(0, limit).map((b: any) => ({
      id: b.id,
      strategy: String(b.strategy || ''),
      expected_profit: typeof b.expected_profit === 'string' ? b.expected_profit : JSON.stringify(b.expected_profit ?? '0'),
      gas_estimate: Number(b.gas_estimate || 0),
      timestamp: b.timestamp || '',
      state,
    }));
  return [...mapList(json.submitted, 'submitted'), ...mapList(json.pending, 'pending')]
    .slice(0, limit);
}
