// API 기본 설정
const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080';

// API 클라이언트 클래스
class ApiClient {
  private baseUrl: string;
  private timeout: number;

  constructor(baseUrl: string = API_BASE_URL, timeout: number = 10000) {
    this.baseUrl = baseUrl;
    this.timeout = timeout;
  }

  private async request<T>(endpoint: string, options: RequestInit = {}): Promise<T | null> {
    const url = `${this.baseUrl}${endpoint}`;
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(url, {
        ...options,
        signal: controller.signal,
        headers: {
          'Content-Type': 'application/json',
          ...options.headers,
        },
      });

      clearTimeout(timeoutId);

      if (!response.ok) {
        console.warn(`API 요청 실패: ${response.status} ${response.statusText} - ${url}`);
        return null; // 에러 대신 null 반환
      }

      return await response.json();
    } catch (error) {
      clearTimeout(timeoutId);
      console.warn(`API 요청 오류: ${url}`, error);
      return null; // 에러 대신 null 반환
    }
  }

  async get<T>(endpoint: string): Promise<T | null> {
    return this.request<T>(endpoint, { method: 'GET' });
  }

  async post<T>(endpoint: string, data?: any): Promise<T | null> {
    return this.request<T>(endpoint, {
      method: 'POST',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  async put<T>(endpoint: string, data?: any): Promise<T | null> {
    return this.request<T>(endpoint, {
      method: 'PUT',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  async delete<T>(endpoint: string): Promise<T | null> {
    return this.request<T>(endpoint, { method: 'DELETE' });
  }
}

// API 클라이언트 인스턴스
const apiClient = new ApiClient();

// 타입 정의
export type Status = {
  is_running: boolean;
  active_opportunities: number;
  submitted_bundles: number;
  total_profit_eth: string;
  success_rate: number;
  uptime_seconds: number;
};

export type StrategyKey = 'sandwich' | 'liquidation' | 'micro' | 'cross';
export type Strategies = Record<StrategyKey, boolean>;

// Alert 관련 타입
export type AlertSeverity = 'info' | 'warning' | 'error' | 'critical';
export type AlertCategory = 'system' | 'strategy' | 'performance' | 'security';

export type Alert = {
  id: string;
  severity: AlertSeverity;
  category: AlertCategory;
  title: string;
  message: string;
  timestamp: string;
  acknowledged: boolean;
  resolved: boolean;
};

export type AlertStats = {
  total: number;
  active: number;
  resolved: number;
  critical: number;
};

export type BundleStatsInfo = {
  total_created: number;
  total_submitted: number;
  total_included: number;
  total_failed: number;
  total_profit: number;
  total_gas_spent: number;
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

export type SystemInfo = {
  api_mode: string;
  network: string;
  rpc_url: string;
  ws_url?: string;
  flashbots_relay_url: string;
  simulation_mode: boolean;
  external_apis: Array<{
    name: string;
    category: string;
    description: string;
    docs?: string;
    env: Array<{
      key: string;
      set: boolean;
    }>;
  }>;
};

export type MempoolTransaction = {
  hash: string;
  from: string;
  to: string;
  value: string;
  gas_price: string;
  gas_limit: number;
  timestamp: string;
  method?: string;
};

export type ProtocolInfo = {
  name: string;
  address: string;
  type: string;
  tvl: string;
  volume_24h: string;
  fees_24h: string;
  last_updated: string;
};

// 기본 상태 반환 함수
export function defaultStatus(): Status {
  return {
    is_running: false,
    active_opportunities: 0,
    submitted_bundles: 0,
    total_profit_eth: '0',
    success_rate: 0,
    uptime_seconds: 0,
  };
}

// API 함수들 - 안전한 방식으로 수정
export async function getStatus(): Promise<Status> {
  const result = await apiClient.get<Status>('/api/status');
  return result || defaultStatus();
}

export async function getBundlesSummary(): Promise<BundlesSummary> {
  const result = await apiClient.get<BundlesSummary>('/api/bundles/summary');
  return result || { stats: { total_created: 0, total_submitted: 0, total_included: 0, total_failed: 0, total_profit: 0, total_gas_spent: 0, avg_submission_time_ms: 0, success_rate: 0 }, submitted_count: 0, pending_count: 0 };
}

export async function getBundlesRecent(limit: number = 10): Promise<BundleRow[]> {
  const result = await apiClient.get<BundleRow[]>(`/api/bundles/recent?limit=${limit}`);
  return result || [];
}

export async function getReport(): Promise<PerformanceReport> {
  const result = await apiClient.get<PerformanceReport>('/api/performance/report');
  return result || { summary: { transactions_processed: 0, opportunities_found: 0, bundles_submitted: 0, bundles_included: 0, total_profit_eth: '0', success_rate: 0, avg_analysis_time_ms: 0, avg_submission_time_ms: 0 }, recommendations: [] };
}

export async function getSystemInfo(): Promise<SystemInfo> {
  const result = await apiClient.get<SystemInfo>('/api/system/info');
  return result || {
    api_mode: 'unknown',
    network: 'unknown',
    rpc_url: 'unknown',
    flashbots_relay_url: 'unknown',
    simulation_mode: false,
    external_apis: []
  };
}

export async function getMempoolTransactions(limit: number = 50): Promise<MempoolTransaction[]> {
  const result = await apiClient.get<MempoolTransaction[]>(`/api/mempool/transactions?limit=${limit}`);
  return result || [];
}

export async function getProtocols(): Promise<ProtocolInfo[]> {
  const result = await apiClient.get<ProtocolInfo[]>('/api/protocols');
  return result || [];
}

export async function getStrategies(): Promise<Strategies> {
  const result = await apiClient.get<Strategies>('/api/strategies');
  return result || { sandwich: false, liquidation: false, micro: false, cross: false };
}

export async function updateStrategies(strategies: Partial<Strategies>): Promise<Strategies> {
  const result = await apiClient.put<Strategies>('/api/strategies', strategies);
  if (!result) {
    throw new Error('전략 업데이트 실패');
  }
  return result;
}

export async function startSearcher(): Promise<{ success: boolean; message: string }> {
  const result = await apiClient.post<{ success: boolean; message: string }>('/api/searcher/start');
  if (!result) {
    throw new Error('서처 시작 실패');
  }
  return result;
}

export async function stopSearcher(): Promise<{ success: boolean; message: string }> {
  const result = await apiClient.post<{ success: boolean; message: string }>('/api/searcher/stop');
  if (!result) {
    throw new Error('서처 중지 실패');
  }
  return result;
}

export async function getLogs(limit: number = 100): Promise<Array<{
  timestamp: string;
  level: string;
  message: string;
  source: string;
}>> {
  const result = await apiClient.get<Array<{
    timestamp: string;
    level: string;
    message: string;
    source: string;
  }>>(`/api/logs?limit=${limit}`);
  return result || [];
}

export async function getAlerts(unacknowledgedOnly: boolean = false): Promise<Alert[]> {
  const endpoint = unacknowledgedOnly ? '/api/alerts?unacknowledged=true' : '/api/alerts';
  const result = await apiClient.get<Alert[]>(endpoint);
  return result || [];
}

export async function resolveAlert(alertId: string): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>(`/api/alerts/${alertId}/resolve`);
  if (!result) {
    throw new Error('알림 해결 실패');
  }
  return result;
}

// 추가 API 함수들 - 안전한 방식으로 수정
export type DetailedPerformance = {
  cpu_usage: number;
  memory_usage: number;
  network_latency: number;
  response_time: number;
  throughput: number;
  error_rate: number;
};

export async function getDetailedPerformance(): Promise<DetailedPerformance> {
  const result = await apiClient.get<DetailedPerformance>('/api/performance/detailed');
  return result || {
    cpu_usage: 0,
    memory_usage: 0,
    network_latency: 0,
    response_time: 0,
    throughput: 0,
    error_rate: 0
  };
}

export async function getAlertStats(): Promise<AlertStats> {
  const result = await apiClient.get<AlertStats>('/api/alerts/stats');
  return result || { total: 0, active: 0, resolved: 0, critical: 0 };
}

export async function acknowledgeAlert(alertId: string): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>(`/api/alerts/${alertId}/acknowledge`);
  return result || { success: false };
}

export async function acknowledgeAllAlerts(): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>('/api/alerts/acknowledge-all');
  return result || { success: false };
}

export async function dismissAlert(alertId: string): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>(`/api/alerts/${alertId}/dismiss`);
  return result || { success: false };
}

export type FlashloanDashboard = {
  total_volume: string;
  total_profit: string;
  active_loans: number;
  success_rate: number;
};

export async function getFlashloanDashboard(): Promise<FlashloanDashboard> {
  const result = await apiClient.get<FlashloanDashboard>('/api/flashloan/dashboard');
  return result || { total_volume: '0', total_profit: '0', active_loans: 0, success_rate: 0 };
}

export type MempoolStatus = {
  total_transactions: number;
  pending_transactions: number;
  avg_gas_price: string;
  network_congestion: number;
};

export async function getMempoolStatus(): Promise<MempoolStatus> {
  const result = await apiClient.get<MempoolStatus>('/api/mempool/status');
  return result || { total_transactions: 0, pending_transactions: 0, avg_gas_price: '0', network_congestion: 0 };
}

export type MicroArbitrageV2Dashboard = {
  total_trades: number;
  total_profit: string;
  success_rate: number;
  avg_profit_per_trade: string;
};

export async function getMicroArbitrageV2Dashboard(): Promise<MicroArbitrageV2Dashboard> {
  const result = await apiClient.get<MicroArbitrageV2Dashboard>('/api/micro-v2/dashboard');
  return result || { total_trades: 0, total_profit: '0', success_rate: 0, avg_profit_per_trade: '0' };
}

export type SchedulerMetrics = {
  active_jobs: number;
  completed_jobs: number;
  failed_jobs: number;
  avg_execution_time: number;
};

export async function getSchedulerMetrics(): Promise<SchedulerMetrics> {
  const result = await apiClient.get<SchedulerMetrics>('/api/scheduler/metrics');
  return result || { active_jobs: 0, completed_jobs: 0, failed_jobs: 0, avg_execution_time: 0 };
}

export type FundingModeMetrics = {
  total_funding: string;
  active_positions: number;
  funding_rate: number;
  utilization_rate: number;
};

export async function getFundingModeMetrics(): Promise<FundingModeMetrics> {
  const result = await apiClient.get<FundingModeMetrics>('/api/funding/metrics');
  return result || { total_funding: '0', active_positions: 0, funding_rate: 0, utilization_rate: 0 };
}

export type MicroOpportunity = {
  id: string;
  pair: string;
  profit: string;
  timestamp: string;
  status: string;
};

export async function getMicroOpportunities(): Promise<MicroOpportunity[]> {
  const result = await apiClient.get<MicroOpportunity[]>('/api/micro/opportunities');
  return result || [];
}

export type MicroTradeHistory = {
  id: string;
  pair: string;
  amount: string;
  profit: string;
  timestamp: string;
};

export async function getMicroTradeHistory(): Promise<MicroTradeHistory[]> {
  const result = await apiClient.get<MicroTradeHistory[]>('/api/micro/trades');
  return result || [];
}

export type MicroArbitrageDashboard = {
  total_trades: number;
  total_profit: string;
  success_rate: number;
  active_opportunities: number;
};

export async function getMicroArbitrageDashboard(): Promise<MicroArbitrageDashboard> {
  const result = await apiClient.get<MicroArbitrageDashboard>('/api/micro/dashboard');
  return result || { total_trades: 0, total_profit: '0', success_rate: 0, active_opportunities: 0 };
}

export type NetworkHealthDashboard = {
  status: string;
  latency: number;
  uptime: number;
  last_check: string;
};

export async function getNetworkHealth(): Promise<NetworkHealthDashboard> {
  const result = await apiClient.get<NetworkHealthDashboard>('/api/network/health');
  return result || { status: 'unknown', latency: 0, uptime: 0, last_check: new Date().toISOString() };
}

export type LatencyTestResult = {
  avg_latency: number;
  min_latency: number;
  max_latency: number;
  test_duration: number;
};

export async function runLatencyTest(): Promise<LatencyTestResult> {
  const result = await apiClient.post<LatencyTestResult>('/api/network/latency-test');
  return result || { avg_latency: 0, min_latency: 0, max_latency: 0, test_duration: 0 };
}

export async function acknowledgeNetworkIncident(incidentId: string): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>(`/api/network/incidents/${incidentId}/acknowledge`);
  return result || { success: false };
}

export type OnChainAnalytics = {
  total_volume: string;
  total_fees: string;
  active_addresses: number;
  transaction_count: number;
};

export async function getOnChainAnalytics(): Promise<OnChainAnalytics> {
  const result = await apiClient.get<OnChainAnalytics>('/api/onchain/analytics');
  return result || { total_volume: '0', total_fees: '0', active_addresses: 0, transaction_count: 0 };
}

export type MevTransaction = {
  hash: string;
  type: string;
  profit: string;
  timestamp: string;
};

export async function getMevTransactions(): Promise<MevTransaction[]> {
  const result = await apiClient.get<MevTransaction[]>('/api/onchain/mev-transactions');
  return result || [];
}

export type WhaleTransaction = {
  hash: string;
  from: string;
  to: string;
  amount: string;
  timestamp: string;
};

export async function getWhaleTransactions(): Promise<WhaleTransaction[]> {
  const result = await apiClient.get<WhaleTransaction[]>('/api/onchain/whale-transactions');
  return result || [];
}

export type FlashLoanActivity = {
  hash: string;
  protocol: string;
  amount: string;
  profit: string;
  timestamp: string;
};

export async function getFlashLoanActivities(): Promise<FlashLoanActivity[]> {
  const result = await apiClient.get<FlashLoanActivity[]>('/api/onchain/flashloan-activities');
  return result || [];
}

export type RiskDashboard = {
  risk_score: number;
  exposure: string;
  max_drawdown: string;
  var_95: string;
  stress_test_results: any;
};

export async function getRiskDashboard(): Promise<RiskDashboard> {
  const result = await apiClient.get<RiskDashboard>('/api/risk/dashboard');
  return result || { risk_score: 0, exposure: '0', max_drawdown: '0', var_95: '0', stress_test_results: null };
}

export type StressTestResult = {
  test_id: string;
  status: string;
  results: any;
  timestamp: string;
};

export async function runStressTest(): Promise<StressTestResult> {
  const result = await apiClient.post<StressTestResult>('/api/risk/stress-test');
  return result || { test_id: '', status: 'failed', results: null, timestamp: new Date().toISOString() };
}

export async function emergencyPauseStrategy(strategyId: string): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>(`/api/strategies/${strategyId}/emergency-pause`);
  return result || { success: false };
}

