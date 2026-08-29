import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import {
  DEVNET_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
  PROTOCOL_ROLE_MEANING_V1,
} from '@/lib/deployments';
import {
  PUBLIC_DEVNET_CUT_V1,
  publicCutExplorerHrefV1,
  publicCutMarketHrefV1,
  publicCutTransactionHrefV1,
  type PublicCutActivityStepV1,
} from '@/lib/publicCutStaging';

const LIFECYCLE = [
  ['01', 'Found', 'Lock collateral and publish the market.', 'found'],
  ['02', 'Join', 'Choose an outcome and enter with a real Position.', 'join'],
  ['03', 'Trade', 'Exchange claims through a signed Direct route.', 'trade'],
  ['04', 'Resolve', 'Read the named oracle path after the deadline.', 'resolve'],
  ['05', 'Redeem', 'Burn winning claims and release their collateral.', 'redeem'],
] as const;

function short(address: string): string {
  return `${address.slice(0, 5)}…${address.slice(-5)}`;
}

/**
 * A screenshot-first public launch page. It deliberately uses the same baked
 * devnet deployment and public workspaces as the app, so its calls to action
 * lead into the thing being announced instead of a parallel marketing shell.
 */
export default function LaunchStory() {
  const cut = PUBLIC_DEVNET_CUT_V1;
  const opened = cut.market !== null;
  const marketHref = publicCutMarketHrefV1(cut);
  return <main className="product-shell launch-shell">
    <Nav current="/live" status="public devnet · live" />

    <section className="launch-hero launch-shot">
      <div className="launch-hero-copy">
        <p className="eyebrow"><span className="launch-live-dot" />Dragon&apos;s Clutch · public devnet</p>
        <h1>Markets that<br />resolve <em>in public.</em></h1>
        <p className="launch-deck">dClutch turns a bounded real-world question into fully collateralized Solana claims. {opened ? 'You can inspect this market, trade on devnet, and watch its steps settle on chain.' : 'You can inspect the deployed programs now. This page will link one checked public market and its transactions when it opens.'}</p>
        <div className="launch-actions">
          <Anchor className="launch-primary" href={marketHref}>{opened ? 'Enter the live market' : 'Explore the deployment'} <span>↗</span></Anchor>
          <Anchor className="launch-secondary" href={publicCutExplorerHrefV1(cut)}>{opened ? 'Watch this market on chain' : 'Watch the chain'}</Anchor>
          <Anchor className="launch-secondary" href="/activity">Read activity</Anchor>
        </div>
      </div>

      <aside className="launch-scoreboard" aria-label="Launch facts">
        <div className="launch-network"><span>NETWORK</span><strong>DEVNET</strong><small>FREE TEST SOL · NO REAL VALUE</small></div>
        <div className="launch-stats">
          <article><strong>7</strong><span>programs</span></article>
          <article><strong>64</strong><span>lock cap</span></article>
          <article><strong>0.50%</strong><span>per side</span></article>
        </div>
        <div className="launch-terminal">
          <span>release / current</span>
          <code>FOUND → DIRECT → RESOLVE → REDEEM</code>
          <p><i /> public cluster reachable</p>
        </div>
      </aside>
    </section>

    <section className="launch-rail launch-shot" aria-labelledby="launch-lifecycle">
      <header>
        <p className="eyebrow">One market · one visible lifecycle</p>
        <h2 id="launch-lifecycle">Follow the whole thing.</h2>
        <p>{opened ? 'These links lead to real devnet transactions, not a replay rendered from fixtures. Open the explorer at any point and inspect the accounts yourself.' : 'No public market is named yet. When one opens, this same checked manifest supplies its Market and transaction links; until then, no lifecycle activity is invented.'}</p>
      </header>
      <ol>
        {LIFECYCLE.map(([number, title, detail, step]) => <li key={title}>
          <span>{number}</span><strong>{title}</strong><p>{detail}</p>
          {step === 'join'
            ? <Anchor href={`${marketHref}#join`}>{opened ? 'Check your standing and join →' : 'See what joining creates →'}</Anchor>
            : step === null || publicCutTransactionHrefV1(step as PublicCutActivityStepV1, cut) === null ? null : <Anchor href={publicCutTransactionHrefV1(step as PublicCutActivityStepV1, cut)!}>Open {title.toLowerCase()} transaction →</Anchor>}
        </li>)}
      </ol>
    </section>

    <section className="launch-grid">
      <article className="launch-card launch-card-wide">
        <p className="eyebrow">What changed</p>
        <h2>{opened ? 'The whole route fits devnet now.' : 'The public route is staged.'}</h2>
        <p>{opened ? 'Founding stays within Solana\'s 64-account lock limit. Direct trade uses signed, portable tickets. Resolution can use the sponsored SOL/USD Pyth account without a paid API key. Redemption returns collateral through the same public market.' : 'The deployment is public, but no current-compatible Market is named by this cut. Opening it will be one checked manifest update, not a fixture replay or a marketing-only address list.'}</p>
        <div className="launch-tags"><span>≤64 accounts</span><span>sponsored Pyth</span><span>portable Direct tickets</span><span>full collateral</span></div>
      </article>

      <article className="launch-card launch-card-acid">
        <span className="launch-card-index">LIVE / 01</span>
        <h2>No token.<br />No presale.<br />Just the protocol.</h2>
        <p>Devnet SOL and devnet collateral are test assets. Use them, break things, and tell us where the edges feel wrong.</p>
        <Anchor href="/portfolio">Connect a devnet wallet →</Anchor>
      </article>
    </section>

    <section className="launch-programs launch-shot">
      <header><div><p className="eyebrow">The deployed substrate</p><h2>Seven programs. Familiar addresses.</h2></div><Anchor className="launch-secondary" href="/release">Open release view</Anchor></header>
      <div className="launch-program-grid">
        {PROTOCOL_ROLES_V1.map((role) => <article key={role}>
          <span>{role}</span>
          <strong>{short(DEVNET_DEPLOYMENT_V1.programs[role])}</strong>
          <p>{PROTOCOL_ROLE_MEANING_V1[role]}</p>
        </article>)}
      </div>
    </section>

    <section className="launch-finale">
      <p className="eyebrow">The demo is the network</p>
      <h2>Don&apos;t take our word for it.<br /><em>Open the market.</em></h2>
      <div className="launch-actions">
        <Anchor className="launch-primary" href={marketHref}>{opened ? 'Open the market' : 'Explore markets'} <span>↗</span></Anchor>
        <Anchor className="launch-secondary" href="/trade">Prepare a Direct trade</Anchor>
        <Anchor className="launch-secondary" href="/smoke">Read the public run</Anchor>
      </div>
      <p className="launch-fineprint">Public Solana devnet preview. Test assets have no monetary value. This is low-assurance software under active development, not a financial product.</p>
    </section>
  </main>;
}
