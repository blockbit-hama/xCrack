'use client';

import { useState } from 'react';

export default function PerformancePage() {
  return (
    <main className="p-6 space-y-6">
      <div className="border-b pb-4">
        <h1 className="text-3xl font-bold">성능 분석</h1>
        <p className="text-gray-600 mt-1">시스템 성능 모니터링</p>
      </div>
      
      <div className="bg-white dark:bg-gray-800 p-6 rounded-lg shadow">
        <h2 className="text-xl font-semibold mb-4">성능 데이터</h2>
        <p className="text-gray-500">성능 분석 데이터 준비 중입니다</p>
      </div>
    </main>
  );
}
