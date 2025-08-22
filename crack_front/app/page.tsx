import { getStatus, defaultStatus, getBundlesSummary, getBundlesRecent, getReport } from "../lib/api";

export const dynamic = 'force-dynamic';

export default async function Page() {
  const [status, bundles, recent, report] = await Promise.all([
    getStatus().catch(() => defaultStatus()),
    getBundlesSummary().catch(() => ({ stats: { total_created: 0, total_submitted: 0, total_included: 0, total_failed: 0, total_profit: 0, total_gas_spent: 0, avg_submission_time_ms: 0, success_rate: 0 }, submitted_count: 0, pending_count: 0 })),
    getBundlesRecent(5).catch(() => []),
    getReport().catch(() => ({ summary: { transactions_processed: 0, opportunities_found: 0, bundles_submitted: 0, bundles_included: 0, total_profit_eth: '0', success_rate: 0, avg_analysis_time_ms: 0, avg_submission_time_ms: 0 }, recommendations: [] })),
  ]);

  return (
    <main style={{ display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))', gap: 16 }}>
      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>실행 상태</h3>
        <p>{status.is_running ? '실행 중' : '중지'}</p>
      </div>
      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>기회 처리</h3>
        <p>활성 기회: {status.active_opportunities}</p>
      </div>
      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>번들</h3>
        <p>제출: {status.submitted_bundles}</p>
        <p>통계: 포함 {bundles.stats.total_included} / 실패 {bundles.stats.total_failed}</p>
        <p>대기:{' '}{bundles.pending_count} / 제출:{' '}{bundles.submitted_count}</p>
      </div>
      <div style={{ gridColumn: 'span 3', border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>최근 번들(5)</h3>
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
          </tbody>
        </table>
      </div>
      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>총 수익(ETH)</h3>
        <p>{status.total_profit_eth}</p>
      </div>
      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>성공률</h3>
        <p>{(status.success_rate * 100).toFixed(2)}%</p>
      </div>
      <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>가동 시간</h3>
        <p>{status.uptime_seconds}s</p>
      </div>
      <div style={{ gridColumn: 'span 3', border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
        <h3>요약 지표 & 권장사항</h3>
        <div style={{ display: 'flex', gap: 24, marginBottom: 12 }}>
          <div>평균 분석 시간: {report.summary.avg_analysis_time_ms.toFixed(2)} ms</div>
          <div>평균 제출 시간: {report.summary.avg_submission_time_ms.toFixed(2)} ms</div>
          <div>성공률: {(report.summary.success_rate * 100).toFixed(2)}%</div>
        </div>
        <ul>
          {report.recommendations.slice(0, 3).map((r, i) => (
            <li key={i}>• {r}</li>
          ))}
          {report.recommendations.length === 0 && <li>권장사항 없음</li>}
        </ul>
      </div>
    </main>
  );
}
