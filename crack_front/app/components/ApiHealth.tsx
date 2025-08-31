'use client';

import { useState, useEffect } from 'react';
import { Badge } from '../../components/ui/badge';

export default function ApiHealth() {
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [lastCheck, setLastCheck] = useState<Date | null>(null);

  useEffect(() => {
    const checkHealth = async () => {
      try {
        const response = await fetch('/api/health', { 
          method: 'GET',
          headers: { 'Content-Type': 'application/json' }
        });
        setIsHealthy(response.ok);
        setLastCheck(new Date());
      } catch (error) {
        setIsHealthy(false);
        setLastCheck(new Date());
      }
    };

    // 초기 체크
    checkHealth();

    // 30초마다 체크
    const interval = setInterval(checkHealth, 30000);

    return () => clearInterval(interval);
  }, []);

  return (
    <div className="p-3 border-t border-gray-700">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-gray-400">API 상태</span>
        <Badge 
          variant={isHealthy === null ? 'secondary' : isHealthy ? 'success' : 'destructive'}
          className="text-xs"
        >
          {isHealthy === null ? '체크 중' : isHealthy ? '정상' : '오류'}
        </Badge>
      </div>
      {lastCheck && (
        <div className="text-xs text-gray-500">
          마지막 체크: {lastCheck.toLocaleTimeString()}
        </div>
      )}
    </div>
  );
}
