"use client"

import React, { useEffect, useState } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { motion } from 'framer-motion';
import { 
  getMicroArbitrageV2Dashboard, 
  getSchedulerMetrics, 
  getFundingModeMetrics, 
  getMicroOpportunities, 
  getMicroTradeHistory 
} from "../../lib/api";

interface MicroArbitrageV2Dashboard {
  real_time_status: {
    is_running: boolean;
    current_strategy: string;
    active_opportunities: number;
    last_execution_time: string;
  };
  scheduler_performance: {
    cycles_completed: number;
    avg_cycle_time_ms: number;
    success_rate: number;
    errors_last_hour: number;
  };
  funding_mode_analysis: {
    current_mode: string;
    auto_mode_enabled: boolean;
    flashloan_usage_percentage: number;
    wallet_usage_percentage: number;
  };
  traditional_metrics: {
    total_trades_today: number;
    successful_trades_today: number;
    total_profit_today_usd: string;
    success_rate_percentage: number;
    active_opportunities: number;
  };
}

interface SchedulerMetrics {
  price_update_frequency: number;
  price_updates_per_second: number;
  orderbook_updates_per_second: number;
  opportunities_scanned_per_second: number;
  scheduler_efficiency: number;
  avg_cycle_latency_ms: number;
}

interface FundingModeMetrics {
  auto_mode_decisions: {
    flashloan_selected: number;
    wallet_selected: number;
    skipped_unprofitable: number;
    total_decisions: number;
  };
  profitability_comparison: {
    flashloan_avg_net_profit: string;
    wallet_avg_net_profit: string;
    flashloan_success_rate: number;
    wallet_success_rate: number;
  };
  cost_analysis: {
    flashloan_avg_cost: string;
    wallet_avg_cost: string;
    gas_savings_percentage: number;
  };
}

interface ArbitrageOpportunity {
  id: string;
  pair: string;
  buy_exchange: string;
  sell_exchange: string;
  spread_percentage: number;
  potential_profit_usd: string;
  confidence_score: number;
}

interface MicroTradeHistory {
  id: string;
  timestamp: string;
  pair: string;
  profit_usd: string;
  status: string;
  execution_time_ms: number;
}

