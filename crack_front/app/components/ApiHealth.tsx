"use client";

import { useEffect, useState } from "react";

const BACKEND = process.env.NEXT_PUBLIC_BACKEND_URL || "http://localhost:8080";

export default function ApiHealth() {
  const [ok, setOk] = useState<boolean | null>(null);

  useEffect(() => {
    let alive = true;
    const tick = async () => {
      try {
        const res = await fetch(`${BACKEND}/api/health`, { cache: 'no-cache' });
        if (!alive) return;
        setOk(res.ok);
      } catch {
        if (!alive) return;
        setOk(false);
      }
    };
    tick();
    const id = setInterval(tick, 5000);
    return () => { alive = false; clearInterval(id); };
  }, []);

  const color = ok === null ? '#d1d5db' : ok ? '#10b981' : '#ef4444';
  const label = ok === null ? '확인중' : ok ? 'API 정상' : 'API 오류';

  return (
    <div title={label} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
      <span style={{ width: 10, height: 10, borderRadius: 9999, background: color, display: 'inline-block' }} />
      <span style={{ fontSize: 12, color: '#6b7280' }}>{label}</span>
    </div>
  );
}
