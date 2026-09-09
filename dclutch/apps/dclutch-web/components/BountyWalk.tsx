import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';

import { SMOKE_MARKETS_V1, SMOKE_WALK_BOUNTY_LAMPORTS_V1 } from '@/lib/smokeMarkets';
import { marketDetailHrefV1 } from '@dclutch/sdk/marketHref';
import { docsHrefV1 } from '@/lib/flags';

/**
 * The failure-walk bounty page, written for the wallet that will collect it.
 *
 * Plain words first; builders are sent to the generated route reference rather
 * than receiving a second handwritten ABI here. The refusal table translates
 * each on-chain refusal into the sentence a person needs, with the code beside
 * it rather than in place of it.
 */

const WALK_STEPS = Object.freeze([
  {
    title: 'Check the bounty and deadline',
    body: 'The market sets its fallback outcome, deadline and bounty before opening. If its data source stops responding, anyone can submit the fallback after the deadline.',
  },
  {
    title: 'Wait for the deadline',
    body: 'You can act once the market’s answer window and grace period have ended.',
  },
  {
    title: 'Send one ordinary transaction',
    body: 'Connect your wallet and submit the fallback. Your wallet signs the transaction and pays the network fee.',
  },
  {
    title: 'Get paid in the same transaction',
    body: 'The transaction records the fallback outcome and transfers the bounty to your wallet. A failed transaction costs the network fee.',
  },
]);

const REFUSALS = Object.freeze([
  { human: 'Too early — the deadline has not passed yet. Nothing is wrong; come back after it.', code: '0x800C' },
  { human: 'Someone beat you to it, or the escrow cannot pay. The bounty is paid exactly once.', code: '0x800E' },
  { human: 'The market never prepaid the certificate’s storage, so the walk cannot run. This is the market’s failure, not yours — skip it.', code: '0x8002' },
  { human: 'The accounts in your transaction are not the exact ones this market needs, or two of them are the same. Rebuild the list and try again.', code: '0x8000' },
  { human: 'The instruction bytes are malformed — wrong length, wrong version, or a zero sequence number.', code: '0x8001' },
  { human: 'The market you named does not match the accounts you supplied.', code: '0x8003' },
]);

export default function BountyWalk() {
  const abandoned = SMOKE_MARKETS_V1.abandoned;
  const live = abandoned.address !== null;
  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/bounty" status={live ? 'live on devnet' : 'not live yet'} />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">The failure walk · anyone can finish an abandoned market</p>
        <h1>Get paid to close<br /><em>an abandoned market.</em></h1>
        <p>If a market&apos;s data source stops responding, you can submit its fallback outcome after the deadline and collect its posted bounty. You need a Solana wallet and enough SOL for the transaction fee.</p>
      </div>
      <aside>
        <span>Available bounties</span>
        <strong>{live ? 'Live on Solana devnet' : 'Not live yet'}</strong>
        {live
          ? <p><Anchor href={marketDetailHrefV1(abandoned.address)}>Open the abandoned market</Anchor>{abandoned.liveNote === null ? '' : ` — ${abandoned.liveNote}`}. Check its deadline and available bounty before submitting.</p>
          : <p>No bounty market is listed yet. Available markets will appear here.</p>}
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>How it works</h2><p>Four steps. Only one of them is yours.</p></div></header>
      <div className="market-card-grid">
        {WALK_STEPS.map((step, index) => (
          <article key={step.title} className="portfolio-entry">
            <div className="market-card-top"><span className="phase-chip">{index + 1}</span><strong>{step.title}</strong></div>
            <p className="direct-status">{step.body}</p>
          </article>
        ))}
      </div>
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Payment and fees</h2><p>Each market posts its bounty before opening. The figures below are from a local test run.</p></div></header>
      <div className="trade-v3-evidence">
        <article><span>Bounty (rehearsal)</span><strong>{SMOKE_WALK_BOUNTY_LAMPORTS_V1.toLocaleString('en-US')} lamports</strong><small>each market posts its own number before opening</small></article>
        <article><span>Your cost</span><strong>one network fee</strong><small>5,000 lamports in the rehearsal</small></article>
        <article><span>Transaction size</span><strong>fits a plain packet</strong><small>895 bytes with your one signature; the limit is 1,232</small></article>
        <article><span>Signers</span><strong>just you</strong><small>you are also the one who gets paid</small></article>
      </div>
      <p className="direct-status">The bounty pays once. If someone collects it first, your transaction fails and you still pay the network fee. The market must have funded the storage needed to record its outcome.</p>
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>If your transaction fails</h2><p>Find the error code below for the next step.</p></div></header>
      <ul className="market-bindings">
        {REFUSALS.map((refusal) => (
          <li key={refusal.code} className="check-fail">
            <span aria-hidden="true">×</span>
            <div><strong>{refusal.human}</strong><small>chain code {refusal.code}</small></div>
          </li>
        ))}
      </ul>
    </section>

    <section className="trade-v3-card">
      <header><span>04</span><div><h2>For wallet developers</h2><p>The route reference lists the instruction format and required accounts.</p></div></header>
      <p className="direct-status"><a href={docsHrefV1('reference/abi/routeCensus.html', 'docs/reference/abi/routeCensus.md')}>Open the generated route reference →</a></p>
    </section>

    <footer className="product-footer">
      <span>Market fallback bounties</span>
      <span>{live ? 'Check the linked market for its deadline' : 'No bounty market listed'}</span>
    </footer>
  </PageShell>;
}
