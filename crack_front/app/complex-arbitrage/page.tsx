import { getComplexArbitrageDashboard, getComplexArbitrageStatus, getComplexArbitrageConfig, getComplexArbitrageOpportunities } from "../../lib/api";
import { ComplexArbitrageClient } from "./ComplexArbitrageClient";

export const dynamic = 'force-dynamic';

export default async function ComplexArbitragePage() {
  const [dashboard, status, config, opportunities] = await Promise.all([
    getComplexArbitrageDashboard().catch(() => null),
    getComplexArbitrageStatus().catch(() => null),
    getComplexArbitrageConfig().catch(() => null),
    getComplexArbitrageOpportunities().catch(() => ({ opportunities: [], total: 0 })),
  ]);

  return (
    <main>
      <ComplexArbitrageClient
        initialDashboard={dashboard}
        initialStatus={status}
        initialConfig={config}
        initialOpportunities={opportunities.opportunities || []}
      />
    </main>
  );
}