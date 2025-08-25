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
            <div className="text-gray-500 text-sm mb-2">외부 API</div>
            {sys.external_apis.length === 0 ? (
              <div className="text-sm text-gray-500">없음</div>
            ) : (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                {sys.external_apis.map((api, i) => (
                  <div key={i} className="border rounded p-3">
                    <div className="font-semibold text-sm">{api.name}</div>
                    <div className="text-xs text-gray-500">{api.category}</div>
                    <div className="text-sm mt-1">{api.description}</div>
                    {api.docs && (
                      <div className="text-xs mt-1">
                        문서: <a className="text-blue-600 underline" href={api.docs} target="_blank">{api.docs}</a>
                      </div>
                    )}
                    {api.env?.length ? (
                      <div className="mt-2">
                        <div className="text-xs text-gray-500">관련 환경변수</div>
                        <ul className="text-xs mt-1">
                          {api.env.map((v, j) => (
                            <li key={j} className="flex items-center gap-2">
                              <code className="bg-gray-100 px-1 rounded">{v.key}</code>
                              <span className={v.set ? 'text-emerald-600' : 'text-gray-400'}>{v.set ? '설정됨' : '미설정'}</span>
                            </li>
                          ))}
                        </ul>
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </main>
  )
}
