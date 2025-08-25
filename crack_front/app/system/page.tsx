import { getSystemInfo } from '@/lib/api'

export const dynamic = 'force-dynamic'

export default async function SystemPage() {
  const sys = await getSystemInfo()
  return (
    <main>
      <h2 className="text-xl font-semibold mb-3">시스템 정보</h2>
      {!sys ? (
        <div>시스템 정보를 불러올 수 없습니다.</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="border rounded-lg p-4">
            <div className="text-gray-500 text-sm">API 모드</div>
            <div className="text-lg">{sys.api_mode} {sys.simulation_mode ? '(simulation)' : ''}</div>
          </div>
          <div className="border rounded-lg p-4">
            <div className="text-gray-500 text-sm">네트워크</div>
            <div className="text-lg">{sys.network}</div>
          </div>
          <div className="border rounded-lg p-4 md:col-span-2">
            <div className="text-gray-500 text-sm">RPC / WS</div>
            <div className="text-sm break-all">RPC: {sys.rpc_url}</div>
            <div className="text-sm break-all">WS: {sys.ws_url || '-'}</div>
          </div>
          <div className="border rounded-lg p-4 md:col-span-2">
            <div className="text-gray-500 text-sm">Flashbots Relay</div>
            <div className="text-sm break-all">{sys.flashbots_relay_url}</div>
          </div>
          <div className="border rounded-lg p-4 md:col-span-2">
            <div className="text-gray-500 text-sm">외부 API</div>
            <ul className="list-disc pl-5 text-sm">
              {sys.external_apis.map((e, i) => <li key={i}>{e}</li>)}
              {sys.external_apis.length === 0 && <li>없음</li>}
            </ul>
          </div>
        </div>
      )}
    </main>
  )
}
