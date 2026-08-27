import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  CHECKED_INFRASTRUCTURE_BYTES_V1,
  decodeCheckedInfrastructureV1,
  decodeProtocolInfrastructureProfileV1,
  inspectProtocolInfrastructureV1,
} from './infrastructure';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  CHECKED_MULTIPROGRAM_BYTES,
  REGISTRY_ROLES,
  RENT_SYSVAR_ID,
  SYSVAR_OWNER_ID,
  UPGRADEABLE_LOADER_ID,
  decodeArtifactReleaseV1,
  deriveFinalizedRecordAddressesV1,
  type ArtifactReleaseV1,
  type RegistryRole,
} from './releaseRegistry';
import {
  PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
} from './generated/protocolInfrastructure';
import { type RpcAccount } from './rpc';

const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111';
const ACTIVATION_DOMAIN = new TextEncoder().encode('dclutch:release-activation:v1');

function key(seed: number): PublicKey {
  return new PublicKey(Uint8Array.from({ length: 32 }, () => seed));
}

function account(owner: string, executable: boolean, data: Uint8Array, lamports = '1'): RpcAccount {
  return Object.freeze({ owner, executable, data: new Uint8Array(data), lamports, space: data.length });
}

type ArtifactFixture = Readonly<{
  artifact: ArtifactReleaseV1;
  programAccount: RpcAccount;
  programDataAccount: RpcAccount;
}>;

async function artifactFixture(seed: number): Promise<ArtifactFixture> {
  const loader = new PublicKey(UPGRADEABLE_LOADER_ID);
  const program = key(seed);
  const programData = PublicKey.findProgramAddressSync([program.toBytes()], loader)[0];
  const elf = Uint8Array.from({ length: 64 }, (_, index) => (seed + index) & 0xff);
  const programBytes = new Uint8Array(36);
  new DataView(programBytes.buffer).setUint32(0, 2, true);
  programBytes.set(programData.toBytes(), 4);
  const programDataBytes = new Uint8Array(45 + elf.length);
  new DataView(programDataBytes.buffer).setUint32(0, 3, true);
  new DataView(programDataBytes.buffer).setBigUint64(4, BigInt(seed), true);
  programDataBytes.set(elf, 45);
  const bytes = new Uint8Array(ARTIFACT_RELEASE_BYTES);
  bytes.set(new TextEncoder().encode('DCLTARF1'));
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  bytes.set(program.toBytes(), 16);
  bytes.set(loader.toBytes(), 48);
  bytes.set(programData.toBytes(), 80);
  bytes.fill((seed + 1) & 0xff, 112, 144);
  bytes.set(await sha256(elf), 144);
  view.setBigUint64(176, BigInt(seed), true);
  return Object.freeze({
    artifact: decodeArtifactReleaseV1(bytes),
    programAccount: account(UPGRADEABLE_LOADER_ID, true, programBytes),
    programDataAccount: account(UPGRADEABLE_LOADER_ID, false, programDataBytes),
  });
}

