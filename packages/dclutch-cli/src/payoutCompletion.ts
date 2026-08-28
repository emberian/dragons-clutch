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
  verifyWalletTerminalPayoutPostconditionV3,
  walletTerminalPayoutSummaryV3,
  type PreparedWalletTerminalPayoutV3,
  type WalletTerminalPayoutManifestV3,
  type WalletTerminalPayoutPoststateV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';
import type {
  MultipleAccountObservation,
  RpcAccount,
  TransactionMetaObservation,
} from '@dclutch/sdk/rpc';
import { Keypair, PublicKey, VersionedTransaction } from '@solana/web3.js';

const JOURNAL_FORMAT = 'dclutch-client-operation-journal-v1' as const;
const JOURNAL_OPERATION = 'wallet-terminal-payout-v3' as const;
const JOURNAL_PLAN_FORMAT = 'dclutch-wallet-terminal-payout-journal-plan-v1' as const;
const INPUT_FORMAT = 'dclutch-wallet-terminal-payout-plan-input-v1' as const;
const EVIDENCE_FORMAT = 'dclutch-local-successor-run-evidence-v2' as const;
const MAX_JOURNAL_BYTES = 786_432;
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
  let decoded: unknown;
  try { decoded = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(evidenceBytes)); } catch {
    throw new Error('campaign evidence is not canonical UTF-8 JSON');
  }
  const evidence = object(decoded, 'campaign evidence');
  exactKeys(evidence, [
    'schema', 'rpc_url', 'ledger', 'validator_log', 'plan_sha256', 'core_upgrade_authority_pubkey',
    'private_key_persisted', 'keypair_derivation', 'keypair_seed_sha256', 'foundingCustodyContext',
    'directSelectedManifestEntryIndex', 'completed', 'transactions', 'accounts', 'remaining_execution_seam',
  ], 'campaign evidence');
  if (evidence.schema !== EVIDENCE_FORMAT || evidence.private_key_persisted !== false
      || typeof evidence.rpc_url !== 'string' || typeof evidence.ledger !== 'string'
      || typeof evidence.validator_log !== 'string' || typeof evidence.keypair_derivation !== 'string'
      || typeof evidence.remaining_execution_seam !== 'string') {
    throw new Error('campaign evidence has another schema, key policy, or bounded text shape');
  }
  const expectedPlan = identity(evidence.plan_sha256, 'campaign plan digest');
  if (sha256(planBytes) !== expectedPlan) throw new Error('campaign evidence does not authenticate the exact plan bytes');
  address(evidence.core_upgrade_authority_pubkey, 'campaign Core authority');
  identity(evidence.foundingCustodyContext, 'founding Custody context');
  index(evidence.directSelectedManifestEntryIndex, 'Direct selected manifest entry', 65_535);
  if (evidence.keypair_seed_sha256 !== null) identity(evidence.keypair_seed_sha256, 'campaign keypair seed digest');
  if (!Array.isArray(evidence.completed) || evidence.completed.length === 0
      || evidence.completed.some((entry) => typeof entry !== 'string' || entry.length === 0 || entry.length > 512)
      || new Set(evidence.completed).size !== evidence.completed.length) {
    throw new Error('campaign evidence does not carry one nonempty ordered completed-stage list');
  }
  if (!Array.isArray(evidence.transactions)) throw new Error('campaign evidence transactions are not an array');
  const transactionLabels = new Set<string>();
  for (const [offset, raw] of evidence.transactions.entries()) {
    const row = object(raw, `campaign transaction ${offset}`);
    exactKeys(row, [
      'label', 'signature', 'slot', 'transaction_metadata_available', 'fee_lamports',
      'fee_only_balance_change', 'compute_units_consumed', 'error', 'logs',
    ], `campaign transaction ${offset}`);
    if (typeof row.label !== 'string' || row.label.length === 0 || row.label.length > 512
        || transactionLabels.has(row.label)) throw new Error(`campaign transaction ${offset} has another label shape`);
    transactionLabels.add(row.label);
    exactSignature(row.signature);
    if (typeof row.slot !== 'number' || !Number.isSafeInteger(row.slot) || row.slot < 0
        || typeof row.transaction_metadata_available !== 'boolean'
        || (row.fee_lamports !== null && (typeof row.fee_lamports !== 'number'
          || !Number.isSafeInteger(row.fee_lamports) || row.fee_lamports < 0))
        || (row.compute_units_consumed !== null && (typeof row.compute_units_consumed !== 'number'
          || !Number.isSafeInteger(row.compute_units_consumed) || row.compute_units_consumed < 0))
        || (row.fee_only_balance_change !== null && typeof row.fee_only_balance_change !== 'boolean')
        || !Array.isArray(row.logs)
        || row.logs.some((entry) => typeof entry !== 'string' || entry.length > 512)) {
      throw new Error(`campaign transaction ${offset} has inexact finalized evidence`);
    }
  }
  const accounts = object(evidence.accounts, 'campaign evidence accounts');
  if (Object.keys(accounts).length === 0) throw new Error('campaign evidence has no persisted accounts');
  for (const [label, raw] of Object.entries(accounts)) {
    if (label.length === 0 || label.length > 128) throw new Error('campaign evidence account label is not bounded');
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
  let raw: unknown;
  try { raw = JSON.parse(source); } catch { throw new Error('wallet payout projected input is not JSON'); }
  const value = object(raw, 'wallet payout projected input');
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
  if (source.length === 0 || source.length > MAX_JOURNAL_BYTES) throw new Error('payout journal is outside its exact byte bound');
  let raw: unknown;
  try { raw = JSON.parse(source); } catch { throw new Error('payout journal is not JSON'); }
  const value = object(raw, 'payout journal');
  exactKeys(value, [
    'format', 'operation', 'clusterGenesis', 'market', 'owner', 'operationDigest', 'intentDigest',
    'planDigest', 'intent', 'plan', 'phase', 'signature',
  ], 'payout journal');
  if (value.format !== JOURNAL_FORMAT || value.operation !== JOURNAL_OPERATION
      || (value.phase !== 'unsigned' && value.phase !== 'submitted')
      || typeof value.intent !== 'string' || typeof value.plan !== 'string') throw new Error('payout journal has another format, operation, phase, or payload shape');
  const signature = value.signature;
  if ((value.phase === 'unsigned' && signature !== null) || (value.phase === 'submitted' && typeof signature !== 'string')) {
    throw new Error('payout journal phase and signature disagree');
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
  });
  if (sha256(journal.intent) !== journal.intentDigest || sha256(journal.plan) !== journal.planDigest) {
    throw new Error('payout journal intent or plan bytes differ from their stored digest');
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
  });
  atomicWrite(path, `${JSON.stringify(journal)}\n`);
  return journal;
}

