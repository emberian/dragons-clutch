import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { docsHrefV1 } from '@/lib/flags';

/**
 * `/console` — the directory of the operator consoles.
 *
 * The product pages (Markets, Create, Portfolio, Explorer) are for anyone.
 * Everything listed here is an operator surface for someone operating or
 * building on the protocol. Some entries are read-only readiness views; a
 * route may construct bytes only when its own preflight says so. This page exists so those
 * tools stop masquerading as product pages — one entry per console, one plain
 * sentence per entry saying who it is for and what it does.
 */

type ConsoleEntry = Readonly<{ href: string; name: string; blurb: string }>;

const ENTRIES: readonly ConsoleEntry[] = [
  {
    href: '/workbench',
    name: 'Lifecycle workbench',
    blurb:
      'Read a market lifecycle readiness map against the chain you choose. It does not create, trade, resolve, or redeem.',
  },
  {
    href: '/found',
    name: 'Founding',
    blurb:
      'Inspect the older partial founding packet pair. It cannot open a current devnet market.',
  },
  {
    href: '/product-v2',
    name: 'Product studio',
    blurb:
      'For market authors: write a payoff curve as exact fractions and read back precisely what each outcome would pay.',
  },
  {
    href: '/trade',
    name: 'Direct trade',
    blurb:
      'Inspect one Direct route and preview its exact integer arithmetic. Browser signing and submission are unavailable.',
  },
  {
    href: '/liquidity',
    name: 'Liquidity',
    blurb:
      'For dealers: build and download an unsigned transaction for adding or withdrawing dealer equity, checked against the chain first. You sign and send it yourself, elsewhere.',
  },
  {
    href: '/redeem',
    name: 'Wallet redemption (not open yet)',
    blurb:
      'Connect your wallet and see the claims it holds across the deployment. Paying out winning claims is not available yet; this is where it will happen, and the page tells you so rather than offering a button that cannot work.',
  },
  {
    href: '/resolution',
    name: 'Resolution',
    blurb:
      'A read-only view of what a market still needs before its oracle answer can be accepted, and how far along the one you point it at has got. Resolution is not open yet.',
  },
  {
    href: '/general',
    name: 'General clearing',
    blurb:
      'For operators: paste a clearing plan produced by the operator program, have the browser re-check every field against the chain, and download the unsigned transaction. Nothing is sent from here.',
  },
  {
    href: '/release',
    name: 'Release activation',
    blurb:
      'Activate already-installed checked artifacts against a Registry. This does not update programs and is not the current devnet Upgrade workflow.',
  },
  {
    href: '/operate',
    name: 'Operations',
    blurb:
      'For operators of a running deployment: see every action the deployed programs accept right now, and export unsigned bytes for the ones a browser can build.',
  },
  {
    href: '/local',
    name: 'Local successor',
    blurb:
      'For developers: read the checkpointed local validator and confirm its finalized state matches the published evidence, byte for byte.',
  },
  {
    href: '/campaign',
    name: 'Campaign record',
    blurb:
      'One market founded, resolved and retired on a private rehearsal validator, drawn from the campaign’s own transcript: per-outcome odds, the vault, the work each stage cost, and the terminal answer.',
  },
  {
    href: '/population',
    name: 'Population record',
    blurb:
      'A whole world of markets drawn from one seed and driven on a private validator: every market’s odds path on a shared clock, the run’s own event timeline, and the strip saying what executed against what the chain refused, never attempted, or blocked.',
  },
];

export default function ConsoleDirectory() {
  return <main className="product-shell trade-v3-shell">
    <Nav current="/console" status="operator tools" />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Tools for running and building on dClutch</p>
        <h1>Operator<br /><em>consoles.</em></h1>
        <p>If you came here to look around or to trade, start at{' '}
        <Anchor href="/markets">Markets</Anchor> instead. These are working
        tools, not product pages.</p>
        <p>Being listed here does not mean a page can send a transaction —
        several deliberately cannot.</p>
        <p>Where a console asks you to paste a file, it names the tool that
        produces it, right on the input. The full list is in the README&apos;s
        table <a href={docsHrefV1('readme.html', 'README.md')}>“The artifacts,
        and where they come from”</a>.</p>
      </div>
    </section>

    <section className="console-index" aria-label="Operator consoles">
      {ENTRIES.map((entry) => (
        <Anchor key={entry.href} className="console-entry" href={entry.href}>
          <strong>{entry.name}</strong>
          <span>{entry.blurb}</span>
          <em aria-hidden="true">→</em>
        </Anchor>
      ))}
    </section>
  </main>;
}
