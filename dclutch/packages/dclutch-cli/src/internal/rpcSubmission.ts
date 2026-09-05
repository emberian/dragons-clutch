/**
 * The CLI's signed-packet submission: the checks a terminal owes before the
 * one bounded send the SDK performs.
 *
 * `SolanaRpcClient.sendRawTransaction` is the single transport (one packet,
 * preflight on, the cluster's genesis rechecked, no retry loop). What this
 * file adds is the CLI's own discipline around it: the packet must round-trip
 * as one canonical Solana packet, the journal's signature must be derived
 * from that exact packet, and exact-devnet admission must be reacquired under
 * the acknowledgment the operator recorded, before any byte leaves the
 * process. The returned signature must be the journal's own.
 */
import type { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { VersionedTransaction } from '@solana/web3.js';

import { assertExactDevnetMutation } from '../mutation';
import { transactionSignatureV1 } from '../payoutCompletion';

const SOLANA_PACKET_BYTES = 1_232;

type SubmissionClient = Pick<SolanaRpcClient, 'assertMutationCluster' | 'sendRawTransaction'>;

/** Submit one already-journaled packet after reacquiring exact devnet. */
export async function submitExactDevnetSignedPacketInternal(
  client: SubmissionClient,
  wireBytes: Uint8Array,
  expectedSignature: string,
  devnetAcknowledgment: string,
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
  const submitted = await client.sendRawTransaction(wireBytes, { maxRetries: 0 });
  if (submitted !== expectedSignature) {
    throw new Error('sendTransaction returned another signature than the exact signed packet');
  }
  return expectedSignature;
}
