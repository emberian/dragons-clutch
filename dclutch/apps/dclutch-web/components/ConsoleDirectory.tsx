'use client';

import { useState } from 'react';
import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { Card, CardContent } from '@/components/ui/card';
import {
  capabilityVenueTextV1,
  type CapabilityStage,
  type CapabilityStandingV1,
} from '@dclutch/sdk/capabilityModel';
import { browserActPrerequisitesV1, BROWSER_CAPABILITY_STANDINGS_V1, capabilityWorkspaceV1 } from '@/lib/capabilitySurface';
import { docsHrefV1 } from '@/lib/flags';

/**
 * `/console` — the directory, generated from what the code can actually do.
 *
 * Every line below except the four stage decks and the two evidence tools is
 * derived: the outcomes are the capability catalogue's own labels, the venue
 * line is computed from the browser's import graph, and an act appears here at
 * all only because a route reaches a module that builds its bytes. There is no
 * second description of any workspace on this page, and no status anyone can
 * type — see `packages/dclutch-sdk/lib/capabilityModel.ts` for why that
 * mattered enough to rebuild.
 *
 * Each card is outcome first, venue and authority second, one safety or
 * recovery guarantee third. An act with a wall carries the wall in the same
 * card as the outcome, because a reader deciding whether to start needs both
 * facts at once; an act with no venue at all is not listed here, because a
 * directory of things you can do is not the place to advertise things you
 * cannot. Those live on `/operate`, with the wall that holds them.
 *
 * Fourth, and last to arrive: what the reader must already hold. The venue
 * line answers what an act DOES, and on that evidence alone this page
 * advertised a redemption as one wallet signature sent from here — true of
 * every clause, and useless to a stranger, because its second step opens a
 * file picker for a payout plan a Rust binary under `tools/local-validator/`
 * is the only thing that can author. A card that says what an act does and
 * not what it cannot be begun without is a card that sends a reader into a
 * dead end politely. Like the rest of the page it is derived, not written:
 * see `browserActPrerequisitesV1`.
 */

type SupportConsoleV1 = Readonly<{
  href: string;
  outcome: string;
  venue: string;
  guarantee: string;
}>;

type StageBandV1 = Readonly<{
  stage: CapabilityStage;
  title: string;
  deck: string;
}>;

/** The only editorial facts on this page: how the lifecycle reads. */
const STAGE_BANDS_V1: ReadonlyArray<StageBandV1> = Object.freeze([
  Object.freeze({ stage: 'author', title: 'Author and open', deck: 'Choose the outcomes, set the rules and open a market.' }),
  Object.freeze({ stage: 'trade', title: 'Trade and clear', deck: 'Offer, take, and clear at the prices participants set.' }),
  Object.freeze({ stage: 'resolve', title: 'Resolve', deck: 'Fund resolution, submit a source report and record the outcome.' }),
  Object.freeze({ stage: 'claim', title: 'Claim and retire', deck: 'Redeem what a finished market owes, and retire what it no longer needs.' }),
]);

/**
 * Tools outside the protocol-act catalogue.
 *
 * The workbench only reads lifecycle state. Bearer transfer is a standard
 * Token-2022 act selected by protocol state rather than a privileged dClutch
 * instruction, so putting it in the protocol catalogue would invent a second
 * semantic owner. Both keep the same three lines so the page reads as one thing.
 */
const SUPPORT_CONSOLES_V1: ReadonlyArray<SupportConsoleV1> = Object.freeze([
  Object.freeze({
    href: '/workbench',
    outcome: 'See a market’s next steps',
    venue: 'Browser · view only',
    guarantee: 'See which actions are available and what they require.',
  }),
  Object.freeze({
    href: '/representation',
    outcome: 'Transfer a bearer claim between Token-2022 accounts',
    venue: 'Browser · wallet signatures required',
    guarantee: 'Choose the recipient, review the transfer and sign with the required wallets.',
  }),
]);

/** Where a reader goes to perform one act. */
function destinationV1(standing: CapabilityStandingV1): string {
  // A market-bound act has no address until a Market is chosen, so the
  // directory hands the reader the market list rather than inventing one.
  return capabilityWorkspaceV1(standing.action, null) ?? '/markets';
}

const LISTED_V1 = Object.freeze(BROWSER_CAPABILITY_STANDINGS_V1.filter((candidate) => candidate.venue !== 'no-venue'));

function standingsForStageV1(stage: CapabilityStage): ReadonlyArray<CapabilityStandingV1> {
  return LISTED_V1.filter((candidate) => candidate.action.stage === stage);
}

