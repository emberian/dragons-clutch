import type { Metadata } from 'next';

import BuildingPulse from '@/components/BuildingPulse';

import './building.css';

export const metadata: Metadata = {
  title: 'dClutch · What is being built right now',
  description:
    'The live edge of dClutch development on Solana devnet: what is in flight at this moment, the firsts of the last thirty hours, and the wall ledger — written by hand, dated honestly.',
};

export default function BuildingPage() {
  return <BuildingPulse />;
}
