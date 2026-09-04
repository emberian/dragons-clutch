import { PublicKey } from '@solana/web3.js';

import { u16, u64 } from './bytes';
import { ROUTES_GATED_ON_ANOTHER_MACHINE_V1, routeOtherMachineGateV1 } from './generated/marketPhaseAdmissionV1';
import {
  STATE_MACHINE_RECORDS_V1,
  type StateMachineRecordV1,
  type StateMachineV1,
  stateMachineRecordV1,
} from './generated/stateMachinesV1';

export type { StateMachineV1, StateMachineRecordV1 } from './generated/stateMachinesV1';

/**
 * The eight persisted discriminants that are NOT the Core Market's phase.
 *
 * WHAT WAS MISSING. `marketPhaseAdmissionV1.ts` has published, per route, the
 * machines a gate is over that the Market's phase cannot answer -- a Direct
 * root's lifecycle, a Series ticket's replay phase, a funding-ledger slot, a
 * projected-custody ladder, a Dealer scenario checkpoint, a Source's
 * resolution state -- and no client surface could decode ANY of them. The
 * Direct root's 24-byte lifecycle tail was width-checked and never parsed
 * (`directHotChain.ts` pins `root.data.length` to `232 + 24` and then never
 * reads those 24 bytes); the ticket, checkpoint and projection had no decoder
 * anywhere; the Source had one, for a superseded record. So every act driving
 * such a route answered `needs-chain` FOREVER: the census had read a gate and
 * the reader had no instrument, and "I cannot observe this" is indistinguishable
 * from "this account does not exist" to anyone reading the card.
 *
 * WHAT IS HERE. One decoder per machine, over the generated table in
 * `generated/stateMachinesV1.ts`, which is emitted from each machine's own
 * Rust hostile decoder. Nothing in this file states a magic, a width, an
 * offset or a tag: it looks them up, so a phase added to a Rust enum reaches
 * this decoder by regenerating and a phase REMOVED from one turns a byte that
 * used to decode into a named refusal.
 *
 * WHAT A DECODE IS NOT. Admission. Every one of these states is a NECESSARY
 * condition on some route and never a sufficient one; `capabilityModel.ts`
 * turns an observation into a verdict, and even an admitted verdict has every
 * account, rent, release and derivation check ahead of it.
 *
 * WHERE THE RECORD ENDS AND THE ACCOUNT BEGINS. Three of these records are not
 * whole accounts. The Direct root's and the Dealer root's tails follow the
 * composite capability-root header, whose width belongs to
 * `generated/directInlineV3.ts`; the funding ledger's slot is one row of a
 * variable-width account. So these take the RECORD bytes, and the caller that
 * holds an account slices it with that module's constant -- one fact, one
 * owner, and no second copy of 232 here.
 */

/** One machine's decoded state, or the reason its bytes were refused. */
export type MachineDecodeV1 =
  | Readonly<{
      machine: StateMachineV1;
      status: 'decoded';
      /** The state name its own Rust enum gives this byte. */
      state: string;
      tag: number;
      /** Other u64 lifecycle facts the record carries, by field name. */
      counters: Readonly<Record<string, bigint>>;
    }>
  | Readonly<{ machine: StateMachineV1; status: 'refused'; reason: string }>;

/**
 * One machine as a verdict can consume it.
 *
 * `state` is `null` in exactly two situations, and they are different answers
 * a reader must be able to tell apart: `present: false` means the account was
 * read and is not there, which for several of these machines is the ordinary
 * end of a lifecycle; `present: true` with a null state means bytes were found
 * and refused, which is a defect somewhere and never an admission.
 */
export type MachineObservationV1 = Readonly<{
  machine: StateMachineV1;
  present: boolean;
  state: string | null;
  /** Why the bytes were refused, when they were. */
  refusal: string | null;
}>;

/** Every machine this module can decode. */
export const STATE_MACHINES_V1: ReadonlyArray<StateMachineV1> =
  Object.freeze(STATE_MACHINE_RECORDS_V1.map((record) => record.machine));

function requireRecord(machine: StateMachineV1): StateMachineRecordV1 {
  const record = stateMachineRecordV1(machine);
  if (record === null) throw new Error(`${machine} is not a persisted state machine`);
  return record;
}

