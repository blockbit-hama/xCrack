import './globals.css';
import { Badge } from '../components/ui/badge';
import { ThemeProvider } from '../components/theme-provider';
import { ThemeToggle } from '../components/ui/theme-toggle';
import Link from 'next/link';

export const metadata = {
  title: 'xCrack MEV Dashboard',
  description: 'Advanced MEV Searcher and Arbitrage Bot Dashboard',
  keywords: 'MEV, arbitrage, DeFi, Ethereum, trading bot',
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="ko" suppressHydrationWarning>
      <head>
        <link rel="icon" href="/favicon.ico" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
      </head>
      <body className="min-h-screen bg-background font-sans antialiased">
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          <div className="flex min-h-screen">
            {/* 사이드바 */}
            <aside className="w-64 bg-gray-900 text-white flex flex-col border-r border-gray-800">
              {/* 로고 */}
              <div className="p-6 border-b border-gray-800">
                <div className="flex items-center space-x-3">
                  <div className="w-8 h-8 bg-gradient-to-r from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
                    <span className="text-white font-bold text-sm">XC</span>
                  </div>
                  <div>
                    <h1 className="text-xl font-bold">xCrack</h1>
                    <p className="text-xs text-gray-400">MEV Searcher v2.0</p>
                  </div>
                </div>
              </div>

              {/* 네비게이션 */}
              <nav className="flex-1 p-4 space-y-6">
                {/* 대시보드 */}
                <div>
                  <Link 
                    href="/" 
                    className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors"
                  >
                    <span className="text-lg">📊</span>
                    <span>대시보드</span>
                  </Link>
                </div>

                {/* 핵심 전략 */}
                <div>
                  <h3 className="px-3 text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">
                    핵심 전략
                  </h3>
                  <div className="space-y-1">
                    <Link href="/liquidation" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">💥</span>
                      <span>청산</span>
                      <Badge variant="success" className="text-xs">완성</Badge>
                    </Link>
                    <Link href="/micro-v2" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">🔄</span>
                      <span>차익거래 1</span>
                      <Badge variant="success" className="text-xs">완성</Badge>
                    </Link>
                    <Link href="/complex-arbitrage" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">🌐</span>
                      <span>차익거래 2</span>
                      <Badge variant="success" className="text-xs">완성</Badge>
                    </Link>
                    <Link href="/sandwich" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">🥪</span>
                      <span>샌드위치</span>
                      <Badge variant="warning" className="text-xs">고위험</Badge>
                    </Link>
                  </div>
                </div>

                {/* 모니터링 */}
                <div>
                  <h3 className="px-3 text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">
                    모니터링
                  </h3>
                  <div className="space-y-1">
                    <Link href="/mempool" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">🌊</span>
                      <span>멤풀</span>
                    </Link>
                    <Link href="/bundles" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">📦</span>
                      <span>번들</span>
                    </Link>
                    <Link href="/performance" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">📈</span>
                      <span>성능</span>
                    </Link>
                    <Link href="/alerts" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">🚨</span>
                      <span>알림</span>
                    </Link>
                  </div>
                </div>

                {/* 설정 */}
                <div>
                  <h3 className="px-3 text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">
                    설정
                  </h3>
                  <div className="space-y-1">
                    <Link href="/settings" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">⚙️</span>
                      <span>설정</span>
                    </Link>
                    <Link href="/system" className="flex items-center space-x-3 px-3 py-2 rounded-lg hover:bg-gray-800 transition-colors">
                      <span className="text-lg">🖥️</span>
                      <span>시스템</span>
                    </Link>
                  </div>
                </div>
              </nav>

              {/* API 상태 및 테마 토글 */}
              <div className="p-4 border-t border-gray-800 space-y-3">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs text-gray-400">API 상태</span>
                  <Badge variant="success" className="text-xs">
                    정상
                  </Badge>
                </div>
                <div className="text-xs text-gray-500">
                  마지막 체크: {new Date().toLocaleTimeString()}
                </div>
                <div className="flex justify-center">
                  <ThemeToggle />
                </div>
              </div>
            </aside>

            {/* 메인 콘텐츠 */}
            <main className="flex-1 bg-gray-50 dark:bg-gray-900">
              <div className="p-6">
                {children}
              </div>
            </main>
          </div>
        </ThemeProvider>
      </body>
    </html>
  );
}
