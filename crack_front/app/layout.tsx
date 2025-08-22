export const metadata = {
  title: 'xCrack Dashboard',
  description: 'xCrack MEV Searcher UI',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="ko">
      <body style={{ fontFamily: 'system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial' }}>
        <div style={{ maxWidth: 1080, margin: '0 auto', padding: 24 }}>
          <header style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 24 }}>
            <h1 style={{ fontSize: 20, fontWeight: 700 }}>xCrack Dashboard</h1>
            <nav style={{ display: 'flex', gap: 12 }}>
              <a href="/" style={{ textDecoration: 'none' }}>대시보드</a>
              <a href="/strategies" style={{ textDecoration: 'none' }}>전략</a>
              <a href="/bundles" style={{ textDecoration: 'none' }}>번들</a>
              <a href="/logs" style={{ textDecoration: 'none' }}>로그</a>
              <a href="/settings" style={{ textDecoration: 'none' }}>설정</a>
            </nav>
          </header>
          {children}
        </div>
      </body>
    </html>
  );
}
