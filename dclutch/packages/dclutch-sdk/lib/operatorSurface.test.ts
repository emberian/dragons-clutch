import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  type DeploymentV1,
  type ProgramEvidenceV1,
  type ProtocolRoleV1,
} from './deployments';
import {
  liveDevnetOperatorPresetV1,
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  checkedLiveDevnetOperatorPresetV1,
  type OperatorDeploymentPresetV1,
} from './operatorSurface';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  REGISTRY_ACTIVATED_ROLE_BYTES,
  REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET,
  REGISTRY_ROLES,
  UPGRADEABLE_LOADER_ID,
} from './releaseRegistry';
import { SOLANA_DEVNET_GENESIS_HASH_V1, type RpcAccount, type SolanaRpcClient } from './rpc';

const ACTIVATION_DOMAIN = new TextEncoder().encode('dclutch:release-activation:v1');
const MAINNET_GENESIS = '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d';

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

function account(owner: string, executable: boolean, data: Uint8Array, space = data.length): RpcAccount {
  return Object.freeze({ data: new Uint8Array(data), executable, lamports: '1', owner, space });
}

function programDataAddress(program: string): string {
  return PublicKey.findProgramAddressSync(
    [new PublicKey(program).toBytes()],
    new PublicKey(UPGRADEABLE_LOADER_ID),
  )[0].toBase58();
}

function loaderProgram(programData: string): RpcAccount {
  const data = new Uint8Array(36);
  new DataView(data.buffer).setUint32(0, 2, true);
  data.set(new PublicKey(programData).toBytes(), 4);
  return account(UPGRADEABLE_LOADER_ID, true, data);
}

function loaderProgramData(slot: string, authority: string, space = 5_000_000): RpcAccount {
  const data = new Uint8Array(45);
  const view = new DataView(data.buffer);
  view.setUint32(0, 3, true);
  view.setBigUint64(4, BigInt(slot), true);
  data[12] = 1;
  data.set(new PublicKey(authority).toBytes(), 13);
  return account(UPGRADEABLE_LOADER_ID, false, data, space);
}

async function artifact(program: string, slot: string, authority: string, seed: number): Promise<Readonly<{
  bytes: Uint8Array;
  id: Uint8Array;
  programData: string;
}>> {
  const bytes = new Uint8Array(ARTIFACT_RELEASE_BYTES);
  bytes.set(new TextEncoder().encode('DCLTARF1'));
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  bytes[12] = 1;
  const programData = programDataAddress(program);
  bytes.set(new PublicKey(program).toBytes(), 16);
  bytes.set(new PublicKey(UPGRADEABLE_LOADER_ID).toBytes(), 48);
  bytes.set(new PublicKey(programData).toBytes(), 80);
  bytes.fill(seed, 112, 144);
  bytes.fill(seed + 40, 144, 176);
  view.setBigUint64(176, BigInt(slot), true);
  bytes.set(new PublicKey(authority).toBytes(), 184);
  return Object.freeze({ bytes, id: await sha256(bytes), programData });
}

type Fixture = Readonly<{
  deployment: DeploymentV1;
  evidence: Readonly<Record<ProtocolRoleV1, ProgramEvidenceV1>>;
  preset: OperatorDeploymentPresetV1;
  accounts: ReadonlyMap<string, RpcAccount | null>;
  releaseSetId: string;
}>;

async function fixture(): Promise<Fixture> {
  const authority = key(90);
  const programs = Object.freeze({
    registry: key(10),
    rent: key(11),
    custody: key(12),
    resolution: key(13),
    claims: key(14),
    trading: key(15),
    core: key(16),
  });
  const slots = Object.freeze({
    registry: '810', rent: '811', custody: '812', resolution: '813',
    claims: '814', trading: '815', core: '816',
  });
  const evidence = {} as Record<ProtocolRoleV1, ProgramEvidenceV1>;
  for (const role of Object.keys(programs) as ProtocolRoleV1[]) {
    evidence[role] = Object.freeze({ programData: programDataAddress(programs[role]), deploymentSlot: slots[role] });
  }
  const artifacts = await Promise.all(REGISTRY_ROLES.map((role, index) => artifact(
    programs[role], slots[role], authority, index + 1,
  )));
  const releaseBytes = new Uint8Array(336);
  releaseBytes.set(new TextEncoder().encode('DCLTRLS1'));
  const releaseView = new DataView(releaseBytes.buffer);
  releaseView.setUint16(8, 1, true);
  releaseView.setUint16(10, 1, true);
  REGISTRY_ROLES.forEach((role, index) => {
    releaseBytes.set(new PublicKey(programs[role]).toBytes(), 16 + index * 64);
    releaseBytes.set(artifacts[index].id, 48 + index * 64);
  });
  const releaseIdentity = await sha256(releaseBytes);
  const activationCache = PublicKey.findProgramAddressSync(
    [ACTIVATION_DOMAIN, releaseIdentity],
    new PublicKey(programs.registry),
  )[0].toBase58();
  const activation = new Uint8Array(ACTIVATION_CACHE_BYTES);
  activation.set(new TextEncoder().encode('DCLTACT1'));
  const activationView = new DataView(activation.buffer);
  activationView.setUint16(8, 1, true);
  activationView.setUint16(10, 1, true);
  activation.set(releaseIdentity, 16);
  REGISTRY_ROLES.forEach((_role, index) => {
    const offset = REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + index * REGISTRY_ACTIVATED_ROLE_BYTES;
    activation.set(artifacts[index].id, offset);
    activation.set(artifacts[index].bytes, offset + 32);
  });
  const deployment: DeploymentV1 = Object.freeze({
    cluster: 'devnet',
    label: 'Devnet',
    endpoint: 'https://api.devnet.solana.com',
    genesisHash: SOLANA_DEVNET_GENESIS_HASH_V1,
    programs,
    activationCache,
    provenance: 'hostile fixture',
  });
  const preset = checkedLiveDevnetOperatorPresetV1(deployment, evidence);
  const accounts = new Map<string, RpcAccount | null>();
  for (const role of OPERATOR_ROLES) {
    accounts.set(programs[role], loaderProgram(evidence[role].programData));
    accounts.set(evidence[role].programData, loaderProgramData(slots[role], authority));
  }
  accounts.set(activationCache, account(programs.registry, false, activation));
  return Object.freeze({
    deployment,
    evidence: Object.freeze(evidence),
    preset,
    accounts,
    releaseSetId: Buffer.from(releaseIdentity).toString('hex'),
  });
}

