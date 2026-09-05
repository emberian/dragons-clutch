import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import {
  DEVNET_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
  PROTOCOL_ROLE_MEANING_V1,
} from '@dclutch/sdk/deployments';
import {
  PUBLIC_DEVNET_CUT_V1,
  publicCutExplorerHrefV1,
  publicCutMarketHrefV1,
  publicCutTransactionHrefV1,
  type PublicCutActivityStepV1,
} from '@dclutch/sdk/publicCutStaging';
import { MAX_TX_ACCOUNT_LOCKS_V2 } from '@dclutch/sdk/generated/genericFoundingV1';

// Steps 01-03 are the chain that works today; a reader can do all three. 04 and
// 05 are written in the future tense on purpose -- they are a later release, and
// a numbered rail of five imperatives reads as five things you can do now.
const LIFECYCLE = [
  ['01', 'Found', 'Lock up the collateral and publish the market.', 'found'],
  ['02', 'Join', 'Pick an outcome and put up collateral to hold claims on it.', 'join'],
  ['03', 'Trade', 'Buy and sell claims with a transaction your own wallet signs.', 'trade'],
  ['04', 'Resolve', 'Not yet. After the deadline, the oracle the market named will settle it.', 'resolve'],
  ['05', 'Redeem', 'Not yet — no market has an answer. When one does, winning claims burn here and release the collateral behind them.', 'redeem'],
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
  return <PageShell className="product-shell launch-shell" header={<Nav current="/live" status={opened ? 'public devnet · market open' : 'public devnet · programs deployed'} />}>

    <section className="launch-hero launch-shot">
      <div className="launch-hero-copy">
        <p className="eyebrow"><span className="launch-live-dot" />Dragon&apos;s Clutch · public devnet</p>
        <h1>Markets you can<br />check <em>yourself.</em></h1>
        <p className="launch-deck">dClutch turns a real-world question with a definite answer into fully collateralized Solana claims. {opened ? 'You can read this market and join it on devnet — every step from the chain itself, not from us. Whether it can take a trade is a chain fact, and its own page says which.' : 'You can read the deployed programs right now. When a market opens, this page links to it and to its transactions.'}</p>
        <div className="launch-actions">
          <Anchor className="launch-primary" href={marketHref}>{opened ? 'Enter the live market' : 'Explore the deployment'} <span>↗</span></Anchor>
          <Anchor className="launch-secondary" href={publicCutExplorerHrefV1(cut)}>{opened ? 'Watch this market on chain' : 'Watch the chain'}</Anchor>
          <Anchor className="launch-secondary" href="/activity">Read activity</Anchor>
        </div>
      </div>

      <aside className="launch-scoreboard" aria-label="Launch facts">
        <div className="launch-network"><span>NETWORK</span><strong>DEVNET</strong><small>FREE TEST SOL · NO REAL VALUE</small></div>
        <div className="launch-stats">
          <article><strong>{PROTOCOL_ROLES_V1.length}</strong><span>programs</span></article>
          <article><strong>{MAX_TX_ACCOUNT_LOCKS_V2}</strong><span>lock cap</span></article>
          <article><strong>{opened ? 'YES' : 'NO'}</strong><span>market open</span></article>
        </div>
        <div className="launch-terminal">
          <span>release / current</span>
          <code>FOUND → JOIN → TRADE</code>
          <p><i /> nothing has resolved yet</p>
        </div>
      </aside>
    </section>

    <section className="launch-rail launch-shot" aria-labelledby="launch-lifecycle">
      <header>
        <p className="eyebrow">One market · one visible lifecycle</p>
        <h2 id="launch-lifecycle">Follow the three steps that work today.</h2>
        <p>{opened ? 'Every step below leads into the live market on devnet, not a replay. Open the explorer at any point and read the accounts yourself. A step shows a transaction link only once one has been read back off the chain.' : 'No market is open yet. When one opens, its links appear here. Until then this page shows you nothing it cannot back up.'}</p>
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
        <p className="eyebrow">What changed</p>
        <h2>{opened ? 'Found, join and trade fit devnet now.' : 'Deployed, not yet open.'}</h2>
        <p>{opened ? `Founding a market stays inside Solana's ${MAX_TX_ACCOUNT_LOCKS_V2}-account limit. A trade is a portable ticket your own wallet signs. Redeeming a winning claim runs in the browser too — no market has reached an answer yet, so there is nothing to redeem on this one.` : `${PROTOCOL_ROLES_V1.length} programs are live on devnet and readable now. No market is open on them yet. When one opens, this page links to it and to its transactions.`}</p>
        <div className="launch-tags"><span>≤{MAX_TX_ACCOUNT_LOCKS_V2} accounts</span><span>sponsored Pyth</span><span>portable Direct tickets</span><span>full collateral</span></div>
      </article>

      <article className="launch-card launch-card-acid">
        <span className="launch-card-index">DEVNET / 01</span>
        <h2>No token.<br />No presale.<br />Just the protocol.</h2>
        <p>Devnet SOL and devnet collateral are test assets. Use them, break things, and tell us where the edges feel wrong.</p>
        <Anchor href="/portfolio">Connect a devnet wallet →</Anchor>
      </article>
    </section>

    <section className="launch-programs launch-shot">
      <header><div><p className="eyebrow">The deployed substrate</p><h2>{PROTOCOL_ROLES_V1.length} programs. Familiar addresses.</h2></div><Anchor className="launch-secondary" href="/release">Open release view</Anchor></header>
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
      <h2>Don&apos;t take our word for it.<br /><em>{opened ? 'Open the market.' : 'Read the chain.'}</em></h2>
      <div className="launch-actions">
        <Anchor className="launch-primary" href={marketHref}>{opened ? 'Open the market' : 'Explore markets'} <span>↗</span></Anchor>
        <Anchor className="launch-secondary" href="/trade">See how a trade is built</Anchor>
        <Anchor className="launch-secondary" href="/smoke">Read the public run</Anchor>
        <Anchor className="launch-secondary" href="/campaign">Read one rehearsal lifecycle</Anchor>
        <Anchor className="launch-secondary" href="/population">Explore the simulated population</Anchor>
      </div>
      <p className="launch-fineprint">Public Solana devnet preview. Test assets have no monetary value. This is low-assurance software under active development, not a financial product.</p>
    </section>
  </PageShell>;
}
