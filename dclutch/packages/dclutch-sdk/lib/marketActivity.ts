import { PublicKey, VersionedTransaction } from '@solana/web3.js';

import { ascii, pubkey, slice, u16, u64 } from './bytes';
import { previewDirectInlineV3, type CompactIntentV2Input, type DirectInlineEconomicPreviewV3 } from './directInlineV3';
import { inspectDirectMakerNonceV1, inspectDirectMakerNoncePairV1 } from './directMakerReplay';
import * as DirectAbi from './generated/directInlineV3';
import { LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_MAGIC_V2 } from './generated/coreFound';
import { INSTRUCTION_MAGICS } from './generated/routeCensus';
import { decodeClaimsPositionV2, type ClaimsPositionV2 } from './marketCoreV2';
import { type SolanaRpcClient } from './rpc';

/**
 * WHAT HAPPENED ON THIS MARKET, derived from the market's own records.
 *
 * dClutch publishes no trade index and there is no per-fill account to read: a
 * Direct crossing writes two Claims Positions, two maker replays and the
 * collateral legs, and the TERMS it crossed on live in the transaction that
 * carried it — signed by both parties, at coordinates the Direct ABI names.
 * So this module derives a market's activity the only way the chain supports:
 *
 *   1. the node's own signature history for the Market address;
 *   2. each finalized transaction's bytes, decoded at the generated Direct V3
 *      offsets, which gives outcome, fill, execution price, both signed limits,
 *      both makers and the fee rate they signed;
 *   3. `previewDirectInlineV3` — the SAME function the trade stepper previews
 *      an unsent fill with — for gross, both fees and both net legs, so a
 *      historical crossing and a proposed one can never be computed by two
 *      different rules;
 *   4. the two Positions and the two maker replays, read LIVE, for where those
 *      claims sit now and whether the venue's fee is still owed.
 *
 * NOTHING HERE IS A SECOND DECODER. Every offset, width and magic comes from
 * `lib/generated/directInlineV3.ts` (emitted from the Rust that writes them) or
 * from `lib/generated/routeCensus.ts` (emitted from `dclutch-route-census
 * inventory`). The one thing this file adds is the INVERSE of an encoder the
 * tree already owns: `encodeCompactIntentV2` writes a signed intent at those
 * coordinates and nothing read one back, because until today nobody had a
 * reason to read a crossing that had already happened.
 *
 * WHAT IT CANNOT NAME, said rather than guessed. The route census records
 * `trading/direct_fee_settlement_v1` as an entry route selected by a PREDICATE,
 * not by leading-bytes magic — so no generated table can turn a settlement
 * instruction into a name, and this module does not invent one. A Trading act
 * on this market whose leading bytes match no census magic is reported as
 * exactly that, with its accounts, and the fee's answer is taken from where the
 * protocol actually keeps it: the maker replay's `fee_owed`.
 */

/** Signatures asked of the node's history. Its own listing bound is 50. */
export const MARKET_ACTIVITY_SIGNATURES_V1 = 25;
/**
 * Transactions whose bytes are fetched and decoded. The rest are counted.
 *
 * EIGHT, and the number is a budget rather than a taste. `rpc.ts` measured the
 * public devnet endpoint's burst allowance at roughly eight heavy reads
 * (`MAX_IN_FLIGHT_READS_PER_ENDPOINT_V1`), and this surface's whole read is
 * eight transactions plus a signature listing, two Position probes, one full
 * Position read and one maker-replay pair — about thirteen round trips on a
 * page that has already spent some. Twelve put it reliably past the budget and
 * the extra four rows were the oldest ones on screen.
 */
export const MARKET_ACTIVITY_TRANSACTIONS_V1 = 8;
/** Accounts from those transactions probed for a Claims Position. */
export const MARKET_ACTIVITY_POSITION_SCAN_V1 = 64;
/** `getMultipleAccounts` will not take more than this many at once. */
const ACCOUNTS_PER_READ = 32;
/**
 * Transaction reads in flight at once.
 *
 * TWO, not twelve. The first version of this fired every read with
 * `Promise.all` and the public devnet endpoint rate-limited all twelve — which
 * arrived here as twelve identical caught exceptions, an empty fill list, and a
 * live test that passed because "no crossing yet" was one of its accepted
 * answers. The instrument was disconnected and the reading was indistinguishable
 * from a true absence. It is bounded now, and a read that still refuses carries
 * the node's own reason into its row.
 */
