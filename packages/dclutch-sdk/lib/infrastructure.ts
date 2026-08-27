import { PublicKey } from '@solana/web3.js';

import { ascii, hex, requireNonzero, requireZero, sha256, slice, u16 } from './bytes';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  CHECKED_MULTIPROGRAM_BYTES,
  REGISTRY_ROLES,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  SYSVAR_OWNER_ID,
  authenticateArtifactDeploymentV1,
  decodeArtifactReleaseV1,
  decodeExecutionReleaseSetV1,
  deriveFinalizedRecordAddressesV1,
  type ArtifactReleaseV1,
  type CheckedMultiprogramV1,
  type ExecutionReleaseSetV1,
  type RegistryRole,
} from './releaseRegistry';
import {
  PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1,
} from './generated/protocolInfrastructure';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const CHECKED_INFRASTRUCTURE_BYTES_V1 = 2_280;

const CHECKED_INFRASTRUCTURE_HEADER_BYTES = 16;
const CHECKED_INFRASTRUCTURE_PROFILE_OFFSET = CHECKED_INFRASTRUCTURE_HEADER_BYTES + CHECKED_MULTIPROGRAM_BYTES;
const CHECKED_INFRASTRUCTURE_PROFILE_PDA_OFFSET = CHECKED_INFRASTRUCTURE_PROFILE_OFFSET + PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1;
const CHECKED_INFRASTRUCTURE_REGISTRY_OFFSET = CHECKED_INFRASTRUCTURE_PROFILE_PDA_OFFSET + 32;
const CHECKED_INFRASTRUCTURE_LEAF_BYTES = ARTIFACT_RELEASE_BYTES + 32;
const CHECKED_INFRASTRUCTURE_RENT_OFFSET = CHECKED_INFRASTRUCTURE_REGISTRY_OFFSET + CHECKED_INFRASTRUCTURE_LEAF_BYTES;
const ACTIVATION_PDA_DOMAIN = new TextEncoder().encode('dclutch:release-activation:v1');

export type InfrastructureBindingV1 = Readonly<{
  program: string;
  artifactReleaseId: string;
}>;

export type ProtocolInfrastructureProfileV1 = Readonly<{
  bytes: Uint8Array;
  registry: InfrastructureBindingV1;
  rent: InfrastructureBindingV1;
}>;

export type CheckedInfrastructureV1 = Readonly<{
  bytes: Uint8Array;
  checkedInfrastructureId: string;
  execution: CheckedMultiprogramV1;
  profile: ProtocolInfrastructureProfileV1;
  profilePda: string;
  registryArtifact: ArtifactReleaseV1;
  registryCheckedReleaseId: string;
  rentArtifact: ArtifactReleaseV1;
  rentCheckedReleaseId: string;
}>;

export type InfrastructureRecognitionV1 =
  | Readonly<{ kind: 'internally-consistent/unrecognized' }>
  | Readonly<{ kind: 'supplied-manifest-match'; checkedInfrastructureId: string }>;

export type InfrastructureComponentEvidenceV1 = Readonly<{
  program: string;
  programData: string;
  artifactReleaseId: string;
  semanticReleaseId: string;
  elfDigest: string;
  deploymentSlot: string;
}>;

export type ProtocolInfrastructureInspectionV1 = Readonly<{
  observedSlot: string;
  registryProgram: string;
  activationCache: string;
  executionReleaseSetId: string;
  profilePda: string;
  profileDigest: string;
  core: InfrastructureComponentEvidenceV1;
  registry: InfrastructureComponentEvidenceV1;
  rent: InfrastructureComponentEvidenceV1;
  recognition: InfrastructureRecognitionV1;
}>;

type InfrastructureRpc = Pick<
  SolanaRpcClient,
  'finalizedSlot' | 'multipleAccounts' | 'minimumBalanceForRentExemption'
>;

