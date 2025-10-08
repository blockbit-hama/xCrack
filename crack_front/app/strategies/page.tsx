"use client";

import { useEffect, useState } from "react";
import type { Strategies, StrategyKey } from "../../lib/api";
import { getStrategies, toggleStrategy, getStrategyStats } from "../../lib/api";

export default function StrategiesPage() {
  const [strategies, setStrategies] = useState<Strategies>({
    sandwich: false,
    liquidation: false,
    micro: false,
    cross: false,
  });
  const [loading, setLoading] = useState(true);
  const [stats, setStats] = useState<{ total_strategies: number; active_strategies: number; total_profit: string; success_rate: number } | null>(null);
  const [saving, setSaving] = useState<StrategyKey | null>(null);

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        const [s, st] = await Promise.all([
          getStrategies(),
          getStrategyStats().catch(() => null),
        ]);
        setStrategies(s); setStats(st);
      } finally {
        setLoading(false);
      }
    })();
    const id = setInterval(async () => {
      const [s, st] = await Promise.all([
        getStrategies().catch(() => strategies),
        getStrategyStats().catch(() => null),
      ]);
      setStrategies(s); setStats(st);
    }, 10000);
    return () => clearInterval(id);
  // eslint-disable-next-line react-hooks/exhaustive-deps
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

  const items: { key: StrategyKey; name: string; desc: string; href: string }[] = [
    { key: "sandwich", name: "Sandwich", desc: "프론트/백런 번들 기반", href: "/strategies/sandwich" },
    { key: "liquidation", name: "Liquidation", desc: "Aave/Compound/Maker 청산", href: "/strategies/liquidation" },
    { key: "micro", name: "Micro Arbitrage", desc: "CEX/DEX 미세차익", href: "/strategies/micro" },
  ];

  return (
    <main>
      <h2 className="text-xl font-semibold mb-3">전략 보드</h2>
      {loading ? (
        <div>로딩 중…</div>
      ) : (
        <ul className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {items.map((it) => (
            <li key={it.key} className="border rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div>
                  <a href={it.href} className="font-semibold no-underline hover:underline">{it.name}</a>
                  <div className="text-xs text-gray-500">{it.desc}</div>
                  <div className="mt-1.5 text-xs text-gray-600">
                    {stats && typeof stats.success_rate === 'number' ? (
                      <span>
                        수익률 {stats.success_rate.toFixed(1)}% · 총이익 {stats.total_profit || '0'}
                      </span>
                    ) : (
                      <span>지표 없음</span>
                    )}
                  </div>
                </div>
                <button
                  disabled={saving === it.key}
                  onClick={() => onToggle(it.key)}
                  className={`px-3 py-2 rounded-md min-w-20 ${strategies[it.key] ? 'bg-emerald-500 text-white' : 'bg-gray-200 text-gray-900'} ${saving === it.key ? 'opacity-60 cursor-not-allowed' : 'cursor-pointer'}`}
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
