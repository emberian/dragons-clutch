import { VersionedTransaction } from '@solana/web3.js';
import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  findClientOperationJournalV1, markClientOperationSubmittedV1,
  requireSubmittedSignatureMatchV1, submittedClientOperationWireV1,
  transactionSignatureV1, writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1, type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';

const OPERATION = 'dealer-liquidity-v4' as const;

export type DealerSavedPlanV1 = Readonly<{
  inputJson: string;
  planJson: string;
  unsignedWireBase64: string;
}>;

export function dealerBytesBase64V1(bytes: Uint8Array): string {
  let output = '';
  for (let start = 0; start < bytes.length; start += 8192) output += String.fromCharCode(...bytes.subarray(start, start + 8192));
  return btoa(output);
}

function decodeWire(source: string): Uint8Array {
  if (source.length > 2000 || source.length === 0) throw new Error('Saved Dealer transaction exceeds its packet bound');
  let bytes: Uint8Array;
  try { bytes = Uint8Array.from(atob(source), (c) => c.charCodeAt(0)); } catch { throw new Error('Saved Dealer packet is not base64'); }
  if (dealerBytesBase64V1(bytes) !== source) throw new Error('Saved Dealer packet is not canonical base64');
  return bytes;
}

/** Persist the complete review before any wallet request; never replace ambiguity. */
export async function retainDealerPlanV1(
  storage: ClientOperationJournalStorageV1,
  scope: ClientOperationScopeV1,
  inputJson: string,
  planJson: string,
  transaction: VersionedTransaction,
): Promise<ClientOperationJournalV1> {
  return writeUnsignedClientOperationJournalV1(storage, {
    ...scope, operation: OPERATION,
    operationDigest: hex(await sha256(new TextEncoder().encode(inputJson))),
    intent: inputJson,
    plan: JSON.stringify({ inputJson, planJson, unsignedWireBase64: dealerBytesBase64V1(transaction.serialize()) }),
  });
}

export function findDealerOperationV1(storage: ClientOperationJournalStorageV1, scope: ClientOperationScopeV1) {
  return findClientOperationJournalV1(storage, scope, OPERATION);
}

/** Storage supplies a candidate only. The caller must rerun the native planner. */
export function restoreDealerPlanV1(journal: ClientOperationJournalV1): Readonly<{ saved: DealerSavedPlanV1; transaction: VersionedTransaction }> {
  if (journal.operation !== OPERATION) throw new Error('Saved operation is not Dealer liquidity');
  let raw: unknown;
  try { raw = JSON.parse(journal.plan); } catch { throw new Error('Saved Dealer plan is not JSON'); }
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('Saved Dealer plan is not an object');
  const saved = raw as Record<string, unknown>;
  if (Object.keys(saved).sort().join(',') !== 'inputJson,planJson,unsignedWireBase64'
    || typeof saved.inputJson !== 'string' || saved.inputJson !== journal.intent
    || typeof saved.planJson !== 'string' || typeof saved.unsignedWireBase64 !== 'string') {
    throw new Error('Saved Dealer plan has substituted its input or fields');
  }
  const transaction = VersionedTransaction.deserialize(decodeWire(saved.unsignedWireBase64));
  if (dealerBytesBase64V1(transaction.serialize()) !== saved.unsignedWireBase64
    || transaction.signatures.some((signature) => signature.some((byte) => byte !== 0))) {
    throw new Error('Saved Dealer unsigned packet is not canonical unsigned material');
  }
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (dealerBytesBase64V1(signed.message.serialize()) !== dealerBytesBase64V1(transaction.message.serialize())) {
      throw new Error('Saved Dealer signed message differs from the reviewed message');
    }
  }
  return { saved: saved as DealerSavedPlanV1, transaction };
}

/** Save the packet identity BEFORE the send; a timeout remains recoverable. */
export async function submitDealerOperationV1(
  storage: ClientOperationJournalStorageV1,
  journal: ClientOperationJournalV1,
  signed: VersionedTransaction,
  send: (bytes: Uint8Array) => Promise<string>,
): Promise<ClientOperationJournalV1> {
  const restored = restoreDealerPlanV1(journal);
  if (dealerBytesBase64V1(signed.message.serialize()) !== dealerBytesBase64V1(restored.transaction.message.serialize())) {
    throw new Error('Dealer submission changed the reviewed unsigned message');
  }
  if (journal.phase === 'submitted') throw new Error('Dealer operation was already submitted; recover its exact signature');
  const wire = signed.serialize();
  const signature = transactionSignatureV1(signed.signatures[0]!);
  const submitted = await markClientOperationSubmittedV1(storage, journal, signature, wire);
  requireSubmittedSignatureMatchV1(signature, await send(wire));
  return submitted;
}

/** Every edit invalidates in-flight discovery/build/sign results synchronously. */
export class DealerSelectionGuardV1 {
  private revision = 0;
  invalidate(): number { this.revision += 1; return this.revision; }
  current(): number { return this.revision; }
  require(revision: number): void {
    if (revision !== this.revision) throw new Error('Dealer selection changed while this operation was pending; review the current selection');
  }
}