const TRANSACTIONS_IN_FLIGHT = 2;

async function boundedMap<T, U>(values: ReadonlyArray<T>, limit: number, mapper: (value: T) => Promise<U>): Promise<U[]> {
  const output = new Array<U>(values.length);
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, values.length) }, async () => {
    for (let index = next++; index < values.length; index = next++) output[index] = await mapper(values[index]);
  });
  await Promise.all(workers);
  return output;
}

export const MARKET_ACTIVITY_PROVENANCE_V1 =
  'Every row is the node’s own finalized history for this Market address, decoded from the transaction bytes it returned. A node kept without history answers with an empty list, and that is reported as the node’s answer — never as “nothing ever happened here”.';

// ---------------------------------------------------------------- the census

/** One entry route, named by the generated route census rather than here. */
export type MarketActivityRouteV1 = Readonly<{
  /** The eight leading bytes, as the census writes them. */
  magic: string;
  program: string;
  routeId: string;
  /** The Rust constant that owns the magic. */
  constant: string;
}>;

/**
 * Where a leading-bytes magic starts, and how wide it is.
 *
 * The width is the generated magic's own length rather than the number eight,
 * so a family that ever widened one would move this with it; the offset is
 * zero because "leading bytes" is what a selector magic IS.
 */
const MAGIC_OFFSET = 0;
const MAGIC_BYTES = DirectAbi.HOT_EXECUTION_MAGIC_V3.length;
/** One little-endian u16, which is what version and profile are written as. */
const U16_BYTES = 2;
/**
 * The Hot envelope's version and profile, derived rather than restated.
 *
 * `compileDirectInlineTransactionV3` writes them at 8 and 10 as literals and
 * the generated ABI names neither offset — the Rust that owns the envelope
 * exposes its magic, its width and the family-request offset and stops there.
 * Restating two more numbers here would make this file a third author of a
 * coordinate nobody publishes, so they are derived from the one thing that IS
 * published: the magic sits first and the two u16s follow it in order.
 */
const HOT_VERSION_OFFSET = MAGIC_OFFSET + MAGIC_BYTES;
const HOT_PROFILE_OFFSET = HOT_VERSION_OFFSET + U16_BYTES;

function censusRouteV1(magic: string, programHint: string | null): MarketActivityRouteV1 | null {
  const rows = INSTRUCTION_MAGICS.filter((entry) => entry.magic === magic);
  if (rows.length === 0) return null;
  // One magic can select a route in two programs (`DCLCRH01` is Core's and
  // Custody's). The instruction's own program decides which row is about it;
  // with no hint, a single unambiguous row is still an answer and two are not.
  const matched = programHint === null
    ? (rows.length === 1 ? rows[0] : null)
    : rows.find((entry) => entry.program === programHint) ?? null;
  if (matched === null) return null;
  return Object.freeze({
    magic: matched.magic,
    program: matched.program,
    routeId: matched.routeId,
    constant: matched.constant,
  });
}

// ------------------------------------------------- the signed intent, decoded

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function u32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

/**
 * `encodeCompactIntentV2`, read backwards.
 *
 * Every coordinate is the generated constant the encoder writes at, so the two
 * directions cannot drift apart: a change to the Rust moves both at once, and
 * `marketActivity.test.ts` pins the round trip.
 */
