import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { capabilityAccessSentenceV1, capabilityRouteAccessV1 } from '@dclutch/sdk/capabilityAccess';
import { INSTRUCTION_MAGICS, PREDICATE_SELECTED_ROUTES } from '@dclutch/sdk/generated/routeCensus';
import { magicIsAmbiguousV1 } from '@dclutch/sdk/routeSelector';
import { CAPABILITY_ACTIONS_V1 } from './capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';

/**
 * The user-inaccessible count, recomputed with declarations that were checked.
 *
 * `docs/evidence/C16_REHEARSAL_2026_09_03.md` section 6.2 published **65 of 78
 * strict** and named the thirteen reachable magics by hand. This is the same
 * question asked of instruments the tree regenerates, and the answer moves in
 * both directions at once, which is why the delta is named per capability
 * below rather than summarised.
 *
 * WHY NO FIGURE IS WRITTEN DOWN HERE, learned by this file failing for a
 * reason it was not about. It used to assert `selectable` was 75, `reachable`
 * 6 and `inaccessible` 69. None of those is a claim about capability access:
 * they are one snapshot of a census that is regenerated from the program
 * sources on every run. `DCLTCRQ2` moving into Core's dispatch guard added
 * eleven Core routes to the census -- a change entirely about where a magic
 * check is written, in a program this file names nowhere -- and all three
 * numbers went red at once while every invariant they were standing in for
 * still held.
 *
 * So the figures are derived and the INVARIANTS are asserted:
 *
 *   1. the denominator is exactly the census's own selector tables, read
 *      here from `generated/routeCensus.ts` rather than through the same
 *      `routeSelector.ts` the subject reads, so a selector the derivation
 *      drops or duplicates is red;
 *   2. a route is reachable exactly when an act with a venue declares it;
 *   3. every route an act declares is one the census can select from an
 *      instruction's leading bytes, OR one the census publishes as an
 *      ACTION route below an entry -- which is the whole of what
 *      `declaredOutsideTheDenominator` may hold;
 *   4. the published sentence carries the census's three fields and no
 *      number of its own.
 *
 * The per-capability delta against the rehearsal stays NAMED. A magic is a
 * finding; a count is a consequence.
 */

const census = capabilityRouteAccessV1(BROWSER_CAPABILITY_STANDINGS_V1);

/**
 * Every route id the census's two selector tables carry.
 *
 * Read from the generated tables directly. `capabilityAccess.ts` reads the
 * same rows through `routeSelector.ts`, so the two paths agreeing is a real
 * check on the middle one rather than a restatement of it.
 */
const SELECTABLE_ROUTE_IDS_V1 = new Set(
  [...INSTRUCTION_MAGICS, ...PREDICATE_SELECTED_ROUTES].map((entry) => entry.routeId),
);

/**
 * The kind the census publishes for a route: an `entry`, or an `action` below
 * one.
 *
 * `routes.md` is the census's own publication and the only place the browser
 * can read a route's kind: `INSTRUCTION_MAGICS` carries no `selectors` column,
 * so the TypeScript tables cannot say why a route is unselectable.
 * `capabilityPhaseGate.test.ts` reads the same file the same way.
 */
