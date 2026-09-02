import { ComputeBudgetProgram, PublicKey, VersionedTransaction } from '@solana/web3.js';

import { fromHex, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import * as Abi from './generated/generalSuccessorV5';
import { SYSTEM_PROGRAM_ID } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import {
  acquireUnsignedTransactionDependenciesV1,
  inspectUnsignedTransactionV1,
  type UnsignedTransactionChainReportV1,
  type UnsignedTransactionInspectionV1,
} from './walletHandoff';

const MAX_U64 = 18_446_744_073_709_551_615n;
const ACTIONS = [
  'consider', 'freeze', 'initialize-settlement', 'collect', 'materialize', 'distribute', 'close',
  'open-batch', 'place-order', 'cancel-order', 'close-batch', 'submit-candidate',
  'verify-candidate-row', 'release-order', 'close-candidate',
] as const;
const PLAN_ACTIONS: ReadonlyArray<GeneralSuccessorActionV5> = ACTIONS;
const ARTIFACT_KEYS = ['programSet', 'descriptor', 'config', 'accountProfile', 'lifecyclePolicy', 'requestProfile', 'strategy', 'certificate', 'admission', 'transition', 'effect'] as const;

export type GeneralSuccessorActionV5 = (typeof ACTIONS)[number];
export type GeneralChildRoleV5 = 'claims' | 'custody';
export type GeneralSettlementPhaseV2 = 'collecting' | 'materializing' | 'distributing' | 'ready-to-close' | 'terminal';

export type GeneralDecodedRequestV3 = Readonly<{
  wire: 'v2' | 'v3';
  action: GeneralSuccessorActionV5;
  expectedRevision: bigint;
  subjectId: string | null;
  pageIndex: number;
  executionIndex: number;
  manifestOrderIndex: number;
  primaryStateBump: number;
  secondaryStateBump: number;
  resultStateBump: number;
  bytes: Uint8Array;
}>;

export type GeneralChildRouteV5 = Readonly<{
  route: number;
  role: GeneralChildRoleV5;
  accountStart: number;
  accountCount: number;
  receiptDependencies: ReadonlyArray<Readonly<{
    producerRole: GeneralChildRoleV5;
    producerRoute: number;
    expectedReceiptBytes: number;
  }>>;
}>;

export type GeneralLifecycleStateV5 = Readonly<{
  accountCoordinate: number;
  account: string;
  bump: number;
  isMaterialized: boolean;
}>;

export type GeneralSuccessorPlanDocumentV5 = Readonly<{
  format: 'dclutch/general-successor-plan/v5';
  action: GeneralSuccessorActionV5;
  transactionBase64: string;
  observedSlot: bigint;
  outcomeCount: number;
  scratchPageCount: number;
  heapFrameBytes: number;
  tradingProgram: string;
  lookupTable: string;
  payer: string;
  requiredSigners: ReadonlyArray<string>;
  market: string;
  root: string;
  generation: bigint;
  releaseSet: string;
  rootPrestateDigest: string;
  familyRequestDigest: string;
  checkedManifestDigest: string;
  tradingArtifactRelease: string;
  generalArtifactRelease: string;
  productRecord: string;
  artifacts: Readonly<Record<(typeof ARTIFACT_KEYS)[number], string>>;
  lifecycle: Readonly<{
    primary: GeneralLifecycleStateV5;
    secondary: GeneralLifecycleStateV5 | null;
    conditionalResult: GeneralLifecycleStateV5 | null;
    terminalCoordinate: bigint | null;
    childAccountStart: number;
  }>;
  childRoutes: ReadonlyArray<GeneralChildRouteV5>;
}>;

export type GeneralHotEnvelopeV3 = Readonly<{
  releaseSet: string;
  market: string;
  generation: bigint;
  rootPrestateDigest: string;
  /**
   * The eight caller-mined PDA bumps the envelope's tail carries, in
   * `HotBumpHintsV1` slot order. Zero means the hint is absent and the route
   * searches for that address exactly as it did before the block existed, so
   * an all-zero tail is canonical and is NOT refused -- these bytes were
   * required-zero reserved space only until `d0306a64`.
   */
  bumpHints: ReadonlyArray<number>;
}>;

export type GeneralPlanInspectionV5 = Readonly<{
  plan: GeneralSuccessorPlanDocumentV5;
  transaction: UnsignedTransactionInspectionV1;
  envelope: GeneralHotEnvelopeV3;
  request: GeneralDecodedRequestV3;
}>;

export type GeneralSelectionStatusV2 = Readonly<{
  kind: 'selection';
  phase: 'open' | 'frozen';
  outcomeCount: number;
  revision: bigint;
  submittedCount: number;
  bestCandidateCoordinate: number;
  bestVerifiedRevision: bigint;
  priceScale: bigint;
  productId: string;
  batchId: string;
  policyId: string;
  bestCandidateId: string;
  bestVerifiedDigest: string;
  bestFilledLots: bigint;
  bestQuoteSurplus: bigint;
}>;

export type GeneralSettlementStatusV2 = Readonly<{
  kind: 'settlement';
  phase: GeneralSettlementPhaseV2;
  outcomeCount: number;
  orderCount: number;
  nextOrder: number;
  revision: bigint;
  candidateId: string;
  quoteInventory: bigint;
  completeSetQuantity: bigint;
  terminalCoordinate: bigint;
  inventory: ReadonlyArray<bigint>;
}>;

export type GeneralBatchStatusV1 = Readonly<{
  kind: 'batch';
  phase: 'collecting' | 'closed';
  outcomeCount: number;
  sequence: bigint;
  generation: bigint;
  market: string;
  productId: string;
  configId: string;
  priceScale: bigint;
  collectionCloseSlot: bigint;
  maxOrders: number;
  settlementCloseSlot: bigint;
  orderCount: number;
  openedRootRevision: bigint;
  closedRootRevision: bigint;
  committedQuoteReserve: bigint;
  cancelledCount: number;
}>;

export type GeneralOrderStatusV1 = Readonly<{
  kind: 'order';
  phase: 'placed' | 'cancelled' | 'released';
  outcomeCount: number;
  nonce: bigint;
  owner: string;
  market: string;
  batchId: string;
  generation: bigint;
  maxLots: bigint;
  maxQuoteDebitPerLot: bigint;
  validUntilSlot: bigint;
  admittedSlot: bigint;
  releasedSlot: bigint;
  receivePerLot: ReadonlyArray<bigint>;
  deliverPerLot: ReadonlyArray<bigint>;
}>;

export type GeneralCandidateStatusV1 = Readonly<{
  kind: 'candidate';
  phase: 'submitted' | 'verified' | 'considered';
  outcomeCount: number;
  pageCount: number;
  pageRevision: bigint;
  candidateId: string;
  batchId: string;
  solver: string;
  verifiedDigest: string | null;
  submittedSlot: bigint;
  verifiedRevision: bigint;
  rowCount: number;
  rewardRateLamports: bigint;
  verificationRemaining: bigint;
  cleanupRemaining: bigint;
}>;

export type GeneralVerifierCurrentOrderV2 = Readonly<{
  orderId: string;
  owner: string;
  nonce: bigint;
  maxLots: bigint;
  maxQuoteDebitPerLot: bigint;
  lots: bigint;
  sourcePageIndex: number;
  sourceExecutionIndex: number;
  receivePerLot: ReadonlyArray<bigint>;
  deliverPerLot: ReadonlyArray<bigint>;
}>;

export type GeneralVerifierStatusV2 = Readonly<{
  kind: 'verifier';
  phase: 'initial' | 'streaming' | 'complete';
  outcomeCount: number;
  pageCount: number;
  nextPageIndex: number;
  nextRowIndex: number;
  orderCount: number;
  revision: bigint;
  candidateCoordinate: number;
  candidateId: string;
  productId: string;
  batchId: string;
  priceScale: bigint;
  filledLots: bigint;
  quoteDebit: bigint;
  quoteCredit: bigint;
  prices: ReadonlyArray<bigint>;
  claimInputs: ReadonlyArray<bigint>;
  claimOutputs: ReadonlyArray<bigint>;
  currentOrder: GeneralVerifierCurrentOrderV2 | null;
}>;

export type GeneralVerifiedCandidateStatusV2 = Readonly<{
  kind: 'verified-candidate';
  outcomeCount: number;
  pageCount: number;
  candidateCoordinate: number;
  revision: bigint;
  candidateId: string;
  productId: string;
  batchId: string;
  filledLots: bigint;
  quoteDebit: bigint;
  quoteCredit: bigint;
  priceScale: bigint;
  claimInputs: ReadonlyArray<bigint>;
  claimOutputs: ReadonlyArray<bigint>;
}>;

export type GeneralLocalStateValueV3 = GeneralSelectionStatusV2 | GeneralSettlementStatusV2 | GeneralBatchStatusV1 | GeneralOrderStatusV1 | GeneralCandidateStatusV1 | GeneralVerifierStatusV2;

export type GeneralLocalStateStatusV3 = Readonly<{
  status: GeneralLocalStateValueV3;
  bump: number;
  rentPrincipal: bigint;
  beneficiary: string;
}>;

export type GeneralChainStatusV5 = Readonly<{
  observedSlot: string;
  dependencies: UnsignedTransactionChainReportV1;
  primary: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }>;
  secondary: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }> | null;
  conditionalResult: GeneralVerifiedCandidateStatusV2 | Readonly<{ status: 'vacant'; lamports: bigint }> | null;
  candidateClose: Readonly<{
    cranker: string;
    solver: string;
    closedBatchAccount: string;
    closedBatch: GeneralBatchStatusV1;
  }> | null;
}>;

export type GeneralHotReceiptV3 = Readonly<{
  releaseSet: string;
  market: string;
  generation: bigint;
  root: string;
  requestDigest: string;
  selectedProgram: string;
  rootPrestateDigest: string;
  rootPoststateDigest: string;
  executionDigest: string;
}>;

type MessageView = Readonly<{
  header: Readonly<{ numRequiredSignatures: number }>;
  staticAccountKeys: ReadonlyArray<PublicKey>;
  addressTableLookups: ReadonlyArray<Readonly<{ accountKey: PublicKey }>>;
  compiledInstructions: ReadonlyArray<Readonly<{ programIdIndex: number; accountKeyIndexes: Uint8Array | number[]; data: Uint8Array }>>;
}>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function object(value: unknown, field: string): Record<string, unknown> {
  if (!plain(value)) throw new Error(`${field} is not an object`);
  return value;
}

function text(value: unknown, field: string, maximum = 256): string {
  if (typeof value !== 'string' || value.trim() !== value || value.length === 0 || value.length > maximum) throw new Error(`${field} is not bounded canonical text`);
  return value;
}

function address(value: unknown, field: string): string {
  const input = text(value, field, 64);
  const parsed = new PublicKey(input).toBase58();
  if (parsed !== input) throw new Error(`${field} is not canonical base58 text`);
  return input;
}

function identity(value: unknown, field: string): string {
  const input = text(value, field, 64);
  requireNonzero(fromHex(input, field), field);
  return input;
}

function integer(value: unknown, field: string, maximum = 4_294_967_295): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > maximum) throw new Error(`${field} is not an exact bounded unsigned integer`);
  return value;
}

function integerText(value: unknown, field: string): bigint {
  const input = text(value, field, 32);
  if (!/^(0|[1-9][0-9]*)$/.test(input)) throw new Error(`${field} is not canonical unsigned decimal text`);
  const parsed = BigInt(input);
  if (parsed > MAX_U64) throw new Error(`${field} exceeds u64`);
  return parsed;
}

