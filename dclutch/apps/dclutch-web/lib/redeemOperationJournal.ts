import { VersionedTransaction } from '@solana/web3.js';

import {
  type ClaimsCustodyReplayPlanV1,
  type ClaimsCustodyReplayRequestV1,
  type ClaimsCustodyReplayStateV1,
} from '@dclutch/sdk/claimsCustodyReplay';
import {
  type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';
import { submittedClientOperationWireV1 } from './clientOperationJournal';
import {
  buildWalletTerminalPayoutV3,
  parseWalletTerminalPayoutManifestV3,
  walletTerminalPayoutSummaryV3,
  type PreparedWalletTerminalPayoutV3,
  type WalletTerminalPayoutManifestV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';
import { type SignatureStatusObservation } from '@dclutch/sdk/rpc';

type UnsignedJournalInputV1 = ClientOperationScopeV1 & Readonly<{
  operation: 'claims-replay-create-v1' | 'wallet-terminal-payout-v3';
  operationDigest: string;
  intent: string;
  plan: string;
}>;

const REPLAY_PLAN_FORMAT = 'dclutch-claims-replay-journal-plan-v1' as const;
const PAYOUT_PLAN_FORMAT = 'dclutch-wallet-terminal-payout-journal-plan-v1' as const;
const MAX_BASE64 = 350_000;

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  }
  return btoa(binary);
}

