import { PublicKey, VersionedTransaction } from '@solana/web3.js';

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

const MAX_PLAN_TEXT_BYTES = 64 * 1024;
const MAX_U64 = 18_446_744_073_709_551_615n;
const ACTIONS = ['consider', 'freeze', 'initialize-settlement', 'collect', 'materialize', 'distribute', 'close'] as const;
const ARTIFACT_KEYS = ['programSet', 'descriptor', 'config', 'accountProfile', 'lifecyclePolicy', 'requestProfile', 'strategy', 'certificate', 'admission', 'transition', 'effect'] as const;

export type GeneralSuccessorActionV5 = (typeof ACTIONS)[number];
export type GeneralChildRoleV5 = 'claims' | 'custody';
export type GeneralSettlementPhaseV2 = 'collecting' | 'materializing' | 'distributing' | 'ready-to-close' | 'terminal';

export type GeneralControllerRequestV2 = Readonly<{
  action: GeneralSuccessorActionV5;
  expectedRevision: bigint;
  candidateId: string | null;
  pageIndex: number;
  executionIndex: number;
  manifestOrderIndex: number;
  stateBump: number;
  terminalStateBump: number | null;
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

export type GeneralSuccessorPlanDocumentV5 = Readonly<{
  format: 'dclutch/general-successor-plan/v5';
  action: GeneralSuccessorActionV5;
  transactionBase64: string;
  observedSlot: bigint;
  outcomeCount: number;
  scratchPageCount: number;
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
    primaryState: string;
    primaryStateBump: number;
    terminalState: string | null;
    terminalStateBump: number | null;
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
}>;

export type GeneralPlanInspectionV5 = Readonly<{
  plan: GeneralSuccessorPlanDocumentV5;
  transaction: UnsignedTransactionInspectionV1;
  envelope: GeneralHotEnvelopeV3;
  request: GeneralControllerRequestV2;
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

export type GeneralLocalStateStatusV3 = Readonly<{
  status: GeneralSelectionStatusV2 | GeneralSettlementStatusV2;
  bump: number;
  rentPrincipal: bigint;
  beneficiary: string;
}>;

export type GeneralChainStatusV5 = Readonly<{
  observedSlot: string;
  dependencies: UnsignedTransactionChainReportV1;
  primary: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }>;
  terminal: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }> | null;
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
  if (new TextEncoder().encode(input).length > MAX_PLAN_TEXT_BYTES) throw new Error('General operator plan exceeds the browser byte bound');
  let raw: unknown;
  try { raw = JSON.parse(input); } catch { throw new Error('General operator plan is not JSON'); }
  const value = object(raw, 'General operator plan');
  exactKeys(value, ['format', 'action', 'transactionBase64', 'observedSlot', 'outcomeCount', 'scratchPageCount', 'tradingProgram', 'lookupTable', 'payer', 'requiredSigners', 'market', 'root', 'generation', 'releaseSet', 'rootPrestateDigest', 'familyRequestDigest', 'checkedManifestDigest', 'tradingArtifactRelease', 'generalArtifactRelease', 'productRecord', 'artifacts', 'lifecycle', 'childRoutes'], 'General operator plan');
  if (value.format !== 'dclutch/general-successor-plan/v5') throw new Error('General operator plan format is not V5');
  const selectedAction = action(value.action);
  const outcomeCount = integer(value.outcomeCount, 'outcomeCount');
  const scratchPageCount = integer(value.scratchPageCount, 'scratchPageCount');
  if (outcomeCount === 0 || scratchPageCount === 0) throw new Error('General outcome and scratch-page counts must be nonzero');
  if (!Array.isArray(value.requiredSigners) || value.requiredSigners.length === 0 || value.requiredSigners.length > 32) throw new Error('General required signers are not a bounded nonempty array');
  const requiredSigners = Object.freeze(value.requiredSigners.map((entry, index) => address(entry, `requiredSigners[${index}]`)));
  if (new Set(requiredSigners).size !== requiredSigners.length) throw new Error('General required signers contain a duplicate');
  const payer = address(value.payer, 'payer');
  if (requiredSigners[0] !== payer) throw new Error('General fee payer is not the first required signer');
  const artifactsRaw = object(value.artifacts, 'artifacts');
  exactKeys(artifactsRaw, ARTIFACT_KEYS, 'artifacts');
  const artifacts = Object.freeze(Object.fromEntries(ARTIFACT_KEYS.map((key) => [key, identity(artifactsRaw[key], `artifacts.${key}`)])) as Record<(typeof ARTIFACT_KEYS)[number], string>);
  const lifecycleRaw = object(value.lifecycle, 'lifecycle');
  exactKeys(lifecycleRaw, ['primaryState', 'primaryStateBump', 'terminalState', 'terminalStateBump', 'terminalCoordinate', 'childAccountStart'], 'lifecycle');
  const terminalState = lifecycleRaw.terminalState === null ? null : address(lifecycleRaw.terminalState, 'lifecycle.terminalState');
  const terminalStateBump = lifecycleRaw.terminalStateBump === null ? null : integer(lifecycleRaw.terminalStateBump, 'lifecycle.terminalStateBump', 255);
  const terminalCoordinate = lifecycleRaw.terminalCoordinate === null ? null : integerText(lifecycleRaw.terminalCoordinate, 'lifecycle.terminalCoordinate');
  const expectedChildStart = selectedAction === 'close' ? 9 : 8;
  const lifecycle = Object.freeze({
    primaryState: address(lifecycleRaw.primaryState, 'lifecycle.primaryState'),
    primaryStateBump: integer(lifecycleRaw.primaryStateBump, 'lifecycle.primaryStateBump', 255),
    terminalState,
    terminalStateBump,
    terminalCoordinate,
    childAccountStart: integer(lifecycleRaw.childAccountStart, 'lifecycle.childAccountStart', 65_535),
  });
  if (lifecycle.childAccountStart !== expectedChildStart || (selectedAction === 'close') !== (terminalState !== null && terminalStateBump !== null && terminalCoordinate !== null)) throw new Error('General lifecycle shape differs from the selected action');
  if (terminalState === lifecycle.primaryState) throw new Error('General primary and terminal lifecycle accounts alias');
  const childRoutes = parseChildRoutes(value.childRoutes);
  validateActionRoutes(selectedAction, childRoutes, lifecycle.childAccountStart);
  return Object.freeze({
    format: value.format, action: selectedAction,
    transactionBase64: text(value.transactionBase64, 'transactionBase64', 4_096),
    observedSlot: integerText(value.observedSlot, 'observedSlot'), outcomeCount, scratchPageCount,
    tradingProgram: address(value.tradingProgram, 'tradingProgram'), lookupTable: address(value.lookupTable, 'lookupTable'), payer,
    requiredSigners, market: address(value.market, 'market'), root: address(value.root, 'root'), generation: integerText(value.generation, 'generation'),
    releaseSet: identity(value.releaseSet, 'releaseSet'), rootPrestateDigest: identity(value.rootPrestateDigest, 'rootPrestateDigest'),
    familyRequestDigest: identity(value.familyRequestDigest, 'familyRequestDigest'), checkedManifestDigest: identity(value.checkedManifestDigest, 'checkedManifestDigest'),
    tradingArtifactRelease: identity(value.tradingArtifactRelease, 'tradingArtifactRelease'), generalArtifactRelease: identity(value.generalArtifactRelease, 'generalArtifactRelease'),
    productRecord: identity(value.productRecord, 'productRecord'), artifacts, lifecycle, childRoutes,
  });
}

