import Link from 'next/link';
import { getBundlesSummary, getBundlesRecent } from '@/lib/api';

export const dynamic = 'force-dynamic';

export default async function BundlesPage() {
  const summary = await getBundlesSummary();
  const recent = await getBundlesRecent(20);

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
          {(recent || []).map((b) => (
            <tr key={b.id} className="border-t">
              <td className="p-2">
                <Link href={`/bundles/${b.id}`} className="text-blue-600 underline">{b.id.slice(0, 10)}...</Link>
              </td>
              <td className="p-2">{b.strategy}</td>
              <td className="p-2">{b.expected_profit}</td>
              <td className="p-2">{b.gas_estimate}</td>
              <td className="p-2">{b.timestamp}</td>
              <td className="p-2">{b.state}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
