import type { Metadata } from 'next';

import LaunchStory from '@/components/LaunchStory';

export const metadata: Metadata = {
  title: "Dragon's Clutch · Live on Solana devnet",
  description: 'Follow a fully collateralized dClutch market from founding through trade, resolution, and redemption on public Solana devnet.',
};

export default function LivePage() {
  return <LaunchStory />;
}
