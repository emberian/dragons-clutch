import { PublicKey } from '@solana/web3.js';

import { ascii, hex, isZero, requireNonzero, requireZero, slice, u16, u64 } from './bytes';
import * as Abi from './generated/aggregateRetirementV1';
import {
  decodeClaimsAggregateV2,
  deriveClaimsAggregateAddressV2,
  type MarketCorePhaseV2,
} from './marketCoreV2';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export type AggregateRetirementPhaseV1 =
  | 'ClaimsClosed'
  | 'HoardVaultClosed'
  | 'CustodyReplayClosed';

export type AggregateRetirementCheckpointV1 = Readonly<{
  phase: AggregateRetirementPhaseV1;
  corePrestateDigest: string;
  bundleDigest: string;
  phaseJoinDigest: string;
  claimsReceiptDigest: string;
  closeVaultReceiptDigest: string | null;
  closeReplayReceiptDigest: string | null;
  claimsRefundLamports: string;
  custodyRefundLamports: string;
  generation: string;
  claimsRevision: string;
  custodyRevision: string;
  phaseRevision: string;
}>;

export type AggregateRetirementNextStepV1 =
  | 'prepare'
  | 'close-vault'
  | 'close-replay'
  | 'finish'
  | 'none';

export type AggregateRetirementInspectionV1 = Readonly<{
  status: 'not-admitted' | 'blocked-liabilities' | 'operator-required' | 'in-progress' | 'complete' | 'refused';
  marketAddress: string;
  aggregateAddress: string;
  observedSlot: string;
  nextStep: AggregateRetirementNextStepV1;
  checkpoint: AggregateRetirementCheckpointV1 | null;
  nonzeroClaimCount: number | null;
  browserAction: 'disabled';
  reason: string;
}>;

export type AggregateRetirementInspectionRequestV1 = Readonly<{
  coreProgramId: string;
  claimsProgramId: string;
  marketAddress: string;
  marketPhase: MarketCorePhaseV2;
  marketGeneration: string;
  minimumContextSlot: string;
}>;

type RetirementReaderV1 = Pick<SolanaRpcClient, 'multipleAccounts'>;

