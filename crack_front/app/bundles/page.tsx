import { getBundlesSummary, getBundlesRecent } from "@/lib/api";

export const dynamic = 'force-dynamic';

export default async function BundlesPage() {
  const [summary, recent] = await Promise.all([
    getBundlesSummary().catch(() => ({ stats: { total_included: 0, total_failed: 0, success_rate: 0, avg_submission_time_ms: 0, total_created: 0, total_submitted: 0, total_profit: 0, total_gas_spent: 0 }, submitted_count: 0, pending_count: 0 })),
    getBundlesRecent(50).catch(() => []),
  ]);

  return (
    <main>
      <h2 style={{ marginBottom: 12 }}>번들</h2>
      <div style={{ display: 'flex', gap: 16, marginBottom: 16 }}>
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>포함</div>
          <div style={{ fontWeight: 700 }}>{summary.stats.total_included}</div>
        </div>
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>실패</div>
          <div style={{ fontWeight: 700 }}>{summary.stats.total_failed}</div>
        </div>
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>성공률</div>
          <div style={{ fontWeight: 700 }}>{((summary.stats.success_rate || 0) * 100).toFixed(2)}%</div>
        </div>
        <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 12 }}>
          <div style={{ fontSize: 12, color: '#888' }}>평균 제출 시간</div>
          <div style={{ fontWeight: 700 }}>{(summary.stats.avg_submission_time_ms || 0).toFixed(2)} ms</div>
        </div>
      </div>

      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>최근 번들(50)</h3>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              <th style={{ textAlign: 'left', padding: 8 }}>ID</th>
              <th style={{ textAlign: 'left', padding: 8 }}>전략</th>
              <th style={{ textAlign: 'right', padding: 8 }}>예상이익</th>
              <th style={{ textAlign: 'right', padding: 8 }}>가스</th>
              <th style={{ textAlign: 'left', padding: 8 }}>상태</th>
              <th style={{ textAlign: 'left', padding: 8 }}>시간</th>
            </tr>
          </thead>
          <tbody>
            {recent.map((r) => (
              <tr key={r.id}>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0' }}>{r.id.slice(0, 8)}…</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0' }}>{r.strategy}</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0', textAlign: 'right' }}>{r.expected_profit}</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0', textAlign: 'right' }}>{r.gas_estimate}</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0' }}>{r.state}</td>
                <td style={{ padding: 8, borderTop: '1px solid #f0f0f0' }}>{String(r.timestamp).replace('T', ' ').replace('Z', '')}</td>
              </tr>
            ))}
            {recent.length === 0 && (
              <tr>
                <td colSpan={6} style={{ padding: 12, color: '#888' }}>최근 번들이 없습니다</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </main>
  );
}
