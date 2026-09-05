'use client';

import PageShell from '@/components/PageShell';
import { useSyncExternalStore } from 'react';

import Anchor from '@/components/Anchor';
import MarketDetailWorkspace from '@/components/MarketDetailWorkspace';
import Nav from '@/components/Nav';
import { marketAddressQueryV1 } from '@dclutch/sdk/marketHref';

function subscribeToLocation(onChange: () => void): () => void {
  window.addEventListener('popstate', onChange);
  return () => window.removeEventListener('popstate', onChange);
}

function readSearch(): string {
  return window.location.search;
}

function readServerSearch(): null {
  return null;
}

/** Resolve a Market address from a query without asking a static host for a dynamic file. */
export default function MarketAddressWorkspace() {
  const search = useSyncExternalStore<string | null>(subscribeToLocation, readSearch, readServerSearch);
  const query = marketAddressQueryV1(search);
  if (query.kind === 'ready') return <MarketDetailWorkspace address={query.address} />;

  const resolving = query.kind === 'resolving';
  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/markets" />}>
    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">{resolving ? 'Reading this link' : 'Market link refused'}</p>
        <h1>{resolving ? <>One moment<br /><em>finding the Market.</em></> : <>This link does not name<br /><em>one Market.</em></>}</h1>
        <p>{resolving
          ? 'The page is reading the Market address from this link. Its on-chain detail loads after the address is known.'
          : query.reason}</p>
      </div>
      <aside>
        <span>What you can do</span>
        <strong>{resolving ? 'Wait for the address' : 'Choose a listed Market'}</strong>
        <p>No wallet is connected and no transaction is prepared from this page.</p>
      </aside>
    </section>
    {!resolving && <section className="trade-v3-card">
      <header><span>01</span><div><h2>Open the finalized Market list</h2><p>Each listed Market carries a complete link to this page.</p></div></header>
      <div className="direct-actions"><Anchor className="secondary-action" href="/markets">Discover Markets →</Anchor></div>
    </section>}
  </PageShell>;
}
