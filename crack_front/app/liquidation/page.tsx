import { getLiquidationDashboard, getProtocolStatus, getLiquidationOpportunities } from "../../lib/api";

export const dynamic = 'force-dynamic';

export default async function LiquidationPage() {
  const [dashboard, protocolStatus, opportunities] = await Promise.all([
    getLiquidationDashboard().catch(() => null),
    getProtocolStatus().catch(() => []),
    getLiquidationOpportunities().catch(() => []),
  ]);

  const metrics = dashboard?.metrics || {
    total_opportunities: 0,
    successful_liquidations: 0,
    failed_liquidations: 0,
    success_rate: 0,
    total_profit_usd: '0',
    avg_profit_per_liquidation: '0',
    avg_execution_time_ms: 0,
    flashloan_usage_rate: 0,
    wallet_mode_usage_rate: 0,
    auto_mode_decisions: {
      flashloan_selected: 0,
      wallet_selected: 0,
      total_decisions: 0,
    },
  };

  return (
    <main className="space-y-6">
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-2xl font-bold">청산 전략 v2.0</h1>
        <p className="text-gray-600 mt-1">실시간 프로토콜 포지션 스캐닝 및 지능형 자금 조달</p>
      </div>

      {/* 주요 메트릭 */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">총 기회</h3>
          <p className="text-2xl font-bold">{metrics.total_opportunities}</p>
        </div>
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">성공률</h3>
          <p className="text-2xl font-bold text-green-600">{(metrics.success_rate * 100).toFixed(1)}%</p>
        </div>
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">총 수익</h3>
          <p className="text-2xl font-bold text-blue-600">${parseFloat(metrics.total_profit_usd).toFixed(2)}</p>
        </div>
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">평균 실행시간</h3>
          <p className="text-2xl font-bold">{metrics.avg_execution_time_ms.toFixed(0)}ms</p>
        </div>
      </div>

      {/* 프로토콜 상태 */}
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-4">프로토콜 스캐너 상태</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {protocolStatus.map((protocol) => (
            <div key={protocol.protocol} className="bg-gray-50 rounded-lg p-4">
              <div className="flex justify-between items-start mb-2">
                <h4 className="font-medium capitalize">{protocol.protocol}</h4>
                <span className={`px-2 py-1 rounded text-xs ${
                  protocol.connected 
                    ? 'bg-green-100 text-green-800' 
                    : 'bg-red-100 text-red-800'
                }`}>
                  {protocol.connected ? '연결됨' : '연결 안됨'}
                </span>
              </div>
              <div className="text-sm space-y-1">
                <div className="flex justify-between">
                  <span>모니터링 사용자:</span>
                  <span className="font-medium">{protocol.total_users.toLocaleString()}</span>
                </div>
                <div className="flex justify-between">
                  <span>청산 위험:</span>
                  <span className="font-medium text-red-600">{protocol.liquidatable_positions}</span>
                </div>
                <div className="flex justify-between">
                  <span>총 담보:</span>
                  <span className="font-medium">${parseFloat(protocol.total_collateral_usd).toFixed(0)}</span>
                </div>
                <div className="flex justify-between">
                  <span>평균 건강도:</span>
                  <span className="font-medium">{protocol.avg_health_factor.toFixed(3)}</span>
                </div>
                <div className="flex justify-between">
                  <span>스캔 지연시간:</span>
                  <span className="font-medium">{protocol.scan_latency_ms}ms</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* 자금 조달 모드 분석 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-4">자금 조달 모드 사용률</h3>
          <div className="space-y-3">
            <div>
              <div className="flex justify-between mb-1">
                <span>플래시론 모드</span>
                <span>{(metrics.flashloan_usage_rate * 100).toFixed(1)}%</span>
              </div>
              <div className="w-full bg-gray-200 rounded-full h-2">
                <div 
                  className="bg-blue-600 h-2 rounded-full" 
                  style={{ width: `${metrics.flashloan_usage_rate * 100}%` }}
                ></div>
              </div>
            </div>
            <div>
              <div className="flex justify-between mb-1">
                <span>지갑 모드</span>
                <span>{(metrics.wallet_mode_usage_rate * 100).toFixed(1)}%</span>
              </div>
              <div className="w-full bg-gray-200 rounded-full h-2">
                <div 
                  className="bg-green-600 h-2 rounded-full" 
                  style={{ width: `${metrics.wallet_mode_usage_rate * 100}%` }}
                ></div>
              </div>
            </div>
          </div>
        </div>

        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-4">자동 모드 결정 분석</h3>
          <div className="text-sm space-y-2">
            <div className="flex justify-between">
              <span>플래시론 선택:</span>
              <span className="font-medium">{metrics.auto_mode_decisions.flashloan_selected}</span>
            </div>
            <div className="flex justify-between">
              <span>지갑 선택:</span>
              <span className="font-medium">{metrics.auto_mode_decisions.wallet_selected}</span>
            </div>
            <div className="flex justify-between border-t pt-2">
              <span>총 결정:</span>
              <span className="font-medium">{metrics.auto_mode_decisions.total_decisions}</span>
            </div>
          </div>
          {dashboard?.profitability_analysis && (
            <div className="mt-4 pt-4 border-t">
              <h4 className="font-medium mb-2">수익성 비교</h4>
              <div className="text-xs space-y-1">
                <div className="flex justify-between">
                  <span>플래시론 평균 수익:</span>
                  <span>${parseFloat(dashboard.profitability_analysis.flashloan_vs_wallet.flashloan_avg_profit).toFixed(2)}</span>
                </div>
                <div className="flex justify-between">
                  <span>지갑 평균 수익:</span>
                  <span>${parseFloat(dashboard.profitability_analysis.flashloan_vs_wallet.wallet_avg_profit).toFixed(2)}</span>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* 활성 청산 기회 */}
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-4">활성 청산 기회 ({opportunities.length})</h3>
        {opportunities.length === 0 ? (
          <p className="text-gray-500 text-center py-4">현재 활성 청산 기회가 없습니다</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full border-collapse text-sm">
              <thead>
                <tr className="text-left border-b">
                  <th className="p-2">프로토콜</th>
                  <th className="p-2">사용자</th>
                  <th className="p-2">담보/부채</th>
                  <th className="p-2">건강도</th>
                  <th className="p-2 text-right">예상 수익</th>
                  <th className="p-2">자금조달</th>
                  <th className="p-2">신뢰도</th>
                  <th className="p-2">만료</th>
                </tr>
              </thead>
              <tbody>
                {opportunities.slice(0, 10).map((opp) => (
                  <tr key={opp.id} className="border-b hover:bg-gray-50">
                    <td className="p-2">
                      <span className="capitalize font-medium">{opp.protocol}</span>
                    </td>
                    <td className="p-2 font-mono text-xs">
                      {opp.user_address.slice(0, 6)}...{opp.user_address.slice(-4)}
                    </td>
                    <td className="p-2">
                      <div className="text-xs">
                        <div>{opp.collateral_token}/{opp.debt_token}</div>
                        <div className="text-gray-500">
                          ${parseFloat(opp.collateral_amount).toFixed(0)}
                        </div>
                      </div>
                    </td>
                    <td className="p-2">
                      <span className={`font-medium ${
                        opp.health_factor < 1.0 
                          ? 'text-red-600' 
                          : opp.health_factor < 1.1 
                            ? 'text-orange-600' 
                            : 'text-green-600'
                      }`}>
                        {opp.health_factor.toFixed(3)}
                      </span>
                    </td>
                    <td className="p-2 text-right font-medium text-green-600">
                      ${parseFloat(opp.expected_profit).toFixed(2)}
                    </td>
                    <td className="p-2">
                      <span className={`px-2 py-1 rounded text-xs ${
                        opp.funding_mode === 'flashloan' 
                          ? 'bg-blue-100 text-blue-800' 
                          : 'bg-green-100 text-green-800'
                      }`}>
                        {opp.funding_mode === 'flashloan' ? '플래시론' : '지갑'}
                      </span>
                    </td>
                    <td className="p-2">
                      <div className="flex items-center">
                        <div className={`w-2 h-2 rounded-full mr-1 ${
                          opp.confidence_score >= 0.8 
                            ? 'bg-green-400' 
                            : opp.confidence_score >= 0.6 
                              ? 'bg-yellow-400' 
                              : 'bg-red-400'
                        }`}></div>
                        <span className="text-xs">{(opp.confidence_score * 100).toFixed(0)}%</span>
                      </div>
                    </td>
                    <td className="p-2 text-xs text-gray-500">
                      {new Date(opp.expires_at).toLocaleTimeString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 최근 청산 실행 */}
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-4">최근 청산 실행</h3>
        {!dashboard?.recent_liquidations || dashboard.recent_liquidations.length === 0 ? (
          <p className="text-gray-500 text-center py-4">최근 청산 실행 내역이 없습니다</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full border-collapse text-sm">
              <thead>
                <tr className="text-left border-b">
                  <th className="p-2">시간</th>
                  <th className="p-2">프로토콜</th>
                  <th className="p-2">사용자</th>
                  <th className="p-2 text-right">수익</th>
                  <th className="p-2">실행시간</th>
                  <th className="p-2">자금조달</th>
                  <th className="p-2">상태</th>
                </tr>
              </thead>
              <tbody>
                {dashboard.recent_liquidations.slice(0, 10).map((liq) => (
                  <tr key={liq.id} className="border-b hover:bg-gray-50">
                    <td className="p-2 text-xs">
                      {new Date(liq.timestamp).toLocaleString()}
                    </td>
                    <td className="p-2 capitalize">{liq.protocol}</td>
                    <td className="p-2 font-mono text-xs">
                      {liq.user_address.slice(0, 6)}...{liq.user_address.slice(-4)}
                    </td>
                    <td className="p-2 text-right font-medium text-green-600">
                      ${parseFloat(liq.profit_usd).toFixed(2)}
                    </td>
                    <td className="p-2">{liq.execution_time_ms}ms</td>
                    <td className="p-2">
                      <span className={`px-2 py-1 rounded text-xs ${
                        liq.funding_mode === 'flashloan' 
                          ? 'bg-blue-100 text-blue-800' 
                          : 'bg-green-100 text-green-800'
                      }`}>
                        {liq.funding_mode === 'flashloan' ? '플래시론' : '지갑'}
                      </span>
                    </td>
                    <td className="p-2">
                      <span className={`px-2 py-1 rounded text-xs ${
                        liq.status === 'success' 
                          ? 'bg-green-100 text-green-800' 
                          : 'bg-red-100 text-red-800'
                      }`}>
                        {liq.status === 'success' ? '성공' : '실패'}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </main>
  );
}