function exactKeys(value: Record<string, unknown>, keys: ReadonlyArray<string>, field: string): void {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) throw new Error(`${field} has missing or extraneous fields`);
}

function action(value: unknown): GeneralSuccessorActionV5 {
  if (typeof value !== 'string' || !ACTIONS.includes(value as GeneralSuccessorActionV5)) throw new Error('General plan action is unknown');
  return value as GeneralSuccessorActionV5;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function readU32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function writeU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function writeU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 4).setUint32(0, value, true);
}

function writeU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function base64Bytes(value: string, field: string, exact?: number): Uint8Array {
  if (value.trim() !== value || value.length === 0 || value.length > 4_096 || value.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(value)) throw new Error(`${field} is not bounded canonical base64 text`);
  let output: Uint8Array;
  try { output = Uint8Array.from(atob(value), (character) => character.charCodeAt(0)); } catch { throw new Error(`${field} is not valid base64`); }
  if (exact !== undefined && output.length !== exact) throw new Error(`${field} has the wrong exact width`);
  return output;
}

function pubkeyHex(bytes: Uint8Array, offset: number, field: string): string {
  const value = slice(bytes, offset, 32); requireNonzero(value, field); return new PublicKey(value).toBase58();
}

function idHex(bytes: Uint8Array, offset: number, field: string): string {
  const value = slice(bytes, offset, 32); requireNonzero(value, field); return hex(value);
}

function childRole(value: unknown, field: string): GeneralChildRoleV5 {
  if (value !== 'claims' && value !== 'custody') throw new Error(`${field} is not a General child role`);
  return value;
}

function lifecycleState(value: unknown, field: string): GeneralLifecycleStateV5 {
  const state = object(value, field);
  exactKeys(state, ['accountCoordinate', 'account', 'bump', 'isMaterialized'], field);
  if (typeof state.isMaterialized !== 'boolean') throw new Error(`${field}.isMaterialized is not boolean`);
  return Object.freeze({
    accountCoordinate: integer(state.accountCoordinate, `${field}.accountCoordinate`, 65_535),
    account: address(state.account, `${field}.account`),
    bump: integer(state.bump, `${field}.bump`, 255),
    isMaterialized: state.isMaterialized,
  });
}

function parseChildRoutes(value: unknown): ReadonlyArray<GeneralChildRouteV5> {
  if (!Array.isArray(value) || value.length > 32) throw new Error('General child routes are not a bounded array');
  return Object.freeze(value.map((raw, route) => {
    const row = object(raw, `child route ${route}`);
    exactKeys(row, ['route', 'role', 'accountStart', 'accountCount', 'receiptDependencies'], `child route ${route}`);
    const routeIndex = integer(row.route, `child route ${route}.route`, 65_535);
    const accountStart = integer(row.accountStart, `child route ${route}.accountStart`, 65_535);
    const accountCount = integer(row.accountCount, `child route ${route}.accountCount`, 65_535);
    if (routeIndex !== route || accountCount === 0 || accountStart + accountCount > 65_536) throw new Error(`child route ${route} has noncanonical order or account span`);
    if (!Array.isArray(row.receiptDependencies) || row.receiptDependencies.length > route) throw new Error(`child route ${route} has impossible receipt dependencies`);
    const receiptDependencies = Object.freeze(row.receiptDependencies.map((rawDependency, ordinal) => {
      const dependency = object(rawDependency, `child route ${route} receipt ${ordinal}`);
      exactKeys(dependency, ['producerRole', 'producerRoute', 'expectedReceiptBytes'], `child route ${route} receipt ${ordinal}`);
      const producerRoute = integer(dependency.producerRoute, `child route ${route} receipt ${ordinal}.producerRoute`, 65_535);
      const expectedReceiptBytes = integer(dependency.expectedReceiptBytes, `child route ${route} receipt ${ordinal}.expectedReceiptBytes`, 65_535);
      if (producerRoute >= route || expectedReceiptBytes === 0) throw new Error(`child route ${route} receipt ${ordinal} is not backward and exact-width`);
      return Object.freeze({ producerRole: childRole(dependency.producerRole, `child route ${route} receipt ${ordinal}.producerRole`), producerRoute, expectedReceiptBytes });
    }));
    return Object.freeze({ route: routeIndex, role: childRole(row.role, `child route ${route}.role`), accountStart, accountCount, receiptDependencies });
  }));
}

function validateActionRoutes(actionValue: GeneralSuccessorActionV5, routes: ReadonlyArray<GeneralChildRouteV5>, childStart: number): void {
  const roles: Readonly<Record<GeneralSuccessorActionV5, ReadonlyArray<GeneralChildRoleV5>>> = Object.freeze({
    consider: [], freeze: [], 'initialize-settlement': ['claims', 'custody', 'custody'],
    collect: ['claims', 'custody'], materialize: ['claims', 'custody'], distribute: ['claims', 'custody'],
    close: ['custody', 'claims', 'custody', 'custody'],
    'open-batch': [], 'place-order': [], 'cancel-order': [], 'close-batch': [],
    'submit-candidate': [], 'verify-candidate-row': [], 'release-order': [], 'close-candidate': [],
  });
  const expected = roles[actionValue];
  if (routes.length !== expected.length || routes.some((route, index) => route.role !== expected[index])) throw new Error('General child routes differ from the action-selected Claims/Custody order');
  let cursor = childStart;
  for (const route of routes) {
    if (route.accountStart !== cursor) throw new Error('General child route frames are not contiguous in authenticated logical order');
    cursor += route.accountCount;
  }
  routes.forEach((route, index) => {
    const expectedDependency = actionValue === 'initialize-settlement' && index === 2 ? 1 : actionValue === 'close' && index === 3 ? 2 : null;
    if (expectedDependency === null) {
      if (route.receiptDependencies.length !== 0) throw new Error('General child route carries an undeclared receipt dependency');
      return;
    }
    const dependency = route.receiptDependencies[0];
    if (route.receiptDependencies.length !== 1 || dependency?.producerRole !== 'custody' || dependency.producerRoute !== expectedDependency
        || dependency.expectedReceiptBytes !== Abi.GENERAL_CUSTODY_RECEIPT_BYTES_V1) throw new Error('General Custody receipt dependency order or exact width differs');
  });
}

export function decodeGeneralSuccessorPlanDocumentV5(input: string): GeneralSuccessorPlanDocumentV5 {
  if (new TextEncoder().encode(input).length > Abi.GENERAL_SUCCESSOR_PLAN_MAX_BYTES_V5) throw new Error('General operator plan exceeds the browser byte bound');
  let raw: unknown;
  try { raw = JSON.parse(input); } catch { throw new Error('General operator plan is not JSON'); }
  const value = object(raw, 'General operator plan');
  exactKeys(value, ['format', 'action', 'transactionBase64', 'observedSlot', 'outcomeCount', 'scratchPageCount', 'heapFrameBytes', 'tradingProgram', 'lookupTable', 'payer', 'requiredSigners', 'market', 'root', 'generation', 'releaseSet', 'rootPrestateDigest', 'familyRequestDigest', 'checkedManifestDigest', 'tradingArtifactRelease', 'generalArtifactRelease', 'productRecord', 'artifacts', 'lifecycle', 'childRoutes'], 'General operator plan');
  if (value.format !== 'dclutch/general-successor-plan/v5') throw new Error('General operator plan format is not V5');
  const selectedAction = action(value.action);
  const outcomeCount = integer(value.outcomeCount, 'outcomeCount');
  const scratchPageCount = integer(value.scratchPageCount, 'scratchPageCount');
  const heapFrameBytes = integer(value.heapFrameBytes, 'heapFrameBytes');
  if (outcomeCount === 0 || scratchPageCount === 0) throw new Error('General outcome and scratch-page counts must be nonzero');
  if (heapFrameBytes !== Abi.GENERAL_HOT_HEAP_FRAME_BYTES_V3) throw new Error('General heap frame differs from the measured canonical route resource');
  if (!Array.isArray(value.requiredSigners) || value.requiredSigners.length === 0 || value.requiredSigners.length > 32) throw new Error('General required signers are not a bounded nonempty array');
  const requiredSigners = Object.freeze(value.requiredSigners.map((entry, index) => address(entry, `requiredSigners[${index}]`)));
  if (new Set(requiredSigners).size !== requiredSigners.length) throw new Error('General required signers contain a duplicate');
  const payer = address(value.payer, 'payer');
  if (requiredSigners[0] !== payer) throw new Error('General fee payer is not the first required signer');
  const artifactsRaw = object(value.artifacts, 'artifacts');
  exactKeys(artifactsRaw, ARTIFACT_KEYS, 'artifacts');
  const artifacts = Object.freeze(Object.fromEntries(ARTIFACT_KEYS.map((key) => [key, identity(artifactsRaw[key], `artifacts.${key}`)])) as Record<(typeof ARTIFACT_KEYS)[number], string>);
  const lifecycleRaw = object(value.lifecycle, 'lifecycle');
  exactKeys(lifecycleRaw, ['primary', 'secondary', 'conditionalResult', 'terminalCoordinate', 'childAccountStart'], 'lifecycle');
  const primary = lifecycleState(lifecycleRaw.primary, 'lifecycle.primary');
  const secondary = lifecycleRaw.secondary === null ? null : lifecycleState(lifecycleRaw.secondary, 'lifecycle.secondary');
  const conditionalResult = lifecycleRaw.conditionalResult === null ? null : lifecycleState(lifecycleRaw.conditionalResult, 'lifecycle.conditionalResult');
  const terminalCoordinate = lifecycleRaw.terminalCoordinate === null ? null : integerText(lifecycleRaw.terminalCoordinate, 'lifecycle.terminalCoordinate');
  const hasSecondary = selectedAction === 'close' || selectedAction === 'place-order' || selectedAction === 'cancel-order' || selectedAction === 'verify-candidate-row';
  const hasConditionalResult = selectedAction === 'verify-candidate-row';
  const lifecycle = Object.freeze({
    primary,
    secondary,
    conditionalResult,
    terminalCoordinate,
    childAccountStart: integer(lifecycleRaw.childAccountStart, 'lifecycle.childAccountStart', 65_535),
  });
  if (primary.accountCoordinate !== Abi.GENERAL_PRIMARY_STATE_ACCOUNT_V3
      || hasSecondary !== (secondary !== null)
      || hasConditionalResult !== (conditionalResult !== null)
      || (secondary !== null && secondary.accountCoordinate !== (selectedAction === 'verify-candidate-row' ? Abi.GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3 : Abi.GENERAL_TERMINAL_STATE_ACCOUNT_V3))
      || (conditionalResult !== null && conditionalResult.accountCoordinate !== Abi.GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3)
      || (selectedAction === 'close-candidate' && lifecycle.childAccountStart !== Abi.GENERAL_CLOSE_CANDIDATE_CHILD_START_V3)
      || (selectedAction === 'close') !== (terminalCoordinate !== null)) throw new Error('General lifecycle shape differs from the selected action');
  const selectedStates = [primary, secondary, conditionalResult].filter((state): state is GeneralLifecycleStateV5 => state !== null);
  if (new Set(selectedStates.map((state) => state.account)).size !== selectedStates.length
      || new Set(selectedStates.map((state) => state.accountCoordinate)).size !== selectedStates.length
      || selectedStates.some((state) => state.accountCoordinate >= lifecycle.childAccountStart)) throw new Error('General lifecycle accounts alias or overlap child routes');
  const childRoutes = parseChildRoutes(value.childRoutes);
  validateActionRoutes(selectedAction, childRoutes, lifecycle.childAccountStart);
  return Object.freeze({
    format: value.format, action: selectedAction,
    transactionBase64: text(value.transactionBase64, 'transactionBase64', 4_096),
    observedSlot: integerText(value.observedSlot, 'observedSlot'), outcomeCount, scratchPageCount, heapFrameBytes,
    tradingProgram: address(value.tradingProgram, 'tradingProgram'), lookupTable: address(value.lookupTable, 'lookupTable'), payer,
    requiredSigners, market: address(value.market, 'market'), root: address(value.root, 'root'), generation: integerText(value.generation, 'generation'),
    releaseSet: identity(value.releaseSet, 'releaseSet'), rootPrestateDigest: identity(value.rootPrestateDigest, 'rootPrestateDigest'),
    familyRequestDigest: identity(value.familyRequestDigest, 'familyRequestDigest'), checkedManifestDigest: identity(value.checkedManifestDigest, 'checkedManifestDigest'),
    tradingArtifactRelease: identity(value.tradingArtifactRelease, 'tradingArtifactRelease'), generalArtifactRelease: identity(value.generalArtifactRelease, 'generalArtifactRelease'),
    productRecord: identity(value.productRecord, 'productRecord'), artifacts, lifecycle, childRoutes,
  });
}

