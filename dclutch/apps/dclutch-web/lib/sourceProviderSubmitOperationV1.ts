import { PublicKey, VersionedMessage, VersionedTransaction } from '@solana/web3.js';

import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  submittedClientOperationWireV1,
  type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';
import {
  parseSourceProviderSubmitPlanV1,
  type SourceProviderSubmitAcquisitionV1,
} from './sourceProviderV1';

export const SOURCE_PROVIDER_SUBMIT_JOURNAL_INTENT_FORMAT_V1 = 'dclutch-source-provider-submit-journal-intent-v1' as const;
export const SOURCE_PROVIDER_SUBMIT_JOURNAL_PLAN_FORMAT_V1 = 'dclutch-source-provider-submit-journal-plan-v1' as const;

type SourceProviderSubmitJournalIntentV1 = Readonly<{
  format: typeof SOURCE_PROVIDER_SUBMIT_JOURNAL_INTENT_FORMAT_V1;
  market: string;
  owner: string;
  lifecycle: string;
  update: string;
  route: 'submit';
  observedSlot: string;
}>;

type SourceProviderSubmitJournalPlanV1 = Readonly<{
  format: typeof SOURCE_PROVIDER_SUBMIT_JOURNAL_PLAN_FORMAT_V1;
  rustPlan: string;
  unsignedWireBase64: string;
  lastValidBlockHeight: string;
}>;

export type RestoredSourceProviderSubmitJournalV1 = Readonly<{
  intent: SourceProviderSubmitJournalIntentV1;
  rustPlan: ReturnType<typeof parseSourceProviderSubmitPlanV1>;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  lastValidBlockHeight: string;
}>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function parseObject(source: string, fields: ReadonlyArray<string>, label: string): Record<string, unknown> {
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error(`${label} is not JSON`); }
  if (!plain(parsed)) throw new Error(`${label} is not one object`);
  const actual = Object.keys(parsed).sort();
  const expected = [...fields].sort();
  if (actual.length !== expected.length || actual.some((field, index) => field !== expected[index])) throw new Error(`${label} has missing or unknown fields`);
  return parsed;
}

function key(value: unknown, label: string): string {
  if (typeof value !== 'string') throw new Error(`${label} is not text`);
  const parsed = new PublicKey(value).toBase58();
  if (parsed !== value) throw new Error(`${label} is not canonical base58`);
  return parsed;
}

function unsigned(value: unknown, label: string): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)
      || BigInt(value) > 18_446_744_073_709_551_615n) throw new Error(`${label} is not canonical u64 text`);
  return value;
}

function base64(bytes: Uint8Array): string {
  let output = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) output += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  return btoa(output);
}

function base64Bytes(value: unknown): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length > 2_000 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) throw new Error('Source provider submit journal packet is not bounded canonical base64');
  const bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
  if (base64(bytes) !== value) throw new Error('Source provider submit journal packet is not bounded canonical base64');
  return bytes;
}

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

/** Bind the exact Rust submit message before the update account or wallet signs. */
export async function sourceProviderSubmitJournalInputV1(
  scope: ClientOperationScopeV1,
  acquisition: SourceProviderSubmitAcquisitionV1,
): Promise<ClientOperationScopeV1 & Readonly<{
  operation: 'source-provider-submit-v1'; operationDigest: string; intent: string; plan: string;
}>> {
  if (scope.market !== acquisition.market || scope.owner !== acquisition.payer
      || acquisition.plan.requiredSigners[0] !== scope.owner
      || acquisition.plan.requiredSigners[1] !== acquisition.update.publicKey.toBase58()) {
    throw new Error('Source provider submit journal scope, wallet, update signer, and Rust plan disagree');
  }
  const intent: SourceProviderSubmitJournalIntentV1 = Object.freeze({
    format: SOURCE_PROVIDER_SUBMIT_JOURNAL_INTENT_FORMAT_V1,
    market: scope.market,
    owner: scope.owner,
    lifecycle: acquisition.plan.poststate.lifecycle,
    update: acquisition.update.publicKey.toBase58(),
    route: 'submit',
    observedSlot: acquisition.plan.observedSlot,
  });
  const unsignedWire = acquisition.transaction.serialize();
  if (acquisition.transaction.signatures.some((signature) => signature.some((byte) => byte !== 0))) throw new Error('Source provider submit journal received a transaction already signed');
  const plan: SourceProviderSubmitJournalPlanV1 = Object.freeze({
    format: SOURCE_PROVIDER_SUBMIT_JOURNAL_PLAN_FORMAT_V1,
    rustPlan: acquisition.planJson,
    unsignedWireBase64: base64(unsignedWire),
    lastValidBlockHeight: unsigned(acquisition.lastValidBlockHeight, 'last valid block height'),
  });
  return Object.freeze({
    ...scope,
    operation: 'source-provider-submit-v1',
    operationDigest: hex(await sha256(new TextEncoder().encode(JSON.stringify({ intent, plan })))),
    intent: JSON.stringify(intent),
    plan: JSON.stringify(plan),
  });
}

