import type { Metadata } from 'next';

import MarketDetailWorkspace from '@/components/MarketDetailWorkspace';
import { MARKET_REGISTRY_V1, marketEditorialV1 } from '@/lib/marketRegistry';

// The static export (DCLUTCH_PAGES_EXPORT=1, the GitHub Pages artifact) has
// no server to render per-address pages, and an arbitrary Market address is
// chain data the build cannot enumerate — but the shipped market registry is
// a build input, so every market it names gets a real static page with its
// own title and share card. A market the registry does not know still 404s
// as a deep link on the static host and is served at /market?address=…
// instead; the served (worker/local) build renders every address either way.
export function generateStaticParams(): Array<{ address: string }> {
  return Object.keys(MARKET_REGISTRY_V1.markets).map((address) => ({ address }));
}

export async function generateMetadata({ params }: Readonly<{ params: Promise<Readonly<{ address: string }>> }>): Promise<Metadata> {
  const { address } = await params;
  const decoded = decodeURIComponent(address);
  const editorial = marketEditorialV1(decoded);
  // Static metadata is built at export time with no chain read available, so
  // it can only ever carry the editorial half. A row that names a market but
  // not its title has nothing to put in a share card, and an empty card beats
  // a wrong one.
  if (editorial === null || editorial.title === null) return {};
  const title = `${editorial.title} · dClutch`;
  // The question is the description: it is what the market IS, and it is the
  // sentence a pasted link should lead with.
  const description = editorial.question ?? undefined;
  const card = `https://clutch.dregg.pro/og/market-${decoded}.jpg`;
  return {
    title,
    description,
    openGraph: {
      title,
      description,
      siteName: 'dClutch',
      type: 'website',
      images: [{ url: card, width: 1200, height: 630, alt: `${editorial.title} — a dClutch market on Solana devnet.` }],
    },
    twitter: { card: 'summary_large_image', title, description, images: [card] },
  };
}

export default async function MarketDetailPage({ params }: Readonly<{ params: Promise<Readonly<{ address: string }>> }>) {
  const { address } = await params;
  return <MarketDetailWorkspace address={decodeURIComponent(address)} />;
}
