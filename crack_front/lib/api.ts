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

// ---- Micro Arbitrage Specific API ----
export type TradingPairInfo = {
  pair: string;
  base_token: string;
  quote_token: string;
  exchanges: string[];
  spread_percentage: number;
  volume_24h: string;
  last_trade_time: string;
  is_active: boolean;
};

export type ExchangeInfo = {
  name: string;
  type: 'CEX' | 'DEX';
  connected: boolean;
  last_ping_ms: number;
  trading_fee_percentage: number;
  supported_pairs: string[];
  volume_24h: string;
  reliability_score: number;
};

export type ArbitrageOpportunity = {
  id: string;
  pair: string;
  buy_exchange: string;
  sell_exchange: string;
  buy_price: string;
  sell_price: string;
  spread_percentage: number;
  potential_profit_usd: string;
  required_capital: string;
  estimated_gas_cost: string;
  confidence_score: number;
  expires_at: string;
  risk_level: 'low' | 'medium' | 'high';
  execution_time_estimate_ms: number;
};

export type MicroTradeHistory = {
  id: string;
  pair: string;
  buy_exchange: string;
  sell_exchange: string;
  buy_amount: string;
  sell_amount: string;
  profit_usd: string;
  execution_time_ms: number;
  gas_cost: string;
  status: 'success' | 'failed' | 'partial';
  timestamp: string;
  failure_reason?: string;
};

export type MicroArbitrageMetrics = {
  total_trades_today: number;
  successful_trades_today: number;
  total_profit_today_usd: string;
  avg_profit_per_trade: string;
  avg_execution_time_ms: number;
  success_rate_percentage: number;
  active_opportunities: number;
  monitored_pairs: number;
  connected_exchanges: number;
  position_size_utilization: number;
  daily_volume_limit_used: number;
  risk_limit_used: number;
  best_performing_pair: string;
  worst_performing_pair: string;
};

export type PriceAnalytics = {
  pair: string;
  current_spread: number;
  avg_spread_1h: number;
  avg_spread_24h: number;
  max_spread_24h: number;
  min_spread_24h: number;
  spread_volatility: number;
  price_history: TimeSeriesPoint[];
  volume_history: TimeSeriesPoint[];
};

export type MicroArbitrageDashboard = {
  metrics: MicroArbitrageMetrics;
  active_opportunities: ArbitrageOpportunity[];
  recent_trades: MicroTradeHistory[];
  trading_pairs: TradingPairInfo[];
  exchanges: ExchangeInfo[];
  price_analytics: PriceAnalytics[];
  risk_analysis: {
    current_exposure_usd: string;
    max_exposure_usd: string;
    diversification_score: number;
    correlation_risk: number;
    liquidity_risk: number;
  };
};

