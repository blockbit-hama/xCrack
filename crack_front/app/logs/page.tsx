"use client";

import { useEffect, useMemo, useRef, useState } from "react";

type AlertLevel = "Info" | "Warning" | "Error" | "Critical";
type Alert = {
  id: string;
  level: AlertLevel;
  message: string;
  timestamp: number;
  acknowledged: boolean;
};

const BACKEND = process.env.NEXT_PUBLIC_BACKEND_URL || "http://localhost:8080";

export default function LogsPage() {
  const [connected, setConnected] = useState(false);
  const [paused, setPaused] = useState(false);
  const [level, setLevel] = useState<"ALL" | AlertLevel>("ALL");
  const [q, setQ] = useState("");
  const [alerts, setAlerts] = useState<Alert[]>([]);
  const esRef = useRef<EventSource | null>(null);
  const [msg, setMsg] = useState("");

  useEffect(() => {
    if (paused) return;
    const es = new EventSource(`${BACKEND}/api/stream/logs`);
    esRef.current = es;
    es.onopen = () => setConnected(true);
    es.onerror = () => setConnected(false);
    es.addEventListener("alerts", (ev: MessageEvent) => {
      try {
        const json: Alert[] = JSON.parse(ev.data || "[]");
        if (Array.isArray(json) && json.length) {
          setAlerts((prev) => {
            const merged = [...json, ...prev];
            return merged.slice(0, 200); // keep last 200
          });
        }
      } catch (_) {
        // ignore parse errors
      }
    });
    return () => {
      es.close();
      esRef.current = null;
      setConnected(false);
    };
  }, [paused]);

  const filtered = useMemo(() => {
    return alerts.filter((a) => {
      const okLevel = level === "ALL" ? true : a.level === level;
      const okText = q ? (a.message || "").toLowerCase().includes(q.toLowerCase()) : true;
      return okLevel && okText;
    });
  }, [alerts, level, q]);

  return (
    <main>
      <h2 className="text-xl font-semibold mb-3">실시간 로그</h2>
      <div className="flex flex-wrap gap-3 mb-3 items-center">
        <span>상태: {connected ? '연결됨' : '연결 끊김'} {paused && '(일시정지)'}</span>
        <button onClick={() => setPaused((p) => !p)} className="px-3 py-2 rounded-md border border-gray-300 cursor-pointer">
          {paused ? '재개' : '일시정지'}
        </button>
        <select value={level} onChange={(e) => setLevel(e.target.value as any)} className="px-3 py-2 rounded-md border border-gray-300">
          <option value="ALL">ALL</option>
          <option value="Info">Info</option>
          <option value="Warning">Warning</option>
          <option value="Error">Error</option>
          <option value="Critical">Critical</option>
        </select>
        <input
          placeholder="메시지 검색…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          className="flex-1 min-w-[200px] px-3 py-2 rounded-md border border-gray-300"
        />
        <button onClick={() => setAlerts([])} className="px-3 py-2 rounded-md border border-gray-300 cursor-pointer">지우기</button>
        <button
          onClick={async () => {
            setMsg("");
            try {
              const res = await fetch(`${BACKEND}/api/settings`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'ack_all_alerts' }) });
              const j = await res.json();
              if (!j.ok) throw new Error(j.error || 'failed');
              setMsg('알림 전체 확인 완료');
            } catch (e: any) { setMsg(`실패: ${e.message || e}`); }
          }}
          className="px-3 py-2 rounded-md border border-gray-300 cursor-pointer"
        >알림 전체 확인</button>
        <button
          onClick={async () => {
            setMsg("");
            try {
              const res = await fetch(`${BACKEND}/api/settings`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: 'reset_stats' }) });
              const j = await res.json();
              if (!j.ok) throw new Error(j.error || 'failed');
              setMsg('통계 초기화 완료');
            } catch (e: any) { setMsg(`실패: ${e.message || e}`); }
          }}
          className="px-3 py-2 rounded-md border border-gray-300 cursor-pointer"
        >통계 초기화</button>
      </div>
      {msg && <div className="mb-2 text-blue-600">{msg}</div>}

      <div className="border rounded-lg overflow-hidden">
        <table className="w-full border-collapse">
          <thead>
            <tr className="text-left">
              <th className="p-2">시간</th>
              <th className="p-2">레벨</th>
              <th className="p-2">메시지</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((a) => (
              <tr key={a.id} className="border-t">
                <td className="p-2 whitespace-nowrap">{new Date(a.timestamp * 1000).toLocaleString()}</td>
                <td className="p-2">{a.level}</td>
                <td className="p-2">{a.message}</td>
              </tr>
            ))}
            {filtered.length === 0 && (
              <tr>
                <td colSpan={3} className="p-3 text-gray-500">표시할 로그가 없습니다</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </main>
  );
}
