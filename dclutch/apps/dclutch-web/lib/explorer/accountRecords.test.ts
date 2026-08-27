import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  CORE_PHASE_OPEN_TAG,
  CORE_READINESS_READY_TAG,
  CORE_STATE_BYTES,
  CORE_STATE_MAGIC,
  CORE_STATE_VERSION_OFFSET,
  CORE_VERSION,
  LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
} from '../generated/coreFound';
import { REALM_BYTES_V1, REALM_MAGIC_V1 } from '../generated/realmPositionV1';
import { deriveClaimsAggregateAddressV2, deriveMarketCoreAddressV2 } from '../marketCoreV2';
import {
  decodeAgainstSpec,
  leadingMagic,
  magicText,
  specForData,
  specForMagic,
} from './accountRecords';
import { derivationsForRecord } from './derivations';

const CORE_PROGRAM = new PublicKey(new Uint8Array(32).fill(11)).toBase58();
const CLAIMS_PROGRAM = new PublicKey(new Uint8Array(32).fill(12)).toBase58();

/** A Core state whose sixteen fields are set to distinguishable values. */
function coreStateBytes(generation: bigint): Uint8Array {
  const bytes = new Uint8Array(CORE_STATE_BYTES);
  bytes.set(CORE_STATE_MAGIC, 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(CORE_STATE_VERSION_OFFSET, CORE_VERSION, true);
  bytes[10] = CORE_PHASE_OPEN_TAG;
  bytes[11] = CORE_READINESS_READY_TAG;
  // The nine identity seeds, each a distinct filled 32-byte run.
  for (let slot = 0; slot < 8; slot += 1) {
    bytes.fill(0x21 + slot, 16 + slot * 32, 48 + slot * 32);
  }
  view.setBigUint64(272, generation, true);
  view.setBigUint64(280, 3n, true);
  bytes.fill(0x31, 288, 320);
  bytes.fill(0x32, 320, 352);
  return bytes;
}

function aggregateBytes(market: string, claims: number): Uint8Array {
  const bytes = new Uint8Array(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + claims * 8);
  bytes.set(LIABILITY_BASIS_MARKET_MAGIC_V2, 0);
  const view = new DataView(bytes.buffer);
  view.setUint32(LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET, claims, true);
  bytes.set(new PublicKey(market).toBytes(), LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET);
  for (let index = 0; index < claims; index += 1) {
    view.setBigUint64(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + index * 8, BigInt(1000 + index), true);
  }
  return bytes;
}

describe('identifying a record by its own magic', () => {
  it('resolves a Core state by its leading bytes', () => {
    const bytes = coreStateBytes(7n);
    expect(leadingMagic(bytes)).toBe(magicText(CORE_STATE_MAGIC));
    expect(specForData(bytes)?.name).toBe('Market Core state');
  });

  it('resolves nothing for a magic no generated module declares', () => {
    const bytes = new Uint8Array(64);
    bytes.set(new TextEncoder().encode('NOTAMAGC'), 0);
    expect(leadingMagic(bytes)).toBe('NOTAMAGC');
    expect(specForData(bytes)).toBeNull();
  });

  it('resolves nothing when the header is not printable', () => {
    const bytes = new Uint8Array(64);
    bytes[0] = 0x00;
    expect(leadingMagic(bytes)).toBeNull();
    expect(specForData(bytes)).toBeNull();
  });
});

describe('decoding against a spec', () => {
  it('reads the Core state’s typed fields and checks its exact width', () => {
    const spec = specForMagic(magicText(CORE_STATE_MAGIC));
    expect(spec).not.toBeNull();
    if (spec === null) return;
    const decoded = decodeAgainstSpec(spec, coreStateBytes(9n));
    expect(decoded.widthCheck.ok).toBe(true);

    const phase = decoded.fields.find((entry) => entry.label === 'Phase');
    expect(phase?.value).toEqual({ form: 'enum', tag: CORE_PHASE_OPEN_TAG, name: 'Open' });

    const generation = decoded.fields.find((entry) => entry.label === 'Generation');
    expect(generation?.value).toEqual({ form: 'scalar', text: '9' });

    // A 32-byte slot the emission names as a program renders as an address; one
    // it names as an identity renders as hex. The two never swap.
    const registry = decoded.fields.find((entry) => entry.label === 'Registry program');
    expect(registry?.value.form).toBe('address');
    const realm = decoded.fields.find((entry) => entry.label === 'Realm identity');
    expect(realm?.value.form).toBe('identity');
  });

  it('refuses a field that lies past the observed bytes rather than reading garbage', () => {
    const spec = specForMagic(magicText(CORE_STATE_MAGIC));
    if (spec === null) return;
    const truncated = coreStateBytes(1n).slice(0, 64);
    const decoded = decodeAgainstSpec(spec, truncated);
    expect(decoded.widthCheck.ok).toBe(false);
    const late = decoded.fields.find((entry) => entry.label === 'Rent beneficiary');
    expect(late?.value.form).toBe('refused');
    // The early fields still decode: a short account is not a total loss.
    expect(decoded.fields.find((entry) => entry.label === 'Phase')?.value.form).toBe('enum');
  });

  it('sizes a header-and-rows record from its own count field', () => {
    const market = new PublicKey(new Uint8Array(32).fill(5)).toBase58();
    const spec = specForMagic(magicText(LIABILITY_BASIS_MARKET_MAGIC_V2));
    expect(spec).not.toBeNull();
    if (spec === null) return;
    const decoded = decodeAgainstSpec(spec, aggregateBytes(market, 3));
    expect(decoded.widthCheck.ok).toBe(true);
    expect(decoded.rows?.count).toBe(3);
    expect(decoded.rows?.scalars).toEqual(['1000', '1001', '1002']);
  });

  it('marks a row count that disagrees with the observed width', () => {
    const market = new PublicKey(new Uint8Array(32).fill(5)).toBase58();
    const spec = specForMagic(magicText(LIABILITY_BASIS_MARKET_MAGIC_V2));
    if (spec === null) return;
    const bytes = aggregateBytes(market, 3);
    const decoded = decodeAgainstSpec(spec, bytes.slice(0, bytes.length - 8));
    expect(decoded.widthCheck.ok).toBe(false);
    expect(decoded.widthCheck.expected).toContain('3 × 8');
  });

  it('reports a nonzero reserved run instead of ignoring it', () => {
    const spec = specForMagic(REALM_MAGIC_V1);
    expect(spec).not.toBeNull();
    if (spec === null) return;
    const bytes = new Uint8Array(REALM_BYTES_V1);
    bytes.set(new TextEncoder().encode(REALM_MAGIC_V1), 0);
    const clean = decodeAgainstSpec(spec, bytes);
    expect(clean.fields.find((entry) => entry.kind === 'reserved')?.value).toEqual({
      form: 'reserved',
      zero: true,
      hex: '00000000',
    });
    bytes[12] = 1;
    const dirty = decodeAgainstSpec(spec, bytes);
    const reserved = dirty.fields.find((entry) => entry.kind === 'reserved');
    expect(reserved?.value.form).toBe('reserved');
    if (reserved?.value.form === 'reserved') expect(reserved.value.zero).toBe(false);
  });
});

describe('PDA annotation by reproduction', () => {
  it('reproduces a Market’s address from the Market’s own nine seeds', () => {
    const bytes = coreStateBytes(9n);
    // `marketCoreV2.deriveMarketCoreAddressV2` reads the seeds straight off the
    // offsets; this module reads them through the spec's field table. The two
    // are independent readings of the same schema, and they must agree.
    const address = deriveMarketCoreAddressV2(CORE_PROGRAM, bytes);
    const spec = specForMagic(magicText(CORE_STATE_MAGIC));
    if (spec === null) return;
    const decoded = decodeAgainstSpec(spec, bytes);
    const derivations = derivationsForRecord(decoded, bytes, address, CORE_PROGRAM);
    expect(derivations).toHaveLength(1);
    expect(derivations[0].matches).toBe(true);
    expect(derivations[0].derived).toBe(address);
  });

  it('reports a mismatch rather than staying silent about it', () => {
    const bytes = coreStateBytes(9n);
    const spec = specForMagic(magicText(CORE_STATE_MAGIC));
    if (spec === null) return;
    const decoded = decodeAgainstSpec(spec, bytes);
    const elsewhere = new PublicKey(new Uint8Array(32).fill(99)).toBase58();
    const derivations = derivationsForRecord(decoded, bytes, elsewhere, CORE_PROGRAM);
    expect(derivations).toHaveLength(1);
    // An account carrying seeds it does not sit at is a finding, not silence.
    expect(derivations[0].matches).toBe(false);
  });

  it('derives under the account’s ACTUAL owner, so a wrong-program account does not match', () => {
    const bytes = coreStateBytes(9n);
    const address = deriveMarketCoreAddressV2(CORE_PROGRAM, bytes);
    const spec = specForMagic(magicText(CORE_STATE_MAGIC));
    if (spec === null) return;
    const decoded = decodeAgainstSpec(spec, bytes);
    expect(derivationsForRecord(decoded, bytes, address, CLAIMS_PROGRAM)[0].matches).toBe(false);
  });

  it('reproduces a Claims aggregate from the Market it names', () => {
    const market = new PublicKey(new Uint8Array(32).fill(5)).toBase58();
    const address = deriveClaimsAggregateAddressV2(CLAIMS_PROGRAM, market);
    const bytes = aggregateBytes(market, 2);
    const spec = specForMagic(magicText(LIABILITY_BASIS_MARKET_MAGIC_V2));
    if (spec === null) return;
    const decoded = decodeAgainstSpec(spec, bytes);
    const derivations = derivationsForRecord(decoded, bytes, address, CLAIMS_PROGRAM);
    expect(derivations).toHaveLength(1);
    expect(derivations[0].matches).toBe(true);
  });

  it('asserts no derivation for a record whose seeds it cannot recover', () => {
    // The Direct Position's seeds include a maker and an outcome the record
    // does not carry. Saying nothing is the correct answer.
    const bytes = new Uint8Array(REALM_BYTES_V1);
    bytes.set(new TextEncoder().encode('DCLTPOS1'), 0);
    const spec = specForData(bytes);
    expect(spec?.name).toBe('Position');
    if (spec === null) return;
    expect(derivationsForRecord(decodeAgainstSpec(spec, bytes), bytes, CORE_PROGRAM, CORE_PROGRAM)).toEqual([]);
  });
});
