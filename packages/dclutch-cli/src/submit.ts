/**
 * Sign–submit–confirm, once, with the outcome named.
 *
 * One submission per invocation, no retry loop beyond the RPC's own bounded
 * preflight retries; a refusal comes back with its registered name, because a
 * fail-closed protocol's error codes are documentation and the terminal is
 * where they should read as such.
 */
import type { SolanaRpcClient, TransactionMetaObservation } from '@dclutch/sdk/rpc';
import { renderRefusal } from '@dclutch/sdk/refusals';

import { nameRefusals, type Io } from './output';

const POLL_INTERVAL_MS = 500;
const POLL_ATTEMPTS = 60;

export type SubmitOutcome = Readonly<{
  signature: string;
  succeeded: boolean;
  meta: TransactionMetaObservation | null;
}>;

export async function submitAndConfirm(client: SolanaRpcClient, wire: Uint8Array, io: Io): Promise<SubmitOutcome> {
  let signature: string;
  try {
    signature = await client.sendRawTransaction(wire);
  } catch (error) {
    throw new Error(nameRefusals(error instanceof Error ? error.message : String(error)));
  }
  io.out(`submitted ${signature}`);
  for (let attempt = 0; attempt < POLL_ATTEMPTS; attempt += 1) {
    const [status] = await client.signatureStatuses([signature]);
    if (status !== undefined && status.known && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized')) {
      const meta = status.confirmationStatus === 'finalized' ? await client.transaction(signature).catch(() => null) : null;
      if (status.succeeded === false) {
        const named = status.errorText === null ? 'unnamed failure' : nameRefusals(status.errorText);
        io.err(`transaction landed and was refused: ${named}`);
        return Object.freeze({ signature, succeeded: false, meta });
      }
      io.out(`${status.confirmationStatus} at slot ${status.slot ?? 'unknown'}`);
      return Object.freeze({ signature, succeeded: true, meta });
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
  }
  throw new Error(`${signature} was not confirmed within ${(POLL_INTERVAL_MS * POLL_ATTEMPTS) / 1000}s — check the validator`);
}

/** Render the worker's lamport delta from a finalized meta, when available. */
export function lamportDelta(meta: TransactionMetaObservation, address: string): bigint | null {
  const index = meta.accountAddresses.indexOf(address);
  if (index === -1) return null;
  const before = meta.preBalances[index];
  const after = meta.postBalances[index];
  if (before === undefined || after === undefined) return null;
  return BigInt(after) - BigInt(before);
}

/** Name every custom program error appearing in a meta's log tail. */
export function nameLogRefusals(meta: TransactionMetaObservation): ReadonlyArray<string> {
  const named: string[] = [];
  for (const line of meta.logMessages) {
    const match = /custom program error: (0x[0-9a-fA-F]+|\d+)/.exec(line);
    if (match?.[1] !== undefined) named.push(renderRefusal(Number(match[1])).text);
  }
  return named;
}