const ROUTE_KINDS_V1 = new Map(
  [...readFileSync(new URL('../../../docs/reference/routes.md', import.meta.url), 'utf8')
    .matchAll(/^\| `([^`]+)` \| (entry|action) \|/gm)]
    .map(([, route, kind]) => [route, kind] as const),
);

/** The acts a reader of this client can actually reach. */
const VENUED_ACT_IDS_V1 = new Set(
  BROWSER_CAPABILITY_STANDINGS_V1.filter((one) => one.venue !== 'no-venue').map((one) => one.action.id),
);

/** Every route id any act declares, reachable or not. */
const DECLARED_ROUTE_IDS_V1 = new Set(CAPABILITY_ACTIONS_V1.flatMap((one) => one.routes));

/**
 * The thirteen the rehearsal scored reachable, as it wrote them.
 *
 * Its rule was "a client encoder or WASM codec actually constructs the bytes",
 * over any module in the three client trees. Quoted here as a claim to compare
 * against, never as an input to the count.
 */
const REHEARSAL_REACHABLE_MAGICS_V1 = [
  'DCFRRQ03', 'DCLTHOT3', 'DCLTGMF3', 'DCLTGFQ1', 'DCLTPCB2', 'DCLTPUA1', 'DCLTSQ03',
  'DCRRPRQ2', 'DCRRLC02', 'DCLCCR01', 'DCLSDP03', 'DCLRNCI2', 'DCLCUSR1',
] as const;

describe('what a reader of this client can actually reach', () => {
  it('takes its denominator from the census, not from a number written here', () => {
    // A census with an empty denominator would make every ratio below vacuous,
    // and one that quietly dropped a selector would make them wrong without
    // making them empty. Both are the same assertion: these are the census's
    // rows, all of them, once each.
    expect(SELECTABLE_ROUTE_IDS_V1.size).toBeGreaterThan(0);
    expect(new Set(census.rows.map((row) => row.routeId))).toEqual(SELECTABLE_ROUTE_IDS_V1);
    expect(census.selectable).toBe(SELECTABLE_ROUTE_IDS_V1.size);
    expect(census.rows).toHaveLength(census.selectable);
    expect(census.reachable + census.inaccessible).toBe(census.selectable);
  });

  it('counts a route reachable exactly when an act with a venue declares it', () => {
    // Recomputed from the three inputs rather than read back off the subject:
    // the acts, the venues, and the census's selectable set.
    const offered = [...new Set(CAPABILITY_ACTIONS_V1
      .filter((one) => VENUED_ACT_IDS_V1.has(one.id))
      .flatMap((one) => one.routes)
      .filter((route) => SELECTABLE_ROUTE_IDS_V1.has(route)))].sort();
    expect(census.rows.filter((row) => row.reachableActs.length > 0).map((row) => row.routeId)).toEqual(offered);
    expect(census.reachable).toBe(offered.length);
    expect(census.inaccessible).toBe(census.selectable - census.reachable);
  });

  it('names the routes an act offers, Core’s two among them', () => {
    // The list is the finding and stays named: a route entering or leaving
    // reachability is a change a person should have to look at. What is gone
    // is the ratio it used to be pinned beside.
    expect(census.rows.filter((row) => row.reachableActs.length > 0).map((row) => row.routeId)).toEqual([
      'claims/custody_replay_v1::process',
      'claims/terminal_settlement_v3::process',
      'core/execute_provider_v3::process#ExecuteProvider',
      'core/found::process#Found',
      'registry/record_v1::dispatch',
      'resolution/core_effect::process_direct_funding_close_v1',
      'trading/hot_v3::process_hot_execution_v3',
      'trading/user_position_admission_v1::process_user_position_admission_v1',
    ]);
  });

  it('holds every Core route the census selects, which it could hold none of before', () => {
    // Core was outside this denominator entirely while its request magic sat
    // in `Request::decode` instead of the dispatch guard. The rows are read
    // from the generated table, so the day Core's dispatch changes again this
    // agrees with it rather than with a memory of it.
    const coreFromTable = [...new Set([...INSTRUCTION_MAGICS, ...PREDICATE_SELECTED_ROUTES]
      .filter((entry) => entry.program === 'core')
      .map((entry) => entry.routeId))].sort();
    expect(coreFromTable.length).toBeGreaterThan(0);
    expect(census.rows.filter((row) => row.program === 'core').map((row) => row.routeId)).toEqual(coreFromTable);

    // And the caution that comes with them: both reachable Core rows are
    // credited through ONE magic, which selects the other nine as well. The
    // row says an act declares this route; it does not say the bytes prove it,
    // and `capabilityRouteDerivation.test.ts` is where that is settled.
    const reachableCoreMagics = new Set(census.rows
      .filter((row) => row.program === 'core' && row.reachableActs.length > 0)
      .flatMap((row) => row.magics));
    expect([...reachableCoreMagics]).toEqual(['DCLTCRQ2']);
    expect(census.rows.filter((row) => row.magics.includes('DCLTCRQ2')).length)
      .toBeGreaterThan(census.rows.filter((row) => row.magics.includes('DCLTCRQ2') && row.reachableActs.length > 0).length);
  });

  it('publishes a sentence carrying the census’s own three fields', () => {
    // The register prints this. A figure baked into the function instead of
    // read off the census would pass every assertion above and lie on a page.
    expect(capabilityAccessSentenceV1(census)).toBe(
      `${census.reachable} of ${census.selectable} routes a program selects from an instruction’s first eight bytes`
      + ` are reachable from an act on this page; ${census.inaccessible} are not reachable from any client at all.`,
    );
  });
});

describe('every declared route is selectable, or is published as an action below an entry', () => {
  it('reads a kind for more routes than the census can select', () => {
    // The positive control for the parse. An empty or renamed table would make
    // every `toBe('action')` below vacuously unreachable instead of red.
    expect(ROUTE_KINDS_V1.size).toBeGreaterThan(SELECTABLE_ROUTE_IDS_V1.size);
  });

  it('admits no third case: an unselectable declared route is an action route', () => {
    // The invariant the `declaredOutsideTheDenominator` list exists to carry.
    // A declared route that is neither selectable nor published as an action
    // is a route nothing can name at all, and this instrument would have been
    // silently excluding it.
    for (const route of DECLARED_ROUTE_IDS_V1) {
      if (SELECTABLE_ROUTE_IDS_V1.has(route)) continue;
      expect(ROUTE_KINDS_V1.get(route), `${route} is declared, unselectable, and published as no action`).toBe('action');
    }
  });

  it('splits every declared route into counted or named, exhaustively and disjointly', () => {
    const counted = census.rows.filter((row) => row.acts.length > 0).map((row) => row.routeId);
    expect(counted.filter((route) => census.declaredOutsideTheDenominator.includes(route))).toEqual([]);
    expect(new Set([...counted, ...census.declaredOutsideTheDenominator])).toEqual(DECLARED_ROUTE_IDS_V1);
  });

  it('names what no leading-byte count can hold, instead of dropping it', () => {
    // Down from six: `core/found::process#Found` and
    // `core/execute_provider_v3::process#ExecuteProvider` are ENTRY routes and
    // moved into the denominator with the rest of Core. What is left is four
    // routes that sit below an entry, three of which carry the only Market
    // phase gates an act reads.
    expect(census.declaredOutsideTheDenominator).toEqual([
      'core/resolution::process#AdmitTerminal',
      'core/resolution::process#CreateFund',
      'core/resolution::process#VerifyFundReady',
      'trading/user_position_admission_v1::process_user_position_admission_v1#Admit',
    ]);
    for (const route of census.declaredOutsideTheDenominator) expect(ROUTE_KINDS_V1.get(route)).toBe('action');
  });
});

