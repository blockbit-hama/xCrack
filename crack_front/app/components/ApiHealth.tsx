'use client';

import { useState, useEffect } from 'react';
import { Badge } from '../../components/ui/badge';
import { CheckCircle, XCircle, Clock, AlertTriangle } from 'lucide-react';

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080';

export default function ApiHealth() {
  const [isHealthy, setIsHealthy] = useState<boolean | null>(null);
  const [lastCheck, setLastCheck] = useState<Date | null>(null);
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [responseTime, setResponseTime] = useState<number | null>(null);

  useEffect(() => {
    const checkHealth = async () => {
      const startTime = Date.now();
      try {
        const response = await fetch(`${API_BASE_URL}/api/status`, { 
          method: 'GET',
          headers: { 'Content-Type': 'application/json' },
          cache: 'no-cache'
        });
        
        const responseTime = Date.now() - startTime;
        setResponseTime(responseTime);
        
        if (response.ok) {
          setIsHealthy(true);
          setErrorMessage('');
        } else {
          setIsHealthy(false);
          setErrorMessage(`HTTP ${response.status}: ${response.statusText}`);
        }
        setLastCheck(new Date());
      } catch (error: any) {
        const responseTime = Date.now() - startTime;
        setResponseTime(responseTime);
        setIsHealthy(false);
        setErrorMessage(error.message || '연결 실패');
        setLastCheck(new Date());
      }
    };

    // 초기 체크
    checkHealth();

    // 30초마다 체크
    const interval = setInterval(checkHealth, 30000);

    return () => clearInterval(interval);
  }, []);

  const getStatusIcon = () => {
    if (isHealthy === null) return <Clock className="h-3 w-3" />;
    if (isHealthy) return <CheckCircle className="h-3 w-3" />;
    return <XCircle className="h-3 w-3" />;
  };

  const getStatusColor = () => {
    if (isHealthy === null) return 'text-yellow-500';
    if (isHealthy) return 'text-green-500';
    return 'text-red-500';
  };

  const getBadgeVariant = () => {
    if (isHealthy === null) return 'secondary';
    if (isHealthy) return 'success';
    return 'destructive';
  };

  return (
    <div className="p-3 border-t border-gray-700">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center space-x-2">
          <span className="text-xs text-gray-400">API 상태</span>
          <div className={`${getStatusColor()}`}>
            {getStatusIcon()}
          </div>
        </div>
        <Badge 
          variant={getBadgeVariant()}
          className="text-xs"
        >
          {isHealthy === null ? '체크 중' : isHealthy ? '정상' : '오류'}
        </Badge>
      </div>
      
      {responseTime !== null && (
        <div className="text-xs text-gray-500 mb-1">
          응답 시간: {responseTime}ms
        </div>
      )}
      
      {lastCheck && (
        <div className="text-xs text-gray-500 mb-1">
          마지막 체크: {lastCheck.toLocaleTimeString()}
        </div>
      )}
      
      {!isHealthy && errorMessage && (
        <div className="text-xs text-red-400 flex items-center space-x-1">
          <AlertTriangle className="h-3 w-3" />
          <span>{errorMessage}</span>
        </div>
      )}
    </div>
  );
}
