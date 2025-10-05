'use client';

import { useEffect, useState, useCallback } from 'react';
import { 
  getAlerts, 
  getAlertStats, 
  acknowledgeAlert, 
  acknowledgeAllAlerts, 
  dismissAlert,
  Alert, 
  AlertStats, 
  AlertSeverity,
  AlertCategory
} from '@/lib/api';

export default function AlertsPage() {
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const [stats, setStats] = useState<AlertStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<'all' | 'unacknowledged' | AlertSeverity | AlertCategory>('all');
  const [sortBy, setSortBy] = useState<'timestamp' | 'severity'>('timestamp');

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const [alertsData, statsData] = await Promise.all([
        getAlerts(filter === 'unacknowledged'),
        getAlertStats()
      ]);
      
      // alertsData가 배열인지 확인하고 안전하게 설정
      const safeAlerts = Array.isArray(alertsData) ? alertsData : [];
      setAlerts(safeAlerts);
      setStats(statsData);
      setError(null);
    } catch (err) {
      setError('Failed to fetch alerts');
      console.error('Alerts fetch error:', err);
    } finally {
      setLoading(false);
    }
  }, [filter]);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 10000); // 10초마다 업데이트
    return () => clearInterval(interval);
  }, [filter, fetchData]);

  const handleAcknowledgeAlert = async (alertId: string) => {
    const success = await acknowledgeAlert(alertId);
    if (success) {
      setAlerts(alerts.map(alert => 
        alert.id === alertId ? { ...alert, acknowledged: true } : alert
      ));
    }
  };

  const handleAcknowledgeAll = async () => {
    const success = await acknowledgeAllAlerts();
    if (success) {
      setAlerts(alerts.map(alert => ({ ...alert, acknowledged: true })));
    }
  };

  const handleDismissAlert = async (alertId: string) => {
    const success = await dismissAlert(alertId);
    if (success) {
      setAlerts(alerts.filter(alert => alert.id !== alertId));
    }
  };

  const getSeverityColor = (severity: AlertSeverity) => {
    const colors: Record<AlertSeverity, string> = {
      critical: 'bg-red-100 text-red-800 border-red-200',
      error: 'bg-orange-100 text-orange-800 border-orange-200',
      warning: 'bg-yellow-100 text-yellow-800 border-yellow-200',
      info: 'bg-gray-100 text-gray-800 border-gray-200'
    };
    return colors[severity] || colors.info;
  };

  const getCategoryIcon = (category: AlertCategory) => {
    const icons: Record<AlertCategory, string> = {
      system: '🖥️',
      performance: '⚡',
      security: '🛡️',
      strategy: '📈'
    };
    return icons[category] || '📋';
  };

  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString('ko-KR');
  };

  // alerts가 배열인지 확인하고 안전하게 필터링
  const safeAlerts = Array.isArray(alerts) ? alerts : [];
  
  const filteredAlerts = safeAlerts.filter(alert => {
    if (filter === 'all') return true;
    if (filter === 'unacknowledged') return !alert.acknowledged;
    if (['critical', 'high', 'medium', 'low', 'info'].includes(filter as string)) {
      return alert.severity === filter;
    }
    return alert.category === filter;
  });

  const sortedAlerts = [...filteredAlerts].sort((a, b) => {
    if (sortBy === 'timestamp') {
      return new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime();
    } else {
      const severityOrder: Record<AlertSeverity, number> = {
        critical: 4,
        error: 3,
        warning: 2,
        info: 0
      };
      return (severityOrder[b.severity] || 0) - (severityOrder[a.severity] || 0);
    }
  });

  if (loading) {
    return (
      <div className="p-6">
        <h1 className="text-3xl font-bold mb-6">알림 센터</h1>
        <div className="animate-pulse">Loading alerts...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <h1 className="text-3xl font-bold mb-6">알림 센터</h1>
        <div className="text-red-500">Error: {error}</div>
        <button 
          onClick={() => { setError(null); setLoading(true); fetchData(); }}
          className="mt-4 px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600"
        >
          재시도
        </button>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold">알림 센터</h1>
        <div className="flex items-center space-x-4">
          <button
            onClick={handleAcknowledgeAll}
            className="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600"
            disabled={safeAlerts.filter(a => !a.acknowledged).length === 0}
          >
            모두 확인
          </button>
          <div className="flex items-center space-x-2">
            <span className="text-sm">정렬:</span>
            <select 
              value={sortBy} 
              onChange={(e) => setSortBy(e.target.value as 'timestamp' | 'severity')}
              className="px-3 py-1 border rounded"
            >
              <option value="timestamp">시간순</option>
              <option value="severity">심각도순</option>
            </select>
          </div>
        </div>
      </div>

      {/* 알림 통계 */}
      {stats && (
        <div className="grid grid-cols-2 md:grid-cols-6 gap-4">
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">총 알림</h3>
            <p className="text-2xl font-bold">{(stats?.total || 0).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">활성</h3>
            <p className="text-2xl font-bold text-orange-600">{(stats?.active || 0).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">긴급</h3>
            <p className="text-2xl font-bold text-red-600">{(stats?.critical || 0).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">해결됨</h3>
            <p className="text-2xl font-bold text-green-600">{(stats?.resolved || 0).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">필터</h3>
            <p className="text-lg font-bold">{filter}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">정렬</h3>
            <p className="text-lg font-bold">{sortBy}</p>
          </div>
        </div>
      )}

      {/* 필터 */}
      <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
        <div className="flex flex-wrap gap-2">
          <button
            onClick={() => setFilter('all')}
            className={`px-3 py-1 rounded ${filter === 'all' ? 'bg-blue-500 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}`}
          >
            전체 ({safeAlerts.length})
          </button>
          <button
            onClick={() => setFilter('unacknowledged')}
            className={`px-3 py-1 rounded ${filter === 'unacknowledged' ? 'bg-blue-500 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}`}
          >
            미확인 ({safeAlerts.filter(a => !a.acknowledged).length})
          </button>
          
          {/* 심각도 필터 */}
          {(['critical', 'high', 'medium', 'low', 'info'] as AlertSeverity[]).map(severity => (
            <button
              key={severity}
              onClick={() => setFilter(severity)}
              className={`px-3 py-1 rounded ${filter === severity ? 'bg-blue-500 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}`}
            >
              {severity} ({safeAlerts.filter(a => a.severity === severity).length})
            </button>
          ))}
          
          {/* 범주 필터 */}
          {(['system', 'performance', 'security', 'strategy', 'network', 'gas', 'profit'] as AlertCategory[]).map(category => (
            <button
              key={category}
              onClick={() => setFilter(category)}
              className={`px-3 py-1 rounded ${filter === category ? 'bg-blue-500 text-white' : 'bg-gray-200 text-gray-700 hover:bg-gray-300'}`}
            >
              {getCategoryIcon(category)} {category} ({safeAlerts.filter(a => a.category === category).length})
            </button>
          ))}
        </div>
      </div>

      {/* 알림 목록 */}
      <div className="space-y-4">
        {sortedAlerts.map((alert) => (
          <div
            key={alert.id}
            className={`bg-white dark:bg-gray-800 p-6 rounded-lg shadow border-l-4 ${
              alert.acknowledged ? 'opacity-60' : ''
            } ${
              alert.severity === 'critical' ? 'border-red-500' :
              alert.severity === 'error' ? 'border-orange-500' :
              alert.severity === 'warning' ? 'border-yellow-500' :
              'border-gray-500'
            }`}
          >
            <div className="flex items-start justify-between">
              <div className="flex-1">
                <div className="flex items-center space-x-3 mb-2">
                  <span className="text-lg">{getCategoryIcon(alert.category)}</span>
                  <h3 className="text-lg font-semibold">{alert.title}</h3>
                  <span className={`px-2 py-1 rounded-full text-xs font-medium border ${getSeverityColor(alert.severity)}`}>
                    {alert.severity.toUpperCase()}
                  </span>
                  {alert.acknowledged && (
                    <span className="px-2 py-1 rounded-full text-xs font-medium bg-green-100 text-green-800 border border-green-200">
                      ✓ 확인됨
                    </span>
                  )}
                </div>
                <p className="text-gray-600 dark:text-gray-300 mb-3">{alert.message}</p>
                <div className="flex items-center space-x-4 text-sm text-gray-500">
                  <span>카테고리: {alert.category}</span>
                  <span>시간: {formatTimestamp(alert.timestamp)}</span>
                  {alert.resolved && (
                    <span className="text-green-600">✓ 해결됨</span>
                  )}
                </div>
              </div>
              <div className="flex space-x-2 ml-4">
                {!alert.acknowledged && (
                  <button
                    onClick={() => handleAcknowledgeAlert(alert.id)}
                    className="px-3 py-1 bg-green-500 text-white rounded hover:bg-green-600 text-sm"
                  >
                    확인
                  </button>
                )}
                <button
                  onClick={() => handleDismissAlert(alert.id)}
                  className="px-3 py-1 bg-red-500 text-white rounded hover:bg-red-600 text-sm"
                >
                  삭제
                </button>
              </div>
            </div>
          </div>
        ))}
        
        {sortedAlerts.length === 0 && (
          <div className="bg-white dark:bg-gray-800 p-8 rounded-lg shadow text-center">
            <p className="text-gray-500">표시할 알림이 없습니다.</p>
          </div>
        )}
      </div>
    </div>
  );
}
