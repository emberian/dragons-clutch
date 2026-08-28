import { ascii, isZero, requireNonzero, requireZero, slice, u16, u64 } from './bytes';
import * as Abi from './generated/resolutionCertificateV2';

export type ResolutionCertificateKindV2 =
  | 'resolution-success'
  | 'recovery-advanced'
  | 'exhausted'
  | 'resolution-failure';

export type ResolutionCertificateV2 = Readonly<{
  kind: ResolutionCertificateKindV2;
  market: Uint8Array;
  route: Uint8Array;
  sourceMaterial: Uint8Array;
  productRecordDigest: Uint8Array;
  providerEvidence: Uint8Array;
  fundingAllocation: Uint8Array;
  receiptAccount: Uint8Array;
  generation: bigint;
  attemptIndex: number;
  scheduleIndex: number;
  selector: number;
  workPaid: bigint;
  fundingRemaining: bigint;
  resultNumerator: bigint;
  resultDenominator: bigint;
  observedAt: bigint;
}>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function u32(bytes: Uint8Array, offset: number): number {
  const value = slice(bytes, offset, 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
}

function i128(bytes: Uint8Array, offset: number): bigint {
  const value = slice(bytes, offset, 16);
  let decoded = 0n;
  for (let index = value.length - 1; index >= 0; index -= 1) decoded = (decoded << 8n) | BigInt(value[index] ?? 0);
  return decoded >= (1n << 127n) ? decoded - (1n << 128n) : decoded;
}

function kind(tag: number): ResolutionCertificateKindV2 {
  switch (tag) {
    case Abi.RESOLUTION_CERTIFICATE_SUCCESS_KIND_V2: return 'resolution-success';
    case Abi.RESOLUTION_CERTIFICATE_RECOVERY_ADVANCED_KIND_V2: return 'recovery-advanced';
    case Abi.RESOLUTION_CERTIFICATE_EXHAUSTED_KIND_V2: return 'exhausted';
    case Abi.RESOLUTION_CERTIFICATE_FAILURE_KIND_V2: return 'resolution-failure';
    default: throw new Error('ResolutionCertificateV2 has an unknown kind');
  }
}

/** Hostile-decode the exact canonical Rust `ResolutionCertificateV2` ABI. */
export function decodeResolutionCertificateV2(bytes: Uint8Array): ResolutionCertificateV2 {
  if (bytes.length !== Abi.RESOLUTION_CERTIFICATE_BYTES_V2
      || ascii(bytes, Abi.CERTIFICATE_V2_MAGIC_OFFSET, Abi.RESOLUTION_CERTIFICATE_MAGIC_V2.length) !== Abi.RESOLUTION_CERTIFICATE_MAGIC_V2
      || u16(bytes, Abi.CERTIFICATE_V2_VERSION_OFFSET) !== Abi.RESOLUTION_CERTIFICATE_VERSION_V2) {
    throw new Error('ResolutionCertificateV2 has the wrong exact ABI');
  }
  requireZero(bytes, Abi.CERTIFICATE_V2_RESERVED_HEADER_OFFSET,
    Abi.CERTIFICATE_V2_MARKET_OFFSET - Abi.CERTIFICATE_V2_RESERVED_HEADER_OFFSET,
    'ResolutionCertificateV2 header');
  requireZero(bytes, Abi.CERTIFICATE_V2_RESERVED_BODY_OFFSET,
    Abi.CERTIFICATE_V2_WORK_PAID_OFFSET - Abi.CERTIFICATE_V2_RESERVED_BODY_OFFSET,
    'ResolutionCertificateV2 body');
  const certificate = Object.freeze({
    kind: kind(bytes[Abi.CERTIFICATE_V2_KIND_OFFSET] ?? 0),
    market: slice(bytes, Abi.CERTIFICATE_V2_MARKET_OFFSET, 32),
    route: slice(bytes, Abi.CERTIFICATE_V2_ROUTE_OFFSET, 32),
    sourceMaterial: slice(bytes, Abi.CERTIFICATE_V2_SOURCE_MATERIAL_OFFSET, 32),
    productRecordDigest: slice(bytes, Abi.CERTIFICATE_V2_PRODUCT_RECORD_OFFSET, 32),
    providerEvidence: slice(bytes, Abi.CERTIFICATE_V2_PROVIDER_EVIDENCE_OFFSET, 32),
    fundingAllocation: slice(bytes, Abi.CERTIFICATE_V2_FUNDING_ALLOCATION_OFFSET, 32),
    receiptAccount: slice(bytes, Abi.CERTIFICATE_V2_RECEIPT_ACCOUNT_OFFSET, 32),
    generation: u64(bytes, Abi.CERTIFICATE_V2_GENERATION_OFFSET),
    attemptIndex: u32(bytes, Abi.CERTIFICATE_V2_ATTEMPT_INDEX_OFFSET),
    scheduleIndex: u32(bytes, Abi.CERTIFICATE_V2_SCHEDULE_INDEX_OFFSET),
    selector: u32(bytes, Abi.CERTIFICATE_V2_SELECTOR_OFFSET),
    workPaid: u64(bytes, Abi.CERTIFICATE_V2_WORK_PAID_OFFSET),
    fundingRemaining: u64(bytes, Abi.CERTIFICATE_V2_FUNDING_REMAINING_OFFSET),
    resultNumerator: i128(bytes, Abi.CERTIFICATE_V2_RESULT_NUMERATOR_OFFSET),
    resultDenominator: u64(bytes, Abi.CERTIFICATE_V2_RESULT_DENOMINATOR_OFFSET),
    observedAt: u64(bytes, Abi.CERTIFICATE_V2_OBSERVED_AT_OFFSET),
  } satisfies ResolutionCertificateV2);
  for (const [field, identity] of [
    ['market', certificate.market],
    ['source material', certificate.sourceMaterial],
    ['Product record', certificate.productRecordDigest],
    ['receipt account', certificate.receiptAccount],
  ] as const) requireNonzero(identity, `ResolutionCertificateV2 ${field}`);
  if (certificate.generation === 0n) throw new Error('ResolutionCertificateV2 generation is zero');
  if (certificate.kind === 'resolution-success') {
    requireNonzero(certificate.route, 'ResolutionCertificateV2 success route');
    requireNonzero(certificate.providerEvidence, 'ResolutionCertificateV2 provider evidence');
    if (certificate.resultDenominator === 0n || certificate.observedAt === 0n) {
      throw new Error('ResolutionCertificateV2 success has a zero result denominator or observation time');
    }
  } else if (certificate.kind === 'resolution-failure') {
    requireNonzero(certificate.fundingAllocation, 'ResolutionCertificateV2 failure funding allocation');
    if (!isZero(certificate.route) || !isZero(certificate.providerEvidence) || certificate.workPaid === 0n
        || certificate.scheduleIndex !== 0 || certificate.resultNumerator !== 0n
        || certificate.resultDenominator !== 0n || certificate.observedAt !== 0n) {
      throw new Error('ResolutionCertificateV2 failure has a noncanonical shape');
    }
  } else {
    requireNonzero(certificate.route, 'ResolutionCertificateV2 liveness route');
    requireNonzero(certificate.fundingAllocation, 'ResolutionCertificateV2 liveness funding allocation');
    if (!isZero(certificate.providerEvidence) || certificate.selector !== 0 || certificate.workPaid === 0n
        || certificate.resultNumerator !== 0n || certificate.resultDenominator !== 0n
        || certificate.observedAt === 0n) {
      throw new Error('ResolutionCertificateV2 liveness transition has a noncanonical shape');
    }
  }
  return certificate;
}

/** Rejoin a decoded terminal certificate to the facts Core independently persisted. */
export function bindTerminalResolutionCertificateV2(
  certificate: ResolutionCertificateV2,
  expected: Readonly<{
    receiptAccount: Uint8Array;
    market: Uint8Array;
    sourceMaterial: Uint8Array;
    productRecordDigest: Uint8Array;
    generation: bigint;
    selector: number;
    outcomeCount: number;
  }>,
): ResolutionCertificateV2 {
  if (!Number.isInteger(expected.selector) || !Number.isInteger(expected.outcomeCount)
      || expected.outcomeCount < 2 || expected.outcomeCount > 0xffff_ffff) {
    throw new Error('terminal Product has an invalid native u32 selector geometry');
  }
  if (!same(certificate.receiptAccount, expected.receiptAccount)
      || !same(certificate.market, expected.market)
      || !same(certificate.sourceMaterial, expected.sourceMaterial)
      || !same(certificate.productRecordDigest, expected.productRecordDigest)
      || certificate.generation !== expected.generation
      || certificate.selector !== expected.selector) {
    throw new Error('ResolutionCertificateV2 differs from Core terminal authority');
  }
  const failureSelector = expected.outcomeCount - 1;
  if ((certificate.kind === 'resolution-success' && certificate.selector >= failureSelector)
      || (certificate.kind === 'resolution-failure' && certificate.selector !== failureSelector)
      || (certificate.kind !== 'resolution-success' && certificate.kind !== 'resolution-failure')) {
    throw new Error('ResolutionCertificateV2 kind and selector do not join the terminal Product');
  }
  return certificate;
}
