import { getLiquidationDashboard, getProtocolStatus, getLiquidationOpportunities } from "../../lib/api";
import { LiquidationClient } from "./LiquidationClient";

export const dynamic = 'force-dynamic';

export default async function LiquidationPage() {
  const [dashboard, protocolStatus, opportunities] = await Promise.all([
    getLiquidationDashboard().catch(() => null),
    getProtocolStatus().catch(() => []),
    getLiquidationOpportunities().catch(() => ({ opportunities: [], total: 0 })),
  ]);

  return (
    <main>
      <LiquidationClient
        initialDashboard={dashboard}
        initialProtocolStatus={protocolStatus}
        initialOpportunities={opportunities.opportunities || []}
      />
    </main>
  );
}