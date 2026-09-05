import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import { Card, CardContent } from '@/components/ui/card';
import { capabilityAccessSentenceV1, capabilityRouteAccessV1 } from '@dclutch/sdk/capabilityAccess';
import { machineGateCoverageV1, machineGateSentenceV1 } from '@dclutch/sdk/stateMachines';
import {
  capabilitySelectedGateCoverageV1,
  capabilitySelectedGateSentenceV1,
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
  Object.freeze({ stage: 'author', title: 'Author and open', deck: 'Compile the product, check the founding, and open the market.' }),
  Object.freeze({ stage: 'trade', title: 'Trade and clear', deck: 'Offer, take, and clear at the prices participants set.' }),
  Object.freeze({ stage: 'resolve', title: 'Resolve', deck: 'Fund the resolution, admit the outside evidence, and close it out.' }),
  Object.freeze({ stage: 'claim', title: 'Claim and retire', deck: 'Redeem what a finished market owes, and retire what it no longer needs.' }),
]);

/**
 * Two read-only tools that answer a question rather than perform an act.
 *
 * They are not capabilities and deliberately do not appear in the catalogue:
 * neither builds bytes, and calling them acts would put a readiness map beside
 * a redemption. They keep the same three lines so the page reads as one thing.
 */
const SUPPORT_CONSOLES_V1: ReadonlyArray<SupportConsoleV1> = Object.freeze([
  Object.freeze({
    href: '/workbench',
    outcome: 'Read the remaining lifecycle work for one market',
    venue: 'This browser · no key, no signature',
    guarantee: 'Finalized reads only. It produces a readiness map and no transaction.',
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

/**
 * How much of the protocol this page does NOT reach, in its own words.
 *
 * A directory that lists what it has and never says what it lacks reads as a
 * complete one. `docs/evidence/C16_REHEARSAL_2026_09_03.md` measured that gap
 * by hand twice and got two answers an order of magnitude apart; this is the
 * same question asked of the route census, and nothing in the sentence is
 * typed.
 */
const ACCESS_SENTENCE_V1 = capabilityAccessSentenceV1(capabilityRouteAccessV1(BROWSER_CAPABILITY_STANDINGS_V1));
const MACHINE_SENTENCE_V1 = machineGateSentenceV1(
  machineGateCoverageV1(BROWSER_CAPABILITY_STANDINGS_V1.map((standing) => standing.action)),
);
/**
 * The gates the sentence above cannot hold, because they are not on the route.
 *
 * A gate behind a family's classifier binds one act on a route several others
 * declare, so it belongs to neither the phase count nor the machine count. It
 * is computed from the same two tables and the acts' own declared families.
 */
const SELECTED_SENTENCE_V1 = capabilitySelectedGateSentenceV1(
  capabilitySelectedGateCoverageV1(BROWSER_CAPABILITY_STANDINGS_V1.map((standing) => standing.action)),
);

export default function ConsoleDirectory() {
  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/console" status="operator tools" />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Everything dClutch can do, and where each act happens</p>
        <h1>Choose the<br /><em>outcome.</em></h1>
        <p>{LISTED_V1.length} protocol acts are routed below. Each one says what it produces,
        whose authority it asks for, and the one promise it keeps about signing,
        sending, and recovery. Market-participant acts stay on the
        selected <Anchor href="/markets">Market</Anchor>.</p>
        <p>None of these claims is written down. Each is derived from the module
        that builds the act and the route that reaches it, so this page can only
        say what the code does. {WALLED_V1.length} further acts have no venue here
        yet; each names its wall on the <Anchor href="/operate">operations console</Anchor>.
        Artifact inputs name their producer, and the complete provenance table
        is <a href={docsHrefV1('readme.html', 'README.md')}>“The artifacts, and where they come from”</a>.</p>
        <p><strong>And here is what is not on this page.</strong> {ACCESS_SENTENCE_V1} That
        count is the route census’s own: every route a program selects from an
        instruction’s first eight bytes, matched against the acts above and the
        venue each one actually has. It is computed on every render from the
        same tables the cards are, so it cannot drift from them — and it is
        deliberately the harsher of the two readings, because a route some
        module can encode but no act offers is not a capability a person can
        perform.</p>
        <p><strong>And the gates that are not the Market&rsquo;s phase.</strong> {MACHINE_SENTENCE_V1} {SELECTED_SENTENCE_V1} A
        Direct root, a Series ticket, a funding-ledger slot, a projected-custody
        ladder and a Source resolution state are
        separate discriminants in separate accounts, and a Market is
        <em> Open</em> for the whole span in which several of them move. Every
        figure here is computed on each render from the census table and the
        decoders&rsquo; own tag tables, so it says what the code can read and
        never what anyone hoped it could.</p>
      </div>
    </section>

    <section aria-label="Protocol acts by lifecycle stage">
      {STAGE_BANDS_V1.map((band, index) => {
        const standings = standingsForStageV1(band.stage);
        if (standings.length === 0) return null;
        return <Card className="trade-v3-card" key={band.stage}>
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
                  {standing.walls.map((held) => <small key={held.citation} className="console-entry-wall">Known wall · {held.statement}</small>)}
                </span>
                <em aria-hidden="true">→</em>
              </Anchor>;
            })}
          </CardContent>
        </Card>;
      })}
      <Card className="trade-v3-card" key="verify">
        <header><span>{String(STAGE_BANDS_V1.length + 1).padStart(2, '0')}</span><div><h2>Verify the record</h2><p>Compare durable evidence with finalized state.</p></div></header>
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

    <p className="console-stage-note">Stages are the only grouping this page decides. Everything
    else — which acts exist, where each one runs, and what it asks for — comes
    from the capability catalogue and this application&rsquo;s own routes.</p>
  </PageShell>;
}
