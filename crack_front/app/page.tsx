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
    <main className="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-1">실행 상태</h3>
        <p>{status.is_running ? '실행 중' : '중지'}</p>
      </div>
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-1">기회 처리</h3>
        <p>활성 기회: {status.active_opportunities}</p>
      </div>
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-1">번들</h3>
        <p>제출: {status.submitted_bundles}</p>
        <p>통계: 포함 {bundles.stats.total_included} / 실패 {bundles.stats.total_failed}</p>
        <p>대기: {bundles.pending_count} / 제출: {bundles.submitted_count}</p>
      </div>
      <div className="md:col-span-3 border rounded-lg p-4">
        <h3 className="font-semibold mb-2">최근 번들(5)</h3>
        <table className="w-full border-collapse">
          <thead>
            <tr className="text-left">
              <th className="p-2">ID</th>
              <th className="p-2">전략</th>
              <th className="p-2 text-right">예상이익</th>
              <th className="p-2 text-right">가스</th>
              <th className="p-2">상태</th>
              <th className="p-2">시간</th>
            </tr>
          </thead>
          <tbody>
            {recent.map((r) => (
              <tr key={r.id} className="border-t">
                <td className="p-2">{r.id.slice(0, 8)}…</td>
                <td className="p-2">{r.strategy}</td>
                <td className="p-2 text-right">{r.expected_profit}</td>
                <td className="p-2 text-right">{r.gas_estimate}</td>
                <td className="p-2">{r.state}</td>
                <td className="p-2">{String(r.timestamp).replace('T', ' ').replace('Z', '')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-1">총 수익(ETH)</h3>
        <p>{status.total_profit_eth}</p>
      </div>
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-1">성공률</h3>
        <p>{(status.success_rate * 100).toFixed(2)}%</p>
      </div>
      <div className="border rounded-lg p-4">
        <h3 className="font-semibold mb-1">가동 시간</h3>
        <p>{status.uptime_seconds}s</p>
      </div>
      <div className="md:col-span-3 border rounded-lg p-4">
        <h3 className="font-semibold mb-2">요약 지표 & 권장사항</h3>
        <div className="flex flex-col md:flex-row gap-6 mb-3 text-sm">
          <div>평균 분석 시간: {report.summary.avg_analysis_time_ms.toFixed(2)} ms</div>
          <div>평균 제출 시간: {report.summary.avg_submission_time_ms.toFixed(2)} ms</div>
          <div>성공률: {(report.summary.success_rate * 100).toFixed(2)}%</div>
        </div>
        <ul className="list-disc pl-5 text-sm">
          {report.recommendations.slice(0, 3).map((r, i) => (
            <li key={i}>{r}</li>
          ))}
          {report.recommendations.length === 0 && <li>권장사항 없음</li>}
        </ul>
      </div>
    </main>
  );
}