export function decodeGeneralControllerRequestV3(bytes: Uint8Array): GeneralDecodedRequestV3 {
  if (bytes.length !== Abi.GENERAL_REQUEST_BYTES_V3) throw new Error('General controller request has the wrong exact width');
  const wire = same(slice(bytes, 0, 8), Abi.GENERAL_REQUEST_MAGIC_V2) && u16(bytes, 8) === 2 ? 'v2'
    : same(slice(bytes, 0, 8), Abi.GENERAL_REQUEST_MAGIC_V3) && u16(bytes, 8) === 3 ? 'v3' : null;
  if (wire === null) throw new Error('General controller request is neither exact V2 nor exact V3');
  requireZero(bytes, 12, 4, 'General request');
  if (wire === 'v2') requireZero(bytes, Abi.GENERAL_REQUEST_RESULT_BUMP_OFFSET_V3, 1, 'General V2 request');
  const tag = bytes[Abi.GENERAL_REQUEST_ACTION_OFFSET_V3];
  const selectedAction = ACTIONS[tag];
  if (selectedAction === undefined || (wire === 'v2' && tag > Abi.ACTION_CLOSE_V2)) throw new Error('General controller request has an unknown action for its wire generation');
  const subject = slice(bytes, Abi.GENERAL_REQUEST_SUBJECT_ID_OFFSET_V3, 32);
  const subjectId = isZero(subject) ? null : hex(subject);
  const pageIndex = readU32(bytes, Abi.GENERAL_REQUEST_PAGE_INDEX_OFFSET_V3);
  const executionIndex = bytes[Abi.GENERAL_REQUEST_EXECUTION_INDEX_OFFSET_V3];
  const manifestOrderIndex = bytes[Abi.GENERAL_REQUEST_MANIFEST_ORDER_OFFSET_V3];
  const primaryStateBump = bytes[Abi.GENERAL_REQUEST_PRIMARY_BUMP_OFFSET_V3];
  const secondaryStateBump = bytes[Abi.GENERAL_REQUEST_SECONDARY_BUMP_OFFSET_V3];
  const resultStateBump = bytes[Abi.GENERAL_REQUEST_RESULT_BUMP_OFFSET_V3];
  if (wire === 'v2' && selectedAction !== 'close' && secondaryStateBump !== 0) throw new Error('nonterminal General V2 request carries a secondary-state bump');
  if (selectedAction !== 'collect' && selectedAction !== 'distribute' && manifestOrderIndex !== 0) throw new Error('nonrow General request carries a manifest ordinal');
  const expectedRevision = u64(bytes, Abi.GENERAL_REQUEST_EXPECTED_REVISION_OFFSET_V3);
  const canonical = selectedAction === 'freeze' ? subjectId === null && pageIndex === 0 && executionIndex === 0 && secondaryStateBump === 0 && resultStateBump === 0
    : selectedAction === 'consider' ? subjectId !== null && executionIndex === 0 && resultStateBump === 0
      : selectedAction === 'collect' || selectedAction === 'distribute' ? subjectId !== null && secondaryStateBump === 0 && resultStateBump === 0
        : selectedAction === 'initialize-settlement' || selectedAction === 'materialize' ? subjectId !== null && pageIndex === 0 && executionIndex === 0 && secondaryStateBump === 0 && resultStateBump === 0
          : selectedAction === 'close' ? subjectId !== null && pageIndex === 0 && executionIndex === 0 && resultStateBump === 0
            : selectedAction === 'open-batch' || selectedAction === 'close-batch' ? subjectId !== null && pageIndex === 0 && executionIndex === 0 && secondaryStateBump === 0 && resultStateBump === 0
              : selectedAction === 'place-order' || selectedAction === 'cancel-order' ? subjectId !== null && expectedRevision === 0n && pageIndex === 0 && executionIndex === 0 && resultStateBump === 0
                : selectedAction === 'submit-candidate' || selectedAction === 'release-order' || selectedAction === 'close-candidate' ? subjectId !== null && expectedRevision === 0n && pageIndex === 0 && executionIndex === 0 && secondaryStateBump === 0 && resultStateBump === 0
                  : selectedAction === 'verify-candidate-row' && subjectId !== null;
  if (!canonical) throw new Error('General request cursor is noncanonical for its action');
  return Object.freeze({ wire, action: selectedAction, expectedRevision, subjectId, pageIndex, executionIndex, manifestOrderIndex, primaryStateBump, secondaryStateBump, resultStateBump, bytes: new Uint8Array(bytes) });
}

function decodeEnvelope(bytes: Uint8Array): GeneralHotEnvelopeV3 {
  if (bytes.length !== Abi.GENERAL_HOT_ENVELOPE_BYTES_V3 || !same(slice(bytes, 0, 8), Abi.GENERAL_HOT_MAGIC_V3)
      || u16(bytes, 8) !== Abi.GENERAL_HOT_VERSION_V3 || u16(bytes, 10) !== Abi.GENERAL_HOT_PROFILE_V3
      || readU32(bytes, Abi.GENERAL_ENVELOPE_REQUEST_BYTES_OFFSET_V3) !== Abi.GENERAL_REQUEST_BYTES_V3) throw new Error('General Hot envelope is not exact V3 with a 64-byte family request');
  return Object.freeze({
    releaseSet: idHex(bytes, Abi.GENERAL_ENVELOPE_RELEASE_SET_OFFSET_V3, 'Hot release set'),
    market: pubkeyHex(bytes, Abi.GENERAL_ENVELOPE_MARKET_OFFSET_V3, 'Hot Market'),
    generation: u64(bytes, Abi.GENERAL_ENVELOPE_GENERATION_OFFSET_V3),
    rootPrestateDigest: idHex(bytes, Abi.GENERAL_ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET_V3, 'Hot root prestate digest'),
    bumpHints: Object.freeze([...slice(bytes, Abi.GENERAL_ENVELOPE_BUMP_HINTS_OFFSET_V3, Abi.GENERAL_ENVELOPE_BUMP_HINT_COUNT_V3)]),
  });
}

export async function inspectGeneralSuccessorPlanV5(plan: GeneralSuccessorPlanDocumentV5): Promise<GeneralPlanInspectionV5> {
  const transaction = await inspectUnsignedTransactionV1(plan.transactionBase64);
  const message = transaction.transaction.message as unknown as MessageView;
  if (message.compiledInstructions.length !== 2) throw new Error('General plan must contain exactly one heap declaration followed by one Hot instruction');
  if (message.addressTableLookups.length !== 1 || message.addressTableLookups[0]?.accountKey.toBase58() !== plan.lookupTable) throw new Error('General plan does not use its one exact canonical lookup table');
  const heapCompiled = message.compiledInstructions[0];
  const compiled = message.compiledInstructions[1];
  if (heapCompiled === undefined || compiled === undefined) throw new Error('General instruction pair is incomplete');
  const heapProgram = message.staticAccountKeys[heapCompiled.programIdIndex];
  const expectedHeap = ComputeBudgetProgram.requestHeapFrame({ bytes: Abi.GENERAL_HOT_HEAP_FRAME_BYTES_V3 });
  if (heapProgram === undefined || !heapProgram.equals(expectedHeap.programId)
      || heapCompiled.accountKeyIndexes.length !== 0
      || !same(new Uint8Array(heapCompiled.data), new Uint8Array(expectedHeap.data))) throw new Error('General heap declaration or instruction order was substituted');
  const minimumAccounts = Abi.GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3 + 8 + plan.scratchPageCount;
  if (compiled.accountKeyIndexes.length < minimumAccounts) throw new Error(`General transaction is shorter than the ${Abi.GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3}-coordinate Hot frame + admitted strategy + canonical scratch geometry`);
  const program = message.staticAccountKeys[compiled.programIdIndex];
  if (program === undefined || program.toBase58() !== plan.tradingProgram) throw new Error('General transaction invokes another Trading program');
  const signerCount = message.header.numRequiredSignatures;
  const signers = message.staticAccountKeys.slice(0, signerCount).map((key) => key.toBase58());
  if (signers.length !== plan.requiredSigners.length || signers.some((key, index) => key !== plan.requiredSigners[index])) throw new Error('General transaction signer order differs from the operator report');
  const instruction = new Uint8Array(compiled.data);
  if (instruction.length !== Abi.GENERAL_HOT_ENVELOPE_BYTES_V3 + Abi.GENERAL_REQUEST_BYTES_V3) throw new Error('General Hot instruction has another exact width');
  const envelope = decodeEnvelope(slice(instruction, 0, Abi.GENERAL_HOT_ENVELOPE_BYTES_V3));
  const request = decodeGeneralControllerRequestV3(slice(instruction, Abi.GENERAL_HOT_ENVELOPE_BYTES_V3, Abi.GENERAL_REQUEST_BYTES_V3));
  const requestDigest = hex(await sha256(request.bytes));
  if (request.action !== plan.action || envelope.releaseSet !== plan.releaseSet || envelope.market !== plan.market || envelope.generation !== plan.generation
      || envelope.rootPrestateDigest !== plan.rootPrestateDigest || requestDigest !== plan.familyRequestDigest
      || request.primaryStateBump !== plan.lifecycle.primary.bump
      || (plan.lifecycle.secondary?.bump ?? 0) !== request.secondaryStateBump
      || (plan.lifecycle.conditionalResult?.bump ?? 0) !== request.resultStateBump) throw new Error('General operator report differs from the exact transaction request or Hot envelope');
  if (plan.action === 'close') {
    if (request.expectedRevision === MAX_U64 || plan.lifecycle.terminalCoordinate !== request.expectedRevision + 1n) throw new Error('General Close terminal coordinate is not the revision successor');
  }
  return Object.freeze({ plan, transaction, envelope, request });
}

