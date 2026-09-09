import { VersionedTransaction } from '@solana/web3.js';
import { hex, sha256 } from '@dclutch/sdk/bytes';
import { type SolanaRpcClient, type TransactionMetaObservation } from '@dclutch/sdk/rpc';
import {
  clearFinalizedClientOperationJournalV1,
  findClientOperationJournalV1, markClientOperationSubmittedV1,
  requireSubmittedSignatureMatchV1, submittedClientOperationWireV1,
  transactionSignatureV1, writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1, type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';
import { parseStructuredLifecycleIntentV1, type StructuredLifecycleIntentV1 } from './structuredLifecycleModelV1';

const OPERATION = 'structured-lifecycle-v1' as const;
export type StructuredLifecycleSavedReviewV1 = Readonly<{
  nativeInput: string;
  nativePlan: string;
  unsignedPacket: string;
}>;
export type StructuredLifecycleRestoredV1 = Readonly<{
  intent: StructuredLifecycleIntentV1;
  review: StructuredLifecycleSavedReviewV1;
  transaction: VersionedTransaction;
}>;

function base64(bytes: Uint8Array): string { return btoa(String.fromCharCode(...bytes)); }
async function digest(intent: string, plan: string): Promise<string> {
  return hex(await sha256(new TextEncoder().encode(JSON.stringify({ intent, plan }))));
}

/** A journal holds one native step. Replan the same intent only after that step finalizes. */
export async function retainStructuredLifecycleStepV1(
  storage: ClientOperationJournalStorageV1, scope: ClientOperationScopeV1,
  intent: StructuredLifecycleIntentV1, nativeInput: string, nativePlan: string,
  transaction: VersionedTransaction,
): Promise<ClientOperationJournalV1> {
  const canonical = parseStructuredLifecycleIntentV1(intent);
  if (scope.market !== canonical.market || scope.owner !== canonical.payer) throw new Error('Structured journal differs from the selected Market or payer');
  if (transaction.message.staticAccountKeys[0]?.toBase58() !== canonical.payer
      || transaction.message.header.numRequiredSignatures !== 1
      || transaction.signatures.some((signature) => signature.some((byte) => byte !== 0))) {
    throw new Error('Structured review requires one unsigned payer packet');
  }
  const bytes = transaction.serialize();
  if (bytes.length > 1232) throw new Error('Structured step exceeds the Solana packet limit');
  const intentText = JSON.stringify(canonical);
  const plan = JSON.stringify({ nativeInput, nativePlan, unsignedPacket: base64(bytes) });
  return writeUnsignedClientOperationJournalV1(storage, { ...scope, operation: OPERATION,
    operationDigest: await digest(intentText, plan), intent: intentText, plan });
}

export function findStructuredLifecycleStepV1(storage: ClientOperationJournalStorageV1, scope: ClientOperationScopeV1) {
  return findClientOperationJournalV1(storage, scope, OPERATION);
}

/** Stored native input/output remain untrusted until the native adapter rechecks them. */
export async function restoreStructuredLifecycleStepV1(journal: ClientOperationJournalV1): Promise<StructuredLifecycleRestoredV1> {
  if (journal.operation !== OPERATION) throw new Error('Saved operation is not Structured lifecycle');
  const intent = parseStructuredLifecycleIntentV1(JSON.parse(journal.intent));
  if (intent.market !== journal.market || intent.payer !== journal.owner
      || journal.operationDigest !== await digest(journal.intent, journal.plan)) throw new Error('Structured journal changed its intent, scope or review');
  const raw: unknown = JSON.parse(journal.plan);
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Structured review is not an object');
  const value = raw as Record<string, unknown>;
  if (Object.keys(value).sort().join(',') !== 'nativeInput,nativePlan,unsignedPacket'
      || typeof value.nativeInput !== 'string' || typeof value.nativePlan !== 'string'
      || typeof value.unsignedPacket !== 'string' || value.unsignedPacket.length > 1644) throw new Error('Structured review has invalid fields');
  const bytes = Uint8Array.from(atob(value.unsignedPacket), (character) => character.charCodeAt(0));
  if (bytes.length === 0 || bytes.length > 1232 || base64(bytes) !== value.unsignedPacket) throw new Error('Structured review packet is not bounded canonical base64');
  const transaction = VersionedTransaction.deserialize(bytes);
  if (base64(transaction.serialize()) !== value.unsignedPacket
      || transaction.message.staticAccountKeys[0]?.toBase58() !== intent.payer
      || transaction.message.header.numRequiredSignatures !== 1
      || transaction.signatures.some((signature) => signature.some((byte) => byte !== 0))) throw new Error('Structured review changed its unsigned payer packet');
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (hex(signed.message.serialize()) !== hex(transaction.message.serialize())) throw new Error('Structured submitted packet differs from its reviewed message');
  }
  return { intent, review: value as StructuredLifecycleSavedReviewV1, transaction };
}

