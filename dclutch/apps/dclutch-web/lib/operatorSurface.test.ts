import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { capabilityActContractV1, evaluateCapabilityV1 } from './capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1, capabilityWorkspaceV1 } from './capabilitySurface';
import { DEVNET_DEPLOYMENT_V1, DEVNET_PROGRAM_EVIDENCE_V1 } from './deployments';
import {
  ACTIVATION_CACHE_BYTES,
  UPGRADEABLE_LOADER_ID,
} from './releaseRegistry';
import {
  LIVE_DEVNET_OPERATOR_PRESET_V1,
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  checkedLiveDevnetOperatorPresetV1,
  type OperatorCoordinatesV1,
  type OperatorSurfaceSnapshotV1,
} from './operatorSurface';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

function key(byte: number): string { return new PublicKey(new Uint8Array(32).fill(byte)).toBase58(); }
function account(owner: string, executable: boolean, dataBytes = 36): RpcAccount {
  return Object.freeze({ data: new Uint8Array(dataBytes), executable, lamports: '1', owner, space: dataBytes });
}

function loaderProgram(programData: string): RpcAccount {
  const data = new Uint8Array(36);
  new DataView(data.buffer).setUint32(0, 2, true);
  data.set(new PublicKey(programData).toBytes(), 4);
  return Object.freeze({ data, executable: true, lamports: '1', owner: UPGRADEABLE_LOADER_ID, space: data.length });
}

function loaderProgramData(slot: string): RpcAccount {
  const data = new Uint8Array(46);
  const view = new DataView(data.buffer);
  view.setUint32(0, 3, true);
  view.setBigUint64(4, BigInt(slot), true);
  data[12] = 0;
  return Object.freeze({ data, executable: false, lamports: '1', owner: UPGRADEABLE_LOADER_ID, space: data.length });
}

function checkedPresetClient(changes: Readonly<Record<string, RpcAccount | null>> = {}): SolanaRpcClient {
  const accounts = new Map<string, RpcAccount | null>();
  for (const role of OPERATOR_ROLES) {
    const evidence = LIVE_DEVNET_OPERATOR_PRESET_V1.evidence[role];
    accounts.set(LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates[role], loaderProgram(evidence.programData));
    accounts.set(evidence.programData, loaderProgramData(evidence.deploymentSlot));
  }
  accounts.set(
    LIVE_DEVNET_OPERATOR_PRESET_V1.activationCache,
    account(LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates.registry, false, ACTIVATION_CACHE_BYTES),
  );
  for (const [address, next] of Object.entries(changes)) accounts.set(address, next);
  return {
    probe: async () => Object.freeze({
      endpoint: LIVE_DEVNET_OPERATOR_PRESET_V1.endpoint,
      genesisHash: LIVE_DEVNET_OPERATOR_PRESET_V1.genesisHash,
      solanaCore: 'test',
      featureSet: null,
    }),
    finalizedSlot: async () => '900',
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: '901',
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
    multipleAccountDataSlices: async (addresses: ReadonlyArray<string>, offset: number, length: number) => Object.freeze({
      slot: '902',
      accounts: Object.freeze(addresses.map((address) => {
        const found = accounts.get(address) ?? null;
        return Object.freeze({
          address,
          account: found === null ? null : Object.freeze({ ...found, data: found.data.slice(offset, offset + length) }),
        });
      })),
    }),
  } as unknown as SolanaRpcClient;
}