function canonicalAddress(value: string, field: string): string {
  let canonical: string;
  try {
    canonical = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (canonical !== value) throw new Error(`${field} must use canonical base58 text`);
  return canonical;
}

function canonicalPositive(value: string, field: string): bigint {
  if (!/^[1-9][0-9]*$/.test(value)) throw new Error(`${field} must be one positive decimal integer`);
  return BigInt(value);
}

function canonicalSlot(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be one canonical slot`);
  return BigInt(value);
}

function phase(tag: number): AggregateRetirementPhaseV1 {
  switch (tag) {
    case Abi.AGGREGATE_RETIREMENT_PHASE_CLAIMS_CLOSED_V1: return 'ClaimsClosed';
    case Abi.AGGREGATE_RETIREMENT_PHASE_HOARD_VAULT_CLOSED_V1: return 'HoardVaultClosed';
    case Abi.AGGREGATE_RETIREMENT_PHASE_CUSTODY_REPLAY_CLOSED_V1: return 'CustodyReplayClosed';
    default: throw new Error('aggregate retirement checkpoint has an unknown phase');
  }
}

function exactDigest(bytes: Uint8Array, offset: number, field: string): string {
  const digest = slice(bytes, offset, 32);
  requireNonzero(digest, field);
  return hex(digest);
}

function optionalDigest(bytes: Uint8Array, offset: number): string | null {
  const digest = slice(bytes, offset, 32);
  return isZero(digest) ? null : hex(digest);
}

/** Hostile-decode the exact Core-owned `DCLTARC1` checkpoint ABI. */
export function decodeAggregateRetirementCheckpointV1(bytes: Uint8Array): AggregateRetirementCheckpointV1 {
  if (bytes.length !== Abi.AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1
      || ascii(bytes, Abi.AGGREGATE_RETIREMENT_MAGIC_OFFSET_V1, Abi.AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1.length)
        !== Abi.AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1
      || u16(bytes, Abi.AGGREGATE_RETIREMENT_VERSION_OFFSET_V1)
        !== Abi.AGGREGATE_RETIREMENT_CHECKPOINT_VERSION_V1) {
    throw new Error('aggregate retirement checkpoint has the wrong exact ABI');
  }
  requireZero(
    bytes,
    Abi.AGGREGATE_RETIREMENT_RESERVED_OFFSET_V1,
    Abi.AGGREGATE_RETIREMENT_RESERVED_BYTES_V1,
    'aggregate retirement checkpoint header',
  );
  const decodedPhase = phase(bytes[Abi.AGGREGATE_RETIREMENT_PHASE_OFFSET_V1] ?? 0);
  const closeVaultReceiptDigest = optionalDigest(bytes, Abi.VAULT_RECEIPT_OFFSET);
  const closeReplayReceiptDigest = optionalDigest(bytes, Abi.REPLAY_RECEIPT_OFFSET);
  const claimsRefundLamports = u64(bytes, Abi.CLAIMS_REFUND_OFFSET);
  const custodyRefundLamports = u64(bytes, Abi.CUSTODY_REFUND_OFFSET);
  const generation = u64(bytes, Abi.GENERATION_OFFSET);
  const claimsRevision = u64(bytes, Abi.CLAIMS_REVISION_OFFSET);
  const custodyRevision = u64(bytes, Abi.CUSTODY_REVISION_OFFSET);
  const phaseRevision = u64(bytes, Abi.PHASE_REVISION_OFFSET);
  if (claimsRefundLamports === 0n || generation === 0n || claimsRevision === 0n || custodyRevision === 0n
      || phaseRevision !== BigInt(bytes[Abi.AGGREGATE_RETIREMENT_PHASE_OFFSET_V1] ?? 0)) {
    throw new Error('aggregate retirement checkpoint has a noncanonical coordinate or revision');
  }
  if (decodedPhase === 'ClaimsClosed'
      && (closeVaultReceiptDigest !== null || closeReplayReceiptDigest !== null || custodyRefundLamports !== 0n)) {
    throw new Error('ClaimsClosed retirement checkpoint carries inactive Custody effects');
  }
  if (decodedPhase === 'HoardVaultClosed'
      && (closeVaultReceiptDigest === null || closeReplayReceiptDigest !== null || custodyRefundLamports === 0n)) {
    throw new Error('HoardVaultClosed retirement checkpoint has a noncanonical effect history');
  }
  if (decodedPhase === 'CustodyReplayClosed'
      && (closeVaultReceiptDigest === null || closeReplayReceiptDigest === null || custodyRefundLamports === 0n)) {
    throw new Error('CustodyReplayClosed retirement checkpoint has a noncanonical effect history');
  }
  return Object.freeze({
    phase: decodedPhase,
    corePrestateDigest: exactDigest(bytes, Abi.CORE_PRESTATE_OFFSET, 'aggregate retirement Core prestate'),
    bundleDigest: exactDigest(bytes, Abi.BUNDLE_DIGEST_OFFSET, 'aggregate retirement bundle'),
    phaseJoinDigest: exactDigest(bytes, Abi.CLAIMS_CONTEXT_OFFSET, 'aggregate retirement phase join'),
    claimsReceiptDigest: exactDigest(bytes, Abi.CLAIMS_RECEIPT_OFFSET, 'aggregate retirement Claims receipt'),
    closeVaultReceiptDigest,
    closeReplayReceiptDigest,
    claimsRefundLamports: claimsRefundLamports.toString(),
    custodyRefundLamports: custodyRefundLamports.toString(),
    generation: generation.toString(),
    claimsRevision: claimsRevision.toString(),
    custodyRevision: custodyRevision.toString(),
    phaseRevision: phaseRevision.toString(),
  });
}

function nextStep(checkpoint: AggregateRetirementCheckpointV1): AggregateRetirementNextStepV1 {
  switch (checkpoint.phase) {
    case 'ClaimsClosed': return 'close-vault';
    case 'HoardVaultClosed': return 'close-replay';
    case 'CustodyReplayClosed': return 'finish';
  }
}

function accountShape(account: RpcAccount, field: string): void {
  if (account.executable) throw new Error(`${field} is unexpectedly executable`);
  if (account.space !== account.data.length) throw new Error(`${field} RPC width disagrees with its decoded bytes`);
}

/**
 * Inspect the one derived Claims aggregate / Core checkpoint retirement seam.
 *
 * This is read-only. It deliberately exposes no constructor, wallet callback,
 * or submission hook: the accepted route is four durable operator mutations,
 * and the browser does not own their original bundle or crash journals.
 */
export async function inspectAggregateRetirementV1(
  client: RetirementReaderV1,
  request: AggregateRetirementInspectionRequestV1,
): Promise<AggregateRetirementInspectionV1> {
  const coreProgramId = canonicalAddress(request.coreProgramId, 'Core program');
  const claimsProgramId = canonicalAddress(request.claimsProgramId, 'Claims program');
  const marketAddress = canonicalAddress(request.marketAddress, 'Market');
  if (coreProgramId === claimsProgramId) throw new Error('Core and Claims program identities alias');
  const generation = canonicalPositive(request.marketGeneration, 'Market generation');
  const floor = canonicalSlot(request.minimumContextSlot, 'retirement observation floor');
  const aggregateAddress = deriveClaimsAggregateAddressV2(claimsProgramId, marketAddress);
  const base = Object.freeze({ marketAddress, aggregateAddress, checkpoint: null, nonzeroClaimCount: null, browserAction: 'disabled' as const });

  if (request.marketPhase !== 'Retiring' && request.marketPhase !== 'Retired') {
    return Object.freeze({
      ...base,
      status: 'not-admitted',
      observedSlot: request.minimumContextSlot,
      nextStep: 'none',
      reason: `You cannot retire this Market while its finalized phase is ${request.marketPhase}. Redemption and liability closure must reach the onchain Retiring prestate first.`,
    });
  }

  const batch = await client.multipleAccounts([aggregateAddress], request.minimumContextSlot);
  const observedSlot = canonicalSlot(batch.slot, 'retirement account observation slot');
  if (observedSlot < floor) throw new Error('retirement account observation regressed below the Market floor');
  if (batch.accounts.length !== 1 || batch.accounts[0]?.address !== aggregateAddress) {
    throw new Error('retirement account read did not return the one derived address requested');
  }
  const account = batch.accounts[0].account;

  if (request.marketPhase === 'Retired') {
    if (account !== null) {
      return Object.freeze({ ...base, status: 'refused', observedSlot: batch.slot, nextStep: 'none', reason: 'The Market says Retired but its derived aggregate/checkpoint account still exists. You should not treat either fact as completion.' });
    }
    return Object.freeze({ ...base, status: 'complete', observedSlot: batch.slot, nextStep: 'none', reason: 'The Market is Retired and its derived aggregate/checkpoint account is absent at the same finalized floor. No further Market action is admitted.' });
  }

  if (account === null) {
    return Object.freeze({ ...base, status: 'refused', observedSlot: batch.slot, nextStep: 'none', reason: 'This Retiring Market has no derived Claims aggregate or Core checkpoint at the finalized floor, so you cannot infer a retirement phase.' });
  }
  try {
    accountShape(account, 'retirement aggregate/checkpoint');
    if (account.owner === claimsProgramId) {
      const aggregate = decodeClaimsAggregateV2(aggregateAddress, account.data);
      if (aggregate.logicalMarket !== marketAddress || BigInt(aggregate.generation) !== generation) {
        throw new Error('Claims aggregate does not join this Market incarnation');
      }
      const nonzeroClaimCount = aggregate.supplyAtoms.filter((value) => BigInt(value) !== 0n).length;
      if (nonzeroClaimCount !== 0) {
        return Object.freeze({
          ...base,
          status: 'blocked-liabilities',
          observedSlot: batch.slot,
          nextStep: 'none',
          nonzeroClaimCount,
          reason: `You cannot prepare aggregate retirement: ${nonzeroClaimCount} ${nonzeroClaimCount === 1 ? 'claim entry is' : 'claim entries are'} still nonzero. Retirement never writes through a nonzero liability vector, and this reader does not add distinct claim supplies into one misleading total.`,
        });
      }
      return Object.freeze({
        ...base,
        status: 'operator-required',
        observedSlot: batch.slot,
        nextStep: 'prepare',
        nonzeroClaimCount: 0,
        reason: 'The Claims aggregate is empty and still Claims-owned. That proves the visible liability precondition only. You need the Rust-authored four-step campaign, its original bundle, checked release, and durable journals; this browser cannot start it.',
      });
    }
    if (account.owner === coreProgramId) {
      const checkpoint = decodeAggregateRetirementCheckpointV1(account.data);
      if (BigInt(checkpoint.generation) !== generation) throw new Error('retirement checkpoint belongs to another Market generation');
      if (checkpoint.claimsRefundLamports !== account.lamports) throw new Error('retirement checkpoint lamports differ from its persisted Claims refund');
      return Object.freeze({
        ...base,
        status: 'in-progress',
        observedSlot: batch.slot,
        nextStep: nextStep(checkpoint),
        checkpoint,
        reason: `Retirement is durably paused after ${checkpoint.phase}. You need the Rust-authored campaign and its saved journal to authenticate and resume ${nextStep(checkpoint)}; this browser will not reconstruct those mutation bytes.`,
      });
    }
    throw new Error('derived retirement account is owned by neither selected Claims nor selected Core');
  } catch (error) {
    return Object.freeze({
      ...base,
      status: 'refused',
      observedSlot: batch.slot,
      nextStep: 'none',
      reason: `Retirement state refused: ${error instanceof Error ? error.message : 'the derived account failed without a usable reason'}.`,
    });
  }
}
