import { describe, expect, it } from 'vitest';

import vectorsJson from '../../fixtures/founding/generic-founding-vectors.json';
import {
  GENERIC_FOUNDING_REQUEST_BYTES_V1,
  GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_OFFSET_V1,
  GENERIC_FOUNDING_STAGES_V1,
} from '../generated/genericFoundingV1';
import { hex } from '../bytes';
import {
  decodeGenericFoundingAckV1,
  decodeGenericFoundingRequestV1,
  encodeGenericFoundingRequestV1,
  genericFoundingFundingListIdV1,
  withGenericFoundingStageV1,
  type GenericFoundingRequestInputV1,
  type GenericFoundingStageNameV1,
} from './genericFoundingRequest';

type Vectors = Readonly<{
  schema: string;
  provenance: string;
  requests: ReadonlyArray<Readonly<{ name: string; bytes: string }>>;
  acks: ReadonlyArray<Readonly<{ name: string; bytes: string }>>;
  fundingListIds: ReadonlyArray<Readonly<{ name: string; members: ReadonlyArray<string>; id: string }>>;
}>;

const vectors = vectorsJson as Vectors;

function bytes(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
}

function repeated(byte: number): string {
  return byte.toString(16).padStart(2, '0').repeat(32);
}

/**
 * The literals `generic_founding_v1.rs`'s own `mod tests::request` uses.
 *
 * These are transcribed from the Rust test, not from the emitted vector: the
 * point of the comparison below is that two independent encoders fed the same
 * *named* inputs produce the same bytes. Reading the inputs back out of the
 * vector would make the test compare the vector with itself.
 */
function canonical(stage: GenericFoundingStageNameV1): GenericFoundingRequestInputV1 {
  return {
    stage,
    fundingCount: 3,
    releaseSet: repeated(1),
    market: repeated(2),
    capabilityRoot: repeated(3),
    context: repeated(4),
    founder: repeated(5),
    beneficiary: repeated(6),
    fundingSource: repeated(7),
    hoard: repeated(8),
    projectedReplay: repeated(9),
    fundingListId: repeated(10),
    generation: 11n,
    quantity: 12n,
    basisScale: 13n,
    expirySlot: 14n,
    marketRent: 15n,
    permitRent: 16n,
    projectedResultingRevision: 2n,
    capabilityEntryIndex: 5,
  };
}

const extremal: GenericFoundingRequestInputV1 = {
  stage: 'Open',
  fundingCount: 16,
  releaseSet: repeated(0xa1),
  market: repeated(0xa2),
  capabilityRoot: repeated(0xa3),
  context: repeated(0xa4),
  founder: repeated(0xa5),
  beneficiary: repeated(0xa6),
  fundingSource: repeated(0xa7),
  hoard: repeated(0xa8),
  projectedReplay: repeated(0xa9),
  fundingListId: repeated(0xaa),
  generation: 0xffff_ffff_ffff_ffffn,
  quantity: 1n,
  basisScale: 0xffff_ffff_ffff_ffffn,
  expirySlot: 0xffff_ffff_ffff_ffffn,
  marketRent: 0xffff_ffff_ffff_ffffn,
  permitRent: 0xffff_ffff_ffff_ffffn,
  projectedResultingRevision: 0xffff_ffff_ffff_ffffn,
  capabilityEntryIndex: 0xffff,
};

function vector(name: string): Uint8Array {
  const entry = vectors.requests.find((candidate) => candidate.name === name);
  if (entry === undefined) throw new Error(`missing Rust vector ${name}`);
  return bytes(entry.bytes);
}

