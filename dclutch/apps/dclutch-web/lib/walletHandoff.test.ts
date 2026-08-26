import { PublicKey, SystemProgram, TransactionInstruction, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { type RpcAccount, type SolanaRpcClient } from './rpc';
import { acquireUnsignedTransactionDependenciesV1, inspectUnsignedTransactionV1, requestReadonlyWalletIdentityV1 } from './walletHandoff';

function key(byte: number): PublicKey { return new PublicKey(new Uint8Array(32).fill(byte)); }
function base64(bytes: Uint8Array): string { return Buffer.from(bytes).toString('base64'); }
function account(owner: string, executable: boolean): RpcAccount {
  return Object.freeze({ data: new Uint8Array(0), executable, lamports: '1', owner, space: 0 });
}

function unsignedFixture(): VersionedTransaction {
  const instruction = new TransactionInstruction({
    programId: key(70),
    keys: [{ pubkey: key(71), isSigner: false, isWritable: true }],
    data: Buffer.from([1, 2, 3]),
  });
  const message = new TransactionMessage({ payerKey: key(72), recentBlockhash: key(73).toBase58(), instructions: [instruction] }).compileToV0Message();
  return new VersionedTransaction(message);
}

describe('unsigned wallet handoff', () => {
  it('decodes only unsigned packet-safe transactions and reacquires every dependency', async () => {
    const inspection = await inspectUnsignedTransactionV1(base64(unsignedFixture().serialize()));
    expect(inspection.instructionCount).toBe(1);
    expect(inspection.requiredSignatures).toBe(1);
    expect(inspection.lookupTables).toEqual([]);
    const program = key(70).toBase58();
    const client = {
      finalizedSlot: async () => '50',
      multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
        slot: '51',
        accounts: Object.freeze(addresses.map((address) => Object.freeze({
          address,
          account: address === program ? account(key(90).toBase58(), true) : account(SystemProgram.programId.toBase58(), false),
        }))),
      }),
    } as unknown as SolanaRpcClient;
    const report = await acquireUnsignedTransactionDependenciesV1(client, inspection);
    expect(report.dependencies).toHaveLength(3);
    expect(report.missing).toEqual([]);
    expect(report.nonExecutablePrograms).toEqual([]);
  });

  it('refuses transactions carrying a signature', async () => {
    const transaction = unsignedFixture();
    transaction.signatures[0] = new Uint8Array(64).fill(1);
    await expect(inspectUnsignedTransactionV1(base64(transaction.serialize()))).rejects.toThrow(/already contains/);
  });

  it('requests only a wallet public identity', async () => {
    let connected = false;
    const identity = await requestReadonlyWalletIdentityV1({
      publicKey: { toBase58: () => key(88).toBase58() },
      connect: async () => { connected = true; },
    });
    expect(connected).toBe(true);
    expect(identity.address).toBe(key(88).toBase58());
    expect(identity.label).toContain('no signature');
  });
});
