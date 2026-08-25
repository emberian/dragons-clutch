'use client';

import Link from 'next/link';
import { useMemo, useState } from 'react';

const inventory = [
  { label: 'Below $120', bid: 12, ask: 16, claims: 8_240, tone: 0 },
  { label: '$120 – $159.99', bid: 25, ask: 29, claims: 6_810, tone: 1 },
  { label: '$160 – $199.99', bid: 36, ask: 40, claims: 4_930, tone: 2 },
  { label: '$200 or above', bid: 19, ask: 23, claims: 7_120, tone: 3 },
];

const weights = [18, 27, 31, 24];

export default function LiquidityPreview() {
  const [mode, setMode] = useState<'add' | 'remove'>('add');
  const [amount, setAmount] = useState('5000');
  const collateral = Number(amount) || 0;
  const allocations = useMemo(
    () => weights.map((weight) => (collateral * weight) / 100),
    [collateral],
  );

  return (
    <main className="product-shell liquidity-shell">
      <header className="product-nav">
        <Link className="brand" href="/" aria-label="dClutch markets home">
          <span className="brand-mark" aria-hidden="true">dC</span><span>dClutch</span>
        </Link>
        <nav aria-label="Primary navigation">
          <Link href="/">Markets</Link><Link href="/create">Create</Link><Link className="active" href="/liquidity">Liquidity</Link><Link href="/explorer">Explorer</Link>
        </nav>
        <div className="preview-control"><span className="preview-dot" /> Interface preview</div>
      </header>

      <div className="preview-banner">
        <span>Dealer preview</span>
        Illustrative inventory only—no pool account, wallet authority, or transaction submission is claimed.
        <Link href="/">← Return to market</Link>
      </div>

      <section className="liquidity-heading">
        <div>
          <p className="eyebrow">Inventory-bounded liquidity</p>
          <h1>Fund the inventory.<br /><em>Quote what custody can honor.</em></h1>
        </div>
        <p>Dealer prices are constrained by real collateral and outcome claims. Principal, fees, service capital, and rent never collapse into one convenient number.</p>
      </section>

      <section className="liquidity-grid">
        <div className="inventory-panel">
          <div className="inventory-toolbar">
            <div><span className="panel-label">Illustrative pool surface</span><h2>SOL year-end bands · USDC Realm</h2></div>
            <div className="pool-state"><i /><span>Awaiting local chain</span></div>
          </div>

          <div className="inventory-head" aria-hidden="true">
            <span>Outcome inventory</span><span>Claims</span><span>Bid</span><span>Ask</span><span>Spread</span>
          </div>
          <div className="inventory-rows">
            {inventory.map((row) => (
              <div className="inventory-row" key={row.label}>
                <div><span className={`outcome-swatch swatch-${row.tone}`} /><strong>{row.label}</strong></div>
                <span>{row.claims.toLocaleString()}</span>
                <b>{row.bid}¢</b><b>{row.ask}¢</b><em>{row.ask - row.bid}¢</em>
              </div>
            ))}
          </div>

          <div className="custody-ledger">
            <article><span>LP principal</span><strong>25,000.00 USDC</strong><small>Withdrawable subject to claims</small></article>
            <article><span>Realized fees</span><strong>184.32 USDC</strong><small>Accounted separately</small></article>
            <article><span>Service capital</span><strong>620.00 USDC</strong><small>Prepaid execution liveness</small></article>
            <article><span>Rent custody</span><strong>0.0412 SOL</strong><small>Refund-bound RentCredits</small></article>
          </div>

          <div className="liquidity-proof">
            <div><b>01</b><span><strong>Custodied</strong>Every quote is bounded by pool assets.</span></div>
            <div><b>02</b><span><strong>Fragment-safe fees</strong>Fill splitting does not erase the fee floor.</span></div>
            <div><b>03</b><span><strong>No future-fee liveness</strong>Execution capital is present before activation.</span></div>
          </div>
        </div>

        <aside className="liquidity-ticket" aria-label="Liquidity action preview">
          <div className="ticket-tabs">
            <button className={mode === 'add' ? 'active' : ''} onClick={() => setMode('add')} type="button">Add</button>
            <button className={mode === 'remove' ? 'active' : ''} onClick={() => setMode('remove')} type="button">Remove</button>
          </div>
          <div className="liquidity-ticket-title"><span className="panel-label">LP action</span><h2>{mode === 'add' ? 'Capitalize bounded quotes' : 'Withdraw available custody'}</h2></div>
          <label className="amount-field">
            <span>{mode === 'add' ? 'Collateral contribution' : 'Requested principal'}</span>
            <div><input inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} /><b>USDC</b></div>
          </label>
          <div className="allocation-preview">
            <span className="panel-label">Outcome allocation preview</span>
            {inventory.map((row, index) => (
              <div key={row.label}><span className={`outcome-swatch swatch-${row.tone}`} /><span>{weights[index]}%</span><b>{allocations[index].toLocaleString(undefined, { maximumFractionDigits: 2 })}</b></div>
            ))}
          </div>
          <dl className="ticket-summary">
            <div><dt>Pool shares</dt><dd>Chain-derived at execution</dd></div>
            <div><dt>Price impact</dt><dd>Bounded after observation</dd></div>
            <div><dt>Rent</dt><dd>Separate RentCredit quote</dd></div>
            <div><dt>Authority</dt><dd>Observed LP account</dd></div>
          </dl>
          <button className="connect-action" disabled type="button">Operator wiring in progress</button>
          <p className="ticket-note">This preview never reconstructs an LP identity or submits a transaction.</p>
        </aside>
      </section>

      <footer className="product-footer"><span>dClutch · Dealer liquidity</span><span>Preview data is non-authoritative</span></footer>
    </main>
  );
}
