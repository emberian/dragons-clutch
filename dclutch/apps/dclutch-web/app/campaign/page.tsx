import type { Metadata } from 'next';

import CampaignWorkspace from '@/components/CampaignWorkspace';

export const metadata: Metadata = {
  title: 'dClutch · A campaign — one market’s whole life on a private chain',
  description:
    'A dClutch market founded, resolved and retired on a local rehearsal validator, drawn from the campaign’s own transcript: per-outcome odds, the vault, the work each stage cost, and the terminal answer. Not devnet, not mainnet, and no fills.',
};

export default function CampaignPage() {
  return <CampaignWorkspace />;
}
