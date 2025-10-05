"use client"

import { useState } from 'react'
import { useWebSocket } from '../../lib/hooks/use-websocket'

interface LiquidationProps {
  initialDashboard: any
  initialProtocolStatus: any[]
  initialOpportunities: any[]
}

export function LiquidationClient({ initialDashboard, initialProtocolStatus, initialOpportunities }: LiquidationProps) {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'opportunities' | 'history' | 'settings'>('dashboard')
  const [dashboard] = useState(initialDashboard)
  const [protocolStatus] = useState(initialProtocolStatus)
  const [opportunities] = useState(initialOpportunities)

  // Settings state
  const [settings, setSettings] = useState({
    min_profit_threshold_usd: 100,
    scan_interval_seconds: 10,
    max_concurrent_liquidations: 3,
    use_flashloan: true,
    preferred_flashloan_provider: 'aave_v3' as string,
    gas_price_gwei: 30,
    gas_multiplier: 1.2,
    auto_execute: false,
  })

  const handleSettingsChange = (key: string, value: any) => {
    setSettings(prev => ({ ...prev, [key]: value }))
  }

  const saveSettings = async () => {
    try {
      const response = await fetch('http://localhost:8080/api/liquidation/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(settings)
      })
      const data = await response.json()
      alert(data.message || '설정이 저장되었습니다')
    } catch (error) {
      alert('설정 저장 실패: ' + error)
    }
  }

  const metrics = dashboard || {
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
  }

  return (
    <div className="space-y-6">
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-2xl font-bold">청산 전략 v2.0 통합 대시보드</h1>
        <p className="text-gray-600 mt-1">실시간 프로토콜 포지션 스캐닝 및 지능형 자금 조달</p>
      </div>

      {/* 탭 네비게이션 */}
      <div className="border-b">
        <nav className="flex space-x-4">
          {['dashboard', 'opportunities', 'history', 'settings'].map(tab => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab as any)}
              className={`pb-2 px-1 ${
                activeTab === tab
                  ? 'border-b-2 border-blue-600 text-blue-600 font-medium'
                  : 'text-gray-600 hover:text-gray-900'
              }`}
            >
              {tab === 'dashboard' && '📊 대시보드'}
              {tab === 'opportunities' && '💡 청산 기회'}
              {tab === 'history' && '📜 실행 내역'}
              {tab === 'settings' && '⚙️ 설정'}
            </button>
          ))}
        </nav>
      </div>

      {/* 대시보드 탭 */}
      {activeTab === 'dashboard' && (
        <div className="space-y-6">
          {/* 주요 메트릭 */}
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-4">
            <div className="border rounded-lg p-4 bg-gradient-to-br from-blue-50 to-blue-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">총 청산</h3>
              <p className="text-3xl font-bold text-blue-600">{metrics.total_liquidations}</p>
              <p className="text-xs text-gray-600 mt-1">누적 실행 횟수</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-green-50 to-green-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">성공률</h3>
              <p className="text-3xl font-bold text-green-600">{(metrics.success_rate * 100).toFixed(1)}%</p>
              <p className="text-xs text-gray-600 mt-1">실행 성공률</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-purple-50 to-purple-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">총 수익</h3>
              <p className="text-3xl font-bold text-purple-600">${parseFloat(metrics.total_profit).toFixed(2)}</p>
              <p className="text-xs text-gray-600 mt-1">실현 수익</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-orange-50 to-orange-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">활성 포지션</h3>
              <p className="text-3xl font-bold text-orange-600">{metrics.active_positions}</p>
              <p className="text-xs text-gray-600 mt-1">모니터링 중</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-red-50 to-red-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">대기 중</h3>
              <p className="text-3xl font-bold text-red-600">{metrics.pending_executions}</p>
              <p className="text-xs text-gray-600 mt-1">실행 대기</p>
            </div>
          </div>

          {/* 성능 메트릭 */}
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-4">성능 메트릭</h3>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div className="bg-gray-50 rounded-lg p-3">
                <p className="text-sm text-gray-600">평균 실행 시간</p>
                <p className="text-2xl font-bold">{metrics.performance_metrics.avg_execution_time_ms.toFixed(1)}ms</p>
              </div>
              <div className="bg-gray-50 rounded-lg p-3">
                <p className="text-sm text-gray-600">가동 시간</p>
                <p className="text-2xl font-bold">{(metrics.performance_metrics.uptime_seconds / 3600).toFixed(1)}h</p>
              </div>
              <div className="bg-gray-50 rounded-lg p-3">
                <p className="text-sm text-gray-600">실행 성공률</p>
                <p className="text-2xl font-bold text-green-600">{(metrics.performance_metrics.execution_success_rate * 100).toFixed(1)}%</p>
              </div>
            </div>
          </div>

          {/* 프로토콜 상태 */}
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-4">프로토콜 스캐너 상태</h3>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              {protocolStatus.map((protocol: any, index: number) => (
                <div key={index} className="bg-gray-50 rounded-lg p-4 hover:shadow-md transition-shadow">
                  <div className="flex justify-between items-start mb-3">
                    <h4 className="font-medium text-lg">{protocol.protocol}</h4>
                    <span className={`px-2 py-1 rounded text-xs font-medium ${
                      protocol.status === 'active'
                        ? 'bg-green-100 text-green-800'
                        : 'bg-red-100 text-red-800'
                    }`}>
                      {protocol.status}
                    </span>
                  </div>
                  <div className="space-y-2 text-sm">
                    <div className="flex justify-between">
                      <span className="text-gray-600">모니터링 사용자:</span>
                      <span className="font-medium">{protocol.users_monitored?.toLocaleString() || 0}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">총 TVL:</span>
                      <span className="font-medium text-blue-600">{protocol.total_tvl}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">청산 가능:</span>
                      <span className="font-medium text-orange-600">{protocol.liquidatable_positions}</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">마지막 업데이트:</span>
                      <span className="font-medium text-xs">
                        {new Date(protocol.last_update * 1000).toLocaleTimeString()}
                      </span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* 청산 기회 탭 */}
      {activeTab === 'opportunities' && (
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-4">활성 청산 기회 ({opportunities.length})</h3>
          {opportunities.length === 0 ? (
            <div className="text-center py-12">
              <div className="text-6xl mb-4">💤</div>
              <p className="text-gray-500">현재 활성 청산 기회가 없습니다</p>
              <p className="text-sm text-gray-400 mt-2">시스템이 지속적으로 모니터링 중입니다</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full border-collapse text-sm">
                <thead className="bg-gray-50">
                  <tr className="text-left border-b-2">
                    <th className="p-3">프로토콜</th>
                    <th className="p-3">포지션</th>
                    <th className="p-3">담보</th>
                    <th className="p-3">부채</th>
                    <th className="p-3">청산 임계값</th>
                    <th className="p-3">건강도</th>
                    <th className="p-3 text-right">예상 수익</th>
                    <th className="p-3 text-right">실행 비용</th>
                    <th className="p-3">시간</th>
                    <th className="p-3">액션</th>
                  </tr>
                </thead>
                <tbody>
                  {opportunities.map((opp: any) => (
                    <tr key={opp.id} className="border-b hover:bg-blue-50 transition-colors">
                      <td className="p-3">
                        <span className="px-2 py-1 bg-blue-100 text-blue-800 rounded text-xs font-medium">
                          {opp.protocol}
                        </span>
                      </td>
                      <td className="p-3 font-mono text-xs">{opp.position}</td>
                      <td className="p-3 font-medium">{opp.collateral}</td>
                      <td className="p-3 font-medium">{opp.debt}</td>
                      <td className="p-3">{opp.liquidation_threshold}</td>
                      <td className="p-3">
                        <span className={`font-bold ${
                          opp.health_factor < 1.0
                            ? 'text-red-600'
                            : opp.health_factor < 1.1
                              ? 'text-orange-600'
                              : 'text-green-600'
                        }`}>
                          {opp.health_factor.toFixed(3)}
                        </span>
                      </td>
                      <td className="p-3 text-right font-bold text-green-600">{opp.estimated_profit}</td>
                      <td className="p-3 text-right text-gray-600">{opp.execution_cost}</td>
                      <td className="p-3 text-xs text-gray-500">
                        {new Date(opp.timestamp * 1000).toLocaleTimeString()}
                      </td>
                      <td className="p-3">
                        <button className="px-3 py-1 bg-blue-600 text-white rounded hover:bg-blue-700 text-xs">
                          실행
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* 실행 내역 탭 */}
      {activeTab === 'history' && (
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-4">최근 청산 실행</h3>
          <div className="text-center py-12">
            <div className="text-6xl mb-4">📜</div>
            <p className="text-gray-500">최근 청산 실행 내역이 없습니다</p>
            <p className="text-sm text-gray-400 mt-2">청산 실행 시 여기에 표시됩니다</p>
          </div>
        </div>
      )}

      {/* 설정 탭 */}
      {activeTab === 'settings' && (
        <div className="space-y-6">
          <div className="border rounded-lg p-6">
            <h3 className="font-semibold mb-6 text-lg">청산 전략 설정</h3>

            <div className="space-y-6">
              {/* 수익 임계값 */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium mb-2">최소 수익 임계값 (USD)</label>
                  <input
                    type="number"
                    value={settings.min_profit_threshold_usd}
                    onChange={(e) => handleSettingsChange('min_profit_threshold_usd', parseFloat(e.target.value))}
                    className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                  />
                  <p className="text-xs text-gray-500 mt-1">이 금액 이하의 청산 기회는 무시됩니다</p>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">스캔 간격 (초)</label>
                  <input
                    type="number"
                    value={settings.scan_interval_seconds}
                    onChange={(e) => handleSettingsChange('scan_interval_seconds', parseInt(e.target.value))}
                    className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                  />
                  <p className="text-xs text-gray-500 mt-1">프로토콜 스캔 주기</p>
                </div>
              </div>

              {/* 동시 실행 제한 */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium mb-2">최대 동시 청산 수</label>
                  <input
                    type="number"
                    value={settings.max_concurrent_liquidations}
                    onChange={(e) => handleSettingsChange('max_concurrent_liquidations', parseInt(e.target.value))}
                    className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                  />
                  <p className="text-xs text-gray-500 mt-1">동시에 실행 가능한 청산 개수</p>
                </div>

                <div>
                  <label className="block text-sm font-medium mb-2">가스 가격 (Gwei)</label>
                  <input
                    type="number"
                    value={settings.gas_price_gwei}
                    onChange={(e) => handleSettingsChange('gas_price_gwei', parseFloat(e.target.value))}
                    className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                  />
                  <p className="text-xs text-gray-500 mt-1">기본 가스 가격 설정</p>
                </div>
              </div>

              {/* 플래시론 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4">플래시론 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="flex items-center space-x-2">
                      <input
                        type="checkbox"
                        checked={settings.use_flashloan}
                        onChange={(e) => handleSettingsChange('use_flashloan', e.target.checked)}
                        className="w-4 h-4"
                      />
                      <span className="text-sm font-medium">플래시론 사용</span>
                    </label>
                    <p className="text-xs text-gray-500 mt-1 ml-6">플래시론을 이용한 무자본 청산</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">선호 플래시론 제공자</label>
                    <select
                      value={settings.preferred_flashloan_provider}
                      onChange={(e) => handleSettingsChange('preferred_flashloan_provider', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      disabled={!settings.use_flashloan}
                    >
                      <option value="aave_v3">Aave V3</option>
                      <option value="aave_v2">Aave V2</option>
                      <option value="balancer">Balancer</option>
                    </select>
                  </div>
                </div>
              </div>

              {/* 실행 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4">실행 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">가스 배수</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.gas_multiplier}
                      onChange={(e) => handleSettingsChange('gas_multiplier', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">가스 가격 배수 (1.0 = 기본값)</p>
                  </div>

                  <div>
                    <label className="flex items-center space-x-2">
                      <input
                        type="checkbox"
                        checked={settings.auto_execute}
                        onChange={(e) => handleSettingsChange('auto_execute', e.target.checked)}
                        className="w-4 h-4"
                      />
                      <span className="text-sm font-medium">자동 실행</span>
                    </label>
                    <p className="text-xs text-gray-500 mt-1 ml-6">조건 충족 시 자동으로 청산 실행</p>
                  </div>
                </div>
              </div>

              {/* 저장 버튼 */}
              <div className="border-t pt-6 flex justify-end space-x-4">
                <button
                  onClick={() => {
                    setSettings({
                      min_profit_threshold_usd: 100,
                      scan_interval_seconds: 10,
                      max_concurrent_liquidations: 3,
                      use_flashloan: true,
                      preferred_flashloan_provider: 'aave_v3',
                      gas_price_gwei: 30,
                      gas_multiplier: 1.2,
                      auto_execute: false,
                    })
                  }}
                  className="px-6 py-2 border rounded hover:bg-gray-50"
                >
                  초기화
                </button>
                <button
                  onClick={saveSettings}
                  className="px-6 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
                >
                  설정 저장
                </button>
              </div>
            </div>
          </div>

          {/* 현재 설정 미리보기 */}
          <div className="border rounded-lg p-6 bg-gray-50">
            <h3 className="font-semibold mb-4">현재 설정 미리보기</h3>
            <pre className="text-xs overflow-auto p-4 bg-white rounded border">
              {JSON.stringify(settings, null, 2)}
            </pre>
          </div>
        </div>
      )}
    </div>
  )
}
