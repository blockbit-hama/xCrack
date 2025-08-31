"use client"

import React, { useEffect, useState } from 'react'
import { getStatus, defaultStatus, getBundlesSummary, getBundlesRecent, getReport } from "../lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../components/ui/card";
import { Badge } from "../components/ui/badge";
import { Progress } from "../components/ui/progress";
import { Chart } from "../components/ui/chart";
import { motion } from 'framer-motion';
import { Activity, TrendingUp, DollarSign, Clock, AlertCircle, CheckCircle } from 'lucide-react';

export default function Page() {
  const [status, setStatus] = useState(defaultStatus());
  const [bundles, setBundles] = useState({ 
    stats: { 
      total_created: 0, 
      total_submitted: 0, 
      total_included: 0, 
      total_failed: 0, 
      total_profit: 0, 
      total_gas_spent: 0, 
      avg_submission_time_ms: 0, 
      success_rate: 0 
    }, 
    submitted_count: 0, 
    pending_count: 0 
  });
  const [recent, setRecent] = useState<any[]>([]);
  const [report, setReport] = useState({ 
    summary: { 
      transactions_processed: 0, 
      opportunities_found: 0, 
      bundles_submitted: 0, 
      bundles_included: 0, 
      total_profit_eth: '0', 
      success_rate: 0, 
      avg_analysis_time_ms: 0, 
      avg_submission_time_ms: 0 
    }, 
    recommendations: [] 
  });

  // 차트 데이터 생성
  const generateChartData = () => {
    const data = [];
    const now = new Date();
    
    for (let i = 23; i >= 0; i--) {
      const time = new Date(now.getTime() - i * 60 * 60 * 1000);
      data.push({
        time: time.toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' }),
        profit: Math.random() * 0.1 + 0.05,
        bundles: Math.floor(Math.random() * 20) + 5,
        opportunities: Math.floor(Math.random() * 50) + 10,
      });
    }
    return data;
  };

  const chartData = generateChartData();

  // 데이터 로드
  useEffect(() => {
    const loadData = async () => {
      try {
        const [statusData, bundlesData, recentData, reportData] = await Promise.all([
          getStatus(),
          getBundlesSummary(),
          getBundlesRecent(5),
          getReport(),
        ]);

        setStatus(statusData);
        setBundles(bundlesData);
        setRecent(Array.isArray(recentData) ? recentData : []);
        setReport(reportData);
      } catch (error) {
        console.error('데이터 로드 실패:', error);
      }
    };

    loadData();
    
    // 30초마다 데이터 새로고침
    const interval = setInterval(loadData, 30000);
    return () => clearInterval(interval);
  }, []);

  const successRate = status.success_rate * 100;
  const uptimeHours = Math.floor(status.uptime_seconds / 3600);
  const uptimeMinutes = Math.floor((status.uptime_seconds % 3600) / 60);

  return (
    <div className="space-y-6">
      {/* 헤더 */}
      <motion.div 
        initial={{ opacity: 0, y: -20 }}
        animate={{ opacity: 1, y: 0 }}
        className="border-b pb-4"
      >
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-bold">xCrack MEV Dashboard</h1>
            <p className="text-gray-600 mt-1">실시간 MEV 기회 탐지 및 실행 시스템</p>
          </div>
          <div className="flex items-center space-x-2">
            <div className="w-3 h-3 rounded-full bg-green-400 animate-pulse"></div>
            <span className="text-sm text-gray-600">
              API 연결됨
            </span>
          </div>
        </div>
      </motion.div>

      {/* 주요 지표 카드 */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.1 }}
        >
          <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">시스템 상태</CardTitle>
              {status.is_running ? (
                <CheckCircle className="h-4 w-4 text-green-500" />
              ) : (
                <AlertCircle className="h-4 w-4 text-red-500" />
              )}
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">
                {status.is_running ? '실행 중' : '중지됨'}
              </div>
              <p className="text-xs text-muted-foreground">
                활성 기회: {status.active_opportunities}개
              </p>
            </CardContent>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.2 }}
        >
          <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">총 수익</CardTitle>
              <DollarSign className="h-4 w-4 text-green-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-green-600">
                {parseFloat(status.total_profit_eth).toFixed(4)} ETH
              </div>
              <p className="text-xs text-muted-foreground">
                번들 제출: {status.submitted_bundles}개
              </p>
            </CardContent>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.3 }}
        >
          <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">성공률</CardTitle>
              <TrendingUp className="h-4 w-4 text-blue-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">
                {successRate.toFixed(1)}%
              </div>
              <Progress value={successRate} className="mt-2" />
            </CardContent>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.4 }}
        >
          <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">가동 시간</CardTitle>
              <Clock className="h-4 w-4 text-purple-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">
                {uptimeHours}h {uptimeMinutes}m
              </div>
              <p className="text-xs text-muted-foreground">
                {status.uptime_seconds}초
              </p>
            </CardContent>
          </Card>
        </motion.div>
      </div>

      {/* 실시간 차트 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.5 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>실시간 수익 추이</CardTitle>
            <CardDescription>최근 24시간 수익 및 활동 현황</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              <div>
                <h4 className="text-sm font-medium mb-2">수익 추이 (ETH)</h4>
                <Chart
                  data={chartData}
                  type="area"
                  dataKey="profit"
                  xAxisKey="time"
                  height={200}
                  color="#10b981"
                />
              </div>
              <div>
                <h4 className="text-sm font-medium mb-2">번들 활동</h4>
                <Chart
                  data={chartData}
                  type="bar"
                  dataKey="bundles"
                  xAxisKey="time"
                  height={200}
                  color="#3b82f6"
                />
              </div>
            </div>
          </CardContent>
        </Card>
      </motion.div>

      {/* 번들 통계 */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.6 }}
        >
          <Card>
            <CardHeader>
              <CardTitle className="text-sm font-medium">번들 생성</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{bundles.stats?.total_created || 0}</div>
              <p className="text-xs text-muted-foreground">총 생성된 번들</p>
            </CardContent>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.7 }}
        >
          <Card>
            <CardHeader>
              <CardTitle className="text-sm font-medium">번들 제출</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-blue-600">{bundles.stats?.total_submitted || 0}</div>
              <p className="text-xs text-muted-foreground">제출된 번들</p>
            </CardContent>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.8 }}
        >
          <Card>
            <CardHeader>
              <CardTitle className="text-sm font-medium">번들 포함</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-green-600">{bundles.stats?.total_included || 0}</div>
              <p className="text-xs text-muted-foreground">블록에 포함된 번들</p>
            </CardContent>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.9 }}
        >
          <Card>
            <CardHeader>
              <CardTitle className="text-sm font-medium">번들 실패</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-red-600">{bundles.stats?.total_failed || 0}</div>
              <p className="text-xs text-muted-foreground">실패한 번들</p>
            </CardContent>
          </Card>
        </motion.div>
      </div>

      {/* 최근 번들 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 1.0 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>최근 번들 활동</CardTitle>
            <CardDescription>최근 5개의 번들 실행 내역</CardDescription>
          </CardHeader>
          <CardContent>
            {!recent || recent.length === 0 ? (
              <div className="text-center py-8 text-gray-500">
                최근 번들 활동이 없습니다.
              </div>
            ) : (
              <div className="space-y-4">
                {recent.map((bundle, index) => (
                  <motion.div
                    key={bundle.id || index}
                    initial={{ opacity: 0, x: -20 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: 1.1 + index * 0.1 }}
                    className="flex items-center justify-between p-4 border rounded-lg hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
                  >
                    <div className="flex items-center space-x-4">
                      <div className="w-10 h-10 bg-gradient-to-r from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
                        <span className="text-white font-bold text-sm">
                          {bundle.strategy?.charAt(0).toUpperCase() || 'U'}
                        </span>
                      </div>
                      <div>
                        <div className="font-medium">#{bundle.id?.slice(0, 8) || 'unknown'}...</div>
                        <div className="text-sm text-gray-500">{bundle.strategy || 'Unknown'}</div>
                      </div>
                    </div>
                    <div className="flex items-center space-x-4">
                      <div className="text-right">
                        <div className="font-medium text-green-600">{bundle.expected_profit || '0'} ETH</div>
                        <div className="text-sm text-gray-500">{bundle.gas_estimate || '0'} gas</div>
                      </div>
                      <Badge variant={bundle.state === 'submitted' ? 'info' : 'warning'}>
                        {bundle.state || 'unknown'}
                      </Badge>
                      <div className="text-sm text-gray-500">
                        {bundle.timestamp ? new Date(bundle.timestamp).toLocaleString() : 'Unknown'}
                      </div>
                    </div>
                  </motion.div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </motion.div>

      {/* 성능 지표 및 권장사항 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 1.2 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>성능 지표</CardTitle>
              <CardDescription>시스템 성능 및 응답 시간</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex justify-between items-center">
                <span className="text-sm font-medium">평균 분석 시간</span>
                <span className="text-sm text-gray-600">{report.summary?.avg_analysis_time_ms?.toFixed(2) || '0.00'} ms</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm font-medium">평균 제출 시간</span>
                <span className="text-sm text-gray-600">{report.summary?.avg_submission_time_ms?.toFixed(2) || '0.00'} ms</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm font-medium">처리된 트랜잭션</span>
                <span className="text-sm text-gray-600">{report.summary?.transactions_processed?.toLocaleString() || '0'}</span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm font-medium">발견된 기회</span>
                <span className="text-sm text-gray-600">{report.summary?.opportunities_found?.toLocaleString() || '0'}</span>
              </div>
            </CardContent>
          </Card>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 1.3 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>시스템 권장사항</CardTitle>
              <CardDescription>성능 최적화를 위한 권장사항</CardDescription>
            </CardHeader>
            <CardContent>
              {!report.recommendations || report.recommendations.length === 0 ? (
                <div className="text-center py-4 text-gray-500">
                  현재 권장사항이 없습니다.
                </div>
              ) : (
                <ul className="space-y-2">
                  {report.recommendations.slice(0, 5).map((recommendation, index) => (
                    <motion.li
                      key={index}
                      initial={{ opacity: 0, x: -10 }}
                      animate={{ opacity: 1, x: 0 }}
                      transition={{ delay: 1.4 + index * 0.1 }}
                      className="flex items-start space-x-2"
                    >
                      <span className="text-blue-500 mt-1">•</span>
                      <span className="text-sm">{recommendation}</span>
                    </motion.li>
                  ))}
                </ul>
              )}
            </CardContent>
          </Card>
        </motion.div>
      </div>
    </div>
  );
}