type ActivatedProjectionV1 = Readonly<{
  releaseSetId: string;
  releaseSet: ExecutionReleaseSetV1;
  artifacts: Readonly<Record<RegistryRole, ArtifactReleaseV1>>;
  artifactIds: Readonly<Record<RegistryRole, string>>;
}>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function publicKey(text: string, field: string): PublicKey {
  const value = new PublicKey(text);
  if (value.toBase58() !== text) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function hexBytes(value: string, field: string): Uint8Array {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error(`${field} must be exact lowercase SHA-256 hex`);
  return Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
}

function roleRecord<T>(values: ReadonlyArray<T>): Readonly<Record<RegistryRole, T>> {
  if (values.length !== REGISTRY_ROLES.length) throw new Error('five-role infrastructure projection is incomplete');
  return Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role, index) => [role, values[index]])) as Record<RegistryRole, T>);
}

function accountMap(observation: Awaited<ReturnType<InfrastructureRpc['multipleAccounts']>>): ReadonlyMap<string, RpcAccount | null> {
  return new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === null || account === undefined) throw new Error(`${field} ${address} is absent at finalized commitment`);
  return account;
}

function immutable(artifact: ArtifactReleaseV1, field: string): void {
  if (artifact.upgradeAuthority !== null) throw new Error(`${field} ArtifactRelease is not immutable`);
}

function exactProfileBinding(bytes: Uint8Array, programOffset: number, artifactOffset: number, field: string): InfrastructureBindingV1 {
  const program = slice(bytes, programOffset, 32);
  const artifact = slice(bytes, artifactOffset, 32);
  requireNonzero(program, `${field} program`);
  requireNonzero(artifact, `${field} artifact release`);
  return Object.freeze({
    program: new PublicKey(program).toBase58(),
    artifactReleaseId: hex(artifact),
  });
}

export function decodeProtocolInfrastructureProfileV1(bytes: Uint8Array): ProtocolInfrastructureProfileV1 {
  if (
    bytes.length !== PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
    || !same(slice(bytes, PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1, 8), PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1)
    || u16(bytes, PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1) !== PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1
    || u16(bytes, PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1) !== PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1
  ) throw new Error('infrastructure profile has the wrong exact width, magic, schema, or profile');
  requireZero(bytes, PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1, 4, 'infrastructure profile header');
  const registry = exactProfileBinding(
    bytes,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1,
    'Registry',
  );
  const rent = exactProfileBinding(
    bytes,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1,
    'Rent',
  );
  if (registry.program === rent.program || registry.artifactReleaseId === rent.artifactReleaseId) {
    throw new Error('infrastructure profile aliases Registry and Rent');
  }
  return Object.freeze({ bytes: new Uint8Array(bytes), registry, rent });
}