export function decodeGeneralControllerRequestV2(bytes: Uint8Array): GeneralControllerRequestV2 {
  if (bytes.length !== Abi.GENERAL_REQUEST_BYTES_V2 || !same(slice(bytes, 0, 8), Abi.GENERAL_REQUEST_MAGIC_V2) || u16(bytes, 8) !== 2) throw new Error('General controller request is not exact V2');
  requireZero(bytes, 12, 4, 'General request'); requireZero(bytes, 63, 1, 'General request');
  const tag = bytes[Abi.GENERAL_REQUEST_ACTION_OFFSET_V2];
  const selectedAction = ACTIONS[tag];
  if (selectedAction === undefined) throw new Error('General controller request has an unknown action');
  const candidate = slice(bytes, Abi.GENERAL_REQUEST_CANDIDATE_ID_OFFSET_V2, 32);
  const candidateId = isZero(candidate) ? null : hex(candidate);
  const pageIndex = readU32(bytes, Abi.GENERAL_REQUEST_PAGE_INDEX_OFFSET_V2);
  const executionIndex = bytes[Abi.GENERAL_REQUEST_EXECUTION_INDEX_OFFSET_V2];
  const manifestOrderIndex = bytes[Abi.GENERAL_REQUEST_MANIFEST_ORDER_OFFSET_V2];
  const stateBump = bytes[Abi.GENERAL_REQUEST_STATE_BUMP_OFFSET_V2];
  const terminalRaw = bytes[Abi.GENERAL_REQUEST_TERMINAL_BUMP_OFFSET_V2];
  if (selectedAction !== 'close' && terminalRaw !== 0) throw new Error('nonterminal General request carries a terminal bump');
  if (selectedAction !== 'collect' && selectedAction !== 'distribute' && manifestOrderIndex !== 0) throw new Error('nonrow General request carries a manifest ordinal');
  const rowAction = selectedAction === 'collect' || selectedAction === 'distribute';
  const canonical = selectedAction === 'freeze' ? candidateId === null && pageIndex === 0 && executionIndex === 0
    : selectedAction === 'consider' ? candidateId !== null && executionIndex === 0
      : rowAction ? candidateId !== null
        : candidateId !== null && pageIndex === 0 && executionIndex === 0;
  if (!canonical) throw new Error('General request cursor is noncanonical for its action');
  return Object.freeze({ action: selectedAction, expectedRevision: u64(bytes, Abi.GENERAL_REQUEST_EXPECTED_REVISION_OFFSET_V2), candidateId, pageIndex, executionIndex, manifestOrderIndex, stateBump, terminalStateBump: selectedAction === 'close' ? terminalRaw : null, bytes: new Uint8Array(bytes) });
}

