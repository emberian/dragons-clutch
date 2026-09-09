import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';
import { type SolanaRpcClient } from '@dclutch/sdk/rpc';
import { readStructuredLifecycleSnapshotV1 } from './structuredLifecycleSnapshotV1';
import { StructuredLifecycleSelectionV1 } from './structuredLifecycleModelV1';

const address = (seed: number) => new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
const requests = [{ address: address(1), dataSlice: null }, { address: address(2), dataSlice: { offset: 0, length: 45 } }];
const vacant = { owner: address(3), lamports: '0', executable: false, space: 0, data: new Uint8Array() };

describe('Structured native-selected snapshot transport', () => {
  it('preserves absence and explicit slices without converting vacancy into an account', async () => {
    const client = { multipleAccounts: async (addresses: ReadonlyArray<string>) => ({ slot: '10', accounts: addresses.map((key) => ({ address: key, account: null })) }),
      multipleAccountDataSlices: async (addresses: ReadonlyArray<string>, offset: number, length: number) => {
        expect([offset, length]).toEqual([0, 45]);
        return { slot: '10', accounts: addresses.map((key) => ({ address: key, account: vacant })) };
      } };
    const snapshot = await readStructuredLifecycleSnapshotV1(client, requests, '9', () => {});
    expect(snapshot.accounts[0]!.account).toBeNull();
    expect(snapshot.accounts[1]!.account).toEqual(vacant);
    expect(snapshot.accounts[1]!.dataSlice).toEqual({ offset: 0, length: 45 });
  });

  it('refuses a mixed-slot corpus after bounded retries', async () => {
    let reads = 0;
    const client = { multipleAccounts: async (addresses: ReadonlyArray<string>) => { reads += 1; return { slot: '10', accounts: addresses.map((key) => ({ address: key, account: null })) }; },
      multipleAccountDataSlices: async (addresses: ReadonlyArray<string>) => { reads += 1; return { slot: '11', accounts: addresses.map((key) => ({ address: key, account: vacant })) }; } };
    await expect(readStructuredLifecycleSnapshotV1(client, requests, '9', () => {})).rejects.toThrow('Structured finalized account reads did not converge on one slot after three attempts');
    expect(reads).toBe(6);
  });

  it('rejects stale selection and duplicate requests before native planning', async () => {
    const selection = new StructuredLifecycleSelectionV1(); const epoch = selection.current();
    let reads = 0;
    const read = async (addresses: ReadonlyArray<string>) => { reads += 1; selection.invalidate(); return { slot: '10', accounts: addresses.map((key) => ({ address: key, account: null })) }; };
    const client: Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices'> = { multipleAccounts: read, multipleAccountDataSlices: read };
    await expect(readStructuredLifecycleSnapshotV1(client, [requests[0]!, requests[0]!], '9', () => {})).rejects.toThrow('Structured discovery requires distinct canonical account addresses');
    expect(reads).toBe(0);
    await expect(readStructuredLifecycleSnapshotV1(client, requests, '9', () => selection.require(epoch))).rejects.toThrow('Structured selection changed; review the current Market and action');
    expect(reads).toBe(1);
  });
});
