import { PublicKey, VersionedTransaction } from '@solana/web3.js';

import { hex, sha256 } from './bytes';
import { decodeBase58 } from './explorer/base58';

/**
 * Crash journal for one browser-authorized protocol operation.
 *
 * This is recovery metadata, never authority. Every caller must reacquire the
 * exact route and verify finalized poststate before it treats an entry as
 * complete. The key includes the chain identity, Market, owner, operation kind
 * and onchain operation digest so a journal cannot drift across any of them.
 */

export const CLIENT_OPERATION_JOURNAL_FORMAT_V1 = 'dclutch-client-operation-journal-v1' as const;
export const CLIENT_OPERATION_JOURNAL_OPERATIONS_V1 = Object.freeze([
  'claims-replay-create-v1',
  'wallet-terminal-payout-v3',
  'direct-inline-v3',
] as const);

export type ClientOperationV1 = (typeof CLIENT_OPERATION_JOURNAL_OPERATIONS_V1)[number];
export type ClientOperationJournalPhaseV1 = 'unsigned' | 'submitted';

export type ClientOperationScopeV1 = Readonly<{
  clusterGenesis: string;
  market: string;
  owner: string;
}>;

export type ClientOperationJournalV1 = ClientOperationScopeV1 & Readonly<{
  format: typeof CLIENT_OPERATION_JOURNAL_FORMAT_V1;
  operation: ClientOperationV1;
  operationDigest: string;
  intentDigest: string;
  planDigest: string;
  intent: string;
  plan: string;
  phase: ClientOperationJournalPhaseV1;
  signature: string | null;
  signedWireBase64: string | null;
}>;

export type ClientOperationJournalStorageV1 = Readonly<{
  length: number;
  key(index: number): string | null;
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}>;

const STORAGE_PREFIX = 'dclutch.client-operation-journal.v1';
const MAX_STORAGE_KEYS = 1_024;
const MAX_INTENT_CHARACTERS = 131_072;
const MAX_PLAN_CHARACTERS = 524_288;
const MAX_RECORD_CHARACTERS = 786_432;
const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
const EXACT_FIELDS = Object.freeze([
  'clusterGenesis', 'format', 'intent', 'intentDigest', 'market', 'operation', 'operationDigest',
  'owner', 'phase', 'plan', 'planDigest', 'signature', 'signedWireBase64',
]);

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  }
  return btoa(binary);
}

