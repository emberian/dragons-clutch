import { VersionedTransaction } from '@solana/web3.js';

import { hex, sha256 } from './bytes';
import { submittedClientOperationWireV1, type ClientOperationJournalV1, type ClientOperationScopeV1 } from './clientOperationJournal';
import {
  buildSourceCloseFundTransactionV1,
  parseSourceCloseFundPlanV1,
  type SourceCloseFundAcquisitionV1,
  type SourceCloseFundPlanV1,
  type SourceCloseFundTransactionV1,
} from './sourceCloseFundV1';

export const SOURCE_CLOSE_FUND_JOURNAL_INTENT_FORMAT_V1 = 'dclutch-source-close-fund-journal-intent-v1' as const;
export const SOURCE_CLOSE_FUND_JOURNAL_PLAN_FORMAT_V1 = 'dclutch-source-close-fund-journal-plan-v1' as const;

type IntentV1 = Readonly<{
  format: typeof SOURCE_CLOSE_FUND_JOURNAL_INTENT_FORMAT_V1;
  market: string;
  owner: string;
  observedSlot: string;
  route: 'prepay' | 'close';
  receipt: string;
}>;

type SavedPlanV1 = Readonly<{
  format: typeof SOURCE_CLOSE_FUND_JOURNAL_PLAN_FORMAT_V1;
  rustPlan: string;
  unsignedWireBase64: string;
  lastValidBlockHeight: string;
}>;

export type RestoredSourceCloseFundJournalV1 = Readonly<{
  intent: IntentV1;
  plan: SourceCloseFundPlanV1;
  transaction: VersionedTransaction;
  lastValidBlockHeight: string;
}>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function object(source: string, fields: ReadonlyArray<string>, label: string): Record<string, unknown> {
  let value: unknown;
  try { value = JSON.parse(source); } catch { throw new Error(`${label} is not JSON`); }
  if (!plain(value)) throw new Error(`${label} is not an object`);
  const actual = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (actual.length !== expected.length || actual.some((field, index) => field !== expected[index])) throw new Error(`${label} has missing or unknown fields`);
  return value;
}

function u64(value: unknown, label: string): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)
      || BigInt(value) > 18_446_744_073_709_551_615n) throw new Error(`${label} is not canonical u64`);
  return value;
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  return btoa(binary);
}

function base64Bytes(value: unknown): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length > 2_000 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) throw new Error('Source close journal packet is not bounded canonical base64');
  let bytes: Uint8Array;
  try { bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0)); } catch { throw new Error('Source close journal packet is not bounded canonical base64'); }
  if (base64(bytes) !== value) throw new Error('Source close journal packet is not canonical base64');
  return bytes;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

/** Bind one exact Rust close route and unsigned sole-wallet packet. */
export async function sourceCloseFundJournalInputV1(
  scope: ClientOperationScopeV1,
  acquisition: SourceCloseFundAcquisitionV1,
  transaction: SourceCloseFundTransactionV1,
): Promise<ClientOperationScopeV1 & Readonly<{ operation: 'source-close-fund-v1'; operationDigest: string; intent: string; plan: string }>> {
  if (transaction.payer !== scope.owner || transaction.route !== acquisition.plan.route
      || transaction.observedSlot !== acquisition.plan.observedSlot) throw new Error('Source close scope, route, plan, and packet disagree');
  const rebuilt = buildSourceCloseFundTransactionV1(acquisition, scope.owner, {
    blockhash: transaction.transaction.message.recentBlockhash,
    lastValidBlockHeight: transaction.lastValidBlockHeight,
  });
  if (!same(rebuilt.wireBytes, transaction.wireBytes)) throw new Error('Source close packet does not reproduce the Rust plan exactly');
  const receipt = acquisition.plan.route === 'prepay'
    ? acquisition.plan.prepay!.destination
    : acquisition.plan.accounts!.completion[2];
  if (receipt === undefined) throw new Error('Source close plan omitted its canonical receipt');
  const intent: IntentV1 = Object.freeze({ format: SOURCE_CLOSE_FUND_JOURNAL_INTENT_FORMAT_V1,
    market: scope.market, owner: scope.owner, observedSlot: transaction.observedSlot,
    route: acquisition.plan.route, receipt });
  const plan: SavedPlanV1 = Object.freeze({ format: SOURCE_CLOSE_FUND_JOURNAL_PLAN_FORMAT_V1,
    rustPlan: acquisition.planJson, unsignedWireBase64: base64(transaction.wireBytes),
    lastValidBlockHeight: transaction.lastValidBlockHeight });
  const operationDigest = hex(await sha256(new TextEncoder().encode(JSON.stringify({ intent, plan }))));
  return Object.freeze({ ...scope, operation: 'source-close-fund-v1', operationDigest,
    intent: JSON.stringify(intent), plan: JSON.stringify(plan) });
}

