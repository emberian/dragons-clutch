import { Keypair, SystemProgram, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';
import { type SolanaRpcClient } from '@dclutch/sdk/rpc';
import { archiveFinalizedRefusedClientOperationV1 } from './clientOperationRefusalV1';
import { findDealerOperationV1, retainDealerPlanV1, submitDealerOperationV1 } from './dealerLiquidityOperation';

async function fixture() {
  const rows = new Map<string, string>();
  const storage = { get length() { return rows.size; }, key: (n: number) => [...rows.keys()][n] ?? null, getItem: (key: string) => rows.get(key) ?? null,
    setItem: (key: string, value: string) => { rows.set(key, value); }, removeItem: (key: string) => { rows.delete(key); } };
  const owner = Keypair.fromSeed(new Uint8Array(32).fill(72)); const address = owner.publicKey.toBase58();
  const tx = new VersionedTransaction(new TransactionMessage({ payerKey: owner.publicKey, recentBlockhash: address,
    instructions: [SystemProgram.transfer({ fromPubkey: owner.publicKey, toPubkey: Keypair.fromSeed(new Uint8Array(32).fill(73)).publicKey, lamports: 1 })],
  }).compileToV0Message());
  const scope = { clusterGenesis: address, market: address, owner: address };
  const unsigned = await retainDealerPlanV1(storage, scope, 'input', 'plan', tx); tx.sign([owner]);
  await expect(submitDealerOperationV1(storage, unsigned, tx, async () => { throw new Error('timeout'); })).rejects.toThrow('timeout');
  const journal = (await findDealerOperationV1(storage, scope))!;
  const client = {
    assertMutationCluster: async () => ({ genesisHash: address }),
    signatureStatuses: async () => [{ signature: journal.signature, slot: '14', known: true, confirmationStatus: 'finalized', succeeded: false }],
    transaction: async () => ({ signature: journal.signature, slot: '14', succeeded: false, transactionBytes: tx.serialize(), error: 'AccountNotFound', errorText: 'AccountNotFound', feeLamports: '5000' }),
  } as unknown as Pick<SolanaRpcClient, 'assertMutationCluster' | 'signatureStatuses' | 'transaction'>;
  return { rows, storage, scope, journal, client };
}
describe('finalized refusal recovery', () => {
  it('archives the exact refusal before releasing the active slot', async () => {
    const { rows, storage, scope, journal, client } = await fixture();
    const archive = await archiveFinalizedRefusedClientOperationV1(storage, client, journal);
    expect(archive.errorText).toBe('AccountNotFound');
    expect(await findDealerOperationV1(storage, scope)).toBeNull();
    expect([...rows.values()]).toEqual([JSON.stringify(archive)]);
  });
  it('retains the active operation for confirmed, successful, or substituted outcomes', async () => {
    for (const mode of ['confirmed', 'success', 'substitution']) {
      const { storage, scope, journal, client } = await fixture();
      const observed = await client.transaction(journal.signature!);
      const changed = { ...client,
        signatureStatuses: async () => [{ ...(await client.signatureStatuses([journal.signature!]))[0]!, confirmationStatus: mode === 'confirmed' ? 'confirmed' : 'finalized', succeeded: mode === 'success' }],
        transaction: async () => ({ ...observed!, transactionBytes: mode === 'substitution' ? new Uint8Array([1]) : observed!.transactionBytes }),
      };
      await expect(archiveFinalizedRefusedClientOperationV1(storage, changed, journal)).rejects.toThrow(mode === 'substitution' ? 'exact retained signed packet' : 'Only an exact finalized refusal');
      expect((await findDealerOperationV1(storage, scope))?.phase).toBe('submitted');
    }
  });
});
