export default function StrategiesPage() {
  const items = [
    { name: 'Sandwich', status: '활성', desc: '프론트/백런 번들 기반' },
    { name: 'Liquidation', status: '활성', desc: 'Aave/Compound/Maker 청산' },
    { name: 'Micro Arbitrage', status: '옵션', desc: 'CEX/DEX 미세차익' },
    { name: 'Cross-Chain', status: '옵션', desc: '브리지 기반 크로스체인' },
  ];
  return (
    <main>
      <h2 style={{ marginBottom: 12 }}>전략 보드</h2>
      <ul style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0, 1fr))', gap: 12 }}>
        {items.map((it) => (
          <li key={it.name} style={{ border: '1px solid #eee', borderRadius: 8, padding: 16 }}>
            <div style={{ fontWeight: 700 }}>{it.name}</div>
            <div style={{ fontSize: 12, color: '#888' }}>{it.status}</div>
            <div style={{ marginTop: 8 }}>{it.desc}</div>
          </li>
        ))}
      </ul>
    </main>
  );
}
