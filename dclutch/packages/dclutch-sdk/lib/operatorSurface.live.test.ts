import { describe, expect, it } from 'vitest';

import {
  liveDevnetOperatorPresetV1,
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
} from './operatorSurface';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * The exact outside read the `/operate` preset performs after its button fills
 * the form. It is opt-in because it makes five bounded public-RPC requests:
 * version, genesis, finalized slot, one seven-account read, then one read of
 * six 45-byte ProgramData headers. It never downloads the six ELF bodies.
 */
describe('live devnet operator preset', () => {
  live('matches every published Loader pair and recorded slot at finalized commitment', async () => {
    const snapshot = await acquireOperatorSurfaceV1(
      new SolanaRpcClient(liveDevnetOperatorPresetV1().endpoint),
      liveDevnetOperatorPresetV1().coordinates,
      liveDevnetOperatorPresetV1(),
    );
    expect(snapshot.roles.map((role) => role.role)).toEqual(OPERATOR_ROLES);
    expect(snapshot.deploymentPreset?.genesisHash).toBe(liveDevnetOperatorPresetV1().genesisHash);
    expect(snapshot.deploymentPreset?.activationCache).toBe(liveDevnetOperatorPresetV1().activationCache);
    // The release set is decoded out of the cache account, so a nonzero
    // 32-byte identity here means the five artifacts hashed to their stored
    // identities and their projection hashed to this. The offline suite proves
    // the refusals; this proves the live cohort's cache still decodes at all.
    expect(snapshot.deploymentPreset?.executionReleaseSetId).toMatch(/^[0-9a-f]{64}$/);
    expect(snapshot.deploymentPreset?.executionReleaseSetId).not.toBe('0'.repeat(64));
    expect(snapshot.market).toBeNull();
    expect(snapshot.realm).toBeNull();
  }, 60_000);
});
