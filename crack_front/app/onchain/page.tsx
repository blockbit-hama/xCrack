"use client";

import { useEffect, useState } from "react";
import { 
  getOnChainAnalytics, 
  getMevTransactions, 
  getWhaleTransactions, 
  getFlashLoanActivities,
  type OnChainAnalytics, 
  type OnChainTransaction,
  type TokenInfo,
  type DexPool,
  type BlockInfo 
} from '@/lib/api';

export default function OnChainPage() {
  const [analytics, setAnalytics] = useState<OnChainAnalytics | null>(null);
  const [mevTxs, setMevTxs] = useState<OnChainTransaction[]>([]);
  const [whaleTxs, setWhaleTxs] = useState<OnChainTransaction[]>([]);
  const [flashLoanTxs, setFlashLoanTxs] = useState<OnChainTransaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      setError("");
      try {
        const [analyticsData, mevData, whaleData, flashLoanData] = await Promise.all([
          getOnChainAnalytics(),
          getMevTransactions(10),
          getWhaleTransactions(100000, 10),
          getFlashLoanActivities(10)
        ]);

        setAnalytics(analyticsData);
        setMevTxs(mevData);
        setWhaleTxs(whaleData);
        setFlashLoanTxs(flashLoanData);
      } catch (e: any) {
        setError(e.message || "데이터 로드 실패");
      } finally {
        setLoading(false);
      }
    };

    fetchData();
    const interval = setInterval(fetchData, 15000); // 15초마다 업데이트
    return () => clearInterval(interval);
  }, []);

  const formatAddress = (address: string) => {
    return `${address.slice(0, 6)}...${address.slice(-4)}`;
  };

  const formatNumber = (num: string | number) => {
    const n = typeof num === 'string' ? parseFloat(num) : num;
    if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
    if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
    if (n >= 1e3) return (n / 1e3).toFixed(2) + 'K';
    return n.toFixed(2);
  };

  const getMevTypeColor = (type?: string) => {
    switch (type) {
      case 'sandwich': return 'bg-red-100 text-red-800';
      case 'arbitrage': return 'bg-green-100 text-green-800';
      case 'liquidation': return 'bg-yellow-100 text-yellow-800';
      case 'frontrun': return 'bg-blue-100 text-blue-800';
      case 'backrun': return 'bg-purple-100 text-purple-800';
      default: return 'bg-gray-100 text-gray-800';
    }
  };

  const getRiskColor = (risk: string) => {
    switch (risk) {
      case 'low': return 'text-green-600';
      case 'medium': return 'text-yellow-600';
      case 'high': return 'text-red-600';
      default: return 'text-gray-600';
    }
  };

  if (loading) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">온체인 데이터 분석</h1>
        <div>로딩 중...</div>
      </main>
    );
  }

  if (error) {
    return (
      <main className="p-6">
        <h1 className="text-2xl font-bold mb-6">온체인 데이터 분석</h1>
        <div className="text-red-600">오류: {error}</div>
      </main>
    );
  }

  return (
    <main className="p-6 space-y-6">
      <h1 className="text-2xl font-bold">온체인 데이터 분석</h1>
      
      {/* 최신 블록 정보 */}
      {analytics?.latest_block && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">최신 블록 정보</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">블록 번호</h3>
              <p className="text-lg font-bold">{analytics.latest_block.number.toLocaleString()}</p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">거래 수</h3>
              <p className="text-lg font-bold">{analytics.latest_block.transaction_count}</p>
              <p className="text-sm text-gray-600">MEV: {analytics.latest_block.mev_transactions}</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">가스 사용률</h3>
              <p className="text-lg font-bold">
                {((parseFloat(analytics.latest_block.gas_used) / parseFloat(analytics.latest_block.gas_limit)) * 100).toFixed(1)}%
              </p>
              <p className="text-sm text-gray-600">베이스피: {parseFloat(analytics.latest_block.base_fee).toFixed(2)} Gwei</p>
            </div>
            
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">총 MEV 가치</h3>
              <p className="text-lg font-bold text-green-600">{formatNumber(analytics.latest_block.total_mev_value)} ETH</p>
            </div>
          </div>
        </div>
      )}

      {/* 가스 추적 */}
      {analytics?.gas_tracking && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">가스 추적</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <div className="space-y-2">
              <h3 className="font-medium text-gray-700">현재 베이스피</h3>
              <p className="text-xl font-bold">{parseFloat(analytics.gas_tracking.current_base_fee).toFixed(2)} Gwei</p>
            </div>
            
            <div className="space-y-2">
              <h3 className="font-medium text-gray-700">추천 가스가격</h3>
              <div className="space-y-1 text-sm">
                <div className="flex justify-between">
                  <span>느림:</span>
                  <span className="font-medium">{parseFloat(analytics.gas_tracking.recommended_gas_prices.slow).toFixed(1)} Gwei</span>
                </div>
                <div className="flex justify-between">
                  <span>표준:</span>
                  <span className="font-medium">{parseFloat(analytics.gas_tracking.recommended_gas_prices.standard).toFixed(1)} Gwei</span>
                </div>
                <div className="flex justify-between">
                  <span>빠름:</span>
                  <span className="font-medium">{parseFloat(analytics.gas_tracking.recommended_gas_prices.fast).toFixed(1)} Gwei</span>
                </div>
                <div className="flex justify-between">
                  <span>즉시:</span>
                  <span className="font-medium">{parseFloat(analytics.gas_tracking.recommended_gas_prices.instant).toFixed(1)} Gwei</span>
                </div>
              </div>
            </div>
            
            <div className="space-y-2">
              <h3 className="font-medium text-gray-700">블록 사용률 & 예측</h3>
              <p className="text-lg font-bold">{(analytics.gas_tracking.block_utilization * 100).toFixed(1)}%</p>
              <p className="text-sm text-gray-600">
                다음 베이스피: {parseFloat(analytics.gas_tracking.next_base_fee_estimate).toFixed(2)} Gwei
              </p>
            </div>
          </div>
        </div>
      )}

      {/* MEV 메트릭스 */}
      {analytics?.mev_metrics && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">MEV 메트릭스</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
            <div className="bg-red-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">총 MEV 볼륨</h3>
              <p className="text-lg font-bold">{formatNumber(analytics.mev_metrics.total_mev_volume_eth)} ETH</p>
            </div>
            
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">MEV 거래 수</h3>
              <p className="text-lg font-bold">{analytics.mev_metrics.mev_transactions_count}</p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">샌드위치 공격</h3>
              <p className="text-lg font-bold text-red-600">{analytics.mev_metrics.sandwich_attacks}</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">블록당 평균 MEV</h3>
              <p className="text-lg font-bold">{formatNumber(analytics.mev_metrics.average_mev_per_block)} ETH</p>
            </div>
          </div>

          {/* 상위 MEV 봇들 */}
          {analytics.mev_metrics.top_mev_bots && analytics.mev_metrics.top_mev_bots.length > 0 && (
            <div>
              <h3 className="font-medium text-gray-700 mb-3">상위 MEV 봇</h3>
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b">
                      <th className="text-left py-2">주소</th>
                      <th className="text-right py-2">수익</th>
                      <th className="text-right py-2">거래수</th>
                      <th className="text-right py-2">성공률</th>
                    </tr>
                  </thead>
                  <tbody>
                    {analytics.mev_metrics.top_mev_bots.map((bot, idx) => (
                      <tr key={idx} className="border-b hover:bg-gray-50">
                        <td className="py-2 font-mono">{formatAddress(bot.address)}</td>
                        <td className="py-2 text-right font-medium text-green-600">
                          {formatNumber(bot.profit_eth)} ETH
                        </td>
                        <td className="py-2 text-right">{bot.transaction_count}</td>
                        <td className="py-2 text-right">{(bot.success_rate * 100).toFixed(1)}%</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {/* 유동성 분석 */}
      {analytics?.liquidity_analysis && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">유동성 분석</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">총 TVL</h3>
              <p className="text-lg font-bold">${formatNumber(analytics.liquidity_analysis.total_tvl_usd)}</p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">새로운 풀 (24h)</h3>
              <p className="text-lg font-bold">{analytics.liquidity_analysis.new_pools_24h}</p>
            </div>
            
            <div className="bg-red-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">유동성 제거 (24h)</h3>
              <p className="text-lg font-bold text-red-600">${formatNumber(analytics.liquidity_analysis.removed_liquidity_24h)}</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">IL 위험</h3>
              <p className={`text-lg font-bold ${getRiskColor(analytics.liquidity_analysis.impermanent_loss_risk)}`}>
                {analytics.liquidity_analysis.impermanent_loss_risk.toUpperCase()}
              </p>
            </div>
          </div>

          {/* 상위 풀들 */}
          {analytics.liquidity_analysis.top_pools && analytics.liquidity_analysis.top_pools.length > 0 && (
            <div>
              <h3 className="font-medium text-gray-700 mb-3">상위 유동성 풀</h3>
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="border-b">
                      <th className="text-left py-2">DEX</th>
                      <th className="text-left py-2">페어</th>
                      <th className="text-right py-2">TVL</th>
                      <th className="text-right py-2">24h 볼륨</th>
                      <th className="text-right py-2">수수료</th>
                      <th className="text-right py-2">APY</th>
                    </tr>
                  </thead>
                  <tbody>
                    {analytics.liquidity_analysis.top_pools.slice(0, 10).map((pool: DexPool, idx) => (
                      <tr key={idx} className="border-b hover:bg-gray-50">
                        <td className="py-2">{pool.dex_name}</td>
                        <td className="py-2 font-medium">
                          {pool.token0.symbol}/{pool.token1.symbol}
                        </td>
                        <td className="py-2 text-right">${formatNumber(pool.total_liquidity_usd)}</td>
                        <td className="py-2 text-right">${formatNumber(pool.volume_24h_usd)}</td>
                        <td className="py-2 text-right">{pool.fee_tier}%</td>
                        <td className="py-2 text-right">
                          {pool.apy ? `${pool.apy.toFixed(2)}%` : 'N/A'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {/* 트렌딩 토큰 */}
      {analytics?.trending_tokens && analytics.trending_tokens.length > 0 && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">트렌딩 토큰</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {analytics.trending_tokens.map((token: TokenInfo, idx) => (
              <div key={idx} className="border rounded-lg p-4">
                <div className="flex items-center justify-between mb-2">
                  <h3 className="font-medium">{token.symbol}</h3>
                  <span className={`px-2 py-1 rounded-full text-xs ${
                    token.verified ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'
                  }`}>
                    {token.verified ? '인증됨' : '미인증'}
                  </span>
                </div>
                <div className="text-sm space-y-1">
                  <div>이름: <span className="font-medium">{token.name}</span></div>
                  <div>주소: <span className="font-mono text-xs">{formatAddress(token.address)}</span></div>
                  {token.price_usd && (
                    <div>가격: <span className="font-medium">${parseFloat(token.price_usd).toFixed(4)}</span></div>
                  )}
                  {token.volume_24h && (
                    <div>24h 볼륨: <span className="font-medium">${formatNumber(token.volume_24h)}</span></div>
                  )}
                  {token.market_cap && (
                    <div>시총: <span className="font-medium">${formatNumber(token.market_cap)}</span></div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* MEV 거래 */}
      <div className="bg-white p-6 rounded-lg border">
        <h2 className="text-lg font-semibold mb-4">최근 MEV 거래</h2>
        {mevTxs.length === 0 ? (
          <div className="text-gray-500">최근 MEV 거래가 없습니다.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">시간</th>
                  <th className="text-left py-2">해시</th>
                  <th className="text-left py-2">MEV 타입</th>
                  <th className="text-right py-2">수익</th>
                  <th className="text-right py-2">가스비</th>
                  <th className="text-left py-2">From</th>
                </tr>
              </thead>
              <tbody>
                {mevTxs.map((tx) => (
                  <tr key={tx.hash} className="border-b hover:bg-gray-50">
                    <td className="py-2">{new Date(tx.timestamp).toLocaleString()}</td>
                    <td className="py-2 font-mono text-xs">{formatAddress(tx.hash)}</td>
                    <td className="py-2">
                      {tx.mev_type && (
                        <span className={`px-2 py-1 rounded-full text-xs ${getMevTypeColor(tx.mev_type)}`}>
                          {tx.mev_type}
                        </span>
                      )}
                    </td>
                    <td className="py-2 text-right font-medium text-green-600">
                      {tx.mev_profit ? `${formatNumber(tx.mev_profit)} ETH` : 'N/A'}
                    </td>
                    <td className="py-2 text-right">
                      {formatNumber((parseFloat(tx.gas_used) * parseFloat(tx.gas_price)) / 1e18)} ETH
                    </td>
                    <td className="py-2 font-mono text-xs">{formatAddress(tx.from)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 고래 거래 */}
      <div className="bg-white p-6 rounded-lg border">
        <h2 className="text-lg font-semibold mb-4">고래 거래 (>$100K)</h2>
        {whaleTxs.length === 0 ? (
          <div className="text-gray-500">최근 고래 거래가 없습니다.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">시간</th>
                  <th className="text-left py-2">해시</th>
                  <th className="text-right py-2">가치</th>
                  <th className="text-left py-2">From</th>
                  <th className="text-left py-2">To</th>
                  <th className="text-left py-2">토큰 전송</th>
                </tr>
              </thead>
              <tbody>
                {whaleTxs.map((tx) => (
                  <tr key={tx.hash} className="border-b hover:bg-gray-50">
                    <td className="py-2">{new Date(tx.timestamp).toLocaleString()}</td>
                    <td className="py-2 font-mono text-xs">{formatAddress(tx.hash)}</td>
                    <td className="py-2 text-right font-medium text-blue-600">
                      {formatNumber(parseFloat(tx.value) / 1e18)} ETH
                    </td>
                    <td className="py-2 font-mono text-xs">{formatAddress(tx.from)}</td>
                    <td className="py-2 font-mono text-xs">{tx.to ? formatAddress(tx.to) : 'Contract'}</td>
                    <td className="py-2">
                      {tx.token_transfers.length > 0 ? (
                        <div className="text-xs">
                          {tx.token_transfers.slice(0, 2).map((transfer, idx) => (
                            <div key={idx}>
                              {formatNumber(transfer.amount)} {transfer.token_symbol}
                            </div>
                          ))}
                          {tx.token_transfers.length > 2 && (
                            <div className="text-gray-500">+{tx.token_transfers.length - 2} more</div>
                          )}
                        </div>
                      ) : (
                        'ETH 전송'
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 플래시론 활동 */}
      <div className="bg-white p-6 rounded-lg border">
        <h2 className="text-lg font-semibold mb-4">플래시론 활동</h2>
        {flashLoanTxs.length === 0 ? (
          <div className="text-gray-500">최근 플래시론 활동이 없습니다.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">시간</th>
                  <th className="text-left py-2">해시</th>
                  <th className="text-left py-2">From</th>
                  <th className="text-right py-2">가스비</th>
                  <th className="text-left py-2">DEX 거래</th>
                </tr>
              </thead>
              <tbody>
                {flashLoanTxs.map((tx) => (
                  <tr key={tx.hash} className="border-b hover:bg-gray-50">
                    <td className="py-2">{new Date(tx.timestamp).toLocaleString()}</td>
                    <td className="py-2 font-mono text-xs">{formatAddress(tx.hash)}</td>
                    <td className="py-2 font-mono text-xs">{formatAddress(tx.from)}</td>
                    <td className="py-2 text-right">
                      {formatNumber((parseFloat(tx.gas_used) * parseFloat(tx.gas_price)) / 1e18)} ETH
                    </td>
                    <td className="py-2">
                      {tx.dex_trades && tx.dex_trades.length > 0 ? (
                        <div className="text-xs">
                          {tx.dex_trades.slice(0, 2).map((trade, idx) => (
                            <div key={idx}>
                              {trade.dex_name}: {formatNumber(trade.amount_in)} → {formatNumber(trade.amount_out)}
                            </div>
                          ))}
                          {tx.dex_trades.length > 2 && (
                            <div className="text-gray-500">+{tx.dex_trades.length - 2} more</div>
                          )}
                        </div>
                      ) : (
                        'N/A'
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* 프로토콜 통계 */}
      {analytics?.protocol_stats && (
        <div className="bg-white p-6 rounded-lg border">
          <h2 className="text-lg font-semibold mb-4">프로토콜 통계</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <div className="bg-pink-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">Uniswap V3</h3>
              <p className="text-lg font-bold">${formatNumber(analytics.protocol_stats.uniswap_v3_volume)}</p>
              <p className="text-xs text-gray-600">24h 볼륨</p>
            </div>
            
            <div className="bg-blue-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">Uniswap V2</h3>
              <p className="text-lg font-bold">${formatNumber(analytics.protocol_stats.uniswap_v2_volume)}</p>
              <p className="text-xs text-gray-600">24h 볼륨</p>
            </div>
            
            <div className="bg-orange-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">SushiSwap</h3>
              <p className="text-lg font-bold">${formatNumber(analytics.protocol_stats.sushiswap_volume)}</p>
              <p className="text-xs text-gray-600">24h 볼륨</p>
            </div>
            
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">Aave</h3>
              <p className="text-lg font-bold">${formatNumber(analytics.protocol_stats.aave_tvl)}</p>
              <p className="text-xs text-gray-600">TVL</p>
            </div>
            
            <div className="bg-purple-50 p-4 rounded-lg">
              <h3 className="text-sm font-medium text-gray-500 mb-1">Compound</h3>
              <p className="text-lg font-bold">${formatNumber(analytics.protocol_stats.compound_tvl)}</p>
              <p className="text-xs text-gray-600">TVL</p>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}