/** The caller rechecks native construction/current prestate before the wallet opens. */
export async function signStructuredLifecycleStepV1<T>(
  storage: ClientOperationJournalStorageV1, journal: ClientOperationJournalV1,
  verifyCurrent: (restored: StructuredLifecycleRestoredV1) => Promise<void>,
  sign: (transaction: VersionedTransaction) => Promise<T>,
): Promise<T> {
  const current = await findStructuredLifecycleStepV1(storage, journal);
  if (current === null || current.phase !== 'unsigned' || current.operationDigest !== journal.operationDigest) throw new Error('Structured unsigned review disappeared or changed before wallet handoff');
  const restored = await restoreStructuredLifecycleStepV1(current);
  await verifyCurrent(restored);
  const retained = await findStructuredLifecycleStepV1(storage, journal);
  if (retained === null || retained.phase !== 'unsigned' || retained.operationDigest !== current.operationDigest) throw new Error('Structured unsigned review changed during verification');
  return sign(restored.transaction);
}

/** Persist ambiguity before RPC. Recovery polls the saved signature and never rebuilds it. */
export async function submitStructuredLifecycleStepV1(
  storage: ClientOperationJournalStorageV1, journal: ClientOperationJournalV1,
  signed: VersionedTransaction,
  verifyCurrent: (restored: StructuredLifecycleRestoredV1) => Promise<void>,
  send: (bytes: Uint8Array) => Promise<string>,
): Promise<ClientOperationJournalV1> {
  const current = await findStructuredLifecycleStepV1(storage, journal);
  if (current === null || current.operationDigest !== journal.operationDigest) throw new Error('Structured review disappeared or changed before submission');
  if (current.phase === 'submitted') throw new Error('Structured step already submitted; recover its retained signature');
  const restored = await restoreStructuredLifecycleStepV1(current);
  if (hex(signed.message.serialize()) !== hex(restored.transaction.message.serialize())) throw new Error('Structured signed packet changed the reviewed message');
  await verifyCurrent(restored);
  const submitted = await markClientOperationSubmittedV1(storage, current,
    transactionSignatureV1(signed.signatures[0]!), signed.serialize());
  requireSubmittedSignatureMatchV1(submitted.signature!, await send(submittedClientOperationWireV1(submitted)));
  return submitted;
}

/** Finalized packet identity precedes native poststate verification and journal removal. */
export async function finalizeStructuredLifecycleStepV1(
  storage: ClientOperationJournalStorageV1, journal: ClientOperationJournalV1,
  client: Pick<SolanaRpcClient, 'assertMutationCluster' | 'signatureStatuses' | 'transaction'>,
  verifyFinalized: (restored: StructuredLifecycleRestoredV1, landed: TransactionMetaObservation) => Promise<void>,
): Promise<Readonly<{ signature: string; slot: string }>> {
  const current = await findStructuredLifecycleStepV1(storage, journal);
  if (current === null || current.phase !== 'submitted' || current.operationDigest !== journal.operationDigest) throw new Error('Structured submitted journal disappeared or changed before reconciliation');
  const restored = await restoreStructuredLifecycleStepV1(current);
  if ((await client.assertMutationCluster()).genesisHash !== current.clusterGenesis) throw new Error('Structured recovery endpoint belongs to another chain');
  const status = (await client.signatureStatuses([current.signature!]))[0];
  if (!status?.known || status.confirmationStatus !== 'finalized' || status.slot === null) throw new Error('Structured step is not finalized; its signed packet remains retained');
  if (status.succeeded !== true) throw new Error(`Structured step refused: ${status.errorText ?? 'unknown transaction refusal'}; its packet remains retained`);
  const landed = await client.transaction(current.signature!);
  if (landed === null) throw new Error('Structured finalized packet is not available; keep the saved operation');
  if (!landed.succeeded || landed.signature !== current.signature || landed.slot !== status.slot
      || hex(landed.transactionBytes) !== hex(submittedClientOperationWireV1(current))) throw new Error('Structured finalized transaction differs from the retained signed packet');
  await verifyFinalized(restored, landed);
  await clearFinalizedClientOperationJournalV1(storage, current);
  return { signature: current.signature!, slot: landed.slot };
}