describe('GenericFoundingRequestV1 against the first-party Rust encoder', () => {
  it('is byte-identical to dclutch-market-core-codec for every emitted vector', () => {
    expect(vectors.schema).toBe('dclutch-web-generic-founding-vectors-v1');
    expect(hex(encodeGenericFoundingRequestV1(canonical('FoundAndPermit')))).toBe(vectors.requests[0].bytes);
    expect(hex(encodeGenericFoundingRequestV1(canonical('Open')))).toBe(vectors.requests[1].bytes);
    expect(hex(encodeGenericFoundingRequestV1(extremal))).toBe(vectors.requests[2].bytes);
  });

  it('round-trips every Rust-produced request through decode and re-encode', () => {
    for (const entry of vectors.requests) {
      const decoded = decodeGenericFoundingRequestV1(bytes(entry.bytes));
      expect(hex(encodeGenericFoundingRequestV1(decoded))).toBe(entry.bytes);
    }
  });

  it('separates the two stages by exactly one byte, and that byte is the stage tag', () => {
    const found = encodeGenericFoundingRequestV1(canonical('FoundAndPermit'));
    const open = encodeGenericFoundingRequestV1(withGenericFoundingStageV1(decodeGenericFoundingRequestV1(found), 'Open'));
    const differing = [...found].flatMap((byte, index) => (byte === open[index] ? [] : [index]));
    expect(differing).toEqual([10]);
    expect(GENERIC_FOUNDING_STAGES_V1.map((stage) => stage.tag)).toEqual([found[10], open[10]]);
  });

  it('decodes the Rust-produced acknowledgement', () => {
    const ack = decodeGenericFoundingAckV1(bytes(vectors.acks[0].bytes));
    expect(ack.stage).toBe('FoundAndPermit');
    expect(ack.fundingCount).toBe(3);
    expect(ack.coreProgram).toBe(repeated(20));
    expect(ack.permit).toBe(repeated(21));
    expect(ack.requestDigest).toBe(repeated(22));
    expect(ack.postResourceDigest).toBe(repeated(23));
    // The ack carries the request's own release set, market, funding list and
    // generation through unchanged; that is what makes it checkable against
    // the request the caller sent rather than merely well-formed.
    expect(ack.releaseSet).toBe(repeated(1));
    expect(ack.market).toBe(repeated(2));
    expect(ack.fundingListId).toBe(repeated(10));
    expect(ack.generation).toBe(11n);
  });

  it('reproduces the Rust funding-list digest at one, three, and the maximum sixteen', async () => {
    for (const entry of vectors.fundingListIds) {
      expect(await genericFoundingFundingListIdV1(entry.members)).toBe(entry.id);
    }
  });
});

describe('GenericFoundingRequestV1 refusals', () => {
  it('refuses a nonzero byte in either hostile-checked reserved span', () => {
    for (const offset of [GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1, GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_OFFSET_V1]) {
      const tampered = vector('canonical-found-and-permit');
      tampered[offset] = 1;
      expect(() => decodeGenericFoundingRequestV1(tampered)).toThrow(/noncanonical reserved bytes/);
    }
  });

  it('refuses a truncated or widened request', () => {
    expect(() => decodeGenericFoundingRequestV1(vector('canonical-open').slice(0, GENERIC_FOUNDING_REQUEST_BYTES_V1 - 1))).toThrow(/wrong exact ABI/);
    expect(() => decodeGenericFoundingRequestV1(new Uint8Array(GENERIC_FOUNDING_REQUEST_BYTES_V1 + 1))).toThrow(/wrong exact ABI/);
  });

  it('refuses an undefined stage tag', () => {
    const tampered = vector('canonical-open');
    tampered[10] = 3;
    expect(() => decodeGenericFoundingRequestV1(tampered)).toThrow(/stage tag 3 is undefined/);
  });

  it('refuses each aliasing pair the chain refuses', () => {
    const pairs = [
      ['capabilityRoot', 'context', /capability root and context alias/],
      ['fundingSource', 'hoard', /funding source and Hoard alias/],
      ['projectedReplay', 'hoard', /projected replay and Hoard alias/],
      ['projectedReplay', 'fundingSource', /projected replay and funding source alias/],
    ] as const;
    for (const [left, right, message] of pairs) {
      const base = canonical('FoundAndPermit');
      expect(() => encodeGenericFoundingRequestV1({ ...base, [left]: base[right] })).toThrow(message);
    }
  });

  it('refuses a funding count outside 1..16 and a revision below two', () => {
    const base = canonical('FoundAndPermit');
    expect(() => encodeGenericFoundingRequestV1({ ...base, fundingCount: 0 })).toThrow(/funding count must be 1\.\.16/);
    expect(() => encodeGenericFoundingRequestV1({ ...base, fundingCount: 17 })).toThrow(/funding count must be 1\.\.16/);
    expect(() => encodeGenericFoundingRequestV1({ ...base, projectedResultingRevision: 1n })).toThrow(/at least 2/);
  });

  it('refuses the quantity-times-scale overflow rather than wrapping it', () => {
    const base = canonical('FoundAndPermit');
    expect(() => encodeGenericFoundingRequestV1({ ...base, quantity: 1n << 33n, basisScale: 1n << 33n })).toThrow(/overflows u64/);
  });

  it('refuses a zero identity and a zero required scalar', () => {
    const base = canonical('FoundAndPermit');
    expect(() => encodeGenericFoundingRequestV1({ ...base, hoard: '0'.repeat(64) })).toThrow(/all-zero identity/);
    expect(() => encodeGenericFoundingRequestV1({ ...base, marketRent: 0n })).toThrow(/marketRent must be nonzero/);
  });

  it('refuses a funding list that is empty, oversized, or aliased', async () => {
    await expect(genericFoundingFundingListIdV1([])).rejects.toThrow(/1\.\.16/);
    await expect(genericFoundingFundingListIdV1(Array.from({ length: 17 }, (_, index) => repeated(index)))).rejects.toThrow(/1\.\.16/);
    await expect(genericFoundingFundingListIdV1([repeated(1), repeated(1)])).rejects.toThrow(/aliases/);
  });
});
