import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';

import { docsHrefV1, repositoryHrefV1, smokeStoryEnabledV1 } from '@/lib/flags';

/**
 * The front door.
 *
 * This is `/` — the first page anyone who types the domain sees, so it owes
 * them two things before it owes them a control: what dClutch is, in words a
 * reader who has never opened the repository can follow, and the plain fact
 * that none of it is deployed. Both were already written and vetted for the
 * hand-authored Pages landing (`tools/genref/render-site.mjs`); this is that
 * same copy, moved into the app because the app is what the domain root now
 * serves. The lifecycle workbench that used to sit here is unchanged and still
 * mounted at `/workbench`, which is where it always was as well.
 *
 * No chain is read here and no address is asked for. Every card is a link.
 */
export default function SiteLanding() {
  return <main className="product-shell trade-v3-shell">
    <Nav current="/" />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Fully collateralized markets on real-world numbers</p>
        <h1>Buy the answer<br /><em>you believe in.</em></h1>
        <p>dClutch is a Solana protocol for markets on real-world numbers — where
        a price will be at a stated time, for example. You buy claims on the
        outcome you believe in; if you are right, each claim pays out one
        collateral unit. Every claim is fully backed by collateral locked up
        before the claim exists, so there is no leverage, no liquidation, and no
        way to lose more than you paid.</p>
      </div>
      <aside>
        <span>Where this stands</span>
        <strong>Not live yet</strong>
        <p>dClutch is not deployed on any network. There is no live market, no
        token, and nothing to buy today. Everything on this site describes
        software running on a local test chain — the app below reads whatever
        chain you point it at, and with nothing deployed it will refuse, with
        reasons.</p>
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>The app</h2><p>Point it at a chain and it shows you what is actually on that chain. No sample market, no made-up price. Since nothing is deployed yet, expect it to refuse — and to tell you why.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/markets">Discover markets →</Anchor>
        <Anchor className="secondary-action" href="/create">Create a market →</Anchor>
        <Anchor className="secondary-action" href="/portfolio">Portfolio →</Anchor>
        <Anchor className="secondary-action" href="/explorer">Chain explorer →</Anchor>
        <Anchor className="secondary-action" href="/console">Operator consoles →</Anchor>
      </div>
    </section>

    {smokeStoryEnabledV1() && <section className="trade-v3-card">
      <header><span>··</span><div><h2>Three markets, run in public</h2><p>A price market Pyth settles on its own, a devnet market about a real mainnet event, and one we abandon on purpose so you can finish it and collect the bounty.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/smoke">Read the story →</Anchor>
        <Anchor className="secondary-action" href="/bounty">How the bounty works →</Anchor>
      </div>
    </section>}

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>The documentation</h2><p>What a claim is, how protection works, how to run a market, how to build your own client — and how to try the whole thing yourself.</p></div></header>
      <div className="direct-actions">
        <a className="secondary-action" href={docsHrefV1('guides/README.html', 'docs/guides/README.md')}>Guides →</a>
        <a className="secondary-action" href={docsHrefV1('readme.html', 'README.md')}>The README →</a>
        <a className="secondary-action" href={docsHrefV1('reference/refusals.html', 'docs/reference/refusals.md')}>Every error code →</a>
        <a className="secondary-action" href={docsHrefV1('reference/abi/README.html', 'docs/reference/abi/README.md')}>Exact byte layouts →</a>
        <a className="secondary-action" href={docsHrefV1('notices.html', 'tools/sbom/NOTICES.md')}>Third-party notices →</a>
      </div>
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>The code</h2><p>Building on it? The tests and run logs behind every claim on this site live in the repository, beside the programs they were run against.</p></div></header>
      <div className="direct-actions">
        <a className="secondary-action" href={repositoryHrefV1()}>Repository →</a>
      </div>
    </section>
  </main>;
}
