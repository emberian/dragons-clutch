import { describe, expect, it } from 'vitest';

import {
  LIVE_DEVNET_OPERATOR_PRESET_V1,
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
      new SolanaRpcClient(LIVE_DEVNET_OPERATOR_PRESET_V1.endpoint),
      LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates,
      LIVE_DEVNET_OPERATOR_PRESET_V1,
    );
    expect(snapshot.roles.map((role) => role.role)).toEqual(OPERATOR_ROLES);
    expect(snapshot.deploymentPreset?.genesisHash).toBe(LIVE_DEVNET_OPERATOR_PRESET_V1.genesisHash);
    expect(snapshot.deploymentPreset?.activationCache).toBe(LIVE_DEVNET_OPERATOR_PRESET_V1.activationCache);
    expect(snapshot.market).toBeNull();
    expect(snapshot.realm).toBeNull();
  }, 60_000);
});
