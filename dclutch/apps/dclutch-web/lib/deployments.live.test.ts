import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1, DEVNET_PROGRAM_EVIDENCE_V1, PROTOCOL_ROLES_V1 } from '@dclutch/sdk/deployments';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

const UPGRADEABLE_LOADER = 'BPFLoaderUpgradeab1e11111111111111111111111';

/**
 * The manifest's devnet rows, verified against the public cluster itself:
 * every baked program id must hold a live, executable, loader-owned account,
 * and each Program account must point at exactly the ProgramData address
 * DEPLOY_1.md §2 recorded. Gated on `DCLUTCH_LIVE_DEVNET=1` because it
 * performs real network IO against api.devnet.solana.com.
 */
describe('live devnet deployment manifest', () => {
  live('finds all seven programs executable under the upgradeable loader, with their recorded ProgramData', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const facts = await client.probe();
    expect(facts.genesisHash).toBe(DEVNET_DEPLOYMENT_V1.genesisHash);

    const addresses = PROTOCOL_ROLES_V1.map((role) => DEVNET_DEPLOYMENT_V1.programs[role]);
    const observation = await client.multipleAccounts(addresses);
    for (const [index, role] of PROTOCOL_ROLES_V1.entries()) {
      const entry = observation.accounts[index];
      expect(entry.address, role).toBe(DEVNET_DEPLOYMENT_V1.programs[role]);
      expect(entry.account, `${role} program account`).not.toBeNull();
      expect(entry.account?.executable, `${role} executable`).toBe(true);
      expect(entry.account?.owner, `${role} owner`).toBe(UPGRADEABLE_LOADER);
      // A Loader V3 Program account is 36 bytes: 4-byte state tag (2 =
      // Program) then the ProgramData address.
      const data = entry.account?.data ?? new Uint8Array();
      expect(data.length, `${role} program account width`).toBe(36);
      const { PublicKey } = await import('@solana/web3.js');
      expect(new PublicKey(data.slice(4)).toBase58(), `${role} ProgramData`).toBe(DEVNET_PROGRAM_EVIDENCE_V1[role].programData);
    }
  }, 60_000);

  live('finds the ProgramData accounts that actually hold the code, not just the Program stubs naming them', async () => {
    /**
     * THE QUESTION THIS FILE WAS NOT ASKING, and it cost a day.
     *
     * `solana program close` deletes the ProgramData account and LEAVES the
     * 36-byte Program account behind: still executable, still owned by the
     * loader, still naming the ProgramData address it used to have. Every check
     * above passes on a closed program. Measured on 2026-09-02 at finalized
     * slot 491,864,298: all seven of cohort-8's ProgramData accounts were
     * absent while all seven Program accounts answered every question this
     * suite asked, so the shipped manifest pointed the browser at dead code for
     * a day with a green gate.
     *
     * Reading 45 bytes of each header is enough: the Loader-v3 tag is at 0 and
     * the deployment slot at 4, and neither exists at all if the account is
     * gone. It never downloads an ELF.
     */
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const addresses = PROTOCOL_ROLES_V1.map((role) => DEVNET_PROGRAM_EVIDENCE_V1[role].programData);
    const observation = await client.multipleAccountDataSlices(addresses, 0, 45);
    for (const [index, role] of PROTOCOL_ROLES_V1.entries()) {
      const account = observation.accounts[index].account;
      expect(account, `${role} ProgramData is absent: this program is CLOSED, and its Program account still names it`).not.toBeNull();
      expect(account?.owner, `${role} ProgramData owner`).toBe(UPGRADEABLE_LOADER);
      expect(account?.executable, `${role} ProgramData executable`).toBe(false);
      const data = account?.data ?? new Uint8Array();
      // Loader-v3 state tag 3 is ProgramData, and the 45-byte header is the
      // slice; `space` is the whole ELF-carrying account, which must be larger.
      expect(new DataView(data.buffer, data.byteOffset, data.byteLength).getUint32(0, true), `${role} ProgramData tag`).toBe(3);
      expect(Number(account?.space ?? 0), `${role} ProgramData carries an ELF`).toBeGreaterThan(45);
      const slot = new DataView(data.buffer, data.byteOffset, data.byteLength).getBigUint64(4, true).toString();
      // Never earlier than the recorded slot: the genesis hash already pinned
      // the cluster, so an older reading is stale, not an upgrade.
      expect(BigInt(slot) >= BigInt(DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot), `${role} deployment slot ${slot}`).toBe(true);
    }
  }, 60_000);

  live('finds the activation cache alive under the Registry program', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const cache = DEVNET_DEPLOYMENT_V1.activationCache;
    expect(cache).not.toBeNull();
    const observation = await client.accountInfo(cache as string);
    expect(observation.account).not.toBeNull();
    expect(observation.account?.owner).toBe(DEVNET_DEPLOYMENT_V1.programs.registry);
    expect(observation.account?.executable).toBe(false);
  }, 30_000);
});
