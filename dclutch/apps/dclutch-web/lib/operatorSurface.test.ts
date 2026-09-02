import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import { capabilityActContractV1, evaluateCapabilityV1 } from './capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1, capabilityWorkspaceV1 } from './capabilitySurface';
import { DEVNET_DEPLOYMENT_V1, DEVNET_PROGRAM_EVIDENCE_V1 } from './deployments';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  EXECUTION_RELEASE_SET_BYTES,
  REGISTRY_ACTIVATED_ROLE_BYTES,
  REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET,
  REGISTRY_ACTIVATION_PDA_SEED_V1,
  REGISTRY_ROLES,
  UPGRADEABLE_LOADER_ID,
} from './releaseRegistry';
import {
  liveDevnetOperatorPresetV1,
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  checkedLiveDevnetOperatorPresetV1,
  type OperatorCoordinatesV1,
  type OperatorDeploymentPresetV1,
  type OperatorSurfaceSnapshotV1,
} from './operatorSurface';
import * as operatorSurfaceModule from './operatorSurface';
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

/**
 * One artifact release the Registry would accept, distinct in every identity.
 *
 * `decodeArtifactReleaseV1` requires the magic, schema and profile, a defined
 * upgrade policy, five nonzero identities, no aliasing among Program, Loader
 * and ProgramData, and an upgrade authority consistent with the policy. Policy
 * 0 is `immutable`, whose authority field must be exactly zero -- which is why
 * this writes nothing at offset 184.
 */
