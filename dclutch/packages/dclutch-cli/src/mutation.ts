/**
 * The public CLI's one mutation admission: this invocation names Solana
 * devnet by its full genesis hash, and the endpoint proves that same identity
 * again at every signing or submission boundary.
 *
 * The SDK intentionally also admits explicitly addressed loopback validators
 * for local development. The public trader CLI does not: `--i-mean-devnet`
 * is a narrower, user-visible authorization and must not be widened by an RPC
 * URL, a stale earlier observation, or the SDK's local-validator allowance.
 */
import {
  SOLANA_DEVNET_GENESIS_HASH_V1,
  type LatestBlockhashObservation,
  type MutationClusterAdmissionV1,
  type SolanaRpcClient,
} from '@dclutch/sdk/rpc';

import type { CliContext } from './context';

type MutationAdmissionClient = Pick<SolanaRpcClient, 'assertMutationCluster'>;
type MutationBlockhashClient = MutationAdmissionClient & Pick<SolanaRpcClient, 'latestMutationBlockhash'>;

/** Require the exact, known devnet identity before any signer is consulted. */
export function devnetGenesisAcknowledgment(context: CliContext): string {
  const acknowledgment = context.flags['i-mean-devnet'];
  if (typeof acknowledgment !== 'string') {
    throw new Error('pass --i-mean-devnet <full devnet genesis hash>');
  }
  if (acknowledgment !== SOLANA_DEVNET_GENESIS_HASH_V1) {
    throw new Error(`--i-mean-devnet must equal Solana devnet's full genesis hash ${SOLANA_DEVNET_GENESIS_HASH_V1}`);
  }
  return acknowledgment;
}

/**
 * Reacquire and bind the endpoint identity at one named mutation boundary.
 *
 * Admissions are never cached. A devnet observation made while inspecting a
 * route cannot authorize a later signature if an endpoint or proxy has been
 * substituted in between.
 */
export async function assertExactDevnetMutation(
  client: MutationAdmissionClient,
  acknowledgment: string,
  boundary: string,
): Promise<MutationClusterAdmissionV1> {
  if (acknowledgment !== SOLANA_DEVNET_GENESIS_HASH_V1) {
    throw new Error(`${boundary} refused: the invocation did not acknowledge Solana devnet's exact genesis hash`);
  }
  const admission = await client.assertMutationCluster();
  if (admission.kind !== 'devnet' || admission.genesisHash !== acknowledgment) {
    throw new Error(`${boundary} refused: the endpoint no longer reports the exact acknowledged devnet genesis`);
  }
  return admission;
}

/** Acquire a transaction blockhash only inside a fresh exact-devnet boundary. */
export async function latestExactDevnetBlockhash(
  client: MutationBlockhashClient,
  acknowledgment: string,
  boundary: string,
  minimumContextSlot?: string,
): Promise<LatestBlockhashObservation> {
  await assertExactDevnetMutation(client, acknowledgment, boundary);
  return client.latestMutationBlockhash(minimumContextSlot);
}