export function markPayoutOperationSubmittedV1(
  path: string,
  journal: PayoutOperationJournalV1,
  signature: string,
): PayoutOperationJournalV1 {
  const current = loadPayoutOperationJournalV1(path);
  if (current === null || JSON.stringify(current) !== JSON.stringify(journal)) throw new Error('payout journal changed before submission');
  const exact = exactSignature(signature);
  if (current.phase === 'submitted') {
    if (current.signature !== exact) throw new Error('submitted payout journal names another transaction');
    return current;
  }
  const submitted = Object.freeze({ ...current, phase: 'submitted' as const, signature: exact });
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

function parseJournalPlan(source: string): PayoutJournalPlanV1 {
  let raw: unknown;
  try { raw = JSON.parse(source); } catch { throw new Error('saved payout verifier plan is not JSON'); }
  const value = object(raw, 'saved payout verifier plan');
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

function signatureBytes(value: string): Uint8Array {
  const exact = exactSignature(value);
  let numeric = 0n;
  for (const character of exact) numeric = numeric * 58n + BigInt(BASE58.indexOf(character));
  const significant: number[] = [];
  while (numeric > 0n) { significant.push(Number(numeric & 0xffn)); numeric >>= 8n; }
  significant.reverse();
  let zeroes = 0; while (zeroes < exact.length && exact[zeroes] === '1') zeroes += 1;
  const output = new Uint8Array(zeroes + significant.length); output.set(significant, zeroes);
  if (output.length !== 64) throw new Error('payout signature is not 64 bytes');
  return output;
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

function exactSignedPacket(plan: PreparedWalletTerminalPayoutV3, signature: string): Uint8Array {
  const transaction = VersionedTransaction.deserialize(plan.wireBytes);
  if (transaction.signatures.length !== 1 || plan.requiredSigners.length !== 1) {
    throw new Error('saved payout transaction does not have one exact signer');
  }
  transaction.signatures[0] = signatureBytes(signature);
  return transaction.serialize();
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

function verifyFeeOnlyBalances(meta: TransactionMetaObservation, payer: string): void {
  if (meta.accountAddresses.length !== meta.preBalances.length || meta.preBalances.length !== meta.postBalances.length) {
    throw new Error('finalized payout balance vectors do not cover its exact account list');
  }
  const payerIndex = meta.accountAddresses.indexOf(payer);
  if (payerIndex < 0 || meta.accountAddresses.lastIndexOf(payer) !== payerIndex) throw new Error('finalized payout does not name one exact fee payer');
  const fee = BigInt(meta.feeLamports);
  for (let index = 0; index < meta.preBalances.length; index += 1) {
    const before = BigInt(meta.preBalances[index]!); const after = BigInt(meta.postBalances[index]!);
    if (index === payerIndex ? after + fee !== before : after !== before) {
      throw new Error('finalized payout lamport balances differ by more than the exact payer fee');
    }
  }
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
  if (meta.signature !== journal.signature || !meta.succeeded) throw new Error(`finalized payout signature or status refused: ${meta.errorText ?? 'unknown failure'}`);
  const expectedPacket = exactSignedPacket(plan, journal.signature);
  if (!same(meta.transactionBytes, expectedPacket)) throw new Error('finalized payout packet differs from the exact signed journal transaction');
  const receipt = meta.returnData;
  if (receipt === null || receipt.programId !== plan.report.route.claimsProgram) {
    throw new Error('finalized payout omitted the exact Claims-produced return receipt');
  }
  verifyFeeOnlyBalances(meta, plan.requiredSigners[0]!);
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
    receiptBytes: receipt.data,
    aggregateBytes: aggregate.data,
    positionBytes: position.data,
    custodyReplayBytes: replay.data,
    hoardTokenBytes: hoard.data,
    recipientTokenBytes: recipient.data,
  });
  return Object.freeze({ signature: journal.signature, observedSlot: observation.slot, payout: plan.report.payout });
}