function decodeSelection(bytes: Uint8Array): GeneralSelectionStatusV2 {
  if (bytes.length !== Abi.GENERAL_SELECTION_BYTES_V2
      || !same(slice(bytes, Abi.GENERAL_SELECTION_MAGIC_OFFSET_V2, Abi.GENERAL_SELECTION_MAGIC_V2.length), Abi.GENERAL_SELECTION_MAGIC_V2)
      || u16(bytes, Abi.GENERAL_SELECTION_VERSION_OFFSET_V2) !== Abi.GENERAL_SELECTION_VERSION_V2
      || bytes[Abi.GENERAL_SELECTION_PHASE_OFFSET_V2 + Uint8Array.BYTES_PER_ELEMENT] !== 0) throw new Error('General selection body is not exact V2');
  const phaseByte = bytes[Abi.GENERAL_SELECTION_PHASE_OFFSET_V2];
  const phase = phaseByte === 1 ? 'open' : phaseByte === 2 ? 'frozen' : null;
  if (phase === null) throw new Error('General selection phase is unknown');
  const output = Object.freeze({
    kind: 'selection' as const, phase, outcomeCount: readU32(bytes, Abi.GENERAL_SELECTION_OUTCOME_COUNT_OFFSET_V2),
    revision: u64(bytes, Abi.GENERAL_SELECTION_REVISION_OFFSET_V2), submittedCount: readU32(bytes, Abi.GENERAL_SELECTION_SUBMITTED_COUNT_OFFSET_V2),
    bestCandidateCoordinate: readU32(bytes, Abi.GENERAL_SELECTION_BEST_COORDINATE_OFFSET_V2), bestVerifiedRevision: u64(bytes, Abi.GENERAL_SELECTION_VERIFIED_REVISION_OFFSET_V2),
    priceScale: u64(bytes, Abi.GENERAL_SELECTION_PRICE_SCALE_OFFSET_V2), productId: idHex(bytes, Abi.GENERAL_SELECTION_PRODUCT_ID_OFFSET_V2, 'selection Product'),
    batchId: idHex(bytes, Abi.GENERAL_SELECTION_BATCH_ID_OFFSET_V2, 'selection Batch'), policyId: idHex(bytes, Abi.GENERAL_SELECTION_POLICY_ID_OFFSET_V2, 'selection policy'),
    bestCandidateId: idHex(bytes, Abi.GENERAL_SELECTION_BEST_CANDIDATE_OFFSET_V2, 'best valid submitted candidate'),
    bestVerifiedDigest: idHex(bytes, Abi.GENERAL_SELECTION_VERIFIED_DIGEST_OFFSET_V2, 'selected verified candidate'),
    bestFilledLots: u64(bytes, Abi.GENERAL_SELECTION_FILLED_LOTS_OFFSET_V2), bestQuoteSurplus: u64(bytes, Abi.GENERAL_SELECTION_QUOTE_SURPLUS_OFFSET_V2),
  });
  if (output.outcomeCount === 0 || output.revision === 0n || output.submittedCount === 0 || output.bestCandidateCoordinate === 0 || output.bestVerifiedRevision === 0n || output.priceScale === 0n) throw new Error('General selection has a zero required coordinate');
  return output;
}

function decodeSettlement(bytes: Uint8Array): GeneralSettlementStatusV2 {
  if (bytes.length < Abi.GENERAL_SETTLEMENT_HEADER_BYTES_V2
      || !same(slice(bytes, Abi.GENERAL_SETTLEMENT_MAGIC_OFFSET_V2, Abi.GENERAL_SETTLEMENT_MAGIC_V2.length), Abi.GENERAL_SETTLEMENT_MAGIC_V2)
      || u16(bytes, Abi.GENERAL_SETTLEMENT_VERSION_OFFSET_V2) !== Abi.GENERAL_SETTLEMENT_VERSION_V2
      || bytes[Abi.GENERAL_SETTLEMENT_PHASE_OFFSET_V2 + Uint8Array.BYTES_PER_ELEMENT] !== 0) throw new Error('General settlement body is not exact V2');
  const phaseByTag = new Map<number, GeneralSettlementPhaseV2>([[Abi.GENERAL_PHASE_COLLECTING_V2, 'collecting'], [Abi.GENERAL_PHASE_MATERIALIZING_V2, 'materializing'], [Abi.GENERAL_PHASE_DISTRIBUTING_V2, 'distributing'], [Abi.GENERAL_PHASE_READY_TO_CLOSE_V2, 'ready-to-close'], [Abi.GENERAL_PHASE_TERMINAL_V2, 'terminal']]);
  const phase = phaseByTag.get(bytes[Abi.GENERAL_SETTLEMENT_PHASE_OFFSET_V2]);
  if (phase === undefined) throw new Error('General settlement phase is unknown');
  const outcomeCount = readU32(bytes, Abi.GENERAL_SETTLEMENT_OUTCOME_COUNT_OFFSET_V2);
  if (outcomeCount === 0 || bytes.length !== Abi.GENERAL_SETTLEMENT_HEADER_BYTES_V2 + outcomeCount * Abi.GENERAL_SETTLEMENT_INVENTORY_STRIDE_V2) throw new Error('General settlement runtime width differs from Product N');
  const orderCount = readU32(bytes, Abi.GENERAL_SETTLEMENT_ORDER_COUNT_OFFSET_V2); const nextOrder = readU32(bytes, Abi.GENERAL_SETTLEMENT_NEXT_ORDER_OFFSET_V2);
  const revision = u64(bytes, Abi.GENERAL_SETTLEMENT_REVISION_OFFSET_V2); const terminalCoordinate = u64(bytes, Abi.GENERAL_SETTLEMENT_TERMINAL_OFFSET_V2);
  const streaming = phase === 'collecting' || phase === 'distributing';
  if (orderCount === 0 || revision === 0n || (streaming ? nextOrder >= orderCount : nextOrder !== orderCount) || (phase === 'terminal') !== (terminalCoordinate !== 0n)) throw new Error('General settlement cursor is noncanonical');
  return Object.freeze({
    kind: 'settlement', phase, outcomeCount, orderCount, nextOrder, revision,
    candidateId: idHex(bytes, Abi.GENERAL_SETTLEMENT_CANDIDATE_ID_OFFSET_V2, 'settlement candidate'),
    quoteInventory: u64(bytes, Abi.GENERAL_SETTLEMENT_QUOTE_INVENTORY_OFFSET_V2), completeSetQuantity: u64(bytes, Abi.GENERAL_SETTLEMENT_COMPLETE_SET_OFFSET_V2), terminalCoordinate,
    inventory: Object.freeze(Array.from({ length: outcomeCount }, (_, index) => u64(bytes, Abi.GENERAL_SETTLEMENT_INVENTORY_OFFSET_V2 + index * Abi.GENERAL_SETTLEMENT_INVENTORY_STRIDE_V2))),
  });
}

function decodeBatch(bytes: Uint8Array): GeneralBatchStatusV1 {
  if (bytes.length !== Abi.GENERAL_BATCH_BYTES_V1
      || !same(slice(bytes, Abi.GENERAL_BATCH_MAGIC_OFFSET_V1, Abi.GENERAL_BATCH_MAGIC_V1.length), Abi.GENERAL_BATCH_MAGIC_V1)
      || u16(bytes, Abi.GENERAL_BATCH_VERSION_OFFSET_V1) !== Abi.GENERAL_BATCH_VERSION_V1
      || bytes[Abi.GENERAL_BATCH_PHASE_OFFSET_V1] !== Abi.GENERAL_BATCH_PHASE_V1
      || bytes[Abi.GENERAL_BATCH_PHASE_OFFSET_V1 + Uint8Array.BYTES_PER_ELEMENT] !== 0) throw new Error('General batch body is not exact V1');
  const afterMaxOrders = Abi.GENERAL_BATCH_MAX_ORDERS_OFFSET_V1 + Uint32Array.BYTES_PER_ELEMENT;
  const afterStatus = Abi.GENERAL_BATCH_STATUS_OFFSET_V1 + Uint8Array.BYTES_PER_ELEMENT;
  const afterCancelledCount = Abi.GENERAL_BATCH_CANCELLED_COUNT_OFFSET_V1 + Uint32Array.BYTES_PER_ELEMENT;
  requireZero(bytes, afterMaxOrders, Abi.GENERAL_BATCH_SETTLEMENT_CLOSE_SLOT_OFFSET_V1 - afterMaxOrders, 'General batch');
  requireZero(bytes, afterStatus, Abi.GENERAL_BATCH_ORDER_COUNT_OFFSET_V1 - afterStatus, 'General batch');
  requireZero(bytes, afterCancelledCount, Abi.GENERAL_BATCH_BYTES_V1 - afterCancelledCount, 'General batch');
  const phaseByte = bytes[Abi.GENERAL_BATCH_STATUS_OFFSET_V1];
  const phase = phaseByte === Abi.GENERAL_BATCH_STATUS_COLLECTING_V1 ? 'collecting'
    : phaseByte === Abi.GENERAL_BATCH_STATUS_CLOSED_V1 ? 'closed' : null;
  if (phase === null) throw new Error('General batch status is unknown');
  const value = Object.freeze({
    kind: 'batch' as const, phase,
    outcomeCount: readU32(bytes, Abi.GENERAL_BATCH_OUTCOME_COUNT_OFFSET_V1),
    sequence: u64(bytes, Abi.GENERAL_BATCH_SEQUENCE_OFFSET_V1), generation: u64(bytes, Abi.GENERAL_BATCH_GENERATION_OFFSET_V1),
    market: pubkeyHex(bytes, Abi.GENERAL_BATCH_MARKET_OFFSET_V1, 'batch Market'),
    productId: idHex(bytes, Abi.GENERAL_BATCH_PRODUCT_ID_OFFSET_V1, 'batch Product'), configId: idHex(bytes, Abi.GENERAL_BATCH_CONFIG_ID_OFFSET_V1, 'batch config'),
    priceScale: u64(bytes, Abi.GENERAL_BATCH_PRICE_SCALE_OFFSET_V1), collectionCloseSlot: u64(bytes, Abi.GENERAL_BATCH_COLLECTION_CLOSE_SLOT_OFFSET_V1),
    maxOrders: readU32(bytes, Abi.GENERAL_BATCH_MAX_ORDERS_OFFSET_V1), settlementCloseSlot: u64(bytes, Abi.GENERAL_BATCH_SETTLEMENT_CLOSE_SLOT_OFFSET_V1),
    orderCount: readU32(bytes, Abi.GENERAL_BATCH_ORDER_COUNT_OFFSET_V1), openedRootRevision: u64(bytes, Abi.GENERAL_BATCH_OPENED_ROOT_REVISION_OFFSET_V1),
    closedRootRevision: u64(bytes, Abi.GENERAL_BATCH_CLOSED_ROOT_REVISION_OFFSET_V1), committedQuoteReserve: u64(bytes, Abi.GENERAL_BATCH_COMMITTED_QUOTE_RESERVE_OFFSET_V1),
    cancelledCount: readU32(bytes, Abi.GENERAL_BATCH_CANCELLED_COUNT_OFFSET_V1),
  });
  if (value.outcomeCount === 0 || value.generation === 0n || value.priceScale === 0n || value.maxOrders === 0 || value.openedRootRevision === 0n
      || value.settlementCloseSlot <= value.collectionCloseSlot || value.orderCount > value.maxOrders || value.cancelledCount > value.orderCount
      || (phase === 'collecting' ? value.closedRootRevision !== 0n : value.closedRootRevision <= value.openedRootRevision)) throw new Error('General batch carries noncanonical opening or lifecycle facts');
  return value;
}

