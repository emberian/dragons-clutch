import type { Metadata } from 'next';

import PopulationWorkspace from '@/components/PopulationWorkspace';

export const metadata: Metadata = {
  title: 'Population · dClutch',
  description:
    'A seeded population of markets driven on a private validator: every market’s odds path, the '
    + 'run’s own event timeline, and what it executed against what it could not.',
};

export default function PopulationPage() {
  return <PopulationWorkspace />;
}
