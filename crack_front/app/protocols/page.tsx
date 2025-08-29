import { getProtocolStatus } from "../../lib/api";

export const dynamic = 'force-dynamic';

export default async function ProtocolsPage() {
  const protocolStatus = await getProtocolStatus().catch(() => []);

  return (
    <main className="space-y-6">
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-2xl font-bold">프로토콜 모니터링</h1>
        <p className="text-gray-600 mt-1">Aave/Compound 실시간 포지션 스캐너 상태</p>
      </div>

      {/* 프로토콜 전체 상태 */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">연결된 프로토콜</h3>
          <p className="text-2xl font-bold">{protocolStatus.filter(p => p.connected).length}</p>
          <p className="text-sm text-gray-600">총 {protocolStatus.length}개 중</p>
        </div>
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">모니터링 사용자</h3>
          <p className="text-2xl font-bold">
            {protocolStatus.reduce((sum, p) => sum + p.total_users, 0).toLocaleString()}
          </p>
        </div>
        <div className="border rounded-lg p-4">
          <h3 className="font-semibold mb-1">청산 위험 포지션</h3>
          <p className="text-2xl font-bold text-red-600">
            {protocolStatus.reduce((sum, p) => sum + p.liquidatable_positions, 0)}
          </p>
        </div>
      </div>

      {/* 프로토콜별 상세 정보 */}
      <div className="space-y-4">
        {protocolStatus.length === 0 ? (
          <div className="border rounded-lg p-8 text-center">
            <p className="text-gray-500">프로토콜 스캐너가 연결되지 않았습니다</p>
          </div>
        ) : (
          protocolStatus.map((protocol) => (
            <div key={protocol.protocol} className="border rounded-lg p-6">
              <div className="flex items-center justify-between mb-4">
                <div className="flex items-center space-x-3">
                  <div className="w-12 h-12 bg-gradient-to-r from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
                    <span className="text-white font-bold text-lg">
                      {protocol.protocol.charAt(0).toUpperCase()}
                    </span>
                  </div>
                  <div>
                    <h2 className="text-xl font-bold capitalize">{protocol.protocol}</h2>
                    <p className="text-gray-600">DeFi 대출 프로토콜</p>
                  </div>
                </div>
                <div className="flex items-center space-x-2">
                  <span className={`px-3 py-1 rounded-full text-sm font-medium ${
                    protocol.connected 
                      ? 'bg-green-100 text-green-800' 
                      : 'bg-red-100 text-red-800'
                  }`}>
                    {protocol.connected ? '연결됨' : '연결 안됨'}
                  </span>
                  <div className={`w-3 h-3 rounded-full ${
                    protocol.connected ? 'bg-green-400 animate-pulse' : 'bg-red-400'
                  }`}></div>
                </div>
              </div>

              {protocol.connected ? (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                  {/* 사용자 통계 */}
                  <div className="bg-blue-50 p-4 rounded-lg">
                    <h3 className="text-sm font-medium text-blue-700 mb-1">모니터링 사용자</h3>
                    <p className="text-2xl font-bold text-blue-900">{protocol.total_users.toLocaleString()}</p>
                    <div className="text-xs text-blue-600 mt-1">
                      마지막 스캔: {new Date(protocol.last_scan_time).toLocaleTimeString()}
                    </div>
                  </div>

                  {/* 청산 위험 */}
                  <div className="bg-red-50 p-4 rounded-lg">
                    <h3 className="text-sm font-medium text-red-700 mb-1">청산 위험 포지션</h3>
                    <p className="text-2xl font-bold text-red-900">{protocol.liquidatable_positions}</p>
                    <div className="text-xs text-red-600 mt-1">
                      위험 비율: {protocol.total_users > 0 ? ((protocol.liquidatable_positions / protocol.total_users) * 100).toFixed(2) : 0}%
                    </div>
                  </div>

                  {/* 담보 총액 */}
                  <div className="bg-green-50 p-4 rounded-lg">
                    <h3 className="text-sm font-medium text-green-700 mb-1">총 담보</h3>
                    <p className="text-lg font-bold text-green-900">
                      ${(parseFloat(protocol.total_collateral_usd) / 1000000).toFixed(1)}M
                    </p>
                    <div className="text-xs text-green-600 mt-1">
                      평균 건강도: {protocol.avg_health_factor.toFixed(3)}
                    </div>
                  </div>

                  {/* 성능 */}
                  <div className="bg-purple-50 p-4 rounded-lg">
                    <h3 className="text-sm font-medium text-purple-700 mb-1">스캔 성능</h3>
                    <p className="text-lg font-bold text-purple-900">{protocol.scan_latency_ms}ms</p>
                    <div className="text-xs text-purple-600 mt-1">
                      부채 총액: ${(parseFloat(protocol.total_debt_usd) / 1000000).toFixed(1)}M
                    </div>
                  </div>
                </div>
              ) : (
                <div className="bg-gray-50 p-4 rounded-lg text-center">
                  <p className="text-gray-600">프로토콜 스캐너가 연결되지 않았습니다</p>
                  <p className="text-sm text-gray-500 mt-1">설정을 확인하거나 재시작해주세요</p>
                </div>
              )}

              {/* 추가 정보 */}
              {protocol.connected && (
                <div className="mt-4 pt-4 border-t">
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
                    <div>
                      <span className="text-gray-500">총 담보/부채 비율:</span>
                      <span className="ml-2 font-medium">
                        {(parseFloat(protocol.total_collateral_usd) / parseFloat(protocol.total_debt_usd)).toFixed(2)}
                      </span>
                    </div>
                    <div>
                      <span className="text-gray-500">평균 사용자 담보:</span>
                      <span className="ml-2 font-medium">
                        ${protocol.total_users > 0 ? (parseFloat(protocol.total_collateral_usd) / protocol.total_users).toFixed(0) : 0}
                      </span>
                    </div>
                    <div>
                      <span className="text-gray-500">스캔 상태:</span>
                      <span className={`ml-2 font-medium ${
                        protocol.scan_latency_ms < 100 ? 'text-green-600' :
                        protocol.scan_latency_ms < 500 ? 'text-yellow-600' : 'text-red-600'
                      }`}>
                        {protocol.scan_latency_ms < 100 ? '최적' :
                         protocol.scan_latency_ms < 500 ? '양호' : '지연'}
                      </span>
                    </div>
                  </div>
                </div>
              )}
            </div>
          ))
        )}
      </div>

      {/* 건강도 분포 */}
      {protocolStatus.some(p => p.connected) && (
        <div className="border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">건강도 분포 분석</h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-red-50 p-4 rounded-lg">
              <h3 className="font-medium text-red-700">위험 (HF < 1.1)</h3>
              <p className="text-2xl font-bold text-red-800">
                {protocolStatus.reduce((sum, p) => sum + p.liquidatable_positions, 0)}
              </p>
              <div className="text-sm text-red-600">즉시 청산 위험</div>
            </div>
            <div className="bg-yellow-50 p-4 rounded-lg">
              <h3 className="font-medium text-yellow-700">주의 (HF 1.1-1.5)</h3>
              <p className="text-2xl font-bold text-yellow-800">-</p>
              <div className="text-sm text-yellow-600">모니터링 필요</div>
            </div>
            <div className="bg-green-50 p-4 rounded-lg">
              <h3 className="font-medium text-green-700">안전 (HF > 1.5)</h3>
              <p className="text-2xl font-bold text-green-800">-</p>
              <div className="text-sm text-green-600">정상 상태</div>
            </div>
          </div>
        </div>
      )}

      {/* 프로토콜 비교 */}
      {protocolStatus.length > 1 && (
        <div className="border rounded-lg p-6">
          <h2 className="text-lg font-semibold mb-4">프로토콜 비교</h2>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b">
                  <th className="text-left py-2">프로토콜</th>
                  <th className="text-right py-2">사용자 수</th>
                  <th className="text-right py-2">총 담보 (M$)</th>
                  <th className="text-right py-2">총 부채 (M$)</th>
                  <th className="text-right py-2">평균 건강도</th>
                  <th className="text-right py-2">청산 위험</th>
                  <th className="text-right py-2">스캔 속도</th>
                </tr>
              </thead>
              <tbody>
                {protocolStatus.map((protocol) => (
                  <tr key={protocol.protocol} className="border-b">
                    <td className="py-2 font-medium capitalize">{protocol.protocol}</td>
                    <td className="py-2 text-right">{protocol.total_users.toLocaleString()}</td>
                    <td className="py-2 text-right">
                      ${(parseFloat(protocol.total_collateral_usd) / 1000000).toFixed(1)}M
                    </td>
                    <td className="py-2 text-right">
                      ${(parseFloat(protocol.total_debt_usd) / 1000000).toFixed(1)}M
                    </td>
                    <td className="py-2 text-right font-medium">
                      {protocol.avg_health_factor.toFixed(3)}
                    </td>
                    <td className="py-2 text-right">
                      <span className="text-red-600 font-medium">
                        {protocol.liquidatable_positions}
                      </span>
                    </td>
                    <td className="py-2 text-right">
                      <span className={`${
                        protocol.scan_latency_ms < 100 ? 'text-green-600' :
                        protocol.scan_latency_ms < 500 ? 'text-yellow-600' : 'text-red-600'
                      }`}>
                        {protocol.scan_latency_ms}ms
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </main>
  );
}