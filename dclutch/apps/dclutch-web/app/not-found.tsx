'use client';

import PageShell from '@/components/PageShell';
import { useSyncExternalStore } from 'react';

import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import MarketDetailWorkspace from '@/components/MarketDetailWorkspace';
import { resolveExportedPathnameV1, type ExportedRouteV1 } from '@/lib/exportRouting';

// -------------------------------------------------- the location, as state
//
// Same primitive, and for the same reason, as the explorer's reading of
// `location.search` (components/ChainExplorer.tsx): the location is an
// external system, and reading it through `useSyncExternalStore` gives the
// prerendered HTML and the first client render one agreed value without
// hydrating state inside an effect.

function subscribeToLocation(onChange: () => void): () => void {
  window.addEventListener('popstate', onChange);
  return () => window.removeEventListener('popstate', onChange);
}

function readLocationPathname(): string {
  return window.location.pathname;
}

/** The server has no location; the client re-reads on hydration. */
function readServerPathname(): string {
  return '';
}

/**
 * The document a static host serves for a path it has no file for — and, for
 * that reason, this app's shell for the routes the export cannot prerender.
 *
 * `DCLUTCH_PAGES_EXPORT=1` writes this to `404.html`, which GitHub Pages serves
 * (with a 404 status) for any unmatched path. `/markets/:address` is the one
 * dynamic route in the app: a Market address is chain data no build can
 * enumerate, so the export writes no page for it and a hard load of a permalink
 * lands here. Rather than dead-ending a link someone was handed, this document
 * hydrates, reads `location.pathname`, and renders the route that path names —
 * the standard static-export fallback, and a natural one here, because the
 * Market detail surface reads every byte it shows from the viewer's chosen RPC
 * endpoint after mount. It never had server-rendered content to lose.
 *
 * A path no route claims still gets a real not-found. The fallback resolves
 * routes, it does not invent them; `lib/exportRouting.ts` holds the table and
 * its tests.
 *
 * The prerendered HTML is the `resolving` branch, because at build time there
 * is no pathname to read. The first client render must match it byte for byte
 * or React discards the tree, so the location is read through
 * `useSyncExternalStore`, whose server snapshot is the empty pathname — and
 * the shell says the honest thing until the client snapshot arrives.
 */
export default function NotFound() {
  const pathname = useSyncExternalStore(
    subscribeToLocation,
    readLocationPathname,
    readServerPathname,
  );

  if (pathname === '') return <ResolvingShell />;
  const route: ExportedRouteV1 = resolveExportedPathnameV1(pathname);
  if (route.kind === 'market-detail') {
    return <MarketDetailWorkspace address={route.address} />;
  }
  return <NotFoundSurface pathname={route.pathname} />;
}

/** Server-rendered and first-client-render alike: nothing is known yet. */
function ResolvingShell() {
  return <PageShell className="product-shell trade-v3-shell" header={<SiteNav />}>
    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Resolving this address</p>
        <h1>One moment<br /><em>reading the path</em></h1>
        <p>This page is served for any address the site has no prebuilt document
        for. If it names a Market, its detail surface loads here; if it names
        nothing, you will get a plain not-found instead.</p>
      </div>
    </section>
  </PageShell>;
}

/** A path no route claims. */
function NotFoundSurface({ pathname }: Readonly<{ pathname: string }>) {
  return <PageShell className="product-shell trade-v3-shell" header={<SiteNav />}>
    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">404 · no such page</p>
        <h1>Nothing lives here<br /><em>at this address</em></h1>
        <p>No route on this site answers to <code>{pathname}</code>. Nothing was
        removed and nothing is broken — this path was never a page. The routes
        below are all of them.</p>
      </div>
      <aside>
        <span>What you asked for</span>
        <strong>Not a route</strong>
        <p><code>{pathname}</code></p>
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Where to go instead</h2><p>Every route this site serves, and what each one is for.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/">The front door →</Anchor>
        <Anchor className="secondary-action" href="/markets">Browse the markets →</Anchor>
        <Anchor className="secondary-action" href="/create">Design a market →</Anchor>
        <Anchor className="secondary-action" href="/portfolio">See what a wallet holds →</Anchor>
        <Anchor className="secondary-action" href="/explorer">Look up any account →</Anchor>
        <Anchor className="secondary-action" href="/console">Operator tools →</Anchor>
      </div>
    </section>
  </PageShell>;
}

function SiteNav() {
  return <Nav />;
}
