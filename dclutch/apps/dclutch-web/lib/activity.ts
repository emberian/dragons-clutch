import { PublicKey, VersionedTransaction } from '@solana/web3.js';

import { deriveClaimsAggregateAddressV2, deriveClaimsPositionAddressV2 } from './marketCoreV2';
import { type SignatureRecordObservation, type SolanaRpcClient, type TransactionMetaObservation } from './rpc';

/**
 * Activity without an indexer.
 *
 * dClutch publishes no event index and this browser will not invent one. What a
 * plain RPC node does keep is its own per-address signature history, and that is
 * exactly what this surface reads: the node's `getSignaturesForAddress` answer
 * for the owner's wallet plus the Claims Position addresses derived from the
 * Markets the reader named — the same derivation the portfolio uses, so the two
 * surfaces can never disagree about where a Position lives.
 *
 * Provenance is stated, not blurred: every row is the node's history for an
 * address this browser explicitly derived and named, decoded from the finalized
 * transaction bytes the node returned. A node configured without history
 * answers with an empty list, and that is reported as the node's answer — never
 * as "no activity ever happened".
 */

export const ACTIVITY_MAX_MARKETS = 8;
export const ACTIVITY_SIGNATURES_PER_ADDRESS = 20;
export const ACTIVITY_MAX_TRANSACTIONS = 24;

/** Program labels this surface may assert without a user-supplied selection. */
const WELL_KNOWN_PROGRAMS: ReadonlyMap<string, string> = new Map([
  ['11111111111111111111111111111111', 'System Program'],
  ['Ed25519SigVerify111111111111111111111111111', 'Ed25519 signature verification'],
  ['ComputeBudget111111111111111111111111111111', 'Compute budget'],
  ['AddressLookupTab1e1111111111111111111111111', 'Address lookup table'],
  ['TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb', 'Token-2022'],
  ['TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA', 'Token'],
  ['Sysvar1nstructions1111111111111111111111111', 'Instructions sysvar'],
  ['SysvarRent111111111111111111111111111111111', 'Rent sysvar'],
]);

export type ActivityRoleLabelsV1 = Readonly<Record<string, string>>;

export type ActivityWatchedAddressV1 = Readonly<{
  address: string;
  meaning: string;
}>;

export type ActivityProgramTouchV1 = Readonly<{
  address: string;
  label: string | null;
}>;

export type ActivityEntryV1 = Readonly<{
  signature: string;
  slot: string;
  blockTime: string | null;
  succeeded: boolean;
  errorText: string | null;
  watchedAddresses: ReadonlyArray<ActivityWatchedAddressV1>;
  programs: ReadonlyArray<ActivityProgramTouchV1>;
  feeLamports: string | null;
  ownerLamportDelta: string | null;
  detail:
    | Readonly<{ status: 'decoded' }>
    | Readonly<{ status: 'refused'; reason: string }>;
}>;

export type ActivityV1 = Readonly<{
  owner: string;
  watched: ReadonlyArray<ActivityWatchedAddressV1>;
  entries: ReadonlyArray<ActivityEntryV1>;
  truncated: boolean;
  reason: string;
}>;

export type ActivityRequestV1 = Readonly<{
  owner: string;
  claimsProgramId?: string | null;
  marketAddresses?: ReadonlyArray<string>;
  programLabels?: ActivityRoleLabelsV1;
}>;

