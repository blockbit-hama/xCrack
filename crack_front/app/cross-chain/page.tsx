"use client"

import React, { useEffect, useState } from 'react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { motion } from 'framer-motion';
import { Link, ArrowRightLeft, Clock, AlertCircle, TrendingUp } from 'lucide-react';

export default function CrossChainPage() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    // 크로스체인 아비트래지 데이터 로드
    const loadData = async () => {
      try {
        setLoading(false);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
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
        <h1 className="text-2xl font-bold mb-6">크로스체인 아비트래지</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">크로스체인 아비트래지</h1>
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
            <h1 className="text-3xl font-bold">크로스체인 아비트래지</h1>
            <p className="text-gray-600 mt-1">다중 체인 간 가격 차이 포착</p>
          </div>
          <div className="flex items-center space-x-2">
            <Badge variant="info" className="text-xs">Mock 모드</Badge>
          </div>
        </div>
      </motion.div>

      {/* 모드 안내 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.1 }}
      >
        <Card className="border-blue-200 bg-blue-50">
          <CardHeader>
            <CardTitle className="flex items-center space-x-2 text-blue-800">
              <AlertCircle className="h-5 w-5" />
              <span>Mock 모드</span>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-blue-700 space-y-2">
              <p>• 현재 크로스체인 아비트래지는 Mock 모드로 실행됩니다</p>
              <p>• 실제 브리지 연결 없이 시뮬레이션 데이터를 사용합니다</p>
              <p>• 프로덕션 환경에서는 실제 브리지 통합이 필요합니다</p>
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
              <div className="w-3 h-3 rounded-full bg-green-400 animate-pulse"></div>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-green-600">활성</div>
              <p className="text-xs text-muted-foreground">
                Mock 모드로 실행 중
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
              <CardTitle className="text-sm font-medium">모니터링 체인</CardTitle>
              <Link className="h-4 w-4 text-blue-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">3</div>
              <p className="text-xs text-muted-foreground">
                Ethereum, Polygon, BSC
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
              <CardTitle className="text-sm font-medium">활성 기회</CardTitle>
              <TrendingUp className="h-4 w-4 text-green-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold text-green-600">2</div>
              <p className="text-xs text-muted-foreground">
                수익성 있는 기회
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
              <CardTitle className="text-sm font-medium">평균 실행시간</CardTitle>
              <Clock className="h-4 w-4 text-purple-600" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">45s</div>
              <p className="text-xs text-muted-foreground">
                브리지 처리 시간
              </p>
            </CardContent>
          </Card>
        </motion.div>
      </div>

      {/* 브리지 상태 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.6 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>브리지 연결 상태</CardTitle>
            <CardDescription>지원되는 크로스체인 브리지</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              {[
                { name: "Stargate", status: "연결됨", chains: "Ethereum ↔ Polygon" },
                { name: "Hop Protocol", status: "연결됨", chains: "Ethereum ↔ BSC" },
                { name: "Across Protocol", status: "연결됨", chains: "Ethereum ↔ Arbitrum" },
                { name: "Multichain", status: "연결됨", chains: "Polygon ↔ BSC" },
                { name: "Synapse", status: "연결됨", chains: "Ethereum ↔ Avalanche" },
                { name: "LiFi", status: "연결됨", chains: "모든 체인" }
              ].map((bridge, index) => (
                <motion.div
                  key={bridge.name}
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ delay: 0.7 + index * 0.1 }}
                  className="border rounded-lg p-4"
                >
                  <div className="flex items-center justify-between mb-2">
                    <h3 className="font-medium">{bridge.name}</h3>
                    <Badge variant="success" className="text-xs">연결됨</Badge>
                  </div>
                  <div className="text-sm text-gray-600">
                    <div className="flex items-center space-x-1">
                      <ArrowRightLeft className="h-3 w-3" />
                      <span>{bridge.chains}</span>
                    </div>
                  </div>
                </motion.div>
              ))}
            </div>
          </CardContent>
        </Card>
      </motion.div>

      {/* 활성 기회 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.8 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>활성 아비트래지 기회</CardTitle>
            <CardDescription>크로스체인 가격 차이 기회</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {[
                {
                  pair: "USDC",
                  fromChain: "Ethereum",
                  toChain: "Polygon",
                  spread: "0.15%",
                  profit: "$12.50",
                  bridge: "Stargate",
                  timeLeft: "2m 30s"
                },
                {
                  pair: "WETH",
                  fromChain: "Polygon",
                  toChain: "BSC",
                  spread: "0.08%",
                  profit: "$8.20",
                  bridge: "Multichain",
                  timeLeft: "1m 45s"
                }
              ].map((opportunity, index) => (
                <motion.div
                  key={index}
                  initial={{ opacity: 0, x: -20 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: 0.9 + index * 0.1 }}
                  className="flex items-center justify-between p-4 border rounded-lg hover:bg-gray-50"
                >
                  <div className="flex items-center space-x-4">
                    <div className="w-10 h-10 bg-gradient-to-r from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
                      <span className="text-white font-bold text-sm">
                        {opportunity.pair.slice(0, 2)}
                      </span>
                    </div>
                    <div>
                      <div className="font-medium">{opportunity.pair}</div>
                      <div className="text-sm text-gray-500">
                        {opportunity.fromChain} → {opportunity.toChain}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center space-x-6">
                    <div className="text-right">
                      <div className="font-medium text-green-600">{opportunity.spread}</div>
                      <div className="text-sm text-gray-500">스프레드</div>
                    </div>
                    <div className="text-right">
                      <div className="font-medium text-green-600">{opportunity.profit}</div>
                      <div className="text-sm text-gray-500">예상 수익</div>
                    </div>
                    <div className="text-right">
                      <div className="font-medium">{opportunity.bridge}</div>
                      <div className="text-sm text-gray-500">브리지</div>
                    </div>
                    <div className="text-right">
                      <div className="font-medium text-orange-600">{opportunity.timeLeft}</div>
                      <div className="text-sm text-gray-500">남은 시간</div>
                    </div>
                  </div>
                </motion.div>
              ))}
            </div>
          </CardContent>
        </Card>
      </motion.div>

      {/* 전략 설명 */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 1.0 }}
      >
        <Card>
          <CardHeader>
            <CardTitle>크로스체인 아비트래지 전략</CardTitle>
            <CardDescription>다중 체인 간 가격 차이 포착</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div>
                <h3 className="font-semibold mb-2">전략 원리</h3>
                <ul className="text-sm space-y-1 text-gray-600">
                  <li>• 여러 체인에서 동일한 토큰 가격 모니터링</li>
                  <li>• 가격 차이가 수수료를 초과할 때 실행</li>
                  <li>• 브리지를 통한 토큰 이동</li>
                  <li>• 양방향 아비트래지 기회 포착</li>
                </ul>
              </div>
              <div>
                <h3 className="font-semibold mb-2">리스크 요소</h3>
                <ul className="text-sm space-y-1 text-gray-600">
                  <li>• 브리지 처리 시간 지연</li>
                  <li>• 브리지 수수료 변동</li>
                  <li>• 가격 변동으로 인한 기회 상실</li>
                  <li>• 브리지 보안 리스크</li>
                </ul>
              </div>
            </div>
          </CardContent>
        </Card>
      </motion.div>
    </main>
  );
}
