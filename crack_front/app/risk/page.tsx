"use client";

import { useEffect, useState } from "react";
import { 
  getRiskDashboard, 
  runStressTest, 
  updateRiskLimit,
  acknowledgeRiskEvent,
  emergencyPauseStrategy,
  type RiskDashboard, 
  type RiskLimit,
  type RiskEvent,
  type PositionRisk,
  type StressTestScenario 
} from '@/lib/api';

export default function RiskManagementPage() {
  const [risk, setRisk] = useState<RiskDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [selectedStressTest, setSelectedStressTest] = useState<string>("");
  const [runningStressTest, setRunningStressTest] = useState(false);

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      setError("");
      try {
        const riskData = await getRiskDashboard();
        setRisk(riskData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 30000); // 30초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const getRiskLevelColor = (level: string) => {
    switch (level) {
      case 'low':
      case 'safe':
        return 'text-green-600 bg-green-100';
      case 'medium':
      case 'warning':
        return 'text-yellow-600 bg-yellow-100';
      case 'high':
      case 'breached':
        return 'text-red-600 bg-red-100';
      case 'critical':
      case 'extreme':
        return 'text-red-800 bg-red-200 font-bold';
      default:
        return 'text-gray-600 bg-gray-100';
    }
  };

  const getSeverityColor = (severity: string) => {
    switch (severity) {
      case 'low': return 'bg-blue-500 text-white';
      case 'medium': return 'bg-yellow-500 text-black';
      case 'high': return 'bg-orange-500 text-white';
      case 'critical': return 'bg-red-500 text-white';
      default: return 'bg-gray-500 text-white';
    }
  };

  const formatNumber = (num: string | number) => {
    const n = typeof num === 'string' ? parseFloat(num) : num;
    if (isNaN(n) || !isFinite(n)) return '0.00';
    if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
    if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
    if (n >= 1e3) return (n / 1e3).toFixed(2) + 'K';
    return n.toFixed(2);
  };

  const safeNumber = (value: any, defaultValue: number = 0): number => {
    if (value === undefined || value === null || isNaN(Number(value))) {
      return defaultValue;
    }
    return Number(value);
  };

  const handleStressTest = async () => {
    if (!selectedStressTest) return;
    
    setRunningStressTest(true);
    try {
      const success = await runStressTest(selectedStressTest);
      if (success) {
        // 데이터 새로고침
        const riskData = await getRiskDashboard();
        setRisk(riskData);
      }
    } catch (e) {
      console.error('Stress test failed:', e);
    } finally {
      setRunningStressTest(false);
    }
  };

  const handleEmergencyPause = async (strategy: string) => {
    if (confirm(`정말로 ${strategy} 전략을 긴급 중단하시겠습니까?`)) {
      try {
        const success = await emergencyPauseStrategy(strategy);
        if (success) {
          alert(`${strategy} 전략이 긴급 중단되었습니다.`);
          // 데이터 새로고침
          const riskData = await getRiskDashboard();
          setRisk(riskData);
        }
      } catch (e) {
        console.error('Emergency pause failed:', e);
      }
    }
  };

  const handleAcknowledgeRiskEvent = async (eventId: string) => {
    try {
      const success = await acknowledgeRiskEvent(eventId);
      if (success) {
        setRisk(prev => prev ? {
          ...prev,
          recent_risk_events: prev.recent_risk_events.map(event =>
            event.id === eventId ? { ...event, resolved: true } : event
          )
        } : null);
      }
    } catch (e) {
      console.error('Failed to acknowledge risk event:', e);
    }
  };

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">리스크 관리 대시보드</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">리스크 관리 대시보드</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">리스크 관리 대시보드</h1>
        <div className="flex gap-2">
          <button 
            onClick={() => handleEmergencyPause('all')}
            className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700"
          >
            전체 긴급 중단
          </button>
        </div>
      </div>
      
      {/* 리스크 요약 */}
      {risk?.risk_summary && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">리스크 요약</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">종합 리스크 점수</h3>
              <p className={`text-2xl font-bold ${
                risk.risk_summary.overall_risk_score >= 80 ? 'text-red-600' :
                risk.risk_summary.overall_risk_score >= 60 ? 'text-yellow-600' :
                risk.risk_summary.overall_risk_score >= 40 ? 'text-blue-600' : 'text-green-600'
              }`}>
                {risk.risk_summary.overall_risk_score}/100
              </p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">위험조정수익률</h3>
              <p className="text-2xl font-bold text-green-600">
                {risk.risk_summary.risk_adjusted_return.toFixed(2)}%
              </p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">리스크 용량 사용률</h3>
              <p className={`text-2xl font-bold ${
                risk.risk_summary.risk_capacity_utilization >= 90 ? 'text-red-600' :
                risk.risk_summary.risk_capacity_utilization >= 75 ? 'text-yellow-600' : 'text-blue-600'
              }`}>
                {risk.risk_summary.risk_capacity_utilization.toFixed(1)}%
              </p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">마지막 사건 이후</h3>
              <p className="text-2xl font-bold">
                {risk.risk_summary.days_since_last_incident}일
              </p>
            </div>
            
            <div className="bg-red-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">활성 알림</h3>
              <p className="text-2xl font-bold text-red-600">
                {risk.risk_summary.active_alerts_count}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* 포트폴리오 리스크 */}
      {risk?.portfolio_risk && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">포트폴리오 리스크</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">총 노출도</h3>
              <p className="text-lg font-bold">${formatNumber(risk.portfolio_risk.total_exposure_usd)}</p>
            </div>
            
            <div className="bg-red-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">최대 일일 손실</h3>
              <p className="text-lg font-bold text-red-600">${formatNumber(risk.portfolio_risk.max_daily_loss_usd)}</p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">현재 드로우다운</h3>
              <p className="text-lg font-bold text-yellow-600">
                {safeNumber(risk.portfolio_risk?.current_drawdown_percentage).toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">최대: {safeNumber(risk.portfolio_risk?.max_drawdown_percentage).toFixed(1)}%</p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">승률</h3>
              <p className="text-lg font-bold text-green-600">
                {safeNumber(risk.portfolio_risk?.win_rate_percentage).toFixed(1)}%
              </p>
            </div>
          </div>
          
          <div className="mt-4 grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">샤프 비율</h3>
              <p className="text-lg font-bold">{formatNumber(risk.portfolio_risk.sharpe_ratio || 0)}</p>
            </div>
            
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">소르티노 비율</h3>
              <p className="text-lg font-bold">{formatNumber(risk.portfolio_risk.sortino_ratio || 0)}</p>
            </div>
            
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">평균 승/패 비율</h3>
              <p className="text-lg font-bold">{formatNumber(risk.portfolio_risk.avg_win_loss_ratio || 0)}</p>
            </div>
          </div>
        </div>
      )}

      {/* 포지션 리스크 */}
      {risk?.position_risks && risk.position_risks.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">포지션 리스크</h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">전략</th>
                  <th className="text-left py-2">토큰</th>
                  <th className="text-right py-2">포지션 크기</th>
                  <th className="text-right py-2">사용률</th>
                  <th className="text-right py-2">미실현 손익</th>
                  <th className="text-center py-2">리스크 레벨</th>
                  <th className="text-center py-2">액션</th>
                </tr>
              </thead>
              <tbody>
                {risk.position_risks.map((pos: PositionRisk, idx) => (
                  <tr key={idx} className="border-b hover:bg-gray-50">
                    <td className="py-2 font-medium">{pos.strategy}</td>
                    <td className="py-2">{pos.token_symbol}</td>
                    <td className="py-2 text-right">${formatNumber(pos.position_size_usd)}</td>
                    <td className="py-2 text-right">
                      <span className={pos.utilization_percentage >= 90 ? 'text-red-600 font-bold' : ''}>
                        {pos.utilization_percentage.toFixed(1)}%
                      </span>
                    </td>
                    <td className={`py-2 text-right font-medium ${
                      pos.unrealized_pnl_usd.startsWith('-') ? 'text-red-600' : 'text-green-600'
                    }`}>
                      ${formatNumber(pos.unrealized_pnl_usd)}
                    </td>
                    <td className="py-2 text-center">
                      <span className={`px-2 py-1 rounded-full text-xs ${getRiskLevelColor(pos.risk_level)}`}>
                        {pos.risk_level}
                      </span>
                    </td>
                    <td className="py-2 text-center">
                      {(pos.risk_level === 'high' || pos.risk_level === 'critical') && (
                        <button
                          onClick={() => handleEmergencyPause(pos.strategy)}
                          className="px-2 py-1 bg-red-600 text-white text-xs rounded hover:bg-red-700"
                        >
                          중단
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* 리스크 한도 */}
      {risk?.risk_limits && risk.risk_limits.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">리스크 한도</h2>
          <div className="space-y-4">
            {risk.risk_limits.map((limit: RiskLimit) => (
              <div key={limit.id} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <h3 className="font-medium">{limit.name}</h3>
                    <span className={`px-2 py-1 rounded-full text-xs ${getRiskLevelColor(limit.status)}`}>
                      {limit.status}
                    </span>
                  </div>
                  <div className="text-sm text-gray-500">
                    마지막 업데이트: {new Date(limit.last_updated).toLocaleString()}
                  </div>
                </div>
                
                <div className="flex items-center justify-between mb-2">
                  <div className="text-sm">
                    <span className="text-gray-600">현재: </span>
                    <span className="font-medium">{limit.current_value}</span>
                    <span className="text-gray-600"> / 한도: </span>
                    <span className="font-medium">{limit.limit_value}</span>
                  </div>
                  <div className="text-sm font-medium">
                    사용률: {limit.utilization_percentage.toFixed(1)}%
                  </div>
                </div>
                
                <div className="w-full bg-gray-200 rounded-full h-2">
                  <div 
                    className={`h-2 rounded-full ${
                      limit.utilization_percentage >= 100 ? 'bg-red-500' :
                      limit.utilization_percentage >= 80 ? 'bg-yellow-500' :
                      'bg-green-500'
                    }`}
                    style={{ width: `${Math.min(limit.utilization_percentage, 100)}%` }}
                  ></div>
                </div>
                
                {limit.auto_action && (
                  <div className="mt-2 text-xs text-gray-600">
                    자동 액션: {limit.auto_action}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 가스 리스크 */}
      {risk?.gas_risk && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">가스 리스크</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">현재 가스 가격</h3>
              <p className="text-lg font-bold">{risk.gas_risk.current_gas_price_gwei.toFixed(1)} Gwei</p>
              <p className="text-sm text-gray-600">
                최대 허용: {risk.gas_risk.max_acceptable_gas_gwei} Gwei
              </p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">가스 변동성</h3>
              <p className="text-lg font-bold">{(risk.gas_risk.gas_price_volatility * 100).toFixed(1)}%</p>
            </div>
            
            <div className="bg-red-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">일일 가스 비용</h3>
              <p className="text-lg font-bold">{formatNumber(risk.gas_risk.estimated_daily_gas_cost_eth)} ETH</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">가스/수익 비율</h3>
              <p className={`text-lg font-bold ${
                risk.gas_risk.gas_cost_vs_profit_ratio >= 0.5 ? 'text-red-600' :
                risk.gas_risk.gas_cost_vs_profit_ratio >= 0.3 ? 'text-yellow-600' : 'text-green-600'
              }`}>
                {(risk.gas_risk.gas_cost_vs_profit_ratio * 100).toFixed(1)}%
              </p>
            </div>
          </div>
        </div>
      )}

      {/* 스트레스 테스트 */}
      {risk?.stress_test_scenarios && risk.stress_test_scenarios.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold">스트레스 테스트</h2>
            <div className="flex items-center gap-2">
              <select
                value={selectedStressTest}
                onChange={(e) => setSelectedStressTest(e.target.value)}
                className="px-3 py-1 border rounded text-sm"
              >
                <option value="">시나리오 선택</option>
                {risk.stress_test_scenarios.map((scenario: StressTestScenario) => (
                  <option key={scenario.id} value={scenario.id}>
                    {scenario.name}
                  </option>
                ))}
              </select>
              <button
                onClick={handleStressTest}
                disabled={!selectedStressTest || runningStressTest}
                className="px-3 py-1 bg-orange-600 text-white text-sm rounded hover:bg-orange-700 disabled:opacity-50"
              >
                {runningStressTest ? '실행 중...' : '테스트 실행'}
              </button>
            </div>
          </div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {risk.stress_test_scenarios.map((scenario: StressTestScenario) => (
              <div key={scenario.id} className="border rounded-lg p-4">
                <h3 className="font-medium mb-2">{scenario.name}</h3>
                <p className="text-sm text-gray-600 mb-3">{scenario.description}</p>
                
                <div className="text-sm space-y-1 mb-3">
                  <div>ETH 가격 변화: <span className="font-medium">{scenario.parameters.eth_price_change > 0 ? '+' : ''}{scenario.parameters.eth_price_change}%</span></div>
                  <div>가스 가격 배수: <span className="font-medium">{scenario.parameters.gas_price_multiplier}x</span></div>
                  <div>유동성 감소: <span className="font-medium">{scenario.parameters.liquidity_reduction}%</span></div>
                  <div>변동성 배수: <span className="font-medium">{scenario.parameters.volatility_multiplier}x</span></div>
                </div>
                
                <div className="bg-gray-50 p-3 rounded text-sm">
                  <div className="grid grid-cols-2 gap-2">
                    <div>예상 손실: <span className="font-medium text-red-600">${formatNumber(scenario.results.estimated_loss_usd)}</span></div>
                    <div>위험 포지션: <span className="font-medium">{scenario.results.positions_at_risk}</span></div>
                    <div>청산 확률: <span className="font-medium">{(scenario.results.liquidation_probability * 100).toFixed(1)}%</span></div>
                    <div>회복 시간: <span className="font-medium">{scenario.results.recovery_time_hours}시간</span></div>
                  </div>
                </div>
                
                <div className="text-xs text-gray-500 mt-2">
                  마지막 실행: {new Date(scenario.last_run).toLocaleString()}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 최근 리스크 이벤트 */}
      {risk?.recent_risk_events && risk.recent_risk_events.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">최근 리스크 이벤트</h2>
          <div className="space-y-4">
            {risk.recent_risk_events.map((event: RiskEvent) => (
              <div key={event.id} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <span className={`px-2 py-1 rounded-full text-xs font-medium ${getSeverityColor(event.severity)}`}>
                      {event.severity}
                    </span>
                    <span className="text-xs text-gray-500">{event.event_type}</span>
                    <h3 className="font-medium">{event.title}</h3>
                  </div>
                  <div className="flex items-center gap-2">
                    {!event.resolved && (
                      <button
                        onClick={() => handleAcknowledgeRiskEvent(event.id)}
                        className="px-2 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700"
                      >
                        확인
                      </button>
                    )}
                    <span className={`px-2 py-1 rounded-full text-xs ${
                      event.resolved ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'
                    }`}>
                      {event.resolved ? '해결됨' : '진행중'}
                    </span>
                  </div>
                </div>
                
                <p className="text-sm text-gray-600 mb-2">{event.description}</p>
                
                <div className="text-xs text-gray-500 space-y-1">
                  <div>시간: {new Date(event.timestamp).toLocaleString()}</div>
                  <div>영향 전략: {(event.affected_strategies || []).join(', ') || 'N/A'}</div>
                  <div>영향 규모: ${formatNumber(event.impact_usd)}</div>
                  {event.auto_action_taken && (
                    <div>자동 액션: {event.auto_action_taken}</div>
                  )}
                  {event.manual_action_required && (
                    <div className="text-red-600 font-medium">수동 조치 필요</div>
                  )}
                  {event.resolution_time_minutes && (
                    <div>해결 시간: {event.resolution_time_minutes}분</div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 변동성 및 유동성 메트릭스 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* 변동성 메트릭스 */}
        {risk?.volatility_metrics && risk.volatility_metrics.length > 0 && (
          <div className="bg-white p-6 rounded-lg border">
            <h2 className="text-lg font-semibold mb-4">변동성 메트릭스</h2>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b">
                    <th className="text-left py-2">토큰</th>
                    <th className="text-right py-2">24h</th>
                    <th className="text-right py-2">7d</th>
                    <th className="text-right py-2">30d</th>
                    <th className="text-center py-2">등급</th>
                  </tr>
                </thead>
                <tbody>
                  {risk.volatility_metrics.slice(0, 10).map((vol, idx) => (
                    <tr key={idx} className="border-b hover:bg-gray-50">
                      <td className="py-2 font-medium">{vol.token_symbol}</td>
                      <td className="py-2 text-right">{(vol.volatility_24h * 100).toFixed(1)}%</td>
                      <td className="py-2 text-right">{(vol.volatility_7d * 100).toFixed(1)}%</td>
                      <td className="py-2 text-right">{(vol.volatility_30d * 100).toFixed(1)}%</td>
                      <td className="py-2 text-center">
                        <span className={`px-2 py-1 rounded-full text-xs ${getRiskLevelColor(vol.risk_rating)}`}>
                          {vol.risk_rating}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* 유동성 리스크 */}
        {risk?.liquidity_risks && risk.liquidity_risks.length > 0 && (
          <div className="bg-white p-6 rounded-lg border">
            <h2 className="text-lg font-semibold mb-4">유동성 리스크</h2>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b">
                    <th className="text-left py-2">토큰</th>
                    <th className="text-right py-2">유동성</th>
                    <th className="text-right py-2">스프레드</th>
                    <th className="text-right py-2">점수</th>
                  </tr>
                </thead>
                <tbody>
                  {risk.liquidity_risks.slice(0, 10).map((liq, idx) => (
                    <tr key={idx} className="border-b hover:bg-gray-50">
                      <td className="py-2 font-medium">{liq.token_symbol}</td>
                      <td className="py-2 text-right">${formatNumber(liq.total_liquidity_usd)}</td>
                      <td className="py-2 text-right">{liq.bid_ask_spread_bps} bps</td>
                      <td className={`py-2 text-right font-medium ${
                        liq.liquidity_score >= 80 ? 'text-green-600' :
                        liq.liquidity_score >= 60 ? 'text-yellow-600' : 'text-red-600'
                      }`}>
                        {liq.liquidity_score}/100
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </main>
  );
}