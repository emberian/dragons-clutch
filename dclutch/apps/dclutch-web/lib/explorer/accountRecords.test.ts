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
  STATISTIC_SPEC_BYTES_V1,
  STATISTIC_SPEC_KIND_OFFSET_V1,
  STATISTIC_SPEC_MAGIC,
  STATISTIC_SPEC_ROUNDING_OFFSET_V1,
  STATISTIC_SPEC_SOURCE_SCALE_EXPONENT_OFFSET_V1,
  STATISTIC_SPEC_THRESHOLD_ATOMS_OFFSET_V1,
} from '../generated/coreFound';
import { REALM_BYTES_V1, REALM_MAGIC_V1 } from '../generated/realmPositionV1';
import statisticFixture from '../../fixtures/cohort14-statistic-spec.devnet.json';
import { deriveClaimsAggregateAddressV2, deriveMarketCoreAddressV2 } from '../marketCoreV2';
import {
  decodeAgainstSpec,
  leadingMagic,
  magicText,
  scaleExponentReadingV1,
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

/**
 * The statistic spec, read from a real one.
 *
 * `fixtures/cohort14-statistic-spec.devnet.json` is the account at
 * `4wuSesEnbGX6wmS4CNqP99CkjTUvbSf42bNCEsNMknhY`, whole and unmodified: the
 * `StatisticSpecV1` cohort-14's featured market -- market B, Terminal and
 * paid -- was resolved against, reached by the walk
 * `inspectMarketDeclaredScaleV1` performs and living at the Registry PDA its own
 * schema identity and content digest derive.
 *
 * IT IS CHECKED AGAINST A SECOND READER, not against itself. The fixture's
 * `fields` block was produced by a reader that parsed the coordinates out of the
 * Lean-emitted `generated_statistic_spec_v1.rs`; the assertions below are the
 * explorer's render table reading the same bytes through
 * `lib/generated/coreFound.ts`. Two independent readings of one schema, which
 * must agree -- the same shape as the PDA reproduction below, and for the same
 * reason. A single-reader test here would prove almost nothing: every offset in
 * a 176-byte fixed-width record lands inside the account, so a wrong one
 * renders a plausible value and refuses nothing. Measured: shifting the family
 * tag one byte left this suite GREEN, because this record's family and its
 * rounding boundary are both `1`.
 */
describe('reading a real StatisticSpecV1 off chain', () => {
  const expected = statisticFixture.fields;
  const bytes = Uint8Array.from(statisticFixture.dataHex.match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
  const spec = specForMagic(magicText(STATISTIC_SPEC_MAGIC));
  const read = (label: string) => {
    if (spec === null) throw new Error('the explorer does not render DCLTSTA1');
    return decodeAgainstSpec(spec, bytes).fields.find((entry) => entry.label === label)?.value;
  };

  it('identifies the record from its own leading bytes', () => {
    expect(bytes.length).toBe(STATISTIC_SPEC_BYTES_V1);
    expect(leadingMagic(bytes)).toBe(magicText(STATISTIC_SPEC_MAGIC));
    expect(specForData(bytes)?.name).toBe('Statistic spec');
    expect(spec).not.toBeNull();
    if (spec === null) return;
    expect(decodeAgainstSpec(spec, bytes).widthCheck.ok).toBe(true);
  });

  it('agrees with the second reader on every coordinate', () => {
    if (spec === null) return;
    expect(read('Schema version')).toEqual({ form: 'scalar', text: String(expected.schemaVersion) });
    expect(read('Required samples')).toEqual({ form: 'scalar', text: String(expected.requiredSamples) });
    expect(read('Threshold atoms')).toEqual({ form: 'scalar', text: expected.thresholdAtoms });
    expect(read('Source unit')).toEqual({ form: 'identity', hex: expected.sourceUnitIdHex });
    expect(read('Result unit')).toEqual({ form: 'identity', hex: expected.resultUnitIdHex });
    expect(read('Capacity profile')).toEqual({ form: 'identity', hex: expected.capacityProfileIdHex });
    expect(read('Evaluator release')).toEqual({ form: 'identity', hex: expected.evaluatorReleaseIdHex });
    // The one span the record still reserves, and the browser says whether it
    // holds what the program requires rather than showing fourteen zero bytes.
    expect(read('Reserved')).toEqual({ form: 'reserved', zero: true, hex: '0'.repeat(28) });
  });

  it('names the family and the rounding boundary in the schema’s own words', () => {
    if (spec === null) return;
    // Both tag tables are scraped from the enums in the crate that decodes the
    // bytes, so these names are the schema's and not this file's.
    expect(read('Statistic family')).toEqual({ form: 'enum', tag: expected.kindTag, name: 'Terminal sample' });
    expect(read('Rounding boundary')).toEqual({ form: 'enum', tag: expected.roundingTag, name: 'Exact rational' });
    // `StatisticSpecV1::new` refuses a TerminalSample that does not take exactly
    // one sample, and refuses any non-threshold family carrying a threshold. The
    // program admitted these bytes, so a reading that broke either rule would be
    // this table misreading the account, not the chain writing a bad one.
    expect(expected.kindTag).toBe(1);
    expect(read('Required samples')).toEqual({ form: 'scalar', text: '1' });
    expect(read('Threshold atoms')).toEqual({ form: 'scalar', text: '0' });
  });

  it('says the identity out loud on the two-unit record that declares it', () => {
    if (spec === null) return;
    // TWO DIFFERENT UNITS AND NO SHIFT: the two-scale defect's signature, and
    // the reason the reading is a sentence rather than a bare zero.
    expect(expected.sourceUnitIdHex).not.toBe(expected.resultUnitIdHex);
    expect(expected.sourceScaleExponent).toBe(0);
    expect(read('Source scale exponent')).toEqual({
      form: 'scale',
      exponent: 0,
      reading: 'identity — the observation and the cuts are compared as written',
    });
  });

  it('tells the family, the boundary and a signed threshold apart', () => {
    if (spec === null) return;
    // THE FIXTURE CANNOT PROVE THIS AND SAYS SO. This record's family and its
    // rounding boundary are both `1` and its threshold is `0`, so reading the
    // family one byte late, or the threshold as an unsigned half-width, gives
    // the same answer as reading it right -- measured, by shifting each and
    // watching the cases above stay green. A real record is the authority on
    // the LAYOUT; distinguishing the fields needs values that differ, so these
    // are those bytes with a different admitted family, boundary and threshold
    // written in. Both tags are real: `OddScheduledMedian` and `Ceiling` are
    // rows in the scraped enums, not numbers invented here.
    const variant = Uint8Array.from(bytes);
    const view = new DataView(variant.buffer);
    variant[STATISTIC_SPEC_KIND_OFFSET_V1] = 7;
    variant[STATISTIC_SPEC_ROUNDING_OFFSET_V1] = 3;
    view.setBigInt64(STATISTIC_SPEC_THRESHOLD_ATOMS_OFFSET_V1, -3n, true);
    view.setBigInt64(STATISTIC_SPEC_THRESHOLD_ATOMS_OFFSET_V1 + 8, -1n, true);
    const decoded = decodeAgainstSpec(spec, variant);
    const value = (label: string) => decoded.fields.find((entry) => entry.label === label)?.value;
    expect(value('Statistic family')).toEqual({ form: 'enum', tag: 7, name: 'Odd scheduled median' });
    expect(value('Rounding boundary')).toEqual({ form: 'enum', tag: 3, name: 'Ceiling' });
    // Sixteen bytes read as one signed value. Eight would say 18446744073709551613.
    expect(value('Threshold atoms')).toEqual({ form: 'scalar', text: '-3' });
  });

  it('reads a different sentence off a different exponent, from the same record', () => {
    if (spec === null) return;
    // The same account with a factor written into the four bytes this founding
    // left at the identity -- what the first market founded WITH one decodes to.
    // It is here so the reading is proved DERIVED: a constant string would pass
    // the case above and fail this one.
    const shifted = Uint8Array.from(bytes);
    new DataView(shifted.buffer).setInt32(STATISTIC_SPEC_SOURCE_SCALE_EXPONENT_OFFSET_V1, -8, true);
    expect(decodeAgainstSpec(spec, shifted).fields.find((entry) => entry.label === 'Source scale exponent')?.value).toEqual({
      form: 'scale',
      exponent: -8,
      reading: 'observation atoms × 10^−8 → the cuts’ scale',
    });
    // And the sign is read rather than assumed: a positive shift is admitted too.
    expect(scaleExponentReadingV1(6)).toBe('observation atoms × 10^6 → the cuts’ scale');
  });
});