/** Hostile-decode one exact saved provider submission. */
export async function restoreSourceProviderSubmitJournalV1(
  journal: ClientOperationJournalV1,
): Promise<RestoredSourceProviderSubmitJournalV1> {
  if (journal.operation !== 'source-provider-submit-v1') throw new Error('journal is not a Source provider submit operation');
  const rawIntent = parseObject(journal.intent,
    ['format', 'lifecycle', 'market', 'observedSlot', 'owner', 'route', 'update'], 'Source provider submit journal intent');
  if (rawIntent.format !== SOURCE_PROVIDER_SUBMIT_JOURNAL_INTENT_FORMAT_V1 || rawIntent.route !== 'submit'
      || rawIntent.market !== journal.market || rawIntent.owner !== journal.owner) throw new Error('Source provider submit journal intent substituted its scope or route');
  const intent: SourceProviderSubmitJournalIntentV1 = Object.freeze({
    format: SOURCE_PROVIDER_SUBMIT_JOURNAL_INTENT_FORMAT_V1,
    market: key(rawIntent.market, 'journal Market'),
    owner: key(rawIntent.owner, 'journal owner'),
    lifecycle: key(rawIntent.lifecycle, 'journal lifecycle'),
    update: key(rawIntent.update, 'journal update'),
    route: 'submit',
    observedSlot: unsigned(rawIntent.observedSlot, 'journal observed slot'),
  });
  const rawPlan = parseObject(journal.plan,
    ['format', 'lastValidBlockHeight', 'rustPlan', 'unsignedWireBase64'], 'Source provider submit journal plan');
  if (rawPlan.format !== SOURCE_PROVIDER_SUBMIT_JOURNAL_PLAN_FORMAT_V1 || typeof rawPlan.rustPlan !== 'string') throw new Error('Source provider submit journal plan has another format or no Rust plan');
  const rustPlan = parseSourceProviderSubmitPlanV1(rawPlan.rustPlan);
  if (rustPlan.observedSlot !== intent.observedSlot || rustPlan.poststate.lifecycle !== intent.lifecycle
      || rustPlan.poststate.updateAccount !== intent.update || rustPlan.requiredSigners[0] !== intent.owner
      || rustPlan.requiredSigners[1] !== intent.update) {
    throw new Error('Source provider saved submit plan substituted its route, account, or authority');
  }
  const wireBytes = base64Bytes(rawPlan.unsignedWireBase64);
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wireBytes); } catch { throw new Error('Source provider saved submit packet is not one Solana transaction'); }
  const expectedMessage = VersionedMessage.deserialize(Uint8Array.from(atob(rustPlan.unsignedMessageBase64), (character) => character.charCodeAt(0)));
  if (!sameBytes(transaction.message.serialize(), expectedMessage.serialize())
      || transaction.signatures.length !== 2
      || transaction.signatures.some((signature) => signature.some((byte) => byte !== 0))
      || transaction.message.header.numRequiredSignatures !== 2
      || transaction.message.staticAccountKeys[0]?.toBase58() !== intent.owner
      || transaction.message.staticAccountKeys[1]?.toBase58() !== intent.update
      || !transaction.message.isAccountWritable(1)
      || transaction.message.addressTableLookups.length !== 1
      || transaction.message.addressTableLookups[0]?.accountKey.toBase58() !== rustPlan.lookupTables[0]) {
    throw new Error('Source provider saved submit packet differs from the exact unsigned Rust message');
  }
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (!sameBytes(signed.message.serialize(), transaction.message.serialize())) throw new Error('Source provider submitted packet substituted the exact unsigned Rust message');
  }
  const operationDigest = hex(await sha256(new TextEncoder().encode(JSON.stringify({
    intent: JSON.parse(journal.intent), plan: JSON.parse(journal.plan),
  }))));
  if (operationDigest !== journal.operationDigest) throw new Error('Source provider submit journal operation digest changed');
  return Object.freeze({
    intent,
    rustPlan,
    transaction,
    wireBytes,
    lastValidBlockHeight: unsigned(rawPlan.lastValidBlockHeight, 'last valid block height'),
  });
}
