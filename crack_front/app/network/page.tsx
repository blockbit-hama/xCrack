"use client";

import { useEffect, useState } from "react";
import { 
  getNetworkHealth, 
  acknowledgeNetworkIncident,
  runLatencyTest,
  type NetworkHealthDashboard, 
  type NodeInfo,
  type ExternalServiceStatus,
  type SystemResourceMetrics 
} from '@/lib/api';

export default function NetworkHealthPage() {
  const [health, setHealth] = useState<NetworkHealthDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [latencyTestTarget, setLatencyTestTarget] = useState("8.8.8.8");
  const [runningLatencyTest, setRunningLatencyTest] = useState(false);

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      setError("");
      try {
        const healthData = await getNetworkHealth();
        setHealth(healthData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 10000); // 10초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'healthy':
      case 'operational':
      case 'online':
      case 'resolved':
        return 'bg-green-100 text-green-800';
      case 'degraded':
      case 'fair':
      case 'investigating':
        return 'bg-yellow-100 text-yellow-800';
      case 'down':
      case 'outage':
      case 'offline':
      case 'poor':
      case 'critical':
        return 'bg-red-100 text-red-800';
      case 'unknown':
      case 'open':
        return 'bg-gray-100 text-gray-800';
      default:
        return 'bg-gray-100 text-gray-800';
    }
  };

  const getSeverityColor = (severity: string) => {
    switch (severity) {
      case 'critical': return 'bg-red-500 text-white';
      case 'high': return 'bg-orange-500 text-white';
      case 'medium': return 'bg-yellow-500 text-black';
      case 'low': return 'bg-blue-500 text-white';
      default: return 'bg-gray-500 text-white';
    }
  };

  const getResourceUsageColor = (usage: number) => {
    if (usage >= 90) return 'text-red-600';
    if (usage >= 75) return 'text-yellow-600';
    if (usage >= 50) return 'text-blue-600';
    return 'text-green-600';
  };

  const formatBytes = (bytes: number) => {
    if (bytes >= 1024 * 1024 * 1024) return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
    if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
    if (bytes >= 1024) return (bytes / 1024).toFixed(2) + ' KB';
    return bytes.toFixed(0) + ' B';
  };

  const safeNumber = (value: any, defaultValue: number = 0): number => {
    if (value === undefined || value === null || isNaN(Number(value))) {
      return defaultValue;
    }
    return Number(value);
  };

  const handleLatencyTest = async () => {
    setRunningLatencyTest(true);
    try {
      const result = await runLatencyTest(latencyTestTarget);
      if (result) {
        // 결과를 health 데이터에 추가
        setHealth(prev => prev ? {
          ...prev,
          latency_tests: [result, ...prev.latency_tests.slice(0, 9)]
        } : null);
      }
    } catch (e) {
      console.error('Latency test failed:', e);
    } finally {
      setRunningLatencyTest(false);
    }
  };

  const handleAcknowledgeIncident = async (incidentId: string) => {
    try {
      const success = await acknowledgeNetworkIncident(incidentId);
      if (success) {
        setHealth(prev => prev ? {
          ...prev,
          network_incidents: prev.network_incidents.map(incident =>
            incident.id === incidentId ? { ...incident, status: 'investigating' as const } : incident
          )
        } : null);
      }
    } catch (e) {
      console.error('Failed to acknowledge incident:', e);
    }
  };

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">네트워크 헬스 모니터</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">네트워크 헬스 모니터</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      <h1 className="text-2xl font-bold">네트워크 헬스 모니터</h1>
      
      {/* 네트워크 전체 상태 */}
      {health?.network_metrics && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">네트워크 전체 상태</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">현재 블록</h3>
              <p className="text-lg font-bold">{health.network_metrics.current_block.toLocaleString()}</p>
              <p className="text-sm text-gray-600">
                {health.network_metrics.blocks_behind > 0 ? 
                  `${health.network_metrics.blocks_behind}블록 뒤쳐짐` : 
                  '동기화 완료'
                }
              </p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">평균 블록 시간</h3>
              <p className="text-lg font-bold">{health.network_metrics.avg_block_time_seconds.toFixed(1)}초</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">가스 가격</h3>
              <p className="text-lg font-bold">{health.network_metrics.gas_price_gwei.toFixed(1)} Gwei</p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">대기 중인 거래</h3>
              <p className="text-lg font-bold">{health.network_metrics.pending_transactions.toLocaleString()}</p>
            </div>
          </div>
          
          <div className="mt-4 grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">모니터링 노드</h3>
              <p className="text-lg font-bold">{health.network_metrics.total_nodes_monitored}</p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">정상 노드</h3>
              <p className="text-lg font-bold text-green-600">{health.network_metrics.healthy_nodes}</p>
            </div>
            
            <div className="bg-red-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">문제 노드</h3>
              <p className="text-lg font-bold text-red-600">
                {health.network_metrics.degraded_nodes + health.network_metrics.down_nodes}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* 노드 상태 */}
      {health?.nodes && health.nodes.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">노드 상태</h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">노드명</th>
                  <th className="text-left py-2">타입</th>
                  <th className="text-center py-2">상태</th>
                  <th className="text-right py-2">응답시간</th>
                  <th className="text-right py-2">가동률</th>
                  <th className="text-right py-2">블록 높이</th>
                  <th className="text-center py-2">동기화</th>
                  <th className="text-right py-2">피어</th>
                </tr>
              </thead>
              <tbody>
                {health.nodes.map((node: NodeInfo, idx) => (
                  <tr key={idx} className="border-b hover:bg-gray-50">
                    <td className="py-2 font-medium">{node.name}</td>
                    <td className="py-2">
                      <span className="px-2 py-1 rounded-full text-xs bg-gray-100">
                        {node.type}
                      </span>
                    </td>
                    <td className="py-2 text-center">
                      <span className={`px-2 py-1 rounded-full text-xs ${getStatusColor(node.status)}`}>
                        {node.status}
                      </span>
                    </td>
                    <td className="py-2 text-right">{node.response_time_ms}ms</td>
                    <td className="py-2 text-right">{node.uptime_percentage.toFixed(1)}%</td>
                    <td className="py-2 text-right">{node.block_height.toLocaleString()}</td>
                    <td className="py-2 text-center">
                      <span className={`px-2 py-1 rounded-full text-xs ${
                        node.syncing ? 'bg-yellow-100 text-yellow-800' : 'bg-green-100 text-green-800'
                      }`}>
                        {node.syncing ? '동기화중' : '완료'}
                      </span>
                    </td>
                    <td className="py-2 text-right">{node.peer_count}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Flashbots 상태 */}
      {health?.flashbots_status && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">Flashbots 릴레이 상태</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">릴레이 상태</h3>
              <span className={`px-3 py-1 rounded-full text-sm font-medium ${
                getStatusColor(health.flashbots_status.relay_status)
              }`}>
                {health.flashbots_status.relay_status}
              </span>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">번들 포함률</h3>
              <p className="text-lg font-bold">{(health.flashbots_status.bundle_inclusion_rate * 100).toFixed(1)}%</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">응답 시간</h3>
              <p className="text-lg font-bold">{health.flashbots_status.avg_bundle_response_time_ms}ms</p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">오늘 제출한 번들</h3>
              <p className="text-lg font-bold">{health.flashbots_status.total_bundles_submitted_today}</p>
              <p className="text-sm text-gray-600">성공: {health.flashbots_status.successful_bundles_today}</p>
            </div>
          </div>
          
          <div className="mt-4 grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">릴레이 가동률</h3>
              <p className="text-lg font-bold">{health.flashbots_status.relay_uptime_percentage.toFixed(1)}%</p>
            </div>
            
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">예상 릴레이 부하</h3>
              <p className="text-lg font-bold">{(health.flashbots_status.estimated_relay_load * 100).toFixed(1)}%</p>
            </div>
          </div>
        </div>
      )}

      {/* 외부 서비스 상태 */}
      {health?.external_services && health.external_services.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">외부 서비스 상태</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {health.external_services.map((service: ExternalServiceStatus, idx) => (
              <div key={idx} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium">{service.service_name}</h3>
                  <span className={`px-2 py-1 rounded-full text-xs ${getStatusColor(service.status)}`}>
                    {service.status}
                  </span>
                </div>
                <div className="text-sm space-y-1">
                  <div>카테고리: <span className="font-medium">{service.category}</span></div>
                  <div>응답시간: <span className="font-medium">{service.response_time_ms}ms</span></div>
                  <div>가동률: <span className="font-medium">{service.uptime_percentage.toFixed(1)}%</span></div>
                  <div>에러율: <span className="font-medium">{service.error_rate_percentage.toFixed(2)}%</span></div>
                  {service.rate_limit_remaining && (
                    <div>남은 한도: <span className="font-medium">{service.rate_limit_remaining}</span></div>
                  )}
                  <div>24h 인시던트: <span className="font-medium text-red-600">{service.incidents_24h}</span></div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 시스템 리소스 */}
      {health?.system_resources && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">시스템 리소스</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">CPU 사용률</h3>
              <p className={`text-lg font-bold ${getResourceUsageColor(safeNumber(health.system_resources?.cpu_usage_percentage))}`}>
                {safeNumber(health.system_resources?.cpu_usage_percentage).toFixed(1)}%
              </p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">메모리 사용률</h3>
              <p className={`text-lg font-bold ${getResourceUsageColor(safeNumber(health.system_resources?.memory_usage_percentage))}`}>
                {safeNumber(health.system_resources?.memory_usage_percentage).toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">
                {formatBytes(safeNumber(health.system_resources?.memory_used_mb) * 1024 * 1024)} / {formatBytes(safeNumber(health.system_resources?.memory_total_mb) * 1024 * 1024)}
              </p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">디스크 사용률</h3>
              <p className={`text-lg font-bold ${getResourceUsageColor(safeNumber(health.system_resources?.disk_usage_percentage))}`}>
                {safeNumber(health.system_resources?.disk_usage_percentage).toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">여유: {safeNumber(health.system_resources?.disk_free_gb).toFixed(1)}GB</p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">로드 평균</h3>
              <p className="text-lg font-bold">{safeNumber(health.system_resources?.load_average?.[0]).toFixed(2)}</p>
              <p className="text-sm text-gray-600">
                {health.system_resources?.load_average?.slice(0, 3).map((l: number) => safeNumber(l).toFixed(2)).join(' / ') || '0.00 / 0.00 / 0.00'}
              </p>
            </div>
          </div>
          
          <div className="mt-4 grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">네트워크 I/O</h3>
              <p className="text-sm">
                IN: <span className="font-medium">{safeNumber(health.system_resources?.network_in_mbps).toFixed(1)} Mbps</span><br/>
                OUT: <span className="font-medium">{safeNumber(health.system_resources?.network_out_mbps).toFixed(1)} Mbps</span>
              </p>
            </div>
            
            <div className="bg-gray-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">활성 연결</h3>
              <p className="text-lg font-bold">{safeNumber(health.system_resources?.active_connections)}</p>
            </div>
            
            {health.system_resources?.goroutines_count && (
              <div className="bg-gray-50 p-4 rounded-lg">
                <h3 className="text-sm font-medium text-gray-500 mb-1">고루틴 수</h3>
                <p className="text-lg font-bold">{safeNumber(health.system_resources?.goroutines_count)}</p>
                {health.system_resources?.heap_size_mb && (
                  <p className="text-sm text-gray-600">힙: {safeNumber(health.system_resources?.heap_size_mb).toFixed(1)}MB</p>
                )}
              </div>
            )}
          </div>
        </div>
      )}

      {/* 레이턴시 테스트 */}
      <div className="bg-white p-6 rounded-lg border">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">레이턴시 테스트</h2>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={latencyTestTarget}
              onChange={(e) => setLatencyTestTarget(e.target.value)}
              placeholder="테스트 대상 (예: 8.8.8.8)"
              className="px-3 py-1 border rounded text-sm"
            />
            <button
              onClick={handleLatencyTest}
              disabled={runningLatencyTest}
              className="px-3 py-1 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
            >
              {runningLatencyTest ? '테스트 중...' : '테스트 실행'}
            </button>
          </div>
        </div>
        
        {health?.latency_tests && health.latency_tests.length > 0 && (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">시간</th>
                  <th className="text-left py-2">대상</th>
                  <th className="text-right py-2">레이턴시</th>
                  <th className="text-right py-2">패킷 손실</th>
                  <th className="text-right py-2">지터</th>
                  <th className="text-center py-2">상태</th>
                </tr>
              </thead>
              <tbody>
                {health.latency_tests.map((test, idx) => (
                  <tr key={idx} className="border-b hover:bg-gray-50">
                    <td className="py-2">{new Date(test.timestamp).toLocaleString()}</td>
                    <td className="py-2 font-mono">{test.target}</td>
                    <td className="py-2 text-right font-medium">{test.latency_ms}ms</td>
                    <td className="py-2 text-right">{test.packet_loss_percentage.toFixed(1)}%</td>
                    <td className="py-2 text-right">{test.jitter_ms}ms</td>
                    <td className="py-2 text-center">
                      <span className={`px-2 py-1 rounded-full text-xs ${getStatusColor(test.status)}`}>
                        {test.status}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 네트워크 인시던트 */}
      {health?.network_incidents && health.network_incidents.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">네트워크 인시던트</h2>
          <div className="space-y-4">
            {health.network_incidents.map((incident) => (
              <div key={incident.id} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <span className={`px-2 py-1 rounded-full text-xs font-medium ${getSeverityColor(incident.severity)}`}>
                      {incident.severity}
                    </span>
                    <span className={`px-2 py-1 rounded-full text-xs ${getStatusColor(incident.status)}`}>
                      {incident.status}
                    </span>
                    <h3 className="font-medium">{incident.title}</h3>
                  </div>
                  {incident.status === 'open' && (
                    <button
                      onClick={() => handleAcknowledgeIncident(incident.id)}
                      className="px-3 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700"
                    >
                      확인
                    </button>
                  )}
                </div>
                
                <p className="text-sm text-gray-600 mb-2">{incident.description}</p>
                
                <div className="text-xs text-gray-500 space-y-1">
                  <div>시작: {new Date(incident.started_at).toLocaleString()}</div>
                  {incident.resolved_at && (
                    <div>해결: {new Date(incident.resolved_at).toLocaleString()}</div>
                  )}
                  <div>영향 받는 서비스: {incident.affected_services.join(', ')}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </main>
  );
}