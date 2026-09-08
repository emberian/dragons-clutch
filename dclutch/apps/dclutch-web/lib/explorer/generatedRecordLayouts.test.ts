import { describe, expect, it } from 'vitest';

import * as AGGREGATE_RETIREMENT from '@dclutch/sdk/generated/aggregateRetirementV1';
import * as RATIONAL_DESCRIPTOR from '@dclutch/sdk/generated/rationalRepresentationDescriptorV3';
import * as RATIONAL_LIFECYCLE from '@dclutch/sdk/generated/rationalLifecycleRequestV2';
import * as RATIONAL_REPLAY from '@dclutch/sdk/generated/rationalReplayV2';
import * as RESOLUTION_CERTIFICATE from '@dclutch/sdk/generated/resolutionCertificateV2';

import { decodeAgainstSpec, magicText, specForMagic } from './accountRecords';

function putMagic(output: Uint8Array, magic: string | Uint8Array): void {
  output.set(typeof magic === 'string' ? new TextEncoder().encode(magic) : magic, 0);
}

function putU64(output: Uint8Array, offset: number, value: bigint): void {
  new DataView(output.buffer, output.byteOffset, output.byteLength).setBigUint64(offset, value, true);
}

function putU32(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset, output.byteLength).setUint32(offset, value, true);
}

function putI128(output: Uint8Array, offset: number, value: bigint): void {
  let encoded = BigInt.asUintN(128, value);
  for (let index = 0; index < 16; index += 1) {
    output[offset + index] = Number(encoded & 0xffn);
    encoded >>= 8n;
  }
}

function decoded(magic: string | Uint8Array, data: Uint8Array) {
  const spec = specForMagic(magicText(magic));
  if (spec === null) throw new Error(`missing renderer for ${magicText(magic)}`);
  return decodeAgainstSpec(spec, data);
}

