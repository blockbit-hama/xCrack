"use client";

import { useEffect, useState } from "react";

type StrategyState = Record<string, boolean>;

type SettingsResp = {
  strategies: StrategyState;
  api_port: number;
  metrics_port: number;
};

const BACKEND = process.env.NEXT_PUBLIC_BACKEND_URL || "http://localhost:8080";

export default function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [data, setData] = useState<SettingsResp | null>(null);
  const [msg, setMsg] = useState("");

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        const res = await fetch(`${BACKEND}/api/settings`, { cache: 'no-cache' });
        const json = await res.json();
        setData(json);
      } catch (e) {
        setMsg("설정 로드 실패");
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const callAction = async (action: string) => {
    setMsg("");
    try {
      const res = await fetch(`${BACKEND}/api/settings`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      const json = await res.json();
      if (!json.ok) throw new Error(json.error || 'failed');
      setMsg(`성공: ${action}`);
    } catch (e: any) {
      setMsg(`실패: ${action} (${e.message || e})`);
    }
  };

  return (
    <main>
      <h2 style={{ marginBottom: 12 }}>설정</h2>
      {loading ? (
        <div>로딩 중…</div>
      ) : !data ? (
        <div>데이터 없음</div>
      ) : (
        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <h3>포트</h3>
            <div>API 포트: {data.api_port}</div>
            <div>메트릭 포트: {data.metrics_port}</div>
          </div>

          <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <h3>전략 상태(읽기 전용)</h3>
            <ul>
              {Object.entries(data.strategies).map(([k, v]) => (
                <li key={k}>{k}: {v ? 'ON' : 'OFF'}</li>
              ))}
            </ul>
          </div>

          <div style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <h3>액션</h3>
            <div style={{ display: 'flex', gap: 12 }}>
              <button onClick={() => callAction('reset_stats')} style={{ padding: '6px 10px', borderRadius: 6, border: '1px solid #ddd', cursor: 'pointer' }}>통계 초기화</button>
              <button onClick={() => callAction('ack_all_alerts')} style={{ padding: '6px 10px', borderRadius: 6, border: '1px solid #ddd', cursor: 'pointer' }}>알림 전체 확인</button>
            </div>
          </div>

          {msg && <div style={{ color: '#2563eb' }}>{msg}</div>}
        </div>
      )}
    </main>
  );
}