function base64Bytes(value: unknown, field: string): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length > 2_000 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not bounded canonical base64`);
  }
  let binary: string;
  try { binary = atob(value); } catch { throw new Error(`${field} is not bounded canonical base64`); }
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (base64(bytes) !== value) throw new Error(`${field} is not bounded canonical base64`);
  return bytes;
}

function exactAddress(value: string, field: string): string {
  let parsed: PublicKey;
  try { parsed = new PublicKey(value); } catch { throw new Error(`${field} is not one canonical Solana address`); }
  if (parsed.toBase58() !== value) throw new Error(`${field} is not canonical base58 text`);
  return value;
}

function exactDigest(value: string, field: string): string {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error(`${field} must be exact lowercase SHA-256 hex`);
  return value;
}

function exactOperation(value: unknown): ClientOperationV1 {
  if (typeof value !== 'string' || !CLIENT_OPERATION_JOURNAL_OPERATIONS_V1.includes(value as ClientOperationV1)) {
    throw new Error('operation journal names an unsupported operation');
  }
  return value as ClientOperationV1;
}

function exactText(value: unknown, field: string, maximum: number): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maximum) {
    throw new Error(`${field} must contain 1..${maximum} characters`);
  }
  return value;
}

function exactScope(scope: ClientOperationScopeV1): ClientOperationScopeV1 {
  return Object.freeze({
    clusterGenesis: exactAddress(scope.clusterGenesis, 'cluster genesis'),
    market: exactAddress(scope.market, 'journal Market'),
    owner: exactAddress(scope.owner, 'journal owner'),
  });
}

function exactKeys(value: Record<string, unknown>): void {
  const observed = Object.keys(value).sort();
  if (observed.length !== EXACT_FIELDS.length || observed.some((field, index) => field !== EXACT_FIELDS[index])) {
    throw new Error('operation journal has missing or unknown fields');
  }
}

function journalKeyUnchecked(journal: Pick<ClientOperationJournalV1, 'operation' | 'clusterGenesis' | 'market' | 'owner' | 'operationDigest'>): string {
  return `${STORAGE_PREFIX}:${journal.operation}:${journal.clusterGenesis}:${journal.market}:${journal.owner}:${journal.operationDigest}`;
}

function scopePrefix(scope: ClientOperationScopeV1, operation: ClientOperationV1): string {
  return `${STORAGE_PREFIX}:${operation}:${scope.clusterGenesis}:${scope.market}:${scope.owner}:`;
}

async function textDigest(text: string): Promise<string> {
  return hex(await sha256(new TextEncoder().encode(text)));
}

function parseJournal(source: string): ClientOperationJournalV1 {
  if (source.length === 0 || source.length > MAX_RECORD_CHARACTERS) throw new Error('operation journal is outside its bounded storage size');
  let raw: unknown;
  try { raw = JSON.parse(source); } catch { throw new Error('operation journal is not JSON'); }
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('operation journal must be one object');
  const value = raw as Record<string, unknown>;
  exactKeys(value);
  if (value.format !== CLIENT_OPERATION_JOURNAL_FORMAT_V1) throw new Error('operation journal has another format');
  const phase = value.phase;
  if (phase !== 'unsigned' && phase !== 'submitted') throw new Error('operation journal has an unsupported phase');
  const signature = value.signature;
  const signedWireBase64 = value.signedWireBase64;
  if ((phase === 'unsigned' && (signature !== null || signedWireBase64 !== null))
      || (phase === 'submitted' && (typeof signature !== 'string' || typeof signedWireBase64 !== 'string'))) {
    throw new Error('operation journal phase, signature, and signed packet disagree');
  }
  if (typeof value.clusterGenesis !== 'string' || typeof value.market !== 'string' || typeof value.owner !== 'string'
      || typeof value.operationDigest !== 'string' || typeof value.intentDigest !== 'string' || typeof value.planDigest !== 'string') {
    throw new Error('operation journal has malformed scope or digest fields');
  }
  const scope = exactScope({ clusterGenesis: value.clusterGenesis, market: value.market, owner: value.owner });
  const journal = Object.freeze({
    format: CLIENT_OPERATION_JOURNAL_FORMAT_V1,
    operation: exactOperation(value.operation),
    ...scope,
    operationDigest: exactDigest(value.operationDigest, 'operation digest'),
    intentDigest: exactDigest(value.intentDigest, 'journal intent digest'),
    planDigest: exactDigest(value.planDigest, 'journal plan digest'),
    intent: exactText(value.intent, 'journal intent', MAX_INTENT_CHARACTERS),
    plan: exactText(value.plan, 'journal plan', MAX_PLAN_CHARACTERS),
    phase,
    signature: signature === null ? null : exactTransactionSignatureV1(signature),
    signedWireBase64: signedWireBase64 === null ? null : signedWireBase64 as string,
  });
  if (journal.signedWireBase64 !== null) submittedClientOperationWireV1(journal);
  return journal;
}

/** Encode the transaction id from the already signed packet before RPC submission. */
export function transactionSignatureV1(signature: Uint8Array): string {
  if (!(signature instanceof Uint8Array) || signature.length !== 64 || signature.every((byte) => byte === 0)) {
    throw new Error('signed transaction does not carry one exact first Ed25519 signature');
  }
  const digits = [0];
  for (const byte of signature) {
    let carry = byte;
    for (let index = 0; index < digits.length; index += 1) {
      carry += digits[index]! * 256;
      digits[index] = carry % 58;
      carry = Math.floor(carry / 58);
    }
    while (carry > 0) { digits.push(carry % 58); carry = Math.floor(carry / 58); }
  }
  let leadingZeroes = 0;
  while (leadingZeroes < signature.length && signature[leadingZeroes] === 0) leadingZeroes += 1;
  const encoded = `${'1'.repeat(leadingZeroes)}${digits.reverse().map((digit) => BASE58_ALPHABET[digit]).join('')}`;
  return exactTransactionSignatureV1(encoded);
}

export function exactTransactionSignatureV1(value: unknown): string {
  if (typeof value !== 'string' || value.length < 64 || value.length > 88 || !/^[1-9A-HJ-NP-Za-km-z]+$/.test(value)) {
    throw new Error('submitted journal signature is not canonical base58 text');
  }
  let bytes: Uint8Array;
  try { bytes = decodeBase58(value); } catch { throw new Error('submitted journal signature is not canonical base58 text'); }
  if (bytes.length !== 64 || transactionSignatureV1Unchecked(bytes) !== value || bytes.every((byte) => byte === 0)) {
    throw new Error('submitted journal signature is not one canonical 64-byte transaction id');
  }
  return value;
}

export function requireSubmittedSignatureMatchV1(expected: string, returned: string): void {
  const exactExpected = exactTransactionSignatureV1(expected);
  const exactReturned = exactTransactionSignatureV1(returned);
  if (exactReturned !== exactExpected) throw new Error(`RPC returned ${exactReturned}, not the exact signed packet id ${exactExpected}`);
}

function transactionSignatureV1Unchecked(signature: Uint8Array): string {
  const digits = [0];
  for (const byte of signature) {
    let carry = byte;
    for (let index = 0; index < digits.length; index += 1) {
      carry += digits[index]! * 256;
      digits[index] = carry % 58;
      carry = Math.floor(carry / 58);
    }
    while (carry > 0) { digits.push(carry % 58); carry = Math.floor(carry / 58); }
  }
  let leadingZeroes = 0;
  while (leadingZeroes < signature.length && signature[leadingZeroes] === 0) leadingZeroes += 1;
  return `${'1'.repeat(leadingZeroes)}${digits.reverse().map((digit) => BASE58_ALPHABET[digit]).join('')}`;
}

export async function findClientOperationJournalV1(
  storage: ClientOperationJournalStorageV1,
  rawScope: ClientOperationScopeV1,
  rawOperation: ClientOperationV1,
): Promise<ClientOperationJournalV1 | null> {
  const scope = exactScope(rawScope);
  const operation = exactOperation(rawOperation);
  if (!Number.isSafeInteger(storage.length) || storage.length < 0 || storage.length > MAX_STORAGE_KEYS) {
    throw new Error(`browser storage holds more than the ${MAX_STORAGE_KEYS}-key recovery bound`);
  }
  const prefix = scopePrefix(scope, operation);
  const matches: Array<Readonly<{ key: string; source: string }>> = [];
  for (let index = 0; index < storage.length; index += 1) {
    const key = storage.key(index);
    if (key === null || !key.startsWith(prefix)) continue;
    const source = storage.getItem(key);
    if (source === null) throw new Error('operation journal disappeared during recovery');
    matches.push(Object.freeze({ key, source }));
  }
  if (matches.length > 1) throw new Error(`more than one ${operation} journal exists for this exact chain, Market, and owner; nothing will be replayed`);
  if (matches.length === 0) return null;
  const journal = parseJournal(matches[0]!.source);
  if (journalKeyUnchecked(journal) !== matches[0]!.key) throw new Error('operation journal storage key disagrees with its authenticated scope');
  if (journal.operation !== operation || journal.clusterGenesis !== scope.clusterGenesis
      || journal.market !== scope.market || journal.owner !== scope.owner) {
    throw new Error('operation journal substituted its operation scope');
  }
  if (await textDigest(journal.intent) !== journal.intentDigest) throw new Error('operation journal intent bytes do not match their stored digest');
  if (await textDigest(journal.plan) !== journal.planDigest) throw new Error('operation journal plan bytes do not match their stored digest');
  return journal;
}

export async function writeUnsignedClientOperationJournalV1(
  storage: ClientOperationJournalStorageV1,
  input: ClientOperationScopeV1 & Readonly<{
    operation: ClientOperationV1;
    operationDigest: string;
    intent: string;
    plan: string;
  }>,
): Promise<ClientOperationJournalV1> {
  const scope = exactScope(input);
  const operation = exactOperation(input.operation);
  const operationDigest = exactDigest(input.operationDigest, 'operation digest');
  const intent = exactText(input.intent, 'journal intent', MAX_INTENT_CHARACTERS);
  const plan = exactText(input.plan, 'journal plan', MAX_PLAN_CHARACTERS);
  const existing = await findClientOperationJournalV1(storage, scope, operation);
  if (existing !== null) {
    if (existing.phase === 'submitted') throw new Error(`a submitted ${operation} journal is still unresolved; it will not be overwritten or replayed`);
    if (existing.operationDigest !== operationDigest || existing.intent !== intent || existing.plan !== plan) {
      throw new Error(`another unsigned ${operation} plan is already retained; discard that unsigned plan explicitly before replacing it`);
    }
    return existing;
  }
  const journal: ClientOperationJournalV1 = Object.freeze({
    format: CLIENT_OPERATION_JOURNAL_FORMAT_V1,
    operation,
    ...scope,
    operationDigest,
    intentDigest: await textDigest(intent),
    planDigest: await textDigest(plan),
    intent,
    plan,
    phase: 'unsigned',
    signature: null,
    signedWireBase64: null,
  });
  storage.setItem(journalKeyUnchecked(journal), JSON.stringify(journal));
  return journal;
}

/** Mark ambiguity before sendTransaction: the first packet signature already names the transaction. */
export async function markClientOperationSubmittedV1(
  storage: ClientOperationJournalStorageV1,
  journal: ClientOperationJournalV1,
  signature: string,
  signedWireBytes: Uint8Array,
): Promise<ClientOperationJournalV1> {
  const current = await findClientOperationJournalV1(storage, journal, journal.operation);
  if (current === null || current.operationDigest !== journal.operationDigest) throw new Error('unsigned operation journal disappeared before submission');
  const exactSignature = exactTransactionSignatureV1(signature);
  const signedWireBase64 = base64(signedWireBytes);
  const candidate = Object.freeze({
    ...current,
    phase: 'submitted' as const,
    signature: exactSignature,
    signedWireBase64,
  });
  submittedClientOperationWireV1(candidate);
  if (current.phase === 'submitted') {
    if (current.signature !== exactSignature || current.signedWireBase64 !== signedWireBase64) {
      throw new Error('submitted operation journal already names another transaction packet');
    }
    return current;
  }
  const submitted = candidate;
  storage.setItem(journalKeyUnchecked(submitted), JSON.stringify(submitted));
  return submitted;
}

/** Recover the exact signed bytes that were persisted before the only send. */
export function submittedClientOperationWireV1(journal: ClientOperationJournalV1): Uint8Array {
  if (journal.phase !== 'submitted' || journal.signature === null || journal.signedWireBase64 === null) {
    throw new Error('operation journal does not contain one persisted signed packet');
  }
  const bytes = base64Bytes(journal.signedWireBase64, 'operation journal signed packet');
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(bytes); } catch {
    throw new Error('operation journal signed packet is not one Solana transaction');
  }
  if (base64(transaction.serialize()) !== journal.signedWireBase64
      || transaction.signatures.length === 0
      || transaction.signatures.some((signature) => signature.every((byte) => byte === 0))) {
    throw new Error('operation journal signed packet is not canonical and completely signed');
  }
  if (transactionSignatureV1(transaction.signatures[0]!) !== journal.signature) {
    throw new Error('operation journal signature differs from its exact signed packet');
  }
  return bytes;
}

export async function discardUnsignedClientOperationJournalV1(
  storage: ClientOperationJournalStorageV1,
  journal: ClientOperationJournalV1,
): Promise<void> {
  const current = await findClientOperationJournalV1(storage, journal, journal.operation);
  if (current === null) return;
  if (current.operationDigest !== journal.operationDigest) throw new Error('operation journal changed before discard');
  if (current.phase !== 'unsigned') throw new Error('a submitted operation is ambiguous and cannot be discarded');
  storage.removeItem(journalKeyUnchecked(current));
}

/** Call only after the route-specific verifier proves finalized exact completion. */
export async function clearFinalizedClientOperationJournalV1(
  storage: ClientOperationJournalStorageV1,
  journal: ClientOperationJournalV1,
): Promise<void> {
  const current = await findClientOperationJournalV1(storage, journal, journal.operation);
  if (current === null) return;
  if (current.operationDigest !== journal.operationDigest) throw new Error('operation journal changed before finalized completion');
  storage.removeItem(journalKeyUnchecked(current));
}