function decodeEnvelope(bytes: Uint8Array): GeneralHotEnvelopeV3 {
  if (bytes.length !== Abi.GENERAL_HOT_ENVELOPE_BYTES_V3 || !same(slice(bytes, 0, 8), Abi.GENERAL_HOT_MAGIC_V3)
      || u16(bytes, 8) !== Abi.GENERAL_HOT_VERSION_V3 || u16(bytes, 10) !== Abi.GENERAL_HOT_PROFILE_V3
      || readU32(bytes, Abi.GENERAL_ENVELOPE_REQUEST_BYTES_OFFSET_V3) !== Abi.GENERAL_REQUEST_BYTES_V2) throw new Error('General Hot envelope is not exact V3/V2');
  requireZero(bytes, Abi.GENERAL_ENVELOPE_RESERVED_OFFSET_V3, 8, 'General Hot envelope');
  return Object.freeze({
    releaseSet: idHex(bytes, Abi.GENERAL_ENVELOPE_RELEASE_SET_OFFSET_V3, 'Hot release set'),
    market: pubkeyHex(bytes, Abi.GENERAL_ENVELOPE_MARKET_OFFSET_V3, 'Hot Market'),
    generation: u64(bytes, Abi.GENERAL_ENVELOPE_GENERATION_OFFSET_V3),
    rootPrestateDigest: idHex(bytes, Abi.GENERAL_ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET_V3, 'Hot root prestate digest'),
  });
}