/** Reauthenticate the exact saved unsigned or submitted Source close act. */
export async function restoreSourceCloseFundJournalV1(
  journal: ClientOperationJournalV1,
): Promise<RestoredSourceCloseFundJournalV1> {
  if (journal.operation !== 'source-close-fund-v1') throw new Error('journal is not a Source close operation');
  const rawIntent = object(journal.intent, ['format', 'market', 'observedSlot', 'owner', 'receipt', 'route'], 'Source close intent');
  if (rawIntent.format !== SOURCE_CLOSE_FUND_JOURNAL_INTENT_FORMAT_V1 || rawIntent.market !== journal.market
      || rawIntent.owner !== journal.owner || !['prepay', 'close'].includes(String(rawIntent.route))
      || typeof rawIntent.receipt !== 'string') throw new Error('Source close intent substituted format, scope, route, or receipt');
  const intent: IntentV1 = Object.freeze({ format: SOURCE_CLOSE_FUND_JOURNAL_INTENT_FORMAT_V1,
    market: journal.market, owner: journal.owner, observedSlot: u64(rawIntent.observedSlot, 'observed slot'),
    route: rawIntent.route as 'prepay' | 'close', receipt: rawIntent.receipt });
  const rawPlan = object(journal.plan, ['format', 'lastValidBlockHeight', 'rustPlan', 'unsignedWireBase64'], 'Source close plan');
  if (rawPlan.format !== SOURCE_CLOSE_FUND_JOURNAL_PLAN_FORMAT_V1 || typeof rawPlan.rustPlan !== 'string') throw new Error('Source close saved plan changed format');
  const plan = parseSourceCloseFundPlanV1(rawPlan.rustPlan);
  const receipt = plan.route === 'prepay' ? plan.prepay!.destination : plan.accounts!.completion[2];
  if (plan.route !== intent.route || plan.observedSlot !== intent.observedSlot || receipt !== intent.receipt) throw new Error('Source close Rust plan substituted route, observation, or receipt');
  const wire = base64Bytes(rawPlan.unsignedWireBase64);
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wire); } catch { throw new Error('Source close saved packet is not a transaction'); }
  if (!same(transaction.serialize(), wire) || transaction.signatures.length !== 1
      || transaction.signatures[0]?.some((byte) => byte !== 0)
      || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== journal.owner) throw new Error('Source close packet changed sole-wallet authority');
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (!same(signed.message.serialize(), transaction.message.serialize())) throw new Error('submitted Source close packet substituted the saved message');
  }
  const acquisition: SourceCloseFundAcquisitionV1 = Object.freeze({ plan, planJson: rawPlan.rustPlan,
    snapshotJson: '{}', observationAddresses: Object.freeze([]) });
  const rebuilt = await sourceCloseFundJournalInputV1(journal, acquisition, {
    transaction, wireBytes: wire, payer: journal.owner, route: plan.route,
    observedSlot: intent.observedSlot, lastValidBlockHeight: u64(rawPlan.lastValidBlockHeight, 'last valid block height'),
  });
  if (rebuilt.operationDigest !== journal.operationDigest || rebuilt.intent !== journal.intent || rebuilt.plan !== journal.plan) throw new Error('Source close journal digest mismatch');
  return Object.freeze({ intent, plan, transaction,
    lastValidBlockHeight: u64(rawPlan.lastValidBlockHeight, 'last valid block height') });
}
