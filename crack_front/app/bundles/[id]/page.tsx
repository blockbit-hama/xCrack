import { getBundle } from '../../../lib/api';

export const dynamic = 'force-dynamic';

type Props = { params: Promise<{ id: string }> };

export default async function BundleDetail({ params }: Props) {
  const { id } = await params;
  const bundle = await getBundle(id);
  if (!bundle) {
    return <div className="p-4">Bundle not found</div>;
  }
  return (
    <div className="p-4">
      <h1 className="text-2xl font-bold mb-4">Bundle {id}</h1>
      <div className="grid grid-cols-2 gap-4 mb-6">
        <div className="p-4 border rounded">
          <div className="text-gray-500">Strategy</div>
          <div className="text-xl">{String(bundle.strategy || '')}</div>
        </div>
        <div className="p-4 border rounded">
          <div className="text-gray-500">Expected Profit</div>
          <div className="text-xl">{String(bundle.expected_profit || '0')}</div>
        </div>
        <div className="p-4 border rounded">
          <div className="text-gray-500">Gas Estimate</div>
          <div className="text-xl">{Number(bundle.gas_estimate || 0)}</div>
        </div>
        <div className="p-4 border rounded">
          <div className="text-gray-500">Timestamp</div>
          <div className="text-xl">{String(bundle.timestamp || '')}</div>
        </div>
      </div>

      <pre className="p-4 bg-gray-50 border rounded text-sm overflow-auto">{JSON.stringify(bundle, null, 2)}</pre>
    </div>
  );
}