describe('the delta against the rehearsal’s hand count, per capability', () => {
  const magicsHere = new Set(census.rows.flatMap((row) => row.magics));
  const reachableMagics = new Set(census.rows.filter((row) => row.reachableActs.length > 0).flatMap((row) => row.magics));

  it('splits the rehearsal’s thirteen exhaustively, so no magic falls out of the comparison', () => {
    const buckets = REHEARSAL_REACHABLE_MAGICS_V1.map((magic) => (
      !magicsHere.has(magic) ? 'outside' : reachableMagics.has(magic) ? 'reachable' : 'built-but-unoffered'
    ));
    expect(buckets).toHaveLength(REHEARSAL_REACHABLE_MAGICS_V1.length);
    expect(buckets.filter((bucket) => bucket === 'reachable').length).toBeGreaterThan(0);
  });

  it('three arms the rehearsal did not score reachable are, and only one is a new declaration', () => {
    // `DCLTRIX1` is the Registry instruction `/release` has been compiling
    // since it shipped, and `DCLRFCQ1` is the fund closure `/resolution`
    // plans. The rehearsal's search could not see either, because neither act
    // declared a route for a search to confirm.
    //
    // `DCLTCRQ2` is a different kind of gain and the distinction matters: no
    // client gained anything. `/found` has been compiling those exact bytes
    // since it shipped and `market.found` has declared the route all along.
    // What changed is that the CENSUS can now select on the magic, so the
    // route the act declares finally lands inside the denominator.
    const gained = [...reachableMagics].filter((magic) => !REHEARSAL_REACHABLE_MAGICS_V1.includes(magic as never)).sort();
    expect(gained).toEqual(['DCLRFCQ1', 'DCLTCRQ2', 'DCLTRIX1']);

    // Derived, not left to the prose above: the census gain is the only one of
    // the three that names a candidate SET rather than a route. Asked per
    // (program, magic), because a magic on its own is not a key -- `DCLTRIX1`
    // is eight bytes that Registry and Resolution BOTH dispatch on, for two
    // unrelated routes, and counting its routes without a program would call
    // it ambiguous for a reason that has nothing to do with either.
    const ambiguous = gained.filter((magic) => census.rows
      .filter((row) => row.magics.includes(magic) && row.reachableActs.length > 0)
      .some((row) => magicIsAmbiguousV1(row.program, magic)));
    expect(ambiguous).toEqual(['DCLTCRQ2']);
  });

  it('names the arms a client builds that no act on this board offers', () => {
    // The rehearsal's numerator was "some client module encodes these bytes".
    // This one is "an act a reader can find declares it". The difference is
    // not a disagreement: it is the list of routes this browser can build and
    // does not publish, and three of them the rehearsal itself flagged as
    // child or suffix payloads rather than standalone submissions
    // (`DCLSDP03`, `DCLCUSR1`, `DCLRNCI2`).
    const builtButUnoffered = REHEARSAL_REACHABLE_MAGICS_V1
      .filter((magic) => magicsHere.has(magic) && !reachableMagics.has(magic))
      .sort();
    expect(builtButUnoffered).toEqual([
      'DCFRRQ03', 'DCLRNCI2', 'DCLSDP03', 'DCLTGFQ1', 'DCLTPCB2', 'DCRRLC02', 'DCRRPRQ2',
    ]);
  });

  it('names the arms the rehearsal counted that this denominator cannot hold', () => {
    // `DCLTGMF3`'s predicate decodes a whole struct rather than comparing a
    // magic, and `DCLCUSR1` is a CPI-level Custody request no top-level arm
    // selects — so neither is a route an instruction's first eight bytes can
    // name, whoever builds the bytes.
    const outsideHere = REHEARSAL_REACHABLE_MAGICS_V1.filter((magic) => !magicsHere.has(magic)).sort();
    expect(outsideHere).toEqual(['DCLCUSR1', 'DCLTGMF3']);
  });
});