function decodeOrder(bytes: Uint8Array): GeneralOrderStatusV1 {
  if (bytes.length < Abi.GENERAL_ORDER_ROW_BASE_V1
      || !same(slice(bytes, Abi.GENERAL_ORDER_MAGIC_OFFSET_V1, Abi.GENERAL_ORDER_MAGIC_V1.length), Abi.GENERAL_ORDER_MAGIC_V1)
      || u16(bytes, Abi.GENERAL_ORDER_VERSION_OFFSET_V1) !== Abi.GENERAL_ORDER_VERSION_V1
      || bytes[Abi.GENERAL_ORDER_PHASE_OFFSET_V1] !== Abi.GENERAL_ORDER_PHASE_V1
      || bytes[Abi.GENERAL_ORDER_PHASE_OFFSET_V1 + Uint8Array.BYTES_PER_ELEMENT] !== 0) throw new Error('General order body is not exact V1');
  const afterNonce = Abi.GENERAL_ORDER_NONCE_OFFSET_V1 + BigUint64Array.BYTES_PER_ELEMENT;
  const afterStatePhase = Abi.GENERAL_ORDER_STATE_PHASE_OFFSET_V1 + Uint8Array.BYTES_PER_ELEMENT;
  const afterReleasedSlot = Abi.GENERAL_ORDER_STATE_RELEASED_SLOT_OFFSET_V1 + BigUint64Array.BYTES_PER_ELEMENT;
  requireZero(bytes, afterNonce, Abi.GENERAL_ORDER_OWNER_ID_OFFSET_V1 - afterNonce, 'General order');
  requireZero(bytes, afterStatePhase, Abi.GENERAL_ORDER_STATE_ADMITTED_SLOT_OFFSET_V1 - afterStatePhase, 'General order');
  requireZero(bytes, afterReleasedSlot, Abi.GENERAL_ORDER_STATE_OFFSET_V1 + Abi.GENERAL_ORDER_STATE_BYTES_V1 - afterReleasedSlot, 'General order');
  const outcomeCount = readU32(bytes, Abi.GENERAL_ORDER_OUTCOME_COUNT_OFFSET_V1);
  if (outcomeCount === 0 || bytes.length !== Abi.GENERAL_ORDER_ROW_BASE_V1 + outcomeCount * Abi.GENERAL_ORDER_ROW_STRIDE_V1) throw new Error('General order runtime width differs from Product N');
  const phaseByte = bytes[Abi.GENERAL_ORDER_STATE_PHASE_OFFSET_V1];
  const phase = phaseByte === Abi.GENERAL_ORDER_STATE_PLACED_V1 ? 'placed'
    : phaseByte === Abi.GENERAL_ORDER_STATE_CANCELLED_V1 ? 'cancelled'
      : phaseByte === Abi.GENERAL_ORDER_STATE_RELEASED_V1 ? 'released' : null;
  if (phase === null) throw new Error('General order state is unknown');
  const receivePerLot = Object.freeze(Array.from({ length: outcomeCount }, (_, index) => u64(bytes, Abi.GENERAL_ORDER_ROW_BASE_V1 + index * Abi.GENERAL_ORDER_ROW_STRIDE_V1)));
  const deliverPerLot = Object.freeze(Array.from({ length: outcomeCount }, (_, index) => u64(bytes, Abi.GENERAL_ORDER_ROW_BASE_V1 + index * Abi.GENERAL_ORDER_ROW_STRIDE_V1 + BigUint64Array.BYTES_PER_ELEMENT)));
  if (!receivePerLot.some((quantity, index) => quantity !== 0n || deliverPerLot[index] !== 0n)) throw new Error('General order has no claim movement');
  const value = Object.freeze({
    kind: 'order' as const, phase, outcomeCount,
    nonce: u64(bytes, Abi.GENERAL_ORDER_NONCE_OFFSET_V1), owner: pubkeyHex(bytes, Abi.GENERAL_ORDER_OWNER_ID_OFFSET_V1, 'order owner'),
    market: pubkeyHex(bytes, Abi.GENERAL_ORDER_MARKET_OFFSET_V1, 'order Market'), batchId: idHex(bytes, Abi.GENERAL_ORDER_BATCH_ID_OFFSET_V1, 'order Batch'),
    generation: u64(bytes, Abi.GENERAL_ORDER_GENERATION_OFFSET_V1), maxLots: u64(bytes, Abi.GENERAL_ORDER_MAX_LOTS_OFFSET_V1),
    maxQuoteDebitPerLot: u64(bytes, Abi.GENERAL_ORDER_MAX_QUOTE_DEBIT_PER_LOT_OFFSET_V1), validUntilSlot: u64(bytes, Abi.GENERAL_ORDER_VALID_UNTIL_SLOT_OFFSET_V1),
    admittedSlot: u64(bytes, Abi.GENERAL_ORDER_STATE_ADMITTED_SLOT_OFFSET_V1), releasedSlot: u64(bytes, Abi.GENERAL_ORDER_STATE_RELEASED_SLOT_OFFSET_V1), receivePerLot, deliverPerLot,
  });
  if (value.generation === 0n || value.maxLots === 0n || (phase === 'placed' ? value.releasedSlot !== 0n : value.releasedSlot < value.admittedSlot)) throw new Error('General order carries noncanonical immutable or lifecycle facts');
  return value;
}

/** Hostile-decode one exact General candidate submission body. */
export function decodeGeneralCandidateV1(bytes: Uint8Array): GeneralCandidateStatusV1 {
  if (bytes.length !== Abi.GENERAL_SUBMISSION_BYTES_V1
      || !same(slice(bytes, Abi.GENERAL_SUBMISSION_MAGIC_OFFSET_V1, Abi.GENERAL_SUBMISSION_MAGIC_V1.length), Abi.GENERAL_SUBMISSION_MAGIC_V1)
      || u16(bytes, Abi.GENERAL_SUBMISSION_VERSION_OFFSET_V1) !== Abi.GENERAL_SUBMISSION_VERSION_V1
      || bytes[Abi.GENERAL_SUBMISSION_PHASE_OFFSET_V1] !== Abi.GENERAL_SUBMISSION_PHASE_V1) throw new Error('General candidate submission is not exact V1');
  requireZero(bytes, Abi.GENERAL_SUBMISSION_HEADER_RESERVED_OFFSET_V1, 1, 'General candidate header');
  requireZero(bytes, Abi.GENERAL_SUBMISSION_STATUS_RESERVED_OFFSET_V1, 3, 'General candidate status');
  requireZero(bytes, Abi.GENERAL_SUBMISSION_ROW_RESERVED_OFFSET_V1, 4, 'General candidate row count');
  requireZero(bytes, Abi.GENERAL_SUBMISSION_TAIL_RESERVED_OFFSET_V1, 16, 'General candidate tail');
  const status = bytes[Abi.GENERAL_SUBMISSION_STATUS_OFFSET_V1];
  const phase = status === Abi.GENERAL_SUBMISSION_STATUS_SUBMITTED_V1 ? 'submitted'
    : status === Abi.GENERAL_SUBMISSION_STATUS_VERIFIED_V1 ? 'verified'
      : status === Abi.GENERAL_SUBMISSION_STATUS_CONSIDERED_V1 ? 'considered' : null;
  if (phase === null) throw new Error('General candidate has an unknown lifecycle status');
  const outcomeCount = readU32(bytes, Abi.GENERAL_SUBMISSION_OUTCOME_COUNT_OFFSET_V1);
  const pageCount = readU32(bytes, Abi.GENERAL_SUBMISSION_PAGE_COUNT_OFFSET_V1);
  const rowCount = readU32(bytes, Abi.GENERAL_SUBMISSION_ROW_COUNT_OFFSET_V1);
  const pageRevision = u64(bytes, Abi.GENERAL_SUBMISSION_PAGE_REVISION_OFFSET_V1);
  const rewardRateLamports = u64(bytes, Abi.GENERAL_SUBMISSION_REWARD_RATE_OFFSET_V1);
  const verificationRemaining = u64(bytes, Abi.GENERAL_SUBMISSION_VERIFICATION_REMAINING_OFFSET_V1);
  const cleanupRemaining = u64(bytes, Abi.GENERAL_SUBMISSION_CLEANUP_REMAINING_OFFSET_V1);
  const verifiedBytes = slice(bytes, Abi.GENERAL_SUBMISSION_VERIFIED_DIGEST_OFFSET_V1, 32);
  const verifiedRevision = u64(bytes, Abi.GENERAL_SUBMISSION_VERIFIED_REVISION_OFFSET_V1);
  const verificationCapacity = (BigInt(rowCount) + 1n) * rewardRateLamports;
  if (outcomeCount === 0 || pageCount === 0 || pageRevision === 0n || rowCount < pageCount || rewardRateLamports === 0n
      || verificationCapacity > MAX_U64 || verificationRemaining > verificationCapacity || cleanupRemaining > rewardRateLamports
      || (phase === 'submitted' ? !isZero(verifiedBytes) || verifiedRevision !== 0n : isZero(verifiedBytes) || verifiedRevision === 0n)) {
    throw new Error('General candidate carries noncanonical work or lifecycle facts');
  }
  return Object.freeze({
    kind: 'candidate' as const, phase, outcomeCount, pageCount, pageRevision,
    candidateId: idHex(bytes, Abi.GENERAL_SUBMISSION_CANDIDATE_ID_OFFSET_V1, 'candidate identity'),
    batchId: idHex(bytes, Abi.GENERAL_SUBMISSION_BATCH_ID_OFFSET_V1, 'candidate Batch'),
    solver: pubkeyHex(bytes, Abi.GENERAL_SUBMISSION_SOLVER_ID_OFFSET_V1, 'candidate solver'),
    verifiedDigest: isZero(verifiedBytes) ? null : hex(verifiedBytes),
    submittedSlot: u64(bytes, Abi.GENERAL_SUBMISSION_SUBMITTED_SLOT_OFFSET_V1), verifiedRevision, rowCount,
    rewardRateLamports, verificationRemaining, cleanupRemaining,
  });
}

function verifierTail(bytes: Uint8Array, count: number, tail: number): ReadonlyArray<bigint> {
  const start = Abi.GENERAL_VERIFIER_TAILS_BASE_OFFSET_V2 + tail * count * Abi.GENERAL_VERIFIER_TAIL_ITEM_STRIDE_V2;
  return Object.freeze(Array.from({ length: count }, (_, index) => u64(bytes, start + index * Abi.GENERAL_VERIFIER_TAIL_ITEM_STRIDE_V2)));
}

