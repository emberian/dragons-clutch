import { VersionedTransaction } from '@solana/web3.js';

import { hex, sha256 } from './bytes';
import {
  submittedClientOperationWireV1,
  type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';
import {
  parseSourceReadinessPlanV1,
  type SourceReadinessAcquisitionV1,
  type SourceReadinessRouteV1,
  type SourceReadinessTransactionV1,
} from './sourceReadinessV1';

export const SOURCE_READINESS_JOURNAL_INTENT_FORMAT_V1 = 'dclutch-source-readiness-journal-intent-v1' as const;
export const SOURCE_READINESS_JOURNAL_PLAN_FORMAT_V1 = 'dclutch-source-readiness-journal-plan-v1' as const;

type SourceReadinessJournalIntentV1 = Readonly<{
  format: typeof SOURCE_READINESS_JOURNAL_INTENT_FORMAT_V1;
  market: string;
  owner: string;
  route: 'create' | 'activate' | 'accept';
  observedSlot: string;
}>;

type SourceReadinessJournalPlanV1 = Readonly<{
  format: typeof SOURCE_READINESS_JOURNAL_PLAN_FORMAT_V1;
  rustPlan: string;
  unsignedWireBase64: string;
  lastValidBlockHeight: string;
}>;

export type RestoredSourceReadinessJournalV1 = Readonly<{
  intent: SourceReadinessJournalIntentV1;
  rustPlan: ReturnType<typeof parseSourceReadinessPlanV1>;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  lastValidBlockHeight: string;
}>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, fields: ReadonlyArray<string>, label: string): void {
  const observed = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (observed.length !== expected.length || observed.some((field, index) => field !== expected[index])) {
    throw new Error(`${label} has missing or unknown fields`);
  }
}

function parseObject(source: string, fields: ReadonlyArray<string>, label: string): Record<string, unknown> {
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error(`${label} is not JSON`); }
  if (!plain(parsed)) throw new Error(`${label} is not one object`);
  exactKeys(parsed, fields, label);
  return parsed;
}

function unsigned(value: unknown, field: string): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)
      || BigInt(value) > 18_446_744_073_709_551_615n) throw new Error(`${field} is not canonical u64 decimal text`);
  return value;
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  return btoa(binary);
}

function base64Bytes(value: unknown): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length > 2_000 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error('Source readiness journal packet is not bounded canonical base64');
  }
  let bytes: Uint8Array;
  try { bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0)); } catch {
    throw new Error('Source readiness journal packet is not bounded canonical base64');
  }
  if (base64(bytes) !== value) throw new Error('Source readiness journal packet is not bounded canonical base64');
  return bytes;
}

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

/** Bind one exact Rust plan and unsigned packet before a wallet is requested. */
export async function sourceReadinessJournalInputV1(
  scope: ClientOperationScopeV1,
  acquisition: SourceReadinessAcquisitionV1,
  transaction: SourceReadinessTransactionV1,
): Promise<ClientOperationScopeV1 & Readonly<{
  operation: 'source-readiness-v1';
  operationDigest: string;
  intent: string;
  plan: string;
}>> {
  if (transaction.payer !== scope.owner || acquisition.plan.route !== transaction.route
      || acquisition.plan.observedSlot !== transaction.observedSlot) {
    throw new Error('Source readiness journal scope, Rust plan, and transaction disagree');
  }
  const intent: SourceReadinessJournalIntentV1 = Object.freeze({
    format: SOURCE_READINESS_JOURNAL_INTENT_FORMAT_V1,
    market: scope.market,
    owner: scope.owner,
    route: transaction.route,
    observedSlot: transaction.observedSlot,
  });
  const plan: SourceReadinessJournalPlanV1 = Object.freeze({
    format: SOURCE_READINESS_JOURNAL_PLAN_FORMAT_V1,
    rustPlan: acquisition.planJson,
    unsignedWireBase64: base64(transaction.wireBytes),
    lastValidBlockHeight: transaction.lastValidBlockHeight,
  });
  const operationPreimage = new TextEncoder().encode(JSON.stringify({ intent, plan }));
  return Object.freeze({
    ...scope,
    operation: 'source-readiness-v1',
    operationDigest: hex(await sha256(operationPreimage)),
    intent: JSON.stringify(intent),
    plan: JSON.stringify(plan),
  });
}

