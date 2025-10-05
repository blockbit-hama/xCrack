"use client";

import { useEffect, useState } from "react";
import {
  getNetworkHealth,
  runLatencyTest,
  type NetworkHealthDashboard
} from '@/lib/api';

export default function NetworkHealthPage() {
  const [health, setHealth] = useState<NetworkHealthDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
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
    const interval = setInterval(fetchData, 30000); // 30초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const handleLatencyTest = async () => {
    setRunningLatencyTest(true);
    try {
      const result = await runLatencyTest();
      if (result) {
        console.log('Latency test result:', result);
      }
    } catch (e) {
      console.error('Latency test failed:', e);
    } finally {
      setRunningLatencyTest(false);
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
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-3xl font-bold">네트워크 헬스 모니터</h1>
        <p className="text-gray-600 mt-1">네트워크 상태 및 성능 모니터링</p>
      </div>

      {/* 네트워크 상태 */}
      {health && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">상태</h3>
            <p className={`text-2xl font-bold ${health.status === 'healthy' ? 'text-green-600' : 'text-red-600'}`}>
              {health.status}
            </p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">레이턴시</h3>
            <p className="text-2xl font-bold">{health.latency}ms</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">가동시간</h3>
            <p className="text-2xl font-bold">{health.uptime.toFixed(2)}%</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">마지막 체크</h3>
            <p className="text-sm">{new Date(health.last_check).toLocaleString('ko-KR')}</p>
          </div>
        </div>
      )}

      {/* 레이턴시 테스트 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">레이턴시 테스트</h2>
        <button
          onClick={handleLatencyTest}
          disabled={runningLatencyTest}
          className={`px-4 py-2 rounded ${
            runningLatencyTest
              ? 'bg-gray-400 cursor-not-allowed'
              : 'bg-blue-500 hover:bg-blue-600 text-white'
          }`}
        >
          {runningLatencyTest ? '테스트 실행 중...' : '테스트 실행'}
        </button>
      </div>

      {/* 추가 정보 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">추가 정보</h2>
        <p className="text-gray-500">추가 네트워크 통계 준비 중입니다</p>
      </div>
    </main>
  );
}
