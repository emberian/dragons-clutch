import MarketDetailWorkspace from '@/components/MarketDetailWorkspace';

export default async function MarketDetailPage({ params }: Readonly<{ params: Promise<Readonly<{ address: string }>> }>) {
  const { address } = await params;
  return <MarketDetailWorkspace address={decodeURIComponent(address)} />;
}
