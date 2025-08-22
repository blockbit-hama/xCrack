"use client";

import { useEffect, useState } from "react";
import type { Strategies, StrategyKey } from "@/lib/api";
import { getStrategies, toggleStrategy } from "@/lib/api";

export default function StrategiesPage() {
  const [strategies, setStrategies] = useState<Strategies>({
    sandwich: false,
    liquidation: false,
    micro: false,
    cross: false,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState<StrategyKey | null>(null);

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        const s = await getStrategies();
        setStrategies(s);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const onToggle = async (key: StrategyKey) => {
    const next = !strategies[key];
    setSaving(key);
    try {
      const ok = await toggleStrategy(key, next);
      if (ok) setStrategies({ ...strategies, [key]: next });
      else alert("토글 실패");
    } finally {
      setSaving(null);
    }
  };

  const items: { key: StrategyKey; name: string; desc: string }[] = [
    { key: "sandwich", name: "Sandwich", desc: "프론트/백런 번들 기반" },
    { key: "liquidation", name: "Liquidation", desc: "Aave/Compound/Maker 청산" },
    { key: "micro", name: "Micro Arbitrage", desc: "CEX/DEX 미세차익" },
    { key: "cross", name: "Cross-Chain", desc: "브리지 기반 크로스체인" },
  ];

  return (
    <main>
      <h2 style={{ marginBottom: 12 }}>전략 보드</h2>
      {loading ? (
        <div>로딩 중…</div>
      ) : (
        <ul style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: 12 }}>
          {items.map((it) => (
            <li key={it.key} style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <div>
                  <div style={{ fontWeight: 700 }}>{it.name}</div>
                  <div style={{ fontSize: 12, color: '#888' }}>{it.desc}</div>
                </div>
                <button
                  disabled={saving === it.key}
                  onClick={() => onToggle(it.key)}
                  style={{
                    background: strategies[it.key] ? '#10b981' : '#e5e7eb',
                    color: strategies[it.key] ? 'white' : '#111827',
                    border: 'none',
                    borderRadius: 6,
                    padding: '8px 12px',
                    minWidth: 80,
                    cursor: 'pointer',
                  }}
                >
                  {saving === it.key ? '저장…' : strategies[it.key] ? 'ON' : 'OFF'}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}
