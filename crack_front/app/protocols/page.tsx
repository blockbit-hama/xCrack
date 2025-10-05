import { getProtocolStatus } from "../../lib/api";

export default async function ProtocolsPage() {
  const protocolStatus = await getProtocolStatus().catch(() => []);

  return (
    <main className="p-6 space-y-6">
      <div className="border-b pb-4">
        <h1 className="text-3xl font-bold">프로토콜 스캐너</h1>
        <p className="text-gray-600 mt-1">DeFi 프로토콜 실시간 모니터링</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">연결된 프로토콜</h3>
          <p className="text-2xl font-bold">{protocolStatus.filter(p => p.status === 'active').length}</p>
          <p className="text-sm text-gray-600">총 {protocolStatus.length}개 중</p>
        </div>
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">모니터링 중</h3>
          <p className="text-2xl font-bold">{protocolStatus.length}</p>
        </div>
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">총 TVL</h3>
          <p className="text-2xl font-bold text-green-600">
            {protocolStatus.length > 0 ? protocolStatus[0].total_tvl : '0'}
          </p>
        </div>
      </div>

      <div className="border rounded-lg p-6">
        <h2 className="text-xl font-semibold mb-4">프로토콜 목록</h2>
        {protocolStatus.length === 0 ? (
          <div className="border rounded-lg p-8 text-center">
            <p className="text-gray-500">프로토콜 스캐너가 연결되지 않았습니다</p>
          </div>
        ) : (
          <div className="space-y-4">
            {protocolStatus.map((protocol, index) => (
              <div key={index} className="border rounded-lg p-6">
                <div className="flex items-center justify-between mb-4">
                  <div className="flex items-center space-x-3">
                    <div className="w-12 h-12 bg-gradient-to-r from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
                      <span className="text-white font-bold text-lg">
                        {protocol.protocol.charAt(0).toUpperCase()}
                      </span>
                    </div>
                    <div>
                      <h2 className="text-xl font-bold capitalize">{protocol.protocol}</h2>
                      <p className="text-gray-600">DeFi 프로토콜</p>
                    </div>
                  </div>
                  <div className="flex items-center space-x-2">
                    <span className={`px-3 py-1 rounded-full text-sm font-medium ${
                      protocol.status === 'active'
                        ? 'bg-green-100 text-green-800'
                        : 'bg-red-100 text-red-800'
                    }`}>
                      {protocol.status === 'active' ? '연결됨' : '연결 안됨'}
                    </span>
                    <div className={`w-3 h-3 rounded-full ${
                      protocol.status === 'active' ? 'bg-green-400 animate-pulse' : 'bg-red-400'
                    }`}></div>
                  </div>
                </div>

                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                  <div className="bg-blue-50 p-4 rounded-lg">
                    <h3 className="text-sm font-medium text-blue-700 mb-1">상태</h3>
                    <p className="text-lg font-bold text-blue-900 capitalize">{protocol.status}</p>
                  </div>
                  <div className="bg-green-50 p-4 rounded-lg">
                    <h3 className="text-sm font-medium text-green-700 mb-1">TVL</h3>
                    <p className="text-lg font-bold text-green-900">{protocol.total_tvl}</p>
                  </div>
                  <div className="bg-purple-50 p-4 rounded-lg">
                    <h3 className="text-sm font-medium text-purple-700 mb-1">마지막 업데이트</h3>
                    <p className="text-sm text-purple-900">
                      {new Date(protocol.last_update * 1000).toLocaleString()}
                    </p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </main>
  );
}
