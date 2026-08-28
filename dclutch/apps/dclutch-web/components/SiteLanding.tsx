import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import PublicDeploymentEvidence from '@/components/PublicDeploymentEvidence';
import LandingPulse from '@/components/charts/LandingPulse';

import { DEVNET_DEPLOYMENT_V1 } from '@/lib/deployments';
import { docsHrefV1, repositoryHrefV1, smokeStoryEnabledV1 } from '@/lib/flags';

/**
 * The front door.
 *
 * This is `/` — the first page anyone who types the domain sees, so it owes
 * them two things before it owes them a control: what dClutch is, in words a
 * reader who has never opened the repository can follow, and the plain fact
 * that its current public deployment is a devnet preview. Both were written
 * hand-authored Pages landing (`tools/genref/render-site.mjs`); this is that
 * same copy, moved into the app because the app is what the domain root now
 * serves. The lifecycle workbench that used to sit here is unchanged and still
 * mounted at `/workbench`, which is where it always was as well.
 *
 * No chain is read here and no address is asked for. Every card is a link.
 */
export default function SiteLanding() {
  return <main className="product-shell trade-v3-shell">
    <Nav current="/" status="live devnet programs" />

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
        <strong>On devnet — nothing for sale</strong>
        <p>dClutch runs on Solana&apos;s devnet, a public test network whose
        tokens are worthless by construction. The programs are deployed and
        the first markets are being set up; you can watch it all happen live
        below. There is no token, nothing to buy, and no value at risk
        anywhere.</p>
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>··</span><div><h2>The protocol, by the numbers</h2><p>Three counts, read finalized off the active deployment — never estimated, never cached from an earlier visit. A dash is an unread value, never a zero; a read zero is shown as the zero it is.</p></div></header>
      {/* FE-CHART mount: LandingPulse reads the counts from the active
          deployment and feeds the presentational NumberStrip. */}
      <LandingPulse />
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>The app</h2><p>It opens on the live devnet deployment and shows you what the chain actually contains. No sample market, no made-up price. The seven programs are live; actions that still need an open market refuse and tell you what is missing.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/markets">Discover markets →</Anchor>
        <Anchor className="secondary-action" href="/create">Preview a Market design →</Anchor>
        <Anchor className="secondary-action" href="/portfolio">Portfolio →</Anchor>
        <Anchor className="secondary-action" href="/explorer">Chain explorer →</Anchor>
        <Anchor className="secondary-action" href="/console">Operator consoles →</Anchor>
      </div>
      <p className="direct-status">You can open the human-readable deployment record or download the exact seven program addresses, ProgramData addresses, and observed deployment slots in one click.</p>
      <PublicDeploymentEvidence deployment={DEVNET_DEPLOYMENT_V1} />
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
