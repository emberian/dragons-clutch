import { describe, expect, it } from 'vitest';

import { STATE_MACHINE_RECORDS_V1 } from '@dclutch/sdk/generated/stateMachinesV1';

import vector from '@dclutch/sdk/fixtures/state-machines.devnet.json';
import { CAPABILITY_ROOT_HEADER_BYTES_V1, CAPABILITY_ROOT_MAGIC_V1 } from '@dclutch/sdk/generated/directInlineV3';
import {
  decodeAgainstSpec,
  magicText,
  specForData,
  specForMagic,
  trailingRecordForData,
} from './accountRecords';

/**
 * The eight persisted discriminants, as the explorer renders them.
 *
 * WHAT WAS WRONG. `accountRecords.ts` rendered one of the eight state machines
 * a route gate can be over — the funding ledger's header, and not even its
 * slot statuses. A reader who pasted cohort-15's live Source, its funding
 * ledger's states, or its Direct activation root got "the protocol declares no
 * record with the magic DCLTSRS2", a rendered wrapper with nothing behind it,
 * or a row count and no rows. The bytes were on chain and public and the
 * browser had no word for them.
 *
 * WHAT HOLDS IT NOW. Two arms, and they fail for different reasons.
 *
 * The first is structural and derived: every spec's tags are compared against
 * `STATE_MACHINE_RECORDS_V1` itself, so a state added to a Rust enum reaches
 * the explorer by regenerating and a state renamed there reds here. That arm
 * cannot catch an error in the generated table, because it IS the table.
 *
 * The second is the fixture below, which can. These are accounts read off
 * devnet at a finalized floor — cohort 15, slot 492837406 — and the state each
 * one is in is pinned by NAME here rather than derived from the same table the
 * decoder reads. A tag value that moves under the explorer turns a `Resolved`
 * Source into a `Recovery` one and this file says so.
 */

type FixtureRecord = Readonly<{
  machine: string;
  address: string;
  accountBytes: number;
  recordOffset: number;
  recordHex: string;
}>;

const RECORDS: ReadonlyArray<FixtureRecord> = vector.records;

/**
 * What each captured account was in, at the finalized floor it was read at.
 *
 * Written out rather than derived: a pin that agreed with the decoder by
 * construction would agree with it just as happily when both were wrong.
 */
const OBSERVED: Readonly<Record<string, ReadonlyArray<string>>> = Object.freeze({
  FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG: ['Open'],
  '5QoawNdiiBtggmeFs81UsejxC5XwWayfPFsswN1redBr': ['Primary'],
  JAz42gc4tRTKFEWVzELAHe5tvYUG3SXkQJFVtWrRa5ka: ['Resolved'],
  '7c8Y9rTjSPPn9rAcwoQGicfrpJEXhaMafmZn2KXgvjGF': ['Active'],
  '8Vqh8E14hdZgjuV6VFUiwnv2EJPN1HzSugPa8gUvHjoE': ['Active', 'Active', 'Active'],
});

function bytesOf(hex: string): Uint8Array {
  return Uint8Array.from((hex.match(/../g) ?? []).map((pair) => Number.parseInt(pair, 16)));
}

/** Every state one decoded record is in: its own tag, or one per row. */
function statesOf(record: FixtureRecord): ReadonlyArray<string | null> {
  const bytes = bytesOf(record.recordHex);
  const spec = specForMagic(magicText(bytes.subarray(0, 8)));
  if (spec === null) return [];
  const decoded = decodeAgainstSpec(spec, bytes);
  if (decoded.rows !== null && decoded.rows.states !== null) {
    return decoded.rows.states.map((row) => row.name);
  }
  return decoded.fields
    .filter((entry) => entry.value.form === 'enum')
    .map((entry) => (entry.value.form === 'enum' ? entry.value.name : null));
}

describe('the persisted state machines, as records', () => {
  it('has machines to speak about at all', () => {
    // A table that came back empty would make every loop below vacuous.
    expect(STATE_MACHINE_RECORDS_V1.length).toBeGreaterThanOrEqual(8);
  });

  it('renders a spec for every machine', () => {
    const missing = STATE_MACHINE_RECORDS_V1
      .filter((row) => specForMagic(row.magic) === null)
      .map((row) => row.machine);
    expect(missing).toEqual([]);
  });

  it('gives every spec the states its own machine declares, and no others', () => {
    for (const row of STATE_MACHINE_RECORDS_V1) {
      const spec = specForMagic(row.magic);
      expect(spec, row.machine).not.toBeNull();
      if (spec === null) continue;
      const declared = row.states.map((state) => ({ tag: state.tag, name: state.state }));
      // The tag is a header field for seven of them and one row deep for the
      // funding ledger; either way it names exactly the machine's own states.
      const tags = spec.rowDiscriminant !== undefined
        ? spec.rowDiscriminant.tags
        : (spec.fields.find((entry) => entry.kind === 'enum')?.tags ?? []);
      expect(tags.map((entry) => ({ tag: entry.tag, name: entry.name })), row.machine).toEqual(declared);
    }
  });

  it('places every machine’s discriminant where its own decoder reads it', () => {
    for (const row of STATE_MACHINE_RECORDS_V1) {
      const spec = specForMagic(row.magic);
      if (spec === null) continue;
      if (spec.rowDiscriminant !== undefined) {
        expect(spec.rowDiscriminant.offset, row.machine).toBe(row.tagOffset);
        expect(spec.rowDiscriminant.strideBytes, row.machine).toBe(row.rowBytes);
        continue;
      }
      const enumField = spec.fields.find((entry) => entry.kind === 'enum');
      expect(enumField?.offset, row.machine).toBe(row.tagOffset);
    }
  });

  it('says plainly that only the discriminant and the schema words are published', () => {
    for (const row of STATE_MACHINE_RECORDS_V1) {
      const spec = specForMagic(row.magic);
      if (spec === null || spec.note === null) continue;
      expect(spec.note.length, row.machine).toBeGreaterThan(24);
    }
  });
});

