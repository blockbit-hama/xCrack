"use client";

import { useEffect, useState } from "react";
import { 
  getMicroArbitrageDashboard, 
  getMicroOpportunities, 
  getMicroTradeHistory, 
  type MicroArbitrageDashboard, 
  type ArbitrageOpportunity, 
  type MicroTradeHistory,
  type ExchangeInfo,
  type TradingPairInfo 
} from '@/lib/api';

export default function MicroArbitragePage() {
  const [dashboard, setDashboard] = useState<MicroArbitrageDashboard | null>(null);
  const [opportunities, setOpportunities] = useState<ArbitrageOpportunity[]>([]);
  const [trades, setTrades] = useState<MicroTradeHistory[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      setError("");
      try {
        const [dashboardData, opportunitiesData, tradesData] = await Promise.all([
          getMicroArbitrageDashboard(),
          getMicroOpportunities(),
          getMicroTradeHistory(20)
        ]);

        setDashboard(dashboardData);
        setOpportunities(opportunitiesData);
        setTrades(tradesData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 3000); // 3초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const getRiskColor = (level: string) => {
    switch (level) {
      case 'low': return 'text-green-600';
      case 'medium': return 'text-yellow-600';
      case 'high': return 'text-red-600';
      default: return 'text-gray-600';
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'success': return 'text-green-600 bg-green-100';
      case 'failed': return 'text-red-600 bg-red-100';
      case 'partial': return 'text-yellow-600 bg-yellow-100';
      default: return 'text-gray-600 bg-gray-100';
    }
  };

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로아비트래지 대시보드</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로아비트래지 대시보드</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  const metrics = dashboard?.metrics;

  return (
    <main className="p-6 space-y-6">
      <h1 className="text-2xl font-bold">마이크로아비트래지 대시보드</h1>
      
      {/* 핵심 메트릭스 */}
      {metrics && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div className="bg-white p-4 rounded-lg border">
            <h3 className="text-sm font-medium text-gray-500">오늘 총 거래</h3>
            <p className="text-2xl font-bold">{metrics.total_trades_today}</p>
            <p className="text-sm text-gray-600">성공: {metrics.successful_trades_today}</p>
          </div>
          
          <div className="bg-white p-4 rounded-lg border">
            <h3 className="text-sm font-medium text-gray-500">오늘 총 수익</h3>
            <p className="text-2xl font-bold text-green-600">${metrics.total_profit_today_usd}</p>
            <p className="text-sm text-gray-600">평균: ${metrics.avg_profit_per_trade}</p>
          </div>
          
          <div className="bg-white p-4 rounded-lg border">
            <h3 className="text-sm font-medium text-gray-500">성공률</h3>
            <p className="text-2xl font-bold">{metrics.success_rate_percentage.toFixed(1)}%</p>
            <p className="text-sm text-gray-600">실행시간: {metrics.avg_execution_time_ms}ms</p>
          </div>
          
          <div className="bg-white p-4 rounded-lg border">
            <h3 className="text-sm font-medium text-gray-500">활성 기회</h3>
            <p className="text-2xl font-bold text-blue-600">{metrics.active_opportunities}</p>
            <p className="text-sm text-gray-600">모니터링 페어: {metrics.monitored_pairs}</p>
          </div>
        </div>
      )}

      {/* 거래소 연결 상태 */}
      {dashboard?.exchanges && dashboard.exchanges.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">거래소 연결 상태</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {dashboard.exchanges.map((exchange: ExchangeInfo, idx) => (
              <div key={idx} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium">{exchange.name}</h3>
                  <span className={`px-2 py-1 rounded-full text-xs ${
                    exchange.connected ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'
                  }`}>
                    {exchange.connected ? '연결됨' : '연결끊김'}
                  </span>
                </div>
                <div className="text-sm space-y-1">
                  <div>타입: <span className="font-medium">{exchange.type}</span></div>
                  <div>핑: <span className="font-medium">{exchange.last_ping_ms}ms</span></div>
                  <div>수수료: <span className="font-medium">{exchange.trading_fee_percentage}%</span></div>
                  <div>신뢰도: <span className="font-medium">{(exchange.reliability_score * 100).toFixed(1)}%</span></div>
                  <div>24h 볼륨: <span className="font-medium">${exchange.volume_24h}</span></div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 활성 아비트래지 기회 */}
      <div className="bg-white p-6 rounded-lg border">
        <h2 className="text-lg font-semibold mb-4">활성 아비트래지 기회</h2>
        {opportunities.length === 0 ? (
          <div className="text-gray-500">현재 활성 기회가 없습니다.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">페어</th>
                  <th className="text-left py-2">매수 거래소</th>
                  <th className="text-left py-2">매도 거래소</th>
                  <th className="text-right py-2">스프레드</th>
                  <th className="text-right py-2">예상 수익</th>
                  <th className="text-center py-2">위험도</th>
                  <th className="text-center py-2">신뢰도</th>
                  <th className="text-right py-2">실행시간</th>
                </tr>
              </thead>
              <tbody>
                {opportunities.slice(0, 10).map((opp) => (
                  <tr key={opp.id} className="border-b hover:bg-gray-50">
                    <td className="py-2 font-medium">{opp.pair}</td>
                    <td className="py-2">{opp.buy_exchange}</td>
                    <td className="py-2">{opp.sell_exchange}</td>
                    <td className="py-2 text-right font-medium text-green-600">
                      {opp.spread_percentage.toFixed(3)}%
                    </td>
                    <td className="py-2 text-right font-medium">
                      ${opp.potential_profit_usd}
                    </td>
                    <td className="py-2 text-center">
                      <span className={`px-2 py-1 rounded-full text-xs ${getRiskColor(opp.risk_level)} bg-gray-100`}>
                        {opp.risk_level}
                      </span>
                    </td>
                    <td className="py-2 text-center">{(opp.confidence_score * 100).toFixed(0)}%</td>
                    <td className="py-2 text-right">{opp.execution_time_estimate_ms}ms</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 거래 페어 정보 */}
      {dashboard?.trading_pairs && dashboard.trading_pairs.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">모니터링 중인 거래 페어</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {dashboard.trading_pairs.map((pair: TradingPairInfo, idx) => (
              <div key={idx} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium">{pair.pair}</h3>
                  <span className={`px-2 py-1 rounded-full text-xs ${
                    pair.is_active ? 'bg-green-100 text-green-800' : 'bg-gray-100 text-gray-800'
                  }`}>
                    {pair.is_active ? '활성' : '비활성'}
                  </span>
                </div>
                <div className="text-sm space-y-1">
                  <div>베이스: <span className="font-medium">{pair.base_token}</span></div>
                  <div>쿼트: <span className="font-medium">{pair.quote_token}</span></div>
                  <div>스프레드: <span className="font-medium text-green-600">{pair.spread_percentage.toFixed(3)}%</span></div>
                  <div>24h 볼륨: <span className="font-medium">${pair.volume_24h}</span></div>
                  <div>거래소: <span className="font-medium">{pair.exchanges.join(', ')}</span></div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 최근 거래 기록 */}
      <div className="bg-white p-6 rounded-lg border">
        <h2 className="text-lg font-semibold mb-4">최근 거래 기록</h2>
        {trades.length === 0 ? (
          <div className="text-gray-500">최근 거래가 없습니다.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">시간</th>
                  <th className="text-left py-2">페어</th>
                  <th className="text-left py-2">매수/매도</th>
                  <th className="text-right py-2">수익</th>
                  <th className="text-right py-2">가스비</th>
                  <th className="text-center py-2">상태</th>
                  <th className="text-right py-2">실행시간</th>
                </tr>
              </thead>
              <tbody>
                {trades.map((trade) => (
                  <tr key={trade.id} className="border-b hover:bg-gray-50">
                    <td className="py-2">{new Date(trade.timestamp).toLocaleString()}</td>
                    <td className="py-2 font-medium">{trade.pair}</td>
                    <td className="py-2">
                      <div className="text-xs">
                        <div>매수: {trade.buy_exchange}</div>
                        <div>매도: {trade.sell_exchange}</div>
                      </div>
                    </td>
                    <td className="py-2 text-right font-medium">
                      <span className={trade.profit_usd.startsWith('-') ? 'text-red-600' : 'text-green-600'}>
                        ${trade.profit_usd}
                      </span>
                    </td>
                    <td className="py-2 text-right">${trade.gas_cost}</td>
                    <td className="py-2 text-center">
                      <span className={`px-2 py-1 rounded-full text-xs ${getStatusColor(trade.status)}`}>
                        {trade.status}
                      </span>
                    </td>
                    <td className="py-2 text-right">{trade.execution_time_ms}ms</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 리스크 분석 */}
      {dashboard?.risk_analysis && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">리스크 분석</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">현재 노출도</h3>
              <p className="text-lg font-bold">${dashboard.risk_analysis.current_exposure_usd}</p>
              <p className="text-sm text-gray-600">
                최대: ${dashboard.risk_analysis.max_exposure_usd}
              </p>
            </div>
            
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">분산화 점수</h3>
              <p className="text-lg font-bold">
                {(dashboard.risk_analysis.diversification_score * 100).toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">높을수록 좋음</p>
            </div>
            
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">유동성 위험</h3>
              <p className="text-lg font-bold text-yellow-600">
                {(dashboard.risk_analysis.liquidity_risk * 100).toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">낮을수록 좋음</p>
            </div>
          </div>
        </div>
      )}

      {/* 성능 지표 */}
      {metrics && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">성능 지표</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">포지션 사용률</h3>
              <p className="text-lg font-bold">{(metrics.position_size_utilization * 100).toFixed(1)}%</p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">일일 볼륨 사용률</h3>
              <p className="text-lg font-bold">{(metrics.daily_volume_limit_used * 100).toFixed(1)}%</p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">리스크 한도 사용률</h3>
              <p className="text-lg font-bold">{(metrics.risk_limit_used * 100).toFixed(1)}%</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">연결된 거래소</h3>
              <p className="text-lg font-bold">{metrics.connected_exchanges}</p>
            </div>
          </div>
          
          {(metrics.best_performing_pair || metrics.worst_performing_pair) && (
            <div className="mt-4 grid grid-cols-1 md:grid-cols-2 gap-4">
              {metrics.best_performing_pair && (
                <div className="bg-green-50 p-4 rounded-lg">
                  <h3 className="text-sm font-medium text-green-700 mb-1">최고 성과 페어</h3>
                  <p className="text-lg font-bold text-green-800">{metrics.best_performing_pair}</p>
                </div>
              )}
              
              {metrics.worst_performing_pair && (
                <div className="bg-red-50 p-4 rounded-lg">
                  <h3 className="text-sm font-medium text-red-700 mb-1">최저 성과 페어</h3>
                  <p className="text-lg font-bold text-red-800">{metrics.worst_performing_pair}</p>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </main>
  );
}