function releaseSetBytes(artifacts: Readonly<Record<RegistryRole, ArtifactReleaseV1>>, artifactIds: Readonly<Record<RegistryRole, string>>): Uint8Array {
  const output = new Uint8Array(336);
  output.set(new TextEncoder().encode('DCLTRLS1'));
  const view = new DataView(output.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  REGISTRY_ROLES.forEach((role, index) => {
    output.set(new PublicKey(artifacts[role].program).toBytes(), 16 + index * 64);
    output.set(hexBytes(artifactIds[role], `${role} artifact release`), 48 + index * 64);
  });
  return output;
}

async function decodeActivationCacheV1(bytes: Uint8Array, registryProgram: string, address: string): Promise<ActivatedProjectionV1> {
  if (bytes.length !== ACTIVATION_CACHE_BYTES || ascii(bytes, 0, 8) !== 'DCLTACT1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) {
    throw new Error('activation cache has the wrong exact width, magic, schema, or profile');
  }
  requireZero(bytes, 12, 4, 'activation cache header');
  const releaseSetIdentity = slice(bytes, 16, 32);
  requireNonzero(releaseSetIdentity, 'activation release-set identity');
  const expected = PublicKey.findProgramAddressSync(
    [ACTIVATION_PDA_DOMAIN, releaseSetIdentity],
    publicKey(registryProgram, 'Registry program'),
  )[0].toBase58();
  if (expected !== address) throw new Error('activation cache is not the release-derived Registry PDA');
  const artifacts: ArtifactReleaseV1[] = [];
  const artifactIds: string[] = [];
  for (let index = 0; index < REGISTRY_ROLES.length; index += 1) {
    const offset = 48 + index * (32 + ARTIFACT_RELEASE_BYTES);
    const artifactId = slice(bytes, offset, 32);
    requireNonzero(artifactId, `${REGISTRY_ROLES[index]} cached artifact release`);
    const artifact = decodeArtifactReleaseV1(slice(bytes, offset + 32, ARTIFACT_RELEASE_BYTES));
    if (hex(await sha256(artifact.bytes)) !== hex(artifactId)) throw new Error(`${REGISTRY_ROLES[index]} cached artifact bytes do not hash to their identity`);
    artifacts.push(artifact);
    artifactIds.push(hex(artifactId));
  }
  const artifactRecord = roleRecord(artifacts);
  const artifactIdRecord = roleRecord(artifactIds);
  const releaseBytes = releaseSetBytes(artifactRecord, artifactIdRecord);
  if (hex(await sha256(releaseBytes)) !== hex(releaseSetIdentity)) throw new Error('activation cache release-set projection does not hash to its identity');
  const releaseSet = await decodeExecutionReleaseSetV1(releaseBytes);
  return Object.freeze({
    releaseSetId: hex(releaseSetIdentity),
    releaseSet,
    artifacts: artifactRecord,
    artifactIds: artifactIdRecord,
  });
}

async function decodeEmbeddedCheckedMultiprogramV1(bytes: Uint8Array): Promise<CheckedMultiprogramV1> {
  if (bytes.length !== CHECKED_MULTIPROGRAM_BYTES || ascii(bytes, 0, 8) !== 'DCLTMPR1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 5) {
    throw new Error('checked multiprogram has the wrong exact width, magic, schema, or role count');
  }
  requireZero(bytes, 12, 4, 'checked multiprogram header');
  const releaseSet = await decodeExecutionReleaseSetV1(slice(bytes, 16, 336));
  const artifacts: ArtifactReleaseV1[] = [];
  const checkedReleaseIds: string[] = [];
  for (let index = 0; index < REGISTRY_ROLES.length; index += 1) {
    const role = REGISTRY_ROLES[index];
    const offset = 352 + index * (ARTIFACT_RELEASE_BYTES + 32);
    const artifact = decodeArtifactReleaseV1(slice(bytes, offset, ARTIFACT_RELEASE_BYTES));
    const artifactId = hex(await sha256(artifact.bytes));
    const checkedReleaseId = slice(bytes, offset + ARTIFACT_RELEASE_BYTES, 32);
    requireNonzero(checkedReleaseId, `${role} checked release identity`);
    if (artifact.program !== releaseSet.roles[role].program || artifactId !== releaseSet.roles[role].artifactReleaseId) {
      throw new Error(`${role} embedded artifact does not implement the checked release-set binding`);
    }
    artifacts.push(artifact);
    checkedReleaseIds.push(hex(checkedReleaseId));
  }
  return Object.freeze({
    bytes: new Uint8Array(bytes),
    checkedId: hex(await sha256(bytes)),
    releaseSet,
    artifacts: roleRecord(artifacts),
    checkedReleaseIds: roleRecord(checkedReleaseIds),
  });
}

export async function decodeCheckedInfrastructureV1(bytes: Uint8Array): Promise<CheckedInfrastructureV1> {
  if (bytes.length !== CHECKED_INFRASTRUCTURE_BYTES_V1 || ascii(bytes, 0, 8) !== 'DCLTIEV1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 3) {
    throw new Error('checked infrastructure has the wrong exact width, magic, schema, or component count');
  }
  requireZero(bytes, 12, 4, 'checked infrastructure header');
  const execution = await decodeEmbeddedCheckedMultiprogramV1(slice(bytes, 16, CHECKED_MULTIPROGRAM_BYTES));
  const profile = decodeProtocolInfrastructureProfileV1(slice(
    bytes,
    CHECKED_INFRASTRUCTURE_PROFILE_OFFSET,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
  ));
  const profilePda = new PublicKey(slice(bytes, CHECKED_INFRASTRUCTURE_PROFILE_PDA_OFFSET, 32)).toBase58();
  const registryArtifact = decodeArtifactReleaseV1(slice(bytes, CHECKED_INFRASTRUCTURE_REGISTRY_OFFSET, ARTIFACT_RELEASE_BYTES));
  const registryChecked = slice(bytes, CHECKED_INFRASTRUCTURE_REGISTRY_OFFSET + ARTIFACT_RELEASE_BYTES, 32);
  const rentArtifact = decodeArtifactReleaseV1(slice(bytes, CHECKED_INFRASTRUCTURE_RENT_OFFSET, ARTIFACT_RELEASE_BYTES));
  const rentChecked = slice(bytes, CHECKED_INFRASTRUCTURE_RENT_OFFSET + ARTIFACT_RELEASE_BYTES, 32);
  requireNonzero(registryChecked, 'Registry checked release identity');
  requireNonzero(rentChecked, 'Rent checked release identity');
  immutable(execution.artifacts.core, 'Core');
  immutable(registryArtifact, 'Registry');
  immutable(rentArtifact, 'Rent');
  if (
    execution.artifacts.core.program === registryArtifact.program
    || execution.artifacts.core.program === rentArtifact.program
    || registryArtifact.program === rentArtifact.program
  ) throw new Error('checked infrastructure aliases Core, Registry, or Rent programs');
  const expectedProfilePda = PublicKey.findProgramAddressSync(
    [PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
    new PublicKey(execution.artifacts.core.program),
  )[0].toBase58();
  if (
    profilePda !== expectedProfilePda
    || profile.registry.program !== registryArtifact.program
    || profile.rent.program !== rentArtifact.program
    || profile.registry.artifactReleaseId !== hex(await sha256(registryArtifact.bytes))
    || profile.rent.artifactReleaseId !== hex(await sha256(rentArtifact.bytes))
  ) throw new Error('checked infrastructure profile, PDA, or artifact bindings do not join');
  return Object.freeze({
    bytes: new Uint8Array(bytes),
    checkedInfrastructureId: hex(await sha256(bytes)),
    execution,
    profile,
    profilePda,
    registryArtifact,
    registryCheckedReleaseId: hex(registryChecked),
    rentArtifact,
    rentCheckedReleaseId: hex(rentChecked),
  });
}

function exactRecord(
  account: RpcAccount,
  expectedBytes: Uint8Array,
  registryProgram: string,
  minimumLamports: bigint,
  field: string,
): void {
  if (
    account.owner !== registryProgram
    || account.executable
    || BigInt(account.lamports) < minimumLamports
    || !same(account.data, expectedBytes)
  ) throw new Error(`${field} finalized record bytes, owner, or rent reserve differ`);
}

function vacantStaging(account: RpcAccount | null | undefined, field: string): void {
  if (account === null || account === undefined) return;
  if (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.data.length !== 0) {
    throw new Error(`${field} is not a vacant System-owned staging cursor`);
  }
}

function component(artifactReleaseId: string, artifact: ArtifactReleaseV1): InfrastructureComponentEvidenceV1 {
  return Object.freeze({
    program: artifact.program,
    programData: artifact.programData,
    artifactReleaseId,
    semanticReleaseId: artifact.semanticReleaseId,
    elfDigest: artifact.elfDigest,
    deploymentSlot: artifact.deploymentSlot.toString(),
  });
}

function manifestMatches(
  manifest: CheckedInfrastructureV1,
  profile: ProtocolInfrastructureProfileV1,
  profilePda: string,
  activated: ActivatedProjectionV1,
  registryArtifact: ArtifactReleaseV1,
  rentArtifact: ArtifactReleaseV1,
): boolean {
  if (
    !same(manifest.profile.bytes, profile.bytes)
    || manifest.profilePda !== profilePda
    || !same(manifest.registryArtifact.bytes, registryArtifact.bytes)
    || !same(manifest.rentArtifact.bytes, rentArtifact.bytes)
    || manifest.execution.releaseSet.id !== activated.releaseSetId
    || !same(manifest.execution.releaseSet.bytes, activated.releaseSet.bytes)
  ) return false;
  return REGISTRY_ROLES.every((role) => (
    manifest.execution.releaseSet.roles[role].artifactReleaseId === activated.artifactIds[role]
    && same(manifest.execution.artifacts[role].bytes, activated.artifacts[role].bytes)
  ));
}

export async function inspectProtocolInfrastructureV1(
  client: InfrastructureRpc,
  input: Readonly<{
    registryProgram: string;
    activationCache: string;
    checkedManifest?: Uint8Array;
  }>,
): Promise<ProtocolInfrastructureInspectionV1> {
  const registry = publicKey(input.registryProgram, 'Registry program');
  publicKey(input.activationCache, 'activation cache');
  const floor = await client.finalizedSlot();
  const initial = await client.multipleAccounts([input.activationCache], floor);
  const initialCacheAccount = required(accountMap(initial), input.activationCache, 'activation cache');
  if (initialCacheAccount.owner !== input.registryProgram || initialCacheAccount.executable) throw new Error('activation cache has the wrong Registry owner or executable flag');
  const initialCache = await decodeActivationCacheV1(initialCacheAccount.data, input.registryProgram, input.activationCache);
  const coreArtifact = initialCache.artifacts.core;
  immutable(coreArtifact, 'Core');
  const profilePda = PublicKey.findProgramAddressSync(
    [PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
    new PublicKey(coreArtifact.program),
  )[0].toBase58();

  const profileObservation = await client.multipleAccounts([profilePda], initial.slot);
  const initialProfileAccount = required(accountMap(profileObservation), profilePda, 'infrastructure profile');
  const profile = decodeProtocolInfrastructureProfileV1(initialProfileAccount.data);
  if (profile.registry.program !== input.registryProgram) throw new Error('Core profile selects a different Registry program');
  if (
    coreArtifact.program === profile.registry.program
    || coreArtifact.program === profile.rent.program
    || profile.registry.program === profile.rent.program
  ) throw new Error('Core, Registry, and Rent programs must be distinct');

  const registryRecordAddresses = deriveFinalizedRecordAddressesV1(
    input.registryProgram,
    ARTIFACT_RELEASE_SCHEMA_ID_V1,
    hexBytes(profile.registry.artifactReleaseId, 'Registry artifact release'),
  );
  const rentRecordAddresses = deriveFinalizedRecordAddressesV1(
    input.registryProgram,
    ARTIFACT_RELEASE_SCHEMA_ID_V1,
    hexBytes(profile.rent.artifactReleaseId, 'Rent artifact release'),
  );
  const recordObservation = await client.multipleAccounts(
    [registryRecordAddresses.record, rentRecordAddresses.record],
    profileObservation.slot,
  );
  const recordAccounts = accountMap(recordObservation);
  const registryArtifactAccount = required(recordAccounts, registryRecordAddresses.record, 'Registry artifact');
  const rentArtifactAccount = required(recordAccounts, rentRecordAddresses.record, 'Rent artifact');
  const registryArtifact = decodeArtifactReleaseV1(registryArtifactAccount.data);
  const rentArtifact = decodeArtifactReleaseV1(rentArtifactAccount.data);
  immutable(registryArtifact, 'Registry');
  immutable(rentArtifact, 'Rent');
  if (
    registryArtifact.program !== profile.registry.program
    || rentArtifact.program !== profile.rent.program
    || hex(await sha256(registryArtifact.bytes)) !== profile.registry.artifactReleaseId
    || hex(await sha256(rentArtifact.bytes)) !== profile.rent.artifactReleaseId
  ) throw new Error('profile-selected Registry or Rent artifact record does not join');

  const addresses = [
    profilePda,
    input.activationCache,
    coreArtifact.program,
    coreArtifact.programData,
    registryRecordAddresses.record,
    registryRecordAddresses.staging,
    registryArtifact.program,
    registryArtifact.programData,
    rentRecordAddresses.record,
    rentRecordAddresses.staging,
    rentArtifact.program,
    rentArtifact.programData,
    RENT_SYSVAR_ID,
  ];
  if (new Set(addresses).size !== addresses.length) throw new Error('infrastructure observation aliases named accounts');
  const observation = await client.multipleAccounts(addresses, recordObservation.slot);
  const accounts = accountMap(observation);
  const observedCacheAccount = required(accounts, input.activationCache, 'activation cache');
  const observedProfileAccount = required(accounts, profilePda, 'infrastructure profile');
  if (!same(observedCacheAccount.data, initialCacheAccount.data) || !same(observedProfileAccount.data, initialProfileAccount.data)) throw new Error('cache or profile changed during finalized reacquisition');
  const activated = await decodeActivationCacheV1(observedCacheAccount.data, input.registryProgram, input.activationCache);
  const observedCoreArtifact = activated.artifacts.core;
  if (!same(observedCoreArtifact.bytes, coreArtifact.bytes)) throw new Error('Core artifact changed during finalized reacquisition');

  const [profileRent, cacheRent, artifactRent] = await Promise.all([
    client.minimumBalanceForRentExemption(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1),
    client.minimumBalanceForRentExemption(ACTIVATION_CACHE_BYTES),
    client.minimumBalanceForRentExemption(ARTIFACT_RELEASE_BYTES),
  ]);
  if (
    observedProfileAccount.owner !== observedCoreArtifact.program
    || observedProfileAccount.executable
    || BigInt(observedProfileAccount.lamports) < BigInt(profileRent.lamports)
  ) throw new Error('infrastructure profile has the wrong Core owner, executable flag, or rent reserve');
  if (
    observedCacheAccount.owner !== input.registryProgram
    || observedCacheAccount.executable
    || BigInt(observedCacheAccount.lamports) < BigInt(cacheRent.lamports)
  ) throw new Error('activation cache has the wrong Registry owner, executable flag, or rent reserve');
  const rentSysvar = required(accounts, RENT_SYSVAR_ID, 'Rent sysvar');
  if (rentSysvar.owner !== SYSVAR_OWNER_ID || rentSysvar.executable || rentSysvar.data.length !== 17) throw new Error('Rent sysvar runtime account is not canonical');

  const observedRegistryRecord = required(accounts, registryRecordAddresses.record, 'Registry artifact record');
  const observedRentRecord = required(accounts, rentRecordAddresses.record, 'Rent artifact record');
  exactRecord(observedRegistryRecord, registryArtifact.bytes, input.registryProgram, BigInt(artifactRent.lamports), 'Registry artifact');
  exactRecord(observedRentRecord, rentArtifact.bytes, input.registryProgram, BigInt(artifactRent.lamports), 'Rent artifact');
  vacantStaging(accounts.get(registryRecordAddresses.staging), 'Registry staging cursor');
  vacantStaging(accounts.get(rentRecordAddresses.staging), 'Rent staging cursor');

  await authenticateArtifactDeploymentV1(
    required(accounts, observedCoreArtifact.program, 'Core Program'),
    observedCoreArtifact.program,
    required(accounts, observedCoreArtifact.programData, 'Core ProgramData'),
    observedCoreArtifact.programData,
    observedCoreArtifact,
  );
  await authenticateArtifactDeploymentV1(
    required(accounts, registryArtifact.program, 'Registry Program'),
    registryArtifact.program,
    required(accounts, registryArtifact.programData, 'Registry ProgramData'),
    registryArtifact.programData,
    registryArtifact,
  );
  await authenticateArtifactDeploymentV1(
    required(accounts, rentArtifact.program, 'Rent Program'),
    rentArtifact.program,
    required(accounts, rentArtifact.programData, 'Rent ProgramData'),
    rentArtifact.programData,
    rentArtifact,
  );

  let recognition: InfrastructureRecognitionV1 = Object.freeze({ kind: 'internally-consistent/unrecognized' });
  if (input.checkedManifest !== undefined) {
    const checked = await decodeCheckedInfrastructureV1(input.checkedManifest);
    if (!manifestMatches(checked, profile, profilePda, activated, registryArtifact, rentArtifact)) {
      throw new Error('supplied checked infrastructure manifest does not match current chain state');
    }
    recognition = Object.freeze({
      kind: 'supplied-manifest-match',
      checkedInfrastructureId: checked.checkedInfrastructureId,
    });
  }
  return Object.freeze({
    observedSlot: observation.slot,
    registryProgram: registry.toBase58(),
    activationCache: input.activationCache,
    executionReleaseSetId: activated.releaseSetId,
    profilePda,
    profileDigest: hex(await sha256(profile.bytes)),
    core: component(activated.artifactIds.core, observedCoreArtifact),
    registry: component(profile.registry.artifactReleaseId, registryArtifact),
    rent: component(profile.rent.artifactReleaseId, rentArtifact),
    recognition,
  });
}
