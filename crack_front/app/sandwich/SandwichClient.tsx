"use client"

import { useState, useEffect } from 'react'
import { useWebSocket } from '../../lib/hooks/use-websocket'
import { 
  startSandwichStrategy, 
  stopSandwichStrategy, 
  getSandwichStatus, 
  getSandwichConfig, 
  updateSandwichConfig 
} from '../../lib/api'
import { Card, CardContent, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Play, Square, Settings, Activity, DollarSign, Clock, AlertTriangle, Target, Zap, TrendingUp } from 'lucide-react'

interface SandwichProps {
  initialDashboard: any
  initialStatus: any
  initialConfig: any
  initialOpportunities: any[]
}

export function SandwichClient({ initialDashboard, initialStatus, initialConfig, initialOpportunities }: SandwichProps) {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'opportunities' | 'history' | 'settings'>('dashboard')
  const [dashboard] = useState(initialDashboard)
  const [status] = useState(initialStatus)
  const [opportunities] = useState(initialOpportunities)

  // 샌드위치 전략 상태
  const [isRunning, setIsRunning] = useState(false)
  const [uptime, setUptime] = useState(0)
  const [lastScan, setLastScan] = useState('')
  const [isLoading, setIsLoading] = useState(false)

  // Settings state
  const [settings, setSettings] = useState({
    // 기본 설정
    min_value_eth: 0.1,
    max_gas_price_gwei: 200,
    min_profit_eth: 0.01,
    min_profit_percentage: 0.02,
    max_price_impact: 0.05,
    kelly_risk_factor: 0.5,
    
    // 컨트랙트 설정
    contract_address: '0x0000000000000000000000000000000000000000',
    flashbots_relay_url: 'https://relay.flashbots.net',
    
    // 가스 설정
    gas_limit: 200000,
    gas_per_tx: 200000,
    front_run_priority_fee_gwei: 5,
    back_run_priority_fee_gwei: 2,
    
    // 경쟁 수준별 우선순위 수수료
    priority_fee_low_gwei: 1,
    priority_fee_medium_gwei: 2,
    priority_fee_high_gwei: 5,
    priority_fee_critical_gwei: 10,
    
    // DEX 수수료 설정
    uniswap_v2_fee: 0.003,
    uniswap_v3_fee: 0.003,
    default_fee: 0.003,
    uniswap_v3_fee_tier: 3000,
    
    // 타이밍 설정
    deadline_secs: 300,
    max_wait_blocks: 3,
    wait_seconds: 3,
    stats_interval_secs: 60,
  })

  // 샌드위치 전략 상태 로드
  useEffect(() => {
    loadSandwichStatus()
    loadSandwichConfig()
  }, [])

  // 실시간 상태 업데이트 (5초마다)
  useEffect(() => {
    if (isRunning) {
      const interval = setInterval(() => {
        loadSandwichStatus()
      }, 5000)
      return () => clearInterval(interval)
    }
  }, [isRunning])

  const loadSandwichStatus = async () => {
    try {
      const status = await getSandwichStatus()
      setIsRunning(status.is_running)
      setUptime(status.uptime_seconds)
      setLastScan(status.last_scan)
    } catch (error) {
      console.error('샌드위치 상태 로드 실패:', error)
    }
  }

  const loadSandwichConfig = async () => {
    try {
      const config = await getSandwichConfig()
      setSettings(config)
    } catch (error) {
      console.error('샌드위치 설정 로드 실패:', error)
    }
  }

  // 샌드위치 전략 시작
  const handleStartStrategy = async () => {
    setIsLoading(true)
    try {
      const result = await startSandwichStrategy()
      if (result.success) {
        setIsRunning(true)
        alert('샌드위치 전략이 시작되었습니다!')
        loadSandwichStatus()
      } else {
        alert(`샌드위치 전략 시작 실패: ${result.message}`)
      }
    } catch (error) {
      alert('샌드위치 전략 시작 실패: ' + error)
    } finally {
      setIsLoading(false)
    }
  }

  // 샌드위치 전략 중지
  const handleStopStrategy = async () => {
    setIsLoading(true)
    try {
      const result = await stopSandwichStrategy()
      if (result.success) {
        setIsRunning(false)
        alert('샌드위치 전략이 중지되었습니다!')
        loadSandwichStatus()
      } else {
        alert(`샌드위치 전략 중지 실패: ${result.message}`)
      }
    } catch (error) {
      alert('샌드위치 전략 중지 실패: ' + error)
    } finally {
      setIsLoading(false)
    }
  }

  const handleSettingsChange = (key: string, value: any) => {
    setSettings(prev => ({ ...prev, [key]: value }))
  }

  const saveSettings = async () => {
    try {
      const result = await updateSandwichConfig(settings)
      if (result.success) {
        alert('설정이 저장되었습니다!')
      } else {
        alert(`설정 저장 실패: ${result.message}`)
      }
    } catch (error) {
      alert('설정 저장 실패: ' + error)
    }
  }

  // 가동시간 포맷팅
  const formatUptime = (seconds: number) => {
    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    const secs = seconds % 60
    return `${hours}h ${minutes}m ${secs}s`
  }

  // 설정 검증
  const validateSettings = () => {
    const errors = []
    
    if (!settings.contract_address || settings.contract_address === '0x0000000000000000000000000000000000000000') {
      errors.push('샌드위치 컨트랙트 주소를 설정해주세요')
    }
    
    if (!settings.flashbots_relay_url || !settings.flashbots_relay_url.includes('flashbots')) {
      errors.push('유효한 Flashbots Relay URL을 입력해주세요')
    }
    
    if (settings.min_profit_eth <= 0) {
      errors.push('최소 수익은 0보다 커야 합니다')
    }
    
    return errors
  }

  // 설정 테스트
  const testSettings = async () => {
    const errors = validateSettings()
    if (errors.length > 0) {
      alert('설정 오류:\n' + errors.join('\n'))
      return
    }
    
    try {
      // 여기서 실제로 설정을 테스트하는 API 호출
      alert('설정이 유효합니다! 샌드위치 전략을 시작할 수 있습니다.')
    } catch (error) {
      alert('설정 테스트 실패: ' + error)
    }
  }

  const metrics = dashboard || {
    total_sandwiches: 0,
    total_profit: '0',
    active_opportunities: 0,
    success_rate: 0,
    pending_bundles: 0,
    performance_metrics: {
      avg_execution_time_ms: 0,
      uptime_seconds: 0,
      execution_success_rate: 0
    }
  }

  return (
    <div className="space-y-6 p-6 bg-gray-50 min-h-screen">
      {/* 헤더 */}
      <div className="bg-white rounded-lg shadow-sm border p-6">
        <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-gray-900">샌드위치 전략 v2.0 통합 대시보드</h1>
            <p className="text-gray-600 mt-1">실시간 멤풀 모니터링 및 MEV 번들 실행</p>
          </div>
          
          {/* 샌드위치 전략 제어 패널 */}
          <div className="flex flex-col sm:flex-row items-start sm:items-center space-y-2 sm:space-y-0 sm:space-x-4">
            <div className="flex items-center space-x-2">
              <div className={`w-3 h-3 rounded-full ${isRunning ? 'bg-green-400 animate-pulse' : 'bg-red-400'}`}></div>
              <span className="text-sm font-medium text-gray-700">
                {isRunning ? '실행 중' : '중지됨'}
              </span>
            </div>
            
            <div className="flex space-x-2">
              <button
                onClick={handleStartStrategy}
                disabled={isRunning || isLoading}
                className={`px-4 py-2 rounded-md text-sm font-medium flex items-center ${
                  isRunning || isLoading 
                    ? 'bg-gray-300 text-gray-500 cursor-not-allowed' 
                    : 'bg-green-600 hover:bg-green-700 text-white'
                }`}
              >
                <Play className="w-4 h-4 mr-2" />
                {isLoading ? '시작 중...' : '시작'}
              </button>
              
              <button
                onClick={handleStopStrategy}
                disabled={!isRunning || isLoading}
                className={`px-4 py-2 rounded-md text-sm font-medium flex items-center ${
                  !isRunning || isLoading 
                    ? 'bg-gray-300 text-gray-500 cursor-not-allowed' 
                    : 'bg-red-600 hover:bg-red-700 text-white'
                }`}
              >
                <Square className="w-4 h-4 mr-2" />
                {isLoading ? '중지 중...' : '중지'}
              </button>
            </div>
          </div>
        </div>

        {/* 상태 정보 */}
        {isRunning && (
          <div className="mt-6 grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-blue-50 p-3 rounded-lg flex items-center space-x-2">
              <Clock className="w-4 h-4 text-blue-600" />
              <div>
                <div className="text-xs text-blue-600 font-medium">가동시간</div>
                <div className="text-sm font-semibold text-gray-900">{formatUptime(uptime)}</div>
              </div>
            </div>
            <div className="bg-green-50 p-3 rounded-lg flex items-center space-x-2">
              <Activity className="w-4 h-4 text-green-600" />
              <div>
                <div className="text-xs text-green-600 font-medium">마지막 스캔</div>
                <div className="text-sm font-semibold text-gray-900">{lastScan || '없음'}</div>
              </div>
            </div>
            <div className="bg-purple-50 p-3 rounded-lg flex items-center space-x-2">
              <Target className="w-4 h-4 text-purple-600" />
              <div>
                <div className="text-xs text-purple-600 font-medium">활성 기회</div>
                <div className="text-sm font-semibold text-gray-900">{opportunities.length}개</div>
              </div>
            </div>
          </div>
        )}
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
              {tab === 'opportunities' && '🥪 샌드위치 기회'}
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
              <h3 className="text-sm font-semibold mb-1 text-gray-700">총 샌드위치</h3>
              <p className="text-3xl font-bold text-blue-600">{metrics.total_sandwiches}</p>
              <p className="text-xs text-gray-600 mt-1">누적 실행 횟수</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-green-50 to-green-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">성공률</h3>
              <p className="text-3xl font-bold text-green-600">{(metrics.success_rate * 100).toFixed(1)}%</p>
              <p className="text-xs text-gray-600 mt-1">실행 성공률</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-purple-50 to-purple-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">총 수익</h3>
              <p className="text-3xl font-bold text-purple-600">{parseFloat(metrics.total_profit).toFixed(4)} ETH</p>
              <p className="text-xs text-gray-600 mt-1">실현 수익</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-orange-50 to-orange-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">활성 기회</h3>
              <p className="text-3xl font-bold text-orange-600">{metrics.active_opportunities}</p>
              <p className="text-xs text-gray-600 mt-1">모니터링 중</p>
            </div>
            <div className="border rounded-lg p-4 bg-gradient-to-br from-red-50 to-red-100">
              <h3 className="text-sm font-semibold mb-1 text-gray-700">대기 중</h3>
              <p className="text-3xl font-bold text-red-600">{metrics.pending_bundles}</p>
              <p className="text-xs text-gray-600 mt-1">번들 대기</p>
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

          {/* 경고 메시지 */}
          <div className="border rounded-lg p-4 border-yellow-200 bg-yellow-50">
            <h3 className="font-semibold mb-2 text-yellow-800 flex items-center">
              <AlertTriangle className="w-5 h-5 mr-2" />
              ⚠️ 고위험 전략 주의사항
            </h3>
            <div className="text-yellow-700 space-y-1 text-sm">
              <p>• 샌드위치 공격은 윤리적 및 규제적 리스크가 있습니다</p>
              <p>• 일부 거래소에서는 샌드위치 공격을 감지하여 차단할 수 있습니다</p>
              <p>• 사용 전 관련 법규 및 거래소 정책을 확인하세요</p>
              <p>• 높은 가스비 손실 위험이 있으니 신중하게 사용하세요</p>
            </div>
          </div>
        </div>
      )}

      {/* 샌드위치 기회 탭 */}
      {activeTab === 'opportunities' && (
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-4">활성 샌드위치 기회 ({opportunities.length})</h3>
          {opportunities.length === 0 ? (
            <div className="text-center py-12">
              <div className="text-6xl mb-4">🥪</div>
              <p className="text-gray-500">현재 활성 샌드위치 기회가 없습니다</p>
              <p className="text-sm text-gray-400 mt-2">멤풀에서 대형 스왑 트랜잭션을 모니터링 중입니다</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full border-collapse text-sm">
                <thead className="bg-gray-50">
                  <tr className="text-left border-b-2">
                    <th className="p-3">DEX</th>
                    <th className="p-3">토큰</th>
                    <th className="p-3">금액</th>
                    <th className="p-3">가격 영향</th>
                    <th className="p-3">예상 수익</th>
                    <th className="p-3">성공 확률</th>
                    <th className="p-3">경쟁 수준</th>
                    <th className="p-3">시간</th>
                    <th className="p-3">액션</th>
                  </tr>
                </thead>
                <tbody>
                  {opportunities.map((opp: any) => (
                    <tr key={opp.id} className="border-b hover:bg-blue-50 transition-colors">
                      <td className="p-3">
                        <span className="px-2 py-1 bg-blue-100 text-blue-800 rounded text-xs font-medium">
                          {opp.dex_type}
                        </span>
                      </td>
                      <td className="p-3 font-mono text-xs">{opp.token_pair}</td>
                      <td className="p-3 font-medium">{opp.amount}</td>
                      <td className="p-3">
                        <span className={`font-bold ${
                          opp.price_impact > 0.05
                            ? 'text-red-600'
                            : opp.price_impact > 0.02
                              ? 'text-orange-600'
                              : 'text-green-600'
                        }`}>
                          {(opp.price_impact * 100).toFixed(2)}%
                        </span>
                      </td>
                      <td className="p-3 text-right font-bold text-green-600">{opp.estimated_profit} ETH</td>
                      <td className="p-3 text-right">
                        <span className={`font-bold ${
                          opp.success_probability > 0.7
                            ? 'text-green-600'
                            : opp.success_probability > 0.4
                              ? 'text-orange-600'
                              : 'text-red-600'
                        }`}>
                          {(opp.success_probability * 100).toFixed(1)}%
                        </span>
                      </td>
                      <td className="p-3">
                        <span className={`px-2 py-1 rounded text-xs font-medium ${
                          opp.competition_level === 'Low'
                            ? 'bg-green-100 text-green-800'
                            : opp.competition_level === 'Medium'
                              ? 'bg-yellow-100 text-yellow-800'
                              : opp.competition_level === 'High'
                                ? 'bg-orange-100 text-orange-800'
                                : 'bg-red-100 text-red-800'
                        }`}>
                          {opp.competition_level}
                        </span>
                      </td>
                      <td className="p-3 text-xs text-gray-500">
                        {new Date(opp.detected_at * 1000).toLocaleTimeString()}
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
          <h3 className="font-semibold mb-4">최근 샌드위치 실행</h3>
          <div className="text-center py-12">
            <div className="text-6xl mb-4">📜</div>
            <p className="text-gray-500">최근 샌드위치 실행 내역이 없습니다</p>
            <p className="text-sm text-gray-400 mt-2">샌드위치 실행 시 여기에 표시됩니다</p>
          </div>
        </div>
      )}

      {/* 설정 탭 */}
      {activeTab === 'settings' && (
        <div className="space-y-6">
          <div className="border rounded-lg p-6">
            <h3 className="font-semibold mb-6 text-lg">샌드위치 전략 설정</h3>

            <div className="space-y-6">
              {/* 기본 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-blue-600">🎯 기본 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">최소 거래 가치 (ETH)</label>
                    <input
                      type="number"
                      step="0.01"
                      value={settings.min_value_eth}
                      onChange={(e) => handleSettingsChange('min_value_eth', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">이 금액 이하의 거래는 무시됩니다</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 가스 가격 (Gwei)</label>
                    <input
                      type="number"
                      value={settings.max_gas_price_gwei}
                      onChange={(e) => handleSettingsChange('max_gas_price_gwei', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">이 가격을 초과하면 실행 중단</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최소 수익 (ETH)</label>
                    <input
                      type="number"
                      step="0.001"
                      value={settings.min_profit_eth}
                      onChange={(e) => handleSettingsChange('min_profit_eth', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">이 금액 이하의 수익은 무시됩니다</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최소 수익률 (%)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.min_profit_percentage * 100}
                      onChange={(e) => handleSettingsChange('min_profit_percentage', parseFloat(e.target.value) / 100)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">이 비율 이하의 수익률은 무시됩니다</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 가격 영향 (%)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.max_price_impact * 100}
                      onChange={(e) => handleSettingsChange('max_price_impact', parseFloat(e.target.value) / 100)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">이 비율을 초과하는 가격 영향은 무시됩니다</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Kelly 위험 계수</label>
                    <input
                      type="number"
                      step="0.1"
                      min="0.1"
                      max="1.0"
                      value={settings.kelly_risk_factor}
                      onChange={(e) => handleSettingsChange('kelly_risk_factor', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">0.5 = Half Kelly, 1.0 = Full Kelly</p>
                  </div>
                </div>
              </div>

              {/* 컨트랙트 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-red-600">🔐 컨트랙트 설정</h4>
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">샌드위치 컨트랙트 주소</label>
                    <input
                      type="text"
                      value={settings.contract_address}
                      onChange={(e) => handleSettingsChange('contract_address', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="0x..."
                    />
                    <p className="text-xs text-red-500 mt-1">⚠️ 샌드위치 실행에 사용할 스마트 컨트랙트 주소</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Flashbots Relay URL</label>
                    <input
                      type="url"
                      value={settings.flashbots_relay_url}
                      onChange={(e) => handleSettingsChange('flashbots_relay_url', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">MEV 번들 제출용 Flashbots 릴레이</p>
                  </div>
                </div>
              </div>

              {/* 가스 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-green-600">⛽ 가스 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">가스 한도</label>
                    <input
                      type="number"
                      value={settings.gas_limit}
                      onChange={(e) => handleSettingsChange('gas_limit', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">트랜잭션 최대 가스 사용량</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">트랜잭션당 가스</label>
                    <input
                      type="number"
                      value={settings.gas_per_tx}
                      onChange={(e) => handleSettingsChange('gas_per_tx', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">각 트랜잭션당 가스 사용량</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Front-run 우선순위 수수료 (Gwei)</label>
                    <input
                      type="number"
                      value={settings.front_run_priority_fee_gwei}
                      onChange={(e) => handleSettingsChange('front_run_priority_fee_gwei', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Front-run 트랜잭션 우선순위 수수료</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Back-run 우선순위 수수료 (Gwei)</label>
                    <input
                      type="number"
                      value={settings.back_run_priority_fee_gwei}
                      onChange={(e) => handleSettingsChange('back_run_priority_fee_gwei', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Back-run 트랜잭션 우선순위 수수료</p>
                  </div>
                </div>
              </div>

              {/* 경쟁 수준별 우선순위 수수료 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-purple-600">🏆 경쟁 수준별 우선순위 수수료 (Gwei)</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">낮은 경쟁</label>
                    <input
                      type="number"
                      value={settings.priority_fee_low_gwei}
                      onChange={(e) => handleSettingsChange('priority_fee_low_gwei', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">중간 경쟁</label>
                    <input
                      type="number"
                      value={settings.priority_fee_medium_gwei}
                      onChange={(e) => handleSettingsChange('priority_fee_medium_gwei', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">높은 경쟁</label>
                    <input
                      type="number"
                      value={settings.priority_fee_high_gwei}
                      onChange={(e) => handleSettingsChange('priority_fee_high_gwei', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">치열한 경쟁</label>
                    <input
                      type="number"
                      value={settings.priority_fee_critical_gwei}
                      onChange={(e) => handleSettingsChange('priority_fee_critical_gwei', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                  </div>
                </div>
              </div>

              {/* DEX 수수료 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-orange-600">💱 DEX 수수료 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">Uniswap V2 수수료</label>
                    <input
                      type="number"
                      step="0.001"
                      value={settings.uniswap_v2_fee}
                      onChange={(e) => handleSettingsChange('uniswap_v2_fee', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Uniswap V2 수수료 (0.003 = 0.3%)</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Uniswap V3 수수료</label>
                    <input
                      type="number"
                      step="0.001"
                      value={settings.uniswap_v3_fee}
                      onChange={(e) => handleSettingsChange('uniswap_v3_fee', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Uniswap V3 수수료 (0.003 = 0.3%)</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">기본 DEX 수수료</label>
                    <input
                      type="number"
                      step="0.001"
                      value={settings.default_fee}
                      onChange={(e) => handleSettingsChange('default_fee', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">기본 DEX 수수료</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Uniswap V3 수수료 티어</label>
                    <input
                      type="number"
                      value={settings.uniswap_v3_fee_tier}
                      onChange={(e) => handleSettingsChange('uniswap_v3_fee_tier', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">3000 = 0.3%, 500 = 0.05%, 10000 = 1%</p>
                  </div>
                </div>
              </div>

              {/* 타이밍 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-indigo-600">⏰ 타이밍 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">트랜잭션 데드라인 (초)</label>
                    <input
                      type="number"
                      value={settings.deadline_secs}
                      onChange={(e) => handleSettingsChange('deadline_secs', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">트랜잭션 유효 기간</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 대기 블록 수</label>
                    <input
                      type="number"
                      value={settings.max_wait_blocks}
                      onChange={(e) => handleSettingsChange('max_wait_blocks', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">번들 포함 최대 대기 블록 수</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">블록 확인 대기 시간 (초)</label>
                    <input
                      type="number"
                      value={settings.wait_seconds}
                      onChange={(e) => handleSettingsChange('wait_seconds', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">블록 확인 간격</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">통계 출력 간격 (초)</label>
                    <input
                      type="number"
                      value={settings.stats_interval_secs}
                      onChange={(e) => handleSettingsChange('stats_interval_secs', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">통계 출력 주기</p>
                  </div>
                </div>
              </div>

              {/* 저장 버튼 */}
              <div className="border-t pt-6">
                <div className="flex justify-between items-center">
                  <div className="text-sm text-gray-600">
                    {validateSettings().length > 0 && (
                      <div className="text-red-500">
                        ⚠️ 필수 설정이 누락되었습니다: {validateSettings().length}개
                      </div>
                    )}
                  </div>
                  
                  <div className="flex space-x-4">
                    <Button
                      onClick={() => loadSandwichConfig()}
                      variant="outline"
                    >
                      <Settings className="w-4 h-4 mr-2" />
                      설정 새로고침
                    </Button>
                    
                    <Button
                      onClick={testSettings}
                      className="bg-yellow-600 hover:bg-yellow-700 text-white"
                    >
                      <AlertTriangle className="w-4 h-4 mr-2" />
                      설정 테스트
                    </Button>
                    
                    <Button
                      onClick={saveSettings}
                      className="bg-blue-600 hover:bg-blue-700 text-white"
                    >
                      <Settings className="w-4 h-4 mr-2" />
                      설정 저장
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}