describe('cohort-15’s own accounts', () => {
  it('carries the captured records this file speaks about', () => {
    expect(vector.schema).toBe('dclutch-state-machine-record-vector-v1');
    expect(RECORDS.length).toBe(Object.keys(OBSERVED).length);
    for (const record of RECORDS) expect(Object.keys(OBSERVED)).toContain(record.address);
  });

  for (const record of RECORDS) {
    it(`decodes ${record.machine} ${record.address.slice(0, 6)} as ${(OBSERVED[record.address] ?? []).join(', ')}`, () => {
      expect(statesOf(record)).toEqual(OBSERVED[record.address]);
    });
  }

  it('widths agree with the schema for every captured record', () => {
    for (const record of RECORDS) {
      const bytes = bytesOf(record.recordHex);
      const spec = specForMagic(magicText(bytes.subarray(0, 8)));
      expect(spec, record.address).not.toBeNull();
      if (spec === null) continue;
      expect(decodeAgainstSpec(spec, bytes).widthCheck.ok, record.address).toBe(true);
    }
  });

  it('reads the Direct root’s open-maker count off its own tail', () => {
    const record = RECORDS.find((entry) => entry.machine === 'direct-root');
    expect(record).toBeDefined();
    if (record === undefined) return;
    const bytes = bytesOf(record.recordHex);
    const spec = specForMagic(magicText(bytes.subarray(0, 8)));
    if (spec === null) return;
    const counter = decodeAgainstSpec(spec, bytes).fields.find((entry) => entry.kind === 'u64');
    expect(counter?.label).toBe('openMakerRootCount');
    expect(counter?.value).toEqual({ form: 'scalar', text: '0' });
  });
});

describe('a capability root’s trailing record', () => {
  /**
   * The account, rebuilt around the tail the capture holds.
   *
   * The capture recorded the RECORD, not the 232 bytes of composite capability
   * root in front of it, so the header here is synthesized at the width
   * `directInlineV3.ts` emits — which the capture's own `recordOffset` is
   * checked against below, so the one number this reconstruction depends on is
   * corroborated by the chain rather than assumed.
   */
  function capabilityRootAccount(record: FixtureRecord): Uint8Array {
    const account = new Uint8Array(record.accountBytes);
    account.set(CAPABILITY_ROOT_MAGIC_V1, 0);
    account.set(bytesOf(record.recordHex), record.recordOffset);
    return account;
  }

  const record = RECORDS.find((entry) => entry.machine === 'direct-root');

  it('agrees with the chain about where the tail starts', () => {
    expect(record?.recordOffset).toBe(CAPABILITY_ROOT_HEADER_BYTES_V1);
  });

  it('finds the Direct root behind the capability root the leading magic names', () => {
    expect(record).toBeDefined();
    if (record === undefined) return;
    const account = capabilityRootAccount(record);
    // The leading magic still resolves to the wrapper, which is the reason the
    // tail needed its own lookup at all.
    expect(specForData(account)?.name).toBe('Capability root');
    const trailing = trailingRecordForData(account);
    expect(trailing).not.toBeNull();
    if (trailing === null) return;
    expect(trailing.offset).toBe(CAPABILITY_ROOT_HEADER_BYTES_V1);
    const decoded = decodeAgainstSpec(trailing.spec, account.subarray(trailing.offset));
    expect(decoded.widthCheck.ok).toBe(true);
    expect(decoded.fields.find((entry) => entry.kind === 'enum')?.value).toEqual({
      form: 'enum',
      tag: 0,
      name: 'Open',
    });
  });

  it('claims no tail for an account whose leading magic is not a capability root', () => {
    const source = RECORDS.find((entry) => entry.machine === 'source');
    expect(source).toBeDefined();
    if (source === undefined) return;
    expect(trailingRecordForData(bytesOf(source.recordHex))).toBeNull();
  });

  it('claims no tail when the account is only the header', () => {
    const header = new Uint8Array(CAPABILITY_ROOT_HEADER_BYTES_V1);
    header.set(CAPABILITY_ROOT_MAGIC_V1, 0);
    expect(trailingRecordForData(header)).toBeNull();
  });

  it('claims no tail whose magic this explorer does not render', () => {
    const account = new Uint8Array(CAPABILITY_ROOT_HEADER_BYTES_V1 + 24);
    account.set(CAPABILITY_ROOT_MAGIC_V1, 0);
    // Printable, eight bytes, and not a magic any generated module declares.
    for (let index = 0; index < 8; index += 1) {
      account[CAPABILITY_ROOT_HEADER_BYTES_V1 + index] = 'ZZZZZZZZ'.charCodeAt(index);
    }
    expect(trailingRecordForData(account)).toBeNull();
  });
});