export function decodeCompactIntentV2(bytes: Uint8Array): CompactIntentV2Input {
  if (bytes.length !== DirectAbi.COMPACT_INTENT_BYTES_V2) {
    throw new Error(`signed intent is ${bytes.length} bytes and the ABI declares ${DirectAbi.COMPACT_INTENT_BYTES_V2}`);
  }
  if (!same(slice(bytes, DirectAbi.COMPACT_INTENT_MAGIC_OFFSET_V2, MAGIC_BYTES), DirectAbi.COMPACT_INTENT_MAGIC_V2)) {
    throw new Error('signed intent does not carry the compact-intent magic');
  }
  if (u16(bytes, DirectAbi.COMPACT_INTENT_MAGIC_OFFSET_V2 + MAGIC_BYTES) !== DirectAbi.COMPACT_INTENT_VERSION_V2) {
    throw new Error('signed intent is not exact compact-intent V2');
  }
  const side = bytes[DirectAbi.COMPACT_INTENT_SIDE_OFFSET_V2];
  const lifecycle = bytes[DirectAbi.COMPACT_INTENT_LIFECYCLE_OFFSET_V2];
  if (side !== 0 && side !== 1) throw new Error('signed intent side is neither Sell nor Buy');
  if (lifecycle !== 0 && lifecycle !== 1) throw new Error('signed intent lifecycle is neither FOK nor IOC');
  return Object.freeze({
    side,
    lifecycle,
    outcome: u32(bytes, DirectAbi.COMPACT_INTENT_OUTCOME_OFFSET_V2),
    market: pubkey(slice(bytes, DirectAbi.COMPACT_INTENT_MARKET_OFFSET_V2, 32), 'signed intent Market'),
    generation: u64(bytes, DirectAbi.COMPACT_INTENT_GENERATION_OFFSET_V2),
    nonce: u64(bytes, DirectAbi.COMPACT_INTENT_NONCE_OFFSET_V2),
    validFrom: u64(bytes, DirectAbi.COMPACT_INTENT_VALID_FROM_OFFSET_V2),
    validThrough: u64(bytes, DirectAbi.COMPACT_INTENT_VALID_THROUGH_OFFSET_V2),
    maximumFill: u64(bytes, DirectAbi.COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2),
    limitPrice: u64(bytes, DirectAbi.COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2),
    feeBasisPoints: u16(bytes, DirectAbi.COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2),
    collateralAccount: pubkey(slice(bytes, DirectAbi.COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2, 32), 'signed intent collateral account'),
  });
}

/** Both signed halves of one crossing, and the pair of numbers it crossed at. */
export type DirectInlineFillTermsV1 = Readonly<{
  seller: string;
  buyer: string;
  sellerIntent: CompactIntentV2Input;
  buyerIntent: CompactIntentV2Input;
  fillAtoms: bigint;
  executionPrice: bigint;
}>;

/**
 * The terms of one `InlineOrdinary` crossing, off the Hot instruction it rode.
 *
 * The shape is the encoder's, exactly: a 128-byte family-neutral Hot envelope,
 * then a `DCLTDRQ3` header, then two `(maker, signed preimage)` participants,
 * then the fill and the execution price. Anything that is not that shape is
 * refused by name rather than half-read.
 */
export function decodeDirectInlineOrdinaryFillV3(data: Uint8Array): DirectInlineFillTermsV1 {
  const expected = DirectAbi.HOT_EXECUTION_ENVELOPE_BYTES_V3 + DirectAbi.DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3;
  if (data.length !== expected) throw new Error(`Hot instruction is ${data.length} bytes and an InlineOrdinary fill is ${expected}`);
  if (!same(slice(data, MAGIC_OFFSET, MAGIC_BYTES), DirectAbi.HOT_EXECUTION_MAGIC_V3)
      || u16(data, HOT_VERSION_OFFSET) !== DirectAbi.HOT_EXECUTION_VERSION_V3
      || u16(data, HOT_PROFILE_OFFSET) !== DirectAbi.HOT_EXECUTION_PROFILE_V3) {
    throw new Error('instruction is not one canonical Hot V3 envelope');
  }
  const request = DirectAbi.HOT_FAMILY_REQUEST_OFFSET_V3;
  if (!same(slice(data, request, MAGIC_BYTES), DirectAbi.DIRECT_EXECUTION_REQUEST_MAGIC_V3)
      || u16(data, request + MAGIC_BYTES) !== DirectAbi.DIRECT_EXECUTION_REQUEST_VERSION_V3
      || u32(data, request + DirectAbi.DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3) !== DirectAbi.DIRECT_INLINE_ORDINARY_ACTION_V3) {
    throw new Error('the Hot envelope does not carry a Direct InlineOrdinary request');
  }
  const participants = request + DirectAbi.DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3;
  const stride = DirectAbi.DIRECT_SIGNED_PARTICIPANT_BYTES_V3;
  // A signed participant is the maker key, then the signing preimage: the
  // 32-byte domain identity and then the intent itself.
  const intentAt = (index: number): Uint8Array => slice(
    data,
    participants + index * stride + 32 + 32,
    DirectAbi.COMPACT_INTENT_BYTES_V2,
  );
  const makerAt = (index: number): string => pubkey(slice(data, participants + index * stride, 32), 'Direct maker');
  const tail = participants + 2 * stride;
  const terms = Object.freeze({
    seller: makerAt(0),
    buyer: makerAt(1),
    sellerIntent: decodeCompactIntentV2(intentAt(0)),
    buyerIntent: decodeCompactIntentV2(intentAt(1)),
    fillAtoms: u64(data, tail),
    executionPrice: u64(data, tail + 8),
  });
  if (terms.seller === terms.buyer) throw new Error('a crossing cannot have one identity on both sides');
  if (terms.sellerIntent.side !== 0 || terms.buyerIntent.side !== 1) {
    throw new Error('the two signed intents are not one Sell and one Buy, in that order');
  }
  return terms;
}

