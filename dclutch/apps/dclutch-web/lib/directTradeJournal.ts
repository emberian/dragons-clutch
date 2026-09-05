import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  type ClientOperationScopeV1,
  type ClientOperationJournalV1,
} from './clientOperationJournal';
import { type SignatureStatusObservation } from '@dclutch/sdk/rpc';

/**
 * The Direct crossing's durable-submission seam: the journal input that must
 * be written before the wallet is asked to sign, and the pure judgments the
 * panel makes about a submitted packet afterward.
 *
 * The journal machinery itself (single-writer key, signature match on resume,
 * refuse-to-replay) is `lib/clientOperationJournal.ts`, shared with
 * redemption. This module only states what a Direct crossing's intent, plan,
 * and completion ARE, so those statements can be tested as the pure functions
 * they are.
 */

export type DirectTradeJournalPlanV1 = Readonly<{
  payer: string;
  lookupTable: string;
  routeObservedSlot: string;
  blockhashObservedSlot: string;
  lastValidBlockHeight: string;
  messageBase64: string;
}>;

function canonicalPlanText(plan: DirectTradeJournalPlanV1): string {
  return JSON.stringify(Object.freeze({
    schema: 'dclutch-direct-inline-journal-plan-v1',
    payer: plan.payer,
    lookupTable: plan.lookupTable,
    routeObservedSlot: plan.routeObservedSlot,
    blockhashObservedSlot: plan.blockhashObservedSlot,
    lastValidBlockHeight: plan.lastValidBlockHeight,
    messageBase64: plan.messageBase64,
  }));
}

/** The unsigned journal input for one exact prepared Direct packet. */
export async function directInlineJournalInputV1(
  scope: ClientOperationScopeV1,
  takerTicket: string,
  plan: DirectTradeJournalPlanV1,
  messageBytes: Uint8Array,
): Promise<ClientOperationScopeV1 & Readonly<{
  operation: 'direct-inline-v3';
  operationDigest: string;
  intent: string;
  plan: string;
}>> {
  if (takerTicket.trim() === '') throw new Error('a Direct journal needs the signed taker ticket as its intent');
  if (messageBytes.length === 0) throw new Error('a Direct journal needs the exact prepared message bytes');
  return Object.freeze({
    ...scope,
    operation: 'direct-inline-v3' as const,
    operationDigest: hex(await sha256(messageBytes)),
    intent: takerTicket,
    plan: canonicalPlanText(plan),
  });
}

/** A submitted Direct packet is complete only at finalized success. */
export function directTradeFinalizedCompletionV1(
  status: SignatureStatusObservation | undefined,
): boolean {
  return status !== undefined && status.known && status.succeeded === true
    && status.confirmationStatus === 'finalized';
}

export type DirectTradeBalanceSnapshotV1 = Readonly<{
  positionBalances: ReadonlyArray<bigint>;
  spendableCollateralAtoms: bigint;
}>;

export type DirectTradeBalanceChangeV1 = Readonly<{
  claimIndex: number;
  before: bigint;
  after: bigint;
}>;

/**
 * Exact per-claim changes between two finalized readings of one Position.
 * An executed crossing must move at least one claim balance; a submission
 * that changed nothing is reported as exactly that, never smoothed over.
 */
export function directTradeBalanceChangesV1(
  before: DirectTradeBalanceSnapshotV1,
  after: DirectTradeBalanceSnapshotV1,
): Readonly<{
  claims: ReadonlyArray<DirectTradeBalanceChangeV1>;
  spendableBefore: bigint;
  spendableAfter: bigint;
  moved: boolean;
}> {
  if (before.positionBalances.length !== after.positionBalances.length) {
    throw new Error('the Position claim vector changed width between readings; these are not the same Product');
  }
  const claims: DirectTradeBalanceChangeV1[] = [];
  let moved = false;
  for (let index = 0; index < before.positionBalances.length; index += 1) {
    const beforeAtoms = before.positionBalances[index]!;
    const afterAtoms = after.positionBalances[index]!;
    if (beforeAtoms !== afterAtoms) moved = true;
    claims.push(Object.freeze({ claimIndex: index, before: beforeAtoms, after: afterAtoms }));
  }
  if (before.spendableCollateralAtoms !== after.spendableCollateralAtoms) moved = true;
  return Object.freeze({
    claims: Object.freeze(claims),
    spendableBefore: before.spendableCollateralAtoms,
    spendableAfter: after.spendableCollateralAtoms,
    moved,
  });
}

/** Reader-facing one-liner for one claim's movement. */
export function describeClaimChangeV1(change: DirectTradeBalanceChangeV1): string {
  if (change.before === change.after) return `claim ${change.claimIndex}: unchanged at ${change.after} atoms`;
  const direction = change.after > change.before ? 'gained' : 'gave up';
  const magnitude = change.after > change.before ? change.after - change.before : change.before - change.after;
  return `claim ${change.claimIndex}: ${direction} ${magnitude} atoms (${change.before} → ${change.after})`;
}

export type { ClientOperationJournalV1 };
