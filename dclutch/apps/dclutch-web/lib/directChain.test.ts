import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { fromHex, sha256 } from './bytes';
import { inspectDirectFeePolicy } from './directChain';
import { SolanaRpcClient } from './rpc';

const RAW_RECORD_SEED = new TextEncoder().encode('dclutch-raw-record-v1');
const RELEASE = fromHex('281d896ec0ce69b52443420820bc580ef18ef297e139115df91cea91565c451d', 'release');

function response(result: unknown): Response {
  return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }), { status: 200 });
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

describe('chain-derived compiled Direct authority', () => {
  it('authenticates the exact fee policy content and content-derived PDA', async () => {
    const program = new PublicKey(new Uint8Array(32).fill(44));
    const recipient = new PublicKey(new Uint8Array(32).fill(45));
    const data = new Uint8Array(48);
    data.set(new TextEncoder().encode('DCLTFEE3'));
    new DataView(data.buffer).setUint16(8, 3, true);
    new DataView(data.buffer).setUint16(10, 25, true);
    data.set(recipient.toBytes(), 16);
    const digest = await sha256(data);
    const [address] = PublicKey.findProgramAddressSync([RAW_RECORD_SEED, RELEASE, digest], program);
    const client = new SolanaRpcClient('http://127.0.0.1:8899', async () => response({
      context: { slot: 42 },
      value: { data: [base64(data), 'base64'], executable: false, lamports: 1, owner: program.toBase58(), space: 48 },
    }));
    await expect(inspectDirectFeePolicy(client, program.toBase58(), address.toBase58(), '41')).resolves.toMatchObject({
      address: address.toBase58(), observedSlot: '42', feeBasisPoints: 25, recipient: recipient.toBase58(),
    });
  });

  it('refuses a policy at a substituted address', async () => {
    const program = new PublicKey(new Uint8Array(32).fill(44));
    const data = new Uint8Array(48);
    data.set(new TextEncoder().encode('DCLTFEE3'));
    new DataView(data.buffer).setUint16(8, 3, true);
    data.set(new Uint8Array(32).fill(45), 16);
    const client = new SolanaRpcClient('http://127.0.0.1:8899', async () => response({
      context: { slot: 42 },
      value: { data: [base64(data), 'base64'], executable: false, lamports: 1, owner: program.toBase58(), space: 48 },
    }));
    await expect(inspectDirectFeePolicy(client, program.toBase58(), new PublicKey(new Uint8Array(32).fill(99)).toBase58(), '41')).rejects.toThrow(/content-derived/);
  });
});
