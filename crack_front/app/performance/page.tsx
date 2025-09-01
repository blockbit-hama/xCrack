'use client';

import { useEffect, useState } from 'react';
import { getDetailedPerformance, getPerformanceChart, DetailedPerformanceData, TimeSeriesPoint } from '@/lib/api';

export default function PerformancePage() {
  const [data, setData] = useState<DetailedPerformanceData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [timeRange, setTimeRange] = useState<'1h' | '6h' | '24h' | '7d'>('24h');

  const fetchData = async () => {
    try {
      const perfData = await getDetailedPerformance();
      setData(perfData);
      setError(null);
    } catch (err) {
      setError('Failed to fetch performance data');
      console.error('Performance fetch error:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 30000); // 30초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const formatEth = (eth: string | undefined) => {
    if (!eth) return '0.000000';
    const value = parseFloat(eth);
    return isNaN(value) ? '0.000000' : value.toFixed(6);
  };

  const formatPercentage = (value: number | undefined) => {
    if (value === undefined || isNaN(value)) return '0.0%';
    return `${(value * 100).toFixed(1)}%`;
  };

  const formatTrend = (trend: 'up' | 'down' | 'stable' | undefined) => {
    const icons = { up: '📈', down: '📉', stable: '➡️' };
    const colors = { up: 'text-green-600', down: 'text-red-600', stable: 'text-gray-600' };
    const defaultTrend = 'stable';
    const actualTrend = trend || defaultTrend;
    return { icon: icons[actualTrend], color: colors[actualTrend] };
  };

  if (loading) {
    return (
      <div className="p-6">
        <h1 className="text-3xl font-bold mb-6">상세 성능 대시보드</h1>
        <div className="animate-pulse">Loading performance data...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <h1 className="text-3xl font-bold mb-6">상세 성능 대시보드</h1>
        <div className="text-red-500">Error: {error}</div>
        <button 
          onClick={() => { setError(null); setLoading(true); fetchData(); }}
          className="mt-4 px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
        >
          재시도
        </button>
      </div>
    );
  }

  if (!data) {
    return (
      <div className="p-6">
        <h1 className="text-3xl font-bold mb-6">상세 성능 대시보드</h1>
        <div className="text-gray-500">성능 데이터를 불러올 수 없습니다.</div>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold">상세 성능 대시보드</h1>
        <div className="flex space-x-2">
          {(['1h', '6h', '24h', '7d'] as const).map((range) => (
            <button
              key={range}
              onClick={() => setTimeRange(range)}
              className={`px-3 py-1 rounded ${
                timeRange === range 
                  ? 'bg-blue-500 text-white' 
                  : 'bg-gray-200 text-gray-700 hover:bg-gray-300'
              }`}
            >
              {range}
            </button>
          ))}
        </div>
      </div>

      {/* 수익성 메트릭 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">수익성 분석</h2>
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">총 수익</h3>
            <p className="text-2xl font-bold text-green-600">
              {formatEth(data.profitability_metrics?.total_profit_eth)} ETH
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">수익 트렌드</h3>
            <p className={`text-2xl font-bold ${formatTrend(data.profitability_metrics?.profit_trend).color}`}>
              {formatTrend(data.profitability_metrics?.profit_trend).icon}
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">ROI</h3>
            <p className="text-2xl font-bold text-blue-600">
              {formatPercentage(data.profitability_metrics?.roi_percentage)}
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">손익분기점</h3>
            <p className="text-2xl font-bold text-purple-600">
              {formatEth(data.profitability_metrics?.break_even_point)} ETH
            </p>
          </div>
        </div>
        
        {/* 전략별 수익 분포 */}
        <div className="mt-6">
          <h3 className="text-lg font-medium mb-3">전략별 수익 분포</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {(data.profitability_metrics?.profit_by_strategy || []).map((strategy, index) => (
              <div key={strategy.strategy || index} className="bg-gray-50 dark:bg-gray-700 p-4 rounded">
                <h4 className="font-medium">{strategy.strategy || 'Unknown'}</h4>
                <p className="text-lg font-bold text-green-600">{formatEth(strategy.profit)} ETH</p>
                <p className="text-sm text-gray-500">{(strategy.percentage || 0).toFixed(1)}%</p>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* 전략 성능 비교 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">전략별 성능</h2>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 dark:bg-gray-700">
              <tr>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">전략</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">기회</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">성공</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">성공률</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">총 수익</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">평균 수익</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">분석 시간</th>
                <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">가스 효율</th>
              </tr>
            </thead>
            <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {(data.strategy_performance || []).map((strategy) => (
                <tr key={strategy.strategy || Math.random()} className="hover:bg-gray-50 dark:hover:bg-gray-700">
                  <td className="px-4 py-4 font-medium">{strategy.strategy || 'Unknown'}</td>
                  <td className="px-4 py-4">{(strategy.total_opportunities || 0).toLocaleString()}</td>
                  <td className="px-4 py-4">{(strategy.successful_trades || 0).toLocaleString()}</td>
                  <td className="px-4 py-4">
                    <span className={`font-semibold ${
                      (strategy.success_rate || 0) > 0.7 ? 'text-green-600' : 
                      (strategy.success_rate || 0) > 0.4 ? 'text-yellow-600' : 'text-red-600'
                    }`}>
                      {formatPercentage(strategy.success_rate)}
                    </span>
                  </td>
                  <td className="px-4 py-4 font-semibold text-green-600">
                    {formatEth(strategy.total_profit_eth)} ETH
                  </td>
                  <td className="px-4 py-4">
                    {formatEth(strategy.avg_profit_per_trade)} ETH
                  </td>
                  <td className="px-4 py-4">{strategy.avg_analysis_time_ms || 0}ms</td>
                  <td className="px-4 py-4">
                    <span className={`font-semibold ${
                      (strategy.gas_efficiency || 0) > 0.8 ? 'text-green-600' : 
                      (strategy.gas_efficiency || 0) > 0.6 ? 'text-yellow-600' : 'text-red-600'
                    }`}>
                      {formatPercentage(strategy.gas_efficiency)}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* 가스 분석 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">가스 사용량 분석</h2>
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">평균 가스 가격</h3>
            <p className="text-2xl font-bold">{data.gas_analytics?.avg_gas_price_gwei || 0} Gwei</p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">평균 가스 사용량</h3>
            <p className="text-2xl font-bold">{(data.gas_analytics?.avg_gas_used || 0).toLocaleString()}</p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">총 가스 지출</h3>
            <p className="text-2xl font-bold text-red-600">
              {formatEth(data.gas_analytics?.total_gas_spent_eth)} ETH
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">가스 효율성</h3>
            <p className={`text-2xl font-bold ${
              (data.gas_analytics?.gas_efficiency_score || 0) > 0.8 ? 'text-green-600' : 
              (data.gas_analytics?.gas_efficiency_score || 0) > 0.6 ? 'text-yellow-600' : 'text-red-600'
            }`}>
              {formatPercentage(data.gas_analytics?.gas_efficiency_score)}
            </p>
          </div>
        </div>

        {/* 전략별 가스 사용량 */}
        <div>
          <h3 className="text-lg font-medium mb-3">전략별 가스 사용량</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {(data.gas_analytics?.gas_usage_by_strategy || []).map((strategy) => (
              <div key={strategy.strategy || Math.random()} className="bg-gray-50 dark:bg-gray-700 p-4 rounded">
                <h4 className="font-medium">{strategy.strategy || 'Unknown'}</h4>
                <p className="text-lg font-bold">{(strategy.gas_used || 0).toLocaleString()}</p>
                <p className="text-sm text-gray-500">{(strategy.percentage || 0).toFixed(1)}%</p>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* 시스템 헬스 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">시스템 상태</h2>
        <div className="grid grid-cols-1 md:grid-cols-5 gap-4">
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">가동율</h3>
            <p className={`text-2xl font-bold ${
              (data.system_health?.uptime_percentage || 0) > 99 ? 'text-green-600' : 
              (data.system_health?.uptime_percentage || 0) > 95 ? 'text-yellow-600' : 'text-red-600'
            }`}>
              {formatPercentage((data.system_health?.uptime_percentage || 0) / 100)}
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">응답 시간</h3>
            <p className="text-2xl font-bold">{data.system_health?.avg_response_time_ms || 0}ms</p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">에러율</h3>
            <p className={`text-2xl font-bold ${
              (data.system_health?.error_rate || 0) < 0.01 ? 'text-green-600' : 
              (data.system_health?.error_rate || 0) < 0.05 ? 'text-yellow-600' : 'text-red-600'
            }`}>
              {formatPercentage(data.system_health?.error_rate)}
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">메모리</h3>
            <p className="text-2xl font-bold">{data.system_health?.memory_usage_mb || 0}MB</p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">CPU</h3>
            <p className="text-2xl font-bold">{formatPercentage((data.system_health?.cpu_usage_percentage || 0) / 100)}</p>
          </div>
        </div>
      </div>

      {/* 경쟁 분석 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">경쟁 분석</h2>
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">시장 점유율</h3>
            <p className="text-2xl font-bold text-blue-600">
              {formatPercentage((data.competitive_analysis?.market_share_percentage || 0) / 100)}
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">경쟁자 수</h3>
            <p className="text-2xl font-bold">{data.competitive_analysis?.competitor_count || 0}</p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">우리 성공률</h3>
            <p className="text-2xl font-bold text-green-600">
              {formatPercentage(data.competitive_analysis?.our_success_rate)}
            </p>
          </div>
          <div className="text-center">
            <h3 className="text-sm font-medium text-gray-500">시장 평균</h3>
            <p className="text-2xl font-bold text-gray-600">
              {formatPercentage(data.competitive_analysis?.market_avg_success_rate)}
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
