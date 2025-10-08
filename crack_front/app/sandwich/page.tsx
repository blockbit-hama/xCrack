import { getSandwichDashboard, getSandwichStatus, getSandwichConfig, getSandwichOpportunities } from "../../lib/api";
import { SandwichClient } from "./SandwichClient";

export const dynamic = 'force-dynamic';

export default async function SandwichPage() {
  const [dashboard, status, config, opportunities] = await Promise.all([
    getSandwichDashboard().catch(() => null),
    getSandwichStatus().catch(() => null),
    getSandwichConfig().catch(() => null),
    getSandwichOpportunities().catch(() => ({ opportunities: [], total: 0 })),
  ]);

  return (
    <main>
      <SandwichClient
        initialDashboard={dashboard}
        initialStatus={status}
        initialConfig={config}
        initialOpportunities={opportunities.opportunities || []}
      />
    </main>
  );
}