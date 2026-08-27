/**
 * `GenericFoundingRequestV1` — the 400-byte body DCLTGMF1's account 0 holds.
 *
 * The outer instruction data is eight ASCII bytes and nothing else, so every
 * economic coordinate of an atomic founding travels in this record. Core
 * reauthenticates each repeated field against its own semantic owner before it
 * creates anything, which means a browser that gets this wire wrong does not
 * get a wrong Market — it gets a refusal. That is the reason it is safe to
 * build here at all, and it is not a reason to be casual: the reserved spans at
 * 12..16 and 394..400 are hostile-checked, and a naive port that leaves them
 * unwritten is indistinguishable from a correct one until a chain sees it.
 *
 * Every width, offset, and discriminant is imported from
 * `lib/generated/genericFoundingV1.ts`, which `scripts/generate-generic-founding.mjs`
 * emits by reading `crates/dclutch-market-core-codec/src/generic_founding_v1.rs`.
 * Nothing in this file restates a number.
 */

import {
  GENERIC_FOUNDING_ACK_BYTES_V1,
  GENERIC_FOUNDING_ACK_IDENTITIES_OFFSET_V1,
  GENERIC_FOUNDING_ACK_MAGIC_OFFSET_V1,
  GENERIC_FOUNDING_ACK_MAGIC_V1,
  GENERIC_FOUNDING_ACK_SCALARS_OFFSET_V1,
  GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1,
  GENERIC_FOUNDING_MAX_FUNDING_STATES_V1,
  GENERIC_FOUNDING_REQUEST_BYTES_V1,
  GENERIC_FOUNDING_REQUEST_ENTRY_INDEX_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_FUNDING_COUNT_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_BYTES_V1,
  GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_IDENTITIES_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_IDENTITIES_V1,
  GENERIC_FOUNDING_REQUEST_MAGIC_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_MAGIC_V1,
  GENERIC_FOUNDING_REQUEST_SCALARS_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_SCALARS_V1,
  GENERIC_FOUNDING_REQUEST_STAGE_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_BYTES_V1,
  GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_VERSION_OFFSET_V1,
  GENERIC_FOUNDING_REQUEST_VERSION_V1,
  GENERIC_FOUNDING_STAGES_V1,
} from '../generated/genericFoundingV1';
import { ascii, hex, isZero, requireZero, sha256, slice, u16, u64 } from '../bytes';

const IDENTITY_BYTES = 32;
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_U16 = 0xffff;

export type GenericFoundingStageNameV1 = (typeof GENERIC_FOUNDING_STAGES_V1)[number]['name'];
export type GenericFoundingIdentityNameV1 = (typeof GENERIC_FOUNDING_REQUEST_IDENTITIES_V1)[number];
export type GenericFoundingScalarNameV1 = (typeof GENERIC_FOUNDING_REQUEST_SCALARS_V1)[number];

/**
 * One founding request, in the shape a caller states it.
 *
 * Identities are lowercase 32-byte hex, not base58: the wire holds
 * `Identity`, which is a content digest for six of the ten fields and a
 * program address for the rest, and spelling both as hex keeps the encoder
 * from silently accepting a Pubkey where a digest belongs.
 */
export type GenericFoundingRequestInputV1 = Readonly<
  { stage: GenericFoundingStageNameV1; fundingCount: number; capabilityEntryIndex: number }
  & Readonly<Record<GenericFoundingIdentityNameV1, string>>
  & Readonly<Record<GenericFoundingScalarNameV1, bigint>>
>;

export type GenericFoundingRequestV1 = GenericFoundingRequestInputV1;

function stageTag(name: GenericFoundingStageNameV1): number {
  const stage = GENERIC_FOUNDING_STAGES_V1.find((candidate) => candidate.name === name);
  if (stage === undefined) throw new Error(`${name} is not a generic founding stage`);
  return stage.tag;
}

function stageName(tag: number): GenericFoundingStageNameV1 {
  const stage = GENERIC_FOUNDING_STAGES_V1.find((candidate) => candidate.tag === tag);
  if (stage === undefined) throw new Error(`generic founding stage tag ${tag} is undefined`);
  return stage.name;
}

function identity(value: string, field: string): Uint8Array {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error(`${field} must be exactly 32 lowercase hexadecimal bytes`);
  const bytes = Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
  if (isZero(bytes)) throw new Error(`${field} is the reserved all-zero identity`);
  return bytes;
}

function scalar(value: bigint, field: string): bigint {
  if (typeof value !== 'bigint' || value < 0n || value > MAX_U64) throw new Error(`${field} is outside u64`);
  return value;
}

/**
 * The exact refusal set `GenericFoundingRequestV1::validate` applies.
 *
 * Restated as one function so the browser refuses at the same boundary the
 * chain does, and so a wizard can show the operator *which* coordinate is
 * inadmissible rather than a generic encode failure. The Rust returns one
 * `Error::InvalidCoordinates` for all of it; naming them costs nothing here
 * and the names never reach the wire.
 */
