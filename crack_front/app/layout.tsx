import './globals.css';
export const metadata = {
  title: 'xCrack Dashboard',
  description: 'xCrack MEV Searcher UI',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  const ApiHealth = require('./components/ApiHealth').default;
  return (
    <html lang="ko">
      <body style={{ fontFamily: 'system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial', margin: 0 }}>
        <div style={{ display: 'flex', minHeight: '100vh' }}>
          <aside className="w-[220px] bg-black text-white p-4 flex flex-col gap-4">
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontWeight: 800, fontSize: 16 }}>xCrack</div>
            </div>
            <nav className="flex flex-col gap-3">
              <a href="/" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">대시보드</a>

              {/* 전략/실행 그룹 */}
              <div className="rounded-md" style={{ backgroundColor: '#0b0b0b', border: '1px solid #222' }}>
                <div className="px-3 py-2 text-xs uppercase tracking-wider text-gray-400">전략 / 실행</div>
                <div className="flex flex-col pb-2">
                  <a href="/strategies" className="text-white no-underline px-3 py-2 hover:bg-white/10">전략</a>
                  <a href="/micro" className="text-white no-underline px-3 py-2 hover:bg-white/10">마이크로</a>
                  <a href="/flashloan" className="text-white no-underline px-3 py-2 hover:bg-white/10">플래시론</a>
                  <a href="/multi-asset" className="text-white no-underline px-3 py-2 hover:bg-white/10">다중자산</a>
                  <a href="/bundles" className="text-white no-underline px-3 py-2 hover:bg-white/10">번들</a>
                </div>
              </div>

              {/* 온체인/데이터 그룹 */}
              <div className="rounded-md" style={{ backgroundColor: '#0b0b0b', border: '1px solid #222' }}>
                <div className="px-3 py-2 text-xs uppercase tracking-wider text-gray-400">온체인 / 데이터</div>
                <div className="flex flex-col pb-2">
                  <a href="/mempool" className="text-white no-underline px-3 py-2 hover:bg-white/10">멤풀</a>
                  <a href="/onchain" className="text-white no-underline px-3 py-2 hover:bg-white/10">온체인</a>
                  <a href="/network" className="text-white no-underline px-3 py-2 hover:bg-white/10">네트워크</a>
                </div>
              </div>

              {/* 운영/모니터링 그룹 */}
              <div className="rounded-md" style={{ backgroundColor: '#0b0b0b', border: '1px solid #222' }}>
                <div className="px-3 py-2 text-xs uppercase tracking-wider text-gray-400">운영 / 모니터링</div>
                <div className="flex flex-col pb-2">
                  <a href="/performance" className="text-white no-underline px-3 py-2 hover:bg-white/10">성능</a>
                  <a href="/alerts" className="text-white no-underline px-3 py-2 hover:bg-white/10">알림</a>
                  <a href="/logs" className="text-white no-underline px-3 py-2 hover:bg-white/10">로그</a>
                </div>
              </div>

              {/* 설정 */}
              <div className="rounded-md" style={{ backgroundColor: '#0b0b0b', border: '1px solid #222' }}>
                <div className="px-3 py-2 text-xs uppercase tracking-wider text-gray-400">설정</div>
                <div className="flex flex-col pb-2">
                  <a href="/settings" className="text-white no-underline px-3 py-2 hover:bg-white/10">설정</a>
                  <a href="/system" className="text-white no-underline px-3 py-2 hover:bg-white/10">시스템</a>
                </div>
              </div>
            </nav>
            <div className="mt-auto">
              <ApiHealth />
            </div>
          </aside>
          <main className="flex-1 p-6">
            {children}
          </main>
        </div>
      </body>
    </html>
  );
}