export async function acknowledgeRiskEvent(eventId: string): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>(`/api/risk/events/${eventId}/acknowledge`);
  return result || { success: false };
}

export type StrategyParamsResp = {
  sandwich: {
    min_profit_eth: string;
  };
  liquidation: {
    min_profit_eth: string;
    funding_mode?: string;
    max_flashloan_fee_bps?: number;
    gas_buffer_pct?: number;
    max_concurrent_liquidations?: number;
    execution_timeout_ms?: number;
    dex_aggregator_enabled?: boolean;
    preferred_dex_aggregator?: string;
  };
  micro_arbitrage: {
    min_profit_usd: string;
    funding_mode?: string;
    max_flashloan_fee_bps?: number;
    gas_buffer_pct?: number;
    price_update_interval?: number;
    orderbook_refresh_interval?: number;
    opportunity_scan_interval?: number;
    allow_aggregator_execution?: boolean;
    preferred_aggregators?: string[];
  };
    use_flashloan?: boolean;
    flash_loan_amount?: string;
  };
};

export async function getStrategyParams(): Promise<StrategyParamsResp> {
  const result = await apiClient.get<StrategyParamsResp>('/api/strategies/params');
  return result || {
    sandwich: { min_profit_eth: '0.01' },
    liquidation: { min_profit_eth: '0.01' },
    micro_arbitrage: { min_profit_usd: '10' }
  };
}

