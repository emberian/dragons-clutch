import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';

import { SMOKE_MARKETS_V1 } from '@/lib/smokeMarkets';

/**
 * The failure-walk bounty page, written for the wallet that will collect it.
 *
 * Plain words first; the exact bytes live in a drawer at the bottom for
 * whoever wants to build the transaction by hand. The refusal table translates
 * each on-chain refusal into the sentence a person needs, with the code beside
 * it rather than in place of it.
 */

const WALK_STEPS = Object.freeze([
  {
    title: 'The market promised this before it opened',
    body: 'Every market that relies on an outside data source names, in advance, what happens if that source goes silent: after a deadline, the market falls to a written-down fallback outcome. It also locks up a bounty — real money, escrowed before the market opened — for whoever pushes it there.',
  },
  {
    title: 'Wait for the deadline',
    body: 'The deadline is the end of the market’s answer window plus its grace period. The last second an honest answer could still arrive and the first second you can act are back to back — there is no gap where the market is stuck and nobody can move it.',
  },
  {
    title: 'Send one ordinary transaction',
    body: 'One signature — yours. The transaction is small enough to never need anything special: no lookup tables, no second signer, nothing published by the people who walked away. That is deliberate: the one move that must work when everyone else is gone depends on nobody else.',
  },
  {
    title: 'Get paid in the same transaction',
    body: 'The market moves to its fallback outcome, a certificate records that no data source stood behind the result, and the bounty lands in your wallet — all in one transaction. If any part fails, nothing moves and you are out one network fee.',
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
  return <main className="product-shell trade-v3-shell">
    <Nav current="/bounty" status={live ? 'live on devnet' : 'not live yet'} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">The failure walk · anyone can finish an abandoned market</p>
        <h1>Get paid to close<br /><em>an abandoned market.</em></h1>
        <p>If a market&apos;s data source goes silent, the market does not get stuck. After its deadline, any wallet can push it to the fallback outcome it promised before it opened — and collect a bounty the market set aside for exactly this. You need a wallet and a little SOL for the fee. Nothing else: no account, no permission, no relationship with anyone involved.</p>
      </div>
      <aside>
        <span>Where this stands</span>
        <strong>{live ? 'Live on Solana devnet' : 'Not live yet'}</strong>
        {live
          ? <p>The abandoned market is live at <Anchor href={`/markets/${abandoned.address}`}><code>{abandoned.address}</code></Anchor>{abandoned.liveNote === null ? '' : ` — ${abandoned.liveNote}`}. The walk arms only after the market&apos;s deadline passes and its escrow is funded; until then this page is the practice run.</p>
          : <p>No such market is live on any public network today. We have run this exact walk end-to-end on a local test network — the numbers below come from that run, and each one says so.</p>}
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
      <header><span>02</span><div><h2>What it pays, what it costs</h2><p>The bounty is the market&apos;s own posted number, not ours — read it off the market before you act.</p></div></header>
      <div className="trade-v3-evidence">
        <article><span>Bounty (rehearsal)</span><strong>250,000 lamports</strong><small>each market posts its own number before opening</small></article>
        <article><span>Your cost</span><strong>one network fee</strong><small>5,000 lamports in the rehearsal</small></article>
        <article><span>Transaction size</span><strong>fits a plain packet</strong><small>895 bytes measured; the 1,232 limit is never close</small></article>
        <article><span>Signers</span><strong>just you</strong><small>you are also the one who gets paid</small></article>
      </div>
      <p className="direct-status">Two honest warnings. First: the walk pays once — if someone gets there before you, your transaction bounces and you lose only the fee. Second: the walk only works on a market that prepaid its certificate storage; the smoke markets will, and this page will link to them so you are not guessing.</p>
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>If it bounces, here is what the chain is telling you</h2><p>Every refusal has one meaning. The code is beside the sentence, not instead of it.</p></div></header>
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
      <header><span>04</span><div><h2>The exact transaction, for builders</h2><p>Everything above in bytes. You never need this drawer to collect the bounty with a normal wallet flow once a market is live.</p></div></header>
      <details className="trade-v3-bytes">
        <summary>Show the exact instruction and account list</summary>
        <dl>
          <div><dt>Program</dt><dd>the market&apos;s Resolution role program</dd></div>
          <div><dt>Instruction data</dt><dd>32 bytes exactly: magic &quot;DCLTRIX1&quot; (8) · version 1 as u16 LE (2) · action 6 = CommitDeadlineFailure (1) · five zero bytes · the Market&apos;s generation as u64 LE (8) · a nonzero terminal sequence as u64 LE (8, use 1)</dd></div>
          <div><dt>Accounts, in order (22)</dt><dd>0 you (signer, writable) · 1 the Market · 2 the Core program · 3 the Registry activation cache · 4 the Source resolution state (writable) · 5 the failure certificate address (writable) · 6–17 six finalized record pairs, each raw record then its staging address: Source material, Window, Product, Result domain, Portfolio, Capability manifest · 18 the failure-escrow funding state (writable) · 19 Clock sysvar · 20 Rent sysvar · 21 System program</dd></div>
          <div><dt>Address derivations</dt><dd>source state: [&quot;dclutch/source-state/v2&quot;, market, generation_le] under Resolution · certificate: [&quot;dclutch/resolution-cert/v3&quot;, source_state, [4], sequence_le] under Resolution · escrow: [&quot;dclutch/cap-funding/v1&quot;, market, generation_le, entry_index_le, config_id, release_id] under Resolution · records: [&quot;dclutch-raw-record-v1&quot; | &quot;dclutch-record-stage-v1&quot;, schema_id, content_digest] under Registry</dd></div>
          <div><dt>Ordering rule</dt><dd>exact count, exact order, exact writable flags, and no address may appear twice</dd></div>
          <div><dt>Why one transaction</dt><dd>the outcome move, the certificate write, the escrow debit and your payment all happen together or not at all — the record that says you were paid is the same transaction that paid you</dd></div>
        </dl>
      </details>
    </section>

    <footer className="product-footer">
      <span>A silent data source cannot strand a market — it can only pay you to finish it</span>
      <span>{live ? 'The abandoned market is linked above; the walk is yours once its deadline passes' : 'Live markets: none yet — this page will link them when that changes'}</span>
    </footer>
  </main>;
}
