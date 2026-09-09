import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';

import { SMOKE_MARKETS_V1, smokeIsLiveV1 } from '@/lib/smokeMarkets';
import { marketDetailHrefV1 } from '@dclutch/sdk/marketHref';

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
  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/smoke" status={live ? 'live on devnet' : 'not live yet'} />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Solana devnet examples</p>
        <h1>Three ways a market<br /><em>reaches an outcome.</em></h1>
        <p>These example markets use a price feed, a report from another network, and a fallback when a source stops responding. Each market sets its rules before opening.</p>
      </div>
      <aside>
        <span>Market status</span>
        <strong>{live ? 'Live on Solana devnet' : 'Not live yet'}</strong>
        {live
          ? <p>Open the linked markets below to view their accounts and activity. They use devnet test tokens.</p>
          : <p>These example markets are not listed on devnet yet. Their links will appear below when available.</p>}
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>A price feed</h2><p>Which range will SOL/USD finish in?</p></div></header>
      <p className="direct-status">The market reads a Pyth price for its chosen time window and selects the range containing that price. Claims on the selected range can then be redeemed. If no usable price arrives before the deadline, the market uses its preset fallback.</p>
      {marketLink(SMOKE_MARKETS_V1.price)}
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>A report from another network</h2><p>Did this token graduate on Solana mainnet?</p></div></header>
      <p className="direct-status">A designated reporter reads the token&apos;s mainnet account and signs a report for the devnet market. The market relies on that reporter for the observation. You can compare the signed report with the mainnet account. If the reporter stops responding, the market uses its preset fallback after the deadline.</p>
      {marketLink(SMOKE_MARKETS_V1.graduation)}
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>A source stops responding</h2><p>Submit the fallback and collect the bounty.</p></div></header>
      <p className="direct-status">In this example, the reporter is switched off. Once the deadline passes, anyone can submit the market&apos;s preset fallback outcome and collect its posted bounty.</p>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/bounty">How to collect the bounty →</Anchor>
      </div>
    </section>

    <footer className="product-footer">
      <span>Example markets on Solana devnet</span>
      <span>{live ? 'Available markets are linked above' : 'No launch date set'}</span>
    </footer>
  </PageShell>;
}