export async function updateStrategyParams(strategy: string, params: Record<string, any>): Promise<{ ok: boolean; error?: string }> {
  const result = await apiClient.put<{ ok: boolean; error?: string }>(`/api/strategies/${strategy}/params`, params);
  return result || { ok: false, error: 'Request failed' };
}

export type StrategyStats = {
  total_strategies: number;
  active_strategies: number;
  total_profit: string;
  success_rate: number;
};

export async function getStrategyStats(): Promise<StrategyStats> {
  const result = await apiClient.get<StrategyStats>('/api/strategies/stats');
  return result || { total_strategies: 0, active_strategies: 0, total_profit: '0', success_rate: 0 };
}

export async function toggleStrategy(strategyId: string, enabled: boolean): Promise<{ success: boolean }> {
  const result = await apiClient.post<{ success: boolean }>(`/api/strategies/${strategyId}/toggle`, { enabled });
  return result || { success: false };
}

export type Bundle = {
  id: string;
  strategy: string;
  transactions: any[];
  expected_profit: string;
  gas_estimate: number;
  status: string;
  timestamp: string;
};

export async function getBundle(bundleId: string): Promise<Bundle> {
  const result = await apiClient.get<Bundle>(`/api/bundles/${bundleId}`);
  return result || {
    id: bundleId,
    strategy: 'unknown',
    transactions: [],
    expected_profit: '0',
    gas_estimate: 0,
    status: 'unknown',
    timestamp: new Date().toISOString()
  };
}

