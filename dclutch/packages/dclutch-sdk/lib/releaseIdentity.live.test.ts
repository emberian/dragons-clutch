import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import {
  KNOWN_ABI_RELEASES_V1,
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
 * The current activation cache, TAKEN FROM THE MANIFEST rather than pinned.
 *
 * This was cohort-5's `77PrN82T…`, on the reasoning that a cache is a permanent
 * devnet fact. It is — and it stopped being the cache of the registry this
 * manifest names, because a cohort deploys a FRESH Registry program and a cache
 * belongs to the registry that minted it. Read against cohort-15's registry,
 * cohort-5's cache is not a superseded cache, it is a foreign account. The
 * manifest's own hint is generated per cohort and is the honest input.
 */
const CURRENT_ACTIVATION_CACHE_V1 = DEVNET_DEPLOYMENT_V1.activationCache ?? '';

/**
 * A hint that is permanently NOT this deployment's: the PREVIOUS cohort's cache.
 *
 * This was cohort-1's, chosen because a superseded cache is never deleted and
 * so is a permanent input. That reasoning survives; the shape it produces does
 * not. Measured 2026-09-04: cohort-15's Registry program owns EXACTLY ONE
 * account of width 1288, so there is no same-registry superseded cache on this
 * deployment at all, and the `SUPERSEDED` refusal has no live subject here. The
 * staleness that actually happens is this one — a hint that ages across a
 * cohort boundary and belongs to the retired registry — and it is the shape
 * the shipped manifest carried this morning.
 */
const PREVIOUS_COHORT_ACTIVATION_CACHE_V1 = 'F66BhQey3ESPRQHEQaLFFEwya4xCb6s2Uh27JiUJ1yVc';

describe('live devnet release identity', () => {
  live('reads the release the chain is running and selects this build\'s ABI table for it', async () => {
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
    const session = await openReleaseBoundSessionV1(client, {
      registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
      activationCache: CURRENT_ACTIVATION_CACHE_V1,
    });

    // NOT A COHORT LITERAL. This pinned cohort-5's label and its release-set
    // id, so it could only ever pass on the one cohort it was written during --
    // and it went red on every cohort from 6 to 15 while saying "the ABI table
    // is missing", which was true and was not what the case is about. What it
    // asserts now is the JOIN: the table the client selected must agree with
    // the identity the chain reported, role by role, and the selection must
    // come from the shipped list rather than from nowhere.
    expect(KNOWN_ABI_RELEASES_V1.map((release) => release.label)).toContain(session.release.label);
    expect(session.identity.executionReleaseSetId).toMatch(/^[0-9a-f]{64}$/);
    for (const role of REGISTRY_ROLES) {
      expect(session.release.semanticReleaseIds[role], `${role}: the selected table and the chain must be the same release`)
        .toBe(session.identity.roles[role].semanticReleaseId);
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
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const hint = PREVIOUS_COHORT_ACTIVATION_CACHE_V1;

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

    // AND THE STALE HINT REALLY IS UNUSABLE, by the refusal that applies to it.
    // This expected /SUPERSEDED/, which is the refusal for a cache this
    // registry minted and has since replaced. A cache from the PREVIOUS cohort
    // never reaches that check: it is owned by the retired Registry program, so
    // it is refused one conjunct earlier, by name. Both are correct refusals of
    // the same hint and the difference is which registry minted it -- so the
    // case asserts the reason rather than a string that describes a different
    // staleness.
    await expect(readExecutionReleaseIdentityV1(client, {
      registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
      activationCache: hint,
    })).rejects.toThrow(/not an activation cache/);
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