export async function getMicroArbitrageDashboard(): Promise<MicroArbitrageDashboard | null> {
  try {
    const res = await fetch(`${BASE}/api/strategies/micro/dashboard`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getMicroOpportunities(): Promise<ArbitrageOpportunity[]> {
  try {
    const res = await fetch(`${BASE}/api/strategies/micro/opportunities`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.opportunities || [];
  } catch {
    return [];
  }
}

export async function getMicroTradeHistory(limit = 50): Promise<MicroTradeHistory[]> {
  try {
    const res = await fetch(`${BASE}/api/strategies/micro/trades?limit=${limit}`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.trades || [];
  } catch {
    return [];
  }
}

export async function getMicroPairAnalytics(pair: string): Promise<PriceAnalytics | null> {
  try {
    const res = await fetch(`${BASE}/api/strategies/micro/analytics/${pair}`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

// ---- On-Chain Data Analysis API ----
export type BlockInfo = {
  number: number;
  hash: string;
  timestamp: string;
  gas_used: string;
  gas_limit: string;
  base_fee: string;
  miner: string;
  transaction_count: number;
  mev_transactions: number;
  total_mev_value: string;
};

export type TokenInfo = {
  address: string;
  name: string;
  symbol: string;
  decimals: number;
  total_supply: string;
  market_cap?: string;
  price_usd?: string;
  volume_24h?: string;
  liquidity?: string;
  verified: boolean;
};

export type DexPool = {
  address: string;
  dex_name: string;
  token0: TokenInfo;
  token1: TokenInfo;
  reserve0: string;
  reserve1: string;
  total_liquidity_usd: string;
  volume_24h_usd: string;
  fee_tier: string;
  apy?: number;
  last_trade_time: string;
};

export type OnChainTransaction = {
  hash: string;
  block_number: number;
  from: string;
  to?: string;
  value: string;
  gas_used: string;
  gas_price: string;
  timestamp: string;
  function_name?: string;
  token_transfers: TokenTransfer[];
  mev_type?: 'sandwich' | 'arbitrage' | 'liquidation' | 'frontrun' | 'backrun';
  mev_profit?: string;
  dex_trades?: DexTrade[];
};

export type TokenTransfer = {
  token_address: string;
  token_symbol: string;
  from: string;
  to: string;
  amount: string;
  amount_usd?: string;
};

export type DexTrade = {
  dex_name: string;
  pool_address: string;
  token_in: string;
  token_out: string;
  amount_in: string;
  amount_out: string;
  price_impact: number;
};

export type GasTracking = {
  current_base_fee: string;
  recommended_gas_prices: {
    slow: string;
    standard: string;
    fast: string;
    instant: string;
  };
  gas_price_history: TimeSeriesPoint[];
  block_utilization: number;
  next_base_fee_estimate: string;
};

export type MevMetrics = {
  total_mev_volume_eth: string;
  mev_transactions_count: number;
  sandwich_attacks: number;
  arbitrage_opportunities: number;
  liquidations: number;
  average_mev_per_block: string;
  top_mev_bots: {
    address: string;
    profit_eth: string;
    transaction_count: number;
    success_rate: number;
  }[];
};

export type LiquidityAnalysis = {
  total_tvl_usd: string;
  top_pools: DexPool[];
  new_pools_24h: number;
  removed_liquidity_24h: string;
  added_liquidity_24h: string;
  impermanent_loss_risk: 'low' | 'medium' | 'high';
};

export type OnChainAnalytics = {
  latest_block: BlockInfo;
  gas_tracking: GasTracking;
  mev_metrics: MevMetrics;
  liquidity_analysis: LiquidityAnalysis;
  trending_tokens: TokenInfo[];
  whale_transactions: OnChainTransaction[];
  flash_loan_activities: OnChainTransaction[];
  protocol_stats: {
    uniswap_v3_volume: string;
    uniswap_v2_volume: string;
    sushiswap_volume: string;
    aave_tvl: string;
    compound_tvl: string;
  };
};

export async function getOnChainAnalytics(): Promise<OnChainAnalytics | null> {
  try {
    const res = await fetch(`${BASE}/api/onchain/analytics`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

// ---- Cross-Chain Dashboard API ----
export type CrossDashboard = {
  summary: {
    total_opportunities: number;
    trades_executed: number;
    success_rate: number;
    total_profit: number;
    avg_execution_time: number;
    failed_trades: number;
  };
  recent_routes: { protocol: string; from: string; to: string; avg_time: number; success_rate: number }[];
};

export async function getCrossDashboard(): Promise<CrossDashboard | null> {
  try {
    const res = await fetch(`${BASE}/api/strategies/cross/dashboard`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getTokenAnalysis(tokenAddress: string): Promise<TokenInfo | null> {
  try {
    const res = await fetch(`${BASE}/api/onchain/token/${tokenAddress}`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getPoolAnalysis(poolAddress: string): Promise<DexPool | null> {
  try {
    const res = await fetch(`${BASE}/api/onchain/pool/${poolAddress}`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getMevTransactions(limit = 20): Promise<OnChainTransaction[]> {
  try {
    const res = await fetch(`${BASE}/api/onchain/mev-transactions?limit=${limit}`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.transactions || [];
  } catch {
    return [];
  }
}

export async function getWhaleTransactions(minValueUsd = 100000, limit = 20): Promise<OnChainTransaction[]> {
  try {
    const res = await fetch(`${BASE}/api/onchain/whale-transactions?min_value=${minValueUsd}&limit=${limit}`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.transactions || [];
  } catch {
    return [];
  }
}

export async function getFlashLoanActivities(limit = 20): Promise<OnChainTransaction[]> {
  try {
    const res = await fetch(`${BASE}/api/onchain/flashloans?limit=${limit}`, { cache: 'no-cache' });
    if (!res.ok) return [];
    const json = await res.json();
    return json.transactions || [];
  } catch {
    return [];
  }
}

// ---- Network Health Monitor API ----
export type NodeInfo = {
  name: string;
  url: string;
  type: 'RPC' | 'WebSocket' | 'GraphQL';
  status: 'healthy' | 'degraded' | 'down' | 'unknown';
  last_check: string;
  response_time_ms: number;
  uptime_percentage: number;
  block_height: number;
  syncing: boolean;
  peer_count: number;
  version?: string;
  error_message?: string;
};

export type NetworkMetrics = {
  current_block: number;
  blocks_behind: number;
  avg_block_time_seconds: number;
  network_hash_rate: string;
  difficulty: string;
  pending_transactions: number;
  gas_price_gwei: number;
  total_nodes_monitored: number;
  healthy_nodes: number;
  degraded_nodes: number;
  down_nodes: number;
};

export type FlashbotsStatus = {
  relay_status: 'online' | 'offline' | 'degraded';
  last_bundle_submitted: string;
  bundle_inclusion_rate: number;
  avg_bundle_response_time_ms: number;
  total_bundles_submitted_today: number;
  successful_bundles_today: number;
  relay_uptime_percentage: number;
  estimated_relay_load: number;
};

export type ExternalServiceStatus = {
  service_name: string;
  category: 'price_feed' | 'data_provider' | 'infrastructure' | 'dex' | 'analytics';
  status: 'operational' | 'degraded' | 'outage' | 'unknown';
  response_time_ms: number;
  last_successful_call: string;
  error_rate_percentage: number;
  rate_limit_remaining?: number;
  rate_limit_reset?: string;
  uptime_percentage: number;
  incidents_24h: number;
};

export type SystemResourceMetrics = {
  cpu_usage_percentage: number;
  memory_usage_percentage: number;
  memory_used_mb: number;
  memory_total_mb: number;
  disk_usage_percentage: number;
  disk_free_gb: number;
  network_in_mbps: number;
  network_out_mbps: number;
  active_connections: number;
  load_average: number[];
  goroutines_count?: number;
  heap_size_mb?: number;
};

export type NetworkLatencyTest = {
  target: string;
  latency_ms: number;
  packet_loss_percentage: number;
  jitter_ms: number;
  timestamp: string;
  status: 'good' | 'fair' | 'poor';
};

export type NetworkHealthDashboard = {
  network_metrics: NetworkMetrics;
  nodes: NodeInfo[];
  flashbots_status: FlashbotsStatus;
  external_services: ExternalServiceStatus[];
  system_resources: SystemResourceMetrics;
  latency_tests: NetworkLatencyTest[];
  network_incidents: {
    id: string;
    title: string;
    severity: 'critical' | 'high' | 'medium' | 'low';
    status: 'open' | 'investigating' | 'resolved';
    started_at: string;
    resolved_at?: string;
    affected_services: string[];
    description: string;
  }[];
  performance_trends: {
    block_time_trend: TimeSeriesPoint[];
    gas_price_trend: TimeSeriesPoint[];
    node_uptime_trend: TimeSeriesPoint[];
    system_load_trend: TimeSeriesPoint[];
  };
};

export async function getNetworkHealth(): Promise<NetworkHealthDashboard | null> {
  try {
    const res = await fetch(`${BASE}/api/network/health`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getNodeStatus(nodeUrl: string): Promise<NodeInfo | null> {
  try {
    const res = await fetch(`${BASE}/api/network/node-status?url=${encodeURIComponent(nodeUrl)}`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function getServiceStatus(serviceName: string): Promise<ExternalServiceStatus | null> {
  try {
    const res = await fetch(`${BASE}/api/network/service-status/${serviceName}`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function runLatencyTest(target: string): Promise<NetworkLatencyTest | null> {
  try {
    const res = await fetch(`${BASE}/api/network/latency-test`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ target }),
    });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function acknowledgeNetworkIncident(incidentId: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/network/incidents/${incidentId}/acknowledge`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });
    return res.ok;
  } catch {
    return false;
  }
}

// ---- Risk Management API ----
export type PositionRisk = {
  strategy: string;
  token_symbol: string;
  position_size_usd: string;
  max_position_size_usd: string;
  utilization_percentage: number;
  unrealized_pnl_usd: string;
  risk_level: 'low' | 'medium' | 'high' | 'critical';
  liquidation_price?: string;
  margin_ratio?: number;
  time_to_liquidation_hours?: number;
};

export type VolatilityMetrics = {
  token_symbol: string;
  price_usd: string;
  volatility_24h: number;
  volatility_7d: number;
  volatility_30d: number;
  beta_to_eth: number;
  correlation_to_btc: number;
  risk_rating: 'low' | 'medium' | 'high' | 'extreme';
  var_95_1d: string; // Value at Risk 95% 1-day
  expected_shortfall: string;
};

export type LiquidityRisk = {
  token_symbol: string;
  total_liquidity_usd: string;
  depth_1_percent: string;
  depth_5_percent: string;
  bid_ask_spread_bps: number;
  liquidity_score: number;
  slippage_impact_1k: number;
  slippage_impact_10k: number;
  slippage_impact_100k: number;
  market_impact_score: number;
};

export type CounterpartyRisk = {
  counterparty_name: string;
  counterparty_type: 'dex' | 'cex' | 'protocol' | 'bridge';
  exposure_usd: string;
  risk_score: number;
  credit_rating?: string;
  tvl_usd?: string;
  security_audit_score?: number;
  insurance_coverage?: boolean;
  last_incident?: string;
  incident_count_6m: number;
};

export type GasRisk = {
  current_gas_price_gwei: number;
  gas_price_volatility: number;
  max_acceptable_gas_gwei: number;
  gas_limit_buffer_percentage: number;
  estimated_daily_gas_cost_eth: string;
  gas_cost_vs_profit_ratio: number;
  gas_spike_probability: number;
  optimal_gas_threshold_gwei: number;
};

export type PortfolioRisk = {
  total_exposure_usd: string;
  max_daily_loss_usd: string;
  current_drawdown_percentage: number;
  max_drawdown_percentage: number;
  sharpe_ratio: number;
  sortino_ratio: number;
  calmar_ratio: number;
  win_rate_percentage: number;
  avg_win_loss_ratio: number;
  correlation_matrix: Record<string, Record<string, number>>;
};

export type RiskLimit = {
  id: string;
  name: string;
  type: 'position_size' | 'daily_loss' | 'gas_cost' | 'concentration' | 'leverage';
  current_value: string;
  limit_value: string;
  utilization_percentage: number;
  status: 'safe' | 'warning' | 'breached' | 'critical';
  last_updated: string;
  auto_action?: 'pause' | 'reduce' | 'liquidate';
};

export type StressTestScenario = {
  id: string;
  name: string;
  description: string;
  parameters: {
    eth_price_change: number;
    gas_price_multiplier: number;
    liquidity_reduction: number;
    volatility_multiplier: number;
  };
  results: {
    estimated_loss_usd: string;
    positions_at_risk: number;
    liquidation_probability: number;
    recovery_time_hours: number;
  };
  last_run: string;
};

export type RiskEvent = {
  id: string;
  timestamp: string;
  event_type: 'limit_breach' | 'position_liquidated' | 'high_volatility' | 'gas_spike' | 'counterparty_issue';
  severity: 'low' | 'medium' | 'high' | 'critical';
  title: string;
  description: string;
  affected_strategies: string[];
  impact_usd: string;
  auto_action_taken?: string;
  manual_action_required: boolean;
  resolved: boolean;
  resolution_time_minutes?: number;
};

export type RiskDashboard = {
  portfolio_risk: PortfolioRisk;
  position_risks: PositionRisk[];
  volatility_metrics: VolatilityMetrics[];
  liquidity_risks: LiquidityRisk[];
  counterparty_risks: CounterpartyRisk[];
  gas_risk: GasRisk;
  risk_limits: RiskLimit[];
  stress_test_scenarios: StressTestScenario[];
  recent_risk_events: RiskEvent[];
  risk_summary: {
    overall_risk_score: number;
    risk_adjusted_return: number;
    risk_capacity_utilization: number;
    days_since_last_incident: number;
    active_alerts_count: number;
  };
};

export async function getRiskDashboard(): Promise<RiskDashboard | null> {
  try {
    const res = await fetch(`${BASE}/api/risk/dashboard`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}

export async function runStressTest(scenarioId: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/risk/stress-test/${scenarioId}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function updateRiskLimit(limitId: string, newLimit: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/risk/limits/${limitId}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ limit_value: newLimit }),
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function acknowledgeRiskEvent(eventId: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/risk/events/${eventId}/acknowledge`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function emergencyPauseStrategy(strategy: string): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/api/risk/emergency-pause`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ strategy }),
    });
    return res.ok;
  } catch {
    return false;
  }
}

// ---- Flashloan API ----
export type FlashloanProvider = {
  available: boolean;
  max_amount: string;
  fee_rate: string;
  gas_cost: string;
  last_update: number;
};

export type FlashloanTransaction = {
  tx_hash: string;
  timestamp: number;
  provider: string;
  token: string;
  amount: string;
  fee_paid: string;
  strategy: string;
  profit: string;
  gas_used: string;
  status: 'success' | 'failed' | 'pending';
};

export type FlashloanContract = {
  address: string;
  name: string;
  verified: boolean;
  proxy: boolean;
  implementation: string | null;
};

export type SmartContract = {
  solidity_version: string;
  source_code: string;
};

export type FlashloanDashboard = {
  flashloan_providers: Record<string, FlashloanProvider>;
  recent_flashloans: FlashloanTransaction[];
  performance_metrics: {
    total_flashloans: number;
    total_volume: string;
    total_fees_paid: string;
    total_profit: string;
    success_rate: number;
    avg_profit_per_loan: string;
    most_used_provider: string;
  };
  flashloan_contracts: Record<string, FlashloanContract>;
  smart_contracts: Record<string, SmartContract>;
  gas_analytics: {
    avg_gas_per_flashloan: string;
    most_expensive_flashloan: string;
    cheapest_flashloan: string;
    gas_optimization_savings: string;
  };
};

export async function getFlashloanDashboard(): Promise<FlashloanDashboard | null> {
  try {
    const res = await fetch(`${BASE}/api/flashloan/dashboard`, { cache: 'no-cache' });
    if (!res.ok) return null;
    return res.json();
  } catch {
    return null;
  }
}