describe('generated record layouts', () => {
  it('reads the aggregate checkpoint from its generated coordinates without losing u64 precision', () => {
    const bytes = new Uint8Array(AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1);
    putMagic(bytes, AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1);
    new DataView(bytes.buffer).setUint16(
      AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_VERSION_OFFSET_V1,
      AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_CHECKPOINT_VERSION_V1,
      true,
    );
    bytes[AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_PHASE_OFFSET_V1] =
      AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_PHASE_CUSTODY_REPLAY_CLOSED_V1;
    putU64(bytes, AGGREGATE_RETIREMENT.GENERATION_OFFSET, 18_446_744_073_709_551_614n);
    putU64(bytes, AGGREGATE_RETIREMENT.PHASE_REVISION_OFFSET, 9_007_199_254_740_993n);

    const read = decoded(AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1, bytes);
    expect(read.widthCheck.ok).toBe(true);
    expect(read.fields.find((field) => field.label === 'Retirement phase')).toMatchObject({
      offset: AGGREGATE_RETIREMENT.AGGREGATE_RETIREMENT_PHASE_OFFSET_V1,
      value: { form: 'enum', name: 'Custody replay closed' },
    });
    expect(read.fields.find((field) => field.label === 'Market generation')).toMatchObject({
      offset: AGGREGATE_RETIREMENT.GENERATION_OFFSET,
      value: { form: 'scalar', text: '18446744073709551614' },
    });
    expect(read.fields.find((field) => field.label === 'Phase revision')).toMatchObject({
      offset: AGGREGATE_RETIREMENT.PHASE_REVISION_OFFSET,
      value: { form: 'scalar', text: '9007199254740993' },
    });
  });

  it('reads the compact lifecycle Hot header at the shared owner offsets', () => {
    const bytes = new Uint8Array(RATIONAL_LIFECYCLE.RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4);
    putMagic(bytes, RATIONAL_LIFECYCLE.RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4);
    new DataView(bytes.buffer).setUint16(
      RATIONAL_LIFECYCLE.LIFECYCLE_VERSION_OFFSET,
      RATIONAL_LIFECYCLE.RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4,
      true,
    );
    bytes[RATIONAL_LIFECYCLE.LIFECYCLE_ACTION_OFFSET] = RATIONAL_LIFECYCLE.LIFECYCLE_ACTION_RETIRE_RECEIPT_V2;
    putU64(bytes, RATIONAL_LIFECYCLE.LIFECYCLE_RENT_CREDIT_AFTER_OFFSET, 9_007_199_254_740_993n);

    const read = decoded(RATIONAL_LIFECYCLE.RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4, bytes);
    expect(read.widthCheck.ok).toBe(true);
    expect(read.fields.find((field) => field.label === 'Action')).toMatchObject({
      offset: RATIONAL_LIFECYCLE.LIFECYCLE_ACTION_OFFSET,
      value: { form: 'enum', name: 'Retire receipt' },
    });
    expect(read.fields.find((field) => field.label === 'Rent credit after (lamports)')).toMatchObject({
      offset: RATIONAL_LIFECYCLE.LIFECYCLE_RENT_CREDIT_AFTER_OFFSET,
      value: { form: 'scalar', text: '9007199254740993' },
    });
  });

  it('derives a descriptor width from its generated outcome count and refuses a truncated coefficient run', () => {
    const outcomes = 2;
    const bytes = new Uint8Array(
      RATIONAL_DESCRIPTOR.DESCRIPTOR_HEADER_BYTES + outcomes * RATIONAL_DESCRIPTOR.DESCRIPTOR_COEFFICIENT_BYTES,
    );
    putMagic(bytes, RATIONAL_DESCRIPTOR.DESCRIPTOR_MAGIC_V3);
    new DataView(bytes.buffer).setUint16(
      RATIONAL_DESCRIPTOR.DESCRIPTOR_VERSION_OFFSET,
      RATIONAL_DESCRIPTOR.DESCRIPTOR_SCHEMA_VERSION_V3,
      true,
    );
    putU32(bytes, RATIONAL_DESCRIPTOR.DESCRIPTOR_OUTCOME_COUNT_OFFSET, outcomes);
    putU64(bytes, RATIONAL_DESCRIPTOR.DESCRIPTOR_DENOMINATOR_OFFSET, 9_007_199_254_740_993n);
    putU64(bytes, RATIONAL_DESCRIPTOR.DESCRIPTOR_HEADER_BYTES, 18_446_744_073_709_551_615n);
    putU64(bytes, RATIONAL_DESCRIPTOR.DESCRIPTOR_HEADER_BYTES + RATIONAL_DESCRIPTOR.DESCRIPTOR_COEFFICIENT_BYTES, 7n);

    const read = decoded(RATIONAL_DESCRIPTOR.DESCRIPTOR_MAGIC_V3, bytes);
    expect(read.widthCheck.ok).toBe(true);
    expect(read.fields.find((field) => field.label === 'Coefficient denominator')).toMatchObject({
      offset: RATIONAL_DESCRIPTOR.DESCRIPTOR_DENOMINATOR_OFFSET,
      value: { form: 'scalar', text: '9007199254740993' },
    });
    expect(read.rows).toMatchObject({
      offset: RATIONAL_DESCRIPTOR.DESCRIPTOR_HEADER_BYTES,
      count: outcomes,
      scalars: ['18446744073709551615', '7'],
    });
    expect(decoded(RATIONAL_DESCRIPTOR.DESCRIPTOR_MAGIC_V3, bytes.slice(0, -1)).widthCheck.ok).toBe(false);
  });

  it('reads the Rational replay revision at its generated offset', () => {
    const bytes = new Uint8Array(RATIONAL_REPLAY.RATIONAL_REPLAY_BYTES_V2);
    putMagic(bytes, RATIONAL_REPLAY.RATIONAL_REPLAY_MAGIC_V2);
    new DataView(bytes.buffer).setUint16(
      RATIONAL_REPLAY.RATIONAL_REPLAY_VERSION_OFFSET,
      RATIONAL_REPLAY.RATIONAL_REPLAY_VERSION_V2,
      true,
    );
    putU64(bytes, RATIONAL_REPLAY.RATIONAL_REPLAY_REVISION_OFFSET, 9_007_199_254_740_993n);

    const read = decoded(RATIONAL_REPLAY.RATIONAL_REPLAY_MAGIC_V2, bytes);
    expect(read.widthCheck.ok).toBe(true);
    expect(read.fields.find((field) => field.label === 'Revision')).toMatchObject({
      offset: RATIONAL_REPLAY.RATIONAL_REPLAY_REVISION_OFFSET,
      value: { form: 'scalar', text: '9007199254740993' },
    });
  });

  it('keeps the certificate numerator signed and refuses a truncated fixed record', () => {
    const bytes = new Uint8Array(RESOLUTION_CERTIFICATE.RESOLUTION_CERTIFICATE_BYTES_V2);
    putMagic(bytes, RESOLUTION_CERTIFICATE.RESOLUTION_CERTIFICATE_MAGIC_V2);
    new DataView(bytes.buffer).setUint16(
      RESOLUTION_CERTIFICATE.CERTIFICATE_V2_VERSION_OFFSET,
      RESOLUTION_CERTIFICATE.RESOLUTION_CERTIFICATE_VERSION_V2,
      true,
    );
    bytes[RESOLUTION_CERTIFICATE.CERTIFICATE_V2_KIND_OFFSET] = RESOLUTION_CERTIFICATE.RESOLUTION_CERTIFICATE_FAILURE_KIND_V2;
    putI128(bytes, RESOLUTION_CERTIFICATE.CERTIFICATE_V2_RESULT_NUMERATOR_OFFSET, -(1n << 100n) + 3n);
    putU64(bytes, RESOLUTION_CERTIFICATE.CERTIFICATE_V2_RESULT_DENOMINATOR_OFFSET, 9_007_199_254_740_993n);

    const read = decoded(RESOLUTION_CERTIFICATE.RESOLUTION_CERTIFICATE_MAGIC_V2, bytes);
    expect(read.widthCheck.ok).toBe(true);
    expect(read.fields.find((field) => field.label === 'Certificate kind')).toMatchObject({
      offset: RESOLUTION_CERTIFICATE.CERTIFICATE_V2_KIND_OFFSET,
      value: { form: 'enum', name: 'Failure' },
    });
    expect(read.fields.find((field) => field.label === 'Normalized result numerator')).toMatchObject({
      offset: RESOLUTION_CERTIFICATE.CERTIFICATE_V2_RESULT_NUMERATOR_OFFSET,
      value: { form: 'scalar', text: '-1267650600228229401496703205373' },
    });
    expect(read.fields.find((field) => field.label === 'Result denominator')).toMatchObject({
      offset: RESOLUTION_CERTIFICATE.CERTIFICATE_V2_RESULT_DENOMINATOR_OFFSET,
      value: { form: 'scalar', text: '9007199254740993' },
    });
    const truncated = decoded(RESOLUTION_CERTIFICATE.RESOLUTION_CERTIFICATE_MAGIC_V2, bytes.slice(0, -1));
    expect(truncated.widthCheck.ok).toBe(false);
    expect(truncated.fields.find((field) => field.label === 'Observed at (unix seconds)')?.value.form).toBe('refused');
  });
});