export async function inspectGeneralSuccessorPlanV5(plan: GeneralSuccessorPlanDocumentV5): Promise<GeneralPlanInspectionV5> {
  const transaction = await inspectUnsignedTransactionV1(plan.transactionBase64);
  const message = transaction.transaction.message as unknown as MessageView;
  if (message.compiledInstructions.length !== 1) throw new Error('General plan must contain exactly one Hot instruction');
  if (message.addressTableLookups.length !== 1 || message.addressTableLookups[0]?.accountKey.toBase58() !== plan.lookupTable) throw new Error('General plan does not use its one exact canonical lookup table');
  const compiled = message.compiledInstructions[0];
  const minimumAccounts = Abi.GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3 + 8 + plan.scratchPageCount;
  if (compiled.accountKeyIndexes.length < minimumAccounts) throw new Error('General transaction is shorter than Hot38 + admitted strategy + canonical scratch geometry');
  const program = message.staticAccountKeys[compiled.programIdIndex];
  if (program === undefined || program.toBase58() !== plan.tradingProgram) throw new Error('General transaction invokes another Trading program');
  const signerCount = message.header.numRequiredSignatures;
  const signers = message.staticAccountKeys.slice(0, signerCount).map((key) => key.toBase58());
  if (signers.length !== plan.requiredSigners.length || signers.some((key, index) => key !== plan.requiredSigners[index])) throw new Error('General transaction signer order differs from the operator report');
  const instruction = new Uint8Array(compiled.data);
  if (instruction.length !== Abi.GENERAL_HOT_ENVELOPE_BYTES_V3 + Abi.GENERAL_REQUEST_BYTES_V2) throw new Error('General Hot instruction has another exact width');
  const envelope = decodeEnvelope(slice(instruction, 0, Abi.GENERAL_HOT_ENVELOPE_BYTES_V3));
  const request = decodeGeneralControllerRequestV2(slice(instruction, Abi.GENERAL_HOT_ENVELOPE_BYTES_V3, Abi.GENERAL_REQUEST_BYTES_V2));
  const requestDigest = hex(await sha256(request.bytes));
  if (request.action !== plan.action || envelope.releaseSet !== plan.releaseSet || envelope.market !== plan.market || envelope.generation !== plan.generation
      || envelope.rootPrestateDigest !== plan.rootPrestateDigest || requestDigest !== plan.familyRequestDigest
      || request.stateBump !== plan.lifecycle.primaryStateBump || request.terminalStateBump !== plan.lifecycle.terminalStateBump) throw new Error('General operator report differs from the exact transaction request or Hot envelope');
  if (plan.action === 'close') {
    if (request.expectedRevision === MAX_U64 || plan.lifecycle.terminalCoordinate !== request.expectedRevision + 1n) throw new Error('General Close terminal coordinate is not the revision successor');
  }
  return Object.freeze({ plan, transaction, envelope, request });
}

function decodeSelection(bytes: Uint8Array): GeneralSelectionStatusV2 {
  if (bytes.length !== Abi.GENERAL_SELECTION_BYTES_V2 || !same(slice(bytes, 0, 8), Abi.GENERAL_SELECTION_MAGIC_V2) || u16(bytes, 8) !== 2 || bytes[11] !== 0) throw new Error('General selection body is not exact V2');
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
  if (bytes.length < Abi.GENERAL_SETTLEMENT_HEADER_BYTES_V2 || !same(slice(bytes, 0, 8), Abi.GENERAL_SETTLEMENT_MAGIC_V2) || u16(bytes, 8) !== 2 || bytes[11] !== 0) throw new Error('General settlement body is not exact V2');
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

export function decodeGeneralLocalStateV3(bytes: Uint8Array): GeneralLocalStateStatusV3 {
  if (bytes.length < Abi.GENERAL_LOCAL_STATE_HEADER_BYTES_V3 || !same(slice(bytes, 0, 8), Abi.GENERAL_LOCAL_STATE_MAGIC_V3) || u16(bytes, 8) !== Abi.GENERAL_LOCAL_STATE_VERSION_V3) throw new Error('General local state is not exact V3');
  requireZero(bytes, 12, 4, 'General local-state header'); requireZero(bytes, 56, 8, 'General local-state header');
  const rentPrincipal = u64(bytes, Abi.GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3); const beneficiaryBytes = slice(bytes, Abi.GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3, 32);
  if (rentPrincipal === 0n || isZero(beneficiaryBytes)) throw new Error('General local state has invalid lifecycle facts');
  const body = slice(bytes, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3, bytes.length - Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3);
  const kind = bytes[Abi.GENERAL_LOCAL_STATE_KIND_OFFSET_V3];
  const status = kind === Abi.GENERAL_LOCAL_STATE_SELECTION_KIND_V3 ? decodeSelection(body)
    : kind === Abi.GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3 ? decodeSettlement(body)
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

function statusValue(value: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }>): GeneralSelectionStatusV2 | GeneralSettlementStatusV2 | 'vacant' {
  return value.status === 'vacant' ? 'vacant' : value.status;
}

function validateActionPrestate(inspection: GeneralPlanInspectionV5, primary: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }>, terminal: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }> | null): void {
  const state = statusValue(primary); const request = inspection.request;
  if (request.action === 'consider') {
    if (state !== 'vacant' && (state.kind !== 'selection' || state.phase !== 'open' || state.revision !== request.expectedRevision || state.outcomeCount !== inspection.plan.outcomeCount)) throw new Error('Consider prestate is not the matching open selection');
    if (state === 'vacant' && request.expectedRevision !== 0n) throw new Error('vacant Consider requires revision zero');
  } else if (request.action === 'freeze') {
    if (state === 'vacant' || state.kind !== 'selection' || state.phase !== 'open' || state.revision !== request.expectedRevision || state.outcomeCount !== inspection.plan.outcomeCount) throw new Error('Freeze prestate is not the matching open best-valid-submitted selection');
  } else if (request.action === 'initialize-settlement') {
    if (state !== 'vacant' || request.expectedRevision !== 0n) throw new Error('InitializeSettlement requires a vacant settlement at revision zero');
  } else {
    const expectedPhase: Record<'collect' | 'materialize' | 'distribute' | 'close', GeneralSettlementPhaseV2> = { collect: 'collecting', materialize: 'materializing', distribute: 'distributing', close: 'ready-to-close' };
    if (state === 'vacant' || state.kind !== 'settlement' || state.phase !== expectedPhase[request.action] || state.revision !== request.expectedRevision
        || state.outcomeCount !== inspection.plan.outcomeCount || state.candidateId !== request.candidateId) throw new Error(`${request.action} prestate differs from its exact settlement cursor`);
  }
  if (request.action === 'close') {
    if (terminal === null || statusValue(terminal) !== 'vacant') throw new Error('Close terminal successor is not vacant');
  } else if (terminal !== null) throw new Error('non-Close General plan carries a terminal lifecycle account');
}

