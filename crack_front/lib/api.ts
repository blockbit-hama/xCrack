type Status = {
  is_running: boolean;
  active_opportunities: number;
  submitted_bundles: number;
  total_profit_eth: string;
  success_rate: number;
  uptime_seconds: number;
};

const BASE = process.env.NEXT_PUBLIC_BACKEND_URL || 'http://localhost:8080';

export async function getStatus(): Promise<Status> {
  const res = await fetch(`${BASE}/api/status`, { cache: 'no-cache' });
  if (!res.ok) {
    // fallback to metrics server status
    const res2 = await fetch(`${BASE.replace(':8080', ':9090')}/status`, { cache: 'no-cache' });
    if (!res2.ok) throw new Error('status fetch failed');
    return res2.json();
  }
  return res.json();
}

export function defaultStatus(): Status {
  return {
    is_running: false,
    active_opportunities: 0,
    submitted_bundles: 0,
    total_profit_eth: '0.0',
    success_rate: 0,
    uptime_seconds: 0,
  };
}
