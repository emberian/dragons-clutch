import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1, DEVNET_PROGRAM_EVIDENCE_V1, PROTOCOL_ROLES_V1 } from './deployments';
import { SolanaRpcClient } from './rpc';

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
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
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

  live('finds the activation cache alive under the Registry program', async () => {
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
    const cache = DEVNET_DEPLOYMENT_V1.activationCache;
    expect(cache).not.toBeNull();
    const observation = await client.accountInfo(cache as string);
    expect(observation.account).not.toBeNull();
    expect(observation.account?.owner).toBe(DEVNET_DEPLOYMENT_V1.programs.registry);
    expect(observation.account?.executable).toBe(false);
  }, 30_000);
});
