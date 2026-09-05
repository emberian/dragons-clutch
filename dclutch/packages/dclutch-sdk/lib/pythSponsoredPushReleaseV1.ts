import { PublicKey } from '@solana/web3.js';

import { ascii, fromHex, hex, pubkey, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import {
  PYTH_SPONSORED_PUSH_RELEASE_MAGIC_V1,
} from './generated/protocolConstantsV1';

export const PYTH_SPONSORED_PUSH_RELEASE_BYTES_V1 = 592;
export { PYTH_SPONSORED_PUSH_RELEASE_MAGIC_V1 };
export const PYTH_SPONSORED_PUSH_RELEASE_VERSION_V1 = 1;
export const PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1 =
  'c843f534616a9bcad09c589ebfa80a3163584e5ef1cbf3fcbd6b13568c7ae182';

export type PythSponsoredPushReleaseV1 = Readonly<{
  bytes: Uint8Array;
  clusterId: string;
  receiverProgram: string;
  receiverProgramData: string;
  receiverAbiId: string;
  receiverUpgradeAuthority: string;
  pushOracleProgram: string;
  pushOracleProgramData: string;
  pushOracleAbiId: string;
  pushOracleUpgradeAuthority: string;
  receiverConfig: string;
  receiverConfigDigest: string;
  priceAccount: string;
  feedId: string;
  priceUpdateCodecId: string;
  adapterId: string;
  providerFamilyId: string;
  transportProfileId: string;
  receiverDeploymentSlot: bigint;
  pushOracleDeploymentSlot: bigint;
  shard: number;
  feedAccountBump: number;
  activationTime: bigint;
}>;

export type PythSponsoredPushReleaseInputV1 = Readonly<Omit<PythSponsoredPushReleaseV1, 'bytes'>>;

export type PythSponsoredPushReleaseRecordV1 = Readonly<{
  schemaId: string;
  recordId: string;
  body: Uint8Array;
  release: PythSponsoredPushReleaseV1;
}>;

const PUBLIC_KEY_FIELDS = [
  ['receiverProgram', 48],
  ['receiverProgramData', 80],
  ['receiverUpgradeAuthority', 432],
  ['pushOracleProgram', 144],
  ['pushOracleProgramData', 176],
  ['pushOracleUpgradeAuthority', 464],
  ['receiverConfig', 496],
  ['priceAccount', 240],
] as const;

const IDENTITY_FIELDS = [
  ['clusterId', 16],
  ['receiverAbiId', 112],
  ['pushOracleAbiId', 208],
  ['feedId', 272],
  ['priceUpdateCodecId', 304],
  ['adapterId', 336],
  ['providerFamilyId', 368],
  ['transportProfileId', 400],
  ['receiverConfigDigest', 528],
] as const;

function canonicalPublicKey(value: string, field: string): Uint8Array {
  let key: PublicKey;
  try {
    key = new PublicKey(value);
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (key.toBase58() !== value) throw new Error(`${field} must use canonical base58 text`);
  const bytes = key.toBytes();
  requireNonzero(bytes, field);
  return bytes;
}

function canonicalU64(value: bigint, field: string): bigint {
  if (value <= 0n || value > 0xffff_ffff_ffff_ffffn) {
    throw new Error(`${field} must be one positive u64`);
  }
  return value;
}

function canonicalI64(value: bigint, field: string): bigint {
  if (value < -0x8000_0000_0000_0000n || value > 0x7fff_ffff_ffff_ffffn) {
    throw new Error(`${field} is outside signed i64 range`);
  }
  return value;
}

function canonicalU16(value: number, field: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xffff) {
    throw new Error(`${field} must be one u16`);
  }
  return value;
}

function canonicalU8(value: number, field: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xff) {
    throw new Error(`${field} must be one u8`);
  }
  return value;
}

function i64(bytes: Uint8Array, offset: number): bigint {
  const value = slice(bytes, offset, 8);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getBigInt64(0, true);
}

