import Link from 'next/link';

/**
 * The three-markets story, written for the reader.
 *
 * Three public test markets, each proving that a different kind of truth can
 * settle a market without anyone's permission. Everything here is a plan a
 * visitor can understand in one read; the exact mechanics live behind the
 * bounty page's "show me the exact bytes" drawer, not in the headline.
 */
export default function SmokeStory() {
  return <main className="product-shell trade-v3-shell">
    <header className="product-nav">
      <Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link>
      <nav>
        <Link href="/markets">Markets</Link>
        <Link href="/portfolio">Portfolio</Link>
        <Link href="/activity">Activity</Link>
        <Link className="active" href="/smoke">The smoke</Link>
        <Link href="/bounty">Bounty</Link>
      </nav>
      <span className="preview-control"><i className="preview-dot" />not live yet</span>
    </header>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">The smoke · three markets, three kinds of truth</p>
        <h1>Can a market settle itself?<br /><em>Three public tests.</em></h1>
        <p>A dClutch market is a promise: put money behind an answer, and when the facts arrive, the right side gets paid — automatically, with nobody in the middle to appeal to or be surprised by. Before anything real runs on this protocol, we will run three small public markets on Solana devnet. Each one tests a different way the facts can arrive.</p>
      </div>
      <aside>
        <span>Where this stands</span>
        <strong>Not live yet</strong>
        <p>None of these markets exist today and nothing is deployed to any network. Everything below has been run end-to-end on local test machines. When the markets go live, this page will link straight to them.</p>
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Oracle truth · the price market</h2><p>Will SOL/USD finish inside a chosen range at a chosen time?</p></div></header>
      <p className="direct-status">The answer comes from Pyth, the same price feed most of Solana already trusts. When the window closes, anyone can submit Pyth&apos;s signed price and the market pays the range it landed in. Nobody decides the outcome — the price does. If Pyth publishes nothing usable in the window, the market falls to a fallback outcome that was named before it opened, so your money is never stuck.</p>
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Cross-chain truth · the graduation market</h2><p>Did this token graduate on Solana mainnet? A market on devnet pays out on it.</p></div></header>
      <p className="direct-status">The event happens on one network; the market lives on another. A disclosed messenger reads mainnet and signs exactly what it saw — the raw bytes, never an interpretation. You are trusting that messenger not to lie, and the market says so up front instead of hiding it. Two things keep it honest: every statement it signs can be checked against mainnet by anyone, forever; and if it goes silent, the market does not hang — it walks to its named fallback.</p>
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>Adversarial truth · the abandoned market</h2><p>We will switch the messenger off on purpose. Then you finish the market and get paid for it.</p></div></header>
      <p className="direct-status">This is the test most markets never dare to run: what happens when everyone responsible walks away? Here, the market has already set money aside for exactly this moment. Once the deadline passes, any wallet — yours — can send one ordinary transaction that closes the market to its pre-announced fallback outcome and collects the posted bounty for doing it. No permission, no account, no special software.</p>
      <div className="direct-actions">
        <Link className="secondary-action" href="/bounty">How to collect the bounty →</Link>
      </div>
    </section>

    <footer className="product-footer">
      <span>Three markets, founded once, run in public, then wound down</span>
      <span>Live dates: none yet — this page will say so when that changes</span>
    </footer>
  </main>;
}
