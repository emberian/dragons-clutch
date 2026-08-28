import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { docsHrefV1 } from '@/lib/flags';

/**
 * `/console` — the directory of the operator consoles.
 *
 * The product pages (Markets, Create, Portfolio, Explorer) are for anyone.
 * Everything listed here is a working tool for someone operating or building
 * on the protocol: each console reads real chain state, refuses by name, and
 * mostly hands you unsigned bytes to sign elsewhere. This page exists so those
 * tools stop masquerading as product pages — one entry per console, one plain
 * sentence per entry saying who it is for and what it does.
 */

type ConsoleEntry = Readonly<{ href: string; name: string; blurb: string }>;

const ENTRIES: readonly ConsoleEntry[] = [
  {
    href: '/workbench',
    name: 'Lifecycle workbench',
    blurb:
      'Walk a market’s whole life — author, fund, trade, resolve, claim — against whatever chain you point it at. Start here if you are new to operating dClutch.',
  },
  {
    href: '/found',
    name: 'Founding',
    blurb:
      'For market authors: derive a new market’s accounts from the Registry and download the two unsigned transactions that found it.',
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
      'For traders testing routes: check one trade against live chain state and build its unsigned transaction pair.',
  },
  {
    href: '/liquidity',
    name: 'Liquidity',
    blurb:
      'For dealers: contribute or redeem dealer equity through a chain-checked route, and download the unsigned transaction for it.',
  },
  {
    href: '/redeem',
    name: 'Representation',
    blurb:
      'For claim holders: transfer claim tokens, and prepare the open and retirement steps of redemption from live market state.',
  },
  {
    href: '/resolution',
    name: 'Resolution',
    blurb:
      'The lifecycle workbench opened at the resolve stage — for settling a market whose terminal window has arrived.',
  },
  {
    href: '/general',
    name: 'General clearing',
    blurb:
      'For operators running settlement: paste a clearing plan produced by the operator program, let the browser re-check every field, and download the unsigned packet.',
  },
  {
    href: '/release',
    name: 'Release activation',
    blurb:
      'For operators deploying the protocol: paste a checked release — the evidence bundle the build pipeline produces to prove exactly which code a deployment runs — and activate it against a Registry.',
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
];

export default function ConsoleDirectory() {
  return <main className="product-shell trade-v3-shell">
    <Nav current="/console" status="operator tools" />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">The toolbox behind the product</p>
        <h1>Operator<br /><em>consoles.</em></h1>
        <p>These are working tools for people operating or building on the
        protocol, not product pages. Each one reads real state from the chain
        you point it at, refuses by name when that state disagrees, and in most
        cases hands you unsigned bytes to sign somewhere you trust. If you are
        here to browse or trade, start at <Anchor href="/markets">Markets</Anchor> instead.</p>
        <p>Every file a console asks for has exactly one producer, and the
        console says which, right on the input. The answer key is the
        README&apos;s table <a href={docsHrefV1('readme.html', 'README.md')}>“The
        artifacts, and where they come from”</a> — if a console asks you to
        paste something and you can&apos;t tell where it comes from, that is a
        bug in the console.</p>
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
