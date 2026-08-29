import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';

import { SMOKE_MARKETS_V1, smokeIsLiveV1 } from '@/lib/smokeMarkets';
import { marketDetailHrefV1 } from '@/lib/marketHref';

/**
 * The three-markets story, written for the reader.
 *
 * Three public test markets, each proving that a different kind of truth can
 * settle a market without anyone's permission. Everything here is a plan a
 * visitor can understand in one read; the exact mechanics live behind the
 * bounty page's "show me the exact bytes" drawer, not in the headline.
 */
export default function SmokeStory() {
  const live = smokeIsLiveV1();
  const marketLink = (market: { address: string | null; liveNote: string | null }) =>
    market.address === null ? null : (
      <div className="direct-actions">
        <Anchor className="secondary-action" href={marketDetailHrefV1(market.address)}>Open the live market →</Anchor>
        {market.liveNote === null ? null : <span className="direct-status">{market.liveNote}</span>}
      </div>
    );
  return <main className="product-shell trade-v3-shell">
    <Nav current="/smoke" status={live ? 'live on devnet' : 'not live yet'} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">The smoke · three markets, three kinds of truth</p>
        <h1>Can a market settle itself?<br /><em>Three public tests.</em></h1>
        <p>A dClutch market is a promise: put money behind an answer, and when the facts arrive, the right side gets paid — automatically, with nobody in the middle to appeal to or be surprised by. Before anything real runs on this protocol, we will run three small public markets on Solana devnet. Each one tests a different way the facts can arrive.</p>
      </div>
      <aside>
        <span>Where this stands</span>
        <strong>{live ? 'Live on Solana devnet' : 'Not live yet'}</strong>
        {live
          ? <p>The protocol substrate is deployed on Solana devnet at permanent addresses, and the markets below link straight to their live on-chain accounts as each one is founded. Devnet SOL is free test money — this is a public rehearsal, not an investment.</p>
          : <p>The seven protocol programs are deployed at permanent addresses on Solana devnet. None of these three smoke markets exists yet. Everything below has been rehearsed end-to-end on local test machines; when each market is founded on devnet, this page will link straight to its account.</p>}
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Oracle truth · the price market</h2><p>Will SOL/USD finish inside a chosen range at a chosen time?</p></div></header>
      <p className="direct-status">The answer comes from Pyth, the same price feed most of Solana already trusts. When the window closes, anyone can submit Pyth&apos;s signed price and the market pays the range it landed in. Nobody decides the outcome — the price does. If Pyth publishes nothing usable in the window, the market falls to a fallback outcome that was named before it opened, so your money is never stuck.</p>
      {marketLink(SMOKE_MARKETS_V1.price)}
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Cross-chain truth · the graduation market</h2><p>Did this token graduate on Solana mainnet? A market on devnet pays out on it.</p></div></header>
      <p className="direct-status">The event happens on one network; the market lives on another. A disclosed messenger reads mainnet and signs exactly what it saw — the raw bytes, never an interpretation. You are trusting that messenger not to lie, and the market says so up front instead of hiding it. Two things keep it honest: every statement it signs can be checked against mainnet by anyone, forever; and if it goes silent, the market does not hang — it walks to its named fallback.</p>
      {marketLink(SMOKE_MARKETS_V1.graduation)}
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>Adversarial truth · the abandoned market</h2><p>We will switch the messenger off on purpose. Then you finish the market and get paid for it.</p></div></header>
      <p className="direct-status">This is the test most markets never dare to run: what happens when everyone responsible walks away? Here, the market has already set money aside for exactly this moment. Once the deadline passes, any wallet — yours — can send one ordinary transaction that closes the market to its pre-announced fallback outcome and collects the posted bounty for doing it. No permission, no account, no special software. <strong>That is how it will work; none of these markets is open yet</strong>, so today this is a description rather than an invitation.</p>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/bounty">How to collect the bounty →</Anchor>
      </div>
    </section>

    <footer className="product-footer">
      <span>Three markets, founded once, run in public, then wound down</span>
      <span>{live ? 'The substrate is live on devnet; each market links above as it founds' : 'Live dates: none yet — this page will say so when that changes'}</span>
    </footer>
  </main>;
}