/** Hostile-decode one exact runtime-width candidate verifier body. */
export function decodeGeneralVerifierV2(bytes: Uint8Array): GeneralVerifierStatusV2 {
  if (bytes.length < Abi.GENERAL_VERIFIER_HEADER_BYTES_V2
      || !same(slice(bytes, Abi.GENERAL_VERIFIER_MAGIC_OFFSET_V2, Abi.GENERAL_VERIFIER_MAGIC_V2.length), Abi.GENERAL_VERIFIER_MAGIC_V2)
      || u16(bytes, Abi.GENERAL_VERIFIER_VERSION_OFFSET_V2) !== Abi.GENERAL_VERIFIER_VERSION_V2) throw new Error('General candidate verifier is not exact V2');
  const afterCurrentOrderFlag = Abi.GENERAL_VERIFIER_HAS_CURRENT_ORDER_OFFSET_V2 + Uint8Array.BYTES_PER_ELEMENT;
  const afterCandidateCoordinate = Abi.GENERAL_VERIFIER_CANDIDATE_COORDINATE_OFFSET_V2 + Uint32Array.BYTES_PER_ELEMENT;
  const afterSourceExecutionIndex = Abi.GENERAL_VERIFIER_CURRENT_SOURCE_EXECUTION_INDEX_OFFSET_V2 + Uint32Array.BYTES_PER_ELEMENT;
  requireZero(bytes, afterCurrentOrderFlag, Abi.GENERAL_VERIFIER_OUTCOME_COUNT_OFFSET_V2 - afterCurrentOrderFlag, 'General verifier header');
  requireZero(bytes, afterCandidateCoordinate, Abi.GENERAL_VERIFIER_CANDIDATE_ID_OFFSET_V2 - afterCandidateCoordinate, 'General verifier header');
  requireZero(bytes, afterSourceExecutionIndex, Abi.GENERAL_VERIFIER_HEADER_BYTES_V2 - afterSourceExecutionIndex, 'General verifier header');
  const hasCurrentOrder = bytes[Abi.GENERAL_VERIFIER_HAS_CURRENT_ORDER_OFFSET_V2];
  if (hasCurrentOrder !== 0 && hasCurrentOrder !== 1) throw new Error('General verifier current-order flag is noncanonical');
  const outcomeCount = readU32(bytes, Abi.GENERAL_VERIFIER_OUTCOME_COUNT_OFFSET_V2);
  if (outcomeCount === 0 || bytes.length !== Abi.GENERAL_VERIFIER_HEADER_BYTES_V2 + outcomeCount * 40) throw new Error('General verifier has the wrong runtime width');
  const pageCount = readU32(bytes, Abi.GENERAL_VERIFIER_PAGE_COUNT_OFFSET_V2);
  const nextPageIndex = readU32(bytes, Abi.GENERAL_VERIFIER_NEXT_PAGE_INDEX_OFFSET_V2);
  const nextRowIndex = readU32(bytes, Abi.GENERAL_VERIFIER_NEXT_ROW_INDEX_OFFSET_V2);
  const orderCount = readU32(bytes, Abi.GENERAL_VERIFIER_ORDER_COUNT_OFFSET_V2);
  const revision = u64(bytes, Abi.GENERAL_VERIFIER_REVISION_OFFSET_V2);
  const candidateCoordinate = readU32(bytes, Abi.GENERAL_VERIFIER_CANDIDATE_COORDINATE_OFFSET_V2);
  const priceScale = u64(bytes, Abi.GENERAL_VERIFIER_PRICE_SCALE_OFFSET_V2);
  const filledLots = u64(bytes, Abi.GENERAL_VERIFIER_FILLED_LOTS_OFFSET_V2);
  const quoteDebit = u64(bytes, Abi.GENERAL_VERIFIER_QUOTE_DEBIT_OFFSET_V2);
  const quoteCredit = u64(bytes, Abi.GENERAL_VERIFIER_QUOTE_CREDIT_OFFSET_V2);
  const prices = verifierTail(bytes, outcomeCount, Abi.GENERAL_VERIFIER_PRICES_TAIL_V2);
  const receivePerLot = verifierTail(bytes, outcomeCount, Abi.GENERAL_VERIFIER_CURRENT_RECEIVE_TAIL_V2);
  const deliverPerLot = verifierTail(bytes, outcomeCount, Abi.GENERAL_VERIFIER_CURRENT_DELIVER_TAIL_V2);
  const claimInputs = verifierTail(bytes, outcomeCount, Abi.GENERAL_VERIFIER_CLAIM_INPUTS_TAIL_V2);
  const claimOutputs = verifierTail(bytes, outcomeCount, Abi.GENERAL_VERIFIER_CLAIM_OUTPUTS_TAIL_V2);
  const initial = revision === 0n && nextPageIndex === 0 && nextRowIndex === 0 && orderCount === 0 && filledLots === 0n && quoteDebit === 0n && quoteCredit === 0n
    && hasCurrentOrder === 0 && claimInputs.every((value) => value === 0n) && claimOutputs.every((value) => value === 0n);
  if (pageCount === 0 || candidateCoordinate === 0 || priceScale === 0n || prices.reduce((sum, value) => sum + value, 0n) !== priceScale
      || nextPageIndex > pageCount || (nextPageIndex === pageCount && nextRowIndex !== 0) || (revision === 0n) !== initial || (!initial && orderCount === 0)) {
    throw new Error('General verifier carries a noncanonical cursor');
  }
  let currentOrder: GeneralVerifierCurrentOrderV2 | null = null;
  if (hasCurrentOrder === 1) {
    const maxLots = u64(bytes, Abi.GENERAL_VERIFIER_CURRENT_MAX_LOTS_OFFSET_V2);
    const lots = u64(bytes, Abi.GENERAL_VERIFIER_CURRENT_LOTS_OFFSET_V2);
    const sourcePageIndex = readU32(bytes, Abi.GENERAL_VERIFIER_CURRENT_SOURCE_PAGE_INDEX_OFFSET_V2);
    const sourceExecutionIndex = readU32(bytes, Abi.GENERAL_VERIFIER_CURRENT_SOURCE_EXECUTION_INDEX_OFFSET_V2);
    if (maxLots === 0n || lots === 0n || lots > maxLots || !(sourcePageIndex < nextPageIndex || (sourcePageIndex === nextPageIndex && sourceExecutionIndex < nextRowIndex))) throw new Error('General verifier current order is noncanonical');
    currentOrder = Object.freeze({
      orderId: idHex(bytes, Abi.GENERAL_VERIFIER_CURRENT_ORDER_ID_OFFSET_V2, 'verifier current Order'),
      owner: pubkeyHex(bytes, Abi.GENERAL_VERIFIER_CURRENT_OWNER_ID_OFFSET_V2, 'verifier current owner'),
      nonce: u64(bytes, Abi.GENERAL_VERIFIER_CURRENT_NONCE_OFFSET_V2), maxLots,
      maxQuoteDebitPerLot: u64(bytes, Abi.GENERAL_VERIFIER_CURRENT_MAX_QUOTE_DEBIT_PER_LOT_OFFSET_V2), lots,
      sourcePageIndex, sourceExecutionIndex, receivePerLot, deliverPerLot,
    });
  } else {
    requireZero(bytes, Abi.GENERAL_VERIFIER_CURRENT_ORDER_ID_OFFSET_V2, 104, 'General verifier absent current order');
    if (receivePerLot.some((value) => value !== 0n) || deliverPerLot.some((value) => value !== 0n)) throw new Error('General verifier absent order carries a portfolio');
  }
  return Object.freeze({
    kind: 'verifier' as const, phase: initial ? 'initial' : nextPageIndex === pageCount ? 'complete' : 'streaming',
    outcomeCount, pageCount, nextPageIndex, nextRowIndex, orderCount, revision, candidateCoordinate,
    candidateId: idHex(bytes, Abi.GENERAL_VERIFIER_CANDIDATE_ID_OFFSET_V2, 'verifier Candidate'),
    productId: idHex(bytes, Abi.GENERAL_VERIFIER_PRODUCT_ID_OFFSET_V2, 'verifier Product'),
    batchId: idHex(bytes, Abi.GENERAL_VERIFIER_BATCH_ID_OFFSET_V2, 'verifier Batch'),
    priceScale, filledLots, quoteDebit, quoteCredit, prices, claimInputs, claimOutputs, currentOrder,
  });
}

/** Hostile-decode the raw terminal VerifiedCandidate result account. */
export function decodeGeneralVerifiedCandidateV2(bytes: Uint8Array): GeneralVerifiedCandidateStatusV2 {
  if (bytes.length < Abi.GENERAL_VERIFIED_CANDIDATE_HEADER_BYTES_V2
      || !same(slice(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_MAGIC_OFFSET_V2, Abi.GENERAL_VERIFIED_CANDIDATE_MAGIC_V2.length), Abi.GENERAL_VERIFIED_CANDIDATE_MAGIC_V2)
      || u16(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_VERSION_OFFSET_V2) !== Abi.GENERAL_VERIFIED_CANDIDATE_VERSION_V2
      || bytes[Abi.GENERAL_VERIFIED_CANDIDATE_PHASE_OFFSET_V2] !== Abi.GENERAL_VERIFIED_CANDIDATE_PHASE_V2) throw new Error('General verified candidate is not exact V2');
  const afterVerifiedPhase = Abi.GENERAL_VERIFIED_CANDIDATE_PHASE_OFFSET_V2 + Uint8Array.BYTES_PER_ELEMENT;
  requireZero(bytes, afterVerifiedPhase, Abi.GENERAL_VERIFIED_CANDIDATE_OUTCOME_COUNT_OFFSET_V2 - afterVerifiedPhase, 'General verified candidate header');
  const outcomeCount = readU32(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_OUTCOME_COUNT_OFFSET_V2);
  if (outcomeCount === 0 || bytes.length !== Abi.GENERAL_VERIFIED_CANDIDATE_HEADER_BYTES_V2 + outcomeCount * 16) throw new Error('General verified candidate has the wrong runtime width');
  const pageCount = readU32(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_PAGE_COUNT_OFFSET_V2);
  const candidateCoordinate = readU32(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_CANDIDATE_COORDINATE_OFFSET_V2);
  const revision = u64(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_REVISION_OFFSET_V2);
  const priceScale = u64(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_PRICE_SCALE_OFFSET_V2);
  if (pageCount === 0 || candidateCoordinate === 0 || revision === 0n || priceScale === 0n) throw new Error('General verified candidate has zero canonical coordinates');
  const outputs = Abi.GENERAL_VERIFIED_CANDIDATE_CLAIM_INPUTS_BASE_OFFSET_V2 + outcomeCount * Abi.GENERAL_VERIFIED_CANDIDATE_TAIL_ITEM_STRIDE_V2;
  return Object.freeze({
    kind: 'verified-candidate' as const, outcomeCount, pageCount, candidateCoordinate, revision,
    candidateId: idHex(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_CANDIDATE_ID_OFFSET_V2, 'verified Candidate'),
    productId: idHex(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_PRODUCT_ID_OFFSET_V2, 'verified Product'),
    batchId: idHex(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_BATCH_ID_OFFSET_V2, 'verified Batch'),
    filledLots: u64(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_FILLED_LOTS_OFFSET_V2),
    quoteDebit: u64(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_QUOTE_DEBIT_OFFSET_V2),
    quoteCredit: u64(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_QUOTE_CREDIT_OFFSET_V2), priceScale,
    claimInputs: Object.freeze(Array.from({ length: outcomeCount }, (_, index) => u64(bytes, Abi.GENERAL_VERIFIED_CANDIDATE_CLAIM_INPUTS_BASE_OFFSET_V2 + index * 8))),
    claimOutputs: Object.freeze(Array.from({ length: outcomeCount }, (_, index) => u64(bytes, outputs + index * 8))),
  });
}

export function decodeGeneralLocalStateV3(bytes: Uint8Array): GeneralLocalStateStatusV3 {
  if (bytes.length < Abi.GENERAL_LOCAL_STATE_HEADER_BYTES_V3
      || !same(slice(bytes, Abi.GENERAL_LOCAL_STATE_MAGIC_OFFSET_V3, Abi.GENERAL_LOCAL_STATE_MAGIC_V3.length), Abi.GENERAL_LOCAL_STATE_MAGIC_V3)
      || u16(bytes, Abi.GENERAL_LOCAL_STATE_VERSION_OFFSET_V3) !== Abi.GENERAL_LOCAL_STATE_VERSION_V3) throw new Error('General local state is not exact V3');
  const afterBump = Abi.GENERAL_LOCAL_STATE_BUMP_OFFSET_V3 + Uint8Array.BYTES_PER_ELEMENT;
  const publicKeyBytes = PublicKey.default.toBytes().length;
  const afterBeneficiary = Abi.GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3 + publicKeyBytes;
  requireZero(bytes, afterBump, Abi.GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3 - afterBump, 'General local-state header');
  requireZero(bytes, afterBeneficiary, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 - afterBeneficiary, 'General local-state header');
  const rentPrincipal = u64(bytes, Abi.GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3); const beneficiaryBytes = slice(bytes, Abi.GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3, publicKeyBytes);
  if (rentPrincipal === 0n || isZero(beneficiaryBytes)) throw new Error('General local state has invalid lifecycle facts');
  const body = slice(bytes, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3, bytes.length - Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3);
  const kind = bytes[Abi.GENERAL_LOCAL_STATE_KIND_OFFSET_V3];
  const status = kind === Abi.GENERAL_LOCAL_STATE_SELECTION_KIND_V3 ? decodeSelection(body)
    : kind === Abi.GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3 ? decodeSettlement(body)
      : kind === Abi.GENERAL_LOCAL_STATE_BATCH_KIND_V3 ? decodeBatch(body)
        : kind === Abi.GENERAL_LOCAL_STATE_ORDER_KIND_V3 ? decodeOrder(body)
          : kind === Abi.GENERAL_LOCAL_STATE_CANDIDATE_KIND_V3 ? decodeGeneralCandidateV1(body)
            : kind === Abi.GENERAL_LOCAL_STATE_VERIFIER_KIND_V3 ? decodeGeneralVerifierV2(body)
              : (() => { throw new Error('General local state has an unknown kind'); })();
  return Object.freeze({ status, bump: bytes[Abi.GENERAL_LOCAL_STATE_BUMP_OFFSET_V3], rentPrincipal, beneficiary: new PublicKey(beneficiaryBytes).toBase58() });
}

