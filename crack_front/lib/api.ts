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

export type PerformanceReportSummary = {
  transactions_processed: number;
  opportunities_found: number;
  bundles_submitted: number;
  bundles_included: number;
  total_profit_eth: string;
  success_rate: number;
  avg_analysis_time_ms: number;
  avg_submission_time_ms: number;
};

export type PerformanceReport = {
  summary: PerformanceReportSummary;
  recommendations: string[];
};

export type StrategyStats = Record<string, {
  transactions_analyzed: number;
  opportunities_found: number;
  avg_analysis_time_ms: number;
}>;

// ---- System Info ----
export type EnvVarStatus = { key: string; set: boolean };
export type ExternalApiInfo = {
  name: string;
  category: string;
  description: string;
  docs?: string | null;
  env: EnvVarStatus[];
};

export type SystemInfo = {
  api_mode: string;
  network: string;
  rpc_url: string;
  ws_url?: string | null;
  flashbots_relay_url: string;
  simulation_mode: boolean;
  external_apis: ExternalApiInfo[];
};

export async function getSystemInfo(): Promise<SystemInfo | null> {
  try {
    const res = await fetch(`${BASE}/api/system`, { cache: 'no-cache' });
    if (!res.ok) return null;
    const data = await res.json();
    // Normalize legacy string array format -> rich object format
    if (Array.isArray(data?.external_apis)) {
      data.external_apis = data.external_apis.map((item: any) => {
        if (typeof item === 'string') {
          return {
            name: item,
            category: 'external',
            description: item,
            docs: null,
            env: [],
          } as ExternalApiInfo;
        }
        // Ensure required fields exist
        return {
          name: item?.name ?? 'Unknown',
          category: item?.category ?? 'external',
          description: item?.description ?? item?.name ?? '',
          docs: item?.docs ?? null,
          env: Array.isArray(item?.env) ? item.env : [],
        } as ExternalApiInfo;
      });
    }
    return data as SystemInfo;
  } catch {
    return null;
  }
}

// ---- Strategy Params ----
export type SandwichParams = {
  enabled: boolean;
  min_target_value: string;
  max_slippage: number;
  max_frontrun_size: string;
  min_profit_eth: string;
  min_profit_percentage: number;
  gas_multiplier: number;
  max_gas_price_gwei: string;
  use_flashloan?: boolean;
  flash_loan_amount?: string | null;
};

export type LiquidationParams = {
  enabled: boolean;
  protocols: string[];
  min_health_factor: number;
  max_liquidation_amount: string;
  min_profit_eth: string;
  min_liquidation_amount: string;
  gas_multiplier: number;
  max_gas_price_gwei: string;
  health_factor_threshold: number;
  max_liquidation_size: string;
};

export type MicroParams = {
  enabled: boolean;
  exchanges: any[];
  trading_pairs: string[];
  min_profit_percentage: number;
  min_profit_usd: string;
  max_position_size: string;
  max_concurrent_trades: number;
  execution_timeout_ms: number;
  latency_threshold_ms: number;
  price_update_interval_ms: number;
  order_book_depth: number;
  slippage_tolerance: number;
  fee_tolerance: number;
  risk_limit_per_trade: string;
  daily_volume_limit: string;
  enable_cex_trading: boolean;
  enable_dex_trading: boolean;
  blacklist_tokens: string[];
  priority_tokens: string[];
  runtime_blacklist_ttl_secs: number;
  use_flashloan?: boolean;
  flash_loan_amount?: string | null;
};

export type StrategyParamsResp = {
  sandwich: SandwichParams;
  liquidation: LiquidationParams;
  micro_arbitrage: MicroParams;
  cross_chain_arbitrage?: { enabled: boolean; use_flashloan?: boolean; flash_loan_amount?: string | null };
}

export async function getStrategyParams(): Promise<StrategyParamsResp | null> {
  const res = await fetch(`${BASE}/api/strategies/params`, { cache: 'no-cache' });
  if (!res.ok) return null;
  return res.json();
}

export async function updateStrategyParams(strategy: 'sandwich'|'liquidation'|'micro'|'cross_chain_arbitrage', updates: Record<string, any>): Promise<{ ok: boolean; restart_required?: boolean; error?: string; }> {
  const res = await fetch(`${BASE}/api/strategies/params`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ strategy, updates }),
  });
  if (!res.ok) return { ok: false, error: 'request failed' };
  return res.json();
}

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

export async function getStrategyStats(): Promise<StrategyStats> {
  const res = await fetch(`${BASE}/api/strategies/stats`, { cache: 'no-cache' });
  if (!res.ok) return {};
  const json = await res.json();
  return json.stats || {};
}

