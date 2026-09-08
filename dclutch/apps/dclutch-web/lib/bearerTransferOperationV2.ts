import { PublicKey, VersionedTransaction } from '@solana/web3.js';

import { hex, sha256 } from '@dclutch/sdk/bytes';
import type { BearerTransferPlanV2, BearerTransferPoststateV2 } from '@dclutch/sdk/rationalTokenV2';
import {
  submittedClientOperationWireV1,
  type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';

const INTENT_FORMAT = 'dclutch-bearer-transfer-intent-v2';
const PLAN_FORMAT = 'dclutch-bearer-transfer-plan-v2';
const U64_MAX = 18_446_744_073_709_551_615n;

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactObject(source: string, expected: ReadonlyArray<string>, label: string): Record<string, unknown> {
  let decoded: unknown;
  try { decoded = JSON.parse(source); } catch { throw new Error(`${label} is not JSON`); }
  if (!plain(decoded)) throw new Error(`${label} is not one object`);
  const fields = Object.keys(decoded).sort(); const sorted = [...expected].sort();
  if (fields.length !== sorted.length || fields.some((field, index) => field !== sorted[index])) {
    throw new Error(`${label} has missing or unknown fields`);
  }
  return decoded;
}

function address(value: unknown, label: string): string {
  if (typeof value !== 'string') throw new Error(`${label} is not one canonical Solana address`);
  let canonical: string;
  try { canonical = new PublicKey(value).toBase58(); } catch { throw new Error(`${label} is not one canonical Solana address`); }
  if (canonical !== value) throw new Error(`${label} is not canonical base58 text`);
  return value;
}

function u64(value: unknown, label: string): bigint {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value) || BigInt(value) > U64_MAX) {
    throw new Error(`${label} is not canonical u64`);
  }
  return BigInt(value);
}

function decimal(value: unknown): number {
  if (!Number.isInteger(value) || Number(value) < 0 || Number(value) > 255) throw new Error('display decimals are not canonical u8');
  return Number(value);
}

function poststateJson(poststate: BearerTransferPoststateV2): Record<string, string | number> {
  return {
    authority: poststate.authority, destination: poststate.destination,
    destinationAfter: poststate.destinationAfter.toString(), destinationBefore: poststate.destinationBefore.toString(),
    destinationOwner: poststate.destinationOwner, displayDecimals: poststate.displayDecimals,
    market: poststate.market, mint: poststate.mint, mintController: poststate.mintController,
    mintMetadata: poststate.mintMetadata, mintSupply: poststate.mintSupply.toString(), payer: poststate.payer,
    rawAmount: poststate.rawAmount.toString(), source: poststate.source, sourceAfter: poststate.sourceAfter.toString(),
    sourceBefore: poststate.sourceBefore.toString(), sourceOwner: poststate.sourceOwner,
  };
}

/** Bind recovery metadata to one exact unsigned message and expected balances. */
export async function bearerTransferJournalInputV2(
  scope: ClientOperationScopeV1,
  plan: BearerTransferPlanV2,
  lastValidBlockHeight: string,
): Promise<ClientOperationScopeV1 & Readonly<{
  operation: 'bearer-transfer-v2'; operationDigest: string; intent: string; plan: string;
}>> {
  if (scope.market !== plan.poststate.market || scope.owner !== plan.poststate.payer) {
    throw new Error('Bearer transfer journal scope differs from its Market or payer');
  }
  const expectedSigners = new Set([plan.poststate.payer, plan.poststate.authority]);
  const messageSigners = plan.transaction.message.staticAccountKeys
    .slice(0, plan.transaction.message.header.numRequiredSignatures).map((value) => value.toBase58());
  if (plan.requiredSigners.length !== expectedSigners.size || messageSigners.length !== expectedSigners.size
      || plan.requiredSigners.some((value) => !expectedSigners.has(value))
      || messageSigners.some((value) => !expectedSigners.has(value))
      || plan.transaction.signatures.some((signature) => signature.some((byte) => byte !== 0))) {
    throw new Error('Bearer transfer journal packet has another signer set or is already signed');
  }
  u64(lastValidBlockHeight, 'last valid block height');
  const messageBytes = plan.transaction.message.serialize();
  const intent = JSON.stringify({ format: INTENT_FORMAT, ...poststateJson(plan.poststate) });
  const savedPlan = JSON.stringify({
    format: PLAN_FORMAT, lastValidBlockHeight, messageBase64: base64(messageBytes),
  });
  return Object.freeze({ ...scope, operation: 'bearer-transfer-v2' as const,
    operationDigest: hex(await sha256(messageBytes)), intent, plan: savedPlan });
}

