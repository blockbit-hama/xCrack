"use client";

import { useEffect, useState } from "react";
import {
  getOnChainAnalytics,
  type OnChainAnalytics
} from '../../lib/api';

export default function OnChainPage() {
  const [analytics, setAnalytics] = useState<OnChainAnalytics | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      setError("");
      try {
        const analyticsData = await getOnChainAnalytics();
        setAnalytics(analyticsData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 30000);
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">온체인 분석</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">온체인 분석</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-3xl font-bold">온체인 분석</h1>
        <p className="text-gray-600 mt-1">블록체인 데이터 분석 및 모니터링</p>
      </div>

      {/* 온체인 분석 메트릭 */}
      {analytics && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">총 거래량</h3>
            <p className="text-2xl font-bold">${parseFloat(analytics.total_volume).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">총 수수료</h3>
            <p className="text-2xl font-bold">${parseFloat(analytics.total_fees).toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">활성 주소</h3>
            <p className="text-2xl font-bold">{analytics.active_addresses.toLocaleString()}</p>
          </div>
          <div className="bg-white dark:bg-gray-800 p-4 rounded-lg shadow">
            <h3 className="text-sm font-medium text-gray-500">트랜잭션 수</h3>
            <p className="text-2xl font-bold">{analytics.transaction_count.toLocaleString()}</p>
          </div>
        </div>
      )}

      {/* 추가 정보 */}
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">추가 정보</h2>
        <p className="text-gray-500">추가 온체인 분석 데이터 준비 중입니다</p>
      </div>
    </main>
  );
}