function decodeStateAccount(account: RpcAccount | null, tradingProgram: string, field: string): GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }> {
  if (account === null) throw new Error(`${field} is absent at the finalized observation floor`);
  if (account.owner === tradingProgram && !account.executable) return decodeGeneralLocalStateV3(account.data);
  const lamports = BigInt(account.lamports);
  if (account.owner === SYSTEM_PROGRAM_ID && !account.executable && account.data.length === 0 && lamports > 0n) return Object.freeze({ status: 'vacant' as const, lamports });
  throw new Error(`${field} is neither exact Trading V3 state nor a funded vacant System account`);
}

function decodeResultStateAccount(account: RpcAccount | null, tradingProgram: string, field: string): GeneralVerifiedCandidateStatusV2 | Readonly<{ status: 'vacant'; lamports: bigint }> {
  if (account === null) throw new Error(`${field} is absent at the finalized observation floor`);
  if (account.owner === tradingProgram && !account.executable) return decodeGeneralVerifiedCandidateV2(account.data);
  const lamports = BigInt(account.lamports);
  if (account.owner === SYSTEM_PROGRAM_ID && !account.executable && account.data.length === 0 && lamports > 0n) return Object.freeze({ status: 'vacant' as const, lamports });
  throw new Error(`${field} is neither an exact raw VerifiedCandidate V2 nor a funded vacant System account`);
}

function statusValue(value: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }>): GeneralLocalStateValueV3 | 'vacant' {
  return value.status === 'vacant' ? 'vacant' : value.status;
}

async function recordIdentity(value: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }>, account: RpcAccount | null): Promise<string | null> {
  if (value.status === 'vacant') return null;
  if (value.status.kind === 'batch') return generalBatchOccurrenceIdentityV1(value.status);
  if (value.status.kind === 'candidate') return value.status.candidateId;
  if (account === null) throw new Error('General state identity has no observed account');
  const body = slice(account.data, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3, account.data.length - Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3);
  if (value.status.kind === 'order') {
    const preimage = new Uint8Array(body.length - Abi.GENERAL_ORDER_STATE_BYTES_V1);
    preimage.set(slice(body, 0, Abi.GENERAL_ORDER_STATE_OFFSET_V1));
    preimage.set(slice(body, Abi.GENERAL_ORDER_ROW_BASE_V1, body.length - Abi.GENERAL_ORDER_ROW_BASE_V1), Abi.GENERAL_ORDER_STATE_OFFSET_V1);
    return hex(await sha256(preimage));
  }
  return null;
}

/** Derive the exact slot-independent occurrence identity used by the Rust runtime and operator. */
export async function generalBatchOccurrenceIdentityV1(value: GeneralBatchStatusV1): Promise<string> {
  const outcomeCount = integer(value.outcomeCount, 'General batch outcome count');
  const maxOrders = integer(value.maxOrders, 'General batch max orders');
  if (outcomeCount === 0 || maxOrders === 0 || value.generation === 0n || value.priceScale === 0n) throw new Error('General batch occurrence terms contain a zero required scalar');
  for (const [field, scalar] of [['sequence', value.sequence], ['generation', value.generation], ['price scale', value.priceScale]] as const) {
    if (typeof scalar !== 'bigint' || scalar < 0n || scalar > MAX_U64) throw new Error(`General batch ${field} is outside u64`);
  }
  const market = address(value.market, 'General batch Market');
  const productId = identity(value.productId, 'General batch Product');
  const configId = identity(value.configId, 'General batch config');
  const terms = new Uint8Array(Abi.GENERAL_BATCH_OCCURRENCE_TERMS_BYTES_V1);
  terms.set(Abi.GENERAL_BATCH_OCCURRENCE_TERMS_MAGIC_V1, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_MAGIC_OFFSET_V1);
  writeU16(terms, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_VERSION_OFFSET_V1, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_VERSION_V1);
  terms[Abi.GENERAL_BATCH_OCCURRENCE_TERMS_PHASE_OFFSET_V1] = Abi.GENERAL_BATCH_PHASE_V1;
  writeU32(terms, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_OUTCOME_COUNT_OFFSET_V1, outcomeCount);
  writeU64(terms, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_SEQUENCE_OFFSET_V1, value.sequence);
  writeU64(terms, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_GENERATION_OFFSET_V1, value.generation);
  terms.set(new PublicKey(market).toBytes(), Abi.GENERAL_BATCH_OCCURRENCE_TERMS_MARKET_OFFSET_V1);
  terms.set(fromHex(productId, 'General batch Product'), Abi.GENERAL_BATCH_OCCURRENCE_TERMS_PRODUCT_ID_OFFSET_V1);
  terms.set(fromHex(configId, 'General batch config'), Abi.GENERAL_BATCH_OCCURRENCE_TERMS_CONFIG_ID_OFFSET_V1);
  writeU64(terms, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_PRICE_SCALE_OFFSET_V1, value.priceScale);
  writeU32(terms, Abi.GENERAL_BATCH_OCCURRENCE_TERMS_MAX_ORDERS_OFFSET_V1, maxOrders);
  return hex(await sha256(terms));
}

async function validateActionPrestate(
  inspection: GeneralPlanInspectionV5,
  primary: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }>,
  secondaryState: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }> | null,
  conditionalResult: GeneralVerifiedCandidateStatusV2 | Readonly<{ status: 'vacant'; lamports: bigint }> | null,
  candidateClose: GeneralChainStatusV5['candidateClose'],
  observedSlot: bigint,
  primaryAccount: RpcAccount | null,
  secondaryAccount: RpcAccount | null,
): Promise<void> {
  const state = statusValue(primary); const request = inspection.request;
  const secondary = secondaryState === null ? null : statusValue(secondaryState);
  const primaryIdentity = await recordIdentity(primary, primaryAccount);
  const secondaryIdentity = secondaryState === null ? null : await recordIdentity(secondaryState, secondaryAccount);
  if (primary.status !== 'vacant' && primary.bump !== request.primaryStateBump) throw new Error('General primary state carries another canonical bump');
  if (secondaryState !== null && secondaryState.status !== 'vacant' && secondaryState.bump !== request.secondaryStateBump) throw new Error('General secondary state carries another canonical bump');
  if (request.action === 'consider') {
    if (state !== 'vacant' && (state.kind !== 'selection' || state.phase !== 'open' || state.revision !== request.expectedRevision || state.outcomeCount !== inspection.plan.outcomeCount)) throw new Error('Consider prestate is not the matching open selection');
    if (state === 'vacant' && request.expectedRevision !== 0n) throw new Error('vacant Consider requires revision zero');
  } else if (request.action === 'freeze') {
    if (state === 'vacant' || state.kind !== 'selection' || state.phase !== 'open' || state.revision !== request.expectedRevision || state.outcomeCount !== inspection.plan.outcomeCount) throw new Error('Freeze prestate is not the matching open best-valid-submitted selection');
  } else if (request.action === 'initialize-settlement') {
    if (state !== 'vacant' || request.expectedRevision !== 0n) throw new Error('InitializeSettlement requires a vacant settlement at revision zero');
  } else if (request.action === 'collect' || request.action === 'materialize' || request.action === 'distribute' || request.action === 'close') {
    const expectedPhase: Record<'collect' | 'materialize' | 'distribute' | 'close', GeneralSettlementPhaseV2> = { collect: 'collecting', materialize: 'materializing', distribute: 'distributing', close: 'ready-to-close' };
    if (state === 'vacant' || state.kind !== 'settlement' || state.phase !== expectedPhase[request.action] || state.revision !== request.expectedRevision
        || state.outcomeCount !== inspection.plan.outcomeCount || state.candidateId !== request.subjectId) throw new Error(`${request.action} prestate differs from its exact settlement cursor`);
  } else if (request.action === 'open-batch') {
    if (state !== 'vacant') throw new Error('OpenBatch requires a funded vacant batch successor');
  } else if (request.action === 'place-order') {
    if (state === 'vacant' || state.kind !== 'batch' || state.phase !== 'collecting' || state.outcomeCount !== inspection.plan.outcomeCount
        || state.market !== inspection.plan.market || state.generation !== inspection.plan.generation || secondary !== 'vacant') throw new Error('PlaceOrder prestate is not its exact collecting batch and vacant order successor');
  } else if (request.action === 'cancel-order') {
    if (state === 'vacant' || state.kind !== 'batch' || state.phase !== 'collecting' || state.outcomeCount !== inspection.plan.outcomeCount
        || state.market !== inspection.plan.market || state.generation !== inspection.plan.generation || secondary === null || secondary === 'vacant' || secondary.kind !== 'order'
        || secondary.phase !== 'placed' || secondary.outcomeCount !== state.outcomeCount || secondary.market !== state.market || secondary.generation !== state.generation
        || secondary.batchId !== primaryIdentity || request.subjectId !== secondaryIdentity) throw new Error('CancelOrder prestate does not join its exact collecting batch and placed order');
  } else if (request.action === 'close-batch') {
    if (state === 'vacant' || state.kind !== 'batch' || state.phase !== 'collecting' || state.outcomeCount !== inspection.plan.outcomeCount
        || state.market !== inspection.plan.market || state.generation !== inspection.plan.generation || request.subjectId !== primaryIdentity) throw new Error('CloseBatch prestate is not its exact collecting batch');
  } else if (request.action === 'release-order') {
    if (state === 'vacant' || state.kind !== 'order' || state.phase !== 'placed' || state.outcomeCount !== inspection.plan.outcomeCount
        || state.market !== inspection.plan.market || state.generation !== inspection.plan.generation || request.subjectId !== primaryIdentity) throw new Error('ReleaseOrder prestate is not its exact placed order');
  } else if (request.action === 'submit-candidate') {
    if (state !== 'vacant' || secondary !== null || conditionalResult !== null || request.expectedRevision !== 0n) throw new Error('SubmitCandidate requires one funded vacant Candidate successor');
  } else if (request.action === 'verify-candidate-row') {
    if (state === 'vacant' || state.kind !== 'candidate' || state.phase !== 'submitted' || state.outcomeCount !== inspection.plan.outcomeCount
        || state.candidateId !== request.subjectId || secondary === null || conditionalResult === null
        || !('status' in conditionalResult && conditionalResult.status === 'vacant')) throw new Error('VerifyCandidateRow prestate does not join its submitted Candidate, Verifier, and vacant Result');
    if (secondary === 'vacant') {
      if (request.expectedRevision !== 0n || request.pageIndex !== 0 || request.executionIndex !== 0) throw new Error('vacant VerifyCandidateRow cursor requires the initial row');
    } else if (secondary.kind !== 'verifier' || secondary.phase === 'complete' || secondary.outcomeCount !== state.outcomeCount || secondary.pageCount !== state.pageCount
        || secondary.candidateId !== state.candidateId || secondary.batchId !== state.batchId || secondary.revision !== request.expectedRevision
        || secondary.nextPageIndex !== request.pageIndex || secondary.nextRowIndex !== request.executionIndex) throw new Error('VerifyCandidateRow request differs from its exact streamed Verifier cursor');
  } else if (request.action === 'close-candidate') {
    if (state === 'vacant' || state.kind !== 'candidate' || state.outcomeCount !== inspection.plan.outcomeCount
        || state.candidateId !== request.subjectId || candidateClose === null
        || state.cleanupRemaining !== state.rewardRateLamports
        || candidateClose.solver !== state.solver || candidateClose.closedBatch.phase !== 'closed'
        || candidateClose.closedBatch.outcomeCount !== state.outcomeCount
        || candidateClose.closedBatch.market !== inspection.plan.market
        || candidateClose.closedBatch.generation !== inspection.plan.generation
        || await generalBatchOccurrenceIdentityV1(candidateClose.closedBatch) !== state.batchId) {
      throw new Error('CloseCandidate prestate does not join its exact Candidate, solver, and independently authenticated closed Batch');
    }
    if (state.phase !== 'considered' && observedSlot < candidateClose.closedBatch.settlementCloseSlot) {
      throw new Error('CloseCandidate would censor a live unconsidered Candidate before its Batch settlement deadline');
    }
  }
  if (request.action === 'close' || request.action === 'place-order') {
    if (secondaryState === null || statusValue(secondaryState) !== 'vacant') throw new Error('General secondary successor is not vacant');
  } else if (request.action !== 'cancel-order' && request.action !== 'verify-candidate-row' && secondaryState !== null) throw new Error('General plan carries an undeclared secondary lifecycle account');
}

