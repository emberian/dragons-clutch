import type { ReactNode } from 'react';

import Anchor from '@/components/Anchor';
import ClusterPicker from '@/components/ClusterPicker';
import { docsIndexHrefV1 } from '@/lib/flags';

/**
 * THE site nav. Every page renders this one component — there is deliberately
 * no second copy of the item list anywhere in the app.
 *
 * It happened once: by 2026-08-27 the app had grown ~20 hand-rolled nav bars,
 * no two alike — `/trade` offered Create/Trade/Liquidity/Release, `/release`
 * offered Direct/General/Release/Explorer, and a reader crossing between them
 * watched the site rearrange itself. One component, one canonical item set,
 * ends that class of drift: a page states which path it is and the rest is
 * decided here.
 *
 * The canonical set is the product: Live · Markets · Pulse · Activity ·
 * Design · Portfolio · Explorer · Docs — plus one Console entry for the
 * operator workspaces, which are indexed at /console instead of competing for
 * top-level slots. A console route lights the Console entry so the reader
 * always knows which side of the site they are on. Pulse and Activity are the
 * two aliveness surfaces; they earned their slots the day they became
 * reachable only by typing a URL.
 */

const PRODUCT_ITEMS = [
  { href: '/live', label: 'Live' },
  { href: '/markets', label: 'Markets' },
  { href: '/pulse', label: 'Pulse' },
  { href: '/activity', label: 'Activity' },
  { href: '/create', label: 'Design' },
  { href: '/portfolio', label: 'Portfolio' },
  { href: '/explorer', label: 'Explorer' },
] as const;

/** Every operator-console route. Any of these lights the Console entry. */
export const CONSOLE_PATHS: readonly string[] = [
  '/console',
  '/direct',
  '/found',
  '/general',
  '/liquidity',
  '/local',
  '/operate',
  '/product-v2',
  '/redeem',
  '/release',
  '/resolution',
  '/trade',
  '/workbench',
];

export default function Nav({
  current,
  status = 'devnet preview',
}: Readonly<{
  /** The route this page is served at, e.g. `/markets`. Sets the active item. */
  current?: string;
  /** Right-hand status pill. One short, true phrase about this surface. */
  status?: ReactNode;
}>) {
  const consoleActive = current !== undefined && CONSOLE_PATHS.includes(current);
  return <>
    <header className="product-nav">
      <Anchor className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Anchor>
      <nav aria-label="Primary navigation">
        {PRODUCT_ITEMS.map((item) => (
          <Anchor
            key={item.href}
            className={item.href === current ? 'active' : undefined}
            href={item.href}
            aria-current={item.href === current ? 'page' : undefined}
          >
            {item.label}
          </Anchor>
        ))}
        <Anchor href={docsIndexHrefV1()}>Docs</Anchor>
        <Anchor className={consoleActive ? 'active' : undefined} href="/console" aria-current={consoleActive ? 'page' : undefined}>Console</Anchor>
      </nav>
      <span className="nav-side">
        <ClusterPicker />
        <span className="preview-control"><i className="preview-dot" />{status}</span>
      </span>
    </header>
    <span id="main-content" className="main-content-anchor" tabIndex={-1} />
  </>;
}
