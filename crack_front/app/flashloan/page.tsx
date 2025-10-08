'use client';

import { useState, useEffect } from 'react';
import { getFlashloanDashboard, type FlashloanDashboard } from '../../lib/api';

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
                <div className="text-sm text-gray-500">활성 플래시론</div>
                <div className="text-2xl font-bold">{dashboard.active_loans}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">총 거래량</div>
                <div className="text-2xl font-bold">{formatAmount(dashboard.total_volume)}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">총 수익</div>
                <div className="text-2xl font-bold text-green-600">{formatAmount(dashboard.total_profit)}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">성공률</div>
                <div className="text-2xl font-bold">{(dashboard.success_rate * 100).toFixed(1)}%</div>
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
                <tr>
                  <td colSpan={7} className="px-6 py-4 text-center text-gray-500">
                    플래시론 기록이 없습니다
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Other tabs */}
      {(activeTab === 'contracts' || activeTab === 'code') && (
        <div className="bg-white rounded-lg shadow p-6">
          <div className="text-center text-gray-500">
            {activeTab === 'contracts' ? '컨트랙트 정보' : '코드 예시'} 준비 중입니다
          </div>
        </div>
      )}
    </div>
  );
}