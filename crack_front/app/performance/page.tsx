'use client';

import { useEffect, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { Progress } from "../../components/ui/progress";
import { 
  getDetailedPerformance, 
  getReport, 
  type DetailedPerformance, 
  type PerformanceReport 
} from '../../lib/api';
import { motion } from 'framer-motion';
import { 
  Cpu, 
  MemoryStick, 
  Network, 
  Clock, 
  TrendingUp, 
  AlertTriangle,
  CheckCircle 
} from 'lucide-react';

export default function PerformancePage() {
  const [detailedPerf, setDetailedPerf] = useState<DetailedPerformance | null>(null);
  const [report, setReport] = useState<PerformanceReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const loadData = async () => {
      setLoading(true);
      setError("");
      try {
        const [perfData, reportData] = await Promise.all([
          getDetailedPerformance(),
          getReport()
        ]);
        setDetailedPerf(perfData);
        setReport(reportData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    loadData();
    const interval = setInterval(loadData, 10000); // 10초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">성능 분석</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">성능 분석</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      {/* 헤더 */}
      <motion.div 
        initial={{ opacity: 0, y: -20 }}
        animate={{ opacity: 1, y: 0 }}
        className="border-b pb-4"
      >
        <h1 className="text-3xl font-bold">성능 분석</h1>
        <p className="text-gray-600 mt-1">시스템 성능 및 실행 통계 모니터링</p>
      </motion.div>

      {/* 상세 성능 지표 */}
      {detailedPerf && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <motion.div
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ delay: 0.1 }}
          >
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">CPU 사용률</CardTitle>
                <Cpu className="h-4 w-4 text-blue-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{detailedPerf.cpu_usage.toFixed(1)}%</div>
                <Progress value={detailedPerf.cpu_usage} className="mt-2" />
                <p className="text-xs text-muted-foreground mt-1">
                  {detailedPerf.cpu_usage > 80 ? '높음' : detailedPerf.cpu_usage > 50 ? '보통' : '낮음'}
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
                <CardTitle className="text-sm font-medium">메모리 사용률</CardTitle>
                <MemoryStick className="h-4 w-4 text-green-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{detailedPerf.memory_usage.toFixed(1)}%</div>
                <Progress value={detailedPerf.memory_usage} className="mt-2" />
                <p className="text-xs text-muted-foreground mt-1">
                  {detailedPerf.memory_usage > 80 ? '높음' : detailedPerf.memory_usage > 50 ? '보통' : '낮음'}
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
                <CardTitle className="text-sm font-medium">네트워크 지연시간</CardTitle>
                <Network className="h-4 w-4 text-orange-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{detailedPerf.network_latency.toFixed(0)}ms</div>
                <p className="text-xs text-muted-foreground mt-1">
                  {detailedPerf.network_latency < 100 ? '우수' : detailedPerf.network_latency < 300 ? '보통' : '느림'}
                </p>
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
                <CardTitle className="text-sm font-medium">응답 시간</CardTitle>
                <Clock className="h-4 w-4 text-purple-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{detailedPerf.response_time.toFixed(0)}ms</div>
                <p className="text-xs text-muted-foreground mt-1">
                  평균 API 응답 시간
                </p>
              </CardContent>
            </Card>
          </motion.div>

          <motion.div
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ delay: 0.5 }}
          >
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">처리량</CardTitle>
                <TrendingUp className="h-4 w-4 text-emerald-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{detailedPerf.throughput.toFixed(1)}</div>
                <p className="text-xs text-muted-foreground mt-1">
                  초당 처리 건수
                </p>
              </CardContent>
            </Card>
          </motion.div>

          <motion.div
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ delay: 0.6 }}
          >
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">에러율</CardTitle>
                {detailedPerf.error_rate > 5 ? (
                  <AlertTriangle className="h-4 w-4 text-red-500" />
                ) : (
                  <CheckCircle className="h-4 w-4 text-green-500" />
                )}
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{detailedPerf.error_rate.toFixed(2)}%</div>
                <Progress value={detailedPerf.error_rate} className="mt-2" />
                <p className="text-xs text-muted-foreground mt-1">
                  {detailedPerf.error_rate > 5 ? '높음' : detailedPerf.error_rate > 1 ? '보통' : '낮음'}
                </p>
              </CardContent>
            </Card>
          </motion.div>
        </div>
      )}

      {/* 성능 리포트 */}
      {report && (
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.7 }}
        >
          <Card>
            <CardHeader>
              <CardTitle>실행 성능 리포트</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                <div className="text-center">
                  <div className="text-2xl font-bold text-blue-600">
                    {report.summary.transactions_processed.toLocaleString()}
                  </div>
                  <div className="text-sm text-gray-500">처리된 트랜잭션</div>
                </div>
                <div className="text-center">
                  <div className="text-2xl font-bold text-green-600">
                    {report.summary.opportunities_found.toLocaleString()}
                  </div>
                  <div className="text-sm text-gray-500">발견된 기회</div>
                </div>
                <div className="text-center">
                  <div className="text-2xl font-bold text-purple-600">
                    {report.summary.bundles_submitted.toLocaleString()}
                  </div>
                  <div className="text-sm text-gray-500">제출된 번들</div>
                </div>
                <div className="text-center">
                  <div className="text-2xl font-bold text-orange-600">
                    {report.summary.bundles_included.toLocaleString()}
                  </div>
                  <div className="text-sm text-gray-500">포함된 번들</div>
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <h4 className="font-semibold mb-2">성공률</h4>
                  <div className="flex items-center space-x-2">
                    <Progress value={report.summary.success_rate} className="flex-1" />
                    <span className="text-sm font-medium">{report.summary.success_rate.toFixed(1)}%</span>
                  </div>
                </div>
                <div>
                  <h4 className="font-semibold mb-2">총 수익</h4>
                  <div className="text-lg font-bold text-green-600">
                    {parseFloat(report.summary.total_profit_eth).toFixed(4)} ETH
                  </div>
                </div>
              </div>

              {report.recommendations.length > 0 && (
                <div>
                  <h4 className="font-semibold mb-2">성능 개선 권장사항</h4>
                  <ul className="space-y-1">
                    {report.recommendations.map((rec, index) => (
                      <li key={index} className="text-sm text-gray-600 flex items-start">
                        <span className="text-blue-500 mr-2">•</span>
                        {rec}
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </CardContent>
          </Card>
        </motion.div>
      )}
    </main>
  );
}