function releaseSetBytes(
  artifacts: Readonly<Record<RegistryRole, ArtifactFixture>>,
  artifactIds: Readonly<Record<RegistryRole, Uint8Array>>,
): Uint8Array {
  const output = new Uint8Array(336);
  output.set(new TextEncoder().encode('DCLTRLS1'));
  const view = new DataView(output.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  REGISTRY_ROLES.forEach((role, index) => {
    output.set(new PublicKey(artifacts[role].artifact.program).toBytes(), 16 + index * 64);
    output.set(artifactIds[role], 48 + index * 64);
  });
  return output;
}

type Fixture = Readonly<{
  registryProgram: string;
  activationCache: string;
  checkedManifest: Uint8Array;
  accounts: ReadonlyMap<string, RpcAccount | null>;
}>;

async function fixture(seed: number): Promise<Fixture> {
  const roleFixtures = await Promise.all(REGISTRY_ROLES.map((_, index) => artifactFixture(seed + index * 7)));
  const artifacts = Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role, index) => [role, roleFixtures[index]])) as Record<RegistryRole, ArtifactFixture>);
  const artifactIdValues = await Promise.all(REGISTRY_ROLES.map((role) => sha256(artifacts[role].artifact.bytes)));
  const artifactIds = Object.freeze(Object.fromEntries(REGISTRY_ROLES.map((role, index) => [role, artifactIdValues[index]])) as Record<RegistryRole, Uint8Array>);
  const releaseSet = releaseSetBytes(artifacts, artifactIds);
  const releaseSetId = await sha256(releaseSet);
  const registryFixture = await artifactFixture(seed + 50);
  const rentFixture = await artifactFixture(seed + 60);
  const registryProgram = registryFixture.artifact.program;
  const registryArtifactId = await sha256(registryFixture.artifact.bytes);
  const rentArtifactId = await sha256(rentFixture.artifact.bytes);

  const profile = new Uint8Array(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1);
  profile.set(new TextEncoder().encode('DCLTINF1'));
  const profileView = new DataView(profile.buffer);
  profileView.setUint16(8, 1, true);
  profileView.setUint16(10, 1, true);
  profile.set(new PublicKey(registryFixture.artifact.program).toBytes(), 16);
  profile.set(registryArtifactId, 48);
  profile.set(new PublicKey(rentFixture.artifact.program).toBytes(), 80);
  profile.set(rentArtifactId, 112);
  const profilePda = PublicKey.findProgramAddressSync(
    [PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
    new PublicKey(artifacts.core.artifact.program),
  )[0];

  const activation = new Uint8Array(ACTIVATION_CACHE_BYTES);
  activation.set(new TextEncoder().encode('DCLTACT1'));
  const activationView = new DataView(activation.buffer);
  activationView.setUint16(8, 1, true);
  activationView.setUint16(10, 1, true);
  activation.set(releaseSetId, 16);
  REGISTRY_ROLES.forEach((role, index) => {
    const offset = 48 + index * (32 + ARTIFACT_RELEASE_BYTES);
    activation.set(artifactIds[role], offset);
    activation.set(artifacts[role].artifact.bytes, offset + 32);
  });
  const activationCache = PublicKey.findProgramAddressSync(
    [ACTIVATION_DOMAIN, releaseSetId],
    new PublicKey(registryProgram),
  )[0].toBase58();

  const multiprogram = new Uint8Array(CHECKED_MULTIPROGRAM_BYTES);
  multiprogram.set(new TextEncoder().encode('DCLTMPR1'));
  const multiprogramView = new DataView(multiprogram.buffer);
  multiprogramView.setUint16(8, 1, true);
  multiprogramView.setUint16(10, 5, true);
  multiprogram.set(releaseSet, 16);
  REGISTRY_ROLES.forEach((role, index) => {
    const offset = 352 + index * (ARTIFACT_RELEASE_BYTES + 32);
    multiprogram.set(artifacts[role].artifact.bytes, offset);
    multiprogram.fill(seed + index + 1, offset + ARTIFACT_RELEASE_BYTES, offset + ARTIFACT_RELEASE_BYTES + 32);
  });
  const checkedManifest = new Uint8Array(CHECKED_INFRASTRUCTURE_BYTES_V1);
  checkedManifest.set(new TextEncoder().encode('DCLTIEV1'));
  const checkedView = new DataView(checkedManifest.buffer);
  checkedView.setUint16(8, 1, true);
  checkedView.setUint16(10, 3, true);
  checkedManifest.set(multiprogram, 16);
  checkedManifest.set(profile, 16 + CHECKED_MULTIPROGRAM_BYTES);
  checkedManifest.set(profilePda.toBytes(), 16 + CHECKED_MULTIPROGRAM_BYTES + profile.length);
  let leafOffset = 16 + CHECKED_MULTIPROGRAM_BYTES + profile.length + 32;
  checkedManifest.set(registryFixture.artifact.bytes, leafOffset);
  checkedManifest.fill(seed + 90, leafOffset + ARTIFACT_RELEASE_BYTES, leafOffset + ARTIFACT_RELEASE_BYTES + 32);
  leafOffset += ARTIFACT_RELEASE_BYTES + 32;
  checkedManifest.set(rentFixture.artifact.bytes, leafOffset);
  checkedManifest.fill(seed + 91, leafOffset + ARTIFACT_RELEASE_BYTES, leafOffset + ARTIFACT_RELEASE_BYTES + 32);

  const registryRecord = deriveFinalizedRecordAddressesV1(registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, registryArtifactId);
  const rentRecord = deriveFinalizedRecordAddressesV1(registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, rentArtifactId);
  const accounts = new Map<string, RpcAccount | null>();
  accounts.set(activationCache, account(registryProgram, false, activation));
  accounts.set(profilePda.toBase58(), account(artifacts.core.artifact.program, false, profile));
  for (const role of ['core'] as const) {
    accounts.set(artifacts[role].artifact.program, artifacts[role].programAccount);
    accounts.set(artifacts[role].artifact.programData, artifacts[role].programDataAccount);
  }
  accounts.set(registryRecord.record, account(registryProgram, false, registryFixture.artifact.bytes));
  accounts.set(registryRecord.staging, null);
  accounts.set(registryFixture.artifact.program, registryFixture.programAccount);
  accounts.set(registryFixture.artifact.programData, registryFixture.programDataAccount);
  accounts.set(rentRecord.record, account(registryProgram, false, rentFixture.artifact.bytes));
  accounts.set(rentRecord.staging, account(SYSTEM_PROGRAM_ID, false, new Uint8Array(), '7'));
  accounts.set(rentFixture.artifact.program, rentFixture.programAccount);
  accounts.set(rentFixture.artifact.programData, rentFixture.programDataAccount);
  accounts.set(RENT_SYSVAR_ID, account(SYSVAR_OWNER_ID, false, new Uint8Array(17)));
  return Object.freeze({ registryProgram, activationCache, checkedManifest, accounts });
}

