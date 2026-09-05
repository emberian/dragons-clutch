import { describe, expect, it } from 'vitest';

import {
  TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
  WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1,
  WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1,
} from './generated/walletTerminalPayoutWasmV1';
import {
  acquireWalletTerminalPayoutSnapshotV1,
  deriveWalletTerminalPayoutManifestV1,
} from './walletTerminalPayoutSnapshot';
import { type SolanaRpcClient } from '@dclutch/sdk/rpc';
import { type WalletTerminalPayoutWasmV1 } from './walletTerminalPayoutV1';

/**
 * THE LAST UNIT. The derivation is extracted, compiled, digest-pinned and
 * canaried; what it authenticates is a thirty-six-account finalized snapshot.
 *
 * Every address comes from the DERIVATION'S OWN LIST, never assembled here.
 * That is the mirror hazard prevented at the point it would otherwise be
 * introduced: a client that computed these addresses alongside the derivation
 * would be a second routing implementation, and the two would drift.
 */

const ADDRESSES = ['11111111111111111111111111111112', '11111111111111111111111111111113'];

function planner(built: string[] = []): WalletTerminalPayoutWasmV1 {
  return {
    wallet_terminal_payout_addresses_v1: () => JSON.stringify({
      format: WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1,
      addresses: ADDRESSES,
      accountCount: TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
    }),
    build_wallet_terminal_payout_manifest_v1: (_input: string, snapshot: string) => {
      built.push(snapshot);
      return '{"format":"dclutch-wallet-terminal-payout-v3"}';
    },
    terminal_settlement_account_count_v3: () => TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
    terminal_settlement_request_bytes_v3: () => 640,
    terminal_settlement_candidate_domain_v3: () => '',
  };
}

function client(floors: (string | undefined)[]): SolanaRpcClient {
  return {
    finalizedSlot: async () => '900',
    blockTime: async () => '1790000000',
    // The chunk planner's sizing round: sizes only, no floor recorded, because
    // `floors` counts the rounds the derivation asked for.
    multipleAccountDataSlices: async (addresses: ReadonlyArray<string>) => ({
      slot: '900',
      accounts: addresses.map((address) => ({
        address,
        account: { owner: '11111111111111111111111111111111', executable: false, lamports: '2000000', space: 3, data: new Uint8Array([1]) },
      })),
    }),
    multipleAccounts: async (addresses: ReadonlyArray<string>, floor?: string) => {
      floors.push(floor);
      return {
        slot: '900',
        accounts: addresses.map((address, index) => ({
          address,
          // The second address is vacant, which is a legitimate observation.
          account: index === 1 ? null : {
            owner: '11111111111111111111111111111111',
            executable: false,
            lamports: '2000000',
            space: 0,
            data: new Uint8Array([1, 2, 3]),
          },
        })),
      };
    },
  } as unknown as SolanaRpcClient;
}

describe('the payout snapshot is the derivation’s own frame, read at one floor', () => {
  it('reads exactly the addresses the derivation named, in its order', async () => {
    const floors: (string | undefined)[] = [];
    const acquired = await acquireWalletTerminalPayoutSnapshotV1(client(floors), planner(), '{}');
    const snapshot = JSON.parse(acquired.snapshotJson) as Readonly<{ keys: string[]; accounts: unknown[]; format: string }>;
    expect(snapshot.format).toBe(WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1);
    // Order is load-bearing: the derivation pairs its keys and observations by
    // position, and its boundary refuses a mispairing.
    expect(snapshot.keys).toEqual(ADDRESSES);
    expect(snapshot.accounts.length).toBe(ADDRESSES.length);
  });

  it('takes the floor once and reads every account at it', async () => {
    const floors: (string | undefined)[] = [];
    await acquireWalletTerminalPayoutSnapshotV1(client(floors), planner(), '{}');
    expect(floors.length).toBeGreaterThan(0);
    for (const floor of floors) expect(floor).toBe('900');
  });

  it('carries a vacant account as vacant rather than refusing it', async () => {
    // The derivation decides which of the thirty-six may be empty. This
    // transport does not, and an absent lookup table or record is a real
    // refusal that belongs to Rust with its own reason.
    const acquired = await acquireWalletTerminalPayoutSnapshotV1(client([]), planner(), '{}');
    const snapshot = JSON.parse(acquired.snapshotJson) as Readonly<{ accounts: (unknown | null)[] }>;
    expect(snapshot.accounts[1]).toBeNull();
    expect(snapshot.accounts[0]).not.toBeNull();
  });

  it('pairs every observation with the address it was asked for', async () => {
    const acquired = await acquireWalletTerminalPayoutSnapshotV1(client([]), planner(), '{}');
    const snapshot = JSON.parse(acquired.snapshotJson) as Readonly<{ keys: string[]; accounts: ({ key: string } | null)[] }>;
    // The boundary cross-checks this and refuses a mismatch; the transport is
    // what must not create one.
    expect(snapshot.accounts[0]?.key).toBe(snapshot.keys[0]);
  });

  it('hands the derivation the snapshot and returns its own manifest', async () => {
    const built: string[] = [];
    const manifest = await deriveWalletTerminalPayoutManifestV1(client([]), planner(built), '{}');
    expect(built.length).toBe(1);
    expect(JSON.parse(manifest).format).toBe('dclutch-wallet-terminal-payout-v3');
  });

  it('refuses an address list that is not the derivation’s exact format', async () => {
    const lying = { ...planner(), wallet_terminal_payout_addresses_v1: () => '{"format":"other"}' };
    await expect(acquireWalletTerminalPayoutSnapshotV1(client([]), lying, '{}'))
      .rejects.toThrow(/payout address list is not the exact accepted format/);
  });
});
