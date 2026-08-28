import { SOLANA_DEVNET_GENESIS_HASH_V1, type MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';
import { describe, expect, it, vi } from 'vitest';

import { submitExactDevnetSignedPacketInternal } from '../src/internal/rpcSubmission';

const DEVNET = SOLANA_DEVNET_GENESIS_HASH_V1;

function response(result: unknown): Response {
  return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }), {
    headers: { 'content-type': 'application/json' },
    status: 200,
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
  it('reacquires exact devnet and sends one bounded packet with preflight', async () => {
    const signature = '2'.repeat(88);
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => Object.freeze({
      endpoint: 'https://devnet.example/', genesisHash: DEVNET, kind: 'devnet',
    }));
    const fetcher: typeof fetch = vi.fn(async (input, init) => {
      expect(input).toBe('https://devnet.example/');
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      expect(request).toEqual({
        jsonrpc: '2.0', id: 1, method: 'sendTransaction',
        params: [Buffer.from([1, 2, 3]).toString('base64'), {
          encoding: 'base64', skipPreflight: false, preflightCommitment: 'confirmed', maxRetries: 0,
        }],
      });
      return response(signature);
    });
    await expect(submitExactDevnetSignedPacketInternal(
      client(admitted), Uint8Array.from([1, 2, 3]), DEVNET, { maxRetries: 0 }, fetcher,
    )).resolves.toBe(signature);
    expect(admitted).toHaveBeenCalledOnce();
    expect(fetcher).toHaveBeenCalledOnce();
  });

  it('never reaches admission or RPC for an invalid packet', async () => {
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => Object.freeze({
      endpoint: 'https://devnet.example/', genesisHash: DEVNET, kind: 'devnet',
    }));
    const fetcher = vi.fn<typeof fetch>();
    await expect(submitExactDevnetSignedPacketInternal(
      client(admitted), new Uint8Array(1_233), DEVNET, {}, fetcher,
    )).rejects.toThrow(/1..1232/);
    expect(admitted).not.toHaveBeenCalled();
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('never reaches RPC when exact-devnet admission refuses', async () => {
    const admitted = vi.fn(async (): Promise<MutationClusterAdmissionV1> => {
      throw new Error('mutation refused: the endpoint reports Solana mainnet-beta genesis');
    });
    const fetcher = vi.fn<typeof fetch>();
    await expect(submitExactDevnetSignedPacketInternal(
      client(admitted), Uint8Array.from([1]), DEVNET, {}, fetcher,
    )).rejects.toThrow(/mainnet-beta/);
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('refuses an RPC result that is not a canonical signature', async () => {
    await expect(submitExactDevnetSignedPacketInternal(
      client(), Uint8Array.from([1]), DEVNET, {}, async () => response('not-a-signature'),
    )).rejects.toThrow(/noncanonical base58 signature/);
  });
});
