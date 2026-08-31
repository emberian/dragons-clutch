import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import PublicDeploymentEvidence from '@/components/PublicDeploymentEvidence';
import LandingPulse from '@/components/charts/LandingPulse';

import { DEVNET_DEPLOYMENT_V1 } from '@/lib/deployments';
import { docsHrefV1, repositoryHrefV1, smokeStoryEnabledV1 } from '@/lib/flags';
import { marketEditorialV1 } from '@/lib/marketRegistry';
import { PUBLIC_DEVNET_CUT_V1, publicCutMarketHrefV1 } from '@/lib/publicCutStaging';

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
  // The featured market's editorial name, when the shipped registry has one.
  // The cut names the address; the registry names the market. Either can be
  // absent and the sentence below stays true without it.
  const featuredTitle = PUBLIC_DEVNET_CUT_V1.market === null
    ? null
    : marketEditorialV1(PUBLIC_DEVNET_CUT_V1.market)?.title ?? null;
  return <main className="product-shell trade-v3-shell">
    <Nav current="/" status="live devnet programs" />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Prediction markets on Solana, fully backed by collateral</p>
        <h1>Buy the answer<br /><em>you believe in.</em></h1>
        <p>Pick an outcome — where the SOL price lands on Friday, say — and buy
        claims on it. If you are right, each claim pays you one unit of
        collateral. If you are wrong, it pays nothing.</p>
        <p>Every claim is backed by collateral locked up before the claim
        exists. So there is nothing borrowed, nothing to be liquidated, and no
        way to lose more than you paid.</p>
      </div>
      {/* This aside is the one thing on the page that dates, so it reads the
          same public cut the launch page does rather than hard-coding a
          moment. Opening a market is a fixture edit; the front door should not
          need a second one to stop saying markets are still being set up. */}
      <aside>
        <span>Where this stands</span>
        <strong>On devnet — nothing for sale</strong>
        <p>dClutch runs on Solana&apos;s devnet, a public test network whose
        tokens are worthless by construction. The programs are deployed{' '}
        {PUBLIC_DEVNET_CUT_V1.market === null
          ? <>and the first markets are being set up.</>
          : <>and the first market is <Anchor href={publicCutMarketHrefV1(PUBLIC_DEVNET_CUT_V1)}>open{featuredTitle === null ? '' : ` — ${featuredTitle}`}</Anchor>.</>} There
        is no token, nothing to buy, and no value at risk anywhere. If you want
        to try it, devnet SOL is free from the{' '}
        <a href="https://faucet.solana.com" rel="noreferrer">public faucet</a>.</p>
      </aside>
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
        alt="A dragon's claw cradling a glowing, faceted gem against a dark field — the dClutch key art."
        width={1672}
        height={941}
        loading="lazy"
      />
      <figcaption>The clutch: every claim fully backed by collateral the market holds like treasure, paid out on the answer.</figcaption>
    </figure>

    <section className="trade-v3-card">
      <header><span>··</span><div><h2>What is out there right now</h2></div></header>
      {/* FE-CHART mount: LandingPulse reads the counts from the active
          deployment and feeds the presentational NumberStrip. */}
      <LandingPulse />
    </section>

    <section className="trade-v3-card">
      {/* The second dated sentence on this page, and it dated the same way the
          aside did: it went on saying no market was open after one was. It
          reads the same published cut, so opening a market is still one
          fixture edit and the front door still stops claiming otherwise. */}
      <header><span>01</span><div><h2>Try it</h2><p>Seven programs, deployed on devnet.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/markets">Browse the markets →</Anchor>
        <Anchor className="secondary-action" href="/portfolio">See what a wallet holds →</Anchor>
        <Anchor className="secondary-action" href="/create">Design a market →</Anchor>
        <Anchor className="secondary-action" href="/live">Watch a market being built →</Anchor>
        <Anchor className="secondary-action" href="/explorer">Look up any account →</Anchor>
        <Anchor className="secondary-action" href="/console">Operator tools →</Anchor>
      </div>
      <p className="direct-status">Every program address and the slot it was deployed at:</p>
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

    {/* The field notes. Written 25 August 2026 and committed the same day to a
        separate posters repository, where nothing on this site linked to them
        and no reader could find them. The copy served here is byte-identical
        to that original (dregg-posters b15ca11) — a self-contained page with
        no external font, script, or image, so it is served as-is rather than
        rebuilt into this app's chrome. */}
    <section className="trade-v3-card">
      <header><span>04</span><div><h2>How this was built</h2><p>We built this protocol twice before the version you are reading now, and threw both away on purpose. These notes say why that was the plan from the start, and what survived each time the code did not.</p></div></header>
      <div className="direct-actions">
        <a className="secondary-action" href={FIELD_NOTES_HREF_V1}>Plan to compost at least three →</a>
      </div>
    </section>
  </main>;
}