export type LiquidationDashboard = {
  total_liquidations: number;
  total_profit: string;
  active_positions: number;
  success_rate: number;
  pending_executions?: number;
  performance_metrics?: {
    avg_execution_time_ms: number;
    uptime_seconds: number;
    execution_success_rate: number;
  };
};

export async function getLiquidationDashboard(): Promise<LiquidationDashboard> {
  const result = await apiClient.get<LiquidationDashboard>('/api/liquidation/dashboard');
  return result || {
    total_liquidations: 0,
    total_profit: '0',
    active_positions: 0,
    success_rate: 0,
    pending_executions: 0,
    performance_metrics: {
      avg_execution_time_ms: 0,
      uptime_seconds: 0,
      execution_success_rate: 0
    }
  };
}

export type ProtocolStatus = {
  protocol: string;
  status: string;
  users_monitored?: number;
  total_tvl: string;
  liquidatable_positions?: number;
  last_update: number;
};

export async function getProtocolStatus(): Promise<ProtocolStatus[]> {
  const result = await apiClient.get<ProtocolStatus[]>('/api/protocols/status');
  return result || [];
}

export type LiquidationOpportunity = {
  id: string;
  protocol: string;
  position: string;
  collateral?: string;
  debt?: string;
  health_factor: number;
  liquidation_threshold: number;
  estimated_profit: string;
  execution_cost?: string;
  timestamp: number;
};

