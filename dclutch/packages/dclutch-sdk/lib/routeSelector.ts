/**
 * Which census route a compiled instruction would reach, from its own bytes.
 *
 * WHAT THIS REPLACES. Every route id a capability act declares was typed by
 * hand into `capabilityModel.ts`, checked only against the list of ids
 * `routes.md` publishes — so a name that exists passed, whatever the act's
 * builder actually emitted. That check cannot see the two failures that
 * matter: an act that declares a route its transaction never reaches, and an
 * act that reaches a route it declares nothing about. Both were present.
 *
 * WHAT NAMES A ROUTE. A program dispatches on the first eight bytes of the
 * instruction data, either inline (`INSTRUCTION_MAGICS`) or inside a
 * `fn is_x(instruction_data) -> bool` the census records as a predicate and
 * `PREDICATE_SELECTED_ROUTES` resolves to the magic it compares. So the pair
 * (program, leading eight bytes) is the key, and it is the census's own key:
 * nothing here states a magic, a route id, or a binding between them.
 *
 * WHAT THIS CANNOT NAME, said out loud because the alternative is a consumer
 * reading an empty answer as "no route".
 *
 *   * Core dispatches on a decoded `Action` variant, not on its request magic
 *     — `DCLTCRQ2` appears in no census selector at all. Every Core route an
 *     act drives is therefore invisible to this derivation, including the four
 *     that carry the only Market phase gates a browser acts on today.
 *   * Where one magic selects several routes the answer is the whole candidate
 *     set: Rent's `DCLRNCI2` names Create, Close and Sweep, separated by a
 *     decoded variant this has no offset for. Reported as a set rather than
 *     narrowed by a guess.
 *   * `UNRESOLVED_PREDICATE_ARMS_V1` holds the arms that compare no leading
 *     magic at all (`GenericMarketFoundingCallerBumpsV3::decode(..).is_ok()`).
 *     No eight-byte view will ever name those.
 *
 * A caller that needs "did this act reach a route it did not declare" gets a
 * sound answer for every program but Core; a caller that needs "is this the
 * only route it reached" must read the empty cases above first.
 */

import {
  INSTRUCTION_MAGICS,
  PREDICATE_SELECTED_ROUTES,
  UNRESOLVED_PREDICATE_ARMS_V1,
} from './generated/routeCensus';

/** How the census names this route from an instruction's leading bytes. */
export type RouteSelectionViaV1 = 'magic' | 'predicate';

/** One route an instruction's program and leading eight bytes select. */
export type SelectedRouteV1 = Readonly<{
  routeId: string;
  program: string;
  magic: string;
  via: RouteSelectionViaV1;
  /** The Rust the census read the binding out of. */
  provenance: string;
}>;

/**
 * One protocol instruction, described only by what decides its route.
 *
 * `program` is the census's own program label (`trading`, `claims`, `core`),
 * never an address: an address is a cohort's fact and this is the protocol's.
 */
export type CompiledProtocolInstructionV1 = Readonly<{
  program: string;
  data: Uint8Array;
}>;

/** Every route any instruction's leading eight bytes can select. */
export const LEADING_BYTE_SELECTED_ROUTES_V1: ReadonlyArray<SelectedRouteV1> = Object.freeze([
  ...INSTRUCTION_MAGICS.map((entry) => Object.freeze({
    routeId: entry.routeId,
    program: entry.program,
    magic: entry.magic,
    via: 'magic' as const,
    provenance: entry.provenance,
  })),
  ...PREDICATE_SELECTED_ROUTES.map((entry) => Object.freeze({
    routeId: entry.routeId,
    program: entry.program,
    magic: entry.magic,
    via: 'predicate' as const,
    provenance: entry.provenance,
  })),
]);

/**
 * The eight-byte magic at the head of an instruction, or `null`.
 *
 * Every magic this protocol allocates is eight printable ASCII characters, so
 * a leading span that is not is not a magic and is reported as no magic rather
 * than as a magic nothing matches — the two are different findings.
 */
export function instructionMagicV1(data: Uint8Array): string | null {
  if (data.length < 8) return null;
  let magic = '';
  for (let index = 0; index < 8; index += 1) {
    const byte = data[index]!;
    if (byte < 0x20 || byte > 0x7e) return null;
    magic += String.fromCharCode(byte);
  }
  return magic;
}

/** Every route one program's dispatch selects for this magic. */
export function censusRoutesForMagicV1(program: string, magic: string): ReadonlyArray<SelectedRouteV1> {
  return Object.freeze(LEADING_BYTE_SELECTED_ROUTES_V1.filter(
    (entry) => entry.program === program && entry.magic === magic,
  ));
}

/**
 * Every route this instruction's program and leading bytes select.
 *
 * Empty means the census reads no leading-byte selector for these bytes in
 * this program. That is never "no route": see the header — Core's whole
 * request family lands here.
 */
export function censusRoutesForInstructionV1(
  instruction: CompiledProtocolInstructionV1,
): ReadonlyArray<SelectedRouteV1> {
  const magic = instructionMagicV1(instruction.data);
  return magic === null ? Object.freeze([]) : censusRoutesForMagicV1(instruction.program, magic);
}

/** The route ids a whole compiled instruction sequence selects, deduplicated. */
export function censusRouteIdsForInstructionsV1(
  instructions: ReadonlyArray<CompiledProtocolInstructionV1>,
): ReadonlyArray<string> {
  const ids: string[] = [];
  for (const instruction of instructions) {
    for (const selected of censusRoutesForInstructionV1(instruction)) {
      if (!ids.includes(selected.routeId)) ids.push(selected.routeId);
    }
  }
  return Object.freeze([...ids].sort());
}

/**
 * Whether one magic selects more than one route in one program.
 *
 * A consumer that must name exactly one route has to say so when it cannot,
 * and this is the question it asks first.
 */
export function magicIsAmbiguousV1(program: string, magic: string): boolean {
  const routes = new Set(censusRoutesForMagicV1(program, magic).map((entry) => entry.routeId));
  return routes.size > 1;
}

/** Programs whose dispatch this derivation can name a route in at all. */
export function programsWithALeadingByteSelectorV1(): ReadonlyArray<string> {
  return Object.freeze([...new Set(LEADING_BYTE_SELECTED_ROUTES_V1.map((entry) => entry.program))].sort());
}

/**
 * Dispatch arms no leading-byte view can ever name, carried straight through.
 *
 * Re-exported rather than restated so a consumer reads the census's own
 * sentence about why an arm is missing.
 */
export const UNNAMEABLE_DISPATCH_ARMS_V1 = UNRESOLVED_PREDICATE_ARMS_V1;
