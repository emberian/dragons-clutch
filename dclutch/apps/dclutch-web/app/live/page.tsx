import type { Metadata } from 'next';

import LaunchStory from '@/components/LaunchStory';

export const metadata: Metadata = {
  title: "Dragon's Clutch · Solana devnet preview",
  description: 'The seven dClutch programs are deployed on Solana devnet. See what founding, joining and trading a fully collateralized market look like. Resolving and redeeming are not open yet.',
};

export default function LivePage() {
  return <LaunchStory />;
}
