'use client';

import { useState } from 'react';
import Link from 'next/link';

const defaultOutcomes = ['Below $120', '$120 – $159.99', '$160 – $199.99', '$200 or above'];

export default function CreateMarketPreview() {
  const [title, setTitle] = useState('Where will SOL/USD settle at 16:00 UTC?');
  const [capabilities, setCapabilities] = useState({ direct: true, dealer: true, bearer: false });

  function toggle(key: keyof typeof capabilities) {
    setCapabilities((current) => ({ ...current, [key]: !current[key] }));
  }

  return (
    <main className="product-shell create-shell">
      <header className="product-nav">
        <Link className="brand" href="/" aria-label="dClutch markets home">
          <span className="brand-mark" aria-hidden="true">dC</span><span>dClutch</span>
        </Link>
        <nav aria-label="Primary navigation">
          <Link href="/">Markets</Link><Link className="active" href="/create">Create</Link><Link href="/explorer">Explorer</Link>
        </nav>
        <div className="preview-control"><span className="preview-dot" /> Interface preview</div>
      </header>

      <div className="preview-banner">
        <span>Creation preview</span>
        This form does not yet compile or submit a market. Fields demonstrate the intended immutable commitments.
        <Link href="/">← Return to market</Link>
      </div>

      <section className="create-heading">
        <div><p className="eyebrow">New bounded-state market</p><h1>Define the claim.<br /><em>Fund the whole lifecycle.</em></h1></div>
        <p>A creation draft becomes admissible only after its Product domain, Source, capabilities, collateral Realm, and exact funding quote agree.</p>
      </section>

      <div className="create-layout">
        <aside className="create-steps" aria-label="Creation steps">
          <div className="active"><b>01</b><span><strong>Claim</strong><small>Question and result domain</small></span></div>
          <div><b>02</b><span><strong>Resolution</strong><small>Provider and release</small></span></div>
          <div><b>03</b><span><strong>Capabilities</strong><small>Trading and representation</small></span></div>
          <div><b>04</b><span><strong>Funding</strong><small>Creation and lifecycle escrow</small></span></div>
          <div><b>05</b><span><strong>Review</strong><small>Canonical identities</small></span></div>
        </aside>

        <section className="create-form" aria-label="Market draft">
          <div className="form-section-heading"><span>01</span><div><h2>Claim and outcomes</h2><p>Every possible result belongs to exactly one ordered state.</p></div></div>
          <label><span>Market question</span><input value={title} onChange={(event) => setTitle(event.target.value)} /></label>
          <div className="field-grid">
            <label><span>Measurement</span><input value="SOL / USD price" readOnly /></label>
            <label><span>Observation time</span><input value="2026-12-31 · 16:00 UTC" readOnly /></label>
          </div>
          <div className="outcome-editor">
            <div className="field-label-row"><span>Ordered result partition</span><small>Exact scaled integers</small></div>
            {defaultOutcomes.map((outcome, index) => <div className="editable-outcome" key={outcome}><b>{String(index + 1).padStart(2, '0')}</b><span className={`outcome-swatch swatch-${index}`} /><input value={outcome} readOnly /><em>{index === 0 ? '(-∞, 120)' : index === 3 ? '[200, +∞)' : index === 1 ? '[120, 160)' : '[160, 200)'}</em></div>)}
            <div className="failure-outcome"><b>F</b><span>Provider failure</span><small>Explicit Product outcome · not silently reassigned</small></div>
          </div>

          <div className="section-divider" />
          <div className="form-section-heading"><span>02</span><div><h2>Resolution contract</h2><p>The Source commits to provider semantics before the Market exists.</p></div></div>
          <div className="resolution-card">
            <div className="provider-mark">P</div><div><small>Provider profile</small><strong>Pyth · SOL/USD</strong><p>Bound observation, freshness, confidence, release identity, recovery, and retirement policy.</p></div><span className="configured-tag">Configured</span>
          </div>
          <div className="field-grid three">
            <label><span>Observation grace</span><input value="15 minutes" readOnly /></label>
            <label><span>Recovery window</span><input value="24 hours" readOnly /></label>
            <label><span>Finality</span><input value="Finalized" readOnly /></label>
          </div>

          <div className="section-divider" />
          <div className="form-section-heading"><span>03</span><div><h2>Optional capabilities</h2><p>Selected children are immutable, canonically identified, and prepaid.</p></div></div>
          <div className="capability-grid">
            <button className={capabilities.direct ? 'capability selected' : 'capability'} onClick={() => toggle('direct')} type="button"><span>Direct</span><strong>Signed intent trading</strong><small>FOK · IOC · registered orders</small><i>{capabilities.direct ? 'Included' : 'Not included'}</i></button>
            <button className={capabilities.dealer ? 'capability selected' : 'capability'} onClick={() => toggle('dealer')} type="button"><span>Dealer</span><strong>Custodied liquidity</strong><small>Inventory-bounded execution</small><i>{capabilities.dealer ? 'Included' : 'Not included'}</i></button>
            <button className={capabilities.bearer ? 'capability selected' : 'capability'} onClick={() => toggle('bearer')} type="button"><span>Bearer</span><strong>Transferable outcomes</strong><small>Token-2022 representation</small><i>{capabilities.bearer ? 'Included' : 'Not included'}</i></button>
          </div>
        </section>

        <aside className="draft-summary">
          <span className="panel-label">Draft topology</span>
          <h2>{title || 'Untitled market'}</h2>
          <div className="summary-path">
            <div><b>Product</b><span>4 price bands + failure</span></div><i />
            <div><b>Source</b><span>Pyth release-bound observation</span></div><i />
            <div><b>Market</b><span>USDC Realm · generation 0</span></div>
          </div>
          <dl className="draft-facts">
            <div><dt>Liabilities</dt><dd>Fully collateralized</dd></div>
            <div><dt>Direct venue</dt><dd>{capabilities.direct ? 'Precommitted' : 'None'}</dd></div>
            <div><dt>Dealer pool</dt><dd>{capabilities.dealer ? 'Precommitted' : 'None'}</dd></div>
            <div><dt>Bearer mint</dt><dd>{capabilities.bearer ? 'Precommitted' : 'None'}</dd></div>
            <div><dt>Funding quote</dt><dd>Awaiting chain Rent</dd></div>
          </dl>
          <button className="compile-action" disabled type="button">Compiler wiring in progress</button>
          <p>No draft identity is claimed until the operator compiles canonical bytes and reads current chain-derived funding.</p>
        </aside>
      </div>

      <footer className="product-footer"><span>dClutch · creation studio</span><span>Preview data is non-authoritative</span></footer>
    </main>
  );
}