const ASCII = new TextDecoder('ascii', { fatal: true });

/**
 * Decode one machine's discriminant out of its own record's bytes.
 *
 * `row` selects one funding-ledger slot and is refused for every other
 * machine, because a row index on a record that has no rows is a caller that
 * believes it is reading something else.
 *
 * The checks are the record's own, in the order its Rust decoder runs them:
 * exact width, magic, every pinned header word, then the tag. A byte the Rust
 * decoder refuses is refused here by the same description rather than rendered
 * as an unknown number, because an unknown number on a card reads as a state.
 */
export function decodeMachineStateV1(
  machine: StateMachineV1,
  bytes: Uint8Array,
  row: number | null = null,
): MachineDecodeV1 {
  const record = requireRecord(machine);
  const refuse = (reason: string): MachineDecodeV1 => Object.freeze({ machine, status: 'refused' as const, reason });

  let base = 0;
  if (record.rowBytes === null || record.headerBytes === null) {
    if (row !== null) return refuse(`${record.record} has no rows and a row was named`);
    if (record.bytes === null) return refuse(`${record.record} declares neither an exact width nor rows`);
    if (bytes.length !== record.bytes) {
      return refuse(`${record.record} is exactly ${record.bytes} bytes and ${bytes.length} were read`);
    }
  } else {
    if (row === null) return refuse(`${record.record} holds one slot per selected manifest entry; name the row`);
    if (!Number.isSafeInteger(row) || row < 0) return refuse('a funding-ledger row is a non-negative integer');
    base = record.headerBytes + record.rowBytes * row;
    if (bytes.length < base + record.rowBytes) {
      return refuse(`${record.record} holds no row ${row} in ${bytes.length} bytes`);
    }
    if ((bytes.length - record.headerBytes) % record.rowBytes !== 0) {
      return refuse(`${record.record} is not a header and a whole number of ${record.rowBytes}-byte slots`);
    }
  }

  if (bytes.length < 8) return refuse(`${record.record} carries no magic in ${bytes.length} bytes`);
  let observed: string;
  try {
    observed = ASCII.decode(bytes.subarray(0, 8));
  } catch {
    return refuse(`${record.record} does not open with printable magic`);
  }
  if (observed !== record.magic) {
    return refuse(`${record.record} opens with ${observed} and not ${record.magic}`);
  }
  for (const [offset, value] of record.header) {
    const word = u16(bytes, offset);
    if (word !== value) return refuse(`${record.record} declares schema word ${word} at ${offset} and not ${value}`);
  }

  const tag = bytes[base + record.tagOffset];
  if (tag === undefined) return refuse(`${record.record} has no byte at its own tag offset`);
  const state = record.states.find((entry) => entry.tag === tag) ?? null;
  if (state === null) {
    return refuse(`${record.discriminant} admits no state for byte ${tag}`);
  }
  const counters: Record<string, bigint> = {};
  for (const counter of record.counters) counters[counter.field] = u64(bytes, base + counter.offset);
  return Object.freeze({
    machine,
    status: 'decoded' as const,
    state: state.state,
    tag,
    counters: Object.freeze(counters),
  });
}

/** One decode as an observation, so a refusal and an absence stay distinct. */
export function machineObservationV1(decode: MachineDecodeV1): MachineObservationV1 {
  return decode.status === 'decoded'
    ? Object.freeze({ machine: decode.machine, present: true, state: decode.state, refusal: null })
    : Object.freeze({ machine: decode.machine, present: true, state: null, refusal: decode.reason });
}

/** The observation for a machine whose account was read and is not there. */
export function absentMachineObservationV1(machine: StateMachineV1): MachineObservationV1 {
  return Object.freeze({ machine, present: false, state: null, refusal: null });
}

/**
 * The Direct root's mutable lifecycle tail, parsed.
 *
 * `DirectRootStateV1` is the 24 bytes that follow the composite capability
 * root's 232-byte header. `require_closable` is the conjunction of the phase
 * and this count, so a reader that took only the phase would still call a
 * refused global close ready -- which is why the count travels with it.
 *
 * `Open` here is NOT the Market's `Open`. A Core Market is `Open` for the
 * entire span in which its Direct root moves `Open` to `Retiring` and drains
 * its maker replay accounts, and Direct's retirement runs on a Market that is
 * still trading. That is the whole reason this discriminant has its own type.
 */