export function validateGenericFoundingRequestV1(input: GenericFoundingRequestInputV1): void {
  stageTag(input.stage);
  if (!Number.isSafeInteger(input.fundingCount) || input.fundingCount < 1 || input.fundingCount > GENERIC_FOUNDING_MAX_FUNDING_STATES_V1) {
    throw new Error(`funding count must be 1..${GENERIC_FOUNDING_MAX_FUNDING_STATES_V1}`);
  }
  if (!Number.isSafeInteger(input.capabilityEntryIndex) || input.capabilityEntryIndex < 0 || input.capabilityEntryIndex > MAX_U16) {
    throw new Error('capability entry index is outside u16');
  }
  for (const name of GENERIC_FOUNDING_REQUEST_IDENTITIES_V1) identity(input[name], name);
  for (const name of GENERIC_FOUNDING_REQUEST_SCALARS_V1) scalar(input[name], name);
  for (const name of ['generation', 'quantity', 'basisScale', 'expirySlot', 'marketRent', 'permitRent'] as const) {
    if (input[name] === 0n) throw new Error(`${name} must be nonzero`);
  }
  if (input.projectedResultingRevision < 2n) throw new Error('projected resulting revision must be at least 2');
  if (input.quantity * input.basisScale > MAX_U64) throw new Error('quantity times basis scale overflows u64');
  if (input.capabilityRoot === input.context) throw new Error('capability root and context alias');
  if (input.fundingSource === input.hoard) throw new Error('funding source and Hoard alias');
  if (input.projectedReplay === input.hoard) throw new Error('projected replay and Hoard alias');
  if (input.projectedReplay === input.fundingSource) throw new Error('projected replay and funding source alias');
}

/** Encode the sole canonical fixed request. */
export function encodeGenericFoundingRequestV1(input: GenericFoundingRequestInputV1): Uint8Array {
  validateGenericFoundingRequestV1(input);
  const output = new Uint8Array(GENERIC_FOUNDING_REQUEST_BYTES_V1);
  const view = new DataView(output.buffer);
  output.set(new TextEncoder().encode(GENERIC_FOUNDING_REQUEST_MAGIC_V1), GENERIC_FOUNDING_REQUEST_MAGIC_OFFSET_V1);
  view.setUint16(GENERIC_FOUNDING_REQUEST_VERSION_OFFSET_V1, GENERIC_FOUNDING_REQUEST_VERSION_V1, true);
  output[GENERIC_FOUNDING_REQUEST_STAGE_OFFSET_V1] = stageTag(input.stage);
  output[GENERIC_FOUNDING_REQUEST_FUNDING_COUNT_OFFSET_V1] = input.fundingCount;
  GENERIC_FOUNDING_REQUEST_IDENTITIES_V1.forEach((name, index) => {
    output.set(identity(input[name], name), GENERIC_FOUNDING_REQUEST_IDENTITIES_OFFSET_V1 + index * IDENTITY_BYTES);
  });
  GENERIC_FOUNDING_REQUEST_SCALARS_V1.forEach((name, index) => {
    view.setBigUint64(GENERIC_FOUNDING_REQUEST_SCALARS_OFFSET_V1 + index * 8, input[name], true);
  });
  view.setUint16(GENERIC_FOUNDING_REQUEST_ENTRY_INDEX_OFFSET_V1, input.capabilityEntryIndex, true);
  return output;
}

/** Hostile-decode one exact fixed request. */
export function decodeGenericFoundingRequestV1(bytes: Uint8Array): GenericFoundingRequestV1 {
  if (bytes.length !== GENERIC_FOUNDING_REQUEST_BYTES_V1 || ascii(bytes, GENERIC_FOUNDING_REQUEST_MAGIC_OFFSET_V1, GENERIC_FOUNDING_REQUEST_MAGIC_V1.length) !== GENERIC_FOUNDING_REQUEST_MAGIC_V1) {
    throw new Error('generic founding request has the wrong exact ABI');
  }
  if (u16(bytes, GENERIC_FOUNDING_REQUEST_VERSION_OFFSET_V1) !== GENERIC_FOUNDING_REQUEST_VERSION_V1) {
    throw new Error('generic founding request states an unsupported schema version');
  }
  requireZero(bytes, GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1, GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_BYTES_V1, 'generic founding request header');
  requireZero(bytes, GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_OFFSET_V1, GENERIC_FOUNDING_REQUEST_TAIL_RESERVED_BYTES_V1, 'generic founding request tail');
  const identities = Object.fromEntries(GENERIC_FOUNDING_REQUEST_IDENTITIES_V1.map((name, index) => [
    name,
    hex(slice(bytes, GENERIC_FOUNDING_REQUEST_IDENTITIES_OFFSET_V1 + index * IDENTITY_BYTES, IDENTITY_BYTES)),
  ]));
  const scalars = Object.fromEntries(GENERIC_FOUNDING_REQUEST_SCALARS_V1.map((name, index) => [
    name,
    u64(bytes, GENERIC_FOUNDING_REQUEST_SCALARS_OFFSET_V1 + index * 8),
  ]));
  const decoded = {
    stage: stageName(bytes[GENERIC_FOUNDING_REQUEST_STAGE_OFFSET_V1]),
    fundingCount: bytes[GENERIC_FOUNDING_REQUEST_FUNDING_COUNT_OFFSET_V1],
    capabilityEntryIndex: u16(bytes, GENERIC_FOUNDING_REQUEST_ENTRY_INDEX_OFFSET_V1),
    ...identities,
    ...scalars,
  } as GenericFoundingRequestV1;
  validateGenericFoundingRequestV1(decoded);
  return Object.freeze(decoded);
}

