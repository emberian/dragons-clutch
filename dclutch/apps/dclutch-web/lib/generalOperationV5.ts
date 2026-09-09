import { VersionedTransaction } from '@solana/web3.js';
import { hex, sha256 } from '@dclutch/sdk/bytes';
import { decodeGeneralSuccessorPlanDocumentV5, inspectGeneralSuccessorPlanV5 } from '@dclutch/sdk/generalPlanV5';
import { assertGeneralDeploymentV5, observeGeneralExecutionV5, assertGeneralPreviewCurrentV5, type GeneralExecutionPreviewV5 } from '@dclutch/sdk/generalExecutionV5';
import { MAX_MULTIPLE_ACCOUNTS, type SolanaRpcClient } from '@dclutch/sdk/rpc';
import { requestWalletAddTransactionSignatureV1, submitSignedTransactionV1 } from '@dclutch/sdk/walletHandoff';
import {
  clearFinalizedClientOperationJournalV1, findClientOperationJournalV1, markClientOperationSubmittedV1, requireSubmittedSignatureMatchV1,
  submittedClientOperationWireV1, transactionSignatureV1, writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1, type ClientOperationJournalV1, type ClientOperationScopeV1,
} from './clientOperationJournal';

export async function retainGeneralOperationV5(storage: ClientOperationJournalStorageV1, scope: ClientOperationScopeV1, nativePlan: string, preview: GeneralExecutionPreviewV5) {
  const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(nativePlan));
  if (scope.market !== inspection.plan.market || scope.owner !== inspection.plan.payer) throw new Error('General journal scope differs from native Market or payer');
  if (preview.messageDigest !== hex(await sha256(inspection.transaction.transaction.message.serialize()))) throw new Error('General preview belongs to another native message');
  const intent = JSON.stringify({ format: 'dclutch-general-operation-v5', action: inspection.plan.action, market: scope.market, payer: scope.owner });
  const plan = JSON.stringify({ nativePlan, preview });
  return writeUnsignedClientOperationJournalV1(storage, { ...scope, operation: 'general-v5', intent, plan,
    operationDigest: hex(await sha256(new TextEncoder().encode(JSON.stringify({ intent, plan })))),
  });
}

export async function restoreGeneralOperationV5(journal: ClientOperationJournalV1) {
  if (journal.operation !== 'general-v5') throw new Error('journal is not General V5');
  const plan = JSON.parse(journal.plan) as { nativePlan: string; preview: GeneralExecutionPreviewV5 };
  if (typeof plan !== 'object' || plan === null || Object.keys(plan).sort().join(',') !== 'nativePlan,preview' || typeof plan.nativePlan !== 'string') throw new Error('General journal plan fields differ');
  const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(plan.nativePlan));
  const expectedIntent = JSON.stringify({ format: 'dclutch-general-operation-v5', action: inspection.plan.action, market: inspection.plan.market, payer: inspection.plan.payer });
  if (journal.intent !== expectedIntent || journal.market !== inspection.plan.market || journal.owner !== inspection.plan.payer
      || journal.operationDigest !== hex(await sha256(new TextEncoder().encode(JSON.stringify({ intent: journal.intent, plan: journal.plan }))))) throw new Error('General journal substituted scope, action, or native plan');
  const preview = plan.preview;
  if (preview.messageDigest !== hex(await sha256(inspection.transaction.transaction.message.serialize()))) throw new Error('General retained preview belongs to another native message');
  if (typeof preview !== 'object' || preview === null || !/^(0|[1-9][0-9]*)$/.test(preview.observedSlot) || !Array.isArray(preview.accounts)
      || preview.accounts.length === 0 || preview.accounts.length > MAX_MULTIPLE_ACCOUNTS || !preview.accounts.some((entry) => entry.address === inspection.plan.root)
      || new Set(preview.accounts.map((entry) => entry.address)).size !== preview.accounts.length
      || preview.accounts.some((entry) => !/^[0-9a-f]{64}$/.test(entry.beforeCommitment))) throw new Error('General journal lost its bounded reviewed inputs');
  const transaction = journal.phase === 'submitted' ? VersionedTransaction.deserialize(submittedClientOperationWireV1(journal)) : inspection.transaction.transaction;
  if (hex(transaction.message.serialize()) !== hex(inspection.transaction.transaction.message.serialize())) throw new Error('General journal signed packet substituted the native message');
  return { inspection, preview, transaction, nativePlan: plan.nativePlan };
}

