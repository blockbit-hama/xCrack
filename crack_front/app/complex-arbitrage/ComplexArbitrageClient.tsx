"use client"

import { useState, useEffect } from 'react'
import { useWebSocket } from '../../lib/hooks/use-websocket'
import { 
  startComplexArbitrageStrategy, 
  stopComplexArbitrageStrategy, 
  getComplexArbitrageStatus, 
  getComplexArbitrageConfig, 
  updateComplexArbitrageConfig 
} from '../../lib/api'
import { Card, CardContent, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Play, Square, Settings, Activity, DollarSign, Clock, AlertTriangle, Target, Zap, TrendingUp, RefreshCw, Network } from 'lucide-react'

interface ComplexArbitrageProps {
  initialDashboard: any
  initialStatus: any
  initialConfig: any
  initialOpportunities: any[]
}

export function ComplexArbitrageClient({ initialDashboard, initialStatus, initialConfig, initialOpportunities }: ComplexArbitrageProps) {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'opportunities' | 'history' | 'settings'>('dashboard')
  const [dashboard] = useState(initialDashboard)
  const [status] = useState(initialStatus)
  const [opportunities] = useState(initialOpportunities)

  // 복잡한 아비트리지 전략 상태
  const [isRunning, setIsRunning] = useState(false)
  const [uptime, setUptime] = useState(0)
  const [lastScan, setLastScan] = useState('')
  const [isLoading, setIsLoading] = useState(false)

  // Settings state
  const [settings, setSettings] = useState({
    // 기본 설정
    min_profit_usd: 50.0,
    max_position_size_usd: 100000.0,
    max_path_length: 5,
    min_profit_percentage: 0.5,
    max_concurrent_trades: 2,
    execution_timeout_ms: 60000,
    
    // 전략 설정
    strategies: ['triangular', 'position_migration', 'complex'],
    flashloan_protocols: ['aave_v3'],
    max_flashloan_fee_bps: 9,
    gas_buffer_pct: 25.0,
    
    // 리스크 관리
    max_drawdown_percent: 15.0,
    stop_loss_percent: 8.0,
    take_profit_percent: 3.0,
    max_daily_loss_usd: 5000.0,
    
    // 경로 설정
    max_gas_price_gwei: 100,
    priority_fee_gwei: 5,
    deadline_secs: 300,
    
    // 프로토콜 설정
    aave_v3_pool: '0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2',
    compound_comptroller: '0x3d9819210A31b4961b30EF54bE2aeD79B9c9Cd3B',
    uniswap_v2_factory: '0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f',
    uniswap_v3_factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
  })

  // 복잡한 아비트리지 전략 상태 로드
  useEffect(() => {
    loadComplexArbitrageStatus()
    loadComplexArbitrageConfig()
  }, [])

  // 실시간 상태 업데이트 (5초마다)
  useEffect(() => {
    if (isRunning) {
      const interval = setInterval(() => {
        loadComplexArbitrageStatus()
      }, 5000)
      return () => clearInterval(interval)
    }
  }, [isRunning])

  const loadComplexArbitrageStatus = async () => {
    try {
      const status = await getComplexArbitrageStatus()
      setIsRunning(status.is_running)
      setUptime(status.uptime_seconds)
      setLastScan(status.last_scan)
    } catch (error) {
      console.error('복잡한 아비트리지 상태 로드 실패:', error)
    }
  }

  const loadComplexArbitrageConfig = async () => {
    try {
      const config = await getComplexArbitrageConfig()
      setSettings(config)
    } catch (error) {
      console.error('복잡한 아비트리지 설정 로드 실패:', error)
    }
  }

  // 복잡한 아비트리지 전략 시작
  const handleStartStrategy = async () => {
    setIsLoading(true)
    try {
      const result = await startComplexArbitrageStrategy()
      if (result.success) {
        setIsRunning(true)
        alert('복잡한 아비트리지 전략이 시작되었습니다!')
        loadComplexArbitrageStatus()
      } else {
        alert(`복잡한 아비트리지 전략 시작 실패: ${result.message}`)
      }
    } catch (error) {
      alert('복잡한 아비트리지 전략 시작 실패: ' + error)
    } finally {
      setIsLoading(false)
    }
  }

  // 복잡한 아비트리지 전략 중지
  const handleStopStrategy = async () => {
    setIsLoading(true)
    try {
      const result = await stopComplexArbitrageStrategy()
      if (result.success) {
        setIsRunning(false)
        alert('복잡한 아비트리지 전략이 중지되었습니다!')
        loadComplexArbitrageStatus()
      } else {
        alert(`복잡한 아비트리지 전략 중지 실패: ${result.message}`)
      }
    } catch (error) {
      alert('복잡한 아비트리지 전략 중지 실패: ' + error)
    } finally {
      setIsLoading(false)
    }
  }

  const handleSettingsChange = (key: string, value: any) => {
    setSettings(prev => ({ ...prev, [key]: value }))
  }

  const saveSettings = async () => {
    try {
      const result = await updateComplexArbitrageConfig(settings)
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
    
    if (settings.min_profit_usd <= 0) {
      errors.push('최소 수익은 0보다 커야 합니다')
    }
    
    if (settings.max_position_size_usd <= 0) {
      errors.push('최대 포지션 크기는 0보다 커야 합니다')
    }
    
    if (settings.max_path_length < 2) {
      errors.push('최대 경로 길이는 2 이상이어야 합니다')
    }
    
    if (settings.max_flashloan_fee_bps > 50) {
      errors.push('플래시론 수수료가 너무 높습니다 (최대 0.5%)')
    }
    
    if (settings.max_daily_loss_usd <= 0) {
      errors.push('최대 일일 손실은 0보다 커야 합니다')
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
      alert('설정이 유효합니다! 복잡한 아비트리지 전략을 시작할 수 있습니다.')
    } catch (error) {
      alert('설정 테스트 실패: ' + error)
    }
  }

  const metrics = dashboard || {
    total_trades: 0,
    total_profit: '0',
    success_rate: 0,
    active_opportunities: 0,
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
            <h1 className="text-2xl font-bold text-gray-900">복잡한 아비트리지 통합 대시보드</h1>
            <p className="text-gray-600 mt-1">다중자산 플래시론을 활용한 고급 차익거래 전략</p>
          </div>
          
          {/* 복잡한 아비트리지 전략 제어 패널 */}
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
              <Network className="w-4 h-4 text-purple-600" />
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
              {tab === 'opportunities' && '🌐 복잡한 기회'}
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
              <h3 className="text-sm font-semibold mb-1 text-gray-700">총 거래</h3>
              <p className="text-3xl font-bold text-blue-600">{metrics.total_trades}</p>
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
              <h3 className="text-sm font-semibold mb-1 text-gray-700">활성 기회</h3>
              <p className="text-3xl font-bold text-orange-600">{metrics.active_opportunities}</p>
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

          {/* 전략별 상태 */}
          <div className="border rounded-lg p-4">
            <h3 className="font-semibold mb-4">활성 전략</h3>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div className="bg-gray-50 rounded-lg p-4 hover:shadow-md transition-shadow">
                <div className="flex justify-between items-start mb-3">
                  <h4 className="font-medium text-lg">삼각 아비트리지</h4>
                  <span className="px-2 py-1 rounded text-xs font-medium bg-green-100 text-green-800">
                    활성
                  </span>
                </div>
                <div className="space-y-2 text-sm">
                  <div className="flex justify-between">
                    <span className="text-gray-600">발견된 기회:</span>
                    <span className="font-medium">12개</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-gray-600">평균 수익:</span>
                    <span className="font-medium text-green-600">$245</span>
                  </div>
                </div>
              </div>
              
              <div className="bg-gray-50 rounded-lg p-4 hover:shadow-md transition-shadow">
                <div className="flex justify-between items-start mb-3">
                  <h4 className="font-medium text-lg">포지션 마이그레이션</h4>
                  <span className="px-2 py-1 rounded text-xs font-medium bg-yellow-100 text-yellow-800">
                    대기
                  </span>
                </div>
                <div className="space-y-2 text-sm">
                  <div className="flex justify-between">
                    <span className="text-gray-600">발견된 기회:</span>
                    <span className="font-medium">3개</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-gray-600">평균 수익:</span>
                    <span className="font-medium text-green-600">$1,250</span>
                  </div>
                </div>
              </div>
              
              <div className="bg-gray-50 rounded-lg p-4 hover:shadow-md transition-shadow">
                <div className="flex justify-between items-start mb-3">
                  <h4 className="font-medium text-lg">복합 아비트리지</h4>
                  <span className="px-2 py-1 rounded text-xs font-medium bg-blue-100 text-blue-800">
                    실행 중
                  </span>
                </div>
                <div className="space-y-2 text-sm">
                  <div className="flex justify-between">
                    <span className="text-gray-600">발견된 기회:</span>
                    <span className="font-medium">7개</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-gray-600">평균 수익:</span>
                    <span className="font-medium text-green-600">$890</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 복잡한 기회 탭 */}
      {activeTab === 'opportunities' && (
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-4">활성 복잡한 아비트리지 기회 ({opportunities.length})</h3>
          {opportunities.length === 0 ? (
            <div className="text-center py-12">
              <div className="text-6xl mb-4">🌐</div>
              <p className="text-gray-500">현재 활성 복잡한 아비트리지 기회가 없습니다</p>
              <p className="text-sm text-gray-400 mt-2">다중자산 경로를 모니터링 중입니다</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full border-collapse text-sm">
                <thead className="bg-gray-50">
                  <tr className="text-left border-b-2">
                    <th className="p-3">전략</th>
                    <th className="p-3">경로</th>
                    <th className="p-3">자산</th>
                    <th className="p-3">예상 수익</th>
                    <th className="p-3">수익률</th>
                    <th className="p-3">복잡도</th>
                    <th className="p-3">시간</th>
                    <th className="p-3">액션</th>
                  </tr>
                </thead>
                <tbody>
                  {opportunities.map((opp: any) => (
                    <tr key={opp.id} className="border-b hover:bg-blue-50 transition-colors">
                      <td className="p-3">
                        <span className={`px-2 py-1 rounded text-xs font-medium ${
                          opp.strategy === 'triangular' ? 'bg-green-100 text-green-800' :
                          opp.strategy === 'position_migration' ? 'bg-yellow-100 text-yellow-800' :
                          'bg-blue-100 text-blue-800'
                        }`}>
                          {opp.strategy}
                        </span>
                      </td>
                      <td className="p-3 font-mono text-xs">{opp.path}</td>
                      <td className="p-3 font-mono text-xs">{opp.assets.join(' → ')}</td>
                      <td className="p-3 text-right font-bold text-green-600">${opp.estimated_profit}</td>
                      <td className="p-3 text-right">
                        <span className={`font-bold ${
                          opp.profit_percentage > 2.0
                            ? 'text-green-600'
                            : opp.profit_percentage > 1.0
                              ? 'text-orange-600'
                              : 'text-red-600'
                        }`}>
                          {opp.profit_percentage.toFixed(2)}%
                        </span>
                      </td>
                      <td className="p-3 text-center">
                        <span className={`px-2 py-1 rounded text-xs font-medium ${
                          opp.complexity === 'high' ? 'bg-red-100 text-red-800' :
                          opp.complexity === 'medium' ? 'bg-yellow-100 text-yellow-800' :
                          'bg-green-100 text-green-800'
                        }`}>
                          {opp.complexity}
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
          <h3 className="font-semibold mb-4">최근 복잡한 아비트리지 실행</h3>
          <div className="text-center py-12">
            <div className="text-6xl mb-4">📜</div>
            <p className="text-gray-500">최근 복잡한 아비트리지 실행 내역이 없습니다</p>
            <p className="text-sm text-gray-400 mt-2">복잡한 아비트리지 실행 시 여기에 표시됩니다</p>
          </div>
        </div>
      )}

      {/* 설정 탭 */}
      {activeTab === 'settings' && (
        <div className="space-y-6">
          <div className="border rounded-lg p-6">
            <h3 className="font-semibold mb-6 text-lg">복잡한 아비트리지 전략 설정</h3>

            <div className="space-y-6">
              {/* 기본 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-blue-600">🎯 기본 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">최소 수익 (USD)</label>
                    <input
                      type="number"
                      step="1"
                      value={settings.min_profit_usd}
                      onChange={(e) => handleSettingsChange('min_profit_usd', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">이 금액 이하의 기회는 무시됩니다</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 포지션 크기 (USD)</label>
                    <input
                      type="number"
                      step="1000"
                      value={settings.max_position_size_usd}
                      onChange={(e) => handleSettingsChange('max_position_size_usd', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">한 번에 거래할 수 있는 최대 금액</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 경로 길이</label>
                    <input
                      type="number"
                      min="2"
                      max="10"
                      value={settings.max_path_length}
                      onChange={(e) => handleSettingsChange('max_path_length', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">아비트리지 경로의 최대 단계 수</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최소 수익률 (%)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.min_profit_percentage}
                      onChange={(e) => handleSettingsChange('min_profit_percentage', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">허용할 최소 수익률</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 동시 거래 수</label>
                    <input
                      type="number"
                      min="1"
                      max="5"
                      value={settings.max_concurrent_trades}
                      onChange={(e) => handleSettingsChange('max_concurrent_trades', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">동시에 실행 가능한 거래 개수</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">실행 타임아웃 (ms)</label>
                    <input
                      type="number"
                      value={settings.execution_timeout_ms}
                      onChange={(e) => handleSettingsChange('execution_timeout_ms', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">거래 실행 최대 대기 시간</p>
                  </div>
                </div>
              </div>

              {/* 전략 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-green-600">🌐 전략 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">활성 전략</label>
                    <div className="space-y-2">
                      {['triangular', 'position_migration', 'complex'].map(strategy => (
                        <label key={strategy} className="flex items-center">
                          <input
                            type="checkbox"
                            checked={settings.strategies.includes(strategy)}
                            onChange={(e) => {
                              const newStrategies = e.target.checked
                                ? [...settings.strategies, strategy]
                                : settings.strategies.filter(s => s !== strategy)
                              handleSettingsChange('strategies', newStrategies)
                            }}
                            className="mr-2"
                          />
                          <span className="text-sm capitalize">{strategy.replace('_', ' ')}</span>
                        </label>
                      ))}
                    </div>
                    <p className="text-xs text-gray-500 mt-1">실행할 아비트리지 전략 선택</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">플래시론 프로토콜</label>
                    <div className="space-y-2">
                      {['aave_v3', 'compound', 'dydx'].map(protocol => (
                        <label key={protocol} className="flex items-center">
                          <input
                            type="checkbox"
                            checked={settings.flashloan_protocols.includes(protocol)}
                            onChange={(e) => {
                              const newProtocols = e.target.checked
                                ? [...settings.flashloan_protocols, protocol]
                                : settings.flashloan_protocols.filter(p => p !== protocol)
                              handleSettingsChange('flashloan_protocols', newProtocols)
                            }}
                            className="mr-2"
                          />
                          <span className="text-sm uppercase">{protocol}</span>
                        </label>
                      ))}
                    </div>
                    <p className="text-xs text-gray-500 mt-1">사용할 플래시론 프로토콜 선택</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 플래시론 수수료 (bps)</label>
                    <input
                      type="number"
                      value={settings.max_flashloan_fee_bps}
                      onChange={(e) => handleSettingsChange('max_flashloan_fee_bps', parseInt(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">9 = 0.09%</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">가스 버퍼 (%)</label>
                    <input
                      type="number"
                      step="1"
                      value={settings.gas_buffer_pct}
                      onChange={(e) => handleSettingsChange('gas_buffer_pct', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">가스 가격 변동을 위한 여유분</p>
                  </div>
                </div>
              </div>

              {/* 리스크 관리 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-red-600">⚠️ 리스크 관리</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">최대 드로다운 (%)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.max_drawdown_percent}
                      onChange={(e) => handleSettingsChange('max_drawdown_percent', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">허용할 최대 손실률</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">스탑 로스 (%)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.stop_loss_percent}
                      onChange={(e) => handleSettingsChange('stop_loss_percent', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">개별 거래 손절 기준</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">테이크 프로핏 (%)</label>
                    <input
                      type="number"
                      step="0.1"
                      value={settings.take_profit_percent}
                      onChange={(e) => handleSettingsChange('take_profit_percent', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">개별 거래 익절 기준</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">최대 일일 손실 (USD)</label>
                    <input
                      type="number"
                      step="1000"
                      value={settings.max_daily_loss_usd}
                      onChange={(e) => handleSettingsChange('max_daily_loss_usd', parseFloat(e.target.value))}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">하루 최대 허용 손실</p>
                  </div>
                </div>
              </div>

              {/* 프로토콜 설정 */}
              <div className="border-t pt-6">
                <h4 className="font-medium mb-4 text-indigo-600">🔗 프로토콜 설정</h4>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-medium mb-2">Aave V3 Pool</label>
                    <input
                      type="text"
                      value={settings.aave_v3_pool}
                      onChange={(e) => handleSettingsChange('aave_v3_pool', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Aave V3 풀 주소</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Compound Comptroller</label>
                    <input
                      type="text"
                      value={settings.compound_comptroller}
                      onChange={(e) => handleSettingsChange('compound_comptroller', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Compound Comptroller 주소</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Uniswap V2 Factory</label>
                    <input
                      type="text"
                      value={settings.uniswap_v2_factory}
                      onChange={(e) => handleSettingsChange('uniswap_v2_factory', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Uniswap V2 Factory 주소</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium mb-2">Uniswap V3 Factory</label>
                    <input
                      type="text"
                      value={settings.uniswap_v3_factory}
                      onChange={(e) => handleSettingsChange('uniswap_v3_factory', e.target.value)}
                      className="w-full p-2 border rounded focus:ring-2 focus:ring-blue-500"
                    />
                    <p className="text-xs text-gray-500 mt-1">Uniswap V3 Factory 주소</p>
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
                      onClick={() => loadComplexArbitrageConfig()}
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