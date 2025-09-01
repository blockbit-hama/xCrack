import Link from 'next/link';
import { getBundlesSummary, getBundlesRecent } from '@/lib/api';

export const dynamic = 'force-dynamic';

export default async function BundlesPage() {
  const summary = await getBundlesSummary();
  const recent = await getBundlesRecent(20);

  // recent가 배열인지 확인하고 안전하게 처리
  const recentBundles = Array.isArray(recent) ? recent : [];

  return (
    <div>
      <h1 className="text-2xl font-bold mb-4">Bundles</h1>
      <div className="grid grid-cols-2 gap-4 mb-6">
        <div className="p-4 border rounded">
          <div className="text-gray-500">Total Submitted</div>
          <div className="text-xl">{summary?.stats?.total_submitted || 0}</div>
        </div>
        <div className="p-4 border rounded">
          <div className="text-gray-500">Total Included</div>
          <div className="text-xl">{summary?.stats?.total_included || 0}</div>
        </div>
      </div>

      <table className="w-full border">
        <thead>
          <tr className="bg-gray-100">
            <th className="p-2 text-left">ID</th>
            <th className="p-2 text-left">Strategy</th>
            <th className="p-2 text-left">Expected Profit</th>
            <th className="p-2 text-left">Gas Est.</th>
            <th className="p-2 text-left">Time</th>
            <th className="p-2 text-left">State</th>
          </tr>
        </thead>
        <tbody>
          {recentBundles.length > 0 ? (
            recentBundles.map((b) => (
              <tr key={b.id || Math.random()} className="border-t">
                <td className="p-2">
                  <Link href={`/bundles/${b.id || 'unknown'}`} className="text-blue-600 underline">
                    {b.id ? b.id.slice(0, 10) + '...' : 'Unknown'}
                  </Link>
                </td>
                <td className="p-2">{b.strategy || 'Unknown'}</td>
                <td className="p-2">{b.expected_profit || '0'}</td>
                <td className="p-2">{b.gas_estimate || '0'}</td>
                <td className="p-2">{b.timestamp || 'Unknown'}</td>
                <td className="p-2">{b.state || 'Unknown'}</td>
              </tr>
            ))
          ) : (
            <tr>
              <td colSpan={6} className="p-4 text-center text-gray-500">
                최근 번들이 없습니다.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