function artifactReleaseBytes(seed: number): Uint8Array {
  const bytes = new Uint8Array(ARTIFACT_RELEASE_BYTES);
  bytes.set(new TextEncoder().encode('DCLTARF1'), 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  bytes[12] = 0;
  bytes.set(new Uint8Array(32).fill(seed), 16);
  bytes.set(new Uint8Array(32).fill(seed + 1), 48);
  bytes.set(new Uint8Array(32).fill(seed + 2), 80);
  bytes.set(new Uint8Array(32).fill(seed + 3), 112);
  bytes.set(new Uint8Array(32).fill(seed + 4), 144);
  view.setBigUint64(176, 900n, true);
  return bytes;
}

/**
 * A REAL activation cache, and the preset coordinate its own bytes derive.
 *
 * THE FIXTURE THIS REPLACES WROTE 1,288 ZERO BYTES and called it a cache. That
 * satisfied the only question `acquireOperatorSurfaceV1` used to ask of the
 * account -- is it this wide -- and could not have failed any other, which is
 * exactly why the width check survived: a fixture that agrees with a weaker
 * check can never convict it.
 *
 * This builds what the Registry program actually writes. Five artifact
 * releases, each hashed to the identity stored beside it; those five projected
 * into the 336-byte execution release set; that set hashed to the identity in
 * the cache header. The cache then lives at the PDA that identity derives, so
 * the preset's `activationCache` is COMPUTED FROM THE BYTES rather than
 * asserted next to them -- which is the property the old fixture lacked and
 * the property `decodeActivationCacheV1` checks.
 */
async function honestActivationCacheV1(
  registryProgram: string,
): Promise<Readonly<{ address: string; account: RpcAccount; releaseSetId: string }>> {
  const artifacts = REGISTRY_ROLES.map((_role, index) => artifactReleaseBytes(10 + index * 8));
  const artifactIds = await Promise.all(artifacts.map((bytes) => sha256(bytes)));
  const releaseSet = new Uint8Array(EXECUTION_RELEASE_SET_BYTES);
  releaseSet.set(new TextEncoder().encode('DCLTRLS1'), 0);
  const releaseSetView = new DataView(releaseSet.buffer);
  releaseSetView.setUint16(8, 1, true);
  releaseSetView.setUint16(10, 1, true);
  REGISTRY_ROLES.forEach((_role, index) => {
    releaseSet.set(artifacts[index].slice(16, 48), 16 + index * 64);
    releaseSet.set(artifactIds[index], 48 + index * 64);
  });
  const releaseSetIdentity = await sha256(releaseSet);
  const data = new Uint8Array(ACTIVATION_CACHE_BYTES);
  data.set(new TextEncoder().encode('DCLTACT1'), 0);
  const view = new DataView(data.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  data.set(releaseSetIdentity, 16);
  REGISTRY_ROLES.forEach((_role, index) => {
    const offset = REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + index * REGISTRY_ACTIVATED_ROLE_BYTES;
    data.set(artifactIds[index], offset);
    data.set(artifacts[index], offset + 32);
  });
  const address = PublicKey.findProgramAddressSync(
    [REGISTRY_ACTIVATION_PDA_SEED_V1, releaseSetIdentity],
    new PublicKey(registryProgram),
  )[0].toBase58();
  return Object.freeze({
    address,
    account: Object.freeze({ data, executable: false, lamports: '1', owner: registryProgram, space: data.length }),
    releaseSetId: hex(releaseSetIdentity),
  });
}

let honestPreset: Promise<Readonly<{ preset: OperatorDeploymentPresetV1; cache: RpcAccount; releaseSetId: string }>> | null = null;

/**
 * The shipped preset with its activation cache moved to the fixture's own
 * release, because the shipped coordinate is a devnet PDA of a release these
 * bytes are not. `operatorSurface.live.test.ts` is where the real one is read.
 */
function checkedPresetV1(): Promise<Readonly<{ preset: OperatorDeploymentPresetV1; cache: RpcAccount; releaseSetId: string }>> {
  honestPreset ??= (async () => {
    const shipped = liveDevnetOperatorPresetV1();
    const cache = await honestActivationCacheV1(shipped.coordinates.registry);
    return Object.freeze({
      preset: Object.freeze({ ...shipped, activationCache: cache.address }),
      cache: cache.account,
      releaseSetId: cache.releaseSetId,
    });
  })();
  return honestPreset;
}

async function checkedPresetClient(changes: Readonly<Record<string, RpcAccount | null>> = {}): Promise<SolanaRpcClient> {
  const { preset, cache } = await checkedPresetV1();
  const accounts = new Map<string, RpcAccount | null>();
  for (const role of OPERATOR_ROLES) {
    const evidence = preset.evidence[role];
    accounts.set(preset.coordinates[role], loaderProgram(evidence.programData));
    accounts.set(evidence.programData, loaderProgramData(evidence.deploymentSlot));
  }
  accounts.set(preset.activationCache, cache);
  for (const [address, next] of Object.entries(changes)) accounts.set(address, next);
  return {
    probe: async () => Object.freeze({
      endpoint: liveDevnetOperatorPresetV1().endpoint,
      genesisHash: liveDevnetOperatorPresetV1().genesisHash,
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
    expect(liveDevnetOperatorPresetV1().endpoint).toBe(DEVNET_DEPLOYMENT_V1.endpoint);
    expect(liveDevnetOperatorPresetV1().genesisHash).toBe(DEVNET_DEPLOYMENT_V1.genesisHash);
    expect(liveDevnetOperatorPresetV1().activationCache).toBe(DEVNET_DEPLOYMENT_V1.activationCache);
    expect(Object.keys(liveDevnetOperatorPresetV1().coordinates)).toEqual(OPERATOR_ROLES);
    expect('market' in liveDevnetOperatorPresetV1().coordinates).toBe(false);
    for (const role of OPERATOR_ROLES) {
      expect(liveDevnetOperatorPresetV1().coordinates[role]).toBe(DEVNET_DEPLOYMENT_V1.programs[role]);
      expect(liveDevnetOperatorPresetV1().evidence[role]).toEqual(DEVNET_PROGRAM_EVIDENCE_V1[role]);
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
    const { preset } = await checkedPresetV1();
    const client = await checkedPresetClient() as unknown as { probe: () => Promise<unknown> };
    client.probe = async () => Object.freeze({ endpoint: 'https://rpc.invalid/', genesisHash: key(45), solanaCore: 'test', featureSet: null });
    await expect(acquireOperatorSurfaceV1(
      client as unknown as SolanaRpcClient,
      preset.coordinates,
      preset,
    )).rejects.toThrow(/live-devnet preset refused.*not Solana devnet genesis/);
  });

  it('reacquires every preset Loader pair and release cache before returning a checked verdict', async () => {
    const { preset, releaseSetId } = await checkedPresetV1();
    const snapshot = await acquireOperatorSurfaceV1(
      await checkedPresetClient(),
      preset.coordinates,
      preset,
    );
    expect(snapshot.observedSlot).toBe('902');
    expect(snapshot.deploymentPreset).toEqual({
      label: 'Checked live devnet',
      genesisHash: preset.genesisHash,
      activationCache: preset.activationCache,
      // The release the CACHE names, not one the preset was told. Nothing
      // outside those 1,288 bytes produced this value, and the wallet-terminal
      // payout path compares an imported plan's release set against it.
      executionReleaseSetId: releaseSetId,
      deploymentSlots: Object.fromEntries(OPERATOR_ROLES.map((role) => [role, DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot])),
      upgradedSinceRecord: [],
    });
    expect(snapshot.market).toBeNull();
  });

  it('refuses the wide, empty cache the old fixture called a cache', async () => {
    // THE CONVICTION. 1,288 zero bytes owned by the Registry is precisely what
    // the replaced width check admitted, and precisely what a preset naming no
    // release looks like. `decodeActivationCacheV1` refuses it at the magic.
    const { preset } = await checkedPresetV1();
    await expect(acquireOperatorSurfaceV1(
      await checkedPresetClient({
        [preset.activationCache]: account(preset.coordinates.registry, false, ACTIVATION_CACHE_BYTES),
      }),
      preset.coordinates,
      preset,
    )).rejects.toThrow(/activation cache has the wrong exact width, magic, schema, or profile/);
  });

  it('refuses a well-formed cache that is not the PDA its own release derives', async () => {
    // Width, magic, schema, every artifact hash and the release-set projection
    // all pass; only the address is wrong. A length check cannot see this at
    // all, and it is the difference between reading THE deployment's cache and
    // reading A cache someone put in front of the browser.
    const { preset, cache } = await checkedPresetV1();
    const elsewhere = key(77);
    await expect(acquireOperatorSurfaceV1(
      await checkedPresetClient({ [elsewhere]: cache }),
      preset.coordinates,
      Object.freeze({ ...preset, activationCache: elsewhere }),
    )).rejects.toThrow(/activation cache is not the release-derived Registry PDA/);
  });

  it('reports an upgraded role rather than refusing it, and reads the live slot', async () => {
    // An upgrade in place is what devnet does. Before this, the shipped
    // manifest's slot was asserted as equality and five of the seven roles had
    // moved past it, so the whole preset refused and /operate could not
    // inspect anything at all.
    const role = 'trading';
    const { preset } = await checkedPresetV1();
    const programData = preset.evidence[role].programData;
    const moved = (BigInt(preset.evidence[role].deploymentSlot) + 4_000n).toString();
    const snapshot = await acquireOperatorSurfaceV1(
      await checkedPresetClient({ [programData]: loaderProgramData(moved) }),
      preset.coordinates,
      preset,
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
    const { preset } = await checkedPresetV1();
    const programData = preset.evidence[role].programData;
    const earlier = (BigInt(preset.evidence[role].deploymentSlot) - 1n).toString();
    await expect(acquireOperatorSurfaceV1(
      await checkedPresetClient({ [programData]: loaderProgramData(earlier) }),
      preset.coordinates,
      preset,
    )).rejects.toThrow(/trading DeploymentSlotMismatch.*preset records slot.*reports the earlier/);

    await expect(acquireOperatorSurfaceV1(
      await checkedPresetClient({ [preset.activationCache]: null }),
      preset.coordinates,
      preset,
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

describe('the live preset is derived on demand, never at import', () => {
  /**
   * THE DEFECT THIS CLOSES is a latent bundle bug, not a test inconvenience.
   *
   * `liveDevnetOperatorPresetV1()` used to be a module-scope `const` whose
   * initializer calls `PublicKey.findProgramAddressSync` seven times. That
   * function SEARCHES — it walks 256 nonces and throws `Unable to find a
   * viable program address nonce` when none is off-curve — so the module could
   * throw while merely being imported. It did: past the eighteenth component
   * import in one module graph it threw during collection, while the same
   * module imported alone evaluated fine. Bisected to that exact boundary.
   *
   * A page that happens to import one more component would ship broken, and
   * the stack would name `operatorSurface` rather than whatever pushed the
   * graph over. Deriving on first use instead means the throw lands where a
   * caller can see and handle it, and importing a sibling can never take a
   * page down.
   *
   * Every check the eager version made still runs, unchanged, on first call.
   * This is laziness, not a relaxed guard.
   */
  it('exports a function, and no pre-derived constant', () => {
    // The shape check is the load-bearing one: re-adding the eager const is
    // exactly the regression, and it would otherwise look like a tidy-up.
    expect(typeof liveDevnetOperatorPresetV1).toBe('function');
    expect(Object.keys(operatorSurfaceModule)).not.toContain('liveDevnetOperatorPresetV1()');
  });

  it('derives once and hands back the same frozen preset', () => {
    const first = liveDevnetOperatorPresetV1();
    expect(liveDevnetOperatorPresetV1()).toBe(first);
    expect(Object.isFrozen(first)).toBe(true);
  });

  it('still refuses a preset whose ProgramData is not its Loader-v3 coordinate', () => {
    // The guard the eager derivation existed for, proven to still bite.
    const tampered = {
      ...DEVNET_PROGRAM_EVIDENCE_V1,
      core: { ...DEVNET_PROGRAM_EVIDENCE_V1.core, programData: '11111111111111111111111111111111' },
    };
    expect(() => checkedLiveDevnetOperatorPresetV1(DEVNET_DEPLOYMENT_V1, tampered))
      .toThrow(/is not the canonical Loader-v3 coordinate/);
  });
});
