'use client';

import { useEffect, useState } from 'react';
import { getMempoolStatus, getMempoolTransactions, MempoolStatus, MempoolTransaction } from '@/lib/api';

export default function MempoolPage() {
  const [status, setStatus] = useState<MempoolStatus | null>(null);
  const [transactions, setTransactions] = useState<MempoolTransaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    try {
      const [statusData, txData] = await Promise.all([
        getMempoolStatus(),
        getMempoolTransactions(50)
      ]);
      
      setStatus(statusData);
      setTransactions(txData);
      setError(null);
    } catch (err) {
      setError('Failed to fetch mempool data');
      console.error('Mempool fetch error:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 2000); // 2초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const formatEth = (wei: string) => {
    try {
      const value = BigInt(wei);
      const eth = Number(value) / 1e18;
      return eth.toFixed(6);
    } catch {
      return '0.000000';
    }
  };

  const formatGwei = (wei: string) => {
    try {
      const value = BigInt(wei);
      const gwei = Number(value) / 1e9;
      return Math.round(gwei);
    } catch {
      return 0;
    }
  };

  const safeNumber = (value: any, defaultValue: number = 0): number => {
    if (value === undefined || value === null || isNaN(Number(value))) {
      return defaultValue;
    }
    return Number(value);
  };

  if (loading) {
    return (
      <div className="p-6">
        <h1 className="text-3xl font-bold mb-6">실시간 멤풀 모니터</h1>
        <div className="animate-pulse">Loading mempool data...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <h1 className="text-3xl font-bold mb-6">실시간 멤풀 모니터</h1>
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
        <h1 className="text-3xl font-bold">실시간 멤풀 모니터</h1>
        <div className="flex items-center space-x-2">
          <div className={`w-3 h-3 rounded-full ${status?.is_monitoring && status?.connected ? 'bg-green-500' : 'bg-red-500'}`}></div>
          <span className="text-sm">
            {status?.is_monitoring && status?.connected ? '연결됨' : '연결 끊김'}
          </span>
        </div>
      </div>

      {/* 멤풀 통계 */}
      {status && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">총 트랜잭션</h3>
            <p className="text-2xl font-bold">{safeNumber(status.stats?.total_transactions).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">대기 중</h3>
            <p className="text-2xl font-bold">{safeNumber(status.stats?.pending_transactions).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">TPS</h3>
            <p className="text-2xl font-bold">{safeNumber(status.stats?.transactions_per_second, 0).toFixed(1)}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">평균 가스</h3>
            <p className="text-2xl font-bold">{formatGwei(status.stats?.avg_gas_price || '0')} Gwei</p>
          </div>
        </div>
      )}

      {/* 가스 가격 정보 */}
      {status && (
        <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
          <h2 className="text-xl font-semibold mb-4">가스 가격 분포</h2>
          <div className="grid grid-cols-3 gap-4">
            <div className="text-center">
              <h4 className="text-sm font-medium text-gray-500">최소</h4>
              <p className="text-lg font-bold text-green-600">{formatGwei(status.stats.min_gas_price)} Gwei</p>
            </div>
            <div className="text-center">
              <h4 className="text-sm font-medium text-gray-500">평균</h4>
              <p className="text-lg font-bold text-blue-600">{formatGwei(status.stats.avg_gas_price)} Gwei</p>
            </div>
            <div className="text-center">
              <h4 className="text-sm font-medium text-gray-500">최대</h4>
              <p className="text-lg font-bold text-red-600">{formatGwei(status.stats.max_gas_price)} Gwei</p>
            </div>
          </div>
        </div>
      )}

      {/* DEX 및 MEV 통계 */}
      {status && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
            <h3 className="text-lg font-semibold mb-2">DEX 트랜잭션</h3>
            <p className="text-3xl font-bold text-blue-500">{safeNumber(status.stats?.dex_transactions).toLocaleString()}</p>
            <p className="text-sm text-gray-500 mt-1">
              전체의 {safeNumber(status.stats?.total_transactions) > 0 ? 
                ((safeNumber(status.stats?.dex_transactions) / safeNumber(status.stats?.total_transactions)) * 100).toFixed(1) : 0}%
            </p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
            <h3 className="text-lg font-semibold mb-2">MEV 기회</h3>
            <p className="text-3xl font-bold text-orange-500">{safeNumber(status.stats?.mev_opportunities).toLocaleString()}</p>
            <p className="text-sm text-gray-500 mt-1">
              DEX 트랜잭션의 {safeNumber(status.stats?.dex_transactions) > 0 ? 
                ((safeNumber(status.stats?.mev_opportunities) / safeNumber(status.stats?.dex_transactions)) * 100).toFixed(1) : 0}%
            </p>
          </div>
        </div>
      )}

      {/* 최근 트랜잭션 목록 */}
      <div className="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div className="p-6 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-xl font-semibold">최근 트랜잭션</h2>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-gray-50 dark:bg-gray-700">
              <tr>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Hash</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">From</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">Value</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">가스</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">타입</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">MEV</th>
              </tr>
            </thead>
            <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {transactions.slice(0, 20).map((tx) => (
                <tr key={tx.hash} className="hover:bg-gray-50 dark:hover:bg-gray-700">
                  <td className="px-6 py-4 whitespace-nowrap">
                    <span className="text-sm font-mono text-blue-600">
                      {tx.hash.slice(0, 10)}...{tx.hash.slice(-8)}
                    </span>
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap">
                    <span className="text-sm font-mono text-gray-600">
                      {tx.from.slice(0, 6)}...{tx.from.slice(-4)}
                    </span>
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap">
                    <span className="text-sm">{formatEth(tx.value)} ETH</span>
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap">
                    <span className="text-sm">{formatGwei(tx.gas_price)} Gwei</span>
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap">
                    {tx.decoded_type && (
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
                        {tx.decoded_type}
                      </span>
                    )}
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap">
                    {tx.potential_mev && (
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-orange-100 text-orange-800">
                        MEV
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {transactions.length === 0 && (
          <div className="p-6 text-center text-gray-500">
            트랜잭션 데이터가 없습니다.
          </div>
        )}
      </div>
    </div>
  );
}