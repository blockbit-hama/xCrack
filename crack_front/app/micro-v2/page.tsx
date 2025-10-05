"use client"

import React, { useEffect, useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from "../../components/ui/card";
import {
  getMicroArbitrageV2Dashboard,
  MicroArbitrageV2Dashboard
} from "../../lib/api";

export default function MicroArbitrageV2Page() {
  const [dashboard, setDashboard] = useState<MicroArbitrageV2Dashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const loadData = async () => {
      try {
        const dashboardData = await getMicroArbitrageV2Dashboard();
        setDashboard(dashboardData);
        setError("");
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로 아비트래지 v2.0</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">마이크로 아비트래지 v2.0</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-3xl font-bold">마이크로 아비트래지 v2.0</h1>
        <p className="text-gray-600 mt-1">지능형 자금 조달 시스템</p>
      </div>

      {/* 주요 메트릭 */}
      {dashboard && (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-sm">총 거래</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold">{dashboard.total_trades}</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle className="text-sm">총 수익</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold text-green-600">${parseFloat(dashboard.total_profit).toFixed(2)}</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle className="text-sm">성공률</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold">{(dashboard.success_rate * 100).toFixed(1)}%</p>
            </CardContent>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle className="text-sm">평균 수익/거래</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold text-blue-600">${parseFloat(dashboard.avg_profit_per_trade).toFixed(2)}</p>
            </CardContent>
          </Card>
        </div>
      )}

      {/* 추가 정보 */}
      <Card>
        <CardHeader>
          <CardTitle>상세 정보</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-gray-500">추가 통계 데이터 준비 중입니다</p>
        </CardContent>
      </Card>
    </main>
  );
}