export type LiquidationOpportunitiesResponse = {
  opportunities: LiquidationOpportunity[];
  total: number;
};

export async function getLiquidationOpportunities(): Promise<LiquidationOpportunitiesResponse> {
  const result = await apiClient.get<LiquidationOpportunitiesResponse>('/api/liquidation/opportunities');
  return result || { opportunities: [], total: 0 };
}

// 에러 핸들링 유틸리티
export class ApiError extends Error {
  constructor(
    message: string,
    public status?: number,
    public code?: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// 재시도 로직이 포함된 API 호출
export async function withRetry<T>(
  apiCall: () => Promise<T>,
  maxRetries: number = 3,
  delay: number = 1000
): Promise<T> {
  let lastError: Error;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      return await apiCall();
    } catch (error) {
      lastError = error as Error;
      
      if (attempt === maxRetries) {
        throw lastError;
      }

      // 지수 백오프
      const waitTime = delay * Math.pow(2, attempt - 1);
      await new Promise(resolve => setTimeout(resolve, waitTime));
    }
  }

  throw lastError!;
}

// 캐시된 API 호출
const cache = new Map<string, { data: any; timestamp: number }>();
const CACHE_DURATION = 30000; // 30초

export async function getCached<T>(
  key: string,
  apiCall: () => Promise<T>,
  ttl: number = CACHE_DURATION
): Promise<T> {
  const cached = cache.get(key);
  const now = Date.now();

  if (cached && (now - cached.timestamp) < ttl) {
    return cached.data;
  }

  try {
    const data = await apiCall();
    cache.set(key, { data, timestamp: now });
    return data;
  } catch (error) {
    // 캐시된 데이터가 있으면 반환
    if (cached) {
      return cached.data;
    }
    throw error;
  }
}

// 캐시 클리어
export function clearCache(): void {
  cache.clear();
}

// 특정 키 캐시 클리어
export function clearCacheKey(key: string): void {
  cache.delete(key);
}
