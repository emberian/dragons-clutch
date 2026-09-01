import { Keypair, PublicKey, SystemProgram, TransactionInstruction, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { type RpcAccount, type SolanaRpcClient } from './rpc';
import {
  SOLANA_PACKET_BYTES,
  acquireUnsignedTransactionDependenciesV1,
  inspectUnsignedTransactionV1,
  requestWalletMessageSignatureV1,
  requestWalletCosignTransactionV1,
  requestWalletSubmitCosignTransactionV1,
  requestWalletTransactionSignatureV1,
  submitSignedTransactionV1,
} from './walletHandoff';

const DEVNET_ADMISSION = Object.freeze({
  endpoint: 'https://devnet.example/',
  genesisHash: 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG',
  kind: 'devnet' as const,
});

function admittedClient(
  sendRawTransaction: (
    bytes: Uint8Array,
    options?: Readonly<{ maxRetries?: 0 | 3 }>,
  ) => Promise<string> = async () => 'signature',
) {
  return {
    assertMutationCluster: async () => DEVNET_ADMISSION,
    sendRawTransaction,
  };
}

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

function cosignFixture(wallet: Keypair, resolver: Keypair): VersionedTransaction {
  const instruction = new TransactionInstruction({
    programId: key(70),
    keys: [{ pubkey: resolver.publicKey, isSigner: true, isWritable: false }],
    data: Buffer.from([4, 5, 6]),
  });
  return new VersionedTransaction(new TransactionMessage({
    payerKey: wallet.publicKey,
    recentBlockhash: key(73).toBase58(),
    instructions: [instruction],
  }).compileToV0Message());
}

function submitCosignFixture(wallet: Keypair, update: Keypair): VersionedTransaction {
  const instruction = new TransactionInstruction({
    programId: key(71),
    keys: [{ pubkey: update.publicKey, isSigner: true, isWritable: true }],
    data: Buffer.from([7, 8, 9]),
  });
  return new VersionedTransaction(new TransactionMessage({
    payerKey: wallet.publicKey,
    recentBlockhash: key(74).toBase58(),
    instructions: [instruction],
  }).compileToV0Message());
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

  it('requests exact maker and transaction signatures only after the explicit call', async () => {
    const signer = key(72).toBase58();
    let messageCalls = 0;
    let transactionCalls = 0;
    const wallet = {
      publicKey: { toBase58: () => signer },
      connect: async () => undefined,
      signMessage: async (message: Uint8Array) => {
        messageCalls += 1;
        expect(message).toEqual(Uint8Array.from([9, 8, 7]));
        return new Uint8Array(64).fill(6);
      },
      signTransaction: async (transaction: VersionedTransaction) => {
        transactionCalls += 1;
        transaction.signatures[0] = new Uint8Array(64).fill(7);
        return transaction;
      },
    };
    expect(messageCalls).toBe(0);
    expect(transactionCalls).toBe(0);
    const client = admittedClient();
    await expect(requestWalletMessageSignatureV1(client, wallet, signer, Uint8Array.from([9, 8, 7]))).resolves.toEqual(new Uint8Array(64).fill(6));
    const signed = await requestWalletTransactionSignatureV1(client, wallet, unsignedFixture(), signer);
    expect(signed.complete).toBe(true);
    expect(signed.signer).toBe(signer);
    expect(messageCalls).toBe(1);
    expect(transactionCalls).toBe(1);
  });

  it('authenticates a cross-package wallet result by canonical wire, not constructor identity', async () => {
    const signer = key(72).toBase58();
    const signed = await requestWalletTransactionSignatureV1(admittedClient(), {
      publicKey: { toBase58: () => signer },
      connect: async () => undefined,
      signTransaction: async (transaction: VersionedTransaction) => {
        transaction.signatures[0] = new Uint8Array(64).fill(11);
        // Linked workspaces can load a second web3.js package instance. Model
        // that boundary with a foreign-shaped value carrying only canonical
        // serialization rather than relying on a shared constructor object.
        return { serialize: () => transaction.serialize() };
      },
    }, unsignedFixture(), signer);
    expect(signed.complete).toBe(true);
    expect(signed.signer).toBe(signer);

    await expect(requestWalletTransactionSignatureV1(admittedClient(), {
      publicKey: { toBase58: () => signer },
      connect: async () => undefined,
      signTransaction: async () => ({ serialize: () => new Uint8Array(SOLANA_PACKET_BYTES + 1) }),
    }, unsignedFixture(), signer)).rejects.toThrow(/packet-sized/);
  });

  it('refuses a wallet message rewrite and submits only complete signatures', async () => {
    const transaction = unsignedFixture();
    const signer = key(72).toBase58();
    const client = admittedClient();
    await expect(requestWalletTransactionSignatureV1(client, {
      publicKey: { toBase58: () => signer },
      connect: async () => undefined,
      signTransaction: async (candidate: VersionedTransaction) => {
        candidate.message.recentBlockhash = key(99).toBase58();
        candidate.signatures[0] = new Uint8Array(64).fill(7);
        return candidate;
      },
    }, transaction, signer)).rejects.toThrow(/rewrote/);
    await expect(submitSignedTransactionV1(admittedClient(async () => 'unexpected'), transaction.serialize())).rejects.toThrow(/fully signed/);
    transaction.signatures[0] = new Uint8Array(64).fill(8);
    let submitted = 0;
    await expect(submitSignedTransactionV1(admittedClient(async (bytes, options) => {
      submitted = bytes.length;
      expect(options).toEqual({ maxRetries: 0 });
      return 'signature';
    }), transaction.serialize())).resolves.toBe('signature');
    expect(submitted).toBe(transaction.serialize().length);
  });

  it('refuses a hostile wallet input only after the cluster refuses mainnet', async () => {
    let walletTouched = false;
    const hostileWallet = {};
    Object.defineProperty(hostileWallet, 'connect', {
      get() {
        walletTouched = true;
        throw new Error('wallet getter must not run');
      },
    });
    const mainnetClient = {
      assertMutationCluster: async () => { throw new Error('mutation refused: the endpoint reports Solana mainnet-beta genesis'); },
    };
    await expect(requestWalletMessageSignatureV1(mainnetClient, hostileWallet, key(72).toBase58(), Uint8Array.from([1]))).rejects.toThrow(/mainnet-beta/);
    await expect(requestWalletTransactionSignatureV1(mainnetClient, hostileWallet, unsignedFixture(), key(72).toBase58())).rejects.toThrow(/mainnet-beta/);
    expect(walletTouched).toBe(false);
  });

  it('rechecks admission between wallet signing and submission', async () => {
    const signer = key(72).toBase58();
    let admissions = 0;
    let submitted = false;
    const client = {
      assertMutationCluster: async () => {
        admissions += 1;
        if (admissions > 1) throw new Error('mutation refused: genesis changed before submit');
        return DEVNET_ADMISSION;
      },
      sendRawTransaction: async () => {
        submitted = true;
        return 'unexpected';
      },
    };
    const signed = await requestWalletTransactionSignatureV1(client, {
      publicKey: { toBase58: () => signer },
      connect: async () => undefined,
      signTransaction: async (transaction: VersionedTransaction) => {
        transaction.signatures[0] = new Uint8Array(64).fill(9);
        return transaction;
      },
    }, unsignedFixture(), signer);
    await expect(submitSignedTransactionV1(client, signed.wireBytes)).rejects.toThrow(/genesis changed/);
    expect(admissions).toBe(2);
    expect(submitted).toBe(false);
  });

  it('cosigns only an exact operation-scoped resolver signature in slot one', async () => {
    const walletKey = Keypair.generate();
    const resolver = Keypair.generate();
    const transaction = cosignFixture(walletKey, resolver);
    transaction.sign([resolver]);
    const wallet = {
      publicKey: { toBase58: () => walletKey.publicKey.toBase58() },
      connect: async () => undefined,
      signTransaction: async (candidate: VersionedTransaction) => {
        candidate.sign([walletKey]);
        return candidate;
      },
    };
    const signed = await requestWalletCosignTransactionV1(
      admittedClient(), wallet, transaction,
      walletKey.publicKey.toBase58(), resolver.publicKey.toBase58(),
    );
    expect(signed.complete).toBe(true);
    expect(signed.transaction.signatures.every((signature) => signature.some((byte) => byte !== 0))).toBe(true);

    const missingResolver = cosignFixture(walletKey, resolver);
    await expect(requestWalletCosignTransactionV1(
      admittedClient(), wallet, missingResolver,
      walletKey.publicKey.toBase58(), resolver.publicKey.toBase58(),
    )).rejects.toThrow(/resolver signature/);

    const substituted = cosignFixture(walletKey, resolver);
    substituted.sign([resolver]);
    substituted.message.recentBlockhash = key(99).toBase58();
    await expect(requestWalletCosignTransactionV1(
      admittedClient(), wallet, substituted,
      walletKey.publicKey.toBase58(), resolver.publicKey.toBase58(),
    )).rejects.toThrow(/resolver signature/);

    const hostile = cosignFixture(walletKey, resolver);
    hostile.sign([resolver]);
    await expect(requestWalletCosignTransactionV1(admittedClient(), {
      ...wallet,
      signTransaction: async (candidate: VersionedTransaction) => {
        candidate.sign([walletKey]);
        candidate.signatures[1] = new Uint8Array(64).fill(9);
        return candidate;
      },
    }, hostile, walletKey.publicKey.toBase58(), resolver.publicKey.toBase58())).rejects.toThrow(/another signature slot/);
  });

  it('distinguishes a fresh writable submit signer from a readonly resolver', async () => {
    const walletKey = Keypair.generate();
    const update = Keypair.generate();
    const transaction = submitCosignFixture(walletKey, update);
    transaction.sign([update]);
    const wallet = {
      publicKey: { toBase58: () => walletKey.publicKey.toBase58() },
      connect: async () => undefined,
      signTransaction: async (candidate: VersionedTransaction) => {
        candidate.sign([walletKey]);
        return candidate;
      },
    };
    await expect(requestWalletSubmitCosignTransactionV1(
      admittedClient(), wallet, transaction,
      walletKey.publicKey.toBase58(), update.publicKey.toBase58(),
    )).resolves.toMatchObject({ complete: true });

    const readonly = cosignFixture(walletKey, update);
    readonly.sign([update]);
    await expect(requestWalletSubmitCosignTransactionV1(
      admittedClient(), wallet, readonly,
      walletKey.publicKey.toBase58(), update.publicKey.toBase58(),
    )).rejects.toThrow(/signer ordering changed/);
  });
});
