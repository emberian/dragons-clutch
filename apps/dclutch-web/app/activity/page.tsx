import type { Metadata } from 'next';

import ActivityWorkspace from '@/components/ActivityWorkspace';

export const metadata: Metadata = {
  title: 'dClutch · Activity — what a wallet did',
  description:
    'Finalized transactions for a wallet on the dClutch devnet deployment, newest first, exactly as the node remembers them.',
};

export default function ActivityPage() {
  return <ActivityWorkspace />;
}