// ------------------------------------------------------------------ the rows

export type MarketFillV1 = Readonly<{
  signature: string;
  slot: string;
  /** Unix seconds the block carried, or null when the node kept none. */
  blockTime: string | null;
  terms: DirectInlineFillTermsV1;
  /** Gross, both fees and both net legs — or why they could not be computed. */
  economics: DirectInlineEconomicPreviewV3 | null;
  economicsRefusal: string | null;
  /**
   * Collateral paid per claim, exact, when the division is exact.
   *
   * `executionPrice / priceScale` is the price a claim crossed at, and the two
   * are u64s at an immutable scale — so this is a ratio rendered as a ratio,
   * never a float. Null when gross is unknown or the fill is zero.
   */
  grossPerClaim: Readonly<{ numerator: string; denominator: string }> | null;
}>;

export type MarketActivityRowV1 = Readonly<{
  signature: string;
  slot: string;
  blockTime: string | null;
  succeeded: boolean;
  errorText: string | null;
  feeLamports: string;
  /** Every program the transaction's top-level instructions invoked. */
  programs: ReadonlyArray<string>;
  /** The entry route the census names for this act, or null when none does. */
  route: MarketActivityRouteV1 | null;
  /** Present only on a decoded Direct crossing. */
  fill: MarketFillV1 | null;
  /** Why this row carries no route: an absence with a reason, never a blank. */
  unnamedReason: string | null;
}>;

/** One Position on this market, read live, with what it holds now. */
export type MarketPositionStandingV1 = Readonly<{
  address: string;
  owner: string;
  revision: string;
  balances: ReadonlyArray<string>;
  totalClaims: string;
  /** True when every outcome holds the same count: a complete set, no side taken. */
  level: boolean;
}>;

/** What one party still owes this venue, from the maker replay itself. */
export type MarketFeeStandingV1 = Readonly<{
  maker: string;
  replayAddress: string;
  state: 'vacant' | 'existing';
  feeOwed: string;
  nextNonce: string;
}>;

export type MarketActivityV1 = Readonly<{
  marketAddress: string;
  observedSlot: string;
  rows: ReadonlyArray<MarketActivityRowV1>;
  fills: ReadonlyArray<MarketFillV1>;
  positions: ReadonlyArray<MarketPositionStandingV1>;
  feeStandings: ReadonlyArray<MarketFeeStandingV1>;
  /** Signatures the node listed but whose bytes were not fetched. */
  signaturesNotRead: number;
  /**
   * Rows the node listed, was asked for, and did not return.
   *
   * Separate from `signaturesNotRead` on purpose: one is this surface's own
   * bound and the other is the node refusing, and a page that reported them as
   * one number could not tell a quiet market from a throttled endpoint.
   */
  transactionsRefused: number;
  /** Accounts the Position scan did not reach, because it is bounded. */
  accountsNotScanned: number;
  reason: string;
}>;

export type MarketActivityRequestV1 = Readonly<{
  marketAddress: string;
  tradingProgramId: string;
  claimsProgramId: string;
  /** The Claims aggregate this market's Positions must name. */
  aggregateAddress: string;
  generation: bigint;
  outcomeCount: number;
  /** The Direct config's immutable price scale and fee rate. */
  priceScale: bigint;
  feeBasisPoints: number;
  signatures?: number;
}>;