export async function reacquireGeneralSuccessorStatusV5(client: SolanaRpcClient, inspection: GeneralPlanInspectionV5): Promise<GeneralChainStatusV5> {
  const dependencies = await acquireUnsignedTransactionDependenciesV1(client, inspection.transaction);
  if (dependencies.missing.length !== 0 || dependencies.nonExecutablePrograms.length !== 0) throw new Error('General transaction has missing accounts or non-executable programs');
  const dependencyAddresses = new Set(dependencies.dependencies.map((entry) => entry.address));
  const lifecycleStates = [inspection.plan.lifecycle.primary, inspection.plan.lifecycle.secondary, inspection.plan.lifecycle.conditionalResult]
    .filter((state): state is GeneralLifecycleStateV5 => state !== null);
  for (const [addressValue, field] of [[inspection.plan.market, 'Market'], [inspection.plan.root, 'root'], [inspection.plan.tradingProgram, 'Trading program'], ...lifecycleStates.map((state) => [state.account, `lifecycle account ${state.accountCoordinate}`] as const)] as const) {
    if (!dependencyAddresses.has(addressValue)) throw new Error(`General transaction omits its ${field}`);
  }
  const message = inspection.transaction.transaction.message as unknown as MessageView;
  const compiled = message.compiledInstructions[1];
  if (compiled === undefined) throw new Error('General transaction lost its Hot instruction');
  for (const state of lifecycleStates) {
    const messageIndex = compiled.accountKeyIndexes[state.accountCoordinate];
    if (messageIndex === undefined || dependencies.dependencies[messageIndex]?.address !== state.account) throw new Error(`General lifecycle projection differs at logical account ${state.accountCoordinate}`);
  }
  const dependencyAt = (coordinate: number, field: string) => {
    const messageIndex = compiled.accountKeyIndexes[coordinate];
    const dependency = messageIndex === undefined ? undefined : dependencies.dependencies[messageIndex];
    if (dependency === undefined) throw new Error(`General transaction omits its ${field} at logical account ${coordinate}`);
    return dependency;
  };
  const candidateCloseRouting = inspection.request.action === 'close-candidate' ? Object.freeze({
    cranker: dependencyAt(6, 'CloseCandidate cranker'),
    solver: dependencyAt(7, 'CloseCandidate solver'),
    closedBatch: dependencyAt(Abi.GENERAL_CLOSE_CANDIDATE_BATCH_ACCOUNT_V3, 'CloseCandidate closed Batch'),
  }) : null;
  if (candidateCloseRouting !== null
      && (!candidateCloseRouting.cranker.signer || !candidateCloseRouting.cranker.writable || candidateCloseRouting.cranker.program
        || candidateCloseRouting.solver.signer || !candidateCloseRouting.solver.writable || candidateCloseRouting.solver.program
        || candidateCloseRouting.closedBatch.signer || candidateCloseRouting.closedBatch.writable || candidateCloseRouting.closedBatch.program)) {
    throw new Error('CloseCandidate cranker, solver, or closed-Batch privileges differ from the authenticated operator topology');
  }
  const stateAddresses = [...lifecycleStates.map((state) => state.account), ...(candidateCloseRouting === null ? [] : [candidateCloseRouting.closedBatch.address])];
  const observation = await client.multipleAccounts(stateAddresses, inspection.plan.observedSlot.toString());
  const primaryAccount = observation.accounts[0]?.account ?? null;
  const secondaryAccount = inspection.plan.lifecycle.secondary === null ? null : observation.accounts[1]?.account ?? null;
  const resultIndex = inspection.plan.lifecycle.secondary === null ? 1 : 2;
  const resultAccount = inspection.plan.lifecycle.conditionalResult === null ? null : observation.accounts[resultIndex]?.account ?? null;
  const primary = decodeStateAccount(primaryAccount, inspection.plan.tradingProgram, 'General primary state');
  const secondary = inspection.plan.lifecycle.secondary === null ? null : decodeStateAccount(secondaryAccount, inspection.plan.tradingProgram, 'General secondary state');
  const conditionalResult = inspection.plan.lifecycle.conditionalResult === null ? null : decodeResultStateAccount(resultAccount, inspection.plan.tradingProgram, 'General conditional Result state');
  const closedBatchAccount = candidateCloseRouting === null ? null : observation.accounts[lifecycleStates.length]?.account ?? null;
  const closedBatchState = candidateCloseRouting === null ? null : decodeStateAccount(closedBatchAccount, inspection.plan.tradingProgram, 'CloseCandidate closed Batch evidence');
  if (closedBatchState !== null && (closedBatchState.status === 'vacant' || closedBatchState.status.kind !== 'batch')) {
    throw new Error('CloseCandidate readonly evidence is not one exact materialized General Batch');
  }
  // `status === 'vacant'` is gone and `.kind !== 'batch'` stays, which is not
  // an inconsistency: the guard above narrowed the first away -- TypeScript
  // reported the repeat as a comparison with no overlap -- and did NOT narrow
  // the second, so that one is still the check that selects the Batch member
  // for `closedBatch` below. Dropping both put the wide union back and moved
  // the error two lines down rather than removing it.
  const candidateClose = candidateCloseRouting === null || closedBatchState === null || closedBatchState.status.kind !== 'batch' ? null : Object.freeze({
    cranker: candidateCloseRouting.cranker.address,
    solver: candidateCloseRouting.solver.address,
    closedBatchAccount: candidateCloseRouting.closedBatch.address,
    closedBatch: closedBatchState.status,
  });
  const observed = [primary, ...(secondary === null ? [] : [secondary]), ...(conditionalResult === null ? [] : [conditionalResult])];
  for (let index = 0; index < lifecycleStates.length; index += 1) {
    const value = observed[index];
    const materialized = value !== undefined && !('status' in value && value.status === 'vacant');
    if (materialized !== lifecycleStates[index]?.isMaterialized) throw new Error(`General lifecycle materialization changed at logical account ${lifecycleStates[index]?.accountCoordinate}`);
  }
  await validateActionPrestate(inspection, primary, secondary, conditionalResult, candidateClose, BigInt(observation.slot), primaryAccount, secondaryAccount);
  return Object.freeze({ observedSlot: observation.slot, dependencies, primary, secondary, conditionalResult, candidateClose });
}

export function decodeGeneralHotReceiptV3(base64: string, inspection: GeneralPlanInspectionV5): GeneralHotReceiptV3 {
  const bytes = base64Bytes(base64, 'General Hot receipt', Abi.GENERAL_HOT_ACK_BYTES_V3);
  if (!same(slice(bytes, 0, 8), Abi.GENERAL_HOT_ACK_MAGIC_V3) || u16(bytes, 8) !== Abi.GENERAL_HOT_VERSION_V3 || u16(bytes, 10) !== Abi.GENERAL_HOT_PROFILE_V3) throw new Error('General Hot receipt is not exact V3');
  requireZero(bytes, 12, 4, 'General Hot receipt');
  const receipt = Object.freeze({
    releaseSet: idHex(bytes, Abi.GENERAL_ACK_RELEASE_SET_OFFSET_V3, 'receipt release set'), market: pubkeyHex(bytes, Abi.GENERAL_ACK_MARKET_OFFSET_V3, 'receipt Market'),
    generation: u64(bytes, Abi.GENERAL_ACK_GENERATION_OFFSET_V3), root: pubkeyHex(bytes, Abi.GENERAL_ACK_ROOT_OFFSET_V3, 'receipt root'),
    requestDigest: idHex(bytes, Abi.GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3, 'receipt request digest'), selectedProgram: idHex(bytes, Abi.GENERAL_ACK_SELECTED_PROGRAM_OFFSET_V3, 'receipt selected program'),
    rootPrestateDigest: idHex(bytes, Abi.GENERAL_ACK_ROOT_PRESTATE_DIGEST_OFFSET_V3, 'receipt root prestate'), rootPoststateDigest: idHex(bytes, Abi.GENERAL_ACK_ROOT_POSTSTATE_DIGEST_OFFSET_V3, 'receipt root poststate'),
    executionDigest: idHex(bytes, Abi.GENERAL_ACK_EXECUTION_DIGEST_OFFSET_V3, 'receipt execution digest'),
  });
  const plan = inspection.plan;
  if (receipt.releaseSet !== plan.releaseSet || receipt.market !== plan.market || receipt.generation !== plan.generation || receipt.root !== plan.root
      || receipt.requestDigest !== plan.familyRequestDigest || receipt.selectedProgram !== plan.artifacts.descriptor || receipt.rootPrestateDigest !== plan.rootPrestateDigest) throw new Error('General Hot receipt belongs to another request, Market, root, release, or selected program');
  return receipt;
}

export function generalPlanTemplateV5(): string {
  return JSON.stringify({
    format: 'dclutch/general-successor-plan/v5', action: 'consider', transactionBase64: '', observedSlot: '0', outcomeCount: 1, scratchPageCount: 1, heapFrameBytes: Abi.GENERAL_HOT_HEAP_FRAME_BYTES_V3,
    tradingProgram: '', lookupTable: '', payer: '', requiredSigners: [], market: '', root: '', generation: '0', releaseSet: '', rootPrestateDigest: '',
    familyRequestDigest: '', checkedManifestDigest: '', tradingArtifactRelease: '', generalArtifactRelease: '', productRecord: '',
    artifacts: Object.fromEntries(ARTIFACT_KEYS.map((key) => [key, ''])),
    lifecycle: { primary: { accountCoordinate: 5, account: '', bump: 0, isMaterialized: false }, secondary: null, conditionalResult: null, terminalCoordinate: null, childAccountStart: 8 }, childRoutes: [],
  }, null, 2);
}

export function transactionBytesV5(inspection: GeneralPlanInspectionV5): Uint8Array {
  return new Uint8Array(inspection.transaction.bytes);
}

export function transactionV5(inspection: GeneralPlanInspectionV5): VersionedTransaction {
  return inspection.transaction.transaction;
}