function base64Bytes(value: unknown, field: string): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length > MAX_BASE64 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not bounded canonical base64`);
  }
  let binary: string;
  try { binary = atob(value); } catch { throw new Error(`${field} is not bounded canonical base64`); }
  const output = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (base64(output) !== value) throw new Error(`${field} is not bounded canonical base64`);
  return output;
}

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, expected: ReadonlyArray<string>, field: string): void {
  const observed = Object.keys(value).sort(); const sorted = [...expected].sort();
  if (observed.length !== sorted.length || observed.some((key, index) => key !== sorted[index])) {
    throw new Error(`${field} has missing or unknown fields`);
  }
}

function parsePlan(source: string, field: string): Record<string, unknown> {
  let value: unknown;
  try { value = JSON.parse(source); } catch { throw new Error(`${field} is not JSON`); }
  return object(value, field);
}

function canonicalReplayIntent(request: ClaimsCustodyReplayRequestV1): string {
  return JSON.stringify(Object.freeze({
    format: 'dclutch-claims-replay-journal-intent-v1',
    marketAddress: request.marketAddress,
    claimsProgramId: request.claimsProgramId,
    custodyProgramId: request.custodyProgramId,
    registryProgramId: request.registryProgramId,
    payer: request.payer,
  }));
}

function canonicalReplayPlan(plan: ClaimsCustodyReplayPlanV1): string {
  return JSON.stringify(Object.freeze({
    format: REPLAY_PLAN_FORMAT,
    marketAddress: plan.marketAddress,
    aggregateAddress: plan.aggregateAddress,
    replayAddress: plan.replayAddress,
    callerAuthorityAddress: plan.callerAuthorityAddress,
    activationCacheAddress: plan.activationCacheAddress,
    claimsProgramDataAddress: plan.claimsProgramDataAddress,
    realmRecordAddress: plan.realmRecordAddress,
    realmStagingAddress: plan.realmStagingAddress,
    payer: plan.payer,
    rentLamports: plan.rentLamports,
    custodyRequestBase64: base64(plan.custodyRequestBytes),
    custodyRequestDigestHex: plan.custodyRequestDigestHex,
    instructionBase64: base64(plan.instructionData),
    requiredSigners: plan.requiredSigners,
  }));
}

export function claimsReplayJournalInputV1(
  scope: ClientOperationScopeV1,
  request: ClaimsCustodyReplayRequestV1,
  plan: ClaimsCustodyReplayPlanV1,
): UnsignedJournalInputV1 {
  if (scope.market !== request.marketAddress || scope.owner !== request.payer
      || plan.marketAddress !== request.marketAddress || plan.payer !== request.payer) {
    throw new Error('Claims replay journal scope differs from its exact route');
  }
  return Object.freeze({
    ...scope,
    operation: 'claims-replay-create-v1',
    operationDigest: plan.custodyRequestDigestHex,
    intent: canonicalReplayIntent(request),
    plan: canonicalReplayPlan(plan),
  });
}

export function authenticateClaimsReplayJournalV1(
  journal: ClientOperationJournalV1,
  request: ClaimsCustodyReplayRequestV1,
  plan: ClaimsCustodyReplayPlanV1,
): void {
  const expected = claimsReplayJournalInputV1(journal, request, plan);
  if (journal.operation !== expected.operation || journal.operationDigest !== expected.operationDigest
      || journal.intent !== expected.intent || journal.plan !== expected.plan) {
    throw new Error('saved Claims replay plan differs from the freshly authenticated finalized route');
  }
}

export function claimsReplayFinalizedCompletionV1(
  status: SignatureStatusObservation | undefined,
  state: ClaimsCustodyReplayStateV1,
): state is Extract<ClaimsCustodyReplayStateV1, { status: 'exists' }> {
  return status?.known === true && status.succeeded === true
    && status.confirmationStatus === 'finalized' && state.status === 'exists';
}

export function requireTerminalPayoutRouteScopeV1(
  journal: ClientOperationJournalV1,
  manifest: WalletTerminalPayoutManifestV3,
  expected: Readonly<{ market: string; position: string; owner: string; claimIndex: number }>,
): void {
  const request = manifest.request;
  if (journal.operation !== 'wallet-terminal-payout-v3' || journal.market !== expected.market || journal.owner !== expected.owner
      || request.market !== expected.market || request.position !== expected.position
      || request.owner !== expected.owner || request.claimIndex !== expected.claimIndex) {
    throw new Error('saved payout substitutes the current Market, Position, owner, or winning claim');
  }
}

function canonicalPayoutIntent(manifest: WalletTerminalPayoutManifestV3): string {
  return JSON.stringify(manifest);
}

function canonicalPayoutPlan(plan: PreparedWalletTerminalPayoutV3): string {
  return JSON.stringify(Object.freeze({
    format: PAYOUT_PLAN_FORMAT,
    observedSlot: plan.report.observedSlot,
    lookupTable: plan.lookupTable,
    requiredSigners: plan.requiredSigners,
    unsignedWireBase64: base64(plan.wireBytes),
    aggregateBase64: base64(plan.report.preAggregateBytes),
    positionBase64: base64(plan.report.prePositionBytes),
    custodyReplayBase64: base64(plan.report.preCustodyReplayBytes),
    hoardTokenBase64: base64(plan.report.preHoardTokenBytes),
    recipientTokenBase64: base64(plan.report.preRecipientTokenBytes),
  }));
}

export function terminalPayoutJournalInputV1(
  scope: ClientOperationScopeV1,
  manifest: WalletTerminalPayoutManifestV3,
  plan: PreparedWalletTerminalPayoutV3,
): UnsignedJournalInputV1 {
  if (scope.market !== manifest.request.market || scope.owner !== manifest.request.owner
      || plan.lookupTable !== manifest.lookupTable) throw new Error('terminal payout journal scope differs from its exact route');
  return Object.freeze({
    ...scope,
    operation: 'wallet-terminal-payout-v3',
    operationDigest: walletTerminalPayoutSummaryV3(plan.report).requestDigest,
    intent: canonicalPayoutIntent(manifest),
    plan: canonicalPayoutPlan(plan),
  });
}

/** Rebuild the exact verifier input from storage without trusting it as chain state. */
export async function restoreTerminalPayoutJournalV1(
  journal: ClientOperationJournalV1,
): Promise<Readonly<{ manifest: WalletTerminalPayoutManifestV3; plan: PreparedWalletTerminalPayoutV3 }>> {
  if (journal.operation !== 'wallet-terminal-payout-v3') throw new Error('saved operation is not one terminal payout');
  const manifest = parseWalletTerminalPayoutManifestV3(journal.intent);
  if (manifest.request.market !== journal.market || manifest.request.owner !== journal.owner) {
    throw new Error('saved payout intent substitutes the Market or owner');
  }
  const value = parsePlan(journal.plan, 'saved payout verifier plan');
  exactKeys(value, [
    'format', 'observedSlot', 'lookupTable', 'requiredSigners', 'unsignedWireBase64',
    'aggregateBase64', 'positionBase64', 'custodyReplayBase64', 'hoardTokenBase64', 'recipientTokenBase64',
  ], 'saved payout verifier plan');
  if (value.format !== PAYOUT_PLAN_FORMAT || typeof value.observedSlot !== 'string'
      || value.lookupTable !== manifest.lookupTable || !Array.isArray(value.requiredSigners)
      || value.requiredSigners.some((signer) => typeof signer !== 'string')) {
    throw new Error('saved payout verifier plan has another format, slot, table, or signer shape');
  }
  const savedRequiredSigners = value.requiredSigners as string[];
  const wireBytes = base64Bytes(value.unsignedWireBase64, 'saved unsigned payout transaction');
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wireBytes); } catch { throw new Error('saved payout transaction is not one canonical Solana transaction'); }
  if (base64(transaction.serialize()) !== value.unsignedWireBase64) throw new Error('saved payout transaction is not canonical wire bytes');
  const requiredSigners = transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures).map((address) => address.toBase58());
  if (requiredSigners.length !== savedRequiredSigners.length
      || requiredSigners.some((signer, index) => signer !== savedRequiredSigners[index])) {
    throw new Error('saved payout transaction substitutes its signer set');
  }
  const signedPacketBase64 = manifest.signedPacketBase64;
  const report = await buildWalletTerminalPayoutV3({
    observedSlot: value.observedSlot,
    route: manifest.route,
    custodyContext: manifest.custodyContext,
    request: manifest.request,
    signedPacket: base64Bytes(signedPacketBase64, 'saved SignedDelta packet'),
    payout: manifest.payout,
    aggregateBytes: base64Bytes(value.aggregateBase64, 'saved Claims aggregate prestate'),
    positionBytes: base64Bytes(value.positionBase64, 'saved Position prestate'),
    custodyReplayBytes: base64Bytes(value.custodyReplayBase64, 'saved Custody replay prestate'),
    hoardTokenBytes: base64Bytes(value.hoardTokenBase64, 'saved Hoard prestate'),
    recipientTokenBytes: base64Bytes(value.recipientTokenBase64, 'saved recipient prestate'),
  });
  if (walletTerminalPayoutSummaryV3(report).requestDigest !== journal.operationDigest) {
    throw new Error('saved payout verifier plan substitutes the operation digest');
  }
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (base64(signed.message.serialize()) !== base64(transaction.message.serialize())
        || signed.signatures.length !== requiredSigners.length) {
      throw new Error('saved signed payout packet substitutes its exact unsigned message or signer set');
    }
  }
  return Object.freeze({
    manifest,
    plan: Object.freeze({ transaction, wireBytes, requiredSigners: Object.freeze(requiredSigners), report, lookupTable: manifest.lookupTable }),
  });
}

function payoutSemantics(plan: PreparedWalletTerminalPayoutV3): string {
  return JSON.stringify(Object.freeze({
    lookupTable: plan.lookupTable,
    requiredSigners: plan.requiredSigners,
    requestDigest: walletTerminalPayoutSummaryV3(plan.report).requestDigest,
    aggregateBase64: base64(plan.report.preAggregateBytes),
    positionBase64: base64(plan.report.prePositionBytes),
    custodyReplayBase64: base64(plan.report.preCustodyReplayBytes),
    hoardTokenBase64: base64(plan.report.preHoardTokenBytes),
    recipientTokenBase64: base64(plan.report.preRecipientTokenBytes),
  }));
}

export async function authenticateUnsignedTerminalPayoutJournalV1(
  journal: ClientOperationJournalV1,
  manifest: WalletTerminalPayoutManifestV3,
  plan: PreparedWalletTerminalPayoutV3,
): Promise<void> {
  const expected = terminalPayoutJournalInputV1(journal, manifest, plan);
  const restored = await restoreTerminalPayoutJournalV1(journal);
  if (journal.operation !== expected.operation || journal.operationDigest !== expected.operationDigest
      || journal.intent !== expected.intent || payoutSemantics(restored.plan) !== payoutSemantics(plan)) {
    throw new Error('saved payout plan differs from the freshly authenticated finalized route');
  }
}
