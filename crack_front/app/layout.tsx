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
            <nav className="flex flex-col gap-2">
              <a href="/" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">대시보드</a>
              <a href="/strategies" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">전략</a>
              <a href="/bundles" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">번들</a>
              <a href="/mempool" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">멤풀</a>
              <a href="/performance" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">성능</a>
              <a href="/alerts" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">알림</a>
              <a href="/logs" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">로그</a>
              <a href="/settings" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">설정</a>
              <a href="/system" className="text-white no-underline px-3 py-2 rounded-md hover:bg-white/10">시스템</a>
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
