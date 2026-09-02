import { describe, expect, it } from 'vitest';

import {
  liveDevnetOperatorPresetV1,
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  type OperatorSurfaceReaderV1,
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

  live('binds every role to the upgrade authority its activated release names, and claims no route admission', async () => {
    /**
     * THE TWO CHECKS THE BROWSER USED NOT TO MAKE.
     *
     * `apps/dclutch-web/lib/operatorSurface.ts` was a fork of this module that
     * read the Loader pairs and the cache and stopped there. It never asked who
     * may upgrade these programs, and it never reported that a deployment match
     * is not a route admission. Both are now the SDK owner's, and the browser
     * re-exports it, so this runs the browser's real path.
     *
     * The authority is the load-bearing one. A deployment slot moves on every
     * ordinary upgrade and so cannot be asserted; the AUTHORITY is what makes a
     * generation this deployment rather than someone else's at the same
     * addresses, and it is asserted against the activation cache's own artifact
     * releases rather than against anything shipped in the client.
     */
    const preset = liveDevnetOperatorPresetV1();
    const snapshot = await acquireOperatorSurfaceV1(
      new SolanaRpcClient(preset.endpoint),
      preset.coordinates,
      preset,
    );
    const authorities = snapshot.deploymentPreset?.upgradeAuthorities;
    expect(authorities).toBeDefined();
    // One shared retained authority across the whole generation. The acquire
    // path already refuses more than one; this states the observed value so a
    // reader of the run sees which key it was.
    const distinct = new Set(OPERATOR_ROLES.map((role) => authorities?.[role]));
    expect(distinct.size, `observed upgrade authorities ${[...distinct].join(', ')}`).toBe(1);
    for (const role of OPERATOR_ROLES) {
      expect(authorities?.[role], `${role} upgrade authority`).toMatch(/^[1-9A-HJ-NP-Za-km-z]{32,44}$/);
    }

    // AND THE MISSING JOIN, SAID OUT LOUD. There is no refusal to assert here
    // because the surface never claims a route admission -- it reports that it
    // proved none. That report IS the check: the failure it prevents is a
    // caller reading "the six programs match" as "this route is admitted", and
    // a field that says `unproven` in every branch is what makes that reading
    // impossible to reach by accident.
    expect(snapshot.deploymentPreset?.routeSpecificReleaseAdmission.kind).toBe('unproven');
    expect(snapshot.deploymentPreset?.routeSpecificReleaseAdmission.reason)
      .toContain('no Realm, Market, or route-specific release admission was proved');
  }, 60_000);

  live('refuses by name when a role\u2019s upgrade authority is not the one its release names', async () => {
    /**
     * THE SAME LIVE BYTES, ONE FIELD MOVED.
     *
     * An upgrade authority cannot be moved on devnet to order, and a fixture
     * that fabricates the whole deployment proves only that the fixture agrees
     * with the check. So this reads the REAL headers and perturbs exactly the
     * 32 bytes at offset 13 of the registry's ProgramData -- the authority
     * field -- leaving every other byte, every other account and the entire
     * activation cache untouched. Everything the acquisition checks before this
     * point therefore still passes on real data, and the refusal that arrives
     * is attributable to the one field that changed.
     */
    const preset = liveDevnetOperatorPresetV1();
    const client = new SolanaRpcClient(preset.endpoint);
    const registryProgramData = preset.evidence.registry.programData;
    const moved: OperatorSurfaceReaderV1 = {
      probe: () => client.probe(),
      finalizedSlot: () => client.finalizedSlot(),
      multipleAccounts: (addresses, floor) => client.multipleAccounts(addresses, floor),
      multipleAccountDataSlices: async (addresses, offset, length, floor) => {
        const observation = await client.multipleAccountDataSlices(addresses, offset, length, floor);
        return Object.freeze({
          slot: observation.slot,
          accounts: Object.freeze(observation.accounts.map((entry) => {
            if (entry.address !== registryProgramData || entry.account === null) return entry;
            const data = new Uint8Array(entry.account.data);
            // Offset 13..45 is the Loader-v3 upgrade authority. Flip one bit of
            // its first byte: still a well-formed 32-byte key, still a mutable
            // header, and no longer the key the activated release names.
            data[13] ^= 0x01;
            return Object.freeze({ address: entry.address, account: Object.freeze({ ...entry.account, data }) });
          })),
        });
      },
    };
    await expect(acquireOperatorSurfaceV1(moved, preset.coordinates, preset))
      .rejects.toThrow('registry ProgramData upgrade authority differs from the activated exact-authority release');
  }, 60_000);
});
