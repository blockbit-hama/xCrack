import Link from 'next/link';
import { getBundlesSummary, getBundlesRecent } from '../../lib/api';
import { Card, CardContent, CardHeader, CardTitle } from "../../components/ui/card";
import { Badge } from "../../components/ui/badge";
import { Progress } from "../../components/ui/progress";
import { 
  Package, 
  CheckCircle, 
  Clock, 
  XCircle, 
  TrendingUp, 
  Zap,
  ExternalLink
} from 'lucide-react';

export const dynamic = 'force-dynamic';

export default async function BundlesPage() {
  const summary = await getBundlesSummary();
  const recent = await getBundlesRecent(20);

  // recent가 배열인지 확인하고 안전하게 처리
  const recentBundles = Array.isArray(recent) ? recent : [];

  const successRate = summary?.stats?.total_submitted > 0 
    ? (summary.stats.total_included / summary.stats.total_submitted) * 100 
    : 0;

  const getStateIcon = (state: string) => {
    switch (state) {
      case 'submitted':
        return <Clock className="h-4 w-4 text-yellow-500" />;
      case 'included':
        return <CheckCircle className="h-4 w-4 text-green-500" />;
      case 'failed':
        return <XCircle className="h-4 w-4 text-red-500" />;
      default:
        return <Package className="h-4 w-4 text-gray-500" />;
    }
  };

  const getStateBadge = (state: string) => {
    switch (state) {
      case 'submitted':
        return <Badge className="bg-yellow-100 text-yellow-800">제출됨</Badge>;
      case 'included':
        return <Badge className="bg-green-100 text-green-800">포함됨</Badge>;
      case 'failed':
        return <Badge className="bg-red-100 text-red-800">실패</Badge>;
      default:
        return <Badge className="bg-gray-100 text-gray-800">알 수 없음</Badge>;
    }
  };

  return (
    <main className="p-6 space-y-6">
      {/* 헤더 */}
      <div className="border-b pb-4">
        <h1 className="text-3xl font-bold">번들 관리</h1>
        <p className="text-gray-600 mt-1">MEV 번들 실행 및 모니터링</p>
      </div>

      {/* 통계 카드 */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">총 제출</CardTitle>
            <Package className="h-4 w-4 text-blue-500" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{summary?.stats?.total_submitted || 0}</div>
            <p className="text-xs text-muted-foreground mt-1">
              생성된 번들 수
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">포함됨</CardTitle>
            <CheckCircle className="h-4 w-4 text-green-500" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{summary?.stats?.total_included || 0}</div>
            <p className="text-xs text-muted-foreground mt-1">
              블록에 포함된 번들
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">성공률</CardTitle>
            <TrendingUp className="h-4 w-4 text-purple-500" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{successRate.toFixed(1)}%</div>
            <Progress value={successRate} className="mt-2" />
            <p className="text-xs text-muted-foreground mt-1">
              번들 포함 성공률
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">총 수익</CardTitle>
            <Zap className="h-4 w-4 text-orange-500" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-green-600">
              {(summary?.stats?.total_profit || 0).toFixed(4)} ETH
            </div>
            <p className="text-xs text-muted-foreground mt-1">
              누적 수익
            </p>
          </CardContent>
        </Card>
      </div>

      {/* 최근 번들 목록 */}
      <Card>
        <CardHeader>
          <CardTitle>최근 번들</CardTitle>
        </CardHeader>
        <CardContent>
          {recentBundles.length > 0 ? (
            <div className="space-y-4">
              {recentBundles.map((bundle) => (
                <div key={bundle.id || Math.random()} className="border rounded-lg p-4 hover:bg-gray-50 transition-colors">
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center space-x-3">
                      {getStateIcon(bundle.state)}
                      <div>
                        <h3 className="font-semibold">
                          {bundle.id ? (
                            <Link 
                              href={`/bundles/${bundle.id}`} 
                              className="text-blue-600 hover:text-blue-800 flex items-center space-x-1"
                            >
                              <span>{bundle.id.slice(0, 10)}...</span>
                              <ExternalLink className="h-3 w-3" />
                            </Link>
                          ) : (
                            'Unknown ID'
                          )}
                        </h3>
                        <p className="text-sm text-gray-500">
                          {bundle.strategy || 'Unknown Strategy'}
                        </p>
                      </div>
                    </div>
                    <div className="flex items-center space-x-3">
                      {getStateBadge(bundle.state)}
                      <div className="text-right">
                        <div className="text-sm font-medium">
                          {bundle.expected_profit || '0'} ETH
                        </div>
                        <div className="text-xs text-gray-500">
                          예상 수익
                        </div>
                      </div>
                    </div>
                  </div>
                  
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
                    <div>
                      <span className="text-gray-500">가스 추정:</span>
                      <span className="ml-2 font-medium">{bundle.gas_estimate || '0'}</span>
                    </div>
                    <div>
                      <span className="text-gray-500">생성 시간:</span>
                      <span className="ml-2 font-medium">
                        {bundle.timestamp ? new Date(bundle.timestamp).toLocaleString('ko-KR') : 'Unknown'}
                      </span>
                    </div>
                    <div>
                      <span className="text-gray-500">상태:</span>
                      <span className="ml-2 font-medium capitalize">{bundle.state || 'Unknown'}</span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-center py-12 text-gray-500">
              <Package className="h-12 w-12 mx-auto mb-4 text-gray-300" />
              <p className="text-lg font-medium">최근 번들이 없습니다</p>
              <p className="text-sm mt-2">MEV 기회가 발견되면 번들이 생성됩니다</p>
            </div>
          )}
        </CardContent>
      </Card>
    </main>
  );
}
