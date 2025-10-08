"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { Progress } from "../../components/ui/progress";
import { 
  getRiskDashboard, 
  runStressTest, 
  acknowledgeRiskEvent,
  type RiskDashboard, 
  type StressTestResult 
} from '../../lib/api';
import { motion } from 'framer-motion';
import { 
  AlertTriangle, 
  Shield, 
  TrendingDown, 
  DollarSign, 
  Activity,
  Play,
  CheckCircle,
  XCircle
} from 'lucide-react';

export default function RiskPage() {
  const [dashboard, setDashboard] = useState<RiskDashboard | null>(null);
  const [stressTest, setStressTest] = useState<StressTestResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [runningStressTest, setRunningStressTest] = useState(false);

  useEffect(() => {
    const loadData = async () => {
      setLoading(true);
      setError("");
      try {
        const dashboardData = await getRiskDashboard();
        setDashboard(dashboardData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    loadData();
    const interval = setInterval(loadData, 15000); // 15초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const handleRunStressTest = async () => {
    setRunningStressTest(true);
    try {
      const result = await runStressTest();
      setStressTest(result);
    } catch (e: any) {
      setError(e.message || "스트레스 테스트 실행 실패");
    } finally {
      setRunningStressTest(false);
    }
  };

  const handleAcknowledgeRiskEvent = async (eventId: string) => {
    try {
      await acknowledgeRiskEvent(eventId);
      // 리스크 이벤트 목록 새로고침
      const dashboardData = await getRiskDashboard();
      setDashboard(dashboardData);
    } catch (e: any) {
      setError(e.message || "리스크 이벤트 확인 실패");
    }
  };

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">리스크 관리</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">리스크 관리</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  const getRiskLevel = (score: number) => {
    if (score >= 80) return { level: 'Critical', color: 'text-red-600', bgColor: 'bg-red-100' };
    if (score >= 60) return { level: 'High', color: 'text-orange-600', bgColor: 'bg-orange-100' };
    if (score >= 40) return { level: 'Medium', color: 'text-yellow-600', bgColor: 'bg-yellow-100' };
    return { level: 'Low', color: 'text-green-600', bgColor: 'bg-green-100' };
  };

  return (
    <main className="p-6 space-y-6">
      {/* 헤더 */}
      <motion.div 
        initial={{ opacity: 0, y: -20 }}
        animate={{ opacity: 1, y: 0 }}
        className="border-b pb-4"
      >
        <h1 className="text-3xl font-bold">리스크 관리</h1>
        <p className="text-gray-600 mt-1">포지션 리스크 및 스트레스 테스트 모니터링</p>
      </motion.div>

      {/* 리스크 대시보드 */}
      {dashboard && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <motion.div
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ delay: 0.1 }}
          >
            <Card>
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">리스크 스코어</CardTitle>
                <Shield className="h-4 w-4 text-blue-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{dashboard.risk_score.toFixed(1)}</div>
                <Progress value={dashboard.risk_score} className="mt-2" />
                <div className="mt-2">
                  <Badge className={`${getRiskLevel(dashboard.risk_score).bgColor} ${getRiskLevel(dashboard.risk_score).color}`}>
                    {getRiskLevel(dashboard.risk_score).level}
                  </Badge>
                </div>
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
                <CardTitle className="text-sm font-medium">총 노출</CardTitle>
                <DollarSign className="h-4 w-4 text-green-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">{parseFloat(dashboard.exposure).toFixed(2)} ETH</div>
                <p className="text-xs text-muted-foreground mt-1">
                  현재 포지션 노출
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
                <CardTitle className="text-sm font-medium">최대 손실</CardTitle>
                <TrendingDown className="h-4 w-4 text-red-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold text-red-600">
                  {parseFloat(dashboard.max_drawdown).toFixed(2)} ETH
                </div>
                <p className="text-xs text-muted-foreground mt-1">
                  최대 예상 손실
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
                <CardTitle className="text-sm font-medium">VaR 95%</CardTitle>
                <AlertTriangle className="h-4 w-4 text-orange-500" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {parseFloat(dashboard.var_95).toFixed(2)} ETH
                </div>
                <p className="text-xs text-muted-foreground mt-1">
                  95% 신뢰구간 손실
                </p>
              </CardContent>
            </Card>
          </motion.div>
        </div>
      )}

      {/* 스트레스 테스트 섹션 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.5 }}
      >
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <CardTitle>스트레스 테스트</CardTitle>
              <button
                onClick={handleRunStressTest}
                disabled={runningStressTest}
                className={`px-4 py-2 rounded-md flex items-center space-x-2 ${
                  runningStressTest
                    ? 'bg-gray-400 cursor-not-allowed'
                    : 'bg-blue-500 hover:bg-blue-600 text-white'
                }`}
              >
                <Play className="h-4 w-4" />
                <span>{runningStressTest ? '실행 중...' : '테스트 실행'}</span>
              </button>
            </div>
          </CardHeader>
          <CardContent>
            {stressTest ? (
              <div className="space-y-4">
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                  <div className="text-center">
                    <div className="text-sm text-gray-500">테스트 ID</div>
                    <div className="font-mono text-sm">{stressTest.test_id}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-sm text-gray-500">상태</div>
                    <Badge className={
                      stressTest.status === 'completed' ? 'bg-green-100 text-green-600' :
                      stressTest.status === 'running' ? 'bg-blue-100 text-blue-600' :
                      'bg-red-100 text-red-600'
                    }>
                      {stressTest.status}
                    </Badge>
                  </div>
                  <div className="text-center">
                    <div className="text-sm text-gray-500">실행 시간</div>
                    <div className="text-sm">{new Date(stressTest.timestamp).toLocaleString('ko-KR')}</div>
                  </div>
                </div>
                
                {stressTest.results && (
                  <div className="mt-4 p-4 bg-gray-50 rounded-lg">
                    <h4 className="font-semibold mb-2">테스트 결과</h4>
                    <pre className="text-sm text-gray-600 overflow-auto">
                      {JSON.stringify(stressTest.results, null, 2)}
                    </pre>
                  </div>
                )}
              </div>
            ) : (
              <div className="text-center py-8 text-gray-500">
                <Activity className="h-12 w-12 mx-auto mb-4 text-gray-300" />
                <p>스트레스 테스트를 실행하여 리스크 분석을 시작하세요</p>
              </div>
            )}
          </CardContent>
        </Card>
      </motion.div>

      {/* 리스크 이벤트 관리 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.6 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>리스크 이벤트</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <div className="text-center py-8 text-gray-500">
                <AlertTriangle className="h-12 w-12 mx-auto mb-4 text-gray-300" />
                <p>현재 활성 리스크 이벤트가 없습니다</p>
                <p className="text-sm mt-2">시스템이 정상적으로 모니터링 중입니다</p>
              </div>
            </div>
          </CardContent>
        </Card>
      </motion.div>

      {/* 리스크 관리 권장사항 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.7 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>리스크 관리 권장사항</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="space-y-2">
                <h4 className="font-semibold text-green-600 flex items-center">
                  <CheckCircle className="h-4 w-4 mr-2" />
                  현재 상태 양호
                </h4>
                <ul className="text-sm text-gray-600 space-y-1">
                  <li>• 리스크 스코어가 허용 범위 내에 있습니다</li>
                  <li>• 포지션 노출이 적절히 관리되고 있습니다</li>
                  <li>• 정기적인 모니터링이 활성화되어 있습니다</li>
                </ul>
              </div>
              <div className="space-y-2">
                <h4 className="font-semibold text-blue-600 flex items-center">
                  <Activity className="h-4 w-4 mr-2" />
                  지속 모니터링
                </h4>
                <ul className="text-sm text-gray-600 space-y-1">
                  <li>• 시장 변동성에 따른 리스크 재평가</li>
                  <li>• 포지션 크기 조정 고려</li>
                  <li>• 스트레스 테스트 정기 실행</li>
                </ul>
              </div>
            </div>
          </CardContent>
        </Card>
      </motion.div>
    </main>
  );
}
