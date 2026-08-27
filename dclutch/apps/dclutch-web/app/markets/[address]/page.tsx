import MarketDetailWorkspace from '@/components/MarketDetailWorkspace';

// Static export (DCLUTCH_PAGES_EXPORT=1, the GitHub Pages artifact) has no
// server to render per-address pages, and a Market address is chain data the
// build cannot enumerate — so the export carries no /markets/<address> pages
// and a deep link into one 404s on a static host. This is a static-hosting
// limit, not an app limit: the served (worker/local) build renders every
// address, and the market list still works in the export. An empty param set
// keeps the served build's behavior unchanged.
export function generateStaticParams(): Array<{ address: string }> {
  return [];
}

export default async function MarketDetailPage({ params }: Readonly<{ params: Promise<Readonly<{ address: string }>> }>) {
  const { address } = await params;
  return <MarketDetailWorkspace address={decodeURIComponent(address)} />;
}