export function decodeDirectRootStateV1(tail: Uint8Array): MachineDecodeV1 {
  return decodeMachineStateV1('direct-root', tail);
}

/** The Dealer capability root's own lifecycle tail. */
export function decodeDealerRootStateV1(tail: Uint8Array): MachineDecodeV1 {
  return decodeMachineStateV1('dealer-root', tail);
}

/** One durable Dealer scenario preparation checkpoint. */
export function decodeDealerCheckpointStateV1(bytes: Uint8Array): MachineDecodeV1 {
  return decodeMachineStateV1('dealer-checkpoint', bytes);
}

/** One per-effect Dealer scenario reservation. */
export function decodeDealerReservationStateV1(bytes: Uint8Array): MachineDecodeV1 {
  return decodeMachineStateV1('dealer-reservation', bytes);
}

/** One occurrence ticket's mutable replay state. */
export function decodeSeriesTicketStateV3(bytes: Uint8Array): MachineDecodeV1 {
  return decodeMachineStateV1('series-ticket', bytes);
}

/**
 * One funding-ledger slot's status.
 *
 * The only per-ROW machine of the eight: one ledger account holds a slot for
 * every selected manifest entry, and each walks `Pending` to `Active` to
 * `Closed` independently. `row` is the ledger row for the manifest entry, not
 * the entry index -- the ledger packs only the SELECTED entries, so the two
 * differ whenever the selection is not the whole manifest.
 */
export function decodeFundingLedgerSlotV2(bytes: Uint8Array, row: number): MachineDecodeV1 {
  return decodeMachineStateV1('funding-ledger', bytes, row);
}

/** One projected-custody ladder's phase. */
export function decodeProjectedCustodyStateV2(bytes: Uint8Array): MachineDecodeV1 {
  return decodeMachineStateV1('projected-custody', bytes);
}

/**
 * One Source's resolution state.
 *
 * This decodes `SourceResolutionStateV2` (`DCLTSRS2`), which is what the
 * resolution-core operator reads off the chain. `localSuccessor.ts` carries a
 * decoder for the superseded `DCLTSRS1` record at a different field map; that
 * one is a different account shape and not this machine's current authority.
 */
export function decodeSourceResolutionStateV2(bytes: Uint8Array): MachineDecodeV1 {
  return decodeMachineStateV1('source', bytes);
}

/**
 * One machine gate on one route, answered against what has been observed.
 *
 * `excluded` is a refusal the chain makes before any account is read, and it
 * is published with the machine NAMED: "the Direct root is Retiring and this
 * route admits only Open" is actionable, where "gated on a state machine this
 * observation does not read" was only ever an apology.
 *
 * `unobserved` is the honest degradation and it is now the NARROW case rather
 * than the only one. It means this reader has no observation of that machine
 * at all -- the account was never read, or its bytes were refused -- and it is
 * never an admission.
 */
export type MachineGateVerdictV1 = Readonly<{
  machine: StateMachineV1;
  /** The states this route's own guard admits, from the census's table. */
  states: ReadonlyArray<string>;
  /** The state that was read, or `null` when none was. */
  observed: string | null;
  verdict: 'admitted' | 'excluded' | 'unobserved';
  reason: string;
}>;

/** The observation for one machine, or `null` when the reader holds none. */
export function machineObservationForV1(
  machine: string,
  observations: ReadonlyArray<MachineObservationV1>,
): MachineObservationV1 | null {
  return observations.find((observation) => observation.machine === machine) ?? null;
}

/**
 * Every machine gate one census route carries, answered from observations.
 *
 * The route's sets come from `ROUTES_GATED_ON_ANOTHER_MACHINE_V1`, which is
 * generated from `routes.md`, which is generated from the Rust admission
 * constants the guards check. The state NAMES on both sides therefore have two
 * independent authorities -- the admission constants and each machine's own
 * hostile decoder -- and `stateMachines.test.ts` asserts they agree, so a set
 * naming a state no decoder can produce is red rather than permanently
 * unsatisfiable.
 */
export function routeMachineVerdictsV1(
  route: string,
  observations: ReadonlyArray<MachineObservationV1>,
): ReadonlyArray<MachineGateVerdictV1> {
  const gate = routeOtherMachineGateV1(route);
  if (gate === null) return Object.freeze([]);
  return Object.freeze(gate.gates.map(
    (set) => machineGateVerdictV1(set.machine as StateMachineV1, set.states, observations, route),
  ));
}

/**
 * One machine's admissible set, answered against what has been observed.
 *
 * `enforcedBy` is what the reason NAMES, and it is not always a route. A gate
 * every execution of a route passes is enforced by the route; a gate behind a
 * family's classifier is enforced by that classifier, and saying "`route`
 * admits only direct-root Open" of a gate three quarters of the route's
 * callers never reach would be false in exactly the direction the selection
 * tables exist to prevent. So the caller names the thing that enforces its
 * gate, and the sentence a card prints says which one it was.
 */
export function machineGateVerdictV1(
  machine: StateMachineV1,
  states: ReadonlyArray<string>,
  observations: ReadonlyArray<MachineObservationV1>,
  enforcedBy: string,
): MachineGateVerdictV1 {
  const observation = machineObservationForV1(machine, observations);
  if (observation === null || !observation.present || observation.state === null) {
    const reason = observation === null
      ? `the ${machine} account has not been read at this observation`
      : observation.present
        ? `the ${machine} account was read and its bytes were refused: ${observation.refusal ?? 'no reason given'}`
        : `the ${machine} account was read at this observation and does not exist`;
    return Object.freeze({ machine, states, observed: null, verdict: 'unobserved' as const, reason });
  }
  const admitted = states.includes(observation.state);
  return Object.freeze({
    machine,
    states,
    observed: observation.state,
    verdict: admitted ? ('admitted' as const) : ('excluded' as const),
    reason: admitted
      ? `the ${machine} is ${observation.state}, which \`${enforcedBy}\` admits`
      : `\`${enforcedBy}\` admits only ${machine} ${states.join(' or ')}; this one is ${observation.state}`,
  });
}

/**
 * The Source resolution state's account address for one Market generation.
 *
 * The only machine of the eight whose account a reader holding a Market can
 * name. Seven of the others are reached through a route manifest or a request
 * a client is not holding, which is exactly why `unobserved` remains a real
 * answer rather than a bug.
 *
 * The seed ORDER is not a constant and could not be emitted, so
 * `generate-state-machines-v1.mjs` pins the one Rust expression that states it
 * (`resolution-core-v3-operator/src/lib.rs`) and refuses to emit when it moves
 * -- the same guard `generate-direct-participant-v1.mjs` puts on the Direct
 * token PDA. The domain itself is Lean-emitted and looked up here.
 */
export function sourceResolutionStateAddressV2(
  market: PublicKey | string,
  generation: bigint,
  resolutionProgram: PublicKey | string,
): string {
  const record = requireRecord('source');
  if (record.pdaDomain === null) throw new Error('the Source state publishes no PDA domain');
  if (generation < 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('a Market generation is a u64');
  const seed = new Uint8Array(8);
  new DataView(seed.buffer).setBigUint64(0, generation, true);
  const [address] = PublicKey.findProgramAddressSync(
    [new TextEncoder().encode(record.pdaDomain), new PublicKey(market).toBytes(), seed],
    new PublicKey(resolutionProgram),
  );
  return address.toBase58();
}

/** The one account read this module needs, named structurally. */
export type MachineAccountReaderV1 = Readonly<{
  accountInfo(address: string): Promise<Readonly<{ account: Readonly<{ owner: string; data: Uint8Array }> | null }>>;
}>;

/**
 * Every machine this reader can observe for one Market, at one floor.
 *
 * WHICH IS ONE, and saying so is the point. Seven of the eight machines live
 * at addresses a Market does not determine -- a Direct root and a Dealer root
 * come out of a route manifest, a checkpoint and a reservation out of a
 * request, a ticket out of an occurrence, a funding ledger out of a manifest
 * selection -- so a surface holding only a Market coordinate genuinely cannot
 * read them, and `unobserved` is the truth rather than a gap in this function.
 * The Source state is the exception and it is read here.
 *
 * An account that is not there comes back `present: false` rather than as a
 * throw: a Market whose resolution fund was never created has no Source state,
 * and that is an ordinary observation about a Market, not a failure to observe.
 */
export async function acquireMachineObservationsV1(
  client: MachineAccountReaderV1,
  market: Readonly<{ address: string; generation: bigint }>,
  resolutionProgram: string,
): Promise<ReadonlyArray<MachineObservationV1>> {
  const address = sourceResolutionStateAddressV2(market.address, market.generation, resolutionProgram);
  const observation = await client.accountInfo(address);
  const account = observation.account;
  if (account === null) return Object.freeze([absentMachineObservationV1('source')]);
  if (account.owner !== resolutionProgram) {
    return Object.freeze([Object.freeze({
      machine: 'source' as const,
      present: true,
      state: null,
      refusal: `the Source state at ${address} is owned by ${account.owner} and not the Resolution program`,
    })]);
  }
  return Object.freeze([machineObservationV1(decodeSourceResolutionStateV2(account.data))]);
}

/**
 * What the machine gates reach, counted rather than described.
 *
 * The number a reader of `/console` needs and could not get: how much of the
 * census's other-machine surface any act on the board actually drives. It is
 * computed from the two generated tables on every render, so it cannot drift
 * from the cards beside it, and there is no figure typed anywhere for a
 * `--check` twin to compare.
 */
export type MachineGateCoverageV1 = Readonly<{
  /** Every machine the census gates at least one route on. */
  machines: ReadonlyArray<string>;
  /** Of those, the machines this client can decode. */
  decodable: ReadonlyArray<string>;
  /** Census routes gated on a machine that is not the Market's phase. */
  gatedRoutes: number;
  /** Distinct routes the acts declare, once each. */
  declaredRoutes: number;
  /** Declared routes that are ALSO gated on another machine. */
  intersection: ReadonlyArray<string>;
}>;

export function machineGateCoverageV1(
  acts: ReadonlyArray<Readonly<{ routes: ReadonlyArray<string> }>>,
): MachineGateCoverageV1 {
  const machines: string[] = [];
  for (const entry of ROUTES_GATED_ON_ANOTHER_MACHINE_V1) {
    for (const machine of entry.machines) if (!machines.includes(machine)) machines.push(machine);
  }
  machines.sort();
  const declared = [...new Set(acts.flatMap((act) => [...act.routes]))].sort();
  const gated = new Set(ROUTES_GATED_ON_ANOTHER_MACHINE_V1.map((entry) => entry.route));
  return Object.freeze({
    machines: Object.freeze(machines),
    decodable: Object.freeze(machines.filter((machine) => stateMachineRecordV1(machine) !== null)),
    gatedRoutes: gated.size,
    declaredRoutes: declared.length,
    intersection: Object.freeze(declared.filter((route) => gated.has(route))),
  });
}

/**
 * That coverage as one sentence, with the empty case said out loud.
 *
 * An empty intersection is the finding, not a blank: it means every machine
 * gate the census publishes is over a route no act on the board drives, so the
 * decoders below can answer a question the cards above have not yet asked. A
 * page that rendered nothing here would leave a reader to infer that from an
 * absence, which is the reading this whole surface exists to remove.
 *
 * WHAT IT DOES NOT SAY, since the sentence beside it now does. This counts the
 * gates a ROUTE carries. It used to end "no card here is yet answered by a
 * machine", which was true only while that was the sole way a card could be:
 * a gate behind a family's classifier binds one act on a route several others
 * declare, is in neither table this counts, and IS answered on a card. So the
 * clause is scoped to what this coverage measures, and
 * `capabilitySelectedGateSentenceV1` states the rest.
 */
export function machineGateSentenceV1(coverage: MachineGateCoverageV1): string {
  const reach = `${coverage.gatedRoutes} census routes are gated on a state machine that is not the Market's phase, over ${coverage.machines.length} machines, and this client decodes ${coverage.decodable.length} of them (${coverage.decodable.join(', ')}).`;
  return coverage.intersection.length === 0
    ? `${reach} None of the ${coverage.declaredRoutes} routes the acts above declare is one of them, so no card here is yet answered by a gate the route itself carries.`
    : `${reach} ${coverage.intersection.length} of the ${coverage.declaredRoutes} routes the acts above declare is one of them: ${coverage.intersection.join(', ')}.`;
}
