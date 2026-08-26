import {
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
  type AccountMeta,
} from '@solana/web3.js';

import { ascii, hex, isZero, requireZero, slice, u16, u64 } from './bytes';
import { PACKET_DATA_SIZE } from './directTransaction';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const GENERAL_CANDIDATE_BYTES = 256;
export const GENERAL_EXECUTION_BYTES = 368;
export const GENERAL_PAGE_BYTES = 11_840;
export const GENERAL_POLICY_BYTES = 64;
export const GENERAL_SELECTION_BYTES = 128;
export const GENERAL_SETTLEMENT_BYTES = 208;
export const GENERAL_REQUEST_BYTES = 64;
export const GENERAL_VERIFICATION_BYTES = 960;
export const GENERAL_CERTIFICATE_BYTES = 416;
export const GENERAL_ACTIVATION_BYTES = 1_288;
export const GENERAL_MAX_OUTCOMES = 16;
export const GENERAL_MAX_EXECUTIONS_PER_PAGE = 32;
export const GENERAL_MAX_PAGES = 64;
export const GENERAL_PHYSICAL_ADAPTER_STATUS = 'A transaction is available only after finalized state proves the exact market-scoped PDAs, Registry activation, Loader-v3 program identity, and action account frame. Settlement child wires are not yet committed.';

const MAX_U64 = 18_446_744_073_709_551_615n;

export type GeneralAction = 'consider' | 'freeze' | 'initialize-settlement' | 'collect' | 'materialize' | 'distribute' | 'close';
export type GeneralPhase = 'collecting' | 'materializing' | 'distributing' | 'ready-to-close' | 'terminal';
export type GeneralCriterion = 'maximize filled lots' | 'minimize quote surplus' | 'minimize candidate ID';

export type GeneralCandidateV1 = Readonly<{
  outcomeCount: number;
  candidateId: string;
  productId: string;
  batchId: string;
  pageCount: number;
  priceScale: bigint;
  prices: ReadonlyArray<bigint>;
}>;

export type GeneralExecutionV1 = Readonly<{
  orderId: string;
  ownerId: string;
  nonce: bigint;
  maxLots: bigint;
  maxQuoteDebitPerLot: bigint;
  lots: bigint;
  quoteDebit: bigint;
  quoteCredit: bigint;
  receivePerLot: ReadonlyArray<bigint>;
  deliverPerLot: ReadonlyArray<bigint>;
}>;

export type GeneralPageV1 = Readonly<{
  outcomeCount: number;
  executionCount: number;
  candidateId: string;
  pageIndex: number;
  pageCount: number;
  executions: ReadonlyArray<GeneralExecutionV1>;
}>;

export type GeneralPolicyV1 = Readonly<{
  policyId: string;
  criteria: ReadonlyArray<GeneralCriterion>;
}>;

export type GeneralSelectionV1 = Readonly<{
  closed: boolean;
  batchId: string;
  policyId: string;
  bestCandidateId: string | null;
  revision: bigint;
}>;

export type GeneralSettlementV1 = Readonly<{
  phase: GeneralPhase;
  outcomeCount: number;
  candidateId: string;
  pageCount: number;
  nextPage: number;
  nextExecution: number;
  revision: bigint;
  claimInventory: ReadonlyArray<bigint>;
  quoteInventory: bigint;
  quoteSurplusPaid: bigint;
}>;

export type GeneralVerificationV1 = Readonly<{
  candidate: GeneralCandidateV1;
  nextPage: number;
  revision: bigint;
  hasCurrentOrder: boolean;
  filledLots: bigint;
  quoteInputs: bigint;
  quoteOutputs: bigint;
  claimInputs: ReadonlyArray<bigint>;
  claimOutputs: ReadonlyArray<bigint>;
}>;

export type GeneralCertificateV1 = Readonly<{
  candidateId: string;
  productId: string;
  batchId: string;
  outcomeCount: number;
  pageCount: number;
  filledLots: bigint;
  quoteSurplus: bigint;
  quoteInputs: bigint;
  quoteOutputs: bigint;
  completeSetDirection: 'none' | 'mint' | 'merge';
  completeSetQuantity: bigint;
  claimInputs: ReadonlyArray<bigint>;
  claimOutputs: ReadonlyArray<bigint>;
}>;

export type GeneralActivatedRoleV1 = Readonly<{
  artifactReleaseId: string;
  program: string;
  loaderProgram: string;
  programData: string;
  semanticReleaseId: string;
  elfDigest: string;
  deploymentSlot: bigint;
  upgradeAuthority: string | null;
  bytes: Uint8Array;
}>;

export type GeneralActivationV1 = Readonly<{
  releaseSetId: Uint8Array;
  roles: Readonly<{ core: GeneralActivatedRoleV1; claims: GeneralActivatedRoleV1; trading: GeneralActivatedRoleV1; resolution: GeneralActivatedRoleV1; custody: GeneralActivatedRoleV1 }>;
}>;

export type GeneralAccountKind = 'candidate' | 'page' | 'policy' | 'selection' | 'settlement' | 'verification' | 'certificate' | 'vacant-selection' | 'vacant-settlement' | 'vacant-verification' | 'vacant-certificate';
export type GeneralDecoded = GeneralCandidateV1 | GeneralPageV1 | GeneralPolicyV1 | GeneralSelectionV1 | GeneralSettlementV1 | GeneralVerificationV1 | GeneralCertificateV1 | null;
export type GeneralObservationV1 = Readonly<{
  address: string;
  owner: string;
  observedSlot: string;
  lamports: string;
  kind: GeneralAccountKind;
  value: GeneralDecoded;
  bytes: Uint8Array;
}>;

export type GeneralRefusalV1 = Readonly<{ address: string; observedSlot: string; reason: string }>;
export type GeneralSnapshotV1 = Readonly<{
  programId: string;
  scanSlot: string;
  programExecutable: boolean;
  physicalTransactionsAvailable: false;
  physicalUnavailableReason: string;
  observations: ReadonlyArray<GeneralObservationV1>;
  refused: ReadonlyArray<GeneralRefusalV1>;
}>;

export type GeneralOrderAggregateV1 = Readonly<{
  orderId: string;
  ownerId: string;
  fragments: number;
  filledLots: bigint;
  maxLots: bigint;
  maxQuoteDebitPerLot: bigint;
  expectedQuoteDebit: bigint;
  expectedQuoteCredit: bigint;
  submittedQuoteDebit: bigint;
  submittedQuoteCredit: bigint;
  valid: boolean;
  refusal: string | null;
}>;

