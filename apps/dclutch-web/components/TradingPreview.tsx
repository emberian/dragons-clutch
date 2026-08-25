'use client';

import { useMemo, useState } from 'react';
import Link from 'next/link';

const outcomes = [
  { label: 'Below $120', price: 0.14, move: '+1.8%' },
  { label: '$120 – $159.99', price: 0.27, move: '-0.6%' },
  { label: '$160 – $199.99', price: 0.38, move: '+3.2%' },
  { label: '$200 or above', price: 0.21, move: '-1.1%' },
];

const depth = [42, 61, 78, 55, 88, 66, 49, 73, 93, 70, 58, 81, 65, 47, 57, 40];

export default function TradingPreview() {
  const [selected, setSelected] = useState(2);
  const [side, setSide] = useState<'buy' | 'sell'>('buy');
  const [amount, setAmount] = useState('250');
  const numericAmount = Number(amount) || 0;
  const price = outcomes[selected].price;
  const contracts = useMemo(
    () => (price > 0 ? numericAmount / price : 0),
    [numericAmount, price],
  );

  return (
    <main className="product-shell">
      <header className="product-nav">
        <Link className="brand" href="/" aria-label="dClutch markets home">
          <span className="brand-mark" aria-hidden="true">dC</span>
          <span>dClutch</span>
        </Link>
        <nav aria-label="Primary navigation">
          <Link className="active" href="/">Markets</Link>
          <Link href="/direct">Direct</Link>
          <a href="#positions">Portfolio</a>
          <Link href="/liquidity">Liquidity</Link>
          <Link href="/product-v2">Product V2</Link>
          <Link href="/release">Release</Link>
          <Link href="/local">Local chain</Link>
          <Link href="/explorer">Explorer</Link>
        </nav>
        <div className="preview-control">
          <span className="preview-dot" /> Interface preview
        </div>
      </header>

      <div className="preview-banner">
        <span>Local product preview</span>
        Illustrative state only—no deployed market, wallet connection, or transaction submission.
        <Link href="/create">Preview market creation →</Link>
      </div>

      <section className="market-heading">
        <div>
          <div className="market-kicker"><span>Crypto</span><span>Price bands</span><span>Resolves Dec 31, 2026</span></div>
          <h1>Where will SOL/USD settle at 16:00 UTC?</h1>
          <p>One exhaustive price partition, fully collateralized at creation and resolved from a release-bound Pyth observation.</p>
        </div>
        <div className="market-status">
          <span className="status-live"><i /> Preview</span>
          <strong>$38,420</strong>
          <small>illustrative collateral</small>
        </div>
      </section>

      <section className="trade-grid">
        <div className="market-panel">
          <div className="panel-toolbar">
            <div><span className="panel-label">Probability surface</span><strong>4 mutually exclusive outcomes</strong></div>
            <div className="time-pills"><button type="button">1D</button><button className="selected" type="button">1W</button><button type="button">ALL</button></div>
          </div>

          <div className="depth-chart" aria-label="Illustrative market depth chart">
            <div className="chart-axis"><span>50¢</span><span>40¢</span><span>30¢</span><span>20¢</span><span>10¢</span></div>
            <div className="bars">{depth.map((height, index) => <i key={index} style={{ height: `${height}%` }} />)}</div>
            <span className="chart-watermark">ILLUSTRATIVE</span>
          </div>

          <div className="outcome-list" role="list" aria-label="Market outcomes">
            {outcomes.map((outcome, index) => (
              <button
                className={selected === index ? 'outcome-row selected' : 'outcome-row'}
                key={outcome.label}
                onClick={() => setSelected(index)}
                type="button"
              >
                <span className={`outcome-swatch swatch-${index}`} />
                <span className="outcome-name">{outcome.label}</span>
                <span className={outcome.move.startsWith('+') ? 'outcome-move up' : 'outcome-move'}>{outcome.move}</span>
                <strong>{Math.round(outcome.price * 100)}¢</strong>
              </button>
            ))}
          </div>

          <div className="market-proof">
            <div><span>Partition</span><strong>Exhaustive · disjoint · ordered</strong></div>
            <div><span>Collateral</span><strong>USDC · immutable Realm</strong></div>
            <div><span>Resolution</span><strong>Pyth · release bound</strong></div>
          </div>
        </div>

        <aside className="ticket-panel" aria-label="Order ticket">
          <div className="ticket-tabs">
            <button className={side === 'buy' ? 'active' : ''} onClick={() => setSide('buy')} type="button">Buy</button>
            <button className={side === 'sell' ? 'active' : ''} onClick={() => setSide('sell')} type="button">Sell</button>
          </div>
          <div className="ticket-outcome">
            <span className={`outcome-swatch swatch-${selected}`} />
            <div><small>Selected outcome</small><strong>{outcomes[selected].label}</strong></div>
            <b>{Math.round(price * 100)}¢</b>
          </div>
          <label className="amount-field">
            <span>Max collateral</span>
            <div><input inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} /><b>USDC</b></div>
          </label>
          <div className="quick-amounts">
            {[50, 100, 250, 500].map((value) => <button key={value} onClick={() => setAmount(String(value))} type="button">{value}</button>)}
          </div>
          <dl className="ticket-summary">
            <div><dt>Limit price</dt><dd>{Math.round(price * 100)}¢</dd></div>
            <div><dt>Est. position</dt><dd>{contracts.toLocaleString(undefined, { maximumFractionDigits: 2 })} contracts</dd></div>
            <div><dt>Max loss</dt><dd>{numericAmount.toLocaleString()} USDC</dd></div>
            <div><dt>Order policy</dt><dd>Fill or kill</dd></div>
          </dl>
          <Link className="connect-action" href="/direct">Open real Direct workspace</Link>
          <p className="ticket-note">This illustrative ticket does not transact. The Direct workspace discovers real Markets and constructs exact signing and unsigned transaction bytes.</p>
        </aside>
      </section>

      <section className="product-lower" id="positions">
        <article><span className="panel-label">Your portfolio</span><h2>Positions will be derived from chain custody.</h2><p>No local balance cache will be treated as protocol authority.</p></article>
        <article id="liquidity"><span className="panel-label">Liquidity</span><h2>Quote bounded inventory, not imaginary depth.</h2><p>Preview the Dealer custody, outcome inventory, and separately accounted capital surface.</p><Link className="text-link" href="/liquidity">Open liquidity workspace →</Link></article>
      </section>

      <footer className="product-footer"><span>dClutch · greenfield protocol</span><span>Preview data is non-authoritative</span></footer>
    </main>
  );
}
