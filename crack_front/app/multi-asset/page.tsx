'use client';

import { useState, useEffect } from 'react';

interface MultiAssetOpportunity {
  id: string;
  strategy_type: 'triangular' | 'position_migration' | 'complex';
  borrow_assets: string[];
  borrow_amounts: string[];
  target_assets: string[];
  expected_profit: string;
  profit_percentage: number;
  confidence_score: number;
  gas_estimate: number;
  max_execution_time_ms: number;
  discovered_at: string;
  expires_at: string;
}

interface MultiAssetStats {
  total_opportunities: number;
  executed_trades: number;
  successful_trades: number;
  failed_trades: number;
  total_volume: string;
  total_profit: string;
  success_rate: number;
  triangular_arbitrage_count: number;
  position_migration_count: number;
  complex_arbitrage_count: number;
}

export default function MultiAssetPage() {
  const [stats, setStats] = useState<MultiAssetStats | null>(null);
  const [opportunities, setOpportunities] = useState<MultiAssetOpportunity[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'overview' | 'opportunities' | 'triangular' | 'migration' | 'contracts'>('overview');
  const [selectedStrategy, setSelectedStrategy] = useState<'all' | 'triangular' | 'position_migration' | 'complex'>('all');

  useEffect(() => {
    let mounted = true;
    
    const fetchData = async () => {
      try {
        // Mock data for now - replace with actual API calls
        const mockStats: MultiAssetStats = {
          total_opportunities: 1247,
          executed_trades: 892,
          successful_trades: 756,
          failed_trades: 136,
          total_volume: '2,450,000',
          total_profit: '89,500',
          success_rate: 0.847,
          triangular_arbitrage_count: 623,
          position_migration_count: 189,
          complex_arbitrage_count: 80,
        };

        const mockOpportunities: MultiAssetOpportunity[] = [
          {
            id: 'opp_001',
            strategy_type: 'triangular',
            borrow_assets: ['WETH', 'USDC'],
            borrow_amounts: ['1.0', '2000.0'],
            target_assets: ['WETH', 'USDC'],
            expected_profit: '45.50',
            profit_percentage: 2.1,
            confidence_score: 0.87,
            gas_estimate: 450000,
            max_execution_time_ms: 5000,
            discovered_at: new Date().toISOString(),
            expires_at: new Date(Date.now() + 30000).toISOString(),
          },
          {
            id: 'opp_002',
            strategy_type: 'position_migration',
            borrow_assets: ['USDC', 'WETH'],
            borrow_amounts: ['5000.0', '2.5'],
            target_assets: ['USDC', 'WETH'],
            expected_profit: '125.30',
            profit_percentage: 1.8,
            confidence_score: 0.92,
            gas_estimate: 600000,
            max_execution_time_ms: 8000,
            discovered_at: new Date().toISOString(),
            expires_at: new Date(Date.now() + 60000).toISOString(),
          },
          {
            id: 'opp_003',
            strategy_type: 'complex',
            borrow_assets: ['WETH', 'USDC', 'DAI'],
            borrow_amounts: ['0.5', '1000.0', '500.0'],
            target_assets: ['WETH', 'USDC', 'DAI'],
            expected_profit: '78.90',
            profit_percentage: 3.2,
            confidence_score: 0.79,
            gas_estimate: 750000,
            max_execution_time_ms: 10000,
            discovered_at: new Date().toISOString(),
            expires_at: new Date(Date.now() + 45000).toISOString(),
          },
        ];

        if (mounted) {
          setStats(mockStats);
          setOpportunities(mockOpportunities);
        }
      } catch (error) {
        console.error('Failed to fetch multi-asset data:', error);
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    };
    
    fetchData();
    const interval = setInterval(fetchData, 3000);
    
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  const formatAmount = (amount: string) => {
    const num = parseFloat(amount);
    if (num >= 1000000) return (num / 1000000).toFixed(2) + 'M';
    if (num >= 1000) return (num / 1000).toFixed(2) + 'K';
    return num.toFixed(2);
  };

  const formatTime = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  const getStrategyTypeLabel = (type: string) => {
    switch (type) {
      case 'triangular': return '삼각 아비트래지';
      case 'position_migration': return '포지션 마이그레이션';
      case 'complex': return '복합 아비트래지';
      default: return type;
    }
  };

  const getStrategyTypeColor = (type: string) => {
    switch (type) {
      case 'triangular': return 'bg-blue-100 text-blue-800';
      case 'position_migration': return 'bg-green-100 text-green-800';
      case 'complex': return 'bg-purple-100 text-purple-800';
      default: return 'bg-gray-100 text-gray-800';
    }
  };

  const getConfidenceColor = (score: number) => {
    if (score >= 0.8) return 'text-green-600';
    if (score >= 0.6) return 'text-yellow-600';
    return 'text-red-600';
  };

  if (loading) {
    return (
      <div className="p-6">
        <h1 className="text-2xl font-bold mb-6">다중자산 플래시론</h1>
        <div className="text-center py-8">로딩 중...</div>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="p-6">
        <h1 className="text-2xl font-bold mb-6">다중자산 플래시론</h1>
        <div className="text-center py-8 text-red-600">데이터를 불러올 수 없습니다.</div>
      </div>
    );
  }

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-6">다중자산 플래시론</h1>
      
      {/* Tab Navigation */}
      <div className="flex space-x-1 mb-6 border-b">
        {[
          { id: 'overview', label: '개요' },
          { id: 'opportunities', label: '기회 탐지' },
          { id: 'triangular', label: '삼각 아비트래지' },
          { id: 'migration', label: '포지션 마이그레이션' },
          { id: 'contracts', label: '컨트랙트' }
        ].map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id as any)}
            className={`px-4 py-2 rounded-t-lg transition-colors ${
              activeTab === tab.id
                ? 'bg-blue-500 text-white border-b-2 border-blue-500'
                : 'bg-gray-200 text-gray-700 hover:bg-gray-300'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Overview Tab */}
      {activeTab === 'overview' && (
        <div className="space-y-6">
          {/* Performance Metrics */}
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">성과 지표</h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <div>
                <div className="text-sm text-gray-500">총 기회 수</div>
                <div className="text-2xl font-bold">{stats.total_opportunities.toLocaleString()}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">실행된 거래</div>
                <div className="text-2xl font-bold">{stats.executed_trades.toLocaleString()}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">총 수익</div>
                <div className="text-2xl font-bold text-green-600">${formatAmount(stats.total_profit)}</div>
              </div>
              <div>
                <div className="text-sm text-gray-500">성공률</div>
                <div className="text-2xl font-bold">{(stats.success_rate * 100).toFixed(1)}%</div>
              </div>
            </div>
          </div>

          {/* Strategy Distribution */}
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">전략별 분포</h2>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium text-blue-600">삼각 아비트래지</h3>
                  <span className="text-2xl font-bold">{stats.triangular_arbitrage_count}</span>
                </div>
                <div className="text-sm text-gray-600">
                  A,B → C → A,B 패턴의 다중자산 아비트래지
                </div>
              </div>
              <div className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium text-green-600">포지션 마이그레이션</h3>
                  <span className="text-2xl font-bold">{stats.position_migration_count}</span>
                </div>
                <div className="text-sm text-gray-600">
                  부채 + 담보를 함께 빌려 원자적으로 갈아끼우기
                </div>
              </div>
              <div className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium text-purple-600">복합 아비트래지</h3>
                  <span className="text-2xl font-bold">{stats.complex_arbitrage_count}</span>
                </div>
                <div className="text-sm text-gray-600">
                  복잡한 다중 단계 스왑 시퀀스
                </div>
              </div>
            </div>
          </div>

          {/* Recent Opportunities */}
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">최근 기회</h2>
            <div className="space-y-3">
              {opportunities.slice(0, 3).map((opp) => (
                <div key={opp.id} className="border rounded-lg p-4">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center space-x-2">
                      <span className={`px-2 py-1 text-xs font-semibold rounded-full ${getStrategyTypeColor(opp.strategy_type)}`}>
                        {getStrategyTypeLabel(opp.strategy_type)}
                      </span>
                      <span className="text-sm text-gray-500">#{opp.id}</span>
                    </div>
                    <div className="text-right">
                      <div className="text-lg font-bold text-green-600">+${opp.expected_profit}</div>
                      <div className="text-sm text-gray-500">{opp.profit_percentage.toFixed(2)}%</div>
                    </div>
                  </div>
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
                    <div>
                      <div className="text-gray-500">대출 자산</div>
                      <div className="font-medium">{opp.borrow_assets.join(', ')}</div>
                    </div>
                    <div>
                      <div className="text-gray-500">신뢰도</div>
                      <div className={`font-medium ${getConfidenceColor(opp.confidence_score)}`}>
                        {(opp.confidence_score * 100).toFixed(0)}%
                      </div>
                    </div>
                    <div>
                      <div className="text-gray-500">가스 추정</div>
                      <div className="font-medium">{opp.gas_estimate.toLocaleString()}</div>
                    </div>
                    <div>
                      <div className="text-gray-500">만료 시간</div>
                      <div className="font-medium">{formatTime(opp.expires_at)}</div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Opportunities Tab */}
      {activeTab === 'opportunities' && (
        <div className="bg-white rounded-lg shadow overflow-hidden">
          <div className="p-6 border-b">
            <h2 className="text-lg font-semibold mb-4">실시간 기회 탐지</h2>
            <div className="flex space-x-4">
              <select
                value={selectedStrategy}
                onChange={(e) => setSelectedStrategy(e.target.value as any)}
                className="border rounded px-3 py-2 text-sm"
              >
                <option value="triangular">삼각 아비트래지</option>
                <option value="position_migration">포지션 마이그레이션</option>
                <option value="complex">복합 아비트래지</option>
              </select>
              <button className="bg-blue-500 text-white px-4 py-2 rounded text-sm hover:bg-blue-600">
                스캔 시작
              </button>
            </div>
          </div>
          
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200">
              <thead className="bg-gray-50">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">ID</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">전략</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">대출 자산</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">예상 수익</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">신뢰도</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">가스</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">만료</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">액션</th>
                </tr>
              </thead>
              <tbody className="bg-white divide-y divide-gray-200">
                {opportunities
                  .filter(opp => selectedStrategy === 'all' || opp.strategy_type === selectedStrategy)
                  .map((opp) => (
                  <tr key={opp.id}>
                    <td className="px-6 py-4 whitespace-nowrap text-sm font-mono text-gray-900">
                      {opp.id}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <span className={`px-2 py-1 text-xs font-semibold rounded-full ${getStrategyTypeColor(opp.strategy_type)}`}>
                        {getStrategyTypeLabel(opp.strategy_type)}
                      </span>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {opp.borrow_assets.map((asset, idx) => (
                        <span key={idx}>
                          {asset} ({opp.borrow_amounts[idx]})
                          {idx < opp.borrow_assets.length - 1 && ', '}
                        </span>
                      ))}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm font-medium">
                      <div className="text-green-600">+${opp.expected_profit}</div>
                      <div className="text-xs text-gray-500">{opp.profit_percentage.toFixed(2)}%</div>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap">
                      <span className={`text-sm font-medium ${getConfidenceColor(opp.confidence_score)}`}>
                        {(opp.confidence_score * 100).toFixed(0)}%
                      </span>
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {opp.gas_estimate.toLocaleString()}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900">
                      {formatTime(opp.expires_at)}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-sm">
                      <button className="bg-green-500 text-white px-3 py-1 rounded text-xs hover:bg-green-600">
                        실행
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Triangular Arbitrage Tab */}
      {activeTab === 'triangular' && (
        <div className="space-y-6">
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">삼각 아비트래지 설정</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div>
                <h3 className="font-medium mb-3">토큰 설정</h3>
                <div className="space-y-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">토큰 A</label>
                    <select className="w-full border rounded px-3 py-2">
                      <option value="WETH">WETH</option>
                      <option value="USDC">USDC</option>
                      <option value="DAI">DAI</option>
                      <option value="WBTC">WBTC</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">토큰 B</label>
                    <select className="w-full border rounded px-3 py-2">
                      <option value="USDC">USDC</option>
                      <option value="WETH">WETH</option>
                      <option value="DAI">DAI</option>
                      <option value="WBTC">WBTC</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">중간 토큰 C</label>
                    <select className="w-full border rounded px-3 py-2">
                      <option value="DAI">DAI</option>
                      <option value="USDC">USDC</option>
                      <option value="WETH">WETH</option>
                      <option value="WBTC">WBTC</option>
                    </select>
                  </div>
                </div>
              </div>
              <div>
                <h3 className="font-medium mb-3">거래량 설정</h3>
                <div className="space-y-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">토큰 A 수량</label>
                    <input type="number" className="w-full border rounded px-3 py-2" placeholder="1.0" />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">토큰 B 수량</label>
                    <input type="number" className="w-full border rounded px-3 py-2" placeholder="2000.0" />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">최소 수익률 (%)</label>
                    <input type="number" step="0.1" className="w-full border rounded px-3 py-2" placeholder="1.0" />
                  </div>
                </div>
              </div>
            </div>
            <div className="mt-6 flex space-x-4">
              <button className="bg-blue-500 text-white px-6 py-2 rounded hover:bg-blue-600">
                수익성 계산
              </button>
              <button className="bg-green-500 text-white px-6 py-2 rounded hover:bg-green-600">
                실행
              </button>
            </div>
          </div>

          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">삼각 아비트래지 시각화</h2>
            <div className="flex justify-center items-center">
              <div className="relative">
                {/* A → C */}
                <div className="absolute top-0 left-1/2 transform -translate-x-1/2 -translate-y-8">
                  <div className="bg-blue-100 text-blue-800 px-3 py-1 rounded-full text-sm font-medium">A</div>
                  <div className="text-xs text-center mt-1">WETH</div>
                </div>
                <div className="absolute top-0 left-1/2 transform -translate-x-1/2 -translate-y-4">
                  <div className="w-0 h-0 border-l-4 border-r-4 border-b-4 border-transparent border-b-blue-500"></div>
                </div>

                {/* C */}
                <div className="absolute top-8 left-1/2 transform -translate-x-1/2">
                  <div className="bg-green-100 text-green-800 px-3 py-1 rounded-full text-sm font-medium">C</div>
                  <div className="text-xs text-center mt-1">DAI</div>
                </div>

                {/* B → C */}
                <div className="absolute top-0 right-0 transform translate-x-8 -translate-y-8">
                  <div className="bg-purple-100 text-purple-800 px-3 py-1 rounded-full text-sm font-medium">B</div>
                  <div className="text-xs text-center mt-1">USDC</div>
                </div>
                <div className="absolute top-0 right-0 transform translate-x-4 -translate-y-4">
                  <div className="w-0 h-0 border-l-4 border-r-4 border-b-4 border-transparent border-b-purple-500"></div>
                </div>

                {/* C → A, C → B */}
                <div className="absolute top-16 left-1/2 transform -translate-x-1/2">
                  <div className="w-0 h-0 border-l-4 border-r-4 border-t-4 border-transparent border-t-green-500"></div>
                </div>
                <div className="absolute top-16 right-0 transform translate-x-4">
                  <div className="w-0 h-0 border-l-4 border-r-4 border-t-4 border-transparent border-t-green-500"></div>
                </div>

                {/* A, B (return) */}
                <div className="absolute top-20 left-1/2 transform -translate-x-1/2 -translate-y-8">
                  <div className="bg-blue-100 text-blue-800 px-3 py-1 rounded-full text-sm font-medium">A</div>
                </div>
                <div className="absolute top-20 right-0 transform translate-x-8 -translate-y-8">
                  <div className="bg-purple-100 text-purple-800 px-3 py-1 rounded-full text-sm font-medium">B</div>
                </div>
              </div>
            </div>
            <div className="mt-8 text-center text-sm text-gray-600">
              <p>A,B → C → A,B 패턴으로 수익 창출</p>
            </div>
          </div>
        </div>
      )}

      {/* Position Migration Tab */}
      {activeTab === 'migration' && (
        <div className="space-y-6">
          <div className="bg-white rounded-lg shadow p-6">
            <h2 className="text-lg font-semibold mb-4">포지션 마이그레이션 설정</h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              <div>
                <h3 className="font-medium mb-3">기존 포지션</h3>
                <div className="space-y-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">부채 자산</label>
                    <select className="w-full border rounded px-3 py-2">
                      <option value="USDC">USDC</option>
                      <option value="DAI">DAI</option>
                      <option value="USDT">USDT</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">부채 수량</label>
                    <input type="number" className="w-full border rounded px-3 py-2" placeholder="5000.0" />
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">담보 자산</label>
                    <select className="w-full border rounded px-3 py-2">
                      <option value="WETH">WETH</option>
                      <option value="WBTC">WBTC</option>
                      <option value="LINK">LINK</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">담보 수량</label>
                    <input type="number" className="w-full border rounded px-3 py-2" placeholder="2.5" />
                  </div>
                </div>
              </div>
              <div>
                <h3 className="font-medium mb-3">새 포지션</h3>
                <div className="space-y-3">
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">새 부채 자산</label>
                    <select className="w-full border rounded px-3 py-2">
                      <option value="DAI">DAI</option>
                      <option value="USDC">USDC</option>
                      <option value="USDT">USDT</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">새 담보 자산</label>
                    <select className="w-full border rounded px-3 py-2">
                      <option value="WBTC">WBTC</option>
                      <option value="WETH">WETH</option>
                      <option value="LINK">LINK</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">최소 절약액 (USD)</label>
                    <input type="number" className="w-full border rounded px-3 py-2" placeholder="100.0" />
                  </div>
                </div>
              </div>
            </div>
            <div className="mt-6 flex space-x-4">
              <button className="bg-blue-500 text-white px-6 py-2 rounded hover:bg-blue-600">
                절약액 계산
              </button>
              <button className="bg-green-500 text-white px-6 py-2 rounded hover:bg-green-600">
                마이그레이션 실행
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Contracts Tab */}
      {activeTab === 'contracts' && (
        <div className="bg-white rounded-lg shadow p-6">
          <h2 className="text-lg font-semibold mb-4">다중자산 플래시론 컨트랙트</h2>
          <div className="space-y-4">
            <div className="border rounded-lg p-4">
              <div className="flex items-center justify-between mb-2">
                <h3 className="font-medium">MultiAssetArbitrageStrategy</h3>
                <div className="flex items-center space-x-2">
                  <span className="text-green-600 text-sm">✓ 검증됨</span>
                  <span className="bg-blue-100 text-blue-800 px-2 py-1 rounded text-xs">v1.0.0</span>
                </div>
              </div>
              <div className="text-sm text-gray-600 space-y-1">
                <div className="font-mono">0x742d35Cc65700000000000000000000000000004</div>
                <div>기능: 삼각 아비트래지, 포지션 마이그레이션, 복합 아비트래지</div>
                <div>플래시론 제공업체: Aave v3</div>
                <div>지원 DEX: Uniswap v2/v3, SushiSwap, 1inch</div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}