export type GeneralCandidatePreviewV1 = Readonly<{
  complete: boolean;
  valid: boolean;
  refusal: string | null;
  candidate: GeneralCandidateV1;
  pages: ReadonlyArray<GeneralPageV1>;
  orders: ReadonlyArray<GeneralOrderAggregateV1>;
  filledLots: bigint;
  quoteInputs: bigint;
  quoteOutputs: bigint;
  quoteSurplus: bigint | null;
  completeSetMove: string;
  completeSetDirection: 'none' | 'mint' | 'merge';
  completeSetQuantity: bigint;
}>;

export type GeneralRequestArtifactV1 = Readonly<{
  action: GeneralAction;
  expectedRevision: bigint;
  candidateId: string | null;
  pageIndex: number;
  executionIndex: number;
  bytes: Uint8Array;
  transactionAvailable: false;
  unavailableReason: string;
}>;

export type GeneralOuterAction = 'consider' | 'freeze' | 'initialize-settlement';
export type GeneralOuterAccountsV1 = Readonly<{
  market: string;
  activationCache: string;
  registryProgram: string;
  tradingProgram: string;
  tradingProgramData: string;
  selection: string;
  verification?: string;
  certificate?: string;
  candidate?: string;
  policy?: string;
  page?: string;
  incumbentCertificate?: string;
  settlement?: string;
}>;

export type GeneralUnsignedTransactionV1 = Readonly<{
  action: GeneralOuterAction;
  request: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  accountCount: number;
}>;

function u32(bytes: Uint8Array, offset: number): number {
  if (offset < 0 || offset + 4 > bytes.length) throw new Error('u32 field is truncated');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function header(bytes: Uint8Array, width: number, magic: string): void {
  if (bytes.length !== width || ascii(bytes, 0, 8) !== magic || u16(bytes, 8) !== 1) throw new Error(`${magic} has the wrong exact width, magic, or generated ABI version`);
}

function id(bytes: Uint8Array, offset: number, field: string): string {
  const value = slice(bytes, offset, 32);
  if (isZero(value)) throw new Error(`${field} is zero`);
  return hex(value);
}

function vector(bytes: Uint8Array, offset: number): ReadonlyArray<bigint> {
  return Object.freeze(Array.from({ length: GENERAL_MAX_OUTCOMES }, (_, index) => u64(bytes, offset + index * 8)));
}

function activeVector(bytes: Uint8Array, offset: number, count: number, field: string): ReadonlyArray<bigint> {
  const values = vector(bytes, offset);
  if (values.slice(count).some((value) => value !== 0n)) throw new Error(`${field} inactive capacity is nonzero`);
  return Object.freeze(values.slice(0, count));
}

function activeCount(count: number): void {
  if (!Number.isInteger(count) || count < 1 || count > GENERAL_MAX_OUTCOMES) throw new Error('General outcome count is outside 1..16');
}

export function decodeGeneralCandidateV1(bytes: Uint8Array): GeneralCandidateV1 {
  header(bytes, GENERAL_CANDIDATE_BYTES, 'DCGCAND1');
  requireZero(bytes, 11, 5, 'General candidate header'); requireZero(bytes, 116, 4, 'General candidate page header');
  const outcomeCount = bytes[10]; activeCount(outcomeCount);
  const pageCount = u32(bytes, 112);
  if (pageCount < 1 || pageCount > GENERAL_MAX_PAGES) throw new Error('General candidate page count is outside 1..64');
  const priceScale = u64(bytes, 120);
  if (priceScale === 0n) throw new Error('General candidate price scale is zero');
  const prices = activeVector(bytes, 128, outcomeCount, 'General price simplex');
  if (prices.reduce((sum, value) => sum + value, 0n) !== priceScale) throw new Error('General candidate prices do not sum exactly to their scale');
  return Object.freeze({ outcomeCount, candidateId: id(bytes, 16, 'candidate ID'), productId: id(bytes, 48, 'product ID'), batchId: id(bytes, 80, 'batch ID'), pageCount, priceScale, prices });
}

function decodeExecution(bytes: Uint8Array, outcomeCount: number): GeneralExecutionV1 {
  if (bytes.length !== GENERAL_EXECUTION_BYTES) throw new Error('General execution row has the wrong exact width');
  const maxLots = u64(bytes, 72); const lots = u64(bytes, 88);
  if (maxLots === 0n || lots === 0n || lots > maxLots) throw new Error('General execution has zero or over-limit lots');
  return Object.freeze({
    orderId: id(bytes, 0, 'order ID'), ownerId: id(bytes, 32, 'order owner ID'), nonce: u64(bytes, 64), maxLots,
    maxQuoteDebitPerLot: u64(bytes, 80), lots, quoteDebit: u64(bytes, 96), quoteCredit: u64(bytes, 104),
    receivePerLot: activeVector(bytes, 112, outcomeCount, 'receive vector'), deliverPerLot: activeVector(bytes, 240, outcomeCount, 'deliver vector'),
  });
}

export function decodeGeneralPageV1(bytes: Uint8Array): GeneralPageV1 {
  header(bytes, GENERAL_PAGE_BYTES, 'DCGPAGE1'); requireZero(bytes, 12, 4, 'General page header'); requireZero(bytes, 56, 8, 'General page cursor');
  const outcomeCount = bytes[10]; activeCount(outcomeCount);
  const executionCount = bytes[11];
  if (executionCount < 1 || executionCount > GENERAL_MAX_EXECUTIONS_PER_PAGE) throw new Error('General page execution count is outside 1..32');
  const pageIndex = u32(bytes, 48); const pageCount = u32(bytes, 52);
  if (pageCount < 1 || pageCount > GENERAL_MAX_PAGES || pageIndex >= pageCount) throw new Error('General page cursor is outside its candidate range');
  const executions = Object.freeze(Array.from({ length: executionCount }, (_, index) => decodeExecution(bytes.slice(64 + index * GENERAL_EXECUTION_BYTES, 64 + (index + 1) * GENERAL_EXECUTION_BYTES), outcomeCount)));
  if (bytes.slice(64 + executionCount * GENERAL_EXECUTION_BYTES).some((value) => value !== 0)) throw new Error('General page inactive execution capacity is nonzero');
  return Object.freeze({ outcomeCount, executionCount, candidateId: id(bytes, 16, 'page candidate ID'), pageIndex, pageCount, executions });
}

export function decodeGeneralPolicyV1(bytes: Uint8Array): GeneralPolicyV1 {
  header(bytes, GENERAL_POLICY_BYTES, 'DCGPOLY1'); requireZero(bytes, 11, 5, 'General policy header');
  const count = bytes[10];
  if (count < 1 || count > 16 || bytes.slice(48 + count).some((value) => value !== 0)) throw new Error('General policy criterion prefix is noncanonical');
  const names: GeneralCriterion[] = ['maximize filled lots', 'minimize quote surplus', 'minimize candidate ID'];
  const criteria = Object.freeze(Array.from(bytes.slice(48, 48 + count), (tag) => {
    const criterion = names[tag]; if (criterion === undefined) throw new Error('General policy has an unknown interpreted criterion'); return criterion;
  }));
  if (criteria[criteria.length - 1] !== 'minimize candidate ID') throw new Error('General policy lacks candidate-ID as its deterministic final tie-break');
  return Object.freeze({ policyId: id(bytes, 16, 'policy ID'), criteria });
}

export function decodeGeneralSelectionV1(bytes: Uint8Array): GeneralSelectionV1 {
  header(bytes, GENERAL_SELECTION_BYTES, 'DCGSELC1'); requireZero(bytes, 12, 4, 'General selection header'); requireZero(bytes, 120, 8, 'General selection tail');
  if (bytes[10] > 1 || bytes[11] > 1) throw new Error('General selection boolean is noncanonical');
  const rawBest = slice(bytes, 80, 32); const bestCandidateId = bytes[11] === 1 ? (isZero(rawBest) ? (() => { throw new Error('General selection best candidate is zero'); })() : hex(rawBest)) : null;
  if (bestCandidateId === null && !isZero(rawBest)) throw new Error('General selection absent-best storage is nonzero');
  return Object.freeze({ closed: bytes[10] === 1, batchId: id(bytes, 16, 'selection batch ID'), policyId: id(bytes, 48, 'selection policy ID'), bestCandidateId, revision: u64(bytes, 112) });
}

export function decodeGeneralSettlementV1(bytes: Uint8Array): GeneralSettlementV1 {
  header(bytes, GENERAL_SETTLEMENT_BYTES, 'DCGSETT1'); requireZero(bytes, 13, 3, 'General settlement header');
  const phases: GeneralPhase[] = ['collecting', 'materializing', 'distributing', 'ready-to-close', 'terminal'];
  const phase = phases[bytes[10]]; if (phase === undefined) throw new Error('General settlement phase is unknown');
  const outcomeCount = bytes[11]; activeCount(outcomeCount);
  const nextExecution = bytes[12]; const pageCount = u32(bytes, 48); const nextPage = u32(bytes, 52);
  if (pageCount < 1 || pageCount > GENERAL_MAX_PAGES) throw new Error('General settlement page count is outside 1..64');
  if ((phase === 'collecting' || phase === 'distributing') ? nextPage >= pageCount || nextExecution >= GENERAL_MAX_EXECUTIONS_PER_PAGE : nextPage !== 0 || nextExecution !== 0) throw new Error('General settlement cursor is noncanonical for its phase');
  const claimInventory = activeVector(bytes, 64, outcomeCount, 'settlement claim inventory');
  const quoteInventory = u64(bytes, 192); const quoteSurplusPaid = u64(bytes, 200);
  if (phase === 'terminal' && (quoteInventory !== 0n || claimInventory.some((value) => value !== 0n))) throw new Error('terminal General settlement retains inventory');
  return Object.freeze({ phase, outcomeCount, candidateId: id(bytes, 16, 'settlement candidate ID'), pageCount, nextPage, nextExecution, revision: u64(bytes, 56), claimInventory, quoteInventory, quoteSurplusPaid });
}

export function decodeGeneralVerificationV1(bytes: Uint8Array): GeneralVerificationV1 {
  header(bytes, GENERAL_VERIFICATION_BYTES, 'DCGVERF1'); requireZero(bytes, 11, 5, 'General verification header'); requireZero(bytes, 276, 4, 'General verification cursor');
  if (bytes[10] > 1) throw new Error('General verification current-order flag is noncanonical');
  const candidate = decodeGeneralCandidateV1(bytes.slice(16, 272)); const nextPage = u32(bytes, 272); const revision = u64(bytes, 952);
  if (nextPage > candidate.pageCount || revision !== BigInt(nextPage)) throw new Error('General verification page and revision cursors disagree');
  const hasCurrentOrder = bytes[10] === 1;
  if (hasCurrentOrder) {
    const current = decodeExecution(bytes.slice(280, 648), candidate.outcomeCount); const lots = u64(bytes, 648);
    if (lots === 0n || lots > current.maxLots) throw new Error('General verification current order has invalid aggregate lots');
  } else if (bytes.slice(280, 672).some((value) => value !== 0)) throw new Error('General verification absent-order storage is nonzero');
  const filledLots = u64(bytes, 672); const quoteInputs = u64(bytes, 680); const quoteOutputs = u64(bytes, 688);
  const claimInputs = activeVector(bytes, 696, candidate.outcomeCount, 'verification claim inputs'); const claimOutputs = activeVector(bytes, 824, candidate.outcomeCount, 'verification claim outputs');
  if ((nextPage === 0) !== (!hasCurrentOrder && filledLots === 0n && quoteInputs === 0n && quoteOutputs === 0n)) throw new Error('General verification empty-prefix state is noncanonical');
  return Object.freeze({ candidate, nextPage, revision, hasCurrentOrder, filledLots, quoteInputs, quoteOutputs, claimInputs, claimOutputs });
}

export function decodeGeneralCertificateV1(bytes: Uint8Array): GeneralCertificateV1 {
  header(bytes, GENERAL_CERTIFICATE_BYTES, 'DCGVCER1'); requireZero(bytes, 12, 4, 'General certificate header');
  const directions = ['none', 'mint', 'merge'] as const; const completeSetDirection = directions[bytes[10]];
  if (completeSetDirection === undefined) throw new Error('General certificate complete-set direction is unknown');
  const outcomeCount = bytes[11]; activeCount(outcomeCount); const pageCount = u32(bytes, 112);
  if (pageCount < 1 || pageCount > GENERAL_MAX_PAGES) throw new Error('General certificate page count is outside 1..64');
  const filledLots = u64(bytes, 120); if (filledLots === 0n) throw new Error('General certificate filled lots are zero');
  const completeSetQuantity = u64(bytes, 152);
  if ((completeSetDirection === 'none') !== (completeSetQuantity === 0n)) throw new Error('General certificate complete-set quantity is noncanonical');
  return Object.freeze({
    candidateId: id(bytes, 16, 'certificate candidate ID'), productId: id(bytes, 48, 'certificate product ID'), batchId: id(bytes, 80, 'certificate batch ID'),
    outcomeCount, pageCount, filledLots, quoteSurplus: u64(bytes, 128), quoteInputs: u64(bytes, 136), quoteOutputs: u64(bytes, 144),
    completeSetDirection, completeSetQuantity, claimInputs: activeVector(bytes, 160, outcomeCount, 'certificate claim inputs'), claimOutputs: activeVector(bytes, 288, outcomeCount, 'certificate claim outputs'),
  });
}

function decodeActivatedRole(bytes: Uint8Array, offset: number): GeneralActivatedRoleV1 {
  const role = bytes.slice(offset, offset + 248); const release = role.slice(32);
  if (release.length !== 216 || ascii(release, 0, 8) !== 'DCLTARF1' || u16(release, 8) !== 1 || u16(release, 10) !== 1) throw new Error('activated General role has an unsupported ArtifactRelease');
  requireZero(release, 13, 3, 'activated ArtifactRelease header');
  const policy = release[12]; if (policy > 1) throw new Error('activated ArtifactRelease upgrade policy is noncanonical');
  const identity = (start: number, field: string) => { const value = slice(release, start, 32); if (isZero(value)) throw new Error(`${field} is zero`); return new PublicKey(value).toBase58(); };
  const artifactReleaseId = hex(slice(role, 0, 32)); if (/^0+$/.test(artifactReleaseId)) throw new Error('activated artifact-release identity is zero');
  if (isZero(slice(release, 112, 32)) || isZero(slice(release, 144, 32))) throw new Error('activated semantic release or ELF digest is zero');
  const upgradeBytes = slice(release, 184, 32); const upgradeAuthority = policy === 0 ? null : (isZero(upgradeBytes) ? (() => { throw new Error('activated exact upgrade authority is zero'); })() : new PublicKey(upgradeBytes).toBase58());
  if (policy === 0 && !isZero(upgradeBytes)) throw new Error('activated immutable ArtifactRelease carries upgrade authority bytes');
  return Object.freeze({ artifactReleaseId, program: identity(16, 'activated program'), loaderProgram: identity(48, 'activated loader'), programData: identity(80, 'activated ProgramData'), semanticReleaseId: hex(slice(release, 112, 32)), elfDigest: hex(slice(release, 144, 32)), deploymentSlot: u64(release, 176), upgradeAuthority, bytes: new Uint8Array(role) });
}

export function decodeGeneralActivationV1(bytes: Uint8Array): GeneralActivationV1 {
  if (bytes.length !== GENERAL_ACTIVATION_BYTES || ascii(bytes, 0, 8) !== 'DCLTACT1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) throw new Error('General activation cache has the wrong exact layout');
  requireZero(bytes, 12, 4, 'General activation header'); const releaseSetId = slice(bytes, 16, 32); if (isZero(releaseSetId)) throw new Error('General activation release-set identity is zero');
  const names = ['core', 'claims', 'trading', 'resolution', 'custody'] as const;
  const entries = names.map((name, index) => [name, decodeActivatedRole(bytes, 48 + index * 248)] as const);
  for (let left = 0; left < entries.length; left += 1) for (let right = left + 1; right < entries.length; right += 1) {
    const a = entries[left][1]; const b = entries[right][1]; const sameProgram = a.program === b.program; const sameArtifact = a.artifactReleaseId === b.artifactReleaseId;
    if (sameProgram !== sameArtifact || (sameProgram && !a.bytes.every((value, index) => value === b.bytes[index]))) throw new Error('General activation partially aliases or inconsistently activates two roles');
  }
  return Object.freeze({ releaseSetId, roles: Object.freeze(Object.fromEntries(entries)) as GeneralActivationV1['roles'] });
}

function classify(account: RpcAccount): Readonly<{ kind: GeneralAccountKind; value: GeneralDecoded }> | null {
  if (account.data.every((value) => value === 0)) {
    if (account.data.length === GENERAL_SELECTION_BYTES) return Object.freeze({ kind: 'vacant-selection', value: null });
    if (account.data.length === GENERAL_SETTLEMENT_BYTES) return Object.freeze({ kind: 'vacant-settlement', value: null });
    if (account.data.length === GENERAL_VERIFICATION_BYTES) return Object.freeze({ kind: 'vacant-verification', value: null });
    if (account.data.length === GENERAL_CERTIFICATE_BYTES) return Object.freeze({ kind: 'vacant-certificate', value: null });
  }
  const magic = account.data.length >= 8 ? ascii(account.data, 0, 8) : '';
  if (magic === 'DCGCAND1') return Object.freeze({ kind: 'candidate', value: decodeGeneralCandidateV1(account.data) });
  if (magic === 'DCGPAGE1') return Object.freeze({ kind: 'page', value: decodeGeneralPageV1(account.data) });
  if (magic === 'DCGPOLY1') return Object.freeze({ kind: 'policy', value: decodeGeneralPolicyV1(account.data) });
  if (magic === 'DCGSELC1') return Object.freeze({ kind: 'selection', value: decodeGeneralSelectionV1(account.data) });
  if (magic === 'DCGSETT1') return Object.freeze({ kind: 'settlement', value: decodeGeneralSettlementV1(account.data) });
  if (magic === 'DCGVERF1') return Object.freeze({ kind: 'verification', value: decodeGeneralVerificationV1(account.data) });
  if (magic === 'DCGVCER1') return Object.freeze({ kind: 'certificate', value: decodeGeneralCertificateV1(account.data) });
  return null;
}

export async function scanGeneralSuccessor(client: SolanaRpcClient, programId: string): Promise<GeneralSnapshotV1> {
  const parsed = new PublicKey(programId); if (parsed.toBase58() !== programId) throw new Error('General program ID must be canonical base58 text');
  const programRead = await client.accountInfo(programId);
  const programExecutable = programRead.account !== null && programRead.account.executable;
  const scan = await client.programHeaders(programId);
  const widths = new Set([GENERAL_CANDIDATE_BYTES, GENERAL_PAGE_BYTES, GENERAL_POLICY_BYTES, GENERAL_SELECTION_BYTES, GENERAL_SETTLEMENT_BYTES, GENERAL_VERIFICATION_BYTES, GENERAL_CERTIFICATE_BYTES]);
  const recognized = scan.accounts.filter((entry) => widths.has(entry.account.space));
  if (recognized.length > 128) throw new Error('General scan exceeds the explicit 128-account reacquisition bound');
  const projected = await Promise.all(recognized.map(async (entry): Promise<GeneralObservationV1 | GeneralRefusalV1> => {
    try {
      const read = await client.accountInfo(entry.address, scan.slot);
      if (read.account === null || read.account.owner !== programId || read.account.executable) throw new Error('General data account disappeared or changed owner/executable state');
      const decoded = classify(read.account);
      if (decoded === null) throw new Error('fixed-width account does not carry one generated General magic');
      return Object.freeze({ address: entry.address, owner: read.account.owner, observedSlot: read.slot, lamports: read.account.lamports, kind: decoded.kind, value: decoded.value, bytes: new Uint8Array(read.account.data) });
    } catch (error) {
      return Object.freeze({ address: entry.address, observedSlot: scan.slot, reason: error instanceof Error ? error.message : 'General account refused' });
    }
  }));
  return Object.freeze({
    programId, scanSlot: scan.slot, programExecutable, physicalTransactionsAvailable: false,
    physicalUnavailableReason: GENERAL_PHYSICAL_ADAPTER_STATUS,
    observations: Object.freeze(projected.filter((entry): entry is GeneralObservationV1 => 'kind' in entry)),
    refused: Object.freeze(projected.filter((entry): entry is GeneralRefusalV1 => !('kind' in entry))),
  });
}

function sameVector(left: ReadonlyArray<bigint>, right: ReadonlyArray<bigint>): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function roundedQuote(candidate: GeneralCandidateV1, execution: GeneralExecutionV1, lots: bigint): Readonly<[bigint, bigint]> | null {
  let receivedPerLot = 0n; let deliveredPerLot = 0n;
  for (let index = 0; index < candidate.outcomeCount; index += 1) {
    const received = execution.receivePerLot[index] * candidate.prices[index]; const delivered = execution.deliverPerLot[index] * candidate.prices[index];
    if (received > MAX_U64 || delivered > MAX_U64 || receivedPerLot + received > MAX_U64 || deliveredPerLot + delivered > MAX_U64) return null;
    receivedPerLot += received; deliveredPerLot += delivered;
  }
  const received = receivedPerLot * lots; const delivered = deliveredPerLot * lots;
  if (received > MAX_U64 || delivered > MAX_U64) return null;
  if (delivered <= received && received - delivered + candidate.priceScale - 1n > MAX_U64) return null;
  return delivered <= received
    ? Object.freeze([(received - delivered + candidate.priceScale - 1n) / candidate.priceScale, 0n])
    : Object.freeze([0n, (delivered - received) / candidate.priceScale]);
}

export function previewGeneralCandidate(candidate: GeneralCandidateV1, pages: ReadonlyArray<GeneralPageV1>): GeneralCandidatePreviewV1 {
  const matching = pages.filter((page) => page.candidateId === candidate.candidateId);
  const ordered = [...matching].sort((left, right) => left.pageIndex - right.pageIndex);
  const complete = ordered.length === candidate.pageCount && ordered.every((page, index) => page.pageIndex === index && page.pageCount === candidate.pageCount && page.outcomeCount === candidate.outcomeCount);
  if (!complete) return Object.freeze({ complete: false, valid: false, refusal: 'candidate pages are missing, duplicated, or disagree with the header cursor', candidate, pages: Object.freeze(ordered), orders: Object.freeze([]), filledLots: 0n, quoteInputs: 0n, quoteOutputs: 0n, quoteSurplus: null, completeSetMove: 'unavailable until every page joins', completeSetDirection: 'none' as const, completeSetQuantity: 0n });
  const executions = ordered.flatMap((page) => page.executions);
  for (let index = 1; index < executions.length; index += 1) {
    if (executions[index].orderId < executions[index - 1].orderId) return Object.freeze({ complete: true, valid: false, refusal: 'physical rows are not globally grouped in increasing order identity', candidate, pages: Object.freeze(ordered), orders: Object.freeze([]), filledLots: 0n, quoteInputs: 0n, quoteOutputs: 0n, quoteSurplus: null, completeSetMove: 'unavailable for an ungrouped candidate', completeSetDirection: 'none' as const, completeSetQuantity: 0n });
  }
  const arithmetic: bigint[] = Array.from({ length: candidate.outcomeCount * 2 + 3 }, () => 0n);
  for (const execution of executions) {
    const values = [execution.lots, execution.quoteDebit, execution.quoteCredit];
    for (let outcome = 0; outcome < candidate.outcomeCount; outcome += 1) values.push(execution.deliverPerLot[outcome] * execution.lots, execution.receivePerLot[outcome] * execution.lots);
    for (let index = 0; index < values.length; index += 1) {
      if (values[index] > MAX_U64 || arithmetic[index] + values[index] > MAX_U64) return Object.freeze({ complete: true, valid: false, refusal: 'candidate arithmetic exceeds one fixed-width u64 accumulator', candidate, pages: Object.freeze(ordered), orders: Object.freeze([]), filledLots: 0n, quoteInputs: 0n, quoteOutputs: 0n, quoteSurplus: null, completeSetMove: 'unavailable after arithmetic overflow', completeSetDirection: 'none' as const, completeSetQuantity: 0n });
      arithmetic[index] += values[index];
    }
  }
  const groups = new Map<string, GeneralExecutionV1[]>();
  for (const execution of executions) groups.set(execution.orderId, [...(groups.get(execution.orderId) ?? []), execution]);
  const orders = Object.freeze([...groups.values()].map((fragments): GeneralOrderAggregateV1 => {
    const first = fragments[0];
    const sameTerms = fragments.every((entry) => entry.ownerId === first.ownerId && entry.nonce === first.nonce && entry.maxLots === first.maxLots
      && entry.maxQuoteDebitPerLot === first.maxQuoteDebitPerLot && sameVector(entry.receivePerLot, first.receivePerLot) && sameVector(entry.deliverPerLot, first.deliverPerLot));
    const filledLots = fragments.reduce((sum, entry) => sum + entry.lots, 0n);
    const submittedQuoteDebit = fragments.reduce((sum, entry) => sum + entry.quoteDebit, 0n);
    const submittedQuoteCredit = fragments.reduce((sum, entry) => sum + entry.quoteCredit, 0n);
    const quote = roundedQuote(candidate, first, filledLots); const [expectedQuoteDebit, expectedQuoteCredit] = quote ?? [0n, 0n]; const limit = first.maxQuoteDebitPerLot * filledLots;
    const refusal = !sameTerms ? 'same order ID carries substituted immutable terms across fragments'
      : filledLots > first.maxLots ? 'candidate-wide filled lots exceed the signed maximum'
        : quote === null || limit > MAX_U64 ? 'candidate-wide price or debit-limit arithmetic exceeds u64'
        : submittedQuoteDebit !== expectedQuoteDebit || submittedQuoteCredit !== expectedQuoteCredit ? 'fragment quote portions do not equal the sole candidate-wide rounding result'
          : submittedQuoteDebit > limit ? 'aggregate quote debit exceeds the signed per-lot limit' : null;
    return Object.freeze({ orderId: first.orderId, ownerId: first.ownerId, fragments: fragments.length, filledLots, maxLots: first.maxLots, maxQuoteDebitPerLot: first.maxQuoteDebitPerLot, expectedQuoteDebit, expectedQuoteCredit, submittedQuoteDebit, submittedQuoteCredit, valid: refusal === null, refusal });
  }));
  const claimInputs = Array.from({ length: candidate.outcomeCount }, (_, outcome) => executions.reduce((sum, execution) => sum + execution.deliverPerLot[outcome] * execution.lots, 0n));
  const claimOutputs = Array.from({ length: candidate.outcomeCount }, (_, outcome) => executions.reduce((sum, execution) => sum + execution.receivePerLot[outcome] * execution.lots, 0n));
  const difference = claimOutputs[0] - claimInputs[0];
  const completeSetDirection = difference === 0n ? 'none' : difference > 0n ? 'mint' : 'merge';
  const completeSetQuantity = difference < 0n ? -difference : difference;
  const claimsBalance = difference === 0n ? claimInputs.every((value, index) => value === claimOutputs[index])
    : difference > 0n ? claimInputs.every((value, index) => claimOutputs[index] === value + difference)
      : claimOutputs.every((value, index) => claimInputs[index] === value - difference);
  const completeSetMove = difference === 0n ? 'none' : difference > 0n ? `mint ${difference} complete sets` : `merge ${-difference} complete sets`;
  const quoteInputs = executions.reduce((sum, execution) => sum + execution.quoteDebit, 0n);
  const quoteOutputs = executions.reduce((sum, execution) => sum + execution.quoteCredit, 0n);
  const quoteAfterMaterialization = difference > 0n ? (difference <= quoteInputs ? quoteInputs - difference : null) : difference < 0n ? quoteInputs - difference : quoteInputs;
  const quoteBalances = quoteAfterMaterialization !== null && quoteOutputs <= quoteAfterMaterialization;
  const refusal = orders.some((order) => !order.valid) ? 'one or more per-order aggregate quote certificates are invalid'
    : !claimsBalance ? 'claim inputs and outputs differ by more than one complete-set move'
      : !quoteBalances ? 'quote outputs exceed quote inventory after complete-set materialization' : null;
  return Object.freeze({ complete: true, valid: refusal === null, refusal, candidate, pages: Object.freeze(ordered), orders, filledLots: executions.reduce((sum, execution) => sum + execution.lots, 0n), quoteInputs, quoteOutputs, quoteSurplus: quoteAfterMaterialization === null ? null : quoteAfterMaterialization - quoteOutputs, completeSetMove, completeSetDirection, completeSetQuantity });
}

function parseId(value: string | null): Uint8Array | null {
  if (value === null) return null;
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error('General candidate ID must be exactly 32 lowercase-hex bytes');
  const output = Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
  if (isZero(output)) throw new Error('General candidate ID is zero');
  return output;
}

function putU64(output: Uint8Array, offset: number, value: bigint): void {
  if (value < 0n || value > MAX_U64) throw new Error('General expected revision is not a u64');
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true);
}

export function encodeGeneralRequestV1(action: GeneralAction, expectedRevision: bigint, candidateId: string | null, pageIndex: number, executionIndex = 0): Uint8Array {
  const tags: Record<GeneralAction, number> = { consider: 0, freeze: 1, 'initialize-settlement': 2, collect: 3, materialize: 4, distribute: 5, close: 6 };
  const parsedId = parseId(candidateId);
  if (!Number.isInteger(pageIndex) || pageIndex < 0 || pageIndex > 4_294_967_295) throw new Error('General page index is not a u32');
  if (!Number.isInteger(executionIndex) || executionIndex < 0 || executionIndex > 255) throw new Error('General execution index is not a byte');
  const paged = action === 'collect' || action === 'distribute';
  const consider = action === 'consider';
  if (action === 'freeze' ? parsedId !== null || pageIndex !== 0 || executionIndex !== 0
    : parsedId === null || (paged ? pageIndex >= GENERAL_MAX_PAGES || executionIndex >= GENERAL_MAX_EXECUTIONS_PER_PAGE
      : consider ? pageIndex >= GENERAL_MAX_PAGES || executionIndex !== 0 : pageIndex !== 0 || executionIndex !== 0)) throw new Error('General request coordinates are noncanonical for this action');
  const output = new Uint8Array(GENERAL_REQUEST_BYTES); output.set(new TextEncoder().encode('DCGREQ01'));
  new DataView(output.buffer).setUint16(8, 1, true); output[10] = tags[action]; putU64(output, 16, expectedRevision);
  if (parsedId !== null) output.set(parsedId, 24);
  new DataView(output.buffer).setUint32(56, pageIndex, true);
  output[60] = executionIndex;
  return output;
}

export function buildGeneralActionRequest(input: Readonly<{
  action: GeneralAction;
  selection?: GeneralSelectionV1 | null;
  verification?: GeneralVerificationV1 | null;
  settlement?: GeneralSettlementV1;
  candidate?: GeneralCandidatePreviewV1;
}>): GeneralRequestArtifactV1 {
  let expectedRevision: bigint; let candidateId: string | null; let pageIndex = 0; let executionIndex = 0;
  if (input.action === 'consider') {
    if (input.candidate === undefined || (input.selection !== undefined && input.selection !== null && (input.selection.closed || input.candidate.candidate.batchId !== input.selection.batchId))) throw new Error('consider requires a vacant or open matching selection and a candidate header');
    if (input.verification !== undefined && input.verification !== null && input.verification.candidate.candidateId !== input.candidate.candidate.candidateId) throw new Error('consider verification cursor belongs to another candidate');
    expectedRevision = input.verification?.revision ?? 0n; pageIndex = input.verification?.nextPage ?? 0; candidateId = input.candidate.candidate.candidateId;
    if (pageIndex >= input.candidate.candidate.pageCount) throw new Error('consider verification is already complete');
  } else if (input.action === 'freeze') {
    if (input.selection === undefined || input.selection.closed || input.selection.bestCandidateId === null) throw new Error('freeze requires an open selection with one best valid submitted candidate');
    expectedRevision = input.selection.revision; candidateId = null;
  } else if (input.action === 'initialize-settlement') {
    if (input.selection === undefined || input.selection === null || !input.selection.closed || input.selection.bestCandidateId === null || input.candidate === undefined || input.candidate.candidate.candidateId !== input.selection.bestCandidateId) throw new Error('settlement initialization requires the frozen best candidate and its verified certificate');
    expectedRevision = 0n; candidateId = input.selection.bestCandidateId;
  } else {
    if (input.settlement === undefined || input.candidate === undefined || !input.candidate.valid || input.settlement.candidateId !== input.candidate.candidate.candidateId || input.settlement.pageCount !== input.candidate.candidate.pageCount || input.settlement.outcomeCount !== input.candidate.candidate.outcomeCount) throw new Error('streamed action requires a matching valid candidate and settlement cursor');
    expectedRevision = input.settlement.revision; candidateId = input.settlement.candidateId;
    if (input.action === 'collect') {
      if (input.settlement.phase !== 'collecting') throw new Error('collect requires Collecting phase'); pageIndex = input.settlement.nextPage; executionIndex = input.settlement.nextExecution;
      const page = input.candidate.pages[pageIndex]; const row = page?.executions[executionIndex]; if (row === undefined) throw new Error('collect cursor does not select one joined execution row');
      if (row.deliverPerLot.some((perLot, outcome) => perLot * row.lots + input.settlement!.claimInventory[outcome] > MAX_U64)
          || row.quoteDebit + input.settlement.quoteInventory > MAX_U64) throw new Error('collect would overflow the fixed-width settlement inventory');
    } else if (input.action === 'materialize') {
      if (input.settlement.phase !== 'materializing') throw new Error('materialize requires Materializing phase');
      if (input.candidate.completeSetDirection === 'mint' && input.candidate.completeSetQuantity > input.settlement.quoteInventory) throw new Error('complete-set mint exceeds collected quote inventory');
      if (input.candidate.completeSetDirection === 'merge' && input.settlement.claimInventory.some((amount) => amount < input.candidate!.completeSetQuantity)) throw new Error('complete-set merge exceeds collected claim inventory');
      if (input.candidate.completeSetDirection === 'merge' && input.settlement.quoteInventory + input.candidate.completeSetQuantity > MAX_U64) throw new Error('complete-set merge would overflow quote inventory');
      if (input.candidate.completeSetDirection === 'mint' && input.settlement.claimInventory.some((amount) => amount + input.candidate!.completeSetQuantity > MAX_U64)) throw new Error('complete-set mint would overflow claim inventory');
    } else if (input.action === 'distribute') {
      if (input.settlement.phase !== 'distributing') throw new Error('distribute requires Distributing phase'); pageIndex = input.settlement.nextPage; executionIndex = input.settlement.nextExecution;
      const page = input.candidate.pages[pageIndex]; const row = page?.executions[executionIndex]; if (row === undefined) throw new Error('distribute cursor does not select one joined execution row');
      if (row.receivePerLot.some((perLot, outcome) => perLot * row.lots > input.settlement!.claimInventory[outcome]) || row.quoteCredit > input.settlement.quoteInventory) throw new Error('distribution row exceeds collected claim or quote inventory');
    } else {
      if (input.settlement.phase !== 'ready-to-close') throw new Error('close requires ReadyToClose phase');
      if (input.settlement.claimInventory.some((amount) => amount !== 0n)) throw new Error('close refuses while claim inventory remains');
      if (input.settlement.quoteSurplusPaid + input.settlement.quoteInventory > MAX_U64) throw new Error('close would overflow cumulative quote surplus');
    }
  }
  return Object.freeze({ action: input.action, expectedRevision, candidateId, pageIndex, executionIndex, bytes: encodeGeneralRequestV1(input.action, expectedRevision, candidateId, pageIndex, executionIndex), transactionAvailable: false, unavailableReason: GENERAL_PHYSICAL_ADAPTER_STATUS });
}

const PDA = Object.freeze({
  activation: new TextEncoder().encode('dclutch:release-activation:v1'),
  selection: new TextEncoder().encode('dclutch:general-selection:v1'),
  verification: new TextEncoder().encode('dclutch:general-verification:v1'),
  certificate: new TextEncoder().encode('dclutch:general-certificate:v1'),
  settlement: new TextEncoder().encode('dclutch:general-settlement:v1'),
  candidate: new TextEncoder().encode('dclutch:general-candidate:v1'),
  policy: new TextEncoder().encode('dclutch:general-policy:v1'),
  page: new TextEncoder().encode('dclutch:general-page:v1'),
});

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} is not canonical base58 text`);
  return parsed;
}

function idKey(value: string, field: string): Uint8Array {
  const parsed = parseId(value);
  if (parsed === null) throw new Error(`${field} is absent`);
  return parsed;
}

function pda(program: PublicKey, seeds: ReadonlyArray<Uint8Array>): PublicKey {
  return PublicKey.findProgramAddressSync([...seeds], program)[0];
}

function requireAddress(actual: string | undefined, expected: PublicKey, field: string): string {
  if (actual === undefined || key(actual, field).toBase58() !== expected.toBase58()) throw new Error(`${field} is not the exact market-scoped General PDA`);
  return actual;
}

function u32Seed(value: number): Uint8Array {
  const bytes = new Uint8Array(4); new DataView(bytes.buffer).setUint32(0, value, true); return bytes;
}

export async function reacquireGeneralPhysicalAuthority(
  client: SolanaRpcClient,
  accounts: Pick<GeneralOuterAccountsV1, 'market' | 'activationCache' | 'registryProgram' | 'tradingProgram' | 'tradingProgramData'>,
  minimumContextSlot: string,
): Promise<Readonly<{ activation: GeneralActivationV1; observedSlot: string }>> {
  const addresses = [accounts.market, accounts.activationCache, accounts.registryProgram, accounts.tradingProgram, accounts.tradingProgramData];
  addresses.forEach((address, index) => key(address, ['market', 'activation cache', 'Registry program', 'Trading program', 'Trading ProgramData'][index]));
  const read = await client.multipleAccounts(addresses, minimumContextSlot);
  const [market, cache, registry, trading, programData] = read.accounts.map((entry) => entry.account);
  if (market === null || market.executable) throw new Error('Market is missing or executable');
  if (cache === null || cache.owner !== accounts.registryProgram || cache.executable) throw new Error('activation cache is missing or not Registry-owned data');
  if (registry === null || !registry.executable) throw new Error('Registry program is missing or not executable');
  const activation = decodeGeneralActivationV1(cache.data);
  const expectedCache = pda(key(accounts.registryProgram, 'Registry program'), [PDA.activation, activation.releaseSetId]);
  if (expectedCache.toBase58() !== accounts.activationCache) throw new Error('activation cache is not the exact release-set PDA');
  if (activation.roles.trading.program !== accounts.tradingProgram || activation.roles.trading.programData !== accounts.tradingProgramData) throw new Error('selected General program is not the activated Trading role');
  if (trading === null || !trading.executable || trading.owner !== activation.roles.trading.loaderProgram || trading.data.length !== 36 || u32(trading.data, 0) !== 2 || new PublicKey(trading.data.slice(4, 36)).toBase58() !== accounts.tradingProgramData) throw new Error('Trading program does not link to the activated Loader-v3 ProgramData');
  if (programData === null || programData.executable || programData.owner !== activation.roles.trading.loaderProgram) throw new Error('activated Trading ProgramData is missing or has the wrong loader authority');
  return Object.freeze({ activation, observedSlot: read.slot });
}

export function buildGeneralOuterTransaction(input: Readonly<{
  action: GeneralOuterAction;
  payer: string;
  recentBlockhash: string;
  accounts: GeneralOuterAccountsV1;
  activation: GeneralActivationV1;
  request: GeneralRequestArtifactV1;
  candidate?: GeneralCandidateV1;
  policy?: GeneralPolicyV1;
  selection?: GeneralSelectionV1 | null;
}>): GeneralUnsignedTransactionV1 {
  if (input.request.action !== input.action || input.request.bytes.length !== GENERAL_REQUEST_BYTES) throw new Error('General request does not match the outer action');
  const program = key(input.accounts.tradingProgram, 'Trading program'); const market = key(input.accounts.market, 'Market');
  if (input.activation.roles.trading.program !== program.toBase58() || input.activation.roles.trading.programData !== input.accounts.tradingProgramData) throw new Error('outer action is not bound to the activated Trading artifact');
  const payer = key(input.payer, 'fee payer');
  const common: AccountMeta[] = [
    { pubkey: market, isSigner: false, isWritable: false },
    { pubkey: key(input.accounts.activationCache, 'activation cache'), isSigner: false, isWritable: false },
    { pubkey: key(input.accounts.registryProgram, 'Registry program'), isSigner: false, isWritable: false },
    { pubkey: program, isSigner: false, isWritable: false },
    { pubkey: key(input.accounts.tradingProgramData, 'Trading ProgramData'), isSigner: false, isWritable: false },
  ];
  let metas: AccountMeta[];
  if (input.action === 'consider') {
    if (input.candidate === undefined || input.policy === undefined || input.request.candidateId !== input.candidate.candidateId) throw new Error('Consider lacks its exact candidate and policy semantics');
    const candidateId = idKey(input.candidate.candidateId, 'candidate ID'); const batchId = idKey(input.candidate.batchId, 'batch ID'); const policyId = idKey(input.policy.policyId, 'policy ID');
    const selection = requireAddress(input.accounts.selection, pda(program, [PDA.selection, market.toBytes(), batchId]), 'selection');
    const verification = requireAddress(input.accounts.verification, pda(program, [PDA.verification, market.toBytes(), candidateId]), 'verification');
    const certificate = requireAddress(input.accounts.certificate, pda(program, [PDA.certificate, market.toBytes(), candidateId]), 'certificate');
    const candidate = requireAddress(input.accounts.candidate, pda(program, [PDA.candidate, market.toBytes(), candidateId]), 'candidate');
    const policy = requireAddress(input.accounts.policy, pda(program, [PDA.policy, market.toBytes(), policyId]), 'policy');
    const page = requireAddress(input.accounts.page, pda(program, [PDA.page, market.toBytes(), candidateId, u32Seed(input.request.pageIndex)]), 'candidate page');
    const incumbent = input.selection?.bestCandidateId === null || input.selection === null || input.selection === undefined
      ? market.toBase58()
      : requireAddress(input.accounts.incumbentCertificate, pda(program, [PDA.certificate, market.toBytes(), idKey(input.selection.bestCandidateId, 'incumbent candidate ID')]), 'incumbent certificate');
    metas = [...common, { pubkey: key(selection, 'selection'), isSigner: false, isWritable: true }, { pubkey: key(verification, 'verification'), isSigner: false, isWritable: true }, { pubkey: key(certificate, 'certificate'), isSigner: false, isWritable: true }, { pubkey: key(candidate, 'candidate'), isSigner: false, isWritable: false }, { pubkey: key(policy, 'policy'), isSigner: false, isWritable: false }, { pubkey: key(page, 'candidate page'), isSigner: false, isWritable: false }, { pubkey: key(incumbent, 'incumbent certificate'), isSigner: false, isWritable: false }];
  } else if (input.action === 'freeze') {
    if (input.selection === undefined || input.selection === null) throw new Error('Freeze lacks a decoded selection');
    const selection = requireAddress(input.accounts.selection, pda(program, [PDA.selection, market.toBytes(), idKey(input.selection.batchId, 'batch ID')]), 'selection');
    metas = [...common, { pubkey: key(selection, 'selection'), isSigner: false, isWritable: true }];
  } else {
    if (input.candidate === undefined || input.selection === undefined || input.selection === null || input.request.candidateId !== input.candidate.candidateId) throw new Error('InitializeSettlement lacks its frozen candidate semantics');
    const candidateId = idKey(input.candidate.candidateId, 'candidate ID');
    const selection = requireAddress(input.accounts.selection, pda(program, [PDA.selection, market.toBytes(), idKey(input.selection.batchId, 'batch ID')]), 'selection');
    const settlement = requireAddress(input.accounts.settlement, pda(program, [PDA.settlement, market.toBytes(), candidateId]), 'settlement');
    const certificate = requireAddress(input.accounts.certificate, pda(program, [PDA.certificate, market.toBytes(), candidateId]), 'certificate');
    const candidate = requireAddress(input.accounts.candidate, pda(program, [PDA.candidate, market.toBytes(), candidateId]), 'candidate');
    metas = [...common, { pubkey: key(selection, 'selection'), isSigner: false, isWritable: false }, { pubkey: key(settlement, 'settlement'), isSigner: false, isWritable: true }, { pubkey: key(certificate, 'certificate'), isSigner: false, isWritable: false }, { pubkey: key(candidate, 'candidate'), isSigner: false, isWritable: false }];
  }
  if (metas.some((meta) => meta.pubkey.equals(payer))) throw new Error('fee payer aliases a semantic account and would escalate its privileges');
  const instruction = new TransactionInstruction({ programId: program, keys: metas, data: input.request.bytes });
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: payer, recentBlockhash: input.recentBlockhash, instructions: [instruction] }).compileToV0Message());
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`General transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  return Object.freeze({ action: input.action, request: new Uint8Array(input.request.bytes), transaction, wireBytes: new Uint8Array(wireBytes), requiredSigners: Object.freeze([payer.toBase58()]), accountCount: metas.length });
}
