"use client";

import { useEffect, useState } from "react";
import { getStrategyParams, updateStrategyParams, type StrategyParamsResp } from '@/lib/api';

type StrategyState = Record<string, boolean>;

type SettingsResp = {
  strategies: StrategyState;
  api_port: number;
  metrics_port: number;
};

const BACKEND = process.env.NEXT_PUBLIC_BACKEND_URL || "http://localhost:8080";

export default function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [data, setData] = useState<SettingsResp | null>(null);
  const [msg, setMsg] = useState("");
  const [params, setParams] = useState<StrategyParamsResp | null>(null);

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        // 먼저 연결 테스트
        console.log(`Connecting to backend: ${BACKEND}`);
        
        const healthRes = await fetch(`${BACKEND}/api/health`, { cache: 'no-cache' });
        if (!healthRes.ok) {
          throw new Error(`Backend not responding: ${healthRes.status}`);
        }
        console.log('Backend health check passed');
        
        const res = await fetch(`${BACKEND}/api/settings`, { cache: 'no-cache' });
        
        if (!res.ok) {
          throw new Error(`HTTP ${res.status}: ${res.statusText}`);
        }
        
        const json = await res.json();
        setData(json);
        
        const p = await getStrategyParams();
        setParams(p);
        
        console.log('Settings loaded successfully:', json);
        console.log('Strategy params loaded:', p);
      } catch (e: any) {
        console.error('설정 로드 오류:', e);
        setMsg(`설정 로드 실패: ${e.message || e}`);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const callAction = async (action: string) => {
    setMsg("");
    try {
      const res = await fetch(`${BACKEND}/api/settings`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      const json = await res.json();
      if (!json.ok) throw new Error(json.error || 'failed');
      setMsg(`성공: ${action}`);
    } catch (e: any) {
      setMsg(`실패: ${action} (${e.message || e})`);
    }
  };

  return (
    <main>
      <h2 style={{ marginBottom: 12 }}>설정</h2>
      {loading ? (
        <div>로딩 중…</div>
      ) : !data ? (
        <div>데이터 없음</div>
      ) : (
        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <h3>포트</h3>
            <div>API 포트: {data.api_port}</div>
            <div>메트릭 포트: {data.metrics_port}</div>
          </div>

          <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <h3>전략 상태(읽기 전용)</h3>
            <ul>
              {Object.entries(data.strategies).map(([k, v]) => (
                <li key={k}>{k}: {v ? 'ON' : 'OFF'}</li>
              ))}
            </ul>
          </div>

          <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <h3>액션</h3>
            <div style={{ display: 'flex', gap: 12 }}>
              <button onClick={() => callAction('reset_stats')} style={{ padding: '6px 10px', borderRadius: 6, border: '1px solid #ddd', cursor: 'pointer' }}>통계 초기화</button>
              <button onClick={() => callAction('ack_all_alerts')} style={{ padding: '6px 10px', borderRadius: 6, border: '1px solid #ddd', cursor: 'pointer' }}>알림 전체 확인</button>
            </div>
          </div>

          <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <h3>전략 파라미터(간단 편집)</h3>
            {!params ? (
              <div>파라미터 로딩 실패</div>
            ) : (
              <div style={{ display: 'grid', gap: 12 }}>
                <form onSubmit={async (e) => {
                  e.preventDefault();
                  const v = (e.currentTarget.elements.namedItem('v1') as HTMLInputElement).value;
                  const res = await updateStrategyParams('sandwich', { min_profit_eth: v });
                  setMsg(res.ok ? 'Sandwich 저장 완료(재시작 필요)' : `Sandwich 저장 실패: ${res.error}`);
                }}>
                  <label style={{ fontSize: 12, color: '#666' }}>Sandwich min_profit_eth</label>
                  <div style={{ display: 'flex', gap: 8 }}>
                    <input name="v1" defaultValue={params.sandwich.min_profit_eth} className="border p-2" />
                    <button type="submit" className="px-3 py-2 bg-black text-white rounded">Save</button>
                  </div>
                </form>

                {/* Liquidation v2.0 Settings */}
                <div style={{ border: '1px solid #e5e7eb', borderRadius: 8, padding: 16, backgroundColor: '#f8fafc' }}>
                  <h4 style={{ margin: '0 0 12px 0', color: '#374151' }}>Liquidation v2.0 설정</h4>
                  
                  <form onSubmit={async (e) => {
                    e.preventDefault();
                    const v = (e.currentTarget.elements.namedItem('v2') as HTMLInputElement).value;
                    const res = await updateStrategyParams('liquidation', { min_profit_eth: v });
                    setMsg(res.ok ? 'Liquidation 저장 완료(재시작 필요)' : `Liquidation 저장 실패: ${res.error}`);
                  }}>
                    <label style={{ fontSize: 12, color: '#666' }}>min_profit_eth</label>
                    <div style={{ display: 'flex', gap: 8 }}>
                      <input name="v2" defaultValue={params.liquidation.min_profit_eth} className="border p-2" />
                      <button type="submit" className="px-3 py-2 bg-black text-white rounded">Save</button>
                    </div>
                  </form>

                  <form onSubmit={async (e) => {
                    e.preventDefault();
                    const fundingMode = (e.currentTarget.elements.namedItem('l_funding') as HTMLSelectElement).value;
                    const maxFee = (e.currentTarget.elements.namedItem('l_max_fee') as HTMLInputElement).value;
                    const gasBuffer = (e.currentTarget.elements.namedItem('l_gas_buffer') as HTMLInputElement).value;
                    const updates: any = { funding_mode: fundingMode };
                    if (maxFee) updates.max_flashloan_fee_bps = parseInt(maxFee);
                    if (gasBuffer) updates.gas_buffer_pct = parseFloat(gasBuffer);
                    const res = await updateStrategyParams('liquidation', updates);
                    setMsg(res.ok ? 'Liquidation v2.0 설정 저장 완료(재시작 필요)' : `Liquidation v2.0 설정 실패: ${res.error}`);
                  }}>
                    <label style={{ fontSize: 12, color: '#666' }}>자금 조달 모드</label>
                    <div style={{ display: 'grid', gap: 8 }}>
                      <select name="l_funding" defaultValue={(params as any).liquidation?.funding_mode || 'auto'} className="border p-2">
                        <option value="auto">자동 선택 (최적 수익성)</option>
                        <option value="flashloan">플래시론 모드</option>
                        <option value="wallet">지갑 모드</option>
                      </select>
                      <div style={{ display: 'flex', gap: 8 }}>
                        <input name="l_max_fee" placeholder="최대 플래시론 수수료 (bps)" defaultValue={(params as any).liquidation?.max_flashloan_fee_bps || '9'} className="border p-2" />
                        <input name="l_gas_buffer" placeholder="가스 버퍼 (%)" defaultValue={(params as any).liquidation?.gas_buffer_pct || '20'} className="border p-2" />
                      </div>
                      <button type="submit" className="px-3 py-2 bg-black text-white rounded" style={{ width: 'fit-content' }}>Save</button>
                    </div>
                  </form>

                  <form onSubmit={async (e) => {
                    e.preventDefault();
                    const maxConcurrent = (e.currentTarget.elements.namedItem('l_concurrent') as HTMLInputElement).value;
                    const timeout = (e.currentTarget.elements.namedItem('l_timeout') as HTMLInputElement).value;
                    const dexEnabled = (e.currentTarget.elements.namedItem('l_dex_enabled') as HTMLInputElement).checked;
                    const preferredDex = (e.currentTarget.elements.namedItem('l_preferred_dex') as HTMLSelectElement).value;
                    const updates: any = {};
                    if (maxConcurrent) updates.max_concurrent_liquidations = parseInt(maxConcurrent);
                    if (timeout) updates.execution_timeout_ms = parseInt(timeout);
                    updates.dex_aggregator_enabled = dexEnabled;
                    if (dexEnabled) updates.preferred_dex_aggregator = preferredDex;
                    const res = await updateStrategyParams('liquidation', updates);
                    setMsg(res.ok ? 'Liquidation 실행 설정 저장 완료(재시작 필요)' : `Liquidation 실행 설정 실패: ${res.error}`);
                  }}>
                    <label style={{ fontSize: 12, color: '#666' }}>실행 설정</label>
                    <div style={{ display: 'grid', gap: 8 }}>
                      <div style={{ display: 'flex', gap: 8 }}>
                        <input name="l_concurrent" placeholder="최대 동시 청산 수" defaultValue={(params as any).liquidation?.max_concurrent_liquidations || '3'} className="border p-2" />
                        <input name="l_timeout" placeholder="실행 타임아웃 (ms)" defaultValue={(params as any).liquidation?.execution_timeout_ms || '5000'} className="border p-2" />
                      </div>
                      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                        <label style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
                          <input type="checkbox" name="l_dex_enabled" defaultChecked={(params as any).liquidation?.dex_aggregator_enabled || false} />
                          <span>DEX 어그리게이터 사용</span>
                        </label>
                        <select name="l_preferred_dex" defaultValue={(params as any).liquidation?.preferred_dex_aggregator || 'auto'} className="border p-2">
                          <option value="auto">자동 선택</option>
                          <option value="0x">0x Protocol</option>
                          <option value="1inch">1inch</option>
                        </select>
                      </div>
                      <button type="submit" className="px-3 py-2 bg-black text-white rounded" style={{ width: 'fit-content' }}>Save</button>
                    </div>
                  </form>
                </div>

                {/* Micro Arbitrage v2.0 Settings */}
                <div style={{ border: '1px solid #e5e7eb', borderRadius: 8, padding: 16, backgroundColor: '#f0f9ff' }}>
                  <h4 style={{ margin: '0 0 12px 0', color: '#374151' }}>Micro Arbitrage v2.0 설정</h4>
                  
                  <form onSubmit={async (e) => {
                    e.preventDefault();
                    const v = (e.currentTarget.elements.namedItem('v3') as HTMLInputElement).value;
                    const res = await updateStrategyParams('micro_arbitrage', { min_profit_usd: v });
                    setMsg(res.ok ? 'Micro 저장 완료(재시작 필요)' : `Micro 저장 실패: ${res.error}`);
                  }}>
                    <label style={{ fontSize: 12, color: '#666' }}>min_profit_usd</label>
                    <div style={{ display: 'flex', gap: 8 }}>
                      <input name="v3" defaultValue={params.micro_arbitrage.min_profit_usd} className="border p-2" />
                      <button type="submit" className="px-3 py-2 bg-black text-white rounded">Save</button>
                    </div>
                  </form>

                  <form onSubmit={async (e) => {
                    e.preventDefault();
                    const fundingMode = (e.currentTarget.elements.namedItem('m_funding') as HTMLSelectElement).value;
                    const maxFee = (e.currentTarget.elements.namedItem('m_max_fee') as HTMLInputElement).value;
                    const gasBuffer = (e.currentTarget.elements.namedItem('m_gas_buffer') as HTMLInputElement).value;
                    const updates: any = { funding_mode: fundingMode };
                    if (maxFee) updates.max_flashloan_fee_bps = parseInt(maxFee);
                    if (gasBuffer) updates.gas_buffer_pct = parseFloat(gasBuffer);
                    const res = await updateStrategyParams('micro_arbitrage', updates);
                    setMsg(res.ok ? 'Micro v2.0 자금조달 설정 저장 완료(재시작 필요)' : `Micro v2.0 자금조달 설정 실패: ${res.error}`);
                  }}>
                    <label style={{ fontSize: 12, color: '#666' }}>자금 조달 모드 (v2.0)</label>
                    <div style={{ display: 'grid', gap: 8 }}>
                      <select name="m_funding" defaultValue={(params as any).micro_arbitrage?.funding_mode || 'auto'} className="border p-2">
                        <option value="auto">자동 선택 (수익성 기반)</option>
                        <option value="flashloan">플래시론 모드</option>
                        <option value="wallet">지갑 모드</option>
                      </select>
                      <div style={{ display: 'flex', gap: 8 }}>
                        <input name="m_max_fee" placeholder="최대 플래시론 수수료 (bps)" defaultValue={(params as any).micro_arbitrage?.max_flashloan_fee_bps || '9'} className="border p-2" />
                        <input name="m_gas_buffer" placeholder="가스 버퍼 (%)" defaultValue={(params as any).micro_arbitrage?.gas_buffer_pct || '20'} className="border p-2" />
                      </div>
                      <button type="submit" className="px-3 py-2 bg-black text-white rounded" style={{ width: 'fit-content' }}>Save</button>
                    </div>
                  </form>

                  <form onSubmit={async (e) => {
                    e.preventDefault();
                    const priceInterval = (e.currentTarget.elements.namedItem('m_price_interval') as HTMLInputElement).value;
                    const orderbookInterval = (e.currentTarget.elements.namedItem('m_orderbook_interval') as HTMLInputElement).value;
                    const scanInterval = (e.currentTarget.elements.namedItem('m_scan_interval') as HTMLInputElement).value;
                    const updates: any = {};
                    if (priceInterval) updates.price_update_interval = parseInt(priceInterval);
                    if (orderbookInterval) updates.orderbook_refresh_interval = parseInt(orderbookInterval);
                    if (scanInterval) updates.opportunity_scan_interval = parseInt(scanInterval);
                    const res = await updateStrategyParams('micro_arbitrage', updates);
                    setMsg(res.ok ? 'RealTimeScheduler 설정 저장 완료(재시작 필요)' : `RealTimeScheduler 설정 실패: ${res.error}`);
                  }}>
                    <label style={{ fontSize: 12, color: '#666' }}>RealTimeScheduler 주배 설정 (ms)</label>
                    <div style={{ display: 'grid', gap: 8 }}>
                      <div style={{ display: 'flex', gap: 8 }}>
                        <input name="m_price_interval" placeholder="가격 업데이트 (10ms)" defaultValue={(params as any).micro_arbitrage?.price_update_interval || '10'} className="border p-2" />
                        <input name="m_orderbook_interval" placeholder="오더북 갱신 (50ms)" defaultValue={(params as any).micro_arbitrage?.orderbook_refresh_interval || '50'} className="border p-2" />
                        <input name="m_scan_interval" placeholder="기회 스캔 (100ms)" defaultValue={(params as any).micro_arbitrage?.opportunity_scan_interval || '100'} className="border p-2" />
                      </div>
                      <button type="submit" className="px-3 py-2 bg-black text-white rounded" style={{ width: 'fit-content' }}>Save</button>
                    </div>
                  </form>
                </div>

                {/* Sandwich flashloan is disabled by policy: toggle removed */}

                {/* Micro Aggregator Execution Settings */}
                <form onSubmit={async (e) => {
                  e.preventDefault();
                  const allow = (e.currentTarget.elements.namedItem('m_allow_agg') as HTMLInputElement).checked;
                  const use0x = (e.currentTarget.elements.namedItem('m_use_0x') as HTMLInputElement).checked;
                  const use1inch = (e.currentTarget.elements.namedItem('m_use_1inch') as HTMLInputElement).checked;
                  const preferred: string[] = [];
                  if (use0x) preferred.push('0x');
                  if (use1inch) preferred.push('1inch');
                  const updates: any = { allow_aggregator_execution: allow, preferred_aggregators: preferred };
                  const res = await updateStrategyParams('micro', updates);
                  setMsg(res.ok ? 'Micro 집계기 실행 설정 저장 완료(재시작 필요)' : `Micro 집계기 설정 저장 실패: ${res.error}`);
                }}>
                  <label style={{ fontSize: 12, color: '#666' }}>Micro Aggregator Execution</label>
                  <div style={{ display: 'grid', gap: 8, alignItems: 'center' }}>
                    <label style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                      <input type="checkbox" name="m_allow_agg" defaultChecked={Boolean((params as any).micro_arbitrage?.allow_aggregator_execution)} />
                      <span>집계기(to/data/allowanceTarget) 경로 실행 허용</span>
                    </label>
                    <div style={{ display: 'flex', gap: 16 }}>
                      <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                        <input type="checkbox" name="m_use_0x" defaultChecked={Array.isArray((params as any).micro_arbitrage?.preferred_aggregators) && (params as any).micro_arbitrage.preferred_aggregators.includes('0x')} />
                        <span>0x</span>
                      </label>
                      <label style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
                        <input type="checkbox" name="m_use_1inch" defaultChecked={Array.isArray((params as any).micro_arbitrage?.preferred_aggregators) && (params as any).micro_arbitrage.preferred_aggregators.includes('1inch')} />
                        <span>1inch</span>
                      </label>
                    </div>
                    <button type="submit" className="px-3 py-2 bg-black text-white rounded" style={{ width: 'fit-content' }}>Save</button>
                  </div>
                </form>

                <form onSubmit={async (e) => {
                  e.preventDefault();
                  const enabled = (e.currentTarget.elements.namedItem('c_flash') as HTMLInputElement).checked;
                  const amount = (e.currentTarget.elements.namedItem('c_amt') as HTMLInputElement).value;
                  const updates: any = { use_flashloan: enabled };
                  if (amount) updates.flash_loan_amount = amount;
                  const res = await updateStrategyParams('cross_chain_arbitrage', updates);
                  setMsg(res.ok ? 'Cross-chain flashloan 저장 완료(재시작 필요)' : `Cross-chain flashloan 저장 실패: ${res.error}`);
                }}>
                  <label style={{ fontSize: 12, color: '#666' }}>Cross-Chain Flashloan</label>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    <input type="checkbox" name="c_flash" defaultChecked={Boolean((params as any).cross_chain_arbitrage?.use_flashloan)} />
                    <input name="c_amt" placeholder="amount (optional)" defaultValue={(params as any).cross_chain_arbitrage?.flash_loan_amount || ''} className="border p-2" />
                    <button type="submit" className="px-3 py-2 bg-black text-white rounded">Save</button>
                  </div>
                </form>

                <div style={{ fontSize: 12, color: '#999' }}>저장 시 `config/default.toml`에 반영되며, 적용에는 재시작이 필요합니다.</div>
              </div>
            )}
          </div>

          {msg && <div style={{ color: '#2563eb' }}>{msg}</div>}
        </div>
      )}
    </main>
  );
}