export async function reacquireGeneralSuccessorStatusV5(client: SolanaRpcClient, inspection: GeneralPlanInspectionV5): Promise<GeneralChainStatusV5> {
  const dependencies = await acquireUnsignedTransactionDependenciesV1(client, inspection.transaction);
  if (dependencies.missing.length !== 0 || dependencies.nonExecutablePrograms.length !== 0) throw new Error('General transaction has missing accounts or non-executable programs');
  const dependencyAddresses = new Set(dependencies.dependencies.map((entry) => entry.address));
  for (const [addressValue, field] of [[inspection.plan.market, 'Market'], [inspection.plan.root, 'root'], [inspection.plan.tradingProgram, 'Trading program'], [inspection.plan.lifecycle.primaryState, 'primary state']] as const) {
    if (!dependencyAddresses.has(addressValue)) throw new Error(`General transaction omits its ${field}`);
  }
  if (inspection.plan.lifecycle.terminalState !== null && !dependencyAddresses.has(inspection.plan.lifecycle.terminalState)) throw new Error('General Close transaction omits its terminal state');
  const stateAddresses = [inspection.plan.lifecycle.primaryState, inspection.plan.lifecycle.terminalState].filter((value): value is string => value !== null);
  const observation = await client.multipleAccounts(stateAddresses, inspection.plan.observedSlot.toString());
  const primary = decodeStateAccount(observation.accounts[0]?.account ?? null, inspection.plan.tradingProgram, 'General primary state');
  const terminal = stateAddresses.length === 2 ? decodeStateAccount(observation.accounts[1]?.account ?? null, inspection.plan.tradingProgram, 'General terminal state') : null;
  validateActionPrestate(inspection, primary, terminal);
  return Object.freeze({ observedSlot: observation.slot, dependencies, primary, terminal });
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
    format: 'dclutch/general-successor-plan/v5', action: 'consider', transactionBase64: '', observedSlot: '0', outcomeCount: 1, scratchPageCount: 1,
    tradingProgram: '', lookupTable: '', payer: '', requiredSigners: [], market: '', root: '', generation: '0', releaseSet: '', rootPrestateDigest: '',
    familyRequestDigest: '', checkedManifestDigest: '', tradingArtifactRelease: '', generalArtifactRelease: '', productRecord: '',
    artifacts: Object.fromEntries(ARTIFACT_KEYS.map((key) => [key, ''])),
    lifecycle: { primaryState: '', primaryStateBump: 0, terminalState: null, terminalStateBump: null, terminalCoordinate: null, childAccountStart: 8 }, childRoutes: [],
  }, null, 2);
}

export function transactionBytesV5(inspection: GeneralPlanInspectionV5): Uint8Array {
  return new Uint8Array(inspection.transaction.bytes);
}

export function transactionV5(inspection: GeneralPlanInspectionV5): VersionedTransaction {
  return inspection.transaction.transaction;
}