/**
 * Move one request to another stage, exactly as `with_stage` does.
 *
 * The Open request is the Found request with one byte changed, which is why
 * the acknowledgement cross-checks the stage: two requests that differ only
 * there must not be interchangeable.
 */
export function withGenericFoundingStageV1(request: GenericFoundingRequestV1, stage: GenericFoundingStageNameV1): GenericFoundingRequestV1 {
  return Object.freeze({ ...request, stage });
}

export type GenericFoundingAckV1 = Readonly<{
  stage: GenericFoundingStageNameV1;
  fundingCount: number;
  coreProgram: string;
  releaseSet: string;
  market: string;
  permit: string;
  requestDigest: string;
  postResourceDigest: string;
  fundingListId: string;
  generation: bigint;
}>;

/** Hostile-decode the 248-byte return data an executed stage sets. */
export function decodeGenericFoundingAckV1(bytes: Uint8Array): GenericFoundingAckV1 {
  if (bytes.length !== GENERIC_FOUNDING_ACK_BYTES_V1 || ascii(bytes, GENERIC_FOUNDING_ACK_MAGIC_OFFSET_V1, GENERIC_FOUNDING_ACK_MAGIC_V1.length) !== GENERIC_FOUNDING_ACK_MAGIC_V1) {
    throw new Error('generic founding acknowledgement has the wrong exact ABI');
  }
  if (u16(bytes, GENERIC_FOUNDING_REQUEST_VERSION_OFFSET_V1) !== GENERIC_FOUNDING_REQUEST_VERSION_V1) {
    throw new Error('generic founding acknowledgement states an unsupported schema version');
  }
  requireZero(bytes, GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_OFFSET_V1, GENERIC_FOUNDING_REQUEST_HEAD_RESERVED_BYTES_V1, 'generic founding acknowledgement header');
  const names = ['coreProgram', 'releaseSet', 'market', 'permit', 'requestDigest', 'postResourceDigest', 'fundingListId'] as const;
  const identities = Object.fromEntries(names.map((name, index) => [
    name,
    hex(slice(bytes, GENERIC_FOUNDING_ACK_IDENTITIES_OFFSET_V1 + index * IDENTITY_BYTES, IDENTITY_BYTES)),
  ])) as Record<(typeof names)[number], string>;
  return Object.freeze({
    stage: stageName(bytes[GENERIC_FOUNDING_REQUEST_STAGE_OFFSET_V1]),
    fundingCount: bytes[GENERIC_FOUNDING_REQUEST_FUNDING_COUNT_OFFSET_V1],
    ...identities,
    generation: u64(bytes, GENERIC_FOUNDING_ACK_SCALARS_OFFSET_V1),
  });
}

/**
 * Hash one exact ordered, nonempty, alias-free FundingState address list.
 *
 * Preimage is `domain || 0x00 || u16_le(count) || key...`. This is the one
 * derivation in the request whose value a caller cannot copy from anywhere
 * else: `fundingListId` commits to the FundingState accounts the Found stage
 * will be handed, in order, so a reordered tail is a different request.
 */
export async function genericFoundingFundingListIdV1(fundingStates: ReadonlyArray<string>): Promise<string> {
  if (fundingStates.length === 0 || fundingStates.length > GENERIC_FOUNDING_MAX_FUNDING_STATES_V1) {
    throw new Error(`funding list must name 1..${GENERIC_FOUNDING_MAX_FUNDING_STATES_V1} FundingState accounts`);
  }
  if (new Set(fundingStates).size !== fundingStates.length) throw new Error('funding list aliases a FundingState account');
  const preimage = new Uint8Array(GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1.length + 1 + 2 + fundingStates.length * IDENTITY_BYTES);
  preimage.set(GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1, 0);
  const countOffset = GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1.length + 1;
  new DataView(preimage.buffer).setUint16(countOffset, fundingStates.length, true);
  fundingStates.forEach((entry, index) => {
    preimage.set(identity(entry, `funding state ${index}`), countOffset + 2 + index * IDENTITY_BYTES);
  });
  return hex(await sha256(preimage));
}