/** Keep the economic actor distinct and collect its signature before the fee payer. */
export function nextBearerTransferSignerV2(
  transaction: VersionedTransaction,
  payer: string,
  authority: string,
): Readonly<{ address: string; role: 'source transfer authority' | 'transaction payer' }> | null {
  address(payer, 'payer'); address(authority, 'authority');
  const required = transaction.message.header.numRequiredSignatures;
  const signers = transaction.message.staticAccountKeys.slice(0, required).map((value) => value.toBase58());
  const expected = new Set([payer, authority]);
  if (transaction.signatures.length !== required || signers.length !== expected.size
      || signers.some((value) => !expected.has(value))) throw new Error('Bearer transfer packet has another signer set');
  const empty = (value: string) => transaction.signatures[signers.indexOf(value)]!.every((byte) => byte === 0);
  if (authority !== payer && empty(authority)) return Object.freeze({ address: authority, role: 'source transfer authority' as const });
  if (empty(payer)) return Object.freeze({ address: payer, role: 'transaction payer' as const });
  return null;
}

export type RestoredBearerTransferJournalV2 = Readonly<{
  poststate: BearerTransferPoststateV2;
  lastValidBlockHeight: string;
}>;

/** Hostile-decode a saved plan and bind any submitted packet to its message. */
export async function restoreBearerTransferJournalV2(
  journal: ClientOperationJournalV1,
): Promise<RestoredBearerTransferJournalV2> {
  if (journal.operation !== 'bearer-transfer-v2') throw new Error('journal is not one Bearer transfer');
  const intentFields = ['authority', 'destination', 'destinationAfter', 'destinationBefore', 'destinationOwner',
    'displayDecimals', 'format', 'market', 'mint', 'mintController', 'mintMetadata', 'mintSupply', 'payer', 'rawAmount', 'source', 'sourceAfter',
    'sourceBefore', 'sourceOwner'];
  const value = exactObject(journal.intent, intentFields, 'Bearer transfer intent');
  if (value.format !== INTENT_FORMAT || value.market !== journal.market || value.payer !== journal.owner) {
    throw new Error('Bearer transfer intent substituted its format or scope');
  }
  const poststate: BearerTransferPoststateV2 = Object.freeze({
    market: address(value.market, 'Market'), payer: address(value.payer, 'payer'), authority: address(value.authority, 'authority'),
    mint: address(value.mint, 'Mint'), mintController: address(value.mintController, 'Mint controller'),
    mintMetadata: value.mintMetadata === 'absent' || value.mintMetadata === 'immutable-self-hosted' ? value.mintMetadata : (() => { throw new Error('Mint metadata mode is undefined'); })(),
    source: address(value.source, 'source'), destination: address(value.destination, 'destination'),
    rawAmount: u64(value.rawAmount, 'raw amount'), displayDecimals: decimal(value.displayDecimals),
    mintSupply: u64(value.mintSupply, 'Mint supply'), sourceOwner: address(value.sourceOwner, 'source owner'),
    sourceBefore: u64(value.sourceBefore, 'source before'), sourceAfter: u64(value.sourceAfter, 'source after'),
    destinationOwner: address(value.destinationOwner, 'destination owner'),
    destinationBefore: u64(value.destinationBefore, 'destination before'), destinationAfter: u64(value.destinationAfter, 'destination after'),
  });
  if (poststate.rawAmount === 0n || poststate.sourceBefore - poststate.rawAmount !== poststate.sourceAfter
      || poststate.destinationBefore + poststate.rawAmount !== poststate.destinationAfter) {
    throw new Error('Bearer transfer intent has inconsistent raw balance arithmetic');
  }
  const saved = exactObject(journal.plan, ['format', 'lastValidBlockHeight', 'messageBase64'], 'Bearer transfer plan');
  if (saved.format !== PLAN_FORMAT || typeof saved.messageBase64 !== 'string') throw new Error('Bearer transfer plan changed format');
  let message: Uint8Array;
  try { message = Uint8Array.from(atob(saved.messageBase64), (character) => character.charCodeAt(0)); } catch {
    throw new Error('Bearer transfer saved message is not base64');
  }
  if (message.length === 0 || base64(message) !== saved.messageBase64
      || hex(await sha256(message)) !== journal.operationDigest) throw new Error('Bearer transfer saved message digest differs');
  if (journal.phase === 'submitted') {
    const transaction = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (base64(transaction.message.serialize()) !== saved.messageBase64) {
      throw new Error('submitted Bearer transfer packet substituted the saved message');
    }
  }
  return Object.freeze({ poststate, lastValidBlockHeight: u64(saved.lastValidBlockHeight, 'last valid block height').toString() });
}