// ---- Bundles API ----
export async function getBundlesSummary(): Promise<BundlesSummary> {
  try {
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
  } catch {
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
}

export async function getBundlesRecent(limit = 5): Promise<BundleRow[]> {
  try {
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
  } catch {
    return [];
  }
}

// ---- Bundle detail ----
export async function getBundle(id: string): Promise<any | null> {
  try {
    const res = await fetch(`${BASE}/api/bundles/${id}`, { cache: 'no-cache' });
    if (!res.ok) return null;
    const json = await res.json();
    return json.bundle || null;
  } catch {
    return null;
  }
}

// ---- Report API ----
export async function getReport(): Promise<PerformanceReport> {
  const res = await fetch(`${BASE}/api/report`, { cache: 'no-cache' });
  if (!res.ok) {
    return {
      summary: {
        transactions_processed: 0,
        opportunities_found: 0,
        bundles_submitted: 0,
        bundles_included: 0,
        total_profit_eth: '0',
        success_rate: 0,
        avg_analysis_time_ms: 0,
        avg_submission_time_ms: 0,
      },
      recommendations: [],
    };
  }
  const json = await res.json();
  return { summary: json.summary, recommendations: json.recommendations } as PerformanceReport;
}

// ---- Mempool Monitor API ----
export type MempoolTransaction = {
  hash: string;
  from: string;
  to?: string | null;
  value: string;
  gas_price: string;
  gas_limit: string;
  timestamp: string;
  decoded_type?: string;
  potential_mev?: boolean;
};

export type MempoolStats = {
  total_transactions: number;
  pending_transactions: number;
  avg_gas_price: string;
  min_gas_price: string;
  max_gas_price: string;
  transactions_per_second: number;
  dex_transactions: number;
  mev_opportunities: number;
};

export type MempoolStatus = {
  is_monitoring: boolean;
  connected: boolean;
  last_block: number;
  stats: MempoolStats;
  recent_transactions: MempoolTransaction[];
};

export async function getMempoolStatus(): Promise<MempoolStatus | null> {
  try {
    const res = await fetch(`${BASE}/api/mempool/status`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getMempoolTransactions(limit = 20): Promise<MempoolTransaction[]> {
  try {
    const res = await fetch(`${BASE}/api/mempool/transactions?limit=${limit}`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.transactions || [];
  } catch {
    return [];
  }
}

// ---- Detailed Performance API ----
export type TimeSeriesPoint = {
  timestamp: string;
  value: number;
};

export type StrategyPerformance = {
  strategy: string;
  total_opportunities: number;
  successful_trades: number;
  success_rate: number;
  total_profit_eth: string;
  avg_profit_per_trade: string;
  avg_analysis_time_ms: number;
  avg_execution_time_ms: number;
  gas_efficiency: number;
  hourly_profit: TimeSeriesPoint[];
  daily_success_rate: TimeSeriesPoint[];
};

export type GasAnalytics = {
  avg_gas_price_gwei: number;
  avg_gas_used: number;
  total_gas_spent_eth: string;
  gas_efficiency_score: number;
  gas_price_history: TimeSeriesPoint[];
  gas_usage_by_strategy: { strategy: string; gas_used: number; percentage: number }[];
};

export type ProfitabilityMetrics = {
  total_profit_eth: string;
  profit_trend: 'up' | 'down' | 'stable';
  profit_per_hour: TimeSeriesPoint[];
  profit_by_strategy: { strategy: string; profit: string; percentage: number }[];
  roi_percentage: number;
  break_even_point: string;
};

export type DetailedPerformanceData = {
  strategy_performance: StrategyPerformance[];
  gas_analytics: GasAnalytics;
  profitability_metrics: ProfitabilityMetrics;
  system_health: {
    uptime_percentage: number;
    avg_response_time_ms: number;
    error_rate: number;
    memory_usage_mb: number;
    cpu_usage_percentage: number;
  };
  competitive_analysis: {
    market_share_percentage: number;
    competitor_count: number;
    our_success_rate: number;
    market_avg_success_rate: number;
  };
};

export async function getDetailedPerformance(): Promise<DetailedPerformanceData | null> {
  try {
    const res = await fetch(`${BASE}/api/performance/detailed`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getPerformanceChart(metric: string, timeRange = '24h'): Promise<TimeSeriesPoint[]> {
  try {
    const res = await fetch(`${BASE}/api/performance/chart?metric=${metric}&range=${timeRange}`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.data || [];
  } catch {
    return [];
  }
}

// ---- Alerts API ----
export type AlertSeverity = 'critical' | 'high' | 'medium' | 'low' | 'info';
export type AlertCategory = 'system' | 'performance' | 'security' | 'strategy' | 'network' | 'gas' | 'profit';

export type Alert = {
  id: string;
  title: string;
  message: string;
  severity: AlertSeverity;
  category: AlertCategory;
  timestamp: string;
  acknowledged: boolean;
  source: string;
  metadata?: Record<string, any>;
  action_required?: boolean;
  auto_resolve?: boolean;
  resolved_at?: string | null;
};

export type AlertStats = {
  total_alerts: number;
  unacknowledged_count: number;
  critical_count: number;
  alerts_last_24h: number;
  most_frequent_category: AlertCategory;
  avg_resolution_time_minutes: number;
};

export async function getAlerts(unacknowledged_only = false): Promise<Alert[]> {
  try {
    const res = await fetch(`${BASE}/api/alerts?unacknowledged_only=${unacknowledged_only}`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.alerts || [];
  } catch {
    return [];
  }
}

export async function getAlertStats(): Promise<AlertStats | null> {
  try {
    const res = await fetch(`${BASE}/api/alerts/stats`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function acknowledgeAlert(alertId: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/alerts/${alertId}/acknowledge`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function acknowledgeAllAlerts(): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/alerts/acknowledge-all`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function dismissAlert(alertId: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/alerts/${alertId}/dismiss`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });
    return res.ok;
  } catch {
    return false;
  }
}