/** Hostile-decode an exact saved unsigned/submitted Source readiness act. */
export async function restoreSourceReadinessJournalV1(
  journal: ClientOperationJournalV1,
): Promise<RestoredSourceReadinessJournalV1> {
  if (journal.operation !== 'source-readiness-v1') throw new Error('journal is not a Source readiness operation');
  const rawIntent = parseObject(journal.intent,
    ['format', 'market', 'observedSlot', 'owner', 'route'], 'Source readiness journal intent');
  if (rawIntent.format !== SOURCE_READINESS_JOURNAL_INTENT_FORMAT_V1
      || rawIntent.market !== journal.market || rawIntent.owner !== journal.owner
      || !['create', 'activate', 'accept'].includes(String(rawIntent.route))) {
    throw new Error('Source readiness journal intent substituted its format, scope, or route');
  }
  const intent: SourceReadinessJournalIntentV1 = Object.freeze({
    format: SOURCE_READINESS_JOURNAL_INTENT_FORMAT_V1,
    market: journal.market,
    owner: journal.owner,
    route: rawIntent.route as SourceReadinessJournalIntentV1['route'],
    observedSlot: unsigned(rawIntent.observedSlot, 'journal observed slot'),
  });
  const rawPlan = parseObject(journal.plan,
    ['format', 'lastValidBlockHeight', 'rustPlan', 'unsignedWireBase64'], 'Source readiness journal plan');
  if (rawPlan.format !== SOURCE_READINESS_JOURNAL_PLAN_FORMAT_V1 || typeof rawPlan.rustPlan !== 'string') {
    throw new Error('Source readiness journal plan has another format or no Rust plan');
  }
  const rustPlan = parseSourceReadinessPlanV1(rawPlan.rustPlan);
  if (rustPlan.route !== intent.route || rustPlan.observedSlot !== intent.observedSlot) {
    throw new Error('Source readiness saved Rust plan substituted its route or observation');
  }
  const wireBytes = base64Bytes(rawPlan.unsignedWireBase64);
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wireBytes); } catch {
    throw new Error('Source readiness saved packet is not one Solana transaction');
  }
  if (!sameBytes(transaction.serialize(), wireBytes)
      || transaction.signatures.length !== 1
      || transaction.signatures[0]?.some((byte) => byte !== 0)
      || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== journal.owner) {
    throw new Error('Source readiness saved packet is not canonical, unsigned, and sole-payer');
  }
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (!sameBytes(signed.message.serialize(), transaction.message.serialize())) {
      throw new Error('Source readiness submitted packet substituted the saved unsigned message');
    }
  }
  const rebuilt = await sourceReadinessJournalInputV1(journal, {
    plan: rustPlan,
    planJson: rawPlan.rustPlan,
    snapshotJson: '{}',
    observationAddresses: Object.freeze([]),
  }, {
    transaction,
    wireBytes,
    payer: journal.owner,
    route: intent.route,
    observedSlot: intent.observedSlot,
    lastValidBlockHeight: unsigned(rawPlan.lastValidBlockHeight, 'journal last valid block height'),
  });
  if (rebuilt.operationDigest !== journal.operationDigest
      || rebuilt.intent !== journal.intent || rebuilt.plan !== journal.plan) {
    throw new Error('Source readiness journal digest does not authenticate its intent and plan');
  }
  return Object.freeze({
    intent,
    rustPlan,
    transaction,
    wireBytes,
    lastValidBlockHeight: unsigned(rawPlan.lastValidBlockHeight, 'journal last valid block height'),
  });
}

/** Prove that finalized state advanced exactly one adjacent readiness edge. */
export function sourceReadinessPoststateCompletesV1(
  before: 'create' | 'activate' | 'accept',
  after: SourceReadinessRouteV1,
): boolean {
  return (before === 'create' && after === 'activate')
    || (before === 'activate' && after === 'accept')
    || (before === 'accept' && (after === 'complete' || after === 'consumed-by-founding'));
}
