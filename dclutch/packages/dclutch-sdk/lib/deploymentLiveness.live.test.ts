import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1, PROTOCOL_ROLES_V1 } from './deployments';
import { describeDeploymentLivenessV1, readShippedDeploymentLivenessV1 } from './deploymentLiveness';
import { PUBLIC_DEVNET_CUT_V1, checkedReleaseSetIdsV1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * THE GATE, AGAINST THE CLUSTER THE MANIFEST NAMES.
 *
 * `deployments.live.test.ts` beside this file asks the same first question and
 * has asked it since `0f1d75b27`. It did not stop the defect from shipping a
 * second time, and the reason is structural rather than a matter of anyone
 * remembering: it is `it.skip` unless `DCLUTCH_LIVE_DEVNET=1`, and no tier in
 * `tools/ci/run.sh` set that. A check nothing runs is a check a reader could
 * have run.
 *
 * So this one is run BY THE `web` TIER, which sets the variable for this file
 * alone after proving the cluster answers `getHealth`. That last part is the
 * positive control: an unreachable devnet and a closed cohort produce the same
 * silence otherwise, and the tier must keep "failed" distinct from "never ran".
 *
 * It stays skippable here so that `npm test` in either tree is still offline.
 */
describe('the shipped deployment, against devnet itself', () => {
  live('finds every pinned program ALIVE — the ProgramData, not the stub that outlives it', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const liveness = await readShippedDeploymentLivenessV1(client);
    // The whole description on failure, because the one thing a reader needs
    // when this goes red is WHICH roles are gone.
    expect(describeDeploymentLivenessV1(liveness), describeDeploymentLivenessV1(liveness)).toContain('ALIVE');
    expect(liveness.status).toBe('alive');
    if (liveness.status !== 'alive') return;
    expect(liveness.roles).toHaveLength(PROTOCOL_ROLES_V1.length);
    for (const row of liveness.roles) {
      expect(row.live, `${row.role} ProgramData ${row.programData}`).toBe(true);
      expect(BigInt(row.deploymentSlot ?? '0'), `${row.role} deployment slot`).toBeGreaterThan(0n);
    }
  }, 60_000);

  live('finds the featured market selecting exactly the release set the cut says was checked', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const liveness = await readShippedDeploymentLivenessV1(client);
    expect(liveness.status, liveness.status === 'refused' ? liveness.reason : '').toBe('alive');
    if (liveness.status !== 'alive') return;
    expect(liveness.market).toBe(PUBLIC_DEVNET_CUT_V1.market);
    // Read off the Market's own bytes at offset 208 in this same finalized
    // round, against the table the site publishes. Neither side is a literal.
    expect(checkedReleaseSetIdsV1(PUBLIC_DEVNET_CUT_V1)).toContain(liveness.marketReleaseSetId);
  }, 60_000);
});
