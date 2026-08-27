import {
  ComputeBudgetProgram,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { PACKET_DATA_SIZE } from './directTransaction';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const CHECKED_MULTIPROGRAM_BYTES = 1_592;
export const CHECKED_RELEASE_FIXED_BYTES = 388;
export const EXECUTION_RELEASE_SET_BYTES = 336;
export const ARTIFACT_RELEASE_BYTES = 216;
export const ACTIVATION_CACHE_BYTES = 1_288;
export const REGISTRY_INSTRUCTION_BYTES = 16;
export const REGISTRY_ACTIVATE_ACCOUNT_COUNT = 26;
export const REGISTRY_REAUTH_ACCOUNT_COUNT = 3;
export const REGISTRY_MAX_COMPUTE_UNITS = 1_400_000;
export const LOADER_V3_PROGRAM_BYTES = 36;
export const LOADER_V3_PROGRAMDATA_OFFSET = 45;

export const UPGRADEABLE_LOADER_ID = 'BPFLoaderUpgradeab1e11111111111111111111111';
export const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111';
export const NATIVE_LOADER_ID = 'NativeLoader1111111111111111111111111111111';
export const RENT_SYSVAR_ID = 'SysvarRent111111111111111111111111111111111';
export const SYSVAR_OWNER_ID = 'Sysvar1111111111111111111111111111111';

const RAW_RECORD_SEED = new TextEncoder().encode('dclutch-raw-record-v1');
const STAGING_RECORD_SEED = new TextEncoder().encode('dclutch-record-stage-v1');
const ACTIVATION_SEED = new TextEncoder().encode('dclutch:release-activation:v1');
const EXECUTION_RELEASE_SET_SCHEMA = Uint8Array.from([
  0x8b, 0xa3, 0xbc, 0x19, 0x7f, 0xea, 0xa1, 0x87, 0xa0, 0xa3, 0x92, 0x7b, 0x16, 0xb2, 0x5d, 0x83,
  0x79, 0x2c, 0x5f, 0x33, 0x5a, 0xf2, 0x43, 0x39, 0xa5, 0x4c, 0x38, 0xcc, 0x07, 0x23, 0x03, 0x58,
]);
export const ARTIFACT_RELEASE_SCHEMA_ID_V1 = Uint8Array.from([
  0xae, 0x19, 0xa6, 0x0d, 0xb5, 0x50, 0xb1, 0xa8, 0xa5, 0x1d, 0x46, 0x18, 0xc7, 0x7d, 0xea, 0x54,
  0x21, 0x17, 0x4a, 0x2a, 0x85, 0x5e, 0xe6, 0x77, 0x89, 0x4f, 0xa9, 0x1b, 0x3c, 0xfd, 0x3b, 0x6c,
]);

export const REGISTRY_ROLES = ['core', 'claims', 'trading', 'resolution', 'custody'] as const;
export type RegistryRole = typeof REGISTRY_ROLES[number];

export type ArtifactReleaseV1 = Readonly<{
  bytes: Uint8Array;
  program: string;
  loader: string;
  programData: string;
  semanticReleaseId: string;
  elfDigest: string;
  deploymentSlot: bigint;
  upgradeAuthority: string | null;
}>;

export type CheckedReleaseV1 = Readonly<{
  bytes: Uint8Array;
  checkedReleaseId: string;
  artifact: ArtifactReleaseV1;
  programDigest: string;
  programDataDigest: string;
  programBytes: bigint;
  programDataBytes: bigint;
  elfBytes: bigint;
  sourceRevision: string;
  buildCommand: string;
  assumptions: ReadonlyArray<string>;
}>;

export type ReleaseBindingV1 = Readonly<{ program: string; artifactReleaseId: string }>;
export type ExecutionReleaseSetV1 = Readonly<{
  bytes: Uint8Array;
  id: string;
  roles: Readonly<Record<RegistryRole, ReleaseBindingV1>>;
}>;

export type CheckedMultiprogramV1 = Readonly<{
  bytes: Uint8Array;
  checkedId: string;
  releaseSet: ExecutionReleaseSetV1;
  artifacts: Readonly<Record<RegistryRole, ArtifactReleaseV1>>;
  checkedReleaseIds: Readonly<Record<RegistryRole, string>>;
}>;

export type RegistryRoleAddressesV1 = Readonly<{
  record: string;
  staging: string;
  program: string;
  programData: string;
}>;

export type RegistryActivationPlanV1 = Readonly<{
  observedSlot: string;
  registryProgram: string;
  payer: string;
  cache: string;
  mode: 'create' | 'repeat';
  cacheRentDebitLamports: string;
  releaseSetRecord: string;
  releaseSetStaging: string;
  roles: Readonly<Record<RegistryRole, RegistryRoleAddressesV1>>;
  evidence: CheckedMultiprogramV1;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  computeUnitLimit: number;
}>;

export type RegistryReauthenticationPlanV1 = Readonly<{
  observedSlot: string;
  registryProgram: string;
  payer: string;
  cache: string;
  role: RegistryRole;
  releaseSetId: string;
  artifactReleaseId: string;
  artifact: ArtifactReleaseV1;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  computeUnitLimit: number;
}>;

type RegistryRpc = Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'minimumBalanceForRentExemption' | 'latestBlockhash'>;

function key(text: string, field: string): PublicKey {
  const value = new PublicKey(text);
  if (value.toBase58() !== text) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function allNonzero(values: ReadonlyArray<Uint8Array>, field: string): void {
  for (const value of values) requireNonzero(value, field);
}

function roleRecord<T>(values: ReadonlyArray<T>): Record<RegistryRole, T> {
  if (values.length !== REGISTRY_ROLES.length) throw new Error('five-role projection is incomplete');
  return Object.fromEntries(REGISTRY_ROLES.map((role, index) => [role, values[index]])) as Record<RegistryRole, T>;
}

function canonicalBase64(text: string, field: string): Uint8Array {
  if (text.trim() !== text || text.length === 0 || text.length > 8_000_000 || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(text)) {
    throw new Error(`${field} must be bounded canonical base64`);
  }
  const bytes = Uint8Array.from(atob(text), (value) => value.charCodeAt(0));
  let rebuilt = '';
  for (let offset = 0; offset < bytes.length; offset += 16_384) rebuilt += String.fromCharCode(...bytes.slice(offset, offset + 16_384));
  if (btoa(rebuilt) !== text) throw new Error(`${field} base64 is noncanonical`);
  return bytes;
}

export function decodeManifestBase64(text: string, field: string): Uint8Array {
  return canonicalBase64(text, field);
}

function asciiText(bytes: Uint8Array, offset: number, field: string): Readonly<{ value: string; next: number }> {
  if (offset + 2 > bytes.length) throw new Error(`${field} length prefix is truncated`);
  const width = u16(bytes, offset);
  const next = offset + 2 + width;
  if (width === 0 || width > 4_096 || next > bytes.length) throw new Error(`${field} length is noncanonical`);
  const valueBytes = bytes.slice(offset + 2, next);
  if (valueBytes.some((byte) => byte < 0x20 || byte > 0x7e)) throw new Error(`${field} must be printable single-line ASCII`);
  return Object.freeze({ value: new TextDecoder('ascii', { fatal: true }).decode(valueBytes), next });
}

function artifactBytesFromChecked(bytes: Uint8Array): Uint8Array {
  const output = new Uint8Array(ARTIFACT_RELEASE_BYTES);
  output.set(new TextEncoder().encode('DCLTARF1'), 0);
  new DataView(output.buffer).setUint16(8, 1, true);
  new DataView(output.buffer).setUint16(10, 1, true);
  output[12] = bytes[12] === 0 ? 0 : 1;
  output.set(slice(bytes, 196, 32), 16);
  output.set(slice(bytes, 260, 32), 48);
  output.set(slice(bytes, 228, 32), 80);
  output.set(slice(bytes, 100, 32), 112);
  output.set(slice(bytes, 68, 32), 144);
  output.set(slice(bytes, 52, 8), 176);
  output.set(slice(bytes, 292, 32), 184);
  return output;
}

export function decodeArtifactReleaseV1(bytes: Uint8Array): ArtifactReleaseV1 {
  if (bytes.length !== ARTIFACT_RELEASE_BYTES || ascii(bytes, 0, 8) !== 'DCLTARF1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) {
    throw new Error('artifact release has the wrong exact width, magic, schema, or profile');
  }
  requireZero(bytes, 13, 3, 'artifact release header');
  const policy = bytes[12];
  if (policy !== 0 && policy !== 1) throw new Error('artifact release upgrade policy is undefined');
  const program = slice(bytes, 16, 32);
  const loader = slice(bytes, 48, 32);
  const programData = slice(bytes, 80, 32);
  const semantic = slice(bytes, 112, 32);
  const elf = slice(bytes, 144, 32);
  const authority = slice(bytes, 184, 32);
  allNonzero([program, loader, programData, semantic, elf], 'artifact release identity');
  if (same(program, loader) || same(program, programData) || same(loader, programData)) throw new Error('artifact release aliases Loader identities');
  if ((policy === 0 && !isZero(authority)) || (policy === 1 && isZero(authority))) throw new Error('artifact release upgrade authority is noncanonical');
  return Object.freeze({
    bytes: new Uint8Array(bytes), program: new PublicKey(program).toBase58(), loader: new PublicKey(loader).toBase58(),
    programData: new PublicKey(programData).toBase58(), semanticReleaseId: hex(semantic), elfDigest: hex(elf),
    deploymentSlot: u64(bytes, 176), upgradeAuthority: policy === 0 ? null : new PublicKey(authority).toBase58(),
  });
}

export async function decodeCheckedReleaseV1(bytes: Uint8Array): Promise<CheckedReleaseV1> {
  if (bytes.length < CHECKED_RELEASE_FIXED_BYTES || ascii(bytes, 0, 8) !== 'DCLTREL1' || u16(bytes, 8) !== 1) throw new Error('checked release has the wrong fixed header');
  if (bytes[10] > 1 || bytes[11] !== 1 || bytes[12] > 1) throw new Error('checked release names an unsupported semantic, Loader, or authority kind');
  requireZero(bytes, 14, 2, 'checked release header');
  if (new DataView(bytes.buffer, bytes.byteOffset + 16, 4).getUint32(0, true) !== bytes.length) throw new Error('checked release declared length does not equal its exact bytes');
  const elfBytes = u64(bytes, 28); const programBytes = u64(bytes, 36); const programDataBytes = u64(bytes, 44);
  if (u64(bytes, 20) === 0n || elfBytes < 64n || programBytes !== 36n || u64(bytes, 60) !== 45n || programDataBytes < 45n + elfBytes || programDataBytes > 10_485_760n) throw new Error('checked release carries unsupported Loader-v3 geometry');
  const identities = [68, 100, 132, 164, 196, 228, 260, 324, 356].map((offset) => slice(bytes, offset, 32));
  allNonzero(identities, 'checked release identity');
  if (same(identities[4], identities[5]) || same(identities[4], identities[6]) || same(identities[5], identities[6])) throw new Error('checked release aliases Loader identities');
  const authority = slice(bytes, 292, 32);
  if ((bytes[12] === 0 && !isZero(authority)) || (bytes[12] === 1 && isZero(authority))) throw new Error('checked release upgrade authority is noncanonical');
  let offset = CHECKED_RELEASE_FIXED_BYTES;
  const texts: string[] = [];
  for (let index = 0; index < 6; index += 1) { const decoded = asciiText(bytes, offset, `checked release metadata ${index}`); texts.push(decoded.value); offset = decoded.next; }
  const assumptions: string[] = [];
  for (let index = 0; index < bytes[13]; index += 1) { const decoded = asciiText(bytes, offset, `checked release assumption ${index}`); assumptions.push(decoded.value); offset = decoded.next; }
  if (offset !== bytes.length || assumptions.length === 0 || assumptions.some((value, index) => index > 0 && value <= assumptions[index - 1])) throw new Error('checked release assumptions or trailing bytes are noncanonical');
  const artifact = decodeArtifactReleaseV1(artifactBytesFromChecked(bytes));
  return Object.freeze({
    bytes: new Uint8Array(bytes), checkedReleaseId: hex(await sha256(bytes)), artifact,
    programDigest: hex(identities[2]), programDataDigest: hex(identities[3]), programBytes, programDataBytes, elfBytes,
    sourceRevision: texts[0], buildCommand: texts[5], assumptions: Object.freeze(assumptions),
  });
}

export async function decodeExecutionReleaseSetV1(bytes: Uint8Array): Promise<ExecutionReleaseSetV1> {
  if (bytes.length !== EXECUTION_RELEASE_SET_BYTES || ascii(bytes, 0, 8) !== 'DCLTRLS1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) throw new Error('execution release set has the wrong exact header');
  requireZero(bytes, 12, 4, 'execution release set header');
  const bindings = REGISTRY_ROLES.map((role, index) => {
    const program = slice(bytes, 16 + index * 64, 32); const artifact = slice(bytes, 48 + index * 64, 32);
    allNonzero([program, artifact], `${role} release binding`);
    return Object.freeze({ program: new PublicKey(program).toBase58(), artifactReleaseId: hex(artifact) });
  });
  for (let left = 0; left < bindings.length; left += 1) for (let right = left + 1; right < bindings.length; right += 1) {
    const programAlias = bindings[left].program === bindings[right].program; const artifactAlias = bindings[left].artifactReleaseId === bindings[right].artifactReleaseId;
    if (programAlias !== artifactAlias) throw new Error('release set contains an inconsistent partially aliased role binding');
  }
  return Object.freeze({ bytes: new Uint8Array(bytes), id: hex(await sha256(bytes)), roles: Object.freeze(roleRecord(bindings)) });
}

export async function decodeCheckedMultiprogramV1(bytes: Uint8Array, checkedBytes: Readonly<Record<RegistryRole, Uint8Array>>): Promise<CheckedMultiprogramV1> {
  if (bytes.length !== CHECKED_MULTIPROGRAM_BYTES || ascii(bytes, 0, 8) !== 'DCLTMPR1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 5) throw new Error('checked multiprogram has the wrong exact header');
  requireZero(bytes, 12, 4, 'checked multiprogram header');
  const releaseSet = await decodeExecutionReleaseSetV1(slice(bytes, 16, EXECUTION_RELEASE_SET_BYTES));
  const artifacts: ArtifactReleaseV1[] = []; const checkedReleaseIds: string[] = [];
  const checked = await Promise.all(REGISTRY_ROLES.map((role) => decodeCheckedReleaseV1(checkedBytes[role])));
  for (let index = 0; index < REGISTRY_ROLES.length; index += 1) {
    const role = REGISTRY_ROLES[index]; const offset = 352 + index * 248;
    const artifact = decodeArtifactReleaseV1(slice(bytes, offset, ARTIFACT_RELEASE_BYTES));
    const artifactId = hex(await sha256(artifact.bytes)); const checkedReleaseId = hex(slice(bytes, offset + ARTIFACT_RELEASE_BYTES, 32));
    if (artifact.program !== releaseSet.roles[role].program || artifactId !== releaseSet.roles[role].artifactReleaseId) throw new Error(`${role} artifact does not implement its release-set binding`);
    if (checked[index].checkedReleaseId !== checkedReleaseId || !same(checked[index].artifact.bytes, artifact.bytes)) throw new Error(`${role} full checked release does not rebuild the multiprogram evidence`);
    artifacts.push(artifact); checkedReleaseIds.push(checkedReleaseId);
  }
  return Object.freeze({ bytes: new Uint8Array(bytes), checkedId: hex(await sha256(bytes)), releaseSet, artifacts: Object.freeze(roleRecord(artifacts)), checkedReleaseIds: Object.freeze(roleRecord(checkedReleaseIds)) });
}

function recordPdas(registry: PublicKey, schema: Uint8Array, digest: Uint8Array): Readonly<{ record: string; staging: string }> {
  return Object.freeze({
    record: PublicKey.findProgramAddressSync([RAW_RECORD_SEED, schema, digest], registry)[0].toBase58(),
    staging: PublicKey.findProgramAddressSync([STAGING_RECORD_SEED, schema, digest], registry)[0].toBase58(),
  });
}

export function deriveFinalizedRecordAddressesV1(registryProgram: string, schema: Uint8Array, digest: Uint8Array): Readonly<{ record: string; staging: string }> {
  if (schema.length !== 32 || digest.length !== 32 || isZero(schema) || isZero(digest)) throw new Error('finalized record schema and digest must be nonzero 32-byte identities');
  return recordPdas(key(registryProgram, 'Registry program'), schema, digest);
}

function expectedCacheBytes(evidence: CheckedMultiprogramV1): Uint8Array {
  const output = new Uint8Array(ACTIVATION_CACHE_BYTES); output.set(new TextEncoder().encode('DCLTACT1'));
  const view = new DataView(output.buffer); view.setUint16(8, 1, true); view.setUint16(10, 1, true);
  output.set(Uint8Array.from(evidence.releaseSet.id.match(/../g) ?? [], (value) => Number.parseInt(value, 16)), 16);
  REGISTRY_ROLES.forEach((role, index) => {
    const offset = 48 + index * 248; output.set(Uint8Array.from(evidence.releaseSet.roles[role].artifactReleaseId.match(/../g) ?? [], (value) => Number.parseInt(value, 16)), offset); output.set(evidence.artifacts[role].bytes, offset + 32);
  });
  return output;
}

function parseCache(bytes: Uint8Array, registryProgram: string, cacheAddress: string): Readonly<{ releaseSetId: string; artifacts: Readonly<Record<RegistryRole, ArtifactReleaseV1>>; artifactIds: Readonly<Record<RegistryRole, string>> }> {
  if (bytes.length !== ACTIVATION_CACHE_BYTES || ascii(bytes, 0, 8) !== 'DCLTACT1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) throw new Error('activation cache has the wrong exact header');
  requireZero(bytes, 12, 4, 'activation cache header'); const releaseSetId = hex(slice(bytes, 16, 32)); requireNonzero(slice(bytes, 16, 32), 'activation release-set identity');
  const registry = key(registryProgram, 'Registry program'); const derived = PublicKey.findProgramAddressSync([ACTIVATION_SEED, slice(bytes, 16, 32)], registry)[0].toBase58();
  if (derived !== cacheAddress) throw new Error('activation cache is not the release-derived Registry PDA');
  const artifacts: ArtifactReleaseV1[] = []; const artifactIds: string[] = [];
  for (let index = 0; index < REGISTRY_ROLES.length; index += 1) { const offset = 48 + index * 248; const id = slice(bytes, offset, 32); requireNonzero(id, 'cached artifact release identity'); const artifact = decodeArtifactReleaseV1(slice(bytes, offset + 32, ARTIFACT_RELEASE_BYTES)); artifacts.push(artifact); artifactIds.push(hex(id)); }
  if (artifacts[0].program !== registryProgram) throw new Error('activation cache Core role does not select this Registry program');
  for (let left = 0; left < artifacts.length; left += 1) for (let right = left + 1; right < artifacts.length; right += 1) {
    const programAlias = artifacts[left].program === artifacts[right].program; const artifactAlias = artifactIds[left] === artifactIds[right];
    if (programAlias !== artifactAlias) throw new Error('activation cache contains an inconsistent partially aliased role binding');
    if (programAlias && !same(artifacts[left].bytes, artifacts[right].bytes)) throw new Error('activation cache aliases one role pair to different artifact bytes');
  }
  return Object.freeze({ releaseSetId, artifacts: Object.freeze(roleRecord(artifacts)), artifactIds: Object.freeze(roleRecord(artifactIds)) });
}

function programDataView(program: RpcAccount, programAddress: string, programData: RpcAccount, programDataAddress: string, artifact: ArtifactReleaseV1): Uint8Array {
  if (program.owner !== UPGRADEABLE_LOADER_ID || !program.executable || program.data.length !== LOADER_V3_PROGRAM_BYTES || new DataView(program.data.buffer, program.data.byteOffset, 4).getUint32(0, true) !== 2) throw new Error(`${programAddress} is not an exact Loader-v3 Program account`);
  const link = new PublicKey(slice(program.data, 4, 32)).toBase58(); const derived = PublicKey.findProgramAddressSync([key(programAddress, 'role Program').toBytes()], key(UPGRADEABLE_LOADER_ID, 'Upgradeable Loader'))[0].toBase58();
  if (link !== programDataAddress || derived !== programDataAddress || artifact.program !== programAddress || artifact.programData !== programDataAddress || artifact.loader !== UPGRADEABLE_LOADER_ID) throw new Error('ProgramData link, PDA, or artifact Loader identity does not join');
  if (programData.owner !== UPGRADEABLE_LOADER_ID || programData.executable || programData.data.length <= LOADER_V3_PROGRAMDATA_OFFSET || new DataView(programData.data.buffer, programData.data.byteOffset, 4).getUint32(0, true) !== 3) throw new Error(`${programDataAddress} is not an exact Loader-v3 ProgramData account`);
  const tag = programData.data[12]; if (tag > 1 || (tag === 0 && !isZero(slice(programData.data, 13, 32)))) throw new Error('ProgramData upgrade-authority encoding is noncanonical');
  const authority = tag === 0 ? null : new PublicKey(slice(programData.data, 13, 32)).toBase58();
  if (u64(programData.data, 4) !== artifact.deploymentSlot || authority !== artifact.upgradeAuthority) throw new Error('ProgramData slot or authority differs from the artifact release');
  return slice(programData.data, LOADER_V3_PROGRAMDATA_OFFSET, programData.data.length - LOADER_V3_PROGRAMDATA_OFFSET);
}

async function authenticateDeployment(program: RpcAccount, programAddress: string, programData: RpcAccount, programDataAddress: string, artifact: ArtifactReleaseV1, checked?: CheckedReleaseV1): Promise<number> {
  const elf = programDataView(program, programAddress, programData, programDataAddress, artifact);
  if (hex(await sha256(elf)) !== artifact.elfDigest) throw new Error(`${programAddress} current ELF differs from the finalized artifact release`);
  if (checked !== undefined) {
    if (BigInt(program.data.length) !== checked.programBytes || BigInt(programData.data.length) !== checked.programDataBytes || BigInt(elf.length) !== checked.elfBytes) throw new Error(`${programAddress} account geometry differs from its complete checked release`);
    if (hex(await sha256(program.data)) !== checked.programDigest || hex(await sha256(programData.data)) !== checked.programDataDigest) throw new Error(`${programAddress} current Loader account digest differs from its complete checked release`);
  }
  return elf.length;
}

export async function authenticateArtifactDeploymentV1(program: RpcAccount, programAddress: string, programData: RpcAccount, programDataAddress: string, artifact: ArtifactReleaseV1): Promise<Readonly<{ elfBytes: number; elfDigest: string }>> {
  const elfBytes = await authenticateDeployment(program, programAddress, programData, programDataAddress, artifact);
  return Object.freeze({ elfBytes, elfDigest: artifact.elfDigest });
}

function accountMap(observations: Awaited<ReturnType<RegistryRpc['multipleAccounts']>>): ReadonlyMap<string, RpcAccount | null> {
  return new Map(observations.accounts.map((entry) => [entry.address, entry.account]));
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address); if (account === null || account === undefined) throw new Error(`${field} ${address} is absent at finalized commitment`); return account;
}

function vacancy(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): void {
  const account = accounts.get(address); if (account !== null && account !== undefined && (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.lamports !== '0' || account.data.length !== 0)) throw new Error(`${field} ${address} is not the canonical vacant System account`);
}

function checkedComputeLimit(value: number): number {
  if (!Number.isSafeInteger(value) || value < 1 || value > REGISTRY_MAX_COMPUTE_UNITS) throw new Error(`compute limit must be within 1..${REGISTRY_MAX_COMPUTE_UNITS}`); return value;
}

function compilePacket(payer: PublicKey, registry: PublicKey, accounts: ConstructorParameters<typeof TransactionInstruction>[0]['keys'], data: Uint8Array, recentBlockhash: string, computeUnitLimit: number): Readonly<{ transaction: VersionedTransaction; wireBytes: Uint8Array; signers: ReadonlyArray<string> }> {
  checkedComputeLimit(computeUnitLimit); key(recentBlockhash, 'recent blockhash');
  const instruction = new TransactionInstruction({ programId: registry, keys: accounts, data: Buffer.from(data) });
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: payer, recentBlockhash, instructions: [ComputeBudgetProgram.setComputeUnitLimit({ units: computeUnitLimit }), instruction] }).compileToV0Message());
  const wireBytes = transaction.serialize(); if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`unsigned transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  return Object.freeze({ transaction, wireBytes, signers: Object.freeze(transaction.message.staticAccountKeys.slice(0, transaction.message.header.numRequiredSignatures).map((value) => value.toBase58())) });
}

function registryInstruction(role?: RegistryRole): Uint8Array {
  const bytes = new Uint8Array(REGISTRY_INSTRUCTION_BYTES); bytes.set(new TextEncoder().encode('DCLTRIX1')); new DataView(bytes.buffer).setUint16(8, 1, true);
  if (role !== undefined) { bytes[10] = 1; bytes[11] = REGISTRY_ROLES.indexOf(role); }
  return bytes;
}

export function compileRegistryActivationTransaction(input: Readonly<{ payer: string; registryProgram: string; recentBlockhash: string; computeUnitLimit: number; cache: string; releaseSetRecord: string; releaseSetStaging: string; roles: Readonly<Record<RegistryRole, RegistryRoleAddressesV1>> }>): Readonly<{ transaction: VersionedTransaction; wireBytes: Uint8Array; requiredSigners: ReadonlyArray<string> }> {
  const metas = [
    { pubkey: key(input.payer, 'payer'), isSigner: true, isWritable: true }, { pubkey: key(input.cache, 'activation cache'), isSigner: false, isWritable: true },
    { pubkey: key(input.releaseSetRecord, 'release-set record'), isSigner: false, isWritable: false }, { pubkey: key(input.releaseSetStaging, 'release-set staging cursor'), isSigner: false, isWritable: false },
    ...REGISTRY_ROLES.flatMap((role) => { const value = input.roles[role]; return [
      { pubkey: key(value.record, `${role} record`), isSigner: false, isWritable: false }, { pubkey: key(value.staging, `${role} staging cursor`), isSigner: false, isWritable: false },
      { pubkey: key(value.program, `${role} Program`), isSigner: false, isWritable: false }, { pubkey: key(value.programData, `${role} ProgramData`), isSigner: false, isWritable: false },
    ]; }),
    { pubkey: key(SYSTEM_PROGRAM_ID, 'System Program'), isSigner: false, isWritable: false }, { pubkey: key(RENT_SYSVAR_ID, 'Rent sysvar'), isSigner: false, isWritable: false },
  ];
  if (metas.length !== REGISTRY_ACTIVATE_ACCOUNT_COUNT) throw new Error('activation account frame is not exactly 26 accounts');
  const packet = compilePacket(key(input.payer, 'payer'), key(input.registryProgram, 'Registry program'), metas, registryInstruction(), input.recentBlockhash, input.computeUnitLimit);
  return Object.freeze({ transaction: packet.transaction, wireBytes: packet.wireBytes, requiredSigners: packet.signers });
}

export function compileRegistryReauthenticationTransaction(input: Readonly<{ payer: string; registryProgram: string; recentBlockhash: string; computeUnitLimit: number; cache: string; role: RegistryRole; program: string; programData: string }>): Readonly<{ transaction: VersionedTransaction; wireBytes: Uint8Array; requiredSigners: ReadonlyArray<string> }> {
  const metas = [
    { pubkey: key(input.cache, 'activation cache'), isSigner: false, isWritable: false }, { pubkey: key(input.program, 'role Program'), isSigner: false, isWritable: false }, { pubkey: key(input.programData, 'role ProgramData'), isSigner: false, isWritable: false },
  ];
  const packet = compilePacket(key(input.payer, 'payer'), key(input.registryProgram, 'Registry program'), metas, registryInstruction(input.role), input.recentBlockhash, input.computeUnitLimit);
  return Object.freeze({ transaction: packet.transaction, wireBytes: packet.wireBytes, requiredSigners: packet.signers });
}

export async function prepareRegistryActivation(client: RegistryRpc, input: Readonly<{ registryProgram: string; payer: string; multiprogram: Uint8Array; checkedReleases: Readonly<Record<RegistryRole, Uint8Array>>; computeUnitLimit: number }>): Promise<RegistryActivationPlanV1> {
  const registry = key(input.registryProgram, 'Registry program'); const payer = key(input.payer, 'payer'); checkedComputeLimit(input.computeUnitLimit);
  const evidence = await decodeCheckedMultiprogramV1(input.multiprogram, input.checkedReleases);
  if (evidence.releaseSet.roles.core.program !== input.registryProgram) throw new Error('release set Core program is not the selected Registry program');
  const releaseDigest = await sha256(evidence.releaseSet.bytes); const releasePdas = recordPdas(registry, EXECUTION_RELEASE_SET_SCHEMA, releaseDigest);
  const roleAddresses = await Promise.all(REGISTRY_ROLES.map(async (role): Promise<RegistryRoleAddressesV1> => {
    const artifact = evidence.artifacts[role]; const digest = await sha256(artifact.bytes); const pdas = recordPdas(registry, ARTIFACT_RELEASE_SCHEMA_ID_V1, digest);
    return Object.freeze({ ...pdas, program: artifact.program, programData: artifact.programData });
  }));
  const roles = Object.freeze(roleRecord(roleAddresses)); const cache = PublicKey.findProgramAddressSync([ACTIVATION_SEED, releaseDigest], registry)[0].toBase58();
  const addresses = [...new Set([input.payer, cache, releasePdas.record, releasePdas.staging, ...REGISTRY_ROLES.flatMap((role) => Object.values(roles[role])), SYSTEM_PROGRAM_ID, RENT_SYSVAR_ID])];
  const floor = await client.finalizedSlot(); const observation = await client.multipleAccounts(addresses, floor); const accounts = accountMap(observation);
  const payerAccount = required(accounts, payer.toBase58(), 'payer'); if (payerAccount.owner !== SYSTEM_PROGRAM_ID || payerAccount.executable || payerAccount.data.length !== 0) throw new Error('payer is not a System-owned data-free wallet');
  const system = required(accounts, SYSTEM_PROGRAM_ID, 'System Program'); if (system.owner !== NATIVE_LOADER_ID || !system.executable || system.data.length !== 0) throw new Error('System Program runtime account is not canonical');
  const rent = required(accounts, RENT_SYSVAR_ID, 'Rent sysvar'); if (rent.owner !== SYSVAR_OWNER_ID || rent.executable || rent.data.length !== 17) throw new Error('Rent sysvar runtime account is not canonical');
  const releaseRent = BigInt((await client.minimumBalanceForRentExemption(EXECUTION_RELEASE_SET_BYTES)).lamports); const artifactRent = BigInt((await client.minimumBalanceForRentExemption(ARTIFACT_RELEASE_BYTES)).lamports);
  const releaseRecord = required(accounts, releasePdas.record, 'release-set record'); if (releaseRecord.owner !== input.registryProgram || releaseRecord.executable || !same(releaseRecord.data, evidence.releaseSet.bytes) || BigInt(releaseRecord.lamports) < releaseRent) throw new Error('finalized release-set record bytes, owner, or rent reserve differ from checked evidence'); vacancy(accounts, releasePdas.staging, 'release-set staging cursor');
  let elfBytes = 0;
  for (const role of REGISTRY_ROLES) {
    const addressesForRole = roles[role]; const artifact = evidence.artifacts[role]; const record = required(accounts, addressesForRole.record, `${role} artifact record`);
    if (record.owner !== input.registryProgram || record.executable || !same(record.data, artifact.bytes) || BigInt(record.lamports) < artifactRent) throw new Error(`${role} finalized artifact record bytes, owner, or rent reserve differ from checked evidence`);
    vacancy(accounts, addressesForRole.staging, `${role} staging cursor`);
    elfBytes += await authenticateDeployment(required(accounts, addressesForRole.program, `${role} Program`), addressesForRole.program, required(accounts, addressesForRole.programData, `${role} ProgramData`), addressesForRole.programData, artifact, await decodeCheckedReleaseV1(input.checkedReleases[role]));
  }
  if (!Number.isSafeInteger(elfBytes)) throw new Error('aggregate ELF byte count exceeds browser integer precision');
  const cacheAccount = accounts.get(cache); const expected = expectedCacheBytes(evidence); const cacheRent = BigInt((await client.minimumBalanceForRentExemption(ACTIVATION_CACHE_BYTES)).lamports);
  let mode: 'create' | 'repeat'; let debit: bigint;
  if (cacheAccount === null || cacheAccount === undefined || (cacheAccount.owner === SYSTEM_PROGRAM_ID && !cacheAccount.executable && cacheAccount.lamports === '0' && cacheAccount.data.length === 0)) { mode = 'create'; debit = cacheRent; if (BigInt(payerAccount.lamports) < debit) throw new Error('payer cannot cover exact activation-cache rent'); }
  else { if (cacheAccount.owner !== input.registryProgram || cacheAccount.executable || BigInt(cacheAccount.lamports) < cacheRent || !same(cacheAccount.data, expected)) throw new Error('existing activation cache is not the byte-identical release-derived state'); mode = 'repeat'; debit = 0n; }
  const blockhash = await client.latestBlockhash(observation.slot); const compiled = compileRegistryActivationTransaction({ payer: input.payer, registryProgram: input.registryProgram, recentBlockhash: blockhash.blockhash, computeUnitLimit: input.computeUnitLimit, cache, releaseSetRecord: releasePdas.record, releaseSetStaging: releasePdas.staging, roles });
  return Object.freeze({ observedSlot: observation.slot, registryProgram: input.registryProgram, payer: input.payer, cache, mode, cacheRentDebitLamports: debit.toString(), releaseSetRecord: releasePdas.record, releaseSetStaging: releasePdas.staging, roles, evidence, transaction: compiled.transaction, wireBytes: compiled.wireBytes, requiredSigners: compiled.requiredSigners, computeUnitLimit: input.computeUnitLimit });
}

export async function prepareRegistryReauthentication(client: RegistryRpc, input: Readonly<{ registryProgram: string; payer: string; cache: string; role: RegistryRole; computeUnitLimit: number }>): Promise<RegistryReauthenticationPlanV1> {
  key(input.registryProgram, 'Registry program'); key(input.payer, 'payer'); key(input.cache, 'activation cache'); checkedComputeLimit(input.computeUnitLimit);
  const floor = await client.finalizedSlot(); const first = await client.multipleAccounts([input.cache], floor); const initialCache = required(accountMap(first), input.cache, 'activation cache'); if (initialCache.owner !== input.registryProgram || initialCache.executable) throw new Error('activation cache has the wrong owner or executable flag');
  const projected = parseCache(initialCache.data, input.registryProgram, input.cache); const artifact = projected.artifacts[input.role];
  const addresses = [...new Set([input.payer, input.registryProgram, input.cache, artifact.program, artifact.programData])]; const observation = await client.multipleAccounts(addresses, first.slot); const accounts = accountMap(observation);
  const payer = required(accounts, input.payer, 'payer'); if (payer.owner !== SYSTEM_PROGRAM_ID || payer.executable || payer.data.length !== 0) throw new Error('payer is not a System-owned data-free wallet');
  const registryAccount = required(accounts, input.registryProgram, 'Registry Program'); if (registryAccount.owner !== UPGRADEABLE_LOADER_ID || !registryAccount.executable || registryAccount.data.length !== LOADER_V3_PROGRAM_BYTES || new DataView(registryAccount.data.buffer, registryAccount.data.byteOffset, 4).getUint32(0, true) !== 2) throw new Error('Registry Program is not current Loader-v3 executable state');
  const cacheAccount = required(accounts, input.cache, 'activation cache'); if (!same(cacheAccount.data, initialCache.data) || cacheAccount.owner !== input.registryProgram || cacheAccount.executable) throw new Error('activation cache changed during finalized reacquisition');
  await authenticateDeployment(required(accounts, artifact.program, `${input.role} Program`), artifact.program, required(accounts, artifact.programData, `${input.role} ProgramData`), artifact.programData, artifact);
  const blockhash = await client.latestBlockhash(observation.slot); const compiled = compileRegistryReauthenticationTransaction({ payer: input.payer, registryProgram: input.registryProgram, recentBlockhash: blockhash.blockhash, computeUnitLimit: input.computeUnitLimit, cache: input.cache, role: input.role, program: artifact.program, programData: artifact.programData });
  return Object.freeze({ observedSlot: observation.slot, registryProgram: input.registryProgram, payer: input.payer, cache: input.cache, role: input.role, releaseSetId: projected.releaseSetId, artifactReleaseId: projected.artifactIds[input.role], artifact, transaction: compiled.transaction, wireBytes: compiled.wireBytes, requiredSigners: compiled.requiredSigners, computeUnitLimit: input.computeUnitLimit });
}
