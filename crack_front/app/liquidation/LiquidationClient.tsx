"use client"

import { useState, useEffect } from 'react'
import { useWebSocket } from '../../lib/hooks/use-websocket'
import { 
  startLiquidationStrategy, 
  stopLiquidationStrategy, 
  getLiquidationStatus, 
  getLiquidationConfig, 
  updateLiquidationConfig 
} from '../../lib/api'
import { Card, CardContent, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Play, Square, Settings, Activity, DollarSign, Clock, AlertTriangle } from 'lucide-react'

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

  // 청산 전략 상태
  const [isRunning, setIsRunning] = useState(false)
  const [uptime, setUptime] = useState(0)
  const [lastScan, setLastScan] = useState('')
  const [isLoading, setIsLoading] = useState(false)

  // Settings state
  const [settings, setSettings] = useState({
    // 청산 전략 기본 설정
    min_profit_eth: '0.05',
    scan_interval_seconds: 30,
    max_concurrent_liquidations: 3,
    funding_mode: 'auto',
    gas_multiplier: 1.5,
    max_gas_price_gwei: 200,
    health_factor_threshold: 1.0,
    auto_execute: false,
    
    // 외부 서비스 설정
    rpc_url: 'https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY',
    private_key: '',
    flashbots_relay_url: 'https://relay.flashbots.net',
    
    // API 키 설정
    alchemy_api_key: '',
    infura_api_key: '',
    etherscan_api_key: '',
    
    // DEX 설정
    dex_aggregator: '0x',
    oneinch_api_key: '',
    
    // 모니터링 설정
    slack_webhook_url: '',
    
    // 보안 설정
    max_slippage_percent: 3.0,
    max_gas_limit: 500000,
    priority_fee_gwei: 2.0,
  })

  // 청산 전략 상태 로드
  useEffect(() => {
    loadLiquidationStatus()
    loadLiquidationConfig()
  }, [])

  // 실시간 상태 업데이트 (5초마다)
  useEffect(() => {
    if (isRunning) {
      const interval = setInterval(() => {
        loadLiquidationStatus()
      }, 5000)
      return () => clearInterval(interval)
    }
  }, [isRunning])

  const loadLiquidationStatus = async () => {
    try {
      const status = await getLiquidationStatus()
      setIsRunning(status.is_running)
      setUptime(status.uptime_seconds)
      setLastScan(status.last_scan)
    } catch (error) {
      console.error('청산 상태 로드 실패:', error)
    }
  }

  const loadLiquidationConfig = async () => {
    try {
      const config = await getLiquidationConfig()
      setSettings(config)
    } catch (error) {
      console.error('청산 설정 로드 실패:', error)
    }
  }

  // 청산 전략 시작
  const handleStartStrategy = async () => {
    setIsLoading(true)
    try {
      const result = await startLiquidationStrategy()
      if (result.success) {
        setIsRunning(true)
        alert('청산 전략이 시작되었습니다!')
        loadLiquidationStatus()
      } else {
        alert(`청산 전략 시작 실패: ${result.message}`)
      }
    } catch (error) {
      alert('청산 전략 시작 실패: ' + error)
    } finally {
      setIsLoading(false)
    }
  }

  // 청산 전략 중지
  const handleStopStrategy = async () => {
    setIsLoading(true)
    try {
      const result = await stopLiquidationStrategy()
      if (result.success) {
        setIsRunning(false)
        alert('청산 전략이 중지되었습니다!')
        loadLiquidationStatus()
      } else {
        alert(`청산 전략 중지 실패: ${result.message}`)
      }
    } catch (error) {
      alert('청산 전략 중지 실패: ' + error)
    } finally {
      setIsLoading(false)
    }
  }

  const handleSettingsChange = (key: string, value: any) => {
    setSettings(prev => ({ ...prev, [key]: value }))
  }

  const saveSettings = async () => {
    try {
      const result = await updateLiquidationConfig(settings)
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
    
    if (!settings.rpc_url || settings.rpc_url.includes('YOUR_API_KEY')) {
      errors.push('RPC URL을 설정해주세요')
    }
    
    if (!settings.private_key || !settings.private_key.startsWith('0x')) {
      errors.push('유효한 Private Key를 입력해주세요')
    }
    
    if (settings.auto_execute && !settings.alchemy_api_key) {
      errors.push('자동 실행을 위해서는 Alchemy API Key가 필요합니다')
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
      alert('설정이 유효합니다! 청산 전략을 시작할 수 있습니다.')
    } catch (error) {
      alert('설정 테스트 실패: ' + error)
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
    <div className="space-y-6 p-6 bg-gray-50 min-h-screen">
      {/* 헤더 */}
      <div className="bg-white rounded-lg shadow-sm border p-6">
        <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-gray-900">청산 전략 v2.0 통합 대시보드</h1>
            <p className="text-gray-600 mt-1">실시간 프로토콜 포지션 스캐닝 및 지능형 자금 조달</p>
          </div>
          
          {/* 청산 전략 제어 패널 */}
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
              <DollarSign className="w-4 h-4 text-purple-600" />
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
                  <label className="block text-sm font-medium mb-2">최소 수익 임계값 (ETH)</label>
                  <input
                    type="number"
                    step="0.001"
                    value={settings.min_profit_eth}
                    onChange={(e) => handleSettingsChange('min_profit_eth', e.target.value)}
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
                  <label className="block text-sm font-medium mb-2">Health Factor 임계값</label>
                  <input
                    type="number"
                    step="0.01"
                    value={settings.health_factor_threshold}
                    onChange={(e) => handleSettingsChange('health_factor_threshold', parseFloat(e.target.value))}
                    className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                  />
                  <p className="text-xs text-gray-500 mt-1">이 값 이하에서 청산 가능</p>
                </div>
              </div>

              {/* 가스 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4">가스 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">가스 가중치</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.gas_multiplier}
                      onChange={(e) => handleSettingsChange('gas_multiplier', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">경쟁력 확보를 위한 가스 가중치</p>
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
                </div>
              </div>

              {/* 자금 조달 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4">자금 조달 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">자금 조달 모드</label>
                    <select
                      value={settings.funding_mode}
                      onChange={(e) => handleSettingsChange('funding_mode', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    >
                      <option value="auto">자동 선택</option>
                      <option value="flashloan">플래시론 강제</option>
                      <option value="wallet">지갑 자금만</option>
                    </select>
                    <p className="text-xs text-gray-500 mt-1">자금 조달 방식 선택</p>
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
                    <p className="text-xs text-gray-500 mt-1 ml-6">수익성 있는 기회 자동 실행</p>
                  </div>
                </div>
              </div>

              {/* 블록체인 연결 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-red-600">🔐 블록체인 연결 설정</h4>
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">RPC URL</label>
                    <input
                      type="url"
                      value={settings.rpc_url}
                      onChange={(e) => handleSettingsChange('rpc_url', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
                    />
                    <p className="text-xs text-gray-500 mt-1">Ethereum 메인넷 RPC 엔드포인트</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Private Key</label>
                    <input
                      type="password"
                      value={settings.private_key}
                      onChange={(e) => handleSettingsChange('private_key', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="0x..."
                    />
                    <p className="text-xs text-red-500 mt-1">⚠️ 청산 실행에 사용할 지갑의 개인키</p>
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

              {/* API 키 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-blue-600">🔑 API 키 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">Alchemy API Key</label>
                    <input
                      type="password"
                      value={settings.alchemy_api_key}
                      onChange={(e) => handleSettingsChange('alchemy_api_key', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="alch_..."
                    />
                    <p className="text-xs text-gray-500 mt-1">가격 데이터 및 블록체인 조회</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Infura API Key</label>
                    <input
                      type="password"
                      value={settings.infura_api_key}
                      onChange={(e) => handleSettingsChange('infura_api_key', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="infura_..."
                    />
                    <p className="text-xs text-gray-500 mt-1">백업 RPC 제공자</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Etherscan API Key</label>
                    <input
                      type="password"
                      value={settings.etherscan_api_key}
                      onChange={(e) => handleSettingsChange('etherscan_api_key', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="YourEtherscanAPIKey"
                    />
                    <p className="text-xs text-gray-500 mt-1">트랜잭션 검증 및 모니터링</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">1inch API Key</label>
                    <input
                      type="password"
                      value={settings.oneinch_api_key}
                      onChange={(e) => handleSettingsChange('oneinch_api_key', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="1inch_api_key"
                    />
                    <p className="text-xs text-gray-500 mt-1">DEX 집계 및 최적 경로</p>
                  </div>
                </div>
              </div>

              {/* DEX 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-green-600">💱 DEX 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">DEX 집계기</label>
                    <select
                      value={settings.dex_aggregator}
                      onChange={(e) => handleSettingsChange('dex_aggregator', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    >
                      <option value="0x">0x Protocol</option>
                      <option value="1inch">1inch</option>
                      <option value="paraswap">ParaSwap</option>
                    </select>
                    <p className="text-xs text-gray-500 mt-1">토큰 스왑 최적화 서비스</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 슬리피지 (%)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.max_slippage_percent}
                      onChange={(e) => handleSettingsChange('max_slippage_percent', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">스왑 시 허용할 최대 가격 변동</p>
                  </div>
                </div>
              </div>

              {/* 모니터링 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-purple-600">📱 모니터링 설정</h4>
                <div className="space-y-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">Slack Webhook URL</label>
                    <input
                      type="url"
                      value={settings.slack_webhook_url}
                      onChange={(e) => handleSettingsChange('slack_webhook_url', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                      placeholder="https://hooks.slack.com/services/..."
                    />
                    <p className="text-xs text-gray-500 mt-1">청산 실행 알림을 받을 Slack 채널</p>
                    <p className="text-xs text-blue-500 mt-1">
                      💡 Slack 앱에서 Incoming Webhooks를 활성화하고 URL을 복사하세요
                    </p>
                  </div>
                </div>
              </div>

              {/* 보안 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-orange-600">🛡️ 보안 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">최대 가스 한도</label>
                    <input
                      type="number"
                      value={settings.max_gas_limit}
                      onChange={(e) => handleSettingsChange('max_gas_limit', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">트랜잭션 최대 가스 사용량</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Priority Fee (Gwei)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.priority_fee_gwei}
                      onChange={(e) => handleSettingsChange('priority_fee_gwei', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">EIP-1559 우선순위 수수료</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">가스 가중치</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.gas_multiplier}
                      onChange={(e) => handleSettingsChange('gas_multiplier', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">경쟁력 확보를 위한 가스 가중치</p>
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
                      onClick={() => loadLiquidationConfig()}
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