/** Persisted native intent must still exist before a wallet can be prompted. */
export async function signGeneralOperationV5(storage: ClientOperationJournalStorageV1, client: SolanaRpcClient, journal: ClientOperationJournalV1, wallet: unknown, signer: string, selectedTradingProgram: string, partial?: VersionedTransaction, requireCurrent: () => void = () => {}) {
  const current = await findClientOperationJournalV1(storage, journal, 'general-v5');
  if (current === null || current.operationDigest !== journal.operationDigest || current.phase !== 'unsigned') throw new Error('General unsigned journal disappeared or changed before wallet handoff');
  const restored = await restoreGeneralOperationV5(current);
  assertGeneralDeploymentV5(restored.inspection, selectedTradingProgram);
  const admission = await client.assertMutationCluster();
  if (admission.genesisHash !== journal.clusterGenesis) throw new Error('General wallet chain differs from the retained operation');
  await assertGeneralPreviewCurrentV5(client, restored.preview);
  const transaction = partial ?? restored.transaction;
  if (hex(transaction.message.serialize()) !== hex(restored.transaction.message.serialize())) throw new Error('General partial signatures substituted the native message');
  requireCurrent();
  return requestWalletAddTransactionSignatureV1(client, wallet, transaction, signer);
}

/** Persist the first signature and complete packet before the send boundary; resume reuses them. */
export async function submitGeneralOperationV5(storage: ClientOperationJournalStorageV1, client: SolanaRpcClient, journal: ClientOperationJournalV1, selectedTradingProgram: string, signed?: VersionedTransaction, requireCurrent: () => void = () => {}) {
  const restored = await restoreGeneralOperationV5(journal);
  assertGeneralDeploymentV5(restored.inspection, selectedTradingProgram);
  const admission = await client.assertMutationCluster();
  if (admission.genesisHash !== journal.clusterGenesis) throw new Error('General submission chain differs from the retained operation');
  requireCurrent();
  let retained = journal;
  if (journal.phase === 'unsigned') {
    if (!signed || hex(signed.message.serialize()) !== hex(restored.transaction.message.serialize())) throw new Error('General signed transaction differs from the retained native plan');
    await assertGeneralPreviewCurrentV5(client, restored.preview);
    requireCurrent();
    retained = await markClientOperationSubmittedV1(storage, journal, transactionSignatureV1(signed.signatures[0]!), signed.serialize());
  }
  const wire = submittedClientOperationWireV1(retained);
  requireCurrent();
  const signature = await submitSignedTransactionV1(client, wire);
  requireSubmittedSignatureMatchV1(retained.signature!, signature);
  return retained;
}

/** Preserve the verified receipt and signed packet before releasing the single-operation slot. */
export async function archiveGeneralOperationV5(storage: ClientOperationJournalStorageV1, client: SolanaRpcClient, journal: ClientOperationJournalV1, selectedTradingProgram: string): Promise<void> {
  if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('General archive requires a retained signed operation');
  const admission = await client.assertMutationCluster();
  if (admission.genesisHash !== journal.clusterGenesis) throw new Error('General archive chain differs from the retained operation');
  const restored = await restoreGeneralOperationV5(journal);
  assertGeneralDeploymentV5(restored.inspection, selectedTradingProgram);
  const finalized = await observeGeneralExecutionV5(client, restored.inspection, journal.signature, submittedClientOperationWireV1(journal));
  if (finalized === null) throw new Error('General archive requires finalized execution and matching root poststate');
  const archive = JSON.stringify({ format: 'dclutch-general-finalized-root-v5', journal, finalizedSlot: finalized.transaction.slot, observedSlot: finalized.observedSlot, receipt: finalized.receipt }, (_key, value: unknown) => typeof value === 'bigint' ? value.toString() : value);
  storage.setItem(`dclutch.general-finalized-root.v5:${journal.clusterGenesis}:${journal.operationDigest}`, archive);
  await clearFinalizedClientOperationJournalV1(storage, journal);
}