/** Hostile-decode the exact fixed-layout Rust `PythSponsoredPushReleaseV1`. */
export function decodePythSponsoredPushReleaseV1(bytes: Uint8Array): PythSponsoredPushReleaseV1 {
  if (bytes.length !== PYTH_SPONSORED_PUSH_RELEASE_BYTES_V1
      || ascii(bytes, 0, 8) !== PYTH_SPONSORED_PUSH_RELEASE_MAGIC_V1
      || u16(bytes, 8) !== PYTH_SPONSORED_PUSH_RELEASE_VERSION_V1) {
    throw new Error('PythSponsoredPushReleaseV1 has the wrong exact ABI');
  }
  requireZero(bytes, 10, 6, 'PythSponsoredPushReleaseV1 header');
  requireZero(bytes, 579, 5, 'PythSponsoredPushReleaseV1 body');
  for (const [field, offset] of [...PUBLIC_KEY_FIELDS, ...IDENTITY_FIELDS]) {
    requireNonzero(slice(bytes, offset, 32), `PythSponsoredPushReleaseV1 ${field}`);
  }
  const receiverDeploymentSlot = u64(bytes, 560);
  const pushOracleDeploymentSlot = u64(bytes, 568);
  if (receiverDeploymentSlot === 0n || pushOracleDeploymentSlot === 0n) {
    throw new Error('PythSponsoredPushReleaseV1 has a zero deployment slot');
  }
  return Object.freeze({
    bytes: bytes.slice(),
    clusterId: hex(slice(bytes, 16, 32)),
    receiverProgram: pubkey(slice(bytes, 48, 32), 'Pyth Receiver program'),
    receiverProgramData: pubkey(slice(bytes, 80, 32), 'Pyth Receiver ProgramData'),
    receiverAbiId: hex(slice(bytes, 112, 32)),
    receiverUpgradeAuthority: pubkey(slice(bytes, 432, 32), 'Pyth Receiver upgrade authority'),
    pushOracleProgram: pubkey(slice(bytes, 144, 32), 'Pyth push-oracle program'),
    pushOracleProgramData: pubkey(slice(bytes, 176, 32), 'Pyth push-oracle ProgramData'),
    pushOracleAbiId: hex(slice(bytes, 208, 32)),
    pushOracleUpgradeAuthority: pubkey(slice(bytes, 464, 32), 'Pyth push-oracle upgrade authority'),
    receiverConfig: pubkey(slice(bytes, 496, 32), 'Pyth Receiver config'),
    receiverConfigDigest: hex(slice(bytes, 528, 32)),
    priceAccount: pubkey(slice(bytes, 240, 32), 'Pyth sponsored price account'),
    feedId: hex(slice(bytes, 272, 32)),
    priceUpdateCodecId: hex(slice(bytes, 304, 32)),
    adapterId: hex(slice(bytes, 336, 32)),
    providerFamilyId: hex(slice(bytes, 368, 32)),
    transportProfileId: hex(slice(bytes, 400, 32)),
    receiverDeploymentSlot,
    pushOracleDeploymentSlot,
    shard: u16(bytes, 576),
    feedAccountBump: bytes[578] ?? 0,
    activationTime: i64(bytes, 584),
  } satisfies PythSponsoredPushReleaseV1);
}

/** Encode one canonical sponsored-push release body. This builds no transaction. */
export function encodePythSponsoredPushReleaseV1(input: PythSponsoredPushReleaseInputV1): Uint8Array {
  const bytes = new Uint8Array(PYTH_SPONSORED_PUSH_RELEASE_BYTES_V1);
  bytes.set(new TextEncoder().encode(PYTH_SPONSORED_PUSH_RELEASE_MAGIC_V1), 0);
  new DataView(bytes.buffer).setUint16(8, PYTH_SPONSORED_PUSH_RELEASE_VERSION_V1, true);
  for (const [field, offset] of PUBLIC_KEY_FIELDS) {
    bytes.set(canonicalPublicKey(input[field], `PythSponsoredPushReleaseV1 ${field}`), offset);
  }
  for (const [field, offset] of IDENTITY_FIELDS) {
    const identity = fromHex(input[field], `PythSponsoredPushReleaseV1 ${field}`);
    requireNonzero(identity, `PythSponsoredPushReleaseV1 ${field}`);
    bytes.set(identity, offset);
  }
  const view = new DataView(bytes.buffer);
  view.setBigUint64(560, canonicalU64(input.receiverDeploymentSlot, 'Receiver deployment slot'), true);
  view.setBigUint64(568, canonicalU64(input.pushOracleDeploymentSlot, 'push-oracle deployment slot'), true);
  view.setUint16(576, canonicalU16(input.shard, 'sponsored push shard'), true);
  bytes[578] = canonicalU8(input.feedAccountBump, 'sponsored push feed-account bump');
  view.setBigInt64(584, canonicalI64(input.activationTime, 'sponsored push activation time'), true);
  return bytes;
}

/** Build the exact Registry body/id pair the ProviderRelease must name. */
export async function buildPythSponsoredPushReleaseRecordV1(
  input: PythSponsoredPushReleaseInputV1,
): Promise<PythSponsoredPushReleaseRecordV1> {
  const body = encodePythSponsoredPushReleaseV1(input);
  return Object.freeze({
    schemaId: PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
    recordId: hex(await sha256(body)),
    body,
    release: decodePythSponsoredPushReleaseV1(body),
  });
}