function canonical(value: string, field: string): string {
  const key = new PublicKey(value).toBase58();
  if (key !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

/** The top-level instructions of one finalized transaction, with their programs. */
function topLevelInstructionsV1(
  bytes: Uint8Array,
  accountAddresses: ReadonlyArray<string>,
): ReadonlyArray<Readonly<{ programId: string | null; data: Uint8Array }>> {
  let decoded: VersionedTransaction;
  try {
    decoded = VersionedTransaction.deserialize(bytes);
  } catch {
    return Object.freeze([]);
  }
  return Object.freeze(decoded.message.compiledInstructions.map((instruction) => Object.freeze({
    programId: accountAddresses[instruction.programIdIndex] ?? null,
    data: instruction.data instanceof Uint8Array ? instruction.data : Uint8Array.from(instruction.data),
  })));
}

function grossPerClaimV1(economics: DirectInlineEconomicPreviewV3 | null): Readonly<{ numerator: string; denominator: string }> | null {
  if (economics === null || economics.fill === 0n) return null;
  return Object.freeze({ numerator: economics.grossCollateral.toString(), denominator: economics.fill.toString() });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the read refused without a usable reason';
}

/**
 * Derive one Market's activity, its live Positions and its fee standing.
 *
 * Bounded by construction: one signature listing, at most
 * `MARKET_ACTIVITY_TRANSACTIONS_V1` transaction reads, at most
 * `MARKET_ACTIVITY_POSITION_SCAN_V1` accounts probed with an eight-byte data
 * slice, and one full read of whatever that probe identified as a Position.
 */
export async function inspectMarketActivityV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'signaturesForAddress' | 'transaction' | 'multipleAccounts' | 'multipleAccountDataSlices' | 'accountInfo'>,
  request: MarketActivityRequestV1,
): Promise<MarketActivityV1> {
  const marketAddress = canonical(request.marketAddress, 'Market');
  const tradingProgramId = canonical(request.tradingProgramId, 'Trading program');
  const claimsProgramId = canonical(request.claimsProgramId, 'Claims program');
  const aggregateAddress = canonical(request.aggregateAddress, 'Claims aggregate');
  const wanted = request.signatures ?? MARKET_ACTIVITY_SIGNATURES_V1;
  const observedSlot = await client.finalizedSlot();

  const listing = await client.signaturesForAddress(marketAddress, wanted);
  const read = listing.slice(0, MARKET_ACTIVITY_TRANSACTIONS_V1);
  const observations = await boundedMap(read, TRANSACTIONS_IN_FLIGHT, async (entry) => {
    try {
      const observation = await client.transaction(entry.signature);
      return observation === null
        ? Object.freeze({ observation: null, refusal: 'the node listed this signature and answered null when asked for it' })
        : Object.freeze({ observation, refusal: null });
    } catch (error) {
      return Object.freeze({ observation: null, refusal: errorMessage(error) });
    }
  });

  const rows: MarketActivityRowV1[] = [];
  const fills: MarketFillV1[] = [];
  const candidates: string[] = [];
  const makers = new Set<string>();

  let unread = 0;
  for (const [index, entry] of observations.entries()) {
    const listed = read[index];
    const observation = entry.observation;
    if (observation === null) {
      unread += 1;
      rows.push(Object.freeze({
        signature: listed.signature,
        slot: listed.slot,
        blockTime: listed.blockTime,
        succeeded: listed.succeeded,
        errorText: listed.errorText,
        feeLamports: '0',
        programs: Object.freeze([]),
        route: null,
        fill: null,
        unnamedReason: `this transaction was not read: ${entry.refusal}`,
      }));
      continue;
    }
    for (const address of observation.accountAddresses) {
      if (candidates.length < MARKET_ACTIVITY_POSITION_SCAN_V1 && !candidates.includes(address)) candidates.push(address);
    }
    const instructions = topLevelInstructionsV1(observation.transactionBytes, observation.accountAddresses);
    const programs = Object.freeze([...new Set(instructions.map((entry) => entry.programId).filter((entry): entry is string => entry !== null))]);

    let route: MarketActivityRouteV1 | null = null;
    let fill: MarketFillV1 | null = null;
    let unnamedReason: string | null = null;
    for (const instruction of instructions) {
      if (instruction.data.length < MAGIC_BYTES) continue;
      const magic = ascii(instruction.data, MAGIC_OFFSET, MAGIC_BYTES);
      const named = censusRouteV1(magic, instruction.programId);
      if (named !== null && route === null) route = named;
      if (instruction.programId !== tradingProgramId) continue;
      try {
        const terms = decodeDirectInlineOrdinaryFillV3(instruction.data);
        if (terms.sellerIntent.market !== marketAddress) continue;
        let economics: DirectInlineEconomicPreviewV3 | null = null;
        let economicsRefusal: string | null = null;
        try {
          economics = previewDirectInlineV3(
            {
              market: marketAddress,
              generation: request.generation,
              outcomeCount: request.outcomeCount,
              priceScale: request.priceScale,
              feeBasisPoints: request.feeBasisPoints,
            },
            { intent: terms.sellerIntent },
            { intent: terms.buyerIntent },
            terms.fillAtoms,
            terms.executionPrice,
            BigInt(observation.slot),
          );
        } catch (error) {
          // The preview is the trade stepper's own admission rule. A crossing
          // the chain accepted that this rule will not re-admit is a real
          // disagreement and is shown as one, never smoothed over.
          economicsRefusal = errorMessage(error);
        }
        fill = Object.freeze({
          signature: observation.signature,
          slot: observation.slot,
          blockTime: observation.blockTime,
          terms,
          economics,
          economicsRefusal,
          grossPerClaim: grossPerClaimV1(economics),
        });
        makers.add(terms.seller);
        makers.add(terms.buyer);
      } catch {
        // Not an InlineOrdinary crossing. The census still gets to name it if
        // a magic selects it; otherwise the row says so below.
      }
    }
    if (route === null && fill === null) {
      unnamedReason = programs.includes(tradingProgramId)
        ? 'a Trading act whose leading bytes select no route the generated census names — the census records this program’s remaining entry routes as predicate-selected, so no table can name them'
        : 'no instruction in this transaction begins with a magic the generated route census names';
    }
    rows.push(Object.freeze({
      signature: observation.signature,
      slot: observation.slot,
      blockTime: observation.blockTime,
      succeeded: observation.succeeded,
      errorText: observation.errorText,
      feeLamports: observation.feeLamports,
      programs,
      route,
      fill,
      unnamedReason,
    }));
    if (fill !== null) fills.push(fill);
  }

  const positions = await scanPositionsV1(client, candidates, {
    claimsProgramId,
    aggregateAddress,
    outcomeCount: request.outcomeCount,
    observedSlot,
  });

  const feeStandings = await feeStandingsV1(client, [...makers].sort(), {
    tradingProgramId,
    marketAddress,
    generation: request.generation,
  });

  const reason = listing.length === 0
    ? 'The node returned no signatures for this Market address.'
    : `${rows.length - unread} of ${listing.length} listed transactions read and decoded, ${fills.length} of them a Direct crossing.${
      unread === 0 ? '' : ` ${unread} the node would not return; those rows say so.`}`;

  return Object.freeze({
    marketAddress,
    observedSlot,
    rows: Object.freeze(rows),
    fills: Object.freeze(fills),
    positions,
    feeStandings: Object.freeze(feeStandings),
    signaturesNotRead: listing.length - read.length,
    transactionsRefused: unread,
    accountsNotScanned: Math.max(0, candidates.length - MARKET_ACTIVITY_POSITION_SCAN_V1),
    reason,
  });
}

/**
 * Which of these accounts are Positions on this market, and what they hold.
 *
 * Two passes, because a Hot fill's account list carries whole ProgramData
 * accounts: an eight-byte data slice first, which costs nothing and settles
 * owner, width and magic together, and a full read only of what survived it.
 */
async function scanPositionsV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices'>,
  candidates: ReadonlyArray<string>,
  request: Readonly<{ claimsProgramId: string; aggregateAddress: string; outcomeCount: number; observedSlot: string }>,
): Promise<ReadonlyArray<MarketPositionStandingV1>> {
  if (candidates.length === 0) return Object.freeze([]);
  const width = LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + request.outcomeCount * 8;
  const chunks: string[][] = [];
  for (let index = 0; index < candidates.length; index += ACCOUNTS_PER_READ) {
    chunks.push(candidates.slice(index, index + ACCOUNTS_PER_READ));
  }
  const probes = await Promise.all(chunks.map(async (chunk) => {
    try {
      return await client.multipleAccountDataSlices(chunk, 0, MAGIC_BYTES, request.observedSlot);
    } catch {
      return null;
    }
  }));
  const survivors: string[] = [];
  for (const probe of probes) {
    if (probe === null) continue;
    for (const entry of probe.accounts) {
      const account = entry.account;
      if (account === null || account.executable) continue;
      if (account.owner !== request.claimsProgramId || account.space !== width) continue;
      if (!same(account.data, LIABILITY_BASIS_POSITION_MAGIC_V2)) continue;
      survivors.push(entry.address);
    }
  }
  if (survivors.length === 0) return Object.freeze([]);

  const decoded: ClaimsPositionV2[] = [];
  for (let index = 0; index < survivors.length; index += ACCOUNTS_PER_READ) {
    const observation = await client.multipleAccounts(survivors.slice(index, index + ACCOUNTS_PER_READ), request.observedSlot);
    for (const entry of observation.accounts) {
      if (entry.account === null) continue;
      try {
        const position = decodeClaimsPositionV2(entry.address, entry.account.data);
        if (position.aggregate === request.aggregateAddress) decoded.push(position);
      } catch {
        // An account that carries the magic and the width and still refuses to
        // decode is not a Position; it is left out rather than half-rendered.
      }
    }
  }

  const standings = decoded.map((position) => {
    const total = position.balances.reduce((sum, atoms) => sum + BigInt(atoms), 0n);
    const first = position.balances[0];
    return Object.freeze({
      address: position.address,
      owner: position.owner,
      revision: position.revision,
      balances: position.balances,
      totalClaims: total.toString(),
      level: position.balances.length > 0 && position.balances.every((atoms) => atoms === first),
    });
  });
  // Largest holding first, then by address, so the order is total and never
  // depends on the order the node happened to list the accounts in.
  return Object.freeze([...standings].sort((left, right) => {
    const difference = BigInt(right.totalClaims) - BigInt(left.totalClaims);
    if (difference !== 0n) return difference > 0n ? 1 : -1;
    return left.address.localeCompare(right.address);
  }));
}

