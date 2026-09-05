import { SOLANA_DEVNET_GENESIS_HASH_V1, type MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';
import { Keypair, SystemProgram, Transaction } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';

import { submitExactDevnetSignedPacketInternal } from '../src/internal/rpcSubmission';
import { transactionSignatureV1 } from '../src/payoutCompletion';

const DEVNET = SOLANA_DEVNET_GENESIS_HASH_V1;

function signedPacket(): Readonly<{ wire: Uint8Array; signature: string }> {
  const signer = Keypair.generate();
  const transaction = new Transaction({
    feePayer: signer.publicKey,
    recentBlockhash: SystemProgram.programId.toBase58(),
  }).add(SystemProgram.transfer({ fromPubkey: signer.publicKey, toPubkey: Keypair.generate().publicKey, lamports: 1 }));
  transaction.sign(signer);
  return Object.freeze({
    wire: Uint8Array.from(transaction.serialize()),
    signature: transactionSignatureV1(transaction.signature ?? new Uint8Array()),
  });
}

const admittedDevnet = (): MutationClusterAdmissionV1 => Object.freeze({
  endpoint: 'https://devnet.example/', genesisHash: DEVNET, kind: 'devnet',
});

/** A client whose two methods are the only two the transport may reach. */
function client(
  assertMutationCluster = vi.fn(async () => admittedDevnet()),
  sendRawTransaction = vi.fn(async (_bytes: Uint8Array, _options?: Readonly<{ maxRetries?: 0 | 3 }>): Promise<string> => '2'.repeat(88)),
) {
  return Object.freeze({ assertMutationCluster, sendRawTransaction });
}

describe('CLI signed-packet submission over the SDK transport', () => {
  it('derives the journal signature, reacquires exact devnet, and sends once with maxRetries zero', async () => {
    const packet = signedPacket();
    const admitted = vi.fn(async () => admittedDevnet());
    const send = vi.fn(async (bytes: Uint8Array, options?: Readonly<{ maxRetries?: 0 | 3 }>): Promise<string> => {
      expect(bytes).toEqual(packet.wire);
      expect(options).toEqual({ maxRetries: 0 });
      return packet.signature;
    });
    await expect(submitExactDevnetSignedPacketInternal(client(admitted, send), packet.wire, packet.signature, DEVNET))
      .resolves.toBe(packet.signature);
    expect(admitted).toHaveBeenCalledOnce();
    expect(send).toHaveBeenCalledOnce();
  });

  it('never reaches admission or the transport for an invalid packet', async () => {
    const admitted = vi.fn(async () => admittedDevnet());
    const send = vi.fn(async (): Promise<string> => '2'.repeat(88));
    await expect(submitExactDevnetSignedPacketInternal(client(admitted, send), new Uint8Array(1_233), '2'.repeat(88), DEVNET))
      .rejects.toThrow(/1..1232/);
    expect(admitted).not.toHaveBeenCalled();
    expect(send).not.toHaveBeenCalled();
  });

  it('refuses a journal signature that does not come from the exact packet before the transport', async () => {
    const packet = signedPacket();
    const admitted = vi.fn(async () => admittedDevnet());
    const send = vi.fn(async (): Promise<string> => packet.signature);
    await expect(submitExactDevnetSignedPacketInternal(client(admitted, send), packet.wire, '2'.repeat(88), DEVNET))
      .rejects.toThrow(/journal signature does not match/);
    expect(admitted).not.toHaveBeenCalled();
    expect(send).not.toHaveBeenCalled();
  });

  it('never reaches the transport when exact-devnet admission refuses', async () => {
    const packet = signedPacket();
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => {
      throw new Error('mutation refused: the endpoint reports Solana mainnet-beta genesis');
    });
    const send = vi.fn(async (): Promise<string> => packet.signature);
    await expect(submitExactDevnetSignedPacketInternal(client(admitted, send), packet.wire, packet.signature, DEVNET))
      .rejects.toThrow(/mainnet-beta/);
    expect(send).not.toHaveBeenCalled();
  });

  it('refuses a transport result that is not the exact packet signature', async () => {
    const packet = signedPacket();
    await expect(submitExactDevnetSignedPacketInternal(client(), packet.wire, packet.signature, DEVNET))
      .rejects.toThrow(/another signature than the exact signed packet/);
  });
});
