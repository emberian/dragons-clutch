import { createHash, randomBytes } from 'node:crypto';
import {
  closeSync,
  existsSync,
  fsyncSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { basename, dirname, join } from 'node:path';

import {
  buildWalletTerminalPayoutV3,
  parseWalletTerminalPayoutManifestV3,
  verifyFinalizedWalletTerminalPayoutTransactionV3,
  verifyWalletTerminalPayoutPostconditionV3,
  walletTerminalPayoutSummaryV3,
  type PreparedWalletTerminalPayoutV3,
  type WalletTerminalPayoutManifestV3,
  type WalletTerminalPayoutPoststateV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';
import {
  CLAIMS_CUSTODY_REPLAY_COMPUTE_UNIT_LIMIT_V1,
  encodeClaimsCustodyReplayRequestV1,
  type ClaimsCustodyReplayPlanV1,
} from '@dclutch/sdk/claimsCustodyReplay';
import {
  CALLER_AUTHORITY_PDA_DOMAIN_V1,
  CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1,
  CUSTODY_ABI_VERSION_V1,
  CUSTODY_OPERATION_INITIALIZE_REPLAY_V1,
  CUSTODY_REPLAY_BYTES_V1,
  CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REPLAY_CONTEXT_OFFSET_V1,
  CUSTODY_REPLAY_GENERATION_OFFSET_V1,
  CUSTODY_REPLAY_MAGIC_V1,
  CUSTODY_REPLAY_MARKET_OFFSET_V1,
  CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1,
  CUSTODY_REPLAY_OPEN_VAULT_COUNT_OFFSET_V1,
  CUSTODY_REPLAY_PDA_DOMAIN_V1,
  CUSTODY_REPLAY_REALM_OFFSET_V1,
  CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1,
  CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1,
  CUSTODY_REPLAY_STATUS_OFFSET_V1,
  CUSTODY_REPLAY_VERSION_OFFSET_V1,
  CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REQUEST_CANDIDATE_OFFSET_V1,
  CUSTODY_REQUEST_CONTEXT_OFFSET_V1,
  CUSTODY_REQUEST_DESTINATION_COMPARTMENT_OFFSET_V1,
  CUSTODY_REQUEST_EXPECTED_REVISION_OFFSET_V1,
  CUSTODY_REQUEST_GENERATION_OFFSET_V1,
  CUSTODY_REQUEST_MAGIC_V1,
  CUSTODY_REQUEST_MARKET_OFFSET_V1,
  CUSTODY_REQUEST_AMOUNT_OFFSET_V1,
  CUSTODY_REQUEST_OPERATION_OFFSET_V1,
  CUSTODY_REQUEST_ORDER_NONCE_OFFSET_V1,
  CUSTODY_REQUEST_PAGE_INDEX_OFFSET_V1,
  CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1,
  CUSTODY_REQUEST_PAYER_OFFSET_V1,
  CUSTODY_REQUEST_REALM_OFFSET_V1,
  CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1,
  CUSTODY_REQUEST_RENT_LAMPORTS_OFFSET_V1,
  CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1,
  CUSTODY_REQUEST_RESULTING_REVISION_OFFSET_V1,
  CUSTODY_REQUEST_SOURCE_OFFSET_V1,
  CUSTODY_REQUEST_SOURCE_COMPARTMENT_OFFSET_V1,
  CUSTODY_REQUEST_TRANSFER_INDEX_OFFSET_V1,
  CUSTODY_REQUEST_BYTES_V1,
  CLAIMS_CUSTODY_REPLAY_PARENT_DOMAIN_V1,
  CUSTODY_REQUEST_VERSION_OFFSET_V1,
  EXECUTION_ROLE_CLAIMS_V1,
  REGISTRY_ACTIVATION_PDA_DOMAIN_V1,
  REPLAY_ACCOUNT_ACTIVATION_CACHE_V1,
  REPLAY_ACCOUNT_AGGREGATE_V1,
  REPLAY_ACCOUNT_CLAIMS_PROGRAMDATA_V1,
  REPLAY_ACCOUNT_CLAIMS_PROGRAM_V1,
  REPLAY_ACCOUNT_CORE_MARKET_V1,
  REPLAY_ACCOUNT_CUSTODY_CALLER_AUTHORITY_V1,
  REPLAY_ACCOUNT_CUSTODY_PROGRAM_V1,
  REPLAY_ACCOUNT_CUSTODY_REPLAY_V1,
  REPLAY_ACCOUNT_PAYER_V1,
  REPLAY_ACCOUNT_REALM_STAGING_V1,
  REPLAY_ACCOUNT_REALM_V1,
  REPLAY_ACCOUNT_REGISTRY_PROGRAM_V1,
  REPLAY_ACCOUNT_RENT_SYSVAR_V1,
  REPLAY_ACCOUNT_SYSTEM_PROGRAM_V1,
} from '@dclutch/sdk/generated/claimsCustodyReplayV1';
import { REALM_SCHEMA_RELEASE_ID_V1 } from '@dclutch/sdk/generated/coreFound';
import { deriveClaimsAggregateAddressV2 } from '@dclutch/sdk/marketCoreV2';
import {
  SYSTEM_PROGRAM_ID,
  UPGRADEABLE_LOADER_ID,
  deriveFinalizedRecordAddressesV1,
} from '@dclutch/sdk/releaseRegistry';
import type {
  MultipleAccountObservation,
  RpcAccount,
  TransactionMetaObservation,
} from '@dclutch/sdk/rpc';
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

const JOURNAL_FORMAT = 'dclutch-client-operation-journal-v1' as const;
const JOURNAL_OPERATION = 'wallet-terminal-payout-v3' as const;
const JOURNAL_PLAN_FORMAT = 'dclutch-wallet-terminal-payout-journal-plan-v1' as const;
const REPLAY_JOURNAL_OPERATION = 'claims-custody-replay-create-v1' as const;
const REPLAY_JOURNAL_PLAN_FORMAT = 'dclutch-claims-custody-replay-journal-plan-v1' as const;
const REPLAY_JOURNAL_INTENT_FORMAT = 'dclutch-claims-custody-replay-journal-intent-v1' as const;
const INPUT_FORMAT = 'dclutch-wallet-terminal-payout-plan-input-v1' as const;
const EVIDENCE_FORMAT = 'dclutch-local-successor-run-evidence-v2' as const;
const MAX_JOURNAL_BYTES = 786_432;
const MAX_EVIDENCE_BYTES = 16 * 1024 * 1024;
const MAX_PROJECTED_INPUT_BYTES = 32_768;
const MAX_JSON_DEPTH = 64;
// Consumer pins for `dclutch-custody-contract/src/generated.rs`'s canonical
// CustodyReceiptV1 layout. The Rust contract remains the semantic owner; this
// CLI hostile-decodes the finalized return bytes instead of treating a
// successful transaction status as a receipt.
const CUSTODY_RECEIPT_BYTES_V1 = 384;
const CUSTODY_RECEIPT_MAGIC_V1 = new TextEncoder().encode('DCLCUSC1');
const CUSTODY_RECEIPT_VERSION_OFFSET_V1 = 8;
const CUSTODY_RECEIPT_OPERATION_OFFSET_V1 = 10;
const CUSTODY_RECEIPT_CALLER_ROLE_OFFSET_V1 = 11;
const CUSTODY_RECEIPT_SOURCE_COMPARTMENT_OFFSET_V1 = 12;
const CUSTODY_RECEIPT_DESTINATION_COMPARTMENT_OFFSET_V1 = 13;
const CUSTODY_RECEIPT_TRANSFER_INDEX_OFFSET_V1 = 14;
const CUSTODY_RECEIPT_RELEASE_SET_OFFSET_V1 = 16;
const CUSTODY_RECEIPT_MARKET_OFFSET_V1 = 48;
const CUSTODY_RECEIPT_CONTEXT_OFFSET_V1 = 80;
const CUSTODY_RECEIPT_PARENT_REQUEST_DIGEST_OFFSET_V1 = 112;
const CUSTODY_RECEIPT_REQUEST_DIGEST_OFFSET_V1 = 144;
const CUSTODY_RECEIPT_SOURCE_OFFSET_V1 = 176;
const CUSTODY_RECEIPT_DESTINATION_OFFSET_V1 = 208;
const CUSTODY_RECEIPT_EXPECTED_REVISION_OFFSET_V1 = 240;
const CUSTODY_RECEIPT_RESULTING_REVISION_OFFSET_V1 = 248;
const CUSTODY_RECEIPT_SOURCE_BEFORE_OFFSET_V1 = 256;
const CUSTODY_RECEIPT_SOURCE_AFTER_OFFSET_V1 = 264;
const CUSTODY_RECEIPT_DESTINATION_BEFORE_OFFSET_V1 = 272;
const CUSTODY_RECEIPT_DESTINATION_AFTER_OFFSET_V1 = 280;
const CUSTODY_RECEIPT_AMOUNT_OFFSET_V1 = 288;
const CUSTODY_RECEIPT_RENT_LAMPORTS_OFFSET_V1 = 296;
const CUSTODY_RECEIPT_POSTSTATE_OFFSET_V1 = 304;
const CUSTODY_RECEIPT_REPLAY_DIGEST_OFFSET_V1 = 336;
const CUSTODY_RECEIPT_RESERVED_OFFSET_V1 = 368;
const CUSTODY_REPLAY_LAST_REQUEST_OFFSET_V1 = 224;
const CUSTODY_REPLAY_LAST_POSTSTATE_OFFSET_V1 = 256;
const CUSTODY_POSTSTATE_DOMAIN_V1 = new TextEncoder().encode('dclutch:custody-poststate:v1');
const BASE58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

export type WalletTerminalPayoutPlanInputV1 = Readonly<{
  format: typeof INPUT_FORMAT;
  market: string;
  owner: string;
  recipientOwner: string;
  recipient: string;
  collateralMint: string;
  tokenProgram: string;
  quantity: string;
  claimIndex: number;
  transferIndex: number;
  parentContext: string;
  custodyContext: string;
  releaseSet: string;
  programs: Readonly<{ registry: string; core: string; claims: string; custody: string }>;
  records: Readonly<{
    realm: string;
    product: string;
    resultDomain: string;
    portfolio: string;
    productBasis: string;
    compositionDescriptor: string;
    compositionGraph: string;
    compositionTranslation: string;
    compositionExposure: string;
    terminalRecord: string;
  }>;
}>;

export type PayoutOperationJournalV1 = Readonly<{
  format: typeof JOURNAL_FORMAT;
  operation: typeof JOURNAL_OPERATION;
  clusterGenesis: string;
  market: string;
  owner: string;
  operationDigest: string;
  intentDigest: string;
  planDigest: string;
  intent: string;
  plan: string;
  phase: 'unsigned' | 'submitted';
  signature: string | null;
  signedWireBase64: string | null;
}>;

export type ReplayOperationJournalV1 = Readonly<{
  format: typeof JOURNAL_FORMAT;
  operation: typeof REPLAY_JOURNAL_OPERATION;
  clusterGenesis: string;
  market: string;
  owner: string;
  operationDigest: string;
  intentDigest: string;
  planDigest: string;
  intent: string;
  plan: string;
  phase: 'unsigned' | 'submitted';
  signature: string | null;
  signedWireBase64: string | null;
}>;

export type RestoredReplayOperationV1 = Readonly<{
  market: string;
  owner: string;
  replay: string;
  aggregate: string;
  claimsProgram: string;
  custodyProgram: string;
  registryProgram: string;
  rentLamports: string;
  custodyRequestDigest: string;
  custodyRequestBytes: Uint8Array;
  instructionData: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

type PayoutJournalPlanV1 = Readonly<{
  format: typeof JOURNAL_PLAN_FORMAT;
  observedSlot: string;
  lookupTable: string;
  requiredSigners: ReadonlyArray<string>;
  unsignedWireBase64: string;
  aggregateBase64: string;
  positionBase64: string;
  custodyReplayBase64: string;
  hoardTokenBase64: string;
  recipientTokenBase64: string;
}>;

export type FinalizedPayoutClientV1 = Readonly<{
  transaction(signature: string): Promise<TransactionMetaObservation | null>;
  finalizedSlot(): Promise<string>;
  multipleAccounts(addresses: ReadonlyArray<string>, minimumSlot?: string): Promise<MultipleAccountObservation>;
}>;

type VerifyPoststate = (
  report: PreparedWalletTerminalPayoutV3['report'],
  post: WalletTerminalPayoutPoststateV3,
) => Promise<void>;

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, fields: ReadonlyArray<string>, field: string): void {
  const observed = Object.keys(value).sort(); const expected = [...fields].sort();
  if (observed.length !== expected.length || observed.some((key, index) => key !== expected[index])) {
    throw new Error(`${field} has missing or unknown fields`);
  }
}

/**
 * Parse one bounded JSON value without allowing JSON.parse to erase duplicate
 * object keys first. The scanner consumes the original UTF-8/string boundary,
 * compares decoded key strings at every nesting level, and proves there is no
 * second value or non-whitespace tail before the ordinary value decoder runs.
 */
function exactJson(source: string | Uint8Array, field: string, maximumBytes: number): unknown {
  let text: string;
  let byteLength: number;
  if (typeof source === 'string') {
    text = source;
    byteLength = new TextEncoder().encode(source).byteLength;
  } else {
    byteLength = source.byteLength;
    try { text = new TextDecoder('utf-8', { fatal: true }).decode(source); } catch {
      throw new Error(`${field} is not canonical UTF-8 JSON`);
    }
  }
  if (byteLength === 0 || byteLength > maximumBytes) {
    throw new Error(`${field} is outside its 1..${maximumBytes} byte bound`);
  }

  let cursor = 0;
  const whitespace = () => {
    while (cursor < text.length && (text[cursor] === ' ' || text[cursor] === '\n'
      || text[cursor] === '\r' || text[cursor] === '\t')) cursor += 1;
  };
  const fail = (reason: string): never => { throw new Error(`${field} is not exact JSON: ${reason}`); };
  const string = (): string => {
    if (text[cursor] !== '"') return fail('expected one string');
    const start = cursor++;
    for (;;) {
      if (cursor >= text.length) return fail('unterminated string');
      const character = text[cursor]!;
      if (character === '"') {
        cursor += 1;
        try { return JSON.parse(text.slice(start, cursor)) as string; } catch { return fail('invalid string'); }
      }
      if (character.charCodeAt(0) < 0x20) return fail('unescaped control character');
      if (character === '\\') {
        cursor += 1;
        if (cursor >= text.length) return fail('unterminated escape');
        const escape = text[cursor]!;
        if (escape === 'u') {
          const digits = text.slice(cursor + 1, cursor + 5);
          if (!/^[0-9a-fA-F]{4}$/.test(digits)) return fail('invalid Unicode escape');
          cursor += 5;
          continue;
        }
        if (!'"\\/bfnrt'.includes(escape)) return fail('invalid escape');
      }
      cursor += 1;
    }
  };
  const number = () => {
    if (text[cursor] === '-') cursor += 1;
    if (text[cursor] === '0') cursor += 1;
    else {
      if (!/[1-9]/.test(text[cursor] ?? '')) return fail('invalid number');
      while (/[0-9]/.test(text[cursor] ?? '')) cursor += 1;
    }
    if (text[cursor] === '.') {
      cursor += 1;
      if (!/[0-9]/.test(text[cursor] ?? '')) return fail('invalid fraction');
      while (/[0-9]/.test(text[cursor] ?? '')) cursor += 1;
    }
    if (text[cursor] === 'e' || text[cursor] === 'E') {
      cursor += 1;
      if (text[cursor] === '+' || text[cursor] === '-') cursor += 1;
      if (!/[0-9]/.test(text[cursor] ?? '')) return fail('invalid exponent');
      while (/[0-9]/.test(text[cursor] ?? '')) cursor += 1;
    }
  };
  const value = (depth: number): void => {
    if (depth > MAX_JSON_DEPTH) return fail(`nesting exceeds ${MAX_JSON_DEPTH}`);
    whitespace();
    const character = text[cursor];
    if (character === '{') {
      cursor += 1;
      whitespace();
      const keys = new Set<string>();
      if (text[cursor] === '}') { cursor += 1; return; }
      for (;;) {
        const key = string();
        if (keys.has(key)) return fail(`duplicate JSON object key ${JSON.stringify(key)}`);
        keys.add(key);
        whitespace();
        if (text[cursor] !== ':') return fail('object key has no colon');
        cursor += 1;
        value(depth + 1);
        whitespace();
        if (text[cursor] === '}') { cursor += 1; return; }
        if (text[cursor] !== ',') return fail('object has no comma or closing brace');
        cursor += 1;
        whitespace();
      }
    }
    if (character === '[') {
      cursor += 1;
      whitespace();
      if (text[cursor] === ']') { cursor += 1; return; }
      for (;;) {
        value(depth + 1);
        whitespace();
        if (text[cursor] === ']') { cursor += 1; return; }
        if (text[cursor] !== ',') return fail('array has no comma or closing bracket');
        cursor += 1;
      }
    }
    if (character === '"') { string(); return; }
    for (const literal of ['true', 'false', 'null']) {
      if (text.startsWith(literal, cursor)) { cursor += literal.length; return; }
    }
    if (character === '-' || /[0-9]/.test(character ?? '')) { number(); return; }
    return fail('invalid value');
  };
  value(0);
  whitespace();
  if (cursor !== text.length) fail('trailing data');
  try { return JSON.parse(text); } catch { return fail('value decoder refused'); }
}

function boundedText(value: unknown, field: string, maximumBytes: number): string {
  if (typeof value !== 'string' || value.length === 0 || value.trim() !== value
      || new TextEncoder().encode(value).byteLength > maximumBytes) {
    throw new Error(`${field} is not nonempty canonical text within ${maximumBytes} bytes`);
  }
  return value;
}

function address(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`${field} is not one canonical Solana address`);
  let parsed: PublicKey;
  try { parsed = new PublicKey(value); } catch { throw new Error(`${field} is not one canonical Solana address`); }
  if (parsed.toBase58() !== value) throw new Error(`${field} is not one canonical Solana address`);
  return value;
}

function identity(value: unknown, field: string): string {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value) || /^0{64}$/.test(value)) {
    throw new Error(`${field} is not one nonzero lowercase identity`);
  }
  return value;
}

