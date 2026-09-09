import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import {
  DEVNET_DEPLOYMENT_V1,
  deployedProgramRolesV1,
  PROTOCOL_ROLE_MEANING_V1,
} from '@dclutch/sdk/deployments';
import {
  PUBLIC_DEVNET_CUT_V1,
  publicCutExplorerHrefV1,
  publicCutMarketHrefV1,
  publicCutTransactionHrefV1,
  type PublicCutActivityStepV1,
} from '@dclutch/sdk/publicCutStaging';

// The main stages of a price-range market.
const LIFECYCLE = [
  ['01', 'Found', 'Lock up the collateral and publish the market.', 'found'],
  ['02', 'Join', 'Create your wallet’s market accounts.', 'join'],
  ['03', 'Trade', 'Buy and sell claims with a transaction your own wallet signs.', 'trade'],
  ['04', 'Resolve', 'Submit a source report for the market’s resolution window, or its fallback after the deadline.', 'resolve'],
  ['05', 'Redeem', 'Redeem winning claims for collateral after the market resolves.', 'redeem'],
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
  const roles = deployedProgramRolesV1(DEVNET_DEPLOYMENT_V1);
  const marketHref = publicCutMarketHrefV1(cut);
  return <PageShell className="product-shell launch-shell" header={<Nav current="/live" status="Solana devnet" />}>

    <section className="launch-hero launch-shot">
      <div className="launch-hero-copy">
        <p className="eyebrow"><span className="launch-live-dot" />Dragon&apos;s Clutch · public devnet</p>
        <h1>Explore markets<br />on <em>Solana devnet.</em></h1>
        <p className="launch-deck">dClutch turns a real-world question with a definite answer into fully collateralized Solana claims. {opened ? 'Open the featured market record to see its status and available actions.' : 'Browse the deployment and its recent transactions.'}</p>
        <div className="launch-actions">
          <Anchor className="launch-primary" href={marketHref}>{opened ? 'View featured market' : 'Explore the deployment'} <span>↗</span></Anchor>
          <Anchor className="launch-secondary" href={publicCutExplorerHrefV1(cut)}>{opened ? 'Watch this market on chain' : 'Watch the chain'}</Anchor>
          <Anchor className="launch-secondary" href="/activity">Read activity</Anchor>
        </div>
      </div>

      <aside className="launch-scoreboard" aria-label="Launch facts">
        <div className="launch-network"><span>NETWORK</span><strong>DEVNET</strong><small>TEST TOKENS</small></div>
        <div className="launch-stats">
          <article><strong>{roles.length}</strong><span>programs</span></article>
          <article><strong>{opened ? 'LINKED' : 'NONE'}</strong><span>featured market</span></article>
        </div>
        <div className="launch-terminal">
          <span>market lifecycle</span>
          <code>FOUND → TRADE → RESOLVE → REDEEM</code>
        </div>
      </aside>
    </section>

    <section className="launch-rail launch-shot" aria-labelledby="launch-lifecycle">
      <header>
        <p className="eyebrow">Market lifecycle</p>
        <h2 id="launch-lifecycle">From opening to payout.</h2>
        <p>{opened ? 'Follow the linked transactions or open the market to view its current stage.' : 'No featured market is linked yet. Browse Markets for the selected deployment.'}</p>
      </header>
      <ol>
        {LIFECYCLE.map(([number, title, detail, step]) => <li key={title}>
          <span>{number}</span><strong>{title}</strong><p>{detail}</p>
          {step === 'join'
            /*
              No `#join` fragment. It addressed `JoinPanel`'s default export,
              which no route has rendered since the trade panel started
              importing only `JoinStanding`, so every reader who followed it
              landed at the top of the market page wondering what they had
              missed. Joining is step 1 of the market page's own flow and that
              page is the whole destination.
            */
            ? <Anchor href={marketHref}>{opened ? 'Check your standing and join →' : 'See what joining creates →'}</Anchor>
            : step === null || publicCutTransactionHrefV1(step as PublicCutActivityStepV1, cut) === null ? null : <Anchor href={publicCutTransactionHrefV1(step as PublicCutActivityStepV1, cut)!}>Open {title.toLowerCase()} transaction →</Anchor>}
        </li>)}
      </ol>
    </section>

    <section className="launch-grid">
      <article className="launch-card launch-card-wide">
        <p className="eyebrow">How it works</p>
        <h2>Fully funded payouts.</h2>
        <p>The market locks collateral before issuing claims. Its rules fix the outcomes, source and payout amounts before trading begins.</p>
        <div className="launch-tags"><span>Pyth price feeds</span><span>signed offers</span><span>full collateral</span></div>
      </article>

      <article className="launch-card launch-card-acid">
        <span className="launch-card-index">DEVNET / 01</span>
        <h2>Try it with<br />test tokens.</h2>
        <p>Use a devnet wallet to explore the available market actions.</p>
        <Anchor href="/portfolio">Connect a devnet wallet →</Anchor>
      </article>
    </section>

    <section className="launch-programs launch-shot">
      <header><div><p className="eyebrow">Deployment</p><h2>{roles.length} program addresses.</h2></div><Anchor className="launch-secondary" href="/release">Open release view</Anchor></header>
      <div className="launch-program-grid">
        {roles.map((role) => <article key={role}>
          <span>{role}</span>
          <strong>{short(DEVNET_DEPLOYMENT_V1.programs[role])}</strong>
          <p>{role === 'accelerator' ? 'Runs the shared market calculations.' : PROTOCOL_ROLE_MEANING_V1[role]}</p>
        </article>)}
      </div>
    </section>

    <section className="launch-finale">
      <p className="eyebrow">Get started</p>
      <h2>Choose a market.<br /><em>See what you can do.</em></h2>
      <div className="launch-actions">
        <Anchor className="launch-primary" href={marketHref}>{opened ? 'Open the market' : 'Explore markets'} <span>↗</span></Anchor>
        <Anchor className="launch-secondary" href="/trade">See how a trade is built</Anchor>
        <Anchor className="launch-secondary" href="/smoke">Read the public run</Anchor>
        <Anchor className="launch-secondary" href="/campaign">Follow a sample market</Anchor>
        <Anchor className="launch-secondary" href="/population">Browse simulated markets</Anchor>
      </div>
      <p className="launch-fineprint">Solana devnet · test tokens.</p>
    </section>
  </PageShell>;
}
