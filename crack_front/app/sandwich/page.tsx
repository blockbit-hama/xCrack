"use client"

import React, { useEffect, useState } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { motion } from 'framer-motion';
import { AlertTriangle, Target, Zap, TrendingUp } from 'lucide-react';

export default function SandwichPage() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    // 샌드위치 전략 데이터 로드
    const loadData = async () => {
      try {
        setLoading(false);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
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
        <h1 className="text-2xl font-bold mb-6">샌드위치 전략</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">샌드위치 전략</h1>
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
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-bold">샌드위치 전략</h1>
            <p className="text-gray-600 mt-1">멤풀 모니터링 기반 MEV 전략</p>
          </div>
          <div className="flex items-center space-x-2">
            <AlertTriangle className="h-5 w-5 text-yellow-500" />
            <span className="text-sm text-yellow-600">고위험 전략</span>
          </div>
        </div>
      </motion.div>

      {/* 경고 메시지 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.1 }}
      >
        <Card className="border-yellow-200 bg-yellow-50">
          <CardHeader>
            <CardTitle className="flex items-center space-x-2 text-yellow-800">
              <AlertTriangle className="h-5 w-5" />
              <span>주의사항</span>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-yellow-700 space-y-2">
              <p>• 샌드위치 공격은 윤리적 및 규제적 리스크가 있습니다</p>
              <p>• 일부 거래소에서는 샌드위치 공격을 감지하여 차단할 수 있습니다</p>
              <p>• 사용 전 관련 법규 및 거래소 정책을 확인하세요</p>
            </div>
          </CardContent>
        </Card>
      </motion.div>

      {/* 전략 상태 */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.2 }}
        >
          <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">전략 상태</CardTitle>
              <div className="w-3 h-3 rounded-full bg-red-400"></div>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-red-600">비활성</div>
              <p className="text-xs text-muted-foreground">
                윤리적 고려로 비활성화
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
              <CardTitle className="text-sm font-medium">멤풀 모니터</CardTitle>
              <Target className="h-4 w-4 text-blue-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">0</div>
              <p className="text-xs text-muted-foreground">
                모니터링 중인 대상
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
              <CardTitle className="text-sm font-medium">총 수익</CardTitle>
              <TrendingUp className="h-4 w-4 text-green-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-green-600">0.0000 ETH</div>
              <p className="text-xs text-muted-foreground">
                성공한 샌드위치: 0개
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
              <CardTitle className="text-sm font-medium">평균 수익</CardTitle>
              <Zap className="h-4 w-4 text-purple-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">0.0000 ETH</div>
              <p className="text-xs text-muted-foreground">
                샌드위치당 평균
              </p>
            </CardContent>
          </Card>
        </motion.div>
      </div>

      {/* 전략 설명 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.6 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>샌드위치 전략 개요</CardTitle>
            <CardDescription>MEV 기반 멤풀 공격 전략</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div>
                <h3 className="font-semibold mb-2">전략 원리</h3>
                <ul className="text-sm space-y-1 text-gray-600">
                  <li>• 멤풀에서 대형 스왑 트랜잭션 감지</li>
                  <li>• 해당 트랜잭션 앞뒤로 우리 트랜잭션 삽입</li>
                  <li>• 가격 변동으로부터 수익 추출</li>
                  <li>• MEV 번들을 통한 순서 보장</li>
                </ul>
              </div>
              <div>
                <h3 className="font-semibold mb-2">리스크 요소</h3>
                <ul className="text-sm space-y-1 text-gray-600">
                  <li>• 윤리적 및 규제적 리스크</li>
                  <li>• 거래소 차단 위험</li>
                  <li>• 가스비 손실 가능성</li>
                  <li>• 경쟁자와의 경쟁</li>
                </ul>
              </div>
            </div>
          </CardContent>
        </Card>
      </motion.div>

      {/* 비활성화 안내 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.7 }}
      >
        <Card className="border-gray-200 bg-gray-50">
          <CardContent className="pt-6">
            <div className="text-center space-y-4">
              <div className="w-16 h-16 bg-gray-200 rounded-full flex items-center justify-center mx-auto">
                <AlertTriangle className="h-8 w-8 text-gray-400" />
              </div>
              <div>
                <h3 className="text-lg font-semibold text-gray-700">전략 비활성화</h3>
                <p className="text-gray-600 mt-2">
                  윤리적 고려사항으로 인해 샌드위치 전략은 현재 비활성화되어 있습니다.
                </p>
                <p className="text-sm text-gray-500 mt-2">
                  대신 청산, 마이크로 아비트래지, 크로스체인 아비트래지 전략을 사용하세요.
                </p>
              </div>
            </div>
          </CardContent>
        </Card>
      </motion.div>
    </main>
  );
}