type ClientOptions = Readonly<{
  genesisHash?: string;
  programSlot?: string;
  headerSlot?: string;
  onFullRead?: (addresses: ReadonlyArray<string>) => void;
  onSlice?: (addresses: ReadonlyArray<string>, offset: number, length: number) => void;
}>;

function client(value: Fixture, accounts = value.accounts, options: ClientOptions = {}): SolanaRpcClient {
  return {
    probe: async () => Object.freeze({
      endpoint: value.preset.endpoint,
      genesisHash: options.genesisHash ?? value.preset.genesisHash,
      solanaCore: 'test',
      featureSet: null,
    }),
    finalizedSlot: async () => '900',
    multipleAccounts: async (addresses: ReadonlyArray<string>) => {
      options.onFullRead?.(addresses);
      return Object.freeze({
        slot: options.programSlot ?? '901',
        accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
      });
    },
    multipleAccountDataSlices: async (addresses: ReadonlyArray<string>, offset: number, length: number) => {
      options.onSlice?.(addresses, offset, length);
      return Object.freeze({
        slot: options.headerSlot ?? '902',
        accounts: Object.freeze(addresses.map((address) => {
          const found = accounts.get(address) ?? null;
          return Object.freeze({
            address,
            account: found === null ? null : Object.freeze({ ...found, data: found.data.slice(offset, offset + length) }),
          });
        })),
      });
    },
  } as unknown as SolanaRpcClient;
}

