/**
 * Private transaction transport for CLI commands that already own the exact
 * signed packet and their command-specific durable state machine.
 *
 * This file is not part of `@dclutch/sdk`, and the CLI's exact export map does
 * not expose source modules. Public SDK consumers get a read-only
 * `SolanaRpcClient`; only a caller that crossed its own journal boundary calls
 * this transport.
 */
import type { MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';
import { VersionedTransaction } from '@solana/web3.js';

import { assertExactDevnetMutation } from '../mutation';
import { transactionSignatureV1 } from '../payoutCompletion';

const SOLANA_PACKET_BYTES = 1_232;
const MAX_RPC_RESPONSE_BYTES = 32 * 1024;
const RPC_TIMEOUT_MS = 15_000;

type SubmissionClient = Readonly<{
  endpoint: string;
  assertMutationCluster(): Promise<MutationClusterAdmissionV1>;
}>;

const ambientFetch: typeof fetch = (input, init) => globalThis.fetch(input, init);

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

async function boundedJson(response: Response): Promise<unknown> {
  if (!response.ok) throw new Error(`sendTransaction HTTP status ${response.status}`);
  const declared = response.headers.get('content-length');
  if (declared !== null) {
    const length = Number(declared);
    if (!Number.isSafeInteger(length) || length < 0 || length > MAX_RPC_RESPONSE_BYTES) {
      throw new Error('sendTransaction response exceeds the CLI byte bound');
    }
  }
  const reader = response.body?.getReader();
  if (reader === undefined) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.length > MAX_RPC_RESPONSE_BYTES) throw new Error('sendTransaction response exceeds the CLI byte bound');
    return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
  }
  const chunks: Uint8Array[] = [];
  let length = 0;
  for (;;) {
    const next = await reader.read();
    if (next.done) break;
    length += next.value.length;
    if (length > MAX_RPC_RESPONSE_BYTES) {
      await reader.cancel();
      throw new Error('sendTransaction response exceeds the CLI byte bound');
    }
    chunks.push(next.value);
  }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
}

/** Submit one already-journaled packet after reacquiring exact devnet. */
export async function submitExactDevnetSignedPacketInternal(
  client: SubmissionClient,
  wireBytes: Uint8Array,
  expectedSignature: string,
  devnetAcknowledgment: string,
  fetcher: typeof fetch = ambientFetch,
): Promise<string> {
  if (!(wireBytes instanceof Uint8Array) || wireBytes.length === 0 || wireBytes.length > SOLANA_PACKET_BYTES) {
    throw new Error(`signed transaction must contain 1..${SOLANA_PACKET_BYTES} bytes`);
  }
  let transaction: VersionedTransaction;
  try {
    transaction = VersionedTransaction.deserialize(wireBytes);
  } catch {
    throw new Error('signed transaction is not one canonical Solana packet');
  }
  const canonicalWire = transaction.serialize();
  if (canonicalWire.length !== wireBytes.length
      || canonicalWire.some((byte, index) => byte !== wireBytes[index])) {
    throw new Error('signed transaction does not round-trip to the exact packet');
  }
  const derivedSignature = transactionSignatureV1(transaction.signatures[0] ?? new Uint8Array());
  if (expectedSignature !== derivedSignature) {
    throw new Error('submitted journal signature does not match the exact signed packet');
  }
  await assertExactDevnetMutation(client, devnetAcknowledgment, 'raw transaction transport');
  const requestId = 1;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), RPC_TIMEOUT_MS);
  try {
    const response = await fetcher(client.endpoint, {
      method: 'POST',
      credentials: 'omit',
      redirect: 'error',
      referrerPolicy: 'no-referrer',
      headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: requestId,
        method: 'sendTransaction',
        params: [Buffer.from(wireBytes).toString('base64'), {
          encoding: 'base64',
          skipPreflight: false,
          preflightCommitment: 'confirmed',
          maxRetries: 0,
        }],
      }),
      signal: controller.signal,
    });
    const payload = await boundedJson(response);
    if (!plain(payload) || payload.jsonrpc !== '2.0' || payload.id !== requestId) {
      throw new Error('sendTransaction returned an unbound JSON-RPC envelope');
    }
    if (payload.error !== undefined) {
      const message = plain(payload.error) && typeof payload.error.message === 'string'
        ? payload.error.message.slice(0, 240)
        : 'unknown RPC refusal';
      throw new Error(`sendTransaction refused: ${message}`);
    }
    if (payload.result !== expectedSignature) {
      throw new Error('sendTransaction returned another signature than the exact signed packet');
    }
    return expectedSignature;
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new Error(`sendTransaction timed out after ${RPC_TIMEOUT_MS / 1_000} seconds`);
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}
