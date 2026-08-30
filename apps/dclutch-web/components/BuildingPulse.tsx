import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import NumberStrip from '@/components/charts/NumberStrip';

import { BUILDING_PULSE_V1 } from '@/lib/buildingPulse';
import { repositoryHrefV1 } from '@/lib/flags';

/**
 * `/building` — what is actively in development, for a stranger.
 *
 * This page exists to be read in thirty seconds and screenshotted whole: the
 * hero block (headline, date stamp, number strip) is composed to stand alone
 * as an image, which is why the stamp names the page's own URL — a screenshot
 * that travels should carry its own way home.
 *
 * Every word comes from `fixtures/building-pulse.json`; this component only
 * arranges. Updating the page when the state of work changes is a fixture
 * edit, the same discipline the front door uses for the public cut. The
 * fixture's parser refuses internal vocabulary and undated content, so the
 * page structurally cannot drift into shorthand or pretend to be current.
 *
 * Honest tense is the contract here: everything below is in development, and
 * the stamp says when it was written. No chain is read — the front page's
 * strip is the live one, and this page says so rather than borrowing its
 * authority.
 */
export default function BuildingPulse() {
  const pulse = BUILDING_PULSE_V1;
  return <main className="product-shell trade-v3-shell building-shell">
    <Nav current="/building" status={`hand-written · ${pulse.updatedDate}`} />

    <section className="building-hero">
      <p className="eyebrow">{pulse.eyebrow}</p>
      <h1>{pulse.headline}</h1>
      <p className="building-lede">{pulse.lede}</p>
      <p className="building-stamp">clutch.dregg.pro/building · written by hand · updated {pulse.updatedDate}, {pulse.updatedTime}</p>
      <NumberStrip stats={pulse.stats.map((stat) => ({ label: stat.label, value: stat.value, detail: stat.detail }))} provenance={pulse.statsProvenance} />
    </section>

    <section className="trade-v3-card">
      <header><span>··</span><div><h2>Happening right now</h2><p>The work in flight at the moment this page was written. When it lags, reality is ahead of it — never behind.</p></div></header>
      <div className="building-now">
        {pulse.now.map((item) => <article key={item.title}>
          <h3><i className="building-live-dot" aria-hidden />{item.title}</h3>
          <p>{item.detail}</p>
        </article>)}
      </div>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>The last thirty hours</h2><p>Firsts, each one run against a real chain and written up in the repository&apos;s evidence ledger before it was allowed on this page.</p></div></header>
      <div className="building-recent">
        {pulse.recent.map((item) => <article key={item.title}>
          <h3>{item.title}</h3>
          <p>{item.detail}</p>
        </article>)}
      </div>
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>The wall ledger</h2><p>{pulse.walls.intro}</p></div></header>
      <div className="building-walls">
        {pulse.walls.entries.map((wall) => <article key={wall.name}>
          <h3>{wall.name}</h3>
          <p>{wall.epitaph}</p>
        </article>)}
      </div>
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>Where to look next</h2><p>{pulse.closing}</p></div></header>
      <div className="direct-actions">
        {pulse.links.map((link) => <Anchor key={link.href} className="secondary-action" href={link.href}>{link.label} →</Anchor>)}
        <a className="secondary-action" href={repositoryHrefV1()}>The code, tests and run logs →</a>
      </div>
    </section>
  </main>;
}
