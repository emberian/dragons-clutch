/**
 * Private transaction transport for CLI commands that already own the exact
 * signed packet and their command-specific durable state machine.
 *
 * This file is not part of `@dclutch/sdk` and the CLI package has no library
 * export map. Public SDK consumers get a read-only `SolanaRpcClient`; only a
 * caller that has crossed its own journal boundary calls this transport.
 */
import type { MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';

import { assertExactDevnetMutation } from '../mutation';

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
  devnetAcknowledgment: string,
  options: Readonly<{ maxRetries?: 0 | 3 }> = {},
  fetcher: typeof fetch = ambientFetch,
): Promise<string> {
  if (!(wireBytes instanceof Uint8Array) || wireBytes.length === 0 || wireBytes.length > SOLANA_PACKET_BYTES) {
    throw new Error(`signed transaction must contain 1..${SOLANA_PACKET_BYTES} bytes`);
  }
  await assertExactDevnetMutation(client, devnetAcknowledgment, 'raw transaction transport');
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
        id: 1,
        method: 'sendTransaction',
        params: [Buffer.from(wireBytes).toString('base64'), {
          encoding: 'base64',
          skipPreflight: false,
          preflightCommitment: 'confirmed',
          maxRetries: options.maxRetries ?? 3,
        }],
      }),
      signal: controller.signal,
    });
    const payload = await boundedJson(response);
    if (!plain(payload) || payload.jsonrpc !== '2.0') throw new Error('sendTransaction returned an invalid JSON-RPC envelope');
    if (payload.error !== undefined) {
      const message = plain(payload.error) && typeof payload.error.message === 'string'
        ? payload.error.message.slice(0, 240)
        : 'unknown RPC refusal';
      throw new Error(`sendTransaction refused: ${message}`);
    }
    const signature = payload.result;
    if (typeof signature !== 'string' || signature.length < 64 || signature.length > 96
        || !/^[1-9A-HJ-NP-Za-km-z]+$/.test(signature)) {
      throw new Error('sendTransaction returned a noncanonical base58 signature');
    }
    return signature;
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw new Error(`sendTransaction timed out after ${RPC_TIMEOUT_MS / 1_000} seconds`);
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}
