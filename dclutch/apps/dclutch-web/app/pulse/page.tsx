import type { Metadata } from 'next';

import PulseWorkspace from '@/components/PulseWorkspace';

export const metadata: Metadata = {
  title: 'dClutch · Pulse — is anybody home?',
  description:
    'Whether anything is running against the dClutch devnet deployment right now, read live from the chain and from the simulator’s own status file — never remembered, never assumed.',
};

export default function PulsePage() {
  return <PulseWorkspace />;
}
