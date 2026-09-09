'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import FeaturedMarketStanding from '@/components/FeaturedMarketStanding';
import PublicDeploymentEvidence from '@/components/PublicDeploymentEvidence';
import LandingPulse from '@/components/charts/LandingPulse';

import { useDeploymentV1 } from '@/lib/deploymentStore';
import { docsHrefV1, repositoryHrefV1, smokeStoryEnabledV1 } from '@/lib/flags';

/**
 * The long-form field notes, served as a plain page beside the app.
 *
 * The file is a byte-identical copy of the piece written on 25 August 2026;
 * it carries its own complete styling and loads nothing from anywhere, so the
 * artifact serves it directly at this path instead of re-typesetting it. The
 * trailing slash is load-bearing: the export's directory-index rule serves
 * `<dir>/index.html` for it, and the artifact's link check resolves it.
 */
export const FIELD_NOTES_HREF_V1 = '/notes/plan-to-compost-at-least-three/';

/**
 * The front door.
 *
 * Explain the protocol and link to its existing market and design tools.
 * This component is the domain root in the GitHub Pages export.
 *
 * Market counts and the featured record use the selected deployment through
 * their existing readers. The evidence beside them must use that selection too.
 */
export default function SiteLanding() {
  const deployment = useDeploymentV1();
  return <PageShell className="product-shell trade-v3-shell landing" header={<Nav current="/" status={`${deployment.label} preview`} />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Fully collateralized markets on Solana</p>
        <h1>Markets with<br /><em>fixed payouts.</em></h1>
        <p>Trade claims on future prices and outcomes. Each market defines
        which result wins and what a winning claim pays.</p>
        <p>Collateral backs every claim before it is issued. The source,
        outcome boundaries and payout rules are fixed before the market opens.</p>
        <div className="landing-actions">
          <Anchor className="secondary-action landing-primary" href="/markets">Explore markets →</Anchor>
          <Anchor className="secondary-action" href="/create">Design a market →</Anchor>
        </div>
      </div>
      {/* The featured record is read through the existing deployment reader. */}
      <aside>
        <span>Try dClutch</span>
        <strong>{deployment.cluster === 'devnet' ? 'Devnet preview · test tokens' : `${deployment.label} deployment selected`}</strong>
        <p>{deployment.cluster === 'devnet'
          ? <>Use test tokens to try a market. Get free devnet SOL from the <a href="https://faucet.solana.com" rel="noreferrer">public faucet</a>.</>
          : <>Browse markets on your selected {deployment.label.toLowerCase()} deployment.</>}</p>
        <p><FeaturedMarketStanding /></p>
      </aside>
    </section>

    <section className="trade-v3-card landing-observation">
      <header><span>↗</span><div><h2>Market activity</h2><p>{deployment.label} markets.</p></div><Anchor href="/pulse">Watch activity →</Anchor></header>
      <LandingPulse />
    </section>

    {/* The key art: the one image on the site, and it is the thesis — a
        claw holding a faceted gem the way every market holds the collateral
        that backs its claims. Serves the webp cut (a tenth the bytes); the
        PNG master lives beside it in public/art/ for anyone who wants it. */}
    <figure className="landing-key-art">
      {/* eslint-disable-next-line @next/next/no-img-element -- the static
          export has no image optimizer; the webp IS the optimized cut. */}
      <img
        src="/art/dragons-clutch-key-art-v1-1672w.webp"
        alt="A dragon's claw cradling a glowing, faceted gem against a dark field."
        width={1672}
        height={941}
        loading="lazy"
      />
    </figure>

    <section className="landing-explanation" aria-label="How a market works">
      <div><p className="eyebrow">From a question to a claim</p><h2>One question.<br />A payout for each outcome.</h2>
        <a href={docsHrefV1('guides/reader.html', 'docs/guides/reader.md')}>Read a worked example →</a></div>
      <ol>
        <li><span>01</span><div><h3>Define the possibilities</h3><p>Will SOL be below $90, from $90 to under $110, or at least $110? A market sets these ranges and names the price source and observation time.</p></div></li>
        <li><span>02</span><div><h3>Back the claims</h3><p>One claim on each outcome makes a complete set, backed by one collateral unit. You can trade individual claims while that collateral stays in the market.</p></div></li>
        <li><span>03</span><div><h3>Resolve and redeem</h3><p>The source price selects the winning range. In this example, each winning claim pays one collateral unit; the others pay zero. The market also sets a failure outcome if its source stays unavailable.</p></div></li>
      </ol>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Choose an action</h2><p>Browse market terms, view your holdings or design a market.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/markets">Browse the markets →</Anchor>
        <Anchor className="secondary-action" href="/portfolio">View your holdings →</Anchor>
        <Anchor className="secondary-action" href="/create">Design a market →</Anchor>
        <Anchor className="secondary-action" href="/pulse">Watch activity →</Anchor>
        <Anchor className="secondary-action" href="/explorer">Inspect an account →</Anchor>
        <Anchor className="secondary-action" href="/console">Operator tools →</Anchor>
      </div>
      <details className="landing-evidence"><summary>Deployment record and program addresses</summary>
        <PublicDeploymentEvidence deployment={deployment} />
      </details>
    </section>

    {smokeStoryEnabledV1() && <section className="trade-v3-card">
      <header><span>··</span><div><h2>Three markets, run in public</h2><p>Follow a Pyth price market, a mainnet-event market and a market with a funded failure bounty.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/smoke">Read the story →</Anchor>
        <Anchor className="secondary-action" href="/bounty">How the bounty works →</Anchor>
      </div>
    </section>}

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>The documentation</h2><p>Learn how to trade claims, create a market or build a client.</p></div></header>
      <div className="direct-actions">
        <a className="secondary-action" href={docsHrefV1('guides/README.html', 'docs/guides/README.md')}>Guides →</a>
        <a className="secondary-action" href={docsHrefV1('readme.html', 'README.md')}>Project overview →</a>
        <a className="secondary-action" href={docsHrefV1('reference/refusals.html', 'docs/reference/refusals.md')}>Error codes →</a>
        <a className="secondary-action" href={docsHrefV1('reference/abi/README.html', 'docs/reference/abi/README.md')}>Account layouts →</a>
        <a className="secondary-action" href={docsHrefV1('notices.html', 'tools/sbom/NOTICES.md')}>Third-party notices →</a>
      </div>
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>The code</h2><p>Programs, tests, and run logs.</p></div></header>
      <div className="direct-actions">
        <a className="secondary-action" href={repositoryHrefV1()}>Repository →</a>
      </div>
    </section>

    {/* The field notes. Written 25 August 2026 and committed the same day to a
        separate posters repository, where nothing on this site linked to them
        and no reader could find them. The copy served here is byte-identical
        to that original (dregg-posters b15ca11) — a self-contained page with
        no external font, script, or image, so it is served as-is rather than
        rebuilt into this app's chrome. */}
    <section className="trade-v3-card">
      <header><span>04</span><div><h2>Project history</h2><p>The ideas and decisions behind dClutch.</p></div></header>
      <div className="direct-actions">
        <a className="secondary-action" href={FIELD_NOTES_HREF_V1}>Plan to compost at least three →</a>
      </div>
    </section>
  </PageShell>;
}