/** Acts whose venue is elsewhere, counted rather than hidden. */
const WALLED_V1 = Object.freeze(BROWSER_CAPABILITY_STANDINGS_V1.filter((candidate) => candidate.venue === 'no-venue'));

export default function ConsoleDirectory() {
  const [search, setSearch] = useState('');
  const words = search.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const matches = (standing: CapabilityStandingV1): boolean => {
    const text = [standing.action.action, standing.action.guarantee, capabilityVenueTextV1(standing),
      ...browserActPrerequisitesV1(standing).map((entry) => entry.statement),
      ...standing.walls.map((entry) => entry.statement)].join(' ').toLocaleLowerCase();
    return words.every((word) => text.includes(word));
  };
  const visible = LISTED_V1.filter(matches);
  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/console" status="operator tools" />}>

    <section className="trade-v3-hero hero-solo console-hero">
      <div>
        <p className="eyebrow">The dClutch console</p>
        <h1>What do you<br /><em>want to do?</em></h1>
        <p>Find a tool for each step of a market&rsquo;s life. Each action shows
        what you need, where it runs, and whether it asks for a signature.
        Start trading or redeeming from the <Anchor href="/markets">market list</Anchor>.</p>
      </div>
    </section>

    <nav className="console-stages" aria-label="Market lifecycle">
      {STAGE_BANDS_V1.map((band, index) => <a href={`#console-${band.stage}`} key={band.stage} onClick={() => setSearch('')}>
        <span>{String(index + 1).padStart(2, '0')}</span>
        <strong>{band.title}</strong>
        <small>{standingsForStageV1(band.stage).length} actions</small>
      </a>)}
    </nav>
    <div className="console-find" role="search" aria-label="Find a protocol action">
      <label htmlFor="console-search">Find an action</label>
      <input id="console-search" type="search" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Try redeem, wallet, collateral…" />
      <p role="status">{visible.length} of {LISTED_V1.length} actions</p>
    </div>

    <section aria-label="Protocol acts by lifecycle stage">
      {visible.length > 0 ? null : <div className="console-empty">
        <h2>No action matches that search.</h2>
        <p>Try a different term, or return to the full directory.</p>
        <button className="secondary-action" type="button" onClick={() => setSearch('')}>Show all actions</button>
      </div>}
      {STAGE_BANDS_V1.map((band, index) => {
        const standings = visible.filter((standing) => standing.action.stage === band.stage);
        if (standings.length === 0) return null;
        return <Card className="trade-v3-card console-stage" id={`console-${band.stage}`} key={band.stage}>
          <header><span>{String(index + 1).padStart(2, '0')}</span><div><h2>{band.title}</h2><p>{band.deck}</p></div></header>
          <CardContent className="console-index p-0">
            {standings.map((standing) => {
              const needed = browserActPrerequisitesV1(standing);
              return <Anchor key={standing.action.id} className="console-entry" href={destinationV1(standing)}>
                <strong>{standing.action.action}</strong>
                <span className="console-entry-copy">
                  <b>{capabilityVenueTextV1(standing)}</b>
                  <small>{standing.action.guarantee}</small>
                  {needed.length === 0 ? null : <small className="console-entry-need">
                    Before you start · {needed.map((entry) => entry.statement).join('; and ')}
                  </small>}
                  {standing.walls.map((held) => <small key={held.citation} className="console-entry-wall">Requires · {held.statement}</small>)}
                </span>
                <em aria-hidden="true">→</em>
              </Anchor>;
            })}
          </CardContent>
        </Card>;
      })}
      <Card className="trade-v3-card" key="verify">
        <header><span>{String(STAGE_BANDS_V1.length + 1).padStart(2, '0')}</span><div><h2>Additional tools</h2><p>View market status or transfer a bearer token.</p></div></header>
        <CardContent className="console-index p-0">
          {SUPPORT_CONSOLES_V1.map((support) => <Anchor key={support.href} className="console-entry" href={support.href}>
            <strong>{support.outcome}</strong>
            <span className="console-entry-copy">
              <b>{support.venue}</b>
              <small>{support.guarantee}</small>
            </span>
            <em aria-hidden="true">→</em>
          </Anchor>)}
        </CardContent>
      </Card>
    </section>

    <details className="console-coverage">
      <summary>More tools and file inputs</summary>
      <p>See {WALLED_V1.length} actions still in development on the <Anchor href="/operate">operations page</Anchor>.</p>
      <p>For actions that require a file, the <a href={docsHrefV1('readme.html', 'README.md')}>CLI documentation</a> explains how to create it.</p>
    </details>
  </PageShell>;
}
