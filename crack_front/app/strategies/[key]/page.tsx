import Link from "next/link";
import { getStrategyStats, getBundlesRecent } from "@/lib/api";

export default async function StrategyDetail({ params }: { params: { key: string } }) {
  const key = params.key;
  const nameMap: Record<string, string> = {
    sandwich: "Sandwich",
    liquidation: "Liquidation",
    micro: "Micro Arbitrage",
    cross: "Cross-Chain",
  };
  const display = nameMap[key] || key;

  const [stats, recent] = await Promise.all([
    getStrategyStats().catch(() => ({} as any)),
    getBundlesRecent(20).catch(() => []),
  ]);

  const st: any = (stats as any)[display] || (stats as any)[key] || undefined;
  const bundles = recent.filter((b) => (b.strategy || "").toLowerCase().includes(display.toLowerCase()) || (b.strategy || "").toLowerCase().includes(key.toLowerCase()));

  return (
    <main>
      <div style={{ marginBottom: 12 }}>
        <Link href="/strategies">← 전략 목록</Link>
      </div>
      <h2 style={{ marginBottom: 8 }}>{display} 전략 상세</h2>
      <div style={{ display: 'flex', gap: 16, marginBottom: 16 }}>
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>분석 건수</div>
          <div style={{ fontWeight: 700 }}>{st ? st.transactions_analyzed : '-'}</div>
        </div>
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>발견 건수</div>
          <div style={{ fontWeight: 700 }}>{st ? st.opportunities_found : '-'}</div>
        </div>
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>평균 분석 시간</div>
          <div style={{ fontWeight: 700 }}>{st ? `${st.avg_analysis_time_ms.toFixed(1)} ms` : '-'}</div>
        </div>
      </div>

      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>최근 번들(20)</h3>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              <th style={{ textAlign: 'left', padding: 8 }}>ID</th>
              <th style={{ textAlign: 'right', padding: 8 }}>예상이익</th>
              <th style={{ textAlign: 'right', padding: 8 }}>가스</th>
              <th style={{ textAlign: 'left', padding: 8 }}>상태</th>
              <th style={{ textAlign: 'left', padding: 8 }}>시간</th>
            </tr>
          </thead>
          <tbody>
            {bundles.map((r) => (
              <tr key={r.id}>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0' }}>{r.id.slice(0, 8)}…</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0', textAlign: 'right' }}>{r.expected_profit}</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0', textAlign: 'right' }}>{r.gas_estimate}</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0' }}>{r.state}</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0' }}>{String(r.timestamp).replace('T', ' ').replace('Z', '')}</td>
              </tr>
            ))}
            {bundles.length === 0 && (
              <tr>
                <td colSpan={5} style={{ padding: 12, color: '#888' }}>해당 전략의 최근 번들이 없습니다</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </main>
  );
}
