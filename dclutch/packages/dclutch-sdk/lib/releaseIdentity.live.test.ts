import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import {
  DEVNET_COHORT_5_ABI_RELEASE_V1,
  authenticateReleaseCurrencyV1,
  discoverCurrentActivationCacheV1,
  openReleaseBoundSessionV1,
  readExecutionReleaseIdentityV1,
  selectAbiReleaseV1,
} from './releaseIdentity';
import { REGISTRY_ROLES } from './releaseRegistry';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * The release-identity read, against the real chain.
 *
 * Opt-in on `DCLUTCH_LIVE_DEVNET=1` so an ordinary suite run spends nobody's
 * rate limit — DISC's pattern. The whole probe is four bounded public-RPC
 * reads: two `getAccountInfo` on 1288-byte caches and two 45-byte-per-account
 * ProgramData header reads. It never downloads an ELF body and never writes.
 *
 * The current activation cache, established by reading the chain on
 * 2026-08-29: the Registry owns FIVE accounts of width 1288, one per cohort,
 * and the current one is the single cache whose five pinned deployment slots
 * equal the five live ProgramData deployment slots.
 */
const CURRENT_ACTIVATION_CACHE_V1 = '77PrN82TY4rrQwUjyKBM14A1n3qxktHrN8vd2RcacovK';

/**
 * A cache that is permanently NOT current: DEPLOY-1's, from cohort-1.
 *
 * Activation never deletes a superseded cache, so this address will stay
 * decodable, Registry-owned and 1288 bytes wide forever, and will never again
 * describe the running programs. That makes it a stable input for the
 * follow-the-chain test — unlike the manifest's hint, which is supposed to be
 * current and so cannot be relied on to be stale.
 */
const SUPERSEDED_ACTIVATION_CACHE_V1 = 'Hz6BXyxyf66teABb6Pr6ev9jCZBJJpP5Q9p4sYJwJSkj';

describe('live devnet release identity', () => {
  live('reads the release the chain is running and selects this build\'s ABI table for it', async () => {
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
    const session = await openReleaseBoundSessionV1(client, {
      registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
      activationCache: CURRENT_ACTIVATION_CACHE_V1,
    });

    expect(session.release.label).toBe(DEVNET_COHORT_5_ABI_RELEASE_V1.label);
    expect(session.identity.executionReleaseSetId)
      .toBe('094336271db1146f09f6ff419488af2d3174da762d3b2b468fac635754aa862d');
    for (const role of REGISTRY_ROLES) {
      expect(session.identity.roles[role].semanticReleaseId, role)
        .toBe(DEVNET_COHORT_5_ABI_RELEASE_V1.semanticReleaseIds[role]);
    }
    // Reading identity is worth nothing if the frames are not bound to it.
    expect(session.abi.coreFoundAccountCount).toBeGreaterThan(0);
  }, 60_000);

  /**
   * WHEN THIS GOES RED, IT IS TELLING THE TRUTH AND THE FIX IS ONE STEP.
   *
   * A new cohort activates a new release set, which mints a new cache at a new
   * PDA and can move any role's semantics. If this fails on the identity, the
   * step is the one in the SDK README under "Shipping a new cohort":
   * regenerate the ABI modules and APPEND a table for the observed identity.
   * Do not edit the existing entry — it describes a release that really ran.
   */
  live('the current cache is the one whose pinned slots match the live programs', async () => {
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
    const identity = await readExecutionReleaseIdentityV1(client, {
      registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
      activationCache: CURRENT_ACTIVATION_CACHE_V1,
    });
    await expect(authenticateReleaseCurrencyV1(client, identity)).resolves.toBeUndefined();
    expect(() => selectAbiReleaseV1(identity)).not.toThrow();
  }, 60_000);

  /**
   * The manifest constant is a HINT, and this proves the client outlives it.
   *
   * The hint under test is named here rather than read from the manifest, and
   * that is the point of this edit. When this test was written the manifest
   * itself shipped a stale address, so passing `DEVNET_DEPLOYMENT_V1
   * .activationCache` exercised the follow path by accident — and the test would
   * have started passing VACUOUSLY, through the manifest branch, the moment
   * anyone fixed the manifest. Which is what happened.
   *
   * `SUPERSEDED_ACTIVATION_CACHE_V1` is cohort-1's cache from DEPLOY-1. A
   * superseded cache is never deleted, so it is a permanent devnet fact and a
   * permanent input: it still exists, is still Registry-owned, still carries
   * `DCLTACT1` and still has the exact 1288-byte width, so every cheap health
   * check on it passes. Only its CONTENT is behind, and its pinned deployment
   * slots match nothing on chain.
   *
   * That matters because `ArtifactReleaseV1::authenticate_deployment` pins the
   * deployment slot and ELF digest on chain, so every route that reauthenticates
   * a role against this cache must refuse — and `CORE_FOUND_ACCOUNT_LABELS_V3`
   * index 24 is the activation cache, so a client that trusted a stale constant
   * would pass this dead address straight into the 37-account Found frame.
   *
   * Opening a session against that stale hint must nonetheless succeed, by
   * following the chain to the cache the live programs actually match. This
   * stays green when cohort-6 lands and moves the answer again, which is the
   * entire point: no human updates a constant.
   */
  live('opens a working session from a SUPERSEDED hint by following the chain', async () => {
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
    const hint = SUPERSEDED_ACTIVATION_CACHE_V1;

    const session = await openReleaseBoundSessionV1(client, {
      registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
      activationCache: hint,
    });

    expect(session.identity.activationCache).not.toBe(hint);
    expect(session.source.kind).toBe('discovered');
    if (session.source.kind !== 'discovered') throw new Error('unreachable');
    expect(session.source.supersededManifestCache).toBe(hint);
    expect(session.source.note).toContain(hint);
    expect(session.source.note).toContain(session.identity.activationCache);

    // And the stale hint really is stale, named slot by slot.
    const stale = await readExecutionReleaseIdentityV1(client, {
      registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
      activationCache: hint,
    });
    await expect(authenticateReleaseCurrencyV1(client, stale)).rejects.toThrow(/SUPERSEDED/);
  }, 60_000);

  /**
   * And the manifest's own hint, whatever it currently says, opens a session.
   *
   * This does NOT assert which branch it takes. Right after a generation it is
   * `manifest`; after the next cohort lands it becomes `discovered`; both are
   * correct and the client is upgrade-proof either way. Asserting the branch
   * here would just re-create the coupling the test above was freed from.
   */
  live('opens a working session from whatever the manifest currently ships', async () => {
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
    const shipped = DEVNET_DEPLOYMENT_V1.activationCache;
    expect(shipped).not.toBeNull();

    const session = await openReleaseBoundSessionV1(client, {
      registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
      activationCache: shipped,
    });

    await expect(authenticateReleaseCurrencyV1(client, session.identity)).resolves.toBeUndefined();
    expect(['manifest', 'discovered']).toContain(session.source.kind);
  }, 60_000);

  live('discovery alone finds the current cache with no hint at all', async () => {
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
    const identity = await discoverCurrentActivationCacheV1(client, DEVNET_DEPLOYMENT_V1.programs.registry);
    await expect(authenticateReleaseCurrencyV1(client, identity)).resolves.toBeUndefined();
    expect(() => selectAbiReleaseV1(identity)).not.toThrow();
  }, 60_000);
});
