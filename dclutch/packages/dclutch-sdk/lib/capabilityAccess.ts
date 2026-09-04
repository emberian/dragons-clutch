/**
 * How much of the protocol a person holding this client can actually reach.
 *
 * WHAT THIS REPLACES. `docs/evidence/C16_REHEARSAL_2026_09_03.md` measured
 * **65 of 78 capabilities user-inaccessible, strict**, by hand: it read every
 * `process_instruction` for its dispatch arms, then searched the client trees
 * for each arm's magic in ASCII, `0x..`-list and decimal-list spellings. That
 * is a real measurement and it is not one anybody can re-run, so it aged the
 * moment it was written -- and the figure it corrected (19 of 20, 2026-09-01)
 * had been wrong by an order of magnitude for exactly that reason: it grepped
 * for magics as strings, which TypeScript never writes.
 *
 * THE DENOMINATOR IS NOW THE CENSUS'S OWN. `LEADING_BYTE_SELECTED_ROUTES_V1`
 * is every route a program selects from an instruction's first eight bytes,
 * across both dispatch styles -- the arms that compare the bytes inline and
 * the arms that compare them inside a predicate. That is the same population
 * the rehearsal counted by hand (it found 78 arms / 77 magics; the census
 * finds 75 routes over 72 program-and-magic keys), and it is regenerated with
 * the tree.
 *
 * THE NUMERATOR IS NARROWER THAN THE REHEARSAL'S, on purpose. The rehearsal
 * scored a route reachable when ANY client module encoded its bytes. That
 * counts a route a module can build but no published act offers -- and a
 * capability nobody can find is not a capability a person can perform. Here a
 * route is reachable only when a CAPABILITY ACT declares it and that act has a
 * venue: some page, command or constructor a reader can actually reach. The
 * difference between the two numerators is itself a finding, and
 * `capabilityAccess.test.ts` names it per route.
 *
 * WHAT IS OUTSIDE THE DENOMINATOR ENTIRELY, said out loud because a count with
 * a silent exclusion is the defect this file exists to retire. Core dispatches
 * on a decoded `Action` variant, so its whole request family -- including the
 * four routes that carry the only Market phase gates an act reads today -- can
 * never appear here. `declaredOutsideTheDenominator` lists exactly which
 * declared routes that costs, rather than dropping them.
 */

import { CAPABILITY_ACTIONS_V1, type CapabilityStandingV1 } from './capabilityModel';
import { LEADING_BYTE_SELECTED_ROUTES_V1 } from './routeSelector';

/** One selectable route, and which acts (if any) offer it. */
export type RouteAccessV1 = Readonly<{
  routeId: string;
  program: string;
  /** Every magic that selects it; one arm may admit several. */
  magics: ReadonlyArray<string>;
  /** Acts declaring it, whether they can be performed or not. */
  acts: ReadonlyArray<string>;
  /** Acts declaring it that a reader can actually reach. */
  reachableActs: ReadonlyArray<string>;
}>;

export type CapabilityAccessCensusV1 = Readonly<{
  /** Routes an instruction's leading eight bytes can select. */
  selectable: number;
  /** Of those, the ones a reachable act declares. */
  reachable: number;
  /** The count this client publishes: `selectable - reachable`. */
  inaccessible: number;
  rows: ReadonlyArray<RouteAccessV1>;
  /**
   * Declared routes no leading-byte view can name, so no count can hold them.
   *
   * Every one is a Core `Action` route or an action route below an entry. They
   * are not inaccessible and they are not counted; they are the part of the
   * catalogue this instrument cannot see, listed so that nobody reads the
   * denominator as the protocol.
   */
  declaredOutsideTheDenominator: ReadonlyArray<string>;
}>;

/**
 * The access census, over one client's own standings.
 *
 * `standings` is injected for the same reason the capability surface is: the
 * SDK owns what an act IS, and one application owns what it routes.
 */
export function capabilityRouteAccessV1(
  standings: ReadonlyArray<CapabilityStandingV1>,
): CapabilityAccessCensusV1 {
  const reachableActs = new Set(standings.filter((one) => one.venue !== 'no-venue').map((one) => one.action.id));
  const declaringActs = new Map<string, string[]>();
  for (const act of CAPABILITY_ACTIONS_V1) {
    for (const route of act.routes) {
      declaringActs.set(route, [...(declaringActs.get(route) ?? []), act.id]);
    }
  }

  const magicsByRoute = new Map<string, string[]>();
  const programByRoute = new Map<string, string>();
  for (const entry of LEADING_BYTE_SELECTED_ROUTES_V1) {
    const magics = magicsByRoute.get(entry.routeId) ?? [];
    if (!magics.includes(entry.magic)) magics.push(entry.magic);
    magicsByRoute.set(entry.routeId, magics);
    programByRoute.set(entry.routeId, entry.program);
  }

  const rows = [...magicsByRoute.keys()].sort().map((routeId) => {
    const acts = declaringActs.get(routeId) ?? [];
    return Object.freeze({
      routeId,
      program: programByRoute.get(routeId)!,
      magics: Object.freeze([...magicsByRoute.get(routeId)!].sort()),
      acts: Object.freeze([...acts]),
      reachableActs: Object.freeze(acts.filter((id) => reachableActs.has(id))),
    });
  });
  const reachable = rows.filter((row) => row.reachableActs.length > 0).length;

  return Object.freeze({
    selectable: rows.length,
    reachable,
    inaccessible: rows.length - reachable,
    rows: Object.freeze(rows),
    declaredOutsideTheDenominator: Object.freeze(
      [...declaringActs.keys()].filter((route) => !magicsByRoute.has(route)).sort(),
    ),
  });
}

/** The one sentence a register prints, with no number typed into it. */
export function capabilityAccessSentenceV1(census: CapabilityAccessCensusV1): string {
  return `${census.reachable} of ${census.selectable} routes a program selects from an instruction’s first eight bytes are reachable from an act on this page; ${census.inaccessible} are not reachable from any client at all.`;
}
