import { type SolanaRpcClient } from '@dclutch/sdk/rpc';
import {
  clearFinalizedClientOperationJournalV1, findClientOperationJournalV1,
  submittedClientOperationWireV1,
  type ClientOperationJournalStorageV1, type ClientOperationJournalV1,
} from './clientOperationJournal';

export type FinalizedClientOperationRefusalV1 = Readonly<{
  format: 'dclutch-finalized-client-operation-refusal-v1';
  journal: ClientOperationJournalV1;
  finalizedSlot: string;
  error: unknown;
  errorText: string | null;
  feeLamports: string;
}>;

/**
 * Explicitly finish a refused transaction, keeping its exact evidence first.
 * Unknown, processed, confirmed, successful, or substituted packets never clear
 * the active operation. This does not resubmit or construct a replacement.
 */
export async function archiveFinalizedRefusedClientOperationV1(
  storage: ClientOperationJournalStorageV1,
  client: Pick<SolanaRpcClient, 'assertMutationCluster' | 'signatureStatuses' | 'transaction'>,
  journal: ClientOperationJournalV1,
): Promise<FinalizedClientOperationRefusalV1> {
  const current = await findClientOperationJournalV1(storage, journal, journal.operation);
  if (current === null || current.operationDigest !== journal.operationDigest || current.phase !== 'submitted' || current.signature === null) {
    throw new Error('There is no matching submitted operation to resolve');
  }
  const wire = submittedClientOperationWireV1(current);
  const chain = await client.assertMutationCluster();
  if (chain.genesisHash !== current.clusterGenesis) throw new Error('Refusal recovery is on another chain');
  const status = (await client.signatureStatuses([current.signature]))[0];
  if (!status?.known || status.signature !== current.signature || status.confirmationStatus !== 'finalized' || status.succeeded !== false || status.slot === null) {
    throw new Error('Only an exact finalized refusal can release this operation');
  }
  const transaction = await client.transaction(current.signature);
  if (transaction === null || transaction.signature !== current.signature || transaction.slot !== status.slot || transaction.succeeded !== false || transaction.error === null
    || wire.length !== transaction.transactionBytes.length || wire.some((byte, n) => byte !== transaction.transactionBytes[n])) {
    throw new Error('Finalized refusal does not match the exact retained signed packet');
  }
  const archive: FinalizedClientOperationRefusalV1 = {
    format: 'dclutch-finalized-client-operation-refusal-v1', journal: current, finalizedSlot: transaction.slot,
    error: transaction.error, errorText: transaction.errorText, feeLamports: transaction.feeLamports,
  };
  const key = `dclutch.client-operation-refusal.v1:${current.operation}:${current.clusterGenesis}:${current.market}:${current.owner}:${current.operationDigest}`;
  const encoded = JSON.stringify(archive);
  const prior = storage.getItem(key);
  if (prior !== null && prior !== encoded) throw new Error('Another refusal archive already occupies this operation identity');
  storage.setItem(key, encoded);
  if (storage.getItem(key) !== encoded) throw new Error('The finalized refusal archive was not retained; the operation remains active');
  // Finalized refusal is a terminal outcome: no protocol mutation committed.
  // Its immutable receipt above remains available after the active slot clears.
  await clearFinalizedClientOperationJournalV1(storage, current);
  return archive;
}