function client(value: Fixture) {
  return {
    finalizedSlot: async () => '900',
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: '900',
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: value.accounts.get(address) ?? null }))),
    }),
    minimumBalanceForRentExemption: async (dataLength: number) => Object.freeze({ dataLength, lamports: '1' }),
  };
}

describe('immutable protocol infrastructure inspection', () => {
  it('decodes the exact generated profile and checked evidence', async () => {
    const value = await fixture(11);
    const checked = await decodeCheckedInfrastructureV1(value.checkedManifest);
    expect(value.checkedManifest).toHaveLength(2_280);
    expect(checked.profile.bytes).toHaveLength(144);
    expect(checked.profile.registry.program).toBe(value.registryProgram);
    expect(decodeProtocolInfrastructureProfileV1(checked.profile.bytes)).toEqual(checked.profile);
    expect(checked.execution.artifacts.core.upgradeAuthority).toBeNull();
  });

  it('keeps internal consistency distinct from caller-supplied recognition', async () => {
    const value = await fixture(11);
    const unrecognized = await inspectProtocolInfrastructureV1(client(value), {
      registryProgram: value.registryProgram,
      activationCache: value.activationCache,
    });
    expect(unrecognized.recognition).toEqual({ kind: 'internally-consistent/unrecognized' });
    const recognized = await inspectProtocolInfrastructureV1(client(value), {
      registryProgram: value.registryProgram,
      activationCache: value.activationCache,
      checkedManifest: value.checkedManifest,
    });
    expect(recognized.recognition.kind).toBe('supplied-manifest-match');
    expect(recognized.core.program).not.toBe(recognized.registry.program);
    expect(recognized.registry.program).not.toBe(recognized.rent.program);
  });

  it('refuses counterfeit recognition and current Loader substitution', async () => {
    const known = await fixture(11);
    const counterfeit = await fixture(101);
    await expect(inspectProtocolInfrastructureV1(client(counterfeit), {
      registryProgram: counterfeit.registryProgram,
      activationCache: counterfeit.activationCache,
      checkedManifest: known.checkedManifest,
    })).rejects.toThrow('does not match current chain state');

    const staleAccounts = new Map(known.accounts);
    const checked = await decodeCheckedInfrastructureV1(known.checkedManifest);
    const stale = staleAccounts.get(checked.rentArtifact.programData);
    if (stale !== null && stale !== undefined) {
      const data = new Uint8Array(stale.data);
      data[data.length - 1] ^= 1;
      staleAccounts.set(checked.rentArtifact.programData, account(stale.owner, stale.executable, data));
    }
    const staleFixture = Object.freeze({ ...known, accounts: staleAccounts });
    await expect(inspectProtocolInfrastructureV1(client(staleFixture), {
      registryProgram: staleFixture.registryProgram,
      activationCache: staleFixture.activationCache,
    })).rejects.toThrow('current ELF differs');
  });

  it('refuses profile PDA, manifest mutability, and reserved-byte substitution', async () => {
    const value = await fixture(11);
    const wrongPda = new Uint8Array(value.checkedManifest);
    wrongPda[16 + CHECKED_MULTIPROGRAM_BYTES + 144] ^= 1;
    await expect(decodeCheckedInfrastructureV1(wrongPda)).rejects.toThrow('PDA');
    const reserved = new Uint8Array(value.checkedManifest);
    reserved[12] = 1;
    await expect(decodeCheckedInfrastructureV1(reserved)).rejects.toThrow('reserved');
    const mutableRegistry = new Uint8Array(value.checkedManifest);
    const registryOffset = 16 + CHECKED_MULTIPROGRAM_BYTES + 144 + 32;
    mutableRegistry[registryOffset + 12] = 1;
    mutableRegistry.fill(0x44, registryOffset + 184, registryOffset + 216);
    await expect(decodeCheckedInfrastructureV1(mutableRegistry)).rejects.toThrow('not immutable');
  });
});