/**
 * What each party to a crossing still owes this venue, from the maker replay.
 *
 * A pair goes through `inspectDirectMakerNoncePairV1`, which is two round trips
 * for two makers instead of four — and a crossing has exactly two parties, so
 * that is the shape this almost always takes. A replay that will not read is
 * LEFT OUT rather than reported as zero owed, which is the one wrong thing
 * this surface could say.
 */
async function feeStandingsV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'accountInfo' | 'multipleAccounts'>,
  makers: ReadonlyArray<string>,
  request: Readonly<{ tradingProgramId: string; marketAddress: string; generation: bigint }>,
): Promise<ReadonlyArray<MarketFeeStandingV1>> {
  const base = { tradingProgram: request.tradingProgramId, market: request.marketAddress, generation: request.generation };
  const standing = (maker: string, replay: Readonly<{ address: string; state: 'vacant' | 'existing'; feeOwed: bigint; nextNonce: bigint }>): MarketFeeStandingV1 =>
    Object.freeze({
      maker,
      replayAddress: replay.address,
      state: replay.state,
      feeOwed: replay.feeOwed.toString(),
      nextNonce: replay.nextNonce.toString(),
    });
  if (makers.length === 2) {
    try {
      const pair = await inspectDirectMakerNoncePairV1(client, [
        { ...base, maker: makers[0] },
        { ...base, maker: makers[1] },
      ]);
      return Object.freeze([standing(makers[0], pair[0]), standing(makers[1], pair[1])]);
    } catch {
      return Object.freeze([]);
    }
  }
  const output: MarketFeeStandingV1[] = [];
  for (const maker of makers) {
    try {
      output.push(standing(maker, await inspectDirectMakerNonceV1(client, { ...base, maker })));
    } catch {
      // Left out, deliberately: see above.
    }
  }
  return Object.freeze(output);
}