describe('unified chain-derived operator surface', () => {
  it('reacquires six distinct executable roles and exact Core-owned Market state', async () => {
    const coordinates = Object.fromEntries(OPERATOR_ROLES.map((role, index) => [role, key(index + 1)])) as Record<(typeof OPERATOR_ROLES)[number], string>;
    const market = key(30);
    const client = {
      finalizedSlot: async () => '41',
      multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
        slot: '42',
        accounts: Object.freeze(addresses.map((address, index) => Object.freeze({
          address,
          account: index < OPERATOR_ROLES.length ? account(key(90), true) : account(coordinates.core, false, 184),
        }))),
      }),
    } as unknown as SolanaRpcClient;
    const snapshot = await acquireOperatorSurfaceV1(client, { ...coordinates, market } as OperatorCoordinatesV1);
    expect(snapshot.observedSlot).toBe('42');
    expect(snapshot.roles).toHaveLength(6);
    expect(snapshot.roles.every((role) => role.executable)).toBe(true);
    expect(snapshot.deploymentPreset).toBeNull();
    expect(snapshot.realm).toBeNull();
    expect(snapshot.market).toMatchObject({ address: market, owner: coordinates.core, dataBytes: 184 });
  });

  it('refuses aliased role authority before any RPC read', async () => {
    const coordinates = Object.fromEntries(OPERATOR_ROLES.map((role) => [role, key(1)])) as OperatorCoordinatesV1;
    const client = { finalizedSlot: async () => { throw new Error('must not read'); } } as unknown as SolanaRpcClient;
    await expect(acquireOperatorSurfaceV1(client, coordinates)).rejects.toThrow(/distinct/);
  });

  it('derives the explicit six-role preset from the existing checked devnet authority without inventing a Market', () => {
    expect(LIVE_DEVNET_OPERATOR_PRESET_V1.endpoint).toBe(DEVNET_DEPLOYMENT_V1.endpoint);
    expect(LIVE_DEVNET_OPERATOR_PRESET_V1.genesisHash).toBe(DEVNET_DEPLOYMENT_V1.genesisHash);
    expect(LIVE_DEVNET_OPERATOR_PRESET_V1.activationCache).toBe(DEVNET_DEPLOYMENT_V1.activationCache);
    expect(Object.keys(LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates)).toEqual(OPERATOR_ROLES);
    expect('market' in LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates).toBe(false);
    for (const role of OPERATOR_ROLES) {
      expect(LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates[role]).toBe(DEVNET_DEPLOYMENT_V1.programs[role]);
      expect(LIVE_DEVNET_OPERATOR_PRESET_V1.evidence[role]).toEqual(DEVNET_PROGRAM_EVIDENCE_V1[role]);
    }
  });

  it('refuses missing and partial static deployment rows before exposing a preset', () => {
    const programs = { ...DEVNET_DEPLOYMENT_V1.programs } as Record<string, string>;
    delete programs.core;
    expect(() => checkedLiveDevnetOperatorPresetV1(
      { ...DEVNET_DEPLOYMENT_V1, programs },
      DEVNET_PROGRAM_EVIDENCE_V1,
    )).toThrow(/core preset program is required/);

    const evidence = { ...DEVNET_PROGRAM_EVIDENCE_V1 } as Record<string, unknown>;
    delete evidence.claims;
    expect(() => checkedLiveDevnetOperatorPresetV1(
      DEVNET_DEPLOYMENT_V1,
      evidence,
    )).toThrow(/claims ProgramData evidence is absent or partial/);
  });

  it('refuses every non-devnet projection and a non-devnet endpoint at live reacquisition', async () => {
    expect(() => checkedLiveDevnetOperatorPresetV1(
      { ...DEVNET_DEPLOYMENT_V1, genesisHash: key(44) },
      DEVNET_PROGRAM_EVIDENCE_V1,
    )).toThrow(/does not pin Solana devnet genesis/);
    const client = checkedPresetClient() as unknown as { probe: () => Promise<unknown> };
    client.probe = async () => Object.freeze({ endpoint: 'https://rpc.invalid/', genesisHash: key(45), solanaCore: 'test', featureSet: null });
    await expect(acquireOperatorSurfaceV1(
      client as unknown as SolanaRpcClient,
      LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates,
      LIVE_DEVNET_OPERATOR_PRESET_V1,
    )).rejects.toThrow(/live-devnet preset refused.*not Solana devnet genesis/);
  });

  it('reacquires every preset Loader pair and release cache before returning a checked verdict', async () => {
    const snapshot = await acquireOperatorSurfaceV1(
      checkedPresetClient(),
      LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates,
      LIVE_DEVNET_OPERATOR_PRESET_V1,
    );
    expect(snapshot.observedSlot).toBe('902');
    expect(snapshot.deploymentPreset).toEqual({
      label: 'Checked live devnet',
      genesisHash: LIVE_DEVNET_OPERATOR_PRESET_V1.genesisHash,
      activationCache: LIVE_DEVNET_OPERATOR_PRESET_V1.activationCache,
      deploymentSlots: Object.fromEntries(OPERATOR_ROLES.map((role) => [role, DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot])),
      upgradedSinceRecord: [],
    });
    expect(snapshot.market).toBeNull();
  });

  it('reports an upgraded role rather than refusing it, and reads the live slot', async () => {
    // An upgrade in place is what devnet does. Before this, the shipped
    // manifest's slot was asserted as equality and five of the seven roles had
    // moved past it, so the whole preset refused and /operate could not
    // inspect anything at all.
    const role = 'trading';
    const programData = LIVE_DEVNET_OPERATOR_PRESET_V1.evidence[role].programData;
    const moved = (BigInt(LIVE_DEVNET_OPERATOR_PRESET_V1.evidence[role].deploymentSlot) + 4_000n).toString();
    const snapshot = await acquireOperatorSurfaceV1(
      checkedPresetClient({ [programData]: loaderProgramData(moved) }),
      LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates,
      LIVE_DEVNET_OPERATOR_PRESET_V1,
    );
    // The slot reported is the one on chain, not the one that shipped.
    expect(snapshot.deploymentPreset?.deploymentSlots[role]).toBe(moved);
    expect(snapshot.deploymentPreset?.upgradedSinceRecord).toEqual([role]);
    // Every role that did not move stays absent from the notice.
    expect(snapshot.deploymentPreset?.deploymentSlots.core)
      .toBe(DEVNET_PROGRAM_EVIDENCE_V1.core.deploymentSlot);
  });

  it('still refuses a slot EARLIER than the record, and a partial finalized deployment', async () => {
    // Backwards is not an upgrade. The genesis hash already pinned the
    // cluster, so a deployment slot older than the recorded one cannot be a
    // later state of this program: it is a stale or wrong-generation read.
    const role = 'trading';
    const programData = LIVE_DEVNET_OPERATOR_PRESET_V1.evidence[role].programData;
    const earlier = (BigInt(LIVE_DEVNET_OPERATOR_PRESET_V1.evidence[role].deploymentSlot) - 1n).toString();
    await expect(acquireOperatorSurfaceV1(
      checkedPresetClient({ [programData]: loaderProgramData(earlier) }),
      LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates,
      LIVE_DEVNET_OPERATOR_PRESET_V1,
    )).rejects.toThrow(/trading DeploymentSlotMismatch.*preset records slot.*reports the earlier/);

    await expect(acquireOperatorSurfaceV1(
      checkedPresetClient({ [LIVE_DEVNET_OPERATOR_PRESET_V1.activationCache]: null }),
      LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates,
      LIVE_DEVNET_OPERATOR_PRESET_V1,
    )).rejects.toThrow(/release activation cache is absent/);
  });

  it('gives the capability verdict ladder the exact snapshot it decides on', () => {
    // The operator snapshot is the only chain input a capability status takes,
    // so the ladder is checked here, against this file's own snapshot shape.
    // Everything else about a capability -- venue, authority, walls -- is
    // derived from the browser's import graph and gated in
    // `lib/capabilityEvidence.test.ts`.
    const redeem = BROWSER_CAPABILITY_STANDINGS_V1.find((standing) => standing.action.id === 'claims.redeem');
    expect(redeem).toBeDefined();
    expect(redeem && evaluateCapabilityV1(redeem, null)).toMatchObject({ status: 'needs-chain' });
    const withoutMarket = { market: null } as unknown as OperatorSurfaceSnapshotV1;
    expect(redeem && evaluateCapabilityV1(redeem, withoutMarket)).toMatchObject({ status: 'needs-market' });
    const withMarket = { market: { address: key(44) } } as unknown as OperatorSurfaceSnapshotV1;
    expect(redeem && evaluateCapabilityV1(redeem, withMarket)).toMatchObject({ status: 'ready-to-preflight' });
    expect(redeem && capabilityWorkspaceV1(redeem.action, withMarket)).toBe('/redeem');

    // A market-bound act has no address until a Market is read, and an act
    // with no venue never reaches the chain questions at all.
    const author = BROWSER_CAPABILITY_STANDINGS_V1.find((standing) => standing.action.id === 'direct.author');
    expect(author && capabilityWorkspaceV1(author.action, null)).toBeNull();
    expect(author && capabilityWorkspaceV1(author.action, withMarket)).toBe(`/market?address=${key(44)}`);
    const walled = BROWSER_CAPABILITY_STANDINGS_V1.find((standing) => standing.action.id === 'dealer.trade');
    expect(walled && evaluateCapabilityV1(walled, withMarket)).toMatchObject({ status: 'no-venue' });
    expect(walled && capabilityActContractV1(walled).venue).toBe('Nothing here can build it yet');
  });
});