function canonical(value: string, field: string): string {
  let key: string;
  try {
    key = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (key !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

/** The signed difference of two exact unsigned decimal lamport strings. */
export function lamportDeltaV1(before: string, after: string): string {
  const delta = BigInt(after) - BigInt(before);
  return delta > 0n ? `+${delta}` : delta.toString();
}

function programTouches(
  observation: TransactionMetaObservation,
  labels: ReadonlyMap<string, string>,
): ReadonlyArray<ActivityProgramTouchV1> {
  let message: VersionedTransaction;
  try {
    message = VersionedTransaction.deserialize(observation.transactionBytes);
  } catch {
    return Object.freeze([]);
  }
  const seen = new Set<number>();
  const touches: ActivityProgramTouchV1[] = [];
  for (const instruction of message.message.compiledInstructions) {
    if (seen.has(instruction.programIdIndex)) continue;
    seen.add(instruction.programIdIndex);
    const address = observation.accountAddresses[instruction.programIdIndex];
    if (address === undefined) continue;
    touches.push(Object.freeze({ address, label: labels.get(address) ?? WELL_KNOWN_PROGRAMS.get(address) ?? null }));
  }
  return Object.freeze(touches);
}

/**
 * Read the node's recent signature history for one owner across the Markets
 * they named, then decode each finalized transaction the node still holds.
 */
export async function inspectActivityV1(
  client: Pick<SolanaRpcClient, 'signaturesForAddress' | 'transaction'>,
  request: ActivityRequestV1,
): Promise<ActivityV1> {
  const owner = canonical(request.owner, 'owner address');
  const claimsProgramId = request.claimsProgramId === undefined || request.claimsProgramId === null || request.claimsProgramId === ''
    ? null
    : canonical(request.claimsProgramId, 'Claims program');
  const marketAddresses = Object.freeze([...new Set((request.marketAddresses ?? []).map((address, index) => canonical(address, `Market address ${index + 1}`)))]);
  if (marketAddresses.length > ACTIVITY_MAX_MARKETS) {
    throw new Error(`activity requested ${marketAddresses.length} Markets, above the explicit ${ACTIVITY_MAX_MARKETS}-Market browser bound`);
  }

  const labels = new Map<string, string>();
  for (const [address, label] of Object.entries(request.programLabels ?? {})) {
    labels.set(canonical(address, `labeled program ${label}`), label);
  }

  const watched: ActivityWatchedAddressV1[] = [Object.freeze({ address: owner, meaning: 'owner wallet' })];
  if (claimsProgramId !== null) {
    for (const market of marketAddresses) {
      const aggregate = deriveClaimsAggregateAddressV2(claimsProgramId, market);
      const position = deriveClaimsPositionAddressV2(claimsProgramId, aggregate, owner);
      if (position !== owner) {
        watched.push(Object.freeze({ address: position, meaning: `derived Claims Position for Market ${market}` }));
      }
    }
  }

  const bySignature = new Map<string, Readonly<{ record: SignatureRecordObservation; touched: ActivityWatchedAddressV1[] }>>();
  for (const target of watched) {
    const records = await client.signaturesForAddress(target.address, ACTIVITY_SIGNATURES_PER_ADDRESS);
    for (const record of records) {
      const existing = bySignature.get(record.signature);
      if (existing === undefined) {
        bySignature.set(record.signature, Object.freeze({ record, touched: [target] }));
      } else {
        existing.touched.push(target);
      }
    }
  }

  const ordered = [...bySignature.values()].sort((left, right) => {
    const bySlot = BigInt(right.record.slot) - BigInt(left.record.slot);
    if (bySlot !== 0n) return bySlot > 0n ? 1 : -1;
    return left.record.signature.localeCompare(right.record.signature);
  });
  const truncated = ordered.length > ACTIVITY_MAX_TRANSACTIONS;
  const selected = ordered.slice(0, ACTIVITY_MAX_TRANSACTIONS);

  const entries: ActivityEntryV1[] = [];
  for (const { record, touched } of selected) {
    let observation: TransactionMetaObservation | null;
    try {
      observation = await client.transaction(record.signature);
    } catch (error) {
      entries.push(Object.freeze({
        signature: record.signature,
        slot: record.slot,
        blockTime: record.blockTime,
        succeeded: record.succeeded,
        errorText: record.errorText,
        watchedAddresses: Object.freeze([...touched]),
        programs: Object.freeze([]),
        feeLamports: null,
        ownerLamportDelta: null,
        detail: Object.freeze({ status: 'refused' as const, reason: error instanceof Error ? error.message : 'the finalized transaction read refused without a usable reason' }),
      }));
      continue;
    }
    if (observation === null) {
      entries.push(Object.freeze({
        signature: record.signature,
        slot: record.slot,
        blockTime: record.blockTime,
        succeeded: record.succeeded,
        errorText: record.errorText,
        watchedAddresses: Object.freeze([...touched]),
        programs: Object.freeze([]),
        feeLamports: null,
        ownerLamportDelta: null,
        detail: Object.freeze({ status: 'refused' as const, reason: 'the node lists this signature but no longer serves its transaction' }),
      }));
      continue;
    }
    const ownerIndex = observation.accountAddresses.indexOf(owner);
    const pre = ownerIndex >= 0 ? observation.preBalances[ownerIndex] : undefined;
    const post = ownerIndex >= 0 ? observation.postBalances[ownerIndex] : undefined;
    entries.push(Object.freeze({
      signature: record.signature,
      slot: observation.slot,
      blockTime: observation.blockTime ?? record.blockTime,
      succeeded: observation.succeeded,
      errorText: observation.errorText,
      watchedAddresses: Object.freeze([...touched]),
      programs: programTouches(observation, labels),
      feeLamports: observation.feeLamports,
      ownerLamportDelta: pre !== undefined && post !== undefined ? lamportDeltaV1(pre, post) : null,
      detail: Object.freeze({ status: 'decoded' as const }),
    }));
  }

  return Object.freeze({
    owner,
    watched: Object.freeze(watched),
    entries: Object.freeze(entries),
    truncated,
    reason: entries.length === 0
      ? `The node reports no signature history for ${watched.length} watched address${watched.length === 1 ? '' : 'es'}. That is this node's answer, not a protocol fact: a node without transaction history answers empty for every address.`
      : `${entries.length} finalized transaction${entries.length === 1 ? '' : 's'} across ${watched.length} watched address${watched.length === 1 ? '' : 'es'}, newest first, from this node's own signature history.`,
  });
}
