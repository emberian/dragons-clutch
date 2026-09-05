import { VersionedTransaction } from '@solana/web3.js';

import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  submittedClientOperationWireV1,
  type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';
import type {
  SourceTerminalAcquisitionV1,
  SourceTerminalPlanV1,
  SourceTerminalTransactionV1,
} from './sourceTerminalV1';
import { buildSourceTerminalTransactionV1, parseSourceTerminalPlanV1 } from './sourceTerminalV1';

export const SOURCE_TERMINAL_JOURNAL_INTENT_FORMAT_V1 = 'dclutch-source-terminal-journal-intent-v1' as const;
export const SOURCE_TERMINAL_JOURNAL_PLAN_FORMAT_V1 = 'dclutch-source-terminal-journal-plan-v1' as const;

type IntentV1 = Readonly<{
  format: typeof SOURCE_TERMINAL_JOURNAL_INTENT_FORMAT_V1;
  market: string;
  owner: string;
  observedSlot: string;
  certificate: string;
}>;

type SavedPlanV1 = Readonly<{
  format: typeof SOURCE_TERMINAL_JOURNAL_PLAN_FORMAT_V1;
  rustPlan: string;
  unsignedWireBase64: string;
  lastValidBlockHeight: string;
}>;

export type RestoredSourceTerminalJournalV1 = Readonly<{
  intent: IntentV1;
  plan: SourceTerminalPlanV1;
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
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error('Source terminal journal packet is not bounded canonical base64');
  }
  let bytes: Uint8Array;
  try { bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0)); } catch {
    throw new Error('Source terminal journal packet is not bounded canonical base64');
  }
  if (base64(bytes) !== value) throw new Error('Source terminal journal packet is not canonical base64');
  return bytes;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

/** Bind one exact Rust admission plan and unsigned sole-wallet packet. */
export async function sourceTerminalJournalInputV1(
  scope: ClientOperationScopeV1,
  acquisition: SourceTerminalAcquisitionV1,
  transaction: SourceTerminalTransactionV1,
): Promise<ClientOperationScopeV1 & Readonly<{
  operation: 'source-terminal-v1'; operationDigest: string; intent: string; plan: string;
}>> {
  if (acquisition.plan.route !== 'admit' || transaction.payer !== scope.owner
      || transaction.observedSlot !== acquisition.plan.observedSlot) throw new Error('Source terminal scope, plan, and packet disagree');
  const rebuilt = buildSourceTerminalTransactionV1(acquisition, scope.owner, {
    blockhash: transaction.transaction.message.recentBlockhash,
    lastValidBlockHeight: transaction.lastValidBlockHeight,
  });
  if (!same(rebuilt.wireBytes, transaction.wireBytes)) throw new Error('Source terminal packet does not reproduce the Rust plan exactly');
  const certificate = acquisition.plan.accounts.completion[2];
  if (certificate === undefined || acquisition.plan.accounts.completion[0] !== scope.market) throw new Error('Source terminal plan substituted Market or certificate');
  const intent: IntentV1 = Object.freeze({ format: SOURCE_TERMINAL_JOURNAL_INTENT_FORMAT_V1,
    market: scope.market, owner: scope.owner, observedSlot: transaction.observedSlot, certificate });
  const plan: SavedPlanV1 = Object.freeze({ format: SOURCE_TERMINAL_JOURNAL_PLAN_FORMAT_V1,
    rustPlan: acquisition.planJson, unsignedWireBase64: base64(transaction.wireBytes),
    lastValidBlockHeight: transaction.lastValidBlockHeight });
  const operationDigest = hex(await sha256(new TextEncoder().encode(JSON.stringify({ intent, plan }))));
  return Object.freeze({ ...scope, operation: 'source-terminal-v1', operationDigest,
    intent: JSON.stringify(intent), plan: JSON.stringify(plan) });
}

/** Reauthenticate the exact saved unsigned or submitted terminal admission. */
export async function restoreSourceTerminalJournalV1(
  journal: ClientOperationJournalV1,
): Promise<RestoredSourceTerminalJournalV1> {
  if (journal.operation !== 'source-terminal-v1') throw new Error('journal is not a Source terminal operation');
  const rawIntent = object(journal.intent, ['certificate', 'format', 'market', 'observedSlot', 'owner'], 'Source terminal intent');
  if (rawIntent.format !== SOURCE_TERMINAL_JOURNAL_INTENT_FORMAT_V1 || rawIntent.market !== journal.market
      || rawIntent.owner !== journal.owner || typeof rawIntent.certificate !== 'string') throw new Error('Source terminal intent substituted its format or scope');
  const intent: IntentV1 = Object.freeze({ format: SOURCE_TERMINAL_JOURNAL_INTENT_FORMAT_V1,
    market: journal.market, owner: journal.owner, observedSlot: u64(rawIntent.observedSlot, 'observed slot'),
    certificate: rawIntent.certificate });
  const rawPlan = object(journal.plan, ['format', 'lastValidBlockHeight', 'rustPlan', 'unsignedWireBase64'], 'Source terminal plan');
  if (rawPlan.format !== SOURCE_TERMINAL_JOURNAL_PLAN_FORMAT_V1 || typeof rawPlan.rustPlan !== 'string') throw new Error('Source terminal saved plan changed format');
  const plan = parseSourceTerminalPlanV1(rawPlan.rustPlan);
  if (plan.route !== 'admit') throw new Error('saved Source terminal Rust plan is not an executable admission');
  if (plan.observedSlot !== intent.observedSlot || plan.accounts.completion[0] !== journal.market
      || plan.accounts.completion[2] !== intent.certificate) throw new Error('Source terminal Rust plan substituted its observation or completion');
  const wire = base64Bytes(rawPlan.unsignedWireBase64);
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wire); } catch { throw new Error('Source terminal saved packet is not a transaction'); }
  if (!same(transaction.serialize(), wire) || transaction.signatures.length !== 1
      || transaction.signatures[0]?.some((byte) => byte !== 0)
      || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== journal.owner) throw new Error('Source terminal packet changed sole-wallet authority');
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (!same(signed.message.serialize(), transaction.message.serialize())) throw new Error('submitted Source terminal packet substituted the saved message');
  }
  const rebuilt = await sourceTerminalJournalInputV1(journal,
    { plan, planJson: rawPlan.rustPlan, snapshotJson: '{}', observationAddresses: Object.freeze([]) },
    { transaction, wireBytes: wire, payer: journal.owner, observedSlot: intent.observedSlot,
      lastValidBlockHeight: u64(rawPlan.lastValidBlockHeight, 'last valid block height') });
  if (rebuilt.operationDigest !== journal.operationDigest || rebuilt.intent !== journal.intent || rebuilt.plan !== journal.plan) throw new Error('Source terminal journal digest mismatch');
  return Object.freeze({ intent, plan, transaction,
    lastValidBlockHeight: u64(rawPlan.lastValidBlockHeight, 'last valid block height') });
}
