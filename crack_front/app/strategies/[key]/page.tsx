import Link from "next/link";
import { getStrategyStats, getBundlesRecent } from "../../../lib/api";

export default async function StrategyDetail({ params }: { params: Promise<{ key: string }> }) {
  const { key } = await params;
  const nameMap: Record<string, string> = {
    sandwich: "Sandwich",
    liquidation: "Liquidation",
    micro: "Micro Arbitrage",
  };
  const display = nameMap[key] || key;

  const [stats, recent] = await Promise.all([
    getStrategyStats().catch(() => ({} as any)),
    getBundlesRecent(20).catch(() => []),
  ]);

  const st: any = (stats as any)[display] || (stats as any)[key] || undefined;
  const bundles = recent.filter((b) => (b.strategy || "").toLowerCase().includes(display.toLowerCase()) || (b.strategy || "").toLowerCase().includes(key.toLowerCase()));

  // Build simple profit trend (last N) for inline sparkline (no deps)
  const profits = bundles
    .map((b) => {
      const s = String(b.expected_profit || "0").replace(/[^0-9.\-eE]/g, "");
      const v = parseFloat(s || "0");
      return isNaN(v) ? 0 : v;
    })
    .slice(0, 20)
    .reverse();
  const maxP = profits.length ? Math.max(...profits) : 1;
  const minP = profits.length ? Math.min(...profits) : 0;
  const width = 240;
  const height = 60;
  const points = profits.map((p, i) => {
    const x = (i / Math.max(1, profits.length - 1)) * width;
    const norm = maxP === minP ? 0.5 : (p - minP) / (maxP - minP);
    const y = height - norm * height;
    return `${x},${y}`;
  }).join(" ");

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
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>최근 수익 추세</div>
          <svg width={width} height={height} style={{ display: 'block' }}>
            <polyline fill="none" stroke="#2563eb" strokeWidth="2" points={points} />
          </svg>
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
