"use client";

import { useEffect, useState } from "react";
import { 
  getMicroArbitrageV2Dashboard,
  getSchedulerMetrics,
  getFundingModeMetrics,
  getMicroOpportunities,
  getMicroTradeHistory,
  type MicroArbitrageV2Dashboard,
  type SchedulerMetrics,
  type FundingModeMetrics,
  type ArbitrageOpportunity,
  type MicroTradeHistory 
} from '@/lib/api';

export default function MicroArbitrageV2Page() {
  const [dashboard, setDashboard] = useState<MicroArbitrageV2Dashboard | null>(null);
  const [schedulerMetrics, setSchedulerMetrics] = useState<SchedulerMetrics | null>(null);
  const [fundingMetrics, setFundingMetrics] = useState<FundingModeMetrics | null>(null);
  const [opportunities, setOpportunities] = useState<ArbitrageOpportunity[]>([]);
  const [trades, setTrades] = useState<MicroTradeHistory[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      setError("");
      try {
        const [dashboardData, schedulerData, fundingData, opportunitiesData, tradesData] = await Promise.all([
          getMicroArbitrageV2Dashboard(),
          getSchedulerMetrics(),
          getFundingModeMetrics(),
          getMicroOpportunities(),
          getMicroTradeHistory(15)
        ]);

        setDashboard(dashboardData);
        setSchedulerMetrics(schedulerData);
        setFundingMetrics(fundingData);
        setOpportunities(opportunitiesData);
        setTrades(tradesData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 2000); // 2초마다 업데이트 (실시간 모니터링)
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로 아비트라지 v2.0</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로 아비트라지 v2.0</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-2xl font-bold">마이크로 아비트라지 v2.0</h1>
        <p className="text-gray-600 mt-1">지능형 자금 조달 시스템 · RealTimeScheduler 다층 스케줄링</p>
      </div>

      {/* 실시간 상태 */}
      {dashboard?.real_time_status && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-2">가격 모니터</h3>
            <div className="flex items-center space-x-2">
              <div className={`w-3 h-3 rounded-full ${
                dashboard.real_time_status.price_monitor_active ? 'bg-green-400 animate-pulse' : 'bg-red-400'
              }`}></div>
              <span className="text-sm">
                {dashboard.real_time_status.price_monitor_active ? '활성 (10ms)' : '비활성'}
              </span>
            </div>
          </div>

          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-2">오더북 모니터</h3>
            <div className="flex items-center space-x-2">
              <div className={`w-3 h-3 rounded-full ${
                dashboard.real_time_status.orderbook_monitor_active ? 'bg-green-400 animate-pulse' : 'bg-red-400'
              }`}></div>
              <span className="text-sm">
                {dashboard.real_time_status.orderbook_monitor_active ? '활성 (50ms)' : '비활성'}
              </span>
            </div>
          </div>

          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-2">기회 스캐너</h3>
            <div className="flex items-center space-x-2">
              <div className={`w-3 h-3 rounded-full ${
                dashboard.real_time_status.opportunity_scanner_active ? 'bg-green-400 animate-pulse' : 'bg-red-400'
              }`}></div>
              <span className="text-sm">
                {dashboard.real_time_status.opportunity_scanner_active ? '활성 (100ms)' : '비활성'}
              </span>
            </div>
            <div className="text-xs text-gray-500 mt-1">
              다음 스캔: {dashboard.real_time_status.next_scan_in_ms}ms 후
            </div>
          </div>

          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-2">자금 조달 모드</h3>
            <div className="text-center">
              <span className={`px-3 py-1 rounded-full text-sm font-medium ${
                dashboard.real_time_status.current_funding_mode === 'auto' 
                  ? 'bg-blue-100 text-blue-800'
                  : dashboard.real_time_status.current_funding_mode === 'flashloan'
                    ? 'bg-purple-100 text-purple-800'
                    : 'bg-green-100 text-green-800'
              }`}>
                {dashboard.real_time_status.current_funding_mode === 'auto' ? '자동 선택' :
                 dashboard.real_time_status.current_funding_mode === 'flashloan' ? '플래시론' : '지갑'}
              </span>
            </div>
          </div>
        </div>
      )}

      {/* RealTimeScheduler 메트릭 */}
      {schedulerMetrics && (
        <div className="border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">RealTimeScheduler 성능</h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {/* 주기별 성능 */}
            <div>
              <h3 className="font-medium mb-3">스케줄링 주기</h3>
              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <span className="text-sm">가격 업데이트:</span>
                  <div className="text-right">
                    <div className="font-medium">{schedulerMetrics.price_update_frequency}ms</div>
                    <div className="text-xs text-gray-500">{(schedulerMetrics.price_updates_per_second || 0).toFixed(1)}/s</div>
                  </div>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-sm">오더북 갱신:</span>
                  <div className="text-right">
                    <div className="font-medium">{schedulerMetrics.orderbook_refresh_frequency}ms</div>
                    <div className="text-xs text-gray-500">{(schedulerMetrics.orderbook_updates_per_second || 0).toFixed(1)}/s</div>
                  </div>
                </div>
                <div className="flex justify-between items-center">
                  <span className="text-sm">기회 스캔:</span>
                  <div className="text-right">
                    <div className="font-medium">{schedulerMetrics.opportunity_scan_frequency}ms</div>
                    <div className="text-xs text-gray-500">{(schedulerMetrics.opportunities_scanned_per_second || 0).toFixed(1)}/s</div>
                  </div>
                </div>
              </div>
            </div>

            {/* 효율성 지표 */}
            <div>
              <h3 className="font-medium mb-3">스케줄러 효율성</h3>
              <div className="space-y-3">
                <div>
                  <div className="flex justify-between mb-1">
                    <span className="text-sm">전체 효율성:</span>
                    <span className="font-medium">{((schedulerMetrics.scheduler_efficiency || 0) * 100).toFixed(1)}%</span>
                  </div>
                  <div className="w-full bg-gray-200 rounded-full h-2">
                    <div 
                      className="bg-green-600 h-2 rounded-full" 
                      style={{ width: `${schedulerMetrics.scheduler_efficiency * 100}%` }}
                    ></div>
                  </div>
                </div>
                <div className="flex justify-between">
                  <span className="text-sm">누락 사이클:</span>
                  <span className="font-medium text-orange-600">{schedulerMetrics.missed_cycles_count}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-sm">평균 지연시간:</span>
                  <span className="font-medium">{(schedulerMetrics.avg_cycle_latency_ms || 0).toFixed(2)}ms</span>
                </div>
              </div>
            </div>

            {/* 처리량 지표 */}
            <div>
              <h3 className="font-medium mb-3">실시간 처리량</h3>
              <div className="space-y-3">
                <div className="bg-blue-50 p-3 rounded-lg">
                  <div className="text-sm text-blue-700">가격 업데이트/초</div>
                  <div className="text-xl font-bold text-blue-800">{(schedulerMetrics.price_updates_per_second || 0).toFixed(0)}</div>
                </div>
                <div className="bg-green-50 p-3 rounded-lg">
                  <div className="text-sm text-green-700">오더북 업데이트/초</div>
                  <div className="text-xl font-bold text-green-800">{(schedulerMetrics.orderbook_updates_per_second || 0).toFixed(0)}</div>
                </div>
                <div className="bg-purple-50 p-3 rounded-lg">
                  <div className="text-sm text-purple-700">기회 스캔/초</div>
                  <div className="text-xl font-bold text-purple-800">{(schedulerMetrics.opportunities_scanned_per_second || 0).toFixed(1)}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 자금 조달 모드 분석 */}
      {fundingMetrics && (
        <div className="border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">자금 조달 모드 분석</h2>
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* 자동 모드 결정 */}
            <div>
              <h3 className="font-medium mb-3">자동 모드 결정 현황</h3>
              <div className="space-y-4">
                <div className="bg-blue-50 p-4 rounded-lg">
                  <div className="flex justify-between items-center mb-2">
                    <span className="font-medium">플래시론 선택</span>
                    <span className="text-xl font-bold text-blue-600">
                      {fundingMetrics.auto_mode_decisions?.flashloan_selected || 0}
                    </span>
                  </div>
                  <div className="w-full bg-blue-200 rounded-full h-2">
                    <div 
                      className="bg-blue-600 h-2 rounded-full" 
                      style={{ 
                        width: `${((fundingMetrics.auto_mode_decisions?.flashloan_selected || 0) / (fundingMetrics.auto_mode_decisions?.total_decisions || 1)) * 100}%`                      }}
                    ></div>
                  </div>
                </div>

                <div className="bg-green-50 p-4 rounded-lg">
                  <div className="flex justify-between items-center mb-2">
                    <span className="font-medium">지갑 선택</span>
                    <span className="text-xl font-bold text-green-600">
                      {fundingMetrics.auto_mode_decisions?.wallet_selected || 0}
                    </span>
                  </div>
                  <div className="w-full bg-green-200 rounded-full h-2">
                    <div 
                      className="bg-green-600 h-2 rounded-full" 
                      style={{ 
                        width: `${((fundingMetrics.auto_mode_decisions?.wallet_selected || 0) / (fundingMetrics.auto_mode_decisions?.total_decisions || 1)) * 100}%` 
                      }}
                    ></div>
                  </div>
                <div className="bg-gray-50 p-4 rounded-lg">
                  <div className="flex justify-between">
                    <span className="text-sm">수익성 없어 건너뜀:</span>
                    <span className="font-medium">{fundingMetrics.auto_mode_decisions?.skipped_unprofitable || 0}</span>
                  </div>
                  <div className="flex justify-between border-t pt-2 mt-2">
                    <span className="text-sm font-medium">총 결정:</span>
                    <span className="font-bold">{fundingMetrics.auto_mode_decisions?.total_decisions || 1}</span>
                  </div>
                </div>
              </div>
            </div>

            {/* 수익성 비교 */}
            <div>
              <h3 className="font-medium mb-3">수익성 비교 분석</h3>
              <div className="space-y-4">
                <div className="border rounded-lg p-4">
                  <h4 className="font-medium mb-3">평균 순수익 비교</h4>
                  <div className="flex justify-between items-center mb-2">
                    <span className="text-sm">플래시론:</span>
                    <span className="font-bold text-blue-600">
                      ${parseFloat(fundingMetrics.profitability_comparison?.flashloan_avg_net_profit || 0).toFixed(2)}
                    </span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-sm">지갑:</span>
                    <span className="font-bold text-green-600">
                      ${parseFloat(fundingMetrics.profitability_comparison?.wallet_avg_net_profit || 0).toFixed(2)}
                    </span>
                  </div>
                </div>

                <div className="border rounded-lg p-4">
                  <h4 className="font-medium mb-3">성공률 비교</h4>
                  <div className="flex justify-between items-center mb-2">
                    <span className="text-sm">플래시론:</span>
                    <span className="font-bold text-blue-600">
                      {(fundingMetrics.profitability_comparison.flashloan_success_rate * 100).toFixed(1)}%
                    </span>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-sm">지갑:</span>
                    <span className="font-bold text-green-600">
                      {(fundingMetrics.profitability_comparison.wallet_success_rate * 100).toFixed(1)}%
                    </span>
                  </div>
                </div>

                <div className="border rounded-lg p-4">
                  <h4 className="font-medium mb-3">비용 분석</h4>
                  <div className="space-y-2">
                    <div className="flex justify-between">
                      <span className="text-sm">플래시론 평균 비용:</span>
                      <span className="font-medium">${parseFloat(fundingMetrics.cost_analysis.flashloan_avg_cost).toFixed(2)}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-sm">지갑 평균 비용:</span>
                      <span className="font-medium">${parseFloat(fundingMetrics.cost_analysis.wallet_avg_cost).toFixed(2)}</span>
                    </div>
                    <div className="flex justify-between pt-2 border-t">
                      <span className="text-sm font-medium">가스 절약률:</span>
                      <span className="font-bold text-green-600">{fundingMetrics.cost_analysis.gas_savings_percentage.toFixed(1)}%</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 전통적인 메트릭 (간략화) */}
      {dashboard?.traditional_metrics && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-1">오늘 총 거래</h3>
            <p className="text-2xl font-bold">{dashboard.traditional_metrics.total_trades_today}</p>
            <p className="text-sm text-green-600">성공: {dashboard.traditional_metrics.successful_trades_today}</p>
          </div>
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-1">오늘 총 수익</h3>
            <p className="text-2xl font-bold text-green-600">${dashboard.traditional_metrics.total_profit_today_usd}</p>
          </div>
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-1">성공률</h3>
            <p className="text-2xl font-bold">{dashboard.traditional_metrics.success_rate_percentage.toFixed(1)}%</p>
          </div>
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-1">활성 기회</h3>
            <p className="text-2xl font-bold text-blue-600">{dashboard.traditional_metrics.active_opportunities}</p>
          </div>
        </div>
      )}

      {/* 활성 기회 (간략화) */}
      {opportunities.length > 0 && (
        <div className="border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">활성 아비트라지 기회 ({opportunities.length})</h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">페어</th>
                  <th className="text-left py-2">매수/매도</th>
                  <th className="text-right py-2">스프레드</th>
                  <th className="text-right py-2">예상 수익</th>
                  <th className="text-center py-2">신뢰도</th>
                </tr>
              </thead>
              <tbody>
                {opportunities.slice(0, 5).map((opp) => (
                  <tr key={opp.id} className="border-b hover:bg-gray-50">
                    <td className="py-2 font-medium">{opp.pair}</td>
                    <td className="py-2 text-xs">
                      <div>{opp.buy_exchange} → {opp.sell_exchange}</div>
                    </td>
                    <td className="py-2 text-right font-medium text-green-600">
                      {opp.spread_percentage.toFixed(3)}%
                    </td>
                    <td className="py-2 text-right font-medium">
                      ${opp.potential_profit_usd}
                    </td>
                    <td className="py-2 text-center">{(opp.confidence_score * 100).toFixed(0)}%</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* 최근 거래 (간략화) */}
      {trades.length > 0 && (
        <div className="border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">최근 거래 기록</h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">시간</th>
                  <th className="text-left py-2">페어</th>
                  <th className="text-right py-2">수익</th>
                  <th className="text-center py-2">상태</th>
                  <th className="text-right py-2">실행시간</th>
                </tr>
              </thead>
              <tbody>
                {trades.slice(0, 8).map((trade) => (
                  <tr key={trade.id} className="border-b hover:bg-gray-50">
                    <td className="py-2 text-xs">{new Date(trade.timestamp).toLocaleTimeString()}</td>
                    <td className="py-2 font-medium">{trade.pair}</td>
                    <td className="py-2 text-right font-medium">
                      <span className={trade.profit_usd.startsWith('-') ? 'text-red-600' : 'text-green-600'}>
                        ${trade.profit_usd}
                      </span>
                    </td>
                    <td className="py-2 text-center">
                      <span className={`px-2 py-1 rounded text-xs ${
                        trade.status === 'success' ? 'bg-green-100 text-green-800' : 
                        trade.status === 'failed' ? 'bg-red-100 text-red-800' : 
                        'bg-yellow-100 text-yellow-800'
                      }`}>
                        {trade.status}
                      </span>
                    </td>
                    <td className="py-2 text-right">{trade.execution_time_ms}ms</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </main>
  );
}