describe('checked live-devnet operator SDK', () => {
  it('projects the stable six-role preset from the shared deployment authority without inventing Market or Realm', () => {
    expect(liveDevnetOperatorPresetV1().endpoint).toBe(DEVNET_DEPLOYMENT_V1.endpoint);
    expect(liveDevnetOperatorPresetV1().genesisHash).toBe(DEVNET_DEPLOYMENT_V1.genesisHash);
    expect(liveDevnetOperatorPresetV1().activationCache).toBe(DEVNET_DEPLOYMENT_V1.activationCache);
    expect(Object.keys(liveDevnetOperatorPresetV1().coordinates)).toEqual(OPERATOR_ROLES);
    expect('market' in liveDevnetOperatorPresetV1().coordinates).toBe(false);
    expect('realm' in liveDevnetOperatorPresetV1().coordinates).toBe(false);
    for (const role of OPERATOR_ROLES) {
      expect(liveDevnetOperatorPresetV1().coordinates[role]).toBe(DEVNET_DEPLOYMENT_V1.programs[role]);
      expect(liveDevnetOperatorPresetV1().evidence[role]).toEqual(DEVNET_PROGRAM_EVIDENCE_V1[role]);
    }
  });

  it('authenticates genesis, cache contents, Program links, PDAs, exact-authority tags, and slots without reading ELF bodies', async () => {
    const value = await fixture();
    const fullReads: ReadonlyArray<string>[] = [];
    const slices: Readonly<{ addresses: ReadonlyArray<string>; offset: number; length: number }>[] = [];
    const snapshot = await acquireOperatorSurfaceV1(client(value, value.accounts, {
      onFullRead: (addresses) => fullReads.push(addresses),
      onSlice: (addresses, offset, length) => slices.push({ addresses, offset, length }),
    }), value.preset.coordinates, value.preset);
    const programDataAddresses = OPERATOR_ROLES.map((role) => value.preset.evidence[role].programData);
    expect(fullReads).toHaveLength(1);
    expect(fullReads[0].some((address) => programDataAddresses.includes(address))).toBe(false);
    expect(slices).toEqual([{ addresses: programDataAddresses, offset: 0, length: 45 }]);
    expect(programDataAddresses.reduce((sum, address) => sum + (value.accounts.get(address)?.space ?? 0), 0)).toBeGreaterThan(4 * 1024 * 1024);
    expect(snapshot.deploymentPreset).toMatchObject({
      executionReleaseSetId: value.releaseSetId,
      routeSpecificReleaseAdmission: { kind: 'unproven' },
    });
    expect(snapshot.deploymentPreset?.routeSpecificReleaseAdmission.reason).toMatch(/no Realm, Market, or route-specific release admission was proved/);
    expect(snapshot.market).toBeNull();
    expect(snapshot.realm).toBeNull();
  });

  it('refuses stale, missing, and mislinked ProgramData plus a backwards observation context', async () => {
    const value = await fixture();
    const role = 'trading';
    const programData = value.preset.evidence[role].programData;

    const stale = new Map(value.accounts);
    stale.set(programData, loaderProgramData((BigInt(value.preset.evidence[role].deploymentSlot) - 1n).toString(), key(90)));
    await expect(acquireOperatorSurfaceV1(client(value, stale), value.preset.coordinates, value.preset))
      .rejects.toThrow(/DeploymentSlotMismatch.*stale or wrong-generation/);

    const missing = new Map(value.accounts);
    missing.set(programData, null);
    await expect(acquireOperatorSurfaceV1(client(value, missing), value.preset.coordinates, value.preset))
      .rejects.toThrow(/trading ProgramData is absent/);

    const mislinked = new Map(value.accounts);
    mislinked.set(value.preset.coordinates[role], loaderProgram(value.preset.evidence.claims.programData));
    await expect(acquireOperatorSurfaceV1(client(value, mislinked), value.preset.coordinates, value.preset))
      .rejects.toThrow(/trading Program does not link/);

    await expect(acquireOperatorSurfaceV1(
      client(value, value.accounts, { programSlot: '903', headerSlot: '902' }),
      value.preset.coordinates,
      value.preset,
    )).rejects.toThrow(/ProgramData header observation predates/);
  });

  it('refuses wrong authority tags and a partial or wrong-PDA activation cache', async () => {
    const value = await fixture();
    const role = 'claims';
    const programData = value.preset.evidence[role].programData;
    const wrongTag = new Map(value.accounts);
    const tagBytes = new Uint8Array(value.accounts.get(programData)?.data ?? []);
    tagBytes[12] = 0;
    wrongTag.set(programData, account(UPGRADEABLE_LOADER_ID, false, tagBytes, 5_000_000));
    await expect(acquireOperatorSurfaceV1(client(value, wrongTag), value.preset.coordinates, value.preset))
      .rejects.toThrow(/not the exact mutable Loader-v3 header/);

    const partial = new Map(value.accounts);
    const partialCache = new Uint8Array(value.accounts.get(value.preset.activationCache)?.data ?? []);
    partialCache.fill(0, REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET, REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + REGISTRY_ACTIVATED_ROLE_BYTES);
    partial.set(value.preset.activationCache, account(value.preset.coordinates.registry, false, partialCache));
    await expect(acquireOperatorSurfaceV1(client(value, partial), value.preset.coordinates, value.preset))
      .rejects.toThrow(/activated artifact identity is the reserved all-zero identity/);

    const wrongPda = new Map(value.accounts);
    const movedCache = new Uint8Array(value.accounts.get(value.preset.activationCache)?.data ?? []);
    movedCache[16] ^= 1;
    wrongPda.set(value.preset.activationCache, account(value.preset.coordinates.registry, false, movedCache));
    await expect(acquireOperatorSurfaceV1(client(value, wrongPda), value.preset.coordinates, value.preset))
      .rejects.toThrow(/not the release-derived Registry PDA/);
  });

  it('refuses partial deployment, wrong genesis, mainnet, and malformed static preset rows', async () => {
    const value = await fixture();
    const missing = new Map(value.accounts);
    missing.set(value.preset.coordinates.core, null);
    await expect(acquireOperatorSurfaceV1(client(value, missing), value.preset.coordinates, value.preset))
      .rejects.toThrow(/core program is absent/);
    await expect(acquireOperatorSurfaceV1(
      client(value, value.accounts, { genesisHash: MAINNET_GENESIS }),
      value.preset.coordinates,
      value.preset,
    )).rejects.toThrow(/not Solana devnet genesis/);

    expect(() => checkedLiveDevnetOperatorPresetV1(
      { ...value.deployment, cluster: 'custom', genesisHash: MAINNET_GENESIS },
      value.evidence,
    )).toThrow(/refuses every non-devnet deployment/);
    const programs = { ...value.deployment.programs } as Record<string, string>;
    delete programs.core;
    expect(() => checkedLiveDevnetOperatorPresetV1({ ...value.deployment, programs }, value.evidence))
      .toThrow(/core preset program is required/);
  });
});