function decimal(value: unknown, field: string, allowZero = false): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value) || (!allowZero && value === '0')) {
    throw new Error(`${field} is not one canonical decimal integer`);
  }
  const parsed = BigInt(value);
  if (parsed > 0xffff_ffff_ffff_ffffn) throw new Error(`${field} exceeds u64`);
  return value;
}

function index(value: unknown, field: string, maximum: number): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > maximum) {
    throw new Error(`${field} is not one exact bounded integer`);
  }
  return value;
}

function u16(bytes: Uint8Array, offset: number): number {
  if (offset < 0 || offset + 2 > bytes.length) throw new Error('fixed-layout u16 read is outside its input');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 2).getUint16(0, true);
}

function u64(bytes: Uint8Array, offset: number): bigint {
  if (offset < 0 || offset + 8 > bytes.length) throw new Error('fixed-layout u64 read is outside its input');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 8).getBigUint64(0, true);
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  }
  return btoa(binary);
}

function base64Bytes(value: unknown, field: string): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length > 524_288 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not bounded canonical base64`);
  }
  let binary: string;
  try { binary = atob(value); } catch { throw new Error(`${field} is not bounded canonical base64`); }
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (base64(bytes) !== value) throw new Error(`${field} is not bounded canonical base64`);
  return bytes;
}

function sha256(value: string | Uint8Array): string {
  return createHash('sha256').update(value).digest('hex');
}

/** Hostile-check one exact completed campaign dossier before Rust consumes it. */
export function authenticateCompletedCampaignEvidenceV1(planBytes: Uint8Array, evidenceBytes: Uint8Array): void {
  const evidence = object(exactJson(evidenceBytes, 'campaign evidence', MAX_EVIDENCE_BYTES), 'campaign evidence');
  exactKeys(evidence, [
    'schema', 'rpc_url', 'ledger', 'validator_log', 'plan_sha256', 'core_upgrade_authority_pubkey',
    'private_key_persisted', 'keypair_derivation', 'keypair_seed_sha256', 'foundingCustodyContext',
    'directSelectedManifestEntryIndex', 'completed', 'transactions', 'accounts', 'remaining_execution_seam',
  ], 'campaign evidence');
  if (evidence.schema !== EVIDENCE_FORMAT || evidence.private_key_persisted !== false) {
    throw new Error('campaign evidence has another schema, key policy, or bounded text shape');
  }
  boundedText(evidence.rpc_url, 'campaign RPC URL', 2_048);
  boundedText(evidence.ledger, 'campaign ledger path', 4_096);
  boundedText(evidence.validator_log, 'campaign validator-log path', 4_096);
  const derivation = boundedText(evidence.keypair_derivation, 'campaign keypair derivation', 64);
  if (derivation !== 'random-per-run' && derivation !== 'seeded-deterministic') {
    throw new Error('campaign keypair derivation is not one known policy');
  }
  boundedText(evidence.remaining_execution_seam, 'campaign remaining execution seam', 4_096);
  const expectedPlan = identity(evidence.plan_sha256, 'campaign plan digest');
  if (sha256(planBytes) !== expectedPlan) throw new Error('campaign evidence does not authenticate the exact plan bytes');
  address(evidence.core_upgrade_authority_pubkey, 'campaign Core authority');
  identity(evidence.foundingCustodyContext, 'founding Custody context');
  index(evidence.directSelectedManifestEntryIndex, 'Direct selected manifest entry', 65_535);
  if (evidence.keypair_seed_sha256 !== null) identity(evidence.keypair_seed_sha256, 'campaign keypair seed digest');
  if (!Array.isArray(evidence.completed) || evidence.completed.length === 0 || evidence.completed.length > 512
      || evidence.completed.some((entry) => {
        try { boundedText(entry, 'campaign completed stage', 512); return false; } catch { return true; }
      })
      || new Set(evidence.completed).size !== evidence.completed.length) {
    throw new Error('campaign evidence does not carry one nonempty ordered completed-stage list');
  }
  if (!Array.isArray(evidence.transactions) || evidence.transactions.length > 4_096) {
    throw new Error('campaign evidence transactions are not an array within the 4096-row bound');
  }
  const transactionLabels = new Set<string>();
  for (const [offset, raw] of evidence.transactions.entries()) {
    const row = object(raw, `campaign transaction ${offset}`);
    exactKeys(row, [
      'label', 'signature', 'slot', 'transaction_metadata_available', 'fee_lamports',
      'fee_only_balance_change', 'compute_units_consumed', 'error', 'logs',
    ], `campaign transaction ${offset}`);
    let label: string;
    try { label = boundedText(row.label, `campaign transaction ${offset} label`, 512); } catch {
      throw new Error(`campaign transaction ${offset} has another label shape`);
    }
    if (transactionLabels.has(label)) throw new Error(`campaign transaction ${offset} has another label shape`);
    transactionLabels.add(label);
    exactSignature(row.signature);
    if (typeof row.slot !== 'number' || !Number.isSafeInteger(row.slot) || row.slot < 0
        || typeof row.transaction_metadata_available !== 'boolean'
        || (row.fee_lamports !== null && (typeof row.fee_lamports !== 'number'
          || !Number.isSafeInteger(row.fee_lamports) || row.fee_lamports < 0))
        || (row.compute_units_consumed !== null && (typeof row.compute_units_consumed !== 'number'
          || !Number.isSafeInteger(row.compute_units_consumed) || row.compute_units_consumed < 0))
        || (row.fee_only_balance_change !== null && typeof row.fee_only_balance_change !== 'boolean')
        || !Array.isArray(row.logs) || row.logs.length > 512
        || row.logs.some((entry) => typeof entry !== 'string'
          || new TextEncoder().encode(entry).byteLength > 4_096)) {
      throw new Error(`campaign transaction ${offset} has inexact finalized evidence`);
    }
  }
  const accounts = object(evidence.accounts, 'campaign evidence accounts');
  if (Object.keys(accounts).length === 0 || Object.keys(accounts).length > 4_096) {
    throw new Error('campaign evidence persisted accounts are outside the 1..4096 row bound');
  }
  for (const [label, raw] of Object.entries(accounts)) {
    if (label.length === 0 || new TextEncoder().encode(label).byteLength > 128) {
      throw new Error('campaign evidence account label is not bounded');
    }
    const row = object(raw, `campaign account ${label}`);
    exactKeys(row, ['address', 'owner', 'lamports', 'executable', 'data_len', 'data_sha256', 'account_sha256'], `campaign account ${label}`);
    address(row.address, `campaign account ${label} address`);
    address(row.owner, `campaign account ${label} owner`);
    if (typeof row.lamports !== 'number' || !Number.isSafeInteger(row.lamports) || row.lamports < 0
        || typeof row.data_len !== 'number' || !Number.isSafeInteger(row.data_len) || row.data_len < 0
        || typeof row.executable !== 'boolean') throw new Error(`campaign account ${label} has inexact physical facts`);
    identity(row.data_sha256, `campaign account ${label} data digest`);
    identity(row.account_sha256, `campaign account ${label} account digest`);
  }
}

/** Hostile-decode the exact flat input emitted by `wallet-terminal-payout-input`. */
export function parseWalletTerminalPayoutPlanInputV1(source: string): WalletTerminalPayoutPlanInputV1 {
  const value = object(exactJson(source, 'wallet payout projected input', MAX_PROJECTED_INPUT_BYTES), 'wallet payout projected input');
  exactKeys(value, [
    'format', 'market', 'owner', 'recipientOwner', 'recipient', 'collateralMint', 'tokenProgram',
    'quantity', 'claimIndex', 'transferIndex', 'parentContext', 'custodyContext', 'releaseSet',
    'programs', 'records',
  ], 'wallet payout projected input');
  if (value.format !== INPUT_FORMAT) throw new Error('wallet payout projected input has another format');
  const programs = object(value.programs, 'wallet payout programs');
  exactKeys(programs, ['registry', 'core', 'claims', 'custody'], 'wallet payout programs');
  const records = object(value.records, 'wallet payout records');
  const recordFields = [
    'realm', 'product', 'resultDomain', 'portfolio', 'productBasis', 'compositionDescriptor',
    'compositionGraph', 'compositionTranslation', 'compositionExposure', 'terminalRecord',
  ] as const;
  exactKeys(records, recordFields, 'wallet payout records');
  const owner = address(value.owner, 'wallet payout owner');
  const recipientOwner = address(value.recipientOwner, 'wallet payout recipient owner');
  if (recipientOwner !== owner) throw new Error('wallet payout recipient owner differs from its owner');
  const transferIndex = index(value.transferIndex, 'wallet payout transfer index', 65_535);
  if (transferIndex !== 0) throw new Error('wallet payout transfer index must be zero');
  return Object.freeze({
    format: INPUT_FORMAT,
    market: address(value.market, 'wallet payout Market'),
    owner,
    recipientOwner,
    recipient: address(value.recipient, 'wallet payout recipient'),
    collateralMint: address(value.collateralMint, 'wallet payout collateral Mint'),
    tokenProgram: address(value.tokenProgram, 'wallet payout token program'),
    quantity: decimal(value.quantity, 'wallet payout quantity'),
    claimIndex: index(value.claimIndex, 'wallet payout claim index', 0xffff_ffff),
    transferIndex,
    parentContext: identity(value.parentContext, 'wallet payout parent context'),
    custodyContext: identity(value.custodyContext, 'wallet payout Custody context'),
    releaseSet: identity(value.releaseSet, 'wallet payout release set'),
    programs: Object.freeze({
      registry: address(programs.registry, 'Registry program'),
      core: address(programs.core, 'Core program'),
      claims: address(programs.claims, 'Claims program'),
      custody: address(programs.custody, 'Custody program'),
    }),
    records: Object.freeze(Object.fromEntries(recordFields.map((field) => [field, identity(records[field], `wallet payout ${field} record`)]))) as WalletTerminalPayoutPlanInputV1['records'],
  });
}

function canonicalPlan(plan: PreparedWalletTerminalPayoutV3): PayoutJournalPlanV1 {
  return Object.freeze({
    format: JOURNAL_PLAN_FORMAT,
    observedSlot: plan.report.observedSlot,
    lookupTable: plan.lookupTable,
    requiredSigners: plan.requiredSigners,
    unsignedWireBase64: base64(plan.wireBytes),
    aggregateBase64: base64(plan.report.preAggregateBytes),
    positionBase64: base64(plan.report.prePositionBytes),
    custodyReplayBase64: base64(plan.report.preCustodyReplayBytes),
    hoardTokenBase64: base64(plan.report.preHoardTokenBytes),
    recipientTokenBase64: base64(plan.report.preRecipientTokenBytes),
  });
}

function exactJournal(source: string): PayoutOperationJournalV1 {
  const value = object(exactJson(source, 'payout journal', MAX_JOURNAL_BYTES), 'payout journal');
  exactKeys(value, [
    'format', 'operation', 'clusterGenesis', 'market', 'owner', 'operationDigest', 'intentDigest',
    'planDigest', 'intent', 'plan', 'phase', 'signature', 'signedWireBase64',
  ], 'payout journal');
  if (value.format !== JOURNAL_FORMAT || value.operation !== JOURNAL_OPERATION
      || (value.phase !== 'unsigned' && value.phase !== 'submitted')
      || typeof value.intent !== 'string' || typeof value.plan !== 'string') throw new Error('payout journal has another format, operation, phase, or payload shape');
  const signature = value.signature;
  const signedWireBase64 = value.signedWireBase64;
  if ((value.phase === 'unsigned' && (signature !== null || signedWireBase64 !== null))
      || (value.phase === 'submitted' && (typeof signature !== 'string' || typeof signedWireBase64 !== 'string'))) {
    throw new Error('payout journal phase, signature, and signed packet disagree');
  }
  const journal = Object.freeze({
    format: JOURNAL_FORMAT,
    operation: JOURNAL_OPERATION,
    clusterGenesis: address(value.clusterGenesis, 'journal cluster genesis'),
    market: address(value.market, 'journal Market'),
    owner: address(value.owner, 'journal owner'),
    operationDigest: identity(value.operationDigest, 'journal operation digest'),
    intentDigest: identity(value.intentDigest, 'journal intent digest'),
    planDigest: identity(value.planDigest, 'journal plan digest'),
    intent: value.intent,
    plan: value.plan,
    phase: value.phase,
    signature: signature === null ? null : exactSignature(signature),
    signedWireBase64: signedWireBase64 === null ? null : signedWireBase64 as string,
  });
  exactJson(journal.intent, 'payout journal intent', MAX_JOURNAL_BYTES);
  exactJson(journal.plan, 'saved payout verifier plan', MAX_JOURNAL_BYTES);
  if (sha256(journal.intent) !== journal.intentDigest || sha256(journal.plan) !== journal.planDigest) {
    throw new Error('payout journal intent or plan bytes differ from their stored digest');
  }
  if (journal.phase === 'submitted') {
    signedJournalWireV1(
      journal,
      restorePayoutUnsignedWireV1(journal),
      restorePayoutRequiredSignersV1(journal),
    );
  }
  return journal;
}

function atomicWrite(path: string, source: string): void {
  const temporary = join(dirname(path), `.${basename(path)}.${process.pid}.${randomBytes(8).toString('hex')}.tmp`);
  let descriptor: number | null = null;
  try {
    descriptor = openSync(temporary, 'wx', 0o600);
    writeFileSync(descriptor, source, { encoding: 'utf8' });
    fsyncSync(descriptor);
    closeSync(descriptor);
    descriptor = null;
    renameSync(temporary, path);
    const directoryDescriptor = openSync(dirname(path), 'r');
    try { fsyncSync(directoryDescriptor); } finally { closeSync(directoryDescriptor); }
  } finally {
    if (descriptor !== null) closeSync(descriptor);
    rmSync(temporary, { force: true });
  }
}

export function loadPayoutOperationJournalV1(path: string): PayoutOperationJournalV1 | null {
  if (!existsSync(path)) return null;
  return exactJournal(readFileSync(path, 'utf8'));
}

export function writeUnsignedPayoutOperationJournalV1(
  path: string,
  clusterGenesis: string,
  manifest: WalletTerminalPayoutManifestV3,
  plan: PreparedWalletTerminalPayoutV3,
): PayoutOperationJournalV1 {
  if (existsSync(path)) throw new Error('payout journal already exists; it will not be overwritten');
  const intent = JSON.stringify(manifest);
  const encodedPlan = JSON.stringify(canonicalPlan(plan));
  const journal = Object.freeze({
    format: JOURNAL_FORMAT,
    operation: JOURNAL_OPERATION,
    clusterGenesis: address(clusterGenesis, 'journal cluster genesis'),
    market: address(manifest.request.market, 'journal Market'),
    owner: address(manifest.request.owner, 'journal owner'),
    operationDigest: walletTerminalPayoutSummaryV3(plan.report).requestDigest,
    intentDigest: sha256(intent),
    planDigest: sha256(encodedPlan),
    intent,
    plan: encodedPlan,
    phase: 'unsigned' as const,
    signature: null,
    signedWireBase64: null,
  });
  atomicWrite(path, `${JSON.stringify(journal)}\n`);
  return journal;
}

export function markPayoutOperationSubmittedV1(
  path: string,
  journal: PayoutOperationJournalV1,
  signature: string,
  signedWireBytes: Uint8Array,
): PayoutOperationJournalV1 {
  const current = loadPayoutOperationJournalV1(path);
  if (current === null || JSON.stringify(current) !== JSON.stringify(journal)) throw new Error('payout journal changed before submission');
  const exact = exactSignature(signature);
  const signedWireBase64 = base64(authenticateSignedWireV1(
    restorePayoutUnsignedWireV1(current),
    restorePayoutRequiredSignersV1(current),
    exact,
    signedWireBytes,
  ));
  if (current.phase === 'submitted') {
    if (current.signature !== exact || current.signedWireBase64 !== signedWireBase64) {
      throw new Error('submitted payout journal names another transaction packet');
    }
    return current;
  }
  const submitted = Object.freeze({
    ...current,
    phase: 'submitted' as const,
    signature: exact,
    signedWireBase64,
  });
  atomicWrite(path, `${JSON.stringify(submitted)}\n`);
  return submitted;
}

export function archivePayoutOperationJournalV1(path: string, journal: PayoutOperationJournalV1, reason: 'finalized' | 'discarded'): string {
  const current = loadPayoutOperationJournalV1(path);
  if (current === null || JSON.stringify(current) !== JSON.stringify(journal)) throw new Error('payout journal changed before archival');
  if (reason === 'discarded' && current.phase !== 'unsigned') throw new Error('a submitted payout journal is ambiguous and cannot be discarded');
  if (reason === 'finalized' && current.phase !== 'submitted') throw new Error('an unsigned payout journal cannot be finalized');
  const suffix = current.signature ?? current.operationDigest;
  const destination = `${path}.${reason}.${suffix}.json`;
  if (existsSync(destination)) throw new Error(`payout journal archive already exists at ${destination}`);
  renameSync(path, destination);
  const directoryDescriptor = openSync(dirname(path), 'r');
  try { fsyncSync(directoryDescriptor); } finally { closeSync(directoryDescriptor); }
  return destination;
}

export function replayOperationJournalPathV1(payoutJournalPath: string): string {
  if (payoutJournalPath.length === 0) throw new Error('payout journal path is empty');
  return `${payoutJournalPath}.claims-replay.json`;
}

function keyFromBytes(bytes: Uint8Array, offset: number, field: string): string {
  const value = bytes.slice(offset, offset + 32);
  if (value.length !== 32 || value.every((byte) => byte === 0)) throw new Error(`${field} is absent or zero`);
  return new PublicKey(value).toBase58();
}

function replayIntent(
  plan: ClaimsCustodyReplayPlanV1,
  programs: Readonly<{ claims: string; custody: string; registry: string }>,
): string {
  return JSON.stringify(Object.freeze({
    format: REPLAY_JOURNAL_INTENT_FORMAT,
    market: plan.marketAddress,
    owner: plan.payer,
    replay: plan.replayAddress,
    aggregate: plan.aggregateAddress,
    claimsProgram: programs.claims,
    custodyProgram: programs.custody,
    registryProgram: programs.registry,
    rentLamports: plan.rentLamports,
    custodyRequestDigest: plan.custodyRequestDigestHex,
  }));
}

function replayPlan(plan: ClaimsCustodyReplayPlanV1): string {
  return JSON.stringify(Object.freeze({
    format: REPLAY_JOURNAL_PLAN_FORMAT,
    custodyRequestBase64: base64(plan.custodyRequestBytes),
    instructionDataBase64: base64(plan.instructionData),
    unsignedWireBase64: base64(plan.wireBytes),
    requiredSigners: plan.requiredSigners,
  }));
}

function exactReplayJournal(source: string): ReplayOperationJournalV1 {
  const value = object(exactJson(source, 'Claims replay journal', MAX_JOURNAL_BYTES), 'Claims replay journal');
  exactKeys(value, [
    'format', 'operation', 'clusterGenesis', 'market', 'owner', 'operationDigest', 'intentDigest',
    'planDigest', 'intent', 'plan', 'phase', 'signature', 'signedWireBase64',
  ], 'Claims replay journal');
  if (value.format !== JOURNAL_FORMAT || value.operation !== REPLAY_JOURNAL_OPERATION
      || (value.phase !== 'unsigned' && value.phase !== 'submitted')
      || typeof value.intent !== 'string' || typeof value.plan !== 'string') {
    throw new Error('Claims replay journal has another format, operation, phase, or payload shape');
  }
  if ((value.phase === 'unsigned' && (value.signature !== null || value.signedWireBase64 !== null))
      || (value.phase === 'submitted'
        && (typeof value.signature !== 'string' || typeof value.signedWireBase64 !== 'string'))) {
    throw new Error('Claims replay journal phase, signature, and signed packet disagree');
  }
  const journal = Object.freeze({
    format: JOURNAL_FORMAT,
    operation: REPLAY_JOURNAL_OPERATION,
    clusterGenesis: address(value.clusterGenesis, 'Claims replay journal cluster genesis'),
    market: address(value.market, 'Claims replay journal Market'),
    owner: address(value.owner, 'Claims replay journal owner'),
    operationDigest: identity(value.operationDigest, 'Claims replay operation digest'),
    intentDigest: identity(value.intentDigest, 'Claims replay intent digest'),
    planDigest: identity(value.planDigest, 'Claims replay plan digest'),
    intent: value.intent,
    plan: value.plan,
    phase: value.phase,
    signature: value.signature === null ? null : exactSignature(value.signature),
    signedWireBase64: value.signedWireBase64 === null ? null : value.signedWireBase64 as string,
  });
  exactJson(journal.intent, 'Claims replay journal intent', MAX_JOURNAL_BYTES);
  exactJson(journal.plan, 'Claims replay journal plan', MAX_JOURNAL_BYTES);
  if (sha256(journal.intent) !== journal.intentDigest || sha256(journal.plan) !== journal.planDigest) {
    throw new Error('Claims replay journal intent or plan bytes differ from their stored digest');
  }
  if (journal.phase === 'submitted') {
    const restored = restoreReplayOperationJournalV1(journal);
    signedJournalWireV1(journal, restored.wireBytes, restored.requiredSigners);
  }
  return journal;
}

export function loadReplayOperationJournalV1(path: string): ReplayOperationJournalV1 | null {
  if (!existsSync(path)) return null;
  return exactReplayJournal(readFileSync(path, 'utf8'));
}

export function writeUnsignedReplayOperationJournalV1(
  path: string,
  clusterGenesis: string,
  plan: ClaimsCustodyReplayPlanV1,
  programs: Readonly<{ claims: string; custody: string; registry: string }>,
): ReplayOperationJournalV1 {
  if (existsSync(path)) throw new Error('Claims replay journal already exists; it will not be overwritten');
  const claims = address(programs.claims, 'Claims replay Claims program');
  const custody = address(programs.custody, 'Claims replay Custody program');
  const registry = address(programs.registry, 'Claims replay Registry program');
  if (plan.aggregate.registryProgram !== registry
      || keyFromBytes(plan.custodyRequestBytes, CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1, 'Claims replay caller program') !== claims
      || !plan.transaction.message.staticAccountKeys.some((key) => key.toBase58() === custody)) {
    throw new Error('Claims replay plan substitutes its Claims, Custody, or Registry program');
  }
  const expectedInstruction = encodeClaimsCustodyReplayRequestV1(plan.marketAddress);
  if (!same(plan.instructionData, expectedInstruction)) throw new Error('Claims replay plan substitutes its exact instruction request');
  const intent = replayIntent(plan, { claims, custody, registry });
  const encodedPlan = replayPlan(plan);
  const journal = Object.freeze({
    format: JOURNAL_FORMAT,
    operation: REPLAY_JOURNAL_OPERATION,
    clusterGenesis: address(clusterGenesis, 'Claims replay journal cluster genesis'),
    market: address(plan.marketAddress, 'Claims replay journal Market'),
    owner: address(plan.payer, 'Claims replay journal owner'),
    operationDigest: identity(plan.custodyRequestDigestHex, 'Claims replay operation digest'),
    intentDigest: sha256(intent),
    planDigest: sha256(encodedPlan),
    intent,
    plan: encodedPlan,
    phase: 'unsigned' as const,
    signature: null,
    signedWireBase64: null,
  });
  atomicWrite(path, `${JSON.stringify(journal)}\n`);
  return journal;
}

export function markReplayOperationSubmittedV1(
  path: string,
  journal: ReplayOperationJournalV1,
  signature: string,
  signedWireBytes: Uint8Array,
): ReplayOperationJournalV1 {
  const current = loadReplayOperationJournalV1(path);
  if (current === null || JSON.stringify(current) !== JSON.stringify(journal)) {
    throw new Error('Claims replay journal changed before submission');
  }
  const exact = exactSignature(signature);
  const restored = restoreReplayOperationJournalV1(current);
  const signedWireBase64 = base64(authenticateSignedWireV1(
    restored.wireBytes,
    restored.requiredSigners,
    exact,
    signedWireBytes,
  ));
  if (current.phase === 'submitted') {
    if (current.signature !== exact || current.signedWireBase64 !== signedWireBase64) {
      throw new Error('submitted Claims replay journal names another transaction packet');
    }
    return current;
  }
  const submitted = Object.freeze({
    ...current,
    phase: 'submitted' as const,
    signature: exact,
    signedWireBase64,
  });
  atomicWrite(path, `${JSON.stringify(submitted)}\n`);
  return submitted;
}

export function archiveReplayOperationJournalV1(
  path: string,
  journal: ReplayOperationJournalV1,
  reason: 'finalized' | 'discarded',
): string {
  const current = loadReplayOperationJournalV1(path);
  if (current === null || JSON.stringify(current) !== JSON.stringify(journal)) {
    throw new Error('Claims replay journal changed before archival');
  }
  if (reason === 'discarded' && current.phase !== 'unsigned') {
    throw new Error('a submitted Claims replay journal is ambiguous and cannot be discarded');
  }
  if (reason === 'finalized' && current.phase !== 'submitted') {
    throw new Error('an unsigned Claims replay journal cannot be finalized');
  }
  const suffix = current.signature ?? current.operationDigest;
  const destination = `${path}.${reason}.${suffix}.json`;
  if (existsSync(destination)) throw new Error(`Claims replay journal archive already exists at ${destination}`);
  renameSync(path, destination);
  const directoryDescriptor = openSync(dirname(path), 'r');
  try { fsyncSync(directoryDescriptor); } finally { closeSync(directoryDescriptor); }
  return destination;
}

function parseReplayIntent(source: string) {
  const value = object(exactJson(source, 'Claims replay journal intent', MAX_JOURNAL_BYTES), 'Claims replay journal intent');
  exactKeys(value, [
    'format', 'market', 'owner', 'replay', 'aggregate', 'claimsProgram', 'custodyProgram',
    'registryProgram', 'rentLamports', 'custodyRequestDigest',
  ], 'Claims replay journal intent');
  if (value.format !== REPLAY_JOURNAL_INTENT_FORMAT) throw new Error('Claims replay intent has another format');
  return Object.freeze({
    market: address(value.market, 'Claims replay intent Market'),
    owner: address(value.owner, 'Claims replay intent owner'),
    replay: address(value.replay, 'Claims replay intent replay'),
    aggregate: address(value.aggregate, 'Claims replay intent aggregate'),
    claimsProgram: address(value.claimsProgram, 'Claims replay intent Claims program'),
    custodyProgram: address(value.custodyProgram, 'Claims replay intent Custody program'),
    registryProgram: address(value.registryProgram, 'Claims replay intent Registry program'),
    rentLamports: decimal(value.rentLamports, 'Claims replay intent rent'),
    custodyRequestDigest: identity(value.custodyRequestDigest, 'Claims replay intent request digest'),
  });
}

function zero(bytes: Uint8Array): boolean { return bytes.every((byte) => byte === 0); }

function canonicalReplayTransactionV1(
  intent: ReturnType<typeof parseReplayIntent>,
  custodyRequestBytes: Uint8Array,
  instructionData: Uint8Array,
  recentBlockhash: string,
): VersionedTransaction {
  const releaseSet = custodyRequestBytes.slice(
    CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1,
    CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1 + 32,
  );
  const market = custodyRequestBytes.slice(CUSTODY_REQUEST_MARKET_OFFSET_V1, CUSTODY_REQUEST_MARKET_OFFSET_V1 + 32);
  const realm = custodyRequestBytes.slice(CUSTODY_REQUEST_REALM_OFFSET_V1, CUSTODY_REQUEST_REALM_OFFSET_V1 + 32);
  const context = custodyRequestBytes.slice(CUSTODY_REQUEST_CONTEXT_OFFSET_V1, CUSTODY_REQUEST_CONTEXT_OFFSET_V1 + 32);
  const requestDigest = digestBytes(custodyRequestBytes);
  const claimsProgram = new PublicKey(intent.claimsProgram);
  const custodyProgram = new PublicKey(intent.custodyProgram);
  const registryProgram = new PublicKey(intent.registryProgram);
  const owner = new PublicKey(intent.owner);

  const aggregateAddress = deriveClaimsAggregateAddressV2(intent.claimsProgram, intent.market);
  if (aggregateAddress !== intent.aggregate) {
    throw new Error('saved Claims replay intent substitutes the canonical aggregate PDA');
  }
  const [replay] = PublicKey.findProgramAddressSync([
    CUSTODY_REPLAY_PDA_DOMAIN_V1,
    market,
    releaseSet,
    Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1),
    context,
  ], custodyProgram);
  if (replay.toBase58() !== intent.replay) {
    throw new Error('saved Claims replay intent substitutes the canonical replay PDA');
  }
  const [callerAuthority] = PublicKey.findProgramAddressSync([
    CALLER_AUTHORITY_PDA_DOMAIN_V1,
    releaseSet,
    market,
    Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1),
    context,
    requestDigest,
  ], claimsProgram);
  const [activationCache] = PublicKey.findProgramAddressSync([
    REGISTRY_ACTIVATION_PDA_DOMAIN_V1,
    releaseSet,
  ], registryProgram);
  const [claimsProgramData] = PublicKey.findProgramAddressSync([
    claimsProgram.toBytes(),
  ], new PublicKey(UPGRADEABLE_LOADER_ID));
  const realmRecord = deriveFinalizedRecordAddressesV1(
    intent.registryProgram,
    REALM_SCHEMA_RELEASE_ID_V1,
    realm,
  );

  const keys = new Array<{ pubkey: PublicKey; isSigner: boolean; isWritable: boolean }>(
    CLAIMS_CUSTODY_REPLAY_ACCOUNT_COUNT_V1,
  );
  keys[REPLAY_ACCOUNT_CUSTODY_CALLER_AUTHORITY_V1] = { pubkey: callerAuthority, isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_CORE_MARKET_V1] = { pubkey: new PublicKey(market), isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_ACTIVATION_CACHE_V1] = { pubkey: activationCache, isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_REGISTRY_PROGRAM_V1] = { pubkey: registryProgram, isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_CLAIMS_PROGRAM_V1] = { pubkey: claimsProgram, isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_CLAIMS_PROGRAMDATA_V1] = { pubkey: claimsProgramData, isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_REALM_V1] = { pubkey: new PublicKey(realmRecord.record), isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_REALM_STAGING_V1] = { pubkey: new PublicKey(realmRecord.staging), isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_CUSTODY_REPLAY_V1] = { pubkey: replay, isSigner: false, isWritable: true };
  keys[REPLAY_ACCOUNT_PAYER_V1] = { pubkey: owner, isSigner: true, isWritable: true };
  keys[REPLAY_ACCOUNT_SYSTEM_PROGRAM_V1] = { pubkey: new PublicKey(SYSTEM_PROGRAM_ID), isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_RENT_SYSVAR_V1] = { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_CUSTODY_PROGRAM_V1] = { pubkey: custodyProgram, isSigner: false, isWritable: false };
  keys[REPLAY_ACCOUNT_AGGREGATE_V1] = { pubkey: new PublicKey(aggregateAddress), isSigner: false, isWritable: false };

  const instruction = new TransactionInstruction({
    programId: claimsProgram,
    keys,
    data: Buffer.from(instructionData),
  });
  const budget = ComputeBudgetProgram.setComputeUnitLimit({ units: CLAIMS_CUSTODY_REPLAY_COMPUTE_UNIT_LIMIT_V1 });
  return new VersionedTransaction(new TransactionMessage({
    payerKey: owner,
    recentBlockhash,
    instructions: [budget, instruction],
  }).compileToLegacyMessage());
}

export function restoreReplayOperationJournalV1(journal: ReplayOperationJournalV1): RestoredReplayOperationV1 {
  if (journal.operation !== REPLAY_JOURNAL_OPERATION) throw new Error('saved operation is not one Claims replay creation');
  const intent = parseReplayIntent(journal.intent);
  if (intent.market !== journal.market || intent.owner !== journal.owner
      || intent.custodyRequestDigest !== journal.operationDigest) {
    throw new Error('saved Claims replay intent substitutes its scope or operation digest');
  }
  const value = object(exactJson(journal.plan, 'Claims replay journal plan', MAX_JOURNAL_BYTES), 'Claims replay journal plan');
  exactKeys(value, [
    'format', 'custodyRequestBase64', 'instructionDataBase64', 'unsignedWireBase64', 'requiredSigners',
  ], 'Claims replay journal plan');
  if (value.format !== REPLAY_JOURNAL_PLAN_FORMAT || !Array.isArray(value.requiredSigners)
      || value.requiredSigners.some((signer) => typeof signer !== 'string')) {
    throw new Error('Claims replay saved plan has another format or signer shape');
  }
  const custodyRequestBytes = base64Bytes(value.custodyRequestBase64, 'saved Claims replay Custody request');
  const instructionData = base64Bytes(value.instructionDataBase64, 'saved Claims replay instruction');
  const wireBytes = base64Bytes(value.unsignedWireBase64, 'saved unsigned Claims replay transaction');
  if (custodyRequestBytes.length !== CUSTODY_REQUEST_BYTES_V1
      || sha256(custodyRequestBytes) !== intent.custodyRequestDigest
      || !same(instructionData, encodeClaimsCustodyReplayRequestV1(intent.market))) {
    throw new Error('saved Claims replay request bytes substitute their width, digest, or Market');
  }
  if (!same(custodyRequestBytes.slice(0, CUSTODY_REQUEST_MAGIC_V1.length), CUSTODY_REQUEST_MAGIC_V1)
      || u16(custodyRequestBytes, CUSTODY_REQUEST_VERSION_OFFSET_V1) !== CUSTODY_ABI_VERSION_V1
      || custodyRequestBytes[CUSTODY_REQUEST_OPERATION_OFFSET_V1] !== CUSTODY_OPERATION_INITIALIZE_REPLAY_V1
      || custodyRequestBytes[CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1] !== EXECUTION_ROLE_CLAIMS_V1
      || custodyRequestBytes[CUSTODY_REQUEST_SOURCE_COMPARTMENT_OFFSET_V1] !== 0
      || custodyRequestBytes[CUSTODY_REQUEST_DESTINATION_COMPARTMENT_OFFSET_V1] !== 0
      || u16(custodyRequestBytes, CUSTODY_REQUEST_TRANSFER_INDEX_OFFSET_V1) !== 0
      || keyFromBytes(custodyRequestBytes, CUSTODY_REQUEST_MARKET_OFFSET_V1, 'saved Claims replay request Market') !== intent.market
      || keyFromBytes(custodyRequestBytes, CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1, 'saved Claims replay caller program') !== intent.claimsProgram
      || keyFromBytes(custodyRequestBytes, CUSTODY_REQUEST_PAYER_OFFSET_V1, 'saved Claims replay payer') !== intent.owner
      || keyFromBytes(custodyRequestBytes, CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1, 'saved Claims replay rent refund') !== intent.owner
      || !zero(custodyRequestBytes.slice(CUSTODY_REQUEST_CANDIDATE_OFFSET_V1, CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1))
      || !zero(custodyRequestBytes.slice(CUSTODY_REQUEST_SOURCE_OFFSET_V1, CUSTODY_REQUEST_PAYER_OFFSET_V1))
      || u64(custodyRequestBytes, CUSTODY_REQUEST_EXPECTED_REVISION_OFFSET_V1) !== 0n
      || u64(custodyRequestBytes, CUSTODY_REQUEST_RESULTING_REVISION_OFFSET_V1) !== 1n
      || u64(custodyRequestBytes, CUSTODY_REQUEST_ORDER_NONCE_OFFSET_V1) !== 0n
      || u64(custodyRequestBytes, CUSTODY_REQUEST_AMOUNT_OFFSET_V1) !== 0n
      || zero(custodyRequestBytes.slice(CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1, CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1 + 32))
      || zero(custodyRequestBytes.slice(CUSTODY_REQUEST_REALM_OFFSET_V1, CUSTODY_REQUEST_REALM_OFFSET_V1 + 32))
      || zero(custodyRequestBytes.slice(CUSTODY_REQUEST_CONTEXT_OFFSET_V1, CUSTODY_REQUEST_CONTEXT_OFFSET_V1 + 32))
      || u64(custodyRequestBytes, CUSTODY_REQUEST_GENERATION_OFFSET_V1) === 0n
      || u64(custodyRequestBytes, CUSTODY_REQUEST_RENT_LAMPORTS_OFFSET_V1).toString() !== intent.rentLamports
      || !zero(custodyRequestBytes.slice(CUSTODY_REQUEST_PAGE_INDEX_OFFSET_V1, CUSTODY_REQUEST_BYTES_V1))) {
    throw new Error('saved Claims replay Custody request substitutes its exact InitializeReplay coordinates');
  }
  const expectedParent = digestBytes(
    CLAIMS_CUSTODY_REPLAY_PARENT_DOMAIN_V1,
    custodyRequestBytes.slice(CUSTODY_REQUEST_MARKET_OFFSET_V1, CUSTODY_REQUEST_MARKET_OFFSET_V1 + 32),
    custodyRequestBytes.slice(CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1, CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1 + 32),
    custodyRequestBytes.slice(CUSTODY_REQUEST_CONTEXT_OFFSET_V1, CUSTODY_REQUEST_CONTEXT_OFFSET_V1 + 32),
    new PublicKey(intent.owner).toBytes(),
    le64(BigInt(intent.rentLamports)),
  );
  if (!same(
    custodyRequestBytes.slice(CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1, CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1 + 32),
    expectedParent,
  )) throw new Error('saved Claims replay Custody request substitutes its exact parent digest');
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wireBytes); } catch {
    throw new Error('saved Claims replay transaction is not one Solana transaction');
  }
  if (!same(transaction.serialize(), wireBytes) || transaction.message.addressTableLookups.length !== 0
      || transaction.signatures.length !== 1 || !zero(transaction.signatures[0] ?? new Uint8Array([1]))) {
    throw new Error('saved Claims replay transaction is not one canonical unsigned legacy packet');
  }
  const requiredSigners = transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures).map((signer) => signer.toBase58());
  const savedSigners = (value.requiredSigners as unknown[]).map((signer) => address(signer, 'saved Claims replay signer'));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== intent.owner
      || savedSigners.length !== requiredSigners.length
      || requiredSigners.some((signer, index) => signer !== savedSigners[index])) {
    throw new Error('saved Claims replay transaction substitutes its signer set');
  }
  const canonical = canonicalReplayTransactionV1(
    intent,
    custodyRequestBytes,
    instructionData,
    transaction.message.recentBlockhash,
  );
  if (!same(canonical.serialize(), wireBytes)) {
    throw new Error('saved Claims replay transaction is not the byte-identical complete canonical legacy message');
  }
  return Object.freeze({ ...intent, custodyRequestBytes, instructionData, transaction, wireBytes, requiredSigners: Object.freeze(requiredSigners) });
}

function parseJournalPlan(source: string): PayoutJournalPlanV1 {
  const value = object(exactJson(source, 'saved payout verifier plan', MAX_JOURNAL_BYTES), 'saved payout verifier plan');
  exactKeys(value, [
    'format', 'observedSlot', 'lookupTable', 'requiredSigners', 'unsignedWireBase64',
    'aggregateBase64', 'positionBase64', 'custodyReplayBase64', 'hoardTokenBase64', 'recipientTokenBase64',
  ], 'saved payout verifier plan');
  if (value.format !== JOURNAL_PLAN_FORMAT || typeof value.observedSlot !== 'string'
      || !Array.isArray(value.requiredSigners) || value.requiredSigners.some((signer) => typeof signer !== 'string')) {
    throw new Error('saved payout verifier plan has another format, slot, or signer shape');
  }
  return Object.freeze({
    format: JOURNAL_PLAN_FORMAT,
    observedSlot: decimal(value.observedSlot, 'saved payout observation slot', true),
    lookupTable: address(value.lookupTable, 'saved payout lookup table'),
    requiredSigners: Object.freeze(value.requiredSigners.map((signer) => address(signer, 'saved payout signer'))),
    unsignedWireBase64: typeof value.unsignedWireBase64 === 'string' ? value.unsignedWireBase64 : '',
    aggregateBase64: typeof value.aggregateBase64 === 'string' ? value.aggregateBase64 : '',
    positionBase64: typeof value.positionBase64 === 'string' ? value.positionBase64 : '',
    custodyReplayBase64: typeof value.custodyReplayBase64 === 'string' ? value.custodyReplayBase64 : '',
    hoardTokenBase64: typeof value.hoardTokenBase64 === 'string' ? value.hoardTokenBase64 : '',
    recipientTokenBase64: typeof value.recipientTokenBase64 === 'string' ? value.recipientTokenBase64 : '',
  });
}

function restorePayoutUnsignedWireV1(journal: PayoutOperationJournalV1): Uint8Array {
  return base64Bytes(parseJournalPlan(journal.plan).unsignedWireBase64, 'saved unsigned payout transaction');
}

function restorePayoutRequiredSignersV1(journal: PayoutOperationJournalV1): ReadonlyArray<string> {
  return parseJournalPlan(journal.plan).requiredSigners;
}

export async function restorePayoutOperationJournalV1(journal: PayoutOperationJournalV1): Promise<Readonly<{
  manifest: WalletTerminalPayoutManifestV3;
  plan: PreparedWalletTerminalPayoutV3;
}>> {
  const manifest = parseWalletTerminalPayoutManifestV3(journal.intent);
  if (manifest.request.market !== journal.market || manifest.request.owner !== journal.owner) {
    throw new Error('saved payout intent substitutes the Market or owner');
  }
  const saved = parseJournalPlan(journal.plan);
  if (saved.lookupTable !== manifest.lookupTable) throw new Error('saved payout plan substitutes its lookup table');
  const wireBytes = base64Bytes(saved.unsignedWireBase64, 'saved unsigned payout transaction');
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wireBytes); } catch { throw new Error('saved payout transaction is not one Solana transaction'); }
  if (base64(transaction.serialize()) !== saved.unsignedWireBase64) throw new Error('saved payout transaction is not canonical wire bytes');
  const requiredSigners = transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures).map((signer) => signer.toBase58());
  if (requiredSigners.length !== saved.requiredSigners.length
      || requiredSigners.some((signer, offset) => signer !== saved.requiredSigners[offset])) {
    throw new Error('saved payout transaction substitutes its signer set');
  }
  const report = await buildWalletTerminalPayoutV3({
    observedSlot: saved.observedSlot,
    route: manifest.route,
    custodyContext: manifest.custodyContext,
    request: manifest.request,
    signedPacket: base64Bytes(manifest.signedPacketBase64, 'saved SignedDelta packet'),
    payout: manifest.payout,
    aggregateBytes: base64Bytes(saved.aggregateBase64, 'saved Claims aggregate'),
    positionBytes: base64Bytes(saved.positionBase64, 'saved Claims Position'),
    custodyReplayBytes: base64Bytes(saved.custodyReplayBase64, 'saved Custody replay'),
    hoardTokenBytes: base64Bytes(saved.hoardTokenBase64, 'saved Hoard token account'),
    recipientTokenBytes: base64Bytes(saved.recipientTokenBase64, 'saved recipient token account'),
  });
  if (walletTerminalPayoutSummaryV3(report).requestDigest !== journal.operationDigest) {
    throw new Error('saved payout plan substitutes the operation digest');
  }
  return Object.freeze({
    manifest,
    plan: Object.freeze({ transaction, wireBytes, requiredSigners: Object.freeze(requiredSigners), report, lookupTable: saved.lookupTable }),
  });
}

export function transactionSignatureV1(signature: Uint8Array): string {
  if (signature.length !== 64 || signature.every((byte) => byte === 0)) throw new Error('signed payout has no exact first signature');
  let numeric = 0n;
  for (const byte of signature) numeric = (numeric << 8n) + BigInt(byte);
  let text = '';
  while (numeric > 0n) { text = BASE58[Number(numeric % 58n)] + text; numeric /= 58n; }
  let zeroes = 0; while (zeroes < signature.length && signature[zeroes] === 0) zeroes += 1;
  return exactSignature(`${'1'.repeat(zeroes)}${text}`);
}

function exactSignature(value: unknown): string {
  if (typeof value !== 'string' || value.length < 64 || value.length > 88
      || [...value].some((character) => !BASE58.includes(character))) throw new Error('payout signature is not canonical base58');
  let numeric = 0n;
  for (const character of value) numeric = numeric * 58n + BigInt(BASE58.indexOf(character));
  const significant: number[] = [];
  while (numeric > 0n) { significant.push(Number(numeric & 0xffn)); numeric >>= 8n; }
  significant.reverse();
  let zeroes = 0; while (zeroes < value.length && value[zeroes] === '1') zeroes += 1;
  if (zeroes + significant.length !== 64) throw new Error('payout signature is not 64 bytes');
  return value;
}

export function signPayoutPlanV1(plan: PreparedWalletTerminalPayoutV3, signer: Keypair): Readonly<{
  signature: string;
  wireBytes: Uint8Array;
}> {
  if (plan.requiredSigners.length !== 1 || plan.requiredSigners[0] !== signer.publicKey.toBase58()) {
    throw new Error('payout transaction requires another signer set');
  }
  const transaction = VersionedTransaction.deserialize(plan.wireBytes);
  transaction.sign([signer]);
  const signature = transactionSignatureV1(transaction.signatures[0] ?? new Uint8Array());
  return Object.freeze({ signature, wireBytes: transaction.serialize() });
}

export function signReplayOperationV1(plan: RestoredReplayOperationV1, signer: Keypair): Readonly<{
  signature: string;
  wireBytes: Uint8Array;
}> {
  if (plan.requiredSigners.length !== 1 || plan.requiredSigners[0] !== signer.publicKey.toBase58()) {
    throw new Error('Claims replay transaction requires another signer set');
  }
  const transaction = VersionedTransaction.deserialize(plan.wireBytes);
  transaction.sign([signer]);
  const signature = transactionSignatureV1(transaction.signatures[0] ?? new Uint8Array());
  return Object.freeze({ signature, wireBytes: transaction.serialize() });
}

function authenticateSignedWireV1(
  unsignedWire: Uint8Array,
  requiredSigners: ReadonlyArray<string>,
  signature: string,
  signedWire: Uint8Array,
): Uint8Array {
  if (!(signedWire instanceof Uint8Array) || signedWire.length === 0) {
    throw new Error('signed transaction packet is absent');
  }
  let unsigned: VersionedTransaction;
  let signed: VersionedTransaction;
  try {
    unsigned = VersionedTransaction.deserialize(unsignedWire);
    signed = VersionedTransaction.deserialize(signedWire);
  } catch {
    throw new Error('saved signed transaction is not one Solana packet');
  }
  if (!same(signed.serialize(), signedWire)
      || !same(unsigned.message.serialize(), signed.message.serialize())
      || signed.signatures.length !== 1 || requiredSigners.length !== 1
      || requiredSigners[0] !== signed.message.staticAccountKeys[0]?.toBase58()
      || signed.signatures[0]?.every((byte) => byte === 0)
      || transactionSignatureV1(signed.signatures[0] ?? new Uint8Array()) !== signature) {
    throw new Error('saved signed transaction substitutes its exact message, signer, or signature');
  }
  return new Uint8Array(signedWire);
}

function signedJournalWireV1(
  journal: PayoutOperationJournalV1 | ReplayOperationJournalV1,
  unsignedWire: Uint8Array,
  requiredSigners: ReadonlyArray<string>,
): Uint8Array {
  if (journal.phase !== 'submitted' || journal.signature === null || journal.signedWireBase64 === null) {
    throw new Error('submitted journal does not contain one exact signed packet');
  }
  return authenticateSignedWireV1(
    unsignedWire,
    requiredSigners,
    journal.signature,
    base64Bytes(journal.signedWireBase64, 'saved signed transaction'),
  );
}

/** Read back the exact payout packet durably saved before the only send. */
export function submittedPayoutWireBytesV1(
  journal: PayoutOperationJournalV1,
  plan: PreparedWalletTerminalPayoutV3,
): Uint8Array {
  return signedJournalWireV1(journal, plan.wireBytes, plan.requiredSigners);
}

/** Read back the exact Claims replay packet durably saved before the only send. */
export function submittedReplayWireBytesV1(
  journal: ReplayOperationJournalV1,
  plan: RestoredReplayOperationV1,
): Uint8Array {
  return signedJournalWireV1(journal, plan.wireBytes, plan.requiredSigners);
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

function material(account: RpcAccount | null, owner: string, field: string): RpcAccount {
  if (account === null || account.owner !== owner || account.executable || account.space !== account.data.length
      || !/^(0|[1-9][0-9]*)$/.test(account.lamports) || account.lamports === '0') {
    throw new Error(`${field} is absent or has another owner, executable bit, space, or lamport shape`);
  }
  return account;
}

function digestBytes(...parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const hash = createHash('sha256');
  for (const part of parts) hash.update(part);
  return new Uint8Array(hash.digest());
}

function le64(value: bigint): Uint8Array {
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, value, true);
  return bytes;
}

function verifyReplayCreationBalances(
  meta: TransactionMetaObservation,
  owner: string,
  replay: string,
  rentLamports: bigint,
): void {
  if (meta.accountAddresses.length !== meta.preBalances.length || meta.preBalances.length !== meta.postBalances.length) {
    throw new Error('finalized Claims replay balance vectors do not cover its exact account list');
  }
  const ownerIndex = meta.accountAddresses.indexOf(owner);
  const replayIndex = meta.accountAddresses.indexOf(replay);
  if (ownerIndex < 0 || replayIndex < 0 || ownerIndex === replayIndex
      || meta.accountAddresses.lastIndexOf(owner) !== ownerIndex
      || meta.accountAddresses.lastIndexOf(replay) !== replayIndex) {
    throw new Error('finalized Claims replay does not name one exact payer and replay');
  }
  const fee = BigInt(meta.feeLamports);
  for (let index = 0; index < meta.preBalances.length; index += 1) {
    const before = BigInt(meta.preBalances[index]!);
    const after = BigInt(meta.postBalances[index]!);
    const valid = index === ownerIndex
      ? after + fee + rentLamports === before
      : index === replayIndex ? before === 0n && after === rentLamports : before === after;
    if (!valid) throw new Error('finalized Claims replay balances differ from the exact fee-plus-rent creation');
  }
}

function verifyReplayReceiptAndPoststate(
  plan: RestoredReplayOperationV1,
  receipt: Uint8Array,
  replay: RpcAccount,
): void {
  if (receipt.length !== CUSTODY_RECEIPT_BYTES_V1
      || !same(receipt.slice(0, CUSTODY_RECEIPT_MAGIC_V1.length), CUSTODY_RECEIPT_MAGIC_V1)
      || u16(receipt, CUSTODY_RECEIPT_VERSION_OFFSET_V1) !== CUSTODY_ABI_VERSION_V1
      || receipt[CUSTODY_RECEIPT_OPERATION_OFFSET_V1] !== CUSTODY_OPERATION_INITIALIZE_REPLAY_V1
      || receipt[CUSTODY_RECEIPT_CALLER_ROLE_OFFSET_V1] !== EXECUTION_ROLE_CLAIMS_V1
      || receipt[CUSTODY_RECEIPT_SOURCE_COMPARTMENT_OFFSET_V1] !== 0
      || receipt[CUSTODY_RECEIPT_DESTINATION_COMPARTMENT_OFFSET_V1] !== 0
      || u16(receipt, CUSTODY_RECEIPT_TRANSFER_INDEX_OFFSET_V1) !== 0
      || !zero(receipt.slice(CUSTODY_RECEIPT_SOURCE_OFFSET_V1, CUSTODY_RECEIPT_DESTINATION_OFFSET_V1 + 32))
      || u64(receipt, CUSTODY_RECEIPT_EXPECTED_REVISION_OFFSET_V1) !== 0n
      || u64(receipt, CUSTODY_RECEIPT_RESULTING_REVISION_OFFSET_V1) !== 1n
      || u64(receipt, CUSTODY_RECEIPT_SOURCE_BEFORE_OFFSET_V1) !== 0n
      || u64(receipt, CUSTODY_RECEIPT_SOURCE_AFTER_OFFSET_V1) !== 0n
      || u64(receipt, CUSTODY_RECEIPT_DESTINATION_BEFORE_OFFSET_V1) !== 0n
      || u64(receipt, CUSTODY_RECEIPT_DESTINATION_AFTER_OFFSET_V1) !== 0n
      || u64(receipt, CUSTODY_RECEIPT_AMOUNT_OFFSET_V1) !== 0n
      || u64(receipt, CUSTODY_RECEIPT_RENT_LAMPORTS_OFFSET_V1).toString() !== plan.rentLamports
      || !zero(receipt.slice(CUSTODY_RECEIPT_RESERVED_OFFSET_V1))) {
    throw new Error('finalized Claims replay Custody receipt has another canonical InitializeReplay shape');
  }
  const request = plan.custodyRequestBytes;
  const requestDigest = digestBytes(request);
  const expectedPoststate = digestBytes(
    CUSTODY_POSTSTATE_DOMAIN_V1,
    requestDigest,
    new PublicKey(plan.replay).toBytes(),
    new PublicKey(plan.replay).toBytes(),
    le64(0n), le64(0n), le64(0n), le64(0n), le64(BigInt(plan.rentLamports)),
  );
  for (const [receiptOffset, requestOffset, field] of [
    [CUSTODY_RECEIPT_RELEASE_SET_OFFSET_V1, CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1, 'release set'],
    [CUSTODY_RECEIPT_MARKET_OFFSET_V1, CUSTODY_REQUEST_MARKET_OFFSET_V1, 'Market'],
    [CUSTODY_RECEIPT_CONTEXT_OFFSET_V1, CUSTODY_REQUEST_CONTEXT_OFFSET_V1, 'context'],
    [CUSTODY_RECEIPT_PARENT_REQUEST_DIGEST_OFFSET_V1, CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1, 'parent request'],
  ] as const) {
    if (!same(receipt.slice(receiptOffset, receiptOffset + 32), request.slice(requestOffset, requestOffset + 32))) {
      throw new Error(`finalized Claims replay receipt substitutes its ${field}`);
    }
  }
  if (!same(receipt.slice(CUSTODY_RECEIPT_REQUEST_DIGEST_OFFSET_V1, CUSTODY_RECEIPT_REQUEST_DIGEST_OFFSET_V1 + 32), requestDigest)
      || !same(receipt.slice(CUSTODY_RECEIPT_POSTSTATE_OFFSET_V1, CUSTODY_RECEIPT_POSTSTATE_OFFSET_V1 + 32), expectedPoststate)) {
    throw new Error('finalized Claims replay receipt substitutes its request or poststate commitment');
  }
  if (replay.owner !== plan.custodyProgram || replay.executable || replay.space !== CUSTODY_REPLAY_BYTES_V1
      || replay.data.length !== CUSTODY_REPLAY_BYTES_V1 || replay.lamports !== plan.rentLamports
      || !same(replay.data.slice(0, CUSTODY_REPLAY_MAGIC_V1.length), CUSTODY_REPLAY_MAGIC_V1)
      || u16(replay.data, CUSTODY_REPLAY_VERSION_OFFSET_V1) !== CUSTODY_ABI_VERSION_V1
      || replay.data[CUSTODY_REPLAY_STATUS_OFFSET_V1] !== 1
      || replay.data[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] !== EXECUTION_ROLE_CLAIMS_V1
      || u64(replay.data, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1) !== 1n
      || new DataView(replay.data.buffer, replay.data.byteOffset + CUSTODY_REPLAY_OPEN_VAULT_COUNT_OFFSET_V1, 4).getUint32(0, true) !== 0
      || u64(replay.data, CUSTODY_REPLAY_GENERATION_OFFSET_V1) !== u64(request, CUSTODY_REQUEST_GENERATION_OFFSET_V1)) {
    throw new Error('finalized Claims replay account has another owner, physical shape, or cursor state');
  }
  for (const [replayOffset, requestOffset, field] of [
    [CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1, CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1, 'release set'],
    [CUSTODY_REPLAY_MARKET_OFFSET_V1, CUSTODY_REQUEST_MARKET_OFFSET_V1, 'Market'],
    [CUSTODY_REPLAY_REALM_OFFSET_V1, CUSTODY_REQUEST_REALM_OFFSET_V1, 'Realm'],
    [CUSTODY_REPLAY_CONTEXT_OFFSET_V1, CUSTODY_REQUEST_CONTEXT_OFFSET_V1, 'context'],
    [CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1, CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1, 'Claims program'],
    [CUSTODY_REPLAY_RENT_REFUND_OFFSET_V1, CUSTODY_REQUEST_RENT_REFUND_OFFSET_V1, 'rent refund'],
  ] as const) {
    if (!same(replay.data.slice(replayOffset, replayOffset + 32), request.slice(requestOffset, requestOffset + 32))) {
      throw new Error(`finalized Claims replay account substitutes its ${field}`);
    }
  }
  const replayDigest = digestBytes(replay.data);
  if (!same(replay.data.slice(CUSTODY_REPLAY_LAST_REQUEST_OFFSET_V1, CUSTODY_REPLAY_LAST_REQUEST_OFFSET_V1 + 32), requestDigest)
      || !same(replay.data.slice(CUSTODY_REPLAY_LAST_POSTSTATE_OFFSET_V1), expectedPoststate)
      || !same(receipt.slice(CUSTODY_RECEIPT_REPLAY_DIGEST_OFFSET_V1, CUSTODY_RECEIPT_REPLAY_DIGEST_OFFSET_V1 + 32), replayDigest)) {
    throw new Error('finalized Claims replay receipt and account disagree on their exact digests');
  }
}

/** Authenticate exact finalized Claims outer execution and Custody receipt/poststate. */
export async function finalizeReplayOperationV1(
  client: FinalizedPayoutClientV1,
  journal: ReplayOperationJournalV1,
  plan: RestoredReplayOperationV1,
): Promise<Readonly<{ signature: string; observedSlot: string; replay: string }>> {
  if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('Claims replay journal is not submitted');
  const meta = await client.transaction(journal.signature);
  if (meta === null) throw new Error('Claims replay transaction is not available at finalized commitment yet');
  if (meta.signature !== journal.signature || !meta.succeeded) {
    throw new Error(`finalized Claims replay signature or status refused: ${meta.errorText ?? 'unknown failure'}`);
  }
  const expectedPacket = submittedReplayWireBytesV1(journal, plan);
  if (!same(meta.transactionBytes, expectedPacket)) {
    throw new Error('finalized Claims replay packet differs from the exact signed journal transaction');
  }
  const expectedAddresses = plan.transaction.message.staticAccountKeys.map((key) => key.toBase58());
  if (meta.accountAddresses.length !== expectedAddresses.length
      || meta.accountAddresses.some((address, index) => address !== expectedAddresses[index])) {
    throw new Error('finalized Claims replay substitutes its exact legacy account closure');
  }
  verifyReplayCreationBalances(meta, plan.owner, plan.replay, BigInt(plan.rentLamports));
  if (meta.returnData === null || meta.returnData.programId !== plan.custodyProgram) {
    throw new Error('finalized Claims replay omitted the exact Custody-produced return receipt');
  }
  const floor = await client.finalizedSlot();
  if (BigInt(floor) < BigInt(meta.slot)) throw new Error('finalized account floor has not reached the Claims replay transaction');
  const observation = await client.multipleAccounts([plan.replay], floor);
  if (BigInt(observation.slot) < BigInt(floor) || observation.accounts.length !== 1
      || observation.accounts[0]?.address !== plan.replay || observation.accounts[0].account === null) {
    throw new Error('Claims replay poststate is absent or substitutes its exact finalized account');
  }
  verifyReplayReceiptAndPoststate(plan, meta.returnData.data, observation.accounts[0].account);
  return Object.freeze({ signature: journal.signature, observedSlot: observation.slot, replay: plan.replay });
}

/** Authenticate the exact finalized packet, receipt producer, balances, and persisted poststate. */
export async function finalizePayoutOperationV1(
  client: FinalizedPayoutClientV1,
  journal: PayoutOperationJournalV1,
  plan: PreparedWalletTerminalPayoutV3,
  verifyPoststate: VerifyPoststate = verifyWalletTerminalPayoutPostconditionV3,
): Promise<Readonly<{ signature: string; observedSlot: string; payout: string }>> {
  if (journal.phase !== 'submitted' || journal.signature === null) throw new Error('payout journal is not submitted');
  const meta = await client.transaction(journal.signature);
  if (meta === null) throw new Error('payout transaction is not available at finalized commitment yet');
  const receiptBytes = verifyFinalizedWalletTerminalPayoutTransactionV3(
    meta,
    journal.signature,
    plan,
    submittedPayoutWireBytesV1(journal, plan),
  );
  const floor = await client.finalizedSlot();
  if (BigInt(floor) < BigInt(meta.slot)) throw new Error('finalized account floor has not reached the payout transaction');
  const route = plan.report.route;
  const addresses = [route.aggregate, route.position, route.custodyReplay, route.hoard, route.recipient];
  const observation = await client.multipleAccounts(addresses, floor);
  if (BigInt(observation.slot) < BigInt(floor)) throw new Error('payout poststate observation regressed below its finalized floor');
  if (observation.accounts.length !== addresses.length
      || observation.accounts.some((entry, index) => entry.address !== addresses[index])) {
    throw new Error('payout poststate response substitutes its exact ordered account closure');
  }
  const account = (key: string) => observation.accounts.find((entry) => entry.address === key)?.account ?? null;
  const aggregate = material(account(route.aggregate), route.claimsProgram, 'post-payout Claims aggregate');
  const position = material(account(route.position), route.claimsProgram, 'post-payout Claims Position');
  const replay = material(account(route.custodyReplay), route.custodyProgram, 'post-payout Custody replay');
  const hoard = material(account(route.hoard), route.tokenProgram, 'post-payout Hoard');
  const recipient = material(account(route.recipient), route.tokenProgram, 'post-payout recipient');
  await verifyPoststate(plan.report, {
    receiptBytes,
    aggregateBytes: aggregate.data,
    positionBytes: position.data,
    custodyReplayBytes: replay.data,
    hoardTokenBytes: hoard.data,
    recipientTokenBytes: recipient.data,
  });
  return Object.freeze({ signature: journal.signature, observedSlot: observation.slot, payout: plan.report.payout });
}
