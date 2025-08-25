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
          <aside style={{ width: 220, background: '#0b0b0c', color: '#fff', padding: 16, display: 'flex', flexDirection: 'column', gap: 16 }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div style={{ fontWeight: 800, fontSize: 16 }}>xCrack</div>
            </div>
            <nav style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <a href="/" style={{ color: '#fff', textDecoration: 'none', padding: '8px 10px', borderRadius: 8, background: 'transparent' }}>대시보드</a>
              <a href="/strategies" style={{ color: '#fff', textDecoration: 'none', padding: '8px 10px', borderRadius: 8, background: 'transparent' }}>전략</a>
              <a href="/bundles" style={{ color: '#fff', textDecoration: 'none', padding: '8px 10px', borderRadius: 8, background: 'transparent' }}>번들</a>
              <a href="/logs" style={{ color: '#fff', textDecoration: 'none', padding: '8px 10px', borderRadius: 8, background: 'transparent' }}>로그</a>
              <a href="/settings" style={{ color: '#fff', textDecoration: 'none', padding: '8px 10px', borderRadius: 8, background: 'transparent' }}>설정</a>
            </nav>
            <div style={{ marginTop: 'auto' }}>
              <ApiHealth />
            </div>
          </aside>
          <main style={{ flex: 1, padding: 24 }}>
            {children}
          </main>
        </div>
      </body>
    </html>
  );
}
