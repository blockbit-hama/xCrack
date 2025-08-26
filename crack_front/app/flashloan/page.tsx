'use client';

import { useState, useEffect } from 'react';
import { getFlashloanDashboard, type FlashloanDashboard } from '@/lib/api';

export default function FlashloanPage() {
  const [dashboard, setDashboard] = useState<FlashloanDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'overview' | 'history' | 'contracts' | 'code'>('overview');
  const [selectedContract, setSelectedContract] = useState<string>('flashloan_executor');

  useEffect(() => {
    let mounted = true;
    
    const fetchData = async () => {
      try {
        const data = await getFlashloanDashboard();
        if (mounted && data) {
          setDashboard(data);
        }
      } catch (error) {
        console.error('Failed to fetch flashloan dashboard:', error);
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    };
    
    fetchData();
    const interval = setInterval(fetchData, 5000);
    
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  const formatAmount = (amount: string) => {
    const num = parseFloat(amount);
    if (num >= 1000000) return (num / 1000000).toFixed(2) + 'M';
    if (num >= 1000) return (num / 1000).toFixed(2) + 'K';
    return num.toFixed(2);
  };

  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'success': return 'text-green-600';
      case 'failed': return 'text-red-600';
      case 'pending': return 'text-yellow-600';
      default: return 'text-gray-600';
    }
  };

  const getProviderColor = (available: boolean) => {
    return available ? 'text-green-600' : 'text-red-600';
  };

  if (loading) {
    return (
      <div className="p-6">
        <h1 className="text-2xl font-bold mb-6">플래시론 대시보드</h1>
        <div className="text-center py-8">로딩 중...</div>
      </div>
    );
  }

  if (!dashboard) {
    return (
      <div className="p-6">
        <h1 className="text-2xl font-bold mb-6">플래시론 대시보드</h1>
        <div className="text-center py-8 text-red-600">데이터를 불러올 수 없습니다.</div>
      </div>
    );
  }

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-6">플래시론 대시보드</h1>
      
      {/* Tab Navigation */}
      <div className="flex space-x-1 mb-6 border-b">
        {[
          { id: 'overview', label: '개요' },
          { id: 'history', label: '거래 내역' },
          { id: 'contracts', label: '컨트랙트' },
          { id: 'code', label: '스마트 컨트랙트 코드' }
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id as any)}
            className={`px-4 py-2 rounded-t-lg transition-colors ${
              activeTab === tab.id
                ? 'bg-blue-500 text-white border-b-2 border-blue-500'
                : 'bg-gray-200 text-gray-700 hover:bg-gray-300'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Overview Tab */}
      {activeTab === 'overview' && (
        <div className="space-y-6">
          {/* Performance Metrics */}
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">성과 지표</h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <div>
                <div className="text-sm text-gray-500">총 플래시론 수</div>
                <div className="text-2xl font-bold">{dashboard.performance_metrics.total_flashloans}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">총 거래량</div>
                <div className="text-2xl font-bold">{formatAmount(dashboard.performance_metrics.total_volume.replace(' USD', ''))}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">총 수익</div>
                <div className="text-2xl font-bold text-green-600">{formatAmount(dashboard.performance_metrics.total_profit.replace(' USD', ''))}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">성공률</div>
                <div className="text-2xl font-bold">{(dashboard.performance_metrics.success_rate * 100).toFixed(1)}%</div>
              </div>
            </div>
          </div>

          {/* Flashloan Providers */}
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">플래시론 제공업체</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {Object.entries(dashboard.flashloan_providers).map(([name, provider]) => (
                <div key={name} className="border rounded-lg p-4">
                  <div className="flex items-center justify-between mb-2">
                    <h3 className="font-medium capitalize">{name.replace('_', ' ')}</h3>
                    <span className={`text-sm font-medium ${getProviderColor(provider.available)}`}>
                      {provider.available ? '사용 가능' : '사용 불가'}
                    </span>
                  </div>
                  <div className="text-sm text-gray-600 space-y-1">
                    <div>최대 금액: {provider.max_amount}</div>
                    <div>수수료: {provider.fee_rate}</div>
                    <div>가스 비용: {provider.gas_cost}</div>
                    <div>마지막 업데이트: {formatTime(provider.last_update)}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Gas Analytics */}
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">가스 분석</h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <div>
                <div className="text-sm text-gray-500">평균 가스 사용량</div>
                <div className="text-lg font-bold">{dashboard.gas_analytics.avg_gas_per_flashloan}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">최대 가스 사용량</div>
                <div className="text-lg font-bold">{dashboard.gas_analytics.most_expensive_flashloan}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">최소 가스 사용량</div>
                <div className="text-lg font-bold">{dashboard.gas_analytics.cheapest_flashloan}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">최적화 절약률</div>
                <div className="text-lg font-bold text-green-600">{dashboard.gas_analytics.gas_optimization_savings}</div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* History Tab */}
      {activeTab === 'history' && (
        <div className="bg-white rounded-lg shadow overflow-hidden">
          <h2 className="text-lg font-semibold p-6 border-b">최근 플래시론 거래</h2>
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">TX Hash</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">시간</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">제공업체</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">토큰/금액</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">전략</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">수익</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">상태</th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {dashboard.recent_flashloans.map((tx, index) => (
                  <tr key={index}>
                    <td className="px-6 py-4 whitespace-nowrap text-sm font-mono text-gray-900">
                      {tx.tx_hash}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {formatTime(tx.timestamp)}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900 capitalize">
                      {tx.provider.replace('_', ' ')}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {formatAmount(tx.amount)} {tx.token}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900 capitalize">
                      {tx.strategy}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm font-medium">
                      <span className={parseFloat(tx.profit) >= 0 ? 'text-green-600' : 'text-red-600'}>
                        {parseFloat(tx.profit) >= 0 ? '+' : ''}{tx.profit} USD
                      </span>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <span className={`inline-flex px-2 py-1 text-xs font-semibold rounded-full ${
                        tx.status === 'success' ? 'bg-green-100 text-green-800' :
                        tx.status === 'failed' ? 'bg-red-100 text-red-800' :
                        'bg-yellow-100 text-yellow-800'
                      }`}>
                        {tx.status === 'success' ? '성공' : tx.status === 'failed' ? '실패' : '대기중'}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Contracts Tab */}
      {activeTab === 'contracts' && (
        <div className="bg-white rounded-lg shadow p-6">
          <h2 className="text-lg font-semibold mb-4">플래시론 컨트랙트</h2>
          <div className="space-y-4">
            {Object.entries(dashboard.flashloan_contracts).map(([name, contract]) => (
              <div key={name} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium">{contract.name}</h3>
                  <div className="flex items-center space-x-2">
                    {contract.verified && (
                      <span className="text-green-600 text-sm">✓ 검증됨</span>
                    )}
                    {contract.proxy && (
                      <span className="bg-blue-100 text-blue-800 px-2 py-1 rounded text-xs">Proxy</span>
                    )}
                  </div>
                </div>
                <div className="text-sm text-gray-600 space-y-1">
                  <div className="font-mono">{contract.address}</div>
                  {contract.implementation && (
                    <div>
                      <span className="text-gray-500">구현체:</span>
                      <span className="font-mono ml-2">{contract.implementation}</span>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Smart Contract Code Tab */}
      {activeTab === 'code' && (
        <div className="bg-white rounded-lg shadow">
          <div className="p-6 border-b">
            <h2 className="text-lg font-semibold mb-4">스마트 컨트랙트 소스 코드</h2>
            <select
              value={selectedContract}
              onChange={(e) => setSelectedContract(e.target.value)}
              className="border rounded px-3 py-2 text-sm"
            >
              {Object.entries(dashboard.smart_contracts).map(([name, contract]) => (
                <option key={name} value={name}>
                  {name.replace('_', ' ').replace(/\b\w/g, l => l.toUpperCase())} (Solidity {contract.solidity_version})
                </option>
              ))}
            </select>
          </div>
          
          <div className="p-6">
            <pre className="bg-gray-50 p-4 rounded-lg overflow-x-auto text-sm font-mono whitespace-pre-wrap">
              {dashboard.smart_contracts[selectedContract]?.source_code.replace(/\\n/g, '\n')}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}