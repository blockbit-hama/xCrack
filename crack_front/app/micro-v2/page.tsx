import { getCexDexArbitrageDashboard, getCexDexArbitrageStatus, getCexDexArbitrageConfig, getCexDexArbitrageOpportunities } from "../../lib/api";
import { CexDexArbitrageClient } from "./CexDexArbitrageClient";

export const dynamic = 'force-dynamic';

export default async function CexDexArbitragePage() {
  const [dashboard, status, config, opportunities] = await Promise.all([
    getCexDexArbitrageDashboard().catch(() => null),
    getCexDexArbitrageStatus().catch(() => null),
    getCexDexArbitrageConfig().catch(() => null),
    getCexDexArbitrageOpportunities().catch(() => ({ opportunities: [], total: 0 })),
  ]);

  return (
    <main>
      <CexDexArbitrageClient
        initialDashboard={dashboard}
        initialStatus={status}
        initialConfig={config}
        initialOpportunities={opportunities.opportunities || []}
      />
    </main>
  );
}