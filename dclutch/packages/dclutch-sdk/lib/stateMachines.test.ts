import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import vector from '../fixtures/state-machines.devnet.json';
import { hex as hexOf, sha256 } from './bytes';
import {
  capabilityEntryLedgerMaskV2,
  capabilityFundingLedgerAddressV2,
  capabilityRootAddressV1,
  decodeCapabilityManifestV1,
} from './capabilityManifest';
import {
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_KIND_ID_OFFSET_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_MANIFEST_COUNT_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  CAPABILITY_MANIFEST_PROFILE_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1,
} from './generated/capabilityManifestV1';
import { CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import { CAPABILITY_ROOT_HEADER_BYTES_V1, DIRECT_SUCCESSOR_KIND_ID_V3 } from './generated/directInlineV3';
import {
  ROUTES_GATED_ON_ANOTHER_MACHINE_V1,
  gatedMachinesV1,
  routeMachineStatesV1,
} from './generated/marketPhaseAdmissionV1';
import { STATE_MACHINE_RECORDS_V1, stateMachineRecordV1 } from './generated/stateMachinesV1';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import {
  MARKET_DERIVABLE_MACHINES_V1,
  STATE_MACHINES_V1,
  absentMachineObservationV1,
  acquireMachineObservationsV1,
  decodeDirectRootStateV1,
  decodeFundingLedgerSlotV2,
  decodeMachineStateV1,
  decodeSourceResolutionStateV2,
  machineGateCoverageV1,
  machineGateSentenceV1,
  machineGateVerdictV1,
  machineObservationV1,
  routeMachineVerdictsV1,
  type MachineAccountReaderV1,
  type MachineObservationV1,
  type StateMachineV1,
} from './stateMachines';

/**
 * The eight discriminants that are not the Market's phase, decoded.
 *
 * WHAT THE FIXTURE IS, per machine, because they are not all the same evidence
 * level and pretending they were is the defect this file exists to close.
 *
 *   * `direct-root`, `source` and `funding-ledger` are READ OFF CHAIN.
 *     `fixtures/state-machines.devnet.json` holds the exact finalized bytes of
 *     five cohort-15 accounts at slot 492,837,406, captured with the addresses
 *     and owners they were read at. Those are the three machines that have a
 *     live instance today.
 *   * `dealer-checkpoint`, `dealer-reservation`, `projected-custody`,
 *     `series-ticket` and `dealer-root` have NO on-chain instance on cohort-15,
 *     which was established rather than assumed: a `getProgramAccounts` sweep
 *     of all seven cohort-15 programs for each record's magic returned zero for
 *     each of them. `series-ticket` cannot have one at all today --
 *     `replay.rs:323-345` carries the named debt that nothing dispatched writes
 *     the first valid `TicketStateV3` -- and the projection is closed by the
 *     founding that creates it. Their vectors below are CONSTRUCTED from the
 *     generated table and are labelled as such; what carries them is the
 *     cross-authority check and the hostile cases, not the vector.
 *
 * WHERE THE TEETH ARE. A test that builds a record from the same table it
 * decodes with proves only that the table is self-consistent. Two things here
 * are not that:
 *
 *   1. `the census sets and the decoders agree on every state name`. The state
 *      NAMES come from `routes.md` by way of the Rust `*AdmissionV1` constants
 *      the guards check; the TAGS come from each machine's own hostile decoder.
 *      Those are independent paths out of the Rust, and a set naming a state no
 *      decoder can produce would be a gate no observation could ever satisfy.
 *   2. the chain-read vectors. Nothing in this file computed those bytes.
 */

const hex = (value: string): Uint8Array =>
  Uint8Array.from((value.match(/../g) ?? []).map((byte) => Number.parseInt(byte, 16)));

const chainRecord = (machine: string, address: string): Uint8Array => {
  const found = vector.records.find((record) => record.machine === machine && record.address === address);
  if (found === undefined) throw new Error(`no ${machine} record for ${address} in the devnet vector`);
  return hex(found.recordHex);
};

/** A record built from the generated table alone: canonical header, one tag. */
function constructed(machine: StateMachineV1, state: string): Uint8Array {
  const record = stateMachineRecordV1(machine);
  if (record === null) throw new Error(`${machine} is not a machine`);
  const width = record.bytes ?? (record.headerBytes ?? 0) + (record.rowBytes ?? 0);
  const bytes = new Uint8Array(width);
  bytes.set(new TextEncoder().encode(record.magic), 0);
  const view = new DataView(bytes.buffer);
  for (const [offset, value] of record.header) view.setUint16(offset, value, true);
  const tag = record.states.find((entry) => entry.state === state);
  if (tag === undefined) throw new Error(`${machine} has no state ${state}`);
  bytes[(record.headerBytes ?? 0) + record.tagOffset] = tag.tag;
  return bytes;
}

const decodedState = (machine: StateMachineV1, bytes: Uint8Array, row: number | null = null): string => {
  const decode = decodeMachineStateV1(machine, bytes, row);
  if (decode.status !== 'decoded') throw new Error(`${machine} refused: ${decode.reason}`);
  return decode.state;
};

describe('the state-machine table', () => {
  it('covers every machine the census gates a route on', () => {
    for (const machine of gatedMachinesV1()) {
      expect(STATE_MACHINES_V1, `${machine} gates a route and nothing decodes it`).toContain(machine);
    }
  });

  /**
   * The one check neither authority can pass alone.
   *
   * `routes.md` publishes the sets the Rust guards check; this module's table
   * publishes the bytes the Rust decoders admit. They reach TypeScript through
   * two different generators reading two different halves of the Rust, and a
   * name that appears in one and not the other is a gate that can never be
   * satisfied or a state no route can name. Control: renaming any state in
   * `stateMachinesV1.ts` turns this red naming the route.
   */
  it('agrees with the census sets on every state name', () => {
    let checked = 0;
    for (const entry of ROUTES_GATED_ON_ANOTHER_MACHINE_V1) {
      for (const set of entry.gates) {
        const record = stateMachineRecordV1(set.machine);
        expect(record, `${entry.route} gates on ${set.machine} and nothing decodes it`).not.toBeNull();
        const names = record!.states.map((state) => state.state);
        for (const state of set.states) {
          expect(names, `${entry.route} admits ${set.machine} ${state}, which no decoder produces`).toContain(state);
          checked += 1;
        }
      }
    }
    // A silent zero here would pass every assertion above and check nothing.
    expect(checked).toBeGreaterThan(30);
  });

  it('gives every machine a distinct magic and a non-empty state set', () => {
    const magics = STATE_MACHINE_RECORDS_V1.map((record) => record.magic);
    expect(new Set(magics).size).toBe(magics.length);
    for (const record of STATE_MACHINE_RECORDS_V1) {
      expect(record.magic).toHaveLength(8);
      expect(record.states.length).toBeGreaterThan(0);
      expect(new Set(record.states.map((state) => state.tag)).size).toBe(record.states.length);
    }
  });
});

describe('machines with a live cohort-15 instance', () => {
  /**
   * The Direct root's 24-byte lifecycle tail, PARSED.
   *
   * `directHotChain.ts` pins this account to `232 + 24` bytes and then never
   * reads the 24. Those bytes say the root still admits maker nonces and that
   * no maker replay root is open -- which is exactly what
   * `direct_begin_retiring_v1` and `close_maker_replay_v2` are gated on.
   */
  it('parses the cohort-15 Direct root tail rather than measuring it', () => {
    const tail = chainRecord('direct-root', 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG');
    expect(tail).toHaveLength(24);
    const decode = decodeDirectRootStateV1(tail);
    expect(decode.status).toBe('decoded');
    if (decode.status !== 'decoded') return;
    expect(decode.state).toBe('Open');
    expect(decode.counters.openMakerRootCount).toBe(0n);
  });

  /**
   * Two Sources at one finalized floor, in two different states.
   *
   * A test that only ever saw one state could not tell a decoder from a
   * constant, so both are read and they must DISAGREE.
   */
  it('decodes one unresolved and one resolved Source off the same cohort', () => {
    const primary = chainRecord('source', '5QoawNdiiBtggmeFs81UsejxC5XwWayfPFsswN1redBr');
    const resolved = chainRecord('source', 'JAz42gc4tRTKFEWVzELAHe5tvYUG3SXkQJFVtWrRa5ka');
    expect(decodedState('source', primary)).toBe('Primary');
    expect(decodedState('source', resolved)).toBe('Resolved');
    expect(decodeSourceResolutionStateV2(primary)).not.toEqual(decodeSourceResolutionStateV2(resolved));
  });

  /** Every selected entry of both live ledgers, by row. */
  it('decodes each funding-ledger slot of both live ledgers', () => {
    const trading = chainRecord('funding-ledger', '7c8Y9rTjSPPn9rAcwoQGicfrpJEXhaMafmZn2KXgvjGF');
    const resolution = chainRecord('funding-ledger', '8Vqh8E14hdZgjuV6VFUiwnv2EJPN1HzSugPa8gUvHjoE');
    expect(trading).toHaveLength(48 + 72);
    expect(resolution).toHaveLength(48 + 72 * 3);
    expect(decodedState('funding-ledger', trading, 0)).toBe('Active');
    for (const row of [0, 1, 2]) expect(decodedState('funding-ledger', resolution, row)).toBe('Active');
    // A row past the end is an absence, not a state.
    const past = decodeFundingLedgerSlotV2(trading, 1);
    expect(past.status).toBe('refused');
    if (past.status === 'refused') expect(past.reason).toContain('holds no row 1');
  });
});

describe('machines with no on-chain instance today', () => {
  /**
   * Constructed vectors, said out loud. These five records have no cohort-15
   * account: a `getProgramAccounts` sweep of every cohort-15 program for each
   * magic returned zero. What is proven here is that each machine's own state
   * names round-trip through its own tag offset, and the hostiles below carry
   * the rest.
   */
  it.each(STATE_MACHINE_RECORDS_V1.map((record) => [record.machine] as const))(
    'round-trips every %s state through its own tag offset', (machine) => {
    const record = stateMachineRecordV1(machine);
    expect(record).not.toBeNull();
    for (const state of record!.states) {
      expect(decodedState(machine, constructed(machine, state.state), record!.rowBytes === null ? null : 0))
        .toBe(state.state);
    }
  });

  /**
   * WHAT THE FIXTURES ABOVE CANNOT REFUTE, said rather than left implied.
   *
   * A tag offset is pinned by a chain read only when some live record carries
   * a NONZERO tag there: the `source` pair does -- `JAz42gc4` reads `0x02` at
   * offset 10 and `0x00` at 11, so moving the source offset by one turns the
   * live case red -- and the Direct root does not, because cohort-15's only
   * root is `Open` (tag 0) with five canonical-zero reserved bytes after it.
   * Moving `direct-root`'s tag offset into that run is therefore invisible to
   * every assertion here, and no constructed vector can help: it would write
   * the tag at the moved offset and read it back from the same place.
   *
   * The offset's authority is `DIRECT_ROOT_PHASE_OFFSET_V1` in the Lean
   * emission `generated_successor.rs`, and `abi:state-machines:verify` is what
   * defends it -- the generator refuses to emit anything the emission does not
   * say. That is the gate; this file is not, and claiming otherwise would be
   * the verification theatre the rest of the suite exists to avoid. A live
   * Direct root that has begun retiring would close it, and there is none.
   */
  it('pins the source tag offset against a live record whose tag is nonzero', () => {
    const resolved = chainRecord('source', 'JAz42gc4tRTKFEWVzELAHe5tvYUG3SXkQJFVtWrRa5ka');
    const record = stateMachineRecordV1('source')!;
    expect(resolved[record.tagOffset]).not.toBe(0);
    expect(resolved[record.tagOffset + 1]).toBe(0);
  });
});

describe('a machine record refuses what its Rust decoder refuses', () => {
  it.each(STATE_MACHINE_RECORDS_V1.map((record) => [record.machine] as const))(
    '%s refuses another record\'s magic, a wrong width, a wrong schema word and an unadmitted byte',
    (machine) => {
      const record = stateMachineRecordV1(machine)!;
      const row = record.rowBytes === null ? null : 0;
      const good = constructed(machine, record.states[0]!.state);
      expect(decodeMachineStateV1(machine, good, row).status).toBe('decoded');

      const other = STATE_MACHINE_RECORDS_V1.find((candidate) => candidate.machine !== machine)!;
      const wrongMagic = Uint8Array.from(good);
      wrongMagic.set(new TextEncoder().encode(other.magic), 0);
      const magicRefusal = decodeMachineStateV1(machine, wrongMagic, row);
      expect(magicRefusal.status).toBe('refused');
      if (magicRefusal.status === 'refused') expect(magicRefusal.reason).toContain(other.magic);

      const wrongWidth = decodeMachineStateV1(machine, good.subarray(0, good.length - 1), row);
      expect(wrongWidth.status).toBe('refused');

      const wrongSchema = Uint8Array.from(good);
      const [schemaOffset] = record.header[0]!;
      new DataView(wrongSchema.buffer).setUint16(schemaOffset, 0xbeef, true);
      const schemaRefusal = decodeMachineStateV1(machine, wrongSchema, row);
      expect(schemaRefusal.status).toBe('refused');
      if (schemaRefusal.status === 'refused') expect(schemaRefusal.reason).toContain('schema word');

      // The byte no state of this machine decodes from. `0xff` is outside
      // every one of the eight ranges, which the table itself confirms.
      expect(record.states.some((state) => state.tag === 0xff)).toBe(false);
      const unadmitted = Uint8Array.from(good);
      unadmitted[(record.headerBytes ?? 0) + record.tagOffset] = 0xff;
      const tagRefusal = decodeMachineStateV1(machine, unadmitted, row);
      expect(tagRefusal.status).toBe('refused');
      if (tagRefusal.status === 'refused') {
        expect(tagRefusal.reason).toContain(record.discriminant);
        expect(tagRefusal.reason).toContain('255');
      }
    },
  );

  it('refuses a row on a record that has no rows, and a rowless read of the ledger', () => {
    const direct = chainRecord('direct-root', 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG');
    const rowed = decodeMachineStateV1('direct-root', direct, 0);
    expect(rowed.status).toBe('refused');
    if (rowed.status === 'refused') expect(rowed.reason).toContain('has no rows');

    const ledger = chainRecord('funding-ledger', '7c8Y9rTjSPPn9rAcwoQGicfrpJEXhaMafmZn2KXgvjGF');
    const rowless = decodeMachineStateV1('funding-ledger', ledger, null);
    expect(rowless.status).toBe('refused');
    if (rowless.status === 'refused') expect(rowless.reason).toContain('name the row');
  });

  /**
   * The Direct root tail is the Market's `Open` neighbour and must not decode
   * as one. Both records carry a magic at 0 and a phase near the front, and
   * confusing them is precisely the reading `DirectRootPhaseV1` exists to stop.
   */
  it('does not decode a Direct root tail as any other machine', () => {
    const tail = chainRecord('direct-root', 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG');
    for (const record of STATE_MACHINE_RECORDS_V1) {
      if (record.machine === 'direct-root') continue;
      const decode = decodeMachineStateV1(record.machine, tail, record.rowBytes === null ? null : 0);
      expect(decode.status, `${record.machine} accepted a Direct root tail`).toBe('refused');
    }
  });
});

describe('a route machine gate, answered from an observation', () => {
  const observation = (machine: StateMachineV1, state: string): MachineObservationV1 =>
    machineObservationV1(decodeMachineStateV1(machine, constructed(machine, state)));

  it('admits the Direct root state the route names, off the live tail', () => {
    const tail = chainRecord('direct-root', 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG');
    const observations = [machineObservationV1(decodeDirectRootStateV1(tail))];
    const route = 'trading/direct_token_setup_v1::process_direct_token_setup_v1';
    expect(routeMachineStatesV1(route, 'direct-root')).toEqual(['Open']);
    const verdicts = routeMachineVerdictsV1(route, observations);
    expect(verdicts).toHaveLength(1);
    expect(verdicts[0]!.verdict).toBe('admitted');
    expect(verdicts[0]!.observed).toBe('Open');
  });

  /**
   * The hostile per machine: an act gated on a Direct root phase the root is
   * not in refuses BY THE MACHINE'S NAME.
   *
   * `direct_close_maker_v1` admits `Retiring` alone, and the live root is
   * `Open`, so this is a refusal the chain makes against a real prestate --
   * read off the chain rather than constructed -- and the message names the
   * machine, its admitted set, and what was observed.
   */
  it('refuses by the machine name when the live root is in the other phase', () => {
    const tail = chainRecord('direct-root', 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG');
    const observations = [machineObservationV1(decodeDirectRootStateV1(tail))];
    const route = 'trading/direct_close_maker_v1::process_direct_close_maker_v1';
    expect(routeMachineStatesV1(route, 'direct-root')).toEqual(['Retiring']);
    const [verdict] = routeMachineVerdictsV1(route, observations);
    expect(verdict!.verdict).toBe('excluded');
    expect(verdict!.reason).toContain('direct-root Retiring');
    expect(verdict!.reason).toContain('Open');
  });

  it('refuses a Source capture on a resolved Source and admits it on an unresolved one', () => {
    const route = 'resolution/process_capture#Capture';
    expect(routeMachineStatesV1(route, 'source')).toEqual(['Primary']);
    const primary = machineObservationV1(decodeSourceResolutionStateV2(
      chainRecord('source', '5QoawNdiiBtggmeFs81UsejxC5XwWayfPFsswN1redBr')));
    const resolved = machineObservationV1(decodeSourceResolutionStateV2(
      chainRecord('source', 'JAz42gc4tRTKFEWVzELAHe5tvYUG3SXkQJFVtWrRa5ka')));
    expect(routeMachineVerdictsV1(route, [primary])[0]!.verdict).toBe('admitted');
    expect(routeMachineVerdictsV1(route, [resolved])[0]!.verdict).toBe('excluded');
  });

  it('answers both machines of a route gated on two of them', () => {
    const route = 'core/series_consume::process';
    const verdicts = routeMachineVerdictsV1(route, [
      observation('projected-custody', 'HoardLocked'),
      observation('series-ticket', 'Prepared'),
    ]);
    expect(verdicts.map((verdict) => verdict.machine)).toEqual(['projected-custody', 'series-ticket']);
    expect(verdicts.every((verdict) => verdict.verdict === 'admitted')).toBe(true);

    const half = routeMachineVerdictsV1(route, [observation('projected-custody', 'HoardOpen')]);
    expect(half[0]!.verdict).toBe('excluded');
    expect(half[1]!.verdict).toBe('unobserved');
  });

  /**
   * The narrow case `needs-chain` is now for, and the three answers it must
   * keep apart: never read, read and absent, read and refused.
   */
  it('keeps a missing account, an absent account and refused bytes apart', () => {
    const route = 'trading/direct_token_setup_v1::process_direct_token_setup_v1';
    const missing = routeMachineVerdictsV1(route, [])[0]!;
    expect(missing.verdict).toBe('unobserved');
    expect(missing.reason).toContain('has not been read');

    const absent = routeMachineVerdictsV1(route, [absentMachineObservationV1('direct-root')])[0]!;
    expect(absent.verdict).toBe('unobserved');
    expect(absent.reason).toContain('does not exist');

    const refusedBytes = machineObservationV1(decodeMachineStateV1('direct-root', new Uint8Array(24)));
    const refused = routeMachineVerdictsV1(route, [refusedBytes])[0]!;
    expect(refused.verdict).toBe('unobserved');
    expect(refused.reason).toContain('bytes were refused');
  });

  it('reads nothing at all for a route no machine gates', () => {
    expect(routeMachineVerdictsV1('claims/terminal_settlement_v3::process', [])).toEqual([]);
  });
});

/**
 * The addresses a Market's own header names, and the reads they unlock.
 *
 * WHAT THIS COVERS THAT THE DECODERS DID NOT. Every test above hands a decoder
 * bytes somebody else found. `direct.inline` reported `needs-chain` on every
 * Market ever selected not because its root could not be decoded but because
 * nothing could say WHERE one lives, and the census's own note said seven of
 * the eight machines were unreachable. Two of those seven were not: a Direct
 * root and the Trading funding ledger of its manifest entry are forward
 * projections of the Market's generation and the manifest identity its header
 * commits to.
 *
 * THE MANIFEST HERE IS SYNTHETIC AND THE DECODER IS NOT. The bytes below are
 * built to be exactly what `decodeCapabilityManifestV1` accepts -- one entry,
 * the real Direct successor kind, the canonical `DCLTFQ01` quote -- and the
 * reader is handed them at the address the Registry derivation names for their
 * own hash. So a wrong seed order, a wrong schema, or a record whose bytes do
 * not hash to the identity the Market committed to is a read that finds
 * nothing, which is what the refusal cases assert.
 *
 * `stateMachines.live.test.ts` runs the same derivation against the chain that
 * wrote the real ones.
 */

const DUMMY = (fill: number): string => new PublicKey(new Uint8Array(32).fill(fill)).toBase58();
const MARKET = DUMMY(7);
const REGISTRY = DUMMY(11);
const TRADING = DUMMY(13);
const RESOLUTION = DUMMY(17);
const GENERATION = BigInt(2);

/** The narrowest manifest the canonical decoder accepts, holding one kind. */
function oneEntryManifest(kind: Uint8Array): Uint8Array {
  const bytes = new Uint8Array(CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_BYTES_V1);
  const view = new DataView(bytes.buffer);
  bytes.set(new TextEncoder().encode('DCLTCAP1'), 0);
  view.setUint16(CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1, 1, true);
  view.setUint16(CAPABILITY_MANIFEST_PROFILE_OFFSET_V1, 1, true);
  view.setUint16(CAPABILITY_MANIFEST_COUNT_OFFSET_V1, 1, true);
  const entry = CAPABILITY_MANIFEST_HEADER_BYTES_V1;
  bytes.set(kind, entry + CAPABILITY_ENTRY_KIND_ID_OFFSET_V1);
  for (let identity = 1; identity < 6; identity += 1) {
    bytes.fill(identity + 1, entry + identity * 32, entry + identity * 32 + 32);
  }
  bytes.set(new TextEncoder().encode('DCLTFQ01'), entry + CAPABILITY_ENTRY_QUOTE_OFFSET_V1);
  view.setUint16(entry + CAPABILITY_ENTRY_QUOTE_OFFSET_V1 + 8, 1, true);
  return bytes;
}

type StubAccount = Readonly<{ owner: string; data: Uint8Array }> | null;

function stubReader(accounts: Readonly<Record<string, StubAccount>>): MachineAccountReaderV1 & Readonly<{ reads: ReadonlyArray<string> }> {
  const reads: string[] = [];
  return Object.freeze({
    reads,
    accountInfo: (address: string) => {
      reads.push(address);
      return Promise.resolve(Object.freeze({ account: accounts[address] ?? null }));
    },
  });
}

/** The capability root account shape: the composite header, then the tail. */
function rootAccount(state: string): StubAccount {
  const data = new Uint8Array(CAPABILITY_ROOT_HEADER_BYTES_V1 + 24);
  data.set(constructed('direct-root', state), CAPABILITY_ROOT_HEADER_BYTES_V1);
  return Object.freeze({ owner: TRADING, data });
}

describe('the machines a Market determines the address of', () => {
  const manifestBytes = oneEntryManifest(DIRECT_SUCCESSOR_KIND_ID_V3);
  const market = async () => {
    const manifestId = await sha256(manifestBytes);
    const [entry] = decodeCapabilityManifestV1(manifestBytes);
    return {
      manifestId,
      entry: entry!,
      coordinate: Object.freeze({
        address: MARKET,
        generation: GENERATION,
        capabilityManifestId: hexOf(manifestId),
        registryProgram: REGISTRY,
      }),
      record: deriveFinalizedRecordAddressesV1(REGISTRY, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestId).record,
      root: capabilityRootAddressV1(TRADING, MARKET, GENERATION, manifestId, entry!),
      ledger: capabilityFundingLedgerAddressV2(TRADING, MARKET, GENERATION, manifestId, capabilityEntryLedgerMaskV2(entry!.index)),
    };
  };

  it('reads the Source alone when no Trading program is named', async () => {
    const client = stubReader({});
    const observations = await acquireMachineObservationsV1(client, { address: MARKET, generation: GENERATION }, RESOLUTION);
    expect(observations.map((one) => one.machine)).toEqual(['source']);
    expect(observations[0]!.present).toBe(false);
    // One read, and the coordinate that would have unlocked the others was
    // never supplied -- so nothing about the Direct family was even attempted.
    expect(client.reads).toHaveLength(1);
  });

  /**
   * The whole chain, and the ONE assertion that carries it: the address.
   *
   * Nothing here is handed a root address. It is derived from the Market's
   * generation and the manifest its header commits to, and the test passes
   * only because the stub's account sits at exactly that address.
   */
  it('derives the Direct root and its funding ledger, and answers the gate from them', async () => {
    const { coordinate, record, root, ledger } = await market();
    const client = stubReader({
      [record]: Object.freeze({ owner: REGISTRY, data: manifestBytes }),
      [root]: rootAccount('Open'),
      [ledger]: Object.freeze({ owner: TRADING, data: constructed('funding-ledger', 'Active') }),
    });
    const observations = await acquireMachineObservationsV1(client, coordinate, RESOLUTION, TRADING);
    expect(observations.map((one) => one.machine)).toEqual(['source', 'direct-root', 'funding-ledger']);
    expect(client.reads).toContain(root);
    expect(client.reads).toContain(ledger);

    // The assertion is the AGREEMENT between what decoded and what the census
    // publishes, never a state literal: a set widened to admit everything
    // would fail the disjointness below rather than pass this.
    const setup = routeMachineVerdictsV1('trading/direct_token_setup_v1::process_direct_token_setup_v1', observations)[0]!;
    const close = routeMachineVerdictsV1('trading/direct_close_maker_v1::process_direct_close_maker_v1', observations)[0]!;
    const decoded = observations.find((one) => one.machine === 'direct-root')!.state;
    expect(setup.verdict).toBe(setup.states.includes(decoded!) ? 'admitted' : 'excluded');
    expect([setup.verdict, close.verdict].filter((verdict) => verdict === 'admitted')).toHaveLength(1);

    // RED-PROVEN BY DISABLING THE ROOT READ. The same coordinates with no
    // account at the derived address must lose the admission entirely -- an
    // `unobserved` that names the machine, never the state it used to hold.
    const unactivated = stubReader({ [record]: Object.freeze({ owner: REGISTRY, data: manifestBytes }) });
    const dark = await acquireMachineObservationsV1(unactivated, coordinate, RESOLUTION, TRADING);
    const darkSetup = routeMachineVerdictsV1('trading/direct_token_setup_v1::process_direct_token_setup_v1', dark)[0]!;
    expect(darkSetup.verdict).toBe('unobserved');
    expect(darkSetup.observed).toBeNull();
    expect(darkSetup.reason).toContain('direct-root');
  });

  /**
   * A Market founded and never activated, which is the ordinary case.
   *
   * The root does not exist yet; the manifest that names it does. `Open` is
   * the state this must never report, because the account it would have read
   * has not been written -- and a card reading READY TO PREFLIGHT off a state
   * nobody holds is exactly the defect the family gate was added to remove.
   */
  it('reports a Market with no activation root as absent, never as Open', async () => {
    const { coordinate, record, root: rootAddress, ledger } = await market();
    const client = stubReader({ [record]: Object.freeze({ owner: REGISTRY, data: manifestBytes }) });
    const observations = await acquireMachineObservationsV1(client, coordinate, RESOLUTION, TRADING);
    // "Read and not there" and "never read" are the same word on a card and
    // different facts, so the reads are asserted as well as the answer.
    expect(client.reads).toContain(rootAddress);
    expect(client.reads).toContain(ledger);
    const root = observations.find((one) => one.machine === 'direct-root')!;
    expect(root.present).toBe(false);
    expect(root.state).toBeNull();
    expect(root.refusal).toBeNull();
    const verdict = machineGateVerdictV1('direct-root', ['Open'], observations, 'a classifier');
    expect(verdict.verdict).toBe('unobserved');
    expect(verdict.reason).toContain('does not exist');
  });

  /**
   * A record that is not the manifest the Market committed to is not a
   * manifest, and neither address below it may be published.
   */
  it('refuses both Direct machines when the manifest record does not authenticate', async () => {
    const { coordinate, record, root } = await market();
    const substituted = oneEntryManifest(new Uint8Array(32).fill(9));
    const client = stubReader({
      [record]: Object.freeze({ owner: REGISTRY, data: substituted }),
      [root]: rootAccount('Open'),
    });
    const observations = await acquireMachineObservationsV1(client, coordinate, RESOLUTION, TRADING);
    for (const machine of ['direct-root', 'funding-ledger'] as const) {
      const one = observations.find((candidate) => candidate.machine === machine)!;
      expect(one.state).toBeNull();
      expect(one.refusal).toContain('does not hash to the capability manifest identity');
    }
    // And the substituted record's root was never read, whatever sat there.
    expect(client.reads).not.toContain(root);
  });

  /**
   * A coordinate this reader was handed wrong is not a machine's failure.
   *
   * The Source is derived from the Market and the Resolution program and has
   * nothing to do with the Trading program; a throw on the Direct half that
   * took the Source observation with it would report a machine as failing when
   * the reader's own input did.
   */
  it('keeps a bad Direct coordinate off the Source observation', async () => {
    const client = stubReader({});
    const observations = await acquireMachineObservationsV1(client, Object.freeze({
      address: MARKET, generation: GENERATION, capabilityManifestId: 'not hex', registryProgram: REGISTRY,
    }), RESOLUTION, TRADING);
    expect(observations[0]!.machine).toBe('source');
    expect(observations[0]!.present).toBe(false);
    expect(observations[0]!.refusal).toBeNull();
    for (const machine of ['direct-root', 'funding-ledger'] as const) {
      const one = observations.find((candidate) => candidate.machine === machine)!;
      expect(one.refusal).toContain('could not be addressed from this Market');
    }
  });

  /** A Market whose manifest founded no Direct capability has no such machine. */
  it('says nothing about a Direct root a Market never founded', async () => {
    const other = oneEntryManifest(new Uint8Array(32).fill(5));
    const manifestId = await sha256(other);
    const { record } = deriveFinalizedRecordAddressesV1(REGISTRY, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestId);
    const client = stubReader({ [record]: Object.freeze({ owner: REGISTRY, data: other }) });
    const observations = await acquireMachineObservationsV1(client, Object.freeze({
      address: MARKET, generation: GENERATION, capabilityManifestId: hexOf(manifestId), registryProgram: REGISTRY,
    }), RESOLUTION, TRADING);
    expect(observations.map((one) => one.machine)).toEqual(['source']);
  });

  /**
   * `MARKET_DERIVABLE_MACHINES_V1`, pinned to what the acquisition does.
   *
   * The console prints this list's length as the machines /workbench can go
   * and read. A list is the only honest shape for it -- no table knows that a
   * Direct root's address is a projection of a manifest entry -- so it is
   * pinned rather than asserted: a machine the acquisition speaks about and
   * this list omits is red here, and a name here that no read ever produces is
   * red too.
   */
  it('speaks about exactly the machines it claims a Market determines', async () => {
    const { coordinate, record, root, ledger } = await market();
    const client = stubReader({
      [record]: Object.freeze({ owner: REGISTRY, data: manifestBytes }),
      [root]: rootAccount('Open'),
      [ledger]: Object.freeze({ owner: TRADING, data: constructed('funding-ledger', 'Active') }),
    });
    const observations = await acquireMachineObservationsV1(client, coordinate, RESOLUTION, TRADING);
    expect([...observations.map((one) => one.machine)].sort())
      .toEqual([...MARKET_DERIVABLE_MACHINES_V1].sort());
    // And each of them was reached by a read at a DERIVED address, which is
    // the claim the list is making. An observation manufactured without one
    // would satisfy the set above and fail here.
    for (const address of [root, ledger]) expect(client.reads).toContain(address);
    // The complement is the claim's other half: every machine NOT in the list
    // is one this acquisition never produces, so a gate over it stays honestly
    // unobserved.
    const rest = STATE_MACHINES_V1.filter((machine) => !MARKET_DERIVABLE_MACHINES_V1.includes(machine));
    expect(rest).toHaveLength(STATE_MACHINES_V1.length - MARKET_DERIVABLE_MACHINES_V1.length);
    for (const machine of rest) expect(observations.some((one) => one.machine === machine)).toBe(false);
  });

  /** The coverage sentence counts what can be READ, not only what decodes. */
  it('counts the derivable machines separately from the decodable ones', () => {
    const coverage = machineGateCoverageV1([]);
    expect(coverage.derivable.length).toBeLessThan(coverage.decodable.length);
    for (const machine of coverage.derivable) {
      expect(coverage.decodable).toContain(machine);
      expect(MARKET_DERIVABLE_MACHINES_V1).toContain(machine as StateMachineV1);
    }
    expect(machineGateSentenceV1(coverage)).toContain(coverage.derivable.join(', '));
  });
});