export default function MicroArbitrageV2Page() {
  const [dashboard, setDashboard] = useState<MicroArbitrageV2Dashboard | null>(null);
  const [schedulerMetrics, setSchedulerMetrics] = useState<SchedulerMetrics | null>(null);
  const [fundingMetrics, setFundingMetrics] = useState<FundingModeMetrics | null>(null);
  const [opportunities, setOpportunities] = useState<ArbitrageOpportunity[]>([]);
  const [trades, setTrades] = useState<MicroTradeHistory[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const loadData = async () => {
      try {
        const [dashboardData, schedulerData, fundingData, opportunitiesData, tradesData] = await Promise.all([
          getMicroArbitrageV2Dashboard(),
          getSchedulerMetrics(),
          getFundingModeMetrics(),
          getMicroOpportunities(),
          getMicroTradeHistory(),
        ]);

        setDashboard(dashboardData);
        setSchedulerMetrics(schedulerData);
        setFundingMetrics(fundingData);
        setOpportunities(opportunitiesData || []);
        setTrades(tradesData || []);
        setError("");
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로 아비트래지 v2.0</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로 아비트래지 v2.0</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      {/* 헤더 */}
      <motion.div 
        initial={{ opacity: 0, y: -20 }}
        animate={{ opacity: 1, y: 0 }}
        className="border-b pb-4"
      >
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-bold">마이크로 아비트래지 v2.0</h1>
            <p className="text-gray-600 mt-1">지능형 자금 조달 시스템 · RealTimeScheduler 다층 스케줄링</p>
          </div>
          <div className="flex items-center space-x-2">
            <div className={`w-3 h-3 rounded-full ${dashboard?.real_time_status?.is_running ? 'bg-green-400 animate-pulse' : 'bg-red-400'}`}></div>
            <span className="text-sm text-gray-600">
              {dashboard?.real_time_status?.is_running ? '실행 중' : '중지됨'}
            </span>
          </div>
        </div>
      </motion.div>

      {/* 실시간 상태 */}
      {dashboard?.real_time_status && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>실시간 상태</CardTitle>
              <CardDescription>현재 실행 중인 전략 및 기회</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
                <div className="text-center">
                  <div className="text-2xl font-bold text-blue-600">{dashboard.real_time_status.active_opportunities}</div>
                  <div className="text-sm text-gray-500">활성 기회</div>
                </div>
                <div className="text-center">
                  <div className="text-2xl font-bold text-green-600">{dashboard.real_time_status.current_strategy}</div>
                  <div className="text-sm text-gray-500">현재 전략</div>
                </div>
                <div className="text-center">
                  <div className="text-2xl font-bold text-purple-600">
                    {dashboard.real_time_status.last_execution_time ? 
                      new Date(dashboard.real_time_status.last_execution_time).toLocaleTimeString() : 
                      'N/A'
                    }
                  </div>
                  <div className="text-sm text-gray-500">마지막 실행</div>
                </div>
                <div className="text-center">
                  <Badge variant={dashboard.real_time_status.is_running ? "success" : "destructive"}>
                    {dashboard.real_time_status.is_running ? "실행 중" : "중지됨"}
                  </Badge>
                </div>
              </div>
            </CardContent>
          </Card>
        </motion.div>
      )}

      {/* 스케줄러 성능 */}
      {schedulerMetrics && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.2 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>RealTimeScheduler 성능</CardTitle>
              <CardDescription>다층 스케줄링 시스템 메트릭</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
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
                      <div className="font-medium">{(schedulerMetrics.orderbook_updates_per_second || 0).toFixed(1)}/s</div>
                    </div>
                  </div>
                  <div className="flex justify-between items-center">
                    <span className="text-sm">기회 스캔:</span>
                    <div className="text-right">
                      <div className="font-medium">{(schedulerMetrics.opportunities_scanned_per_second || 0).toFixed(1)}/s</div>
                    </div>
                  </div>
                </div>
                
                <div className="space-y-3">
                  <div className="flex justify-between items-center">
                    <span className="text-sm">스케줄러 효율성:</span>
                    <span className="font-medium">{((schedulerMetrics.scheduler_efficiency || 0) * 100).toFixed(1)}%</span>
                  </div>
                  <div className="w-full bg-gray-200 rounded-full h-2">
                    <div 
                      className="bg-green-600 h-2 rounded-full" 
                      style={{ width: `${(schedulerMetrics.scheduler_efficiency || 0) * 100}%` }}
                    ></div>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-sm font-medium">평균 사이클 지연:</span>
                    <span className="font-medium">{(schedulerMetrics.avg_cycle_latency_ms || 0).toFixed(2)}ms</span>
                  </div>
                </div>

                <div className="space-y-3">
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
                      <div className="text-xl font-bold text-purple-800">{(schedulerMetrics.opportunities_scanned_per_second || 0).toFixed(0)}</div>
                    </div>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </motion.div>
      )}

      {/* 자금 조달 모드 분석 */}
      {fundingMetrics && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.3 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>자금 조달 모드 분석</CardTitle>
              <CardDescription>지능형 자금 조달 시스템</CardDescription>
            </CardHeader>
            <CardContent>
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
                            width: `${((fundingMetrics.auto_mode_decisions?.flashloan_selected || 0) / (fundingMetrics.auto_mode_decisions?.total_decisions || 1)) * 100}%` 
                          }}
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
                          ${parseFloat(fundingMetrics.profitability_comparison?.flashloan_avg_net_profit || '0').toFixed(2)}
                        </span>
                      </div>
                      <div className="flex justify-between items-center">
                        <span className="text-sm">지갑:</span>
                        <span className="font-bold text-green-600">
                          ${parseFloat(fundingMetrics.profitability_comparison?.wallet_avg_net_profit || '0').toFixed(2)}
                        </span>
                      </div>
                    </div>

                    <div className="border rounded-lg p-4">
                      <h4 className="font-medium mb-3">성공률 비교</h4>
                      <div className="flex justify-between items-center mb-2">
                        <span className="text-sm">플래시론:</span>
                        <span className="font-bold text-blue-600">
                          {((fundingMetrics.profitability_comparison?.flashloan_success_rate || 0) * 100).toFixed(1)}%
                        </span>
                      </div>
                      <div className="flex justify-between items-center">
                        <span className="text-sm">지갑:</span>
                        <span className="font-bold text-green-600">
                          {((fundingMetrics.profitability_comparison?.wallet_success_rate || 0) * 100).toFixed(1)}%
                        </span>
                      </div>
                    </div>

                    <div className="border rounded-lg p-4">
                      <h4 className="font-medium mb-3">비용 분석</h4>
                      <div className="space-y-2">
                        <div className="flex justify-between">
                          <span className="text-sm">플래시론 평균 비용:</span>
                          <span className="font-medium">${parseFloat(fundingMetrics.cost_analysis?.flashloan_avg_cost || '0').toFixed(2)}</span>
                        </div>
                        <div className="flex justify-between">
                          <span className="text-sm">지갑 평균 비용:</span>
                          <span className="font-medium">${parseFloat(fundingMetrics.cost_analysis?.wallet_avg_cost || '0').toFixed(2)}</span>
                        </div>
                        <div className="flex justify-between pt-2 border-t">
                          <span className="text-sm font-medium">가스 절약률:</span>
                          <span className="font-bold text-green-600">{(fundingMetrics.cost_analysis?.gas_savings_percentage || 0).toFixed(1)}%</span>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </motion.div>
      )}

      {/* 전통적인 메트릭 */}
      {dashboard?.traditional_metrics && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.4 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>전통적인 메트릭</CardTitle>
              <CardDescription>일반적인 거래 성과 지표</CardDescription>
            </CardHeader>
            <CardContent>
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
            </CardContent>
          </Card>
        </motion.div>
      )}

      {/* 활성 기회 */}
      {opportunities.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.5 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>활성 아비트래지 기회 ({opportunities.length})</CardTitle>
              <CardDescription>현재 감지된 수익 기회</CardDescription>
            </CardHeader>
            <CardContent>
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
            </CardContent>
          </Card>
        </motion.div>
      )}

      {/* 최근 거래 */}
      {trades.length > 0 && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.6 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>최근 거래 기록</CardTitle>
              <CardDescription>실행된 거래 내역</CardDescription>
            </CardHeader>
            <CardContent>
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
            </CardContent>
          </Card>
        </motion.div>
      )}
    </main>
  );
}
