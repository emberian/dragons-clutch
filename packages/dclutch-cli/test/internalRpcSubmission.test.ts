import { SOLANA_DEVNET_GENESIS_HASH_V1, type MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';
import { Keypair, SystemProgram, Transaction } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';

import { submitExactDevnetSignedPacketInternal } from '../src/internal/rpcSubmission';
import { transactionSignatureV1 } from '../src/payoutCompletion';

const DEVNET = SOLANA_DEVNET_GENESIS_HASH_V1;

function response(result: unknown, id: number = 1): Response {
  return new Response(JSON.stringify({ jsonrpc: '2.0', id, result }), {
    headers: { 'content-type': 'application/json' },
    status: 200,
  });
}

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

function client(assertMutationCluster = vi.fn(async (): Promise<MutationClusterAdmissionV1> => Object.freeze({
  endpoint: 'https://devnet.example/',
  genesisHash: DEVNET,
  kind: 'devnet',
}))): Readonly<{ endpoint: string; assertMutationCluster: typeof assertMutationCluster }> {
  return Object.freeze({ endpoint: 'https://devnet.example/', assertMutationCluster });
}

describe('private CLI signed-packet transport', () => {
  it('derives the journal signature, reacquires exact devnet, and sends once with maxRetries zero', async () => {
    const packet = signedPacket();
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => Object.freeze({
      endpoint: 'https://devnet.example/', genesisHash: DEVNET, kind: 'devnet',
    }));
    const fetcher: typeof fetch = vi.fn(async (input, init) => {
      expect(input).toBe('https://devnet.example/');
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      expect(request).toEqual({
        jsonrpc: '2.0', id: 1, method: 'sendTransaction',
        params: [Buffer.from(packet.wire).toString('base64'), {
          encoding: 'base64', skipPreflight: false, preflightCommitment: 'confirmed', maxRetries: 0,
        }],
      });
      return response(packet.signature);
    });
    await expect(submitExactDevnetSignedPacketInternal(
      client(admitted), packet.wire, packet.signature, DEVNET, fetcher,
    )).resolves.toBe(packet.signature);
    expect(admitted).toHaveBeenCalledOnce();
    expect(fetcher).toHaveBeenCalledOnce();
  });

  it('never reaches admission or RPC for an invalid packet', async () => {
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => Object.freeze({
      endpoint: 'https://devnet.example/', genesisHash: DEVNET, kind: 'devnet',
    }));
    const fetcher = vi.fn<typeof fetch>();
    await expect(submitExactDevnetSignedPacketInternal(
      client(admitted), new Uint8Array(1_233), '2'.repeat(88), DEVNET, fetcher,
    )).rejects.toThrow(/1..1232/);
    expect(admitted).not.toHaveBeenCalled();
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('refuses a journal signature that does not come from the exact packet before RPC', async () => {
    const packet = signedPacket();
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => Object.freeze({
      endpoint: 'https://devnet.example/', genesisHash: DEVNET, kind: 'devnet',
    }));
    const fetcher = vi.fn<typeof fetch>();
    await expect(submitExactDevnetSignedPacketInternal(
      client(admitted), packet.wire, '2'.repeat(88), DEVNET, fetcher,
    )).rejects.toThrow(/journal signature does not match/);
    expect(admitted).not.toHaveBeenCalled();
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('never reaches RPC when exact-devnet admission refuses', async () => {
    const packet = signedPacket();
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => {
      throw new Error('mutation refused: the endpoint reports Solana mainnet-beta genesis');
    });
    const fetcher = vi.fn<typeof fetch>();
    await expect(submitExactDevnetSignedPacketInternal(
      client(admitted), packet.wire, packet.signature, DEVNET, fetcher,
    )).rejects.toThrow(/mainnet-beta/);
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('refuses an RPC envelope with another request id', async () => {
    const packet = signedPacket();
    await expect(submitExactDevnetSignedPacketInternal(
      client(), packet.wire, packet.signature, DEVNET, async () => response(packet.signature, 2),
    )).rejects.toThrow(/unbound JSON-RPC envelope/);
  });

  it('refuses an RPC result that is not the exact packet signature', async () => {
    const packet = signedPacket();
    await expect(submitExactDevnetSignedPacketInternal(
      client(), packet.wire, packet.signature, DEVNET, async () => response('2'.repeat(88)),
    )).rejects.toThrow(/another signature than the exact signed packet/);
  });
});
