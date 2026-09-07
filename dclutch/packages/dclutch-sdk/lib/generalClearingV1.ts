import { hex, isZero, pubkey, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import {
  GENERAL_BATCH_MAGIC_V2,
  GENERAL_BATCH_STATUS_CLEARED_V2,
  GENERAL_BATCH_STATUS_CLOSED_V2,
  GENERAL_BATCH_STATUS_COLLECTING_V2,
  GENERAL_BATCH_V1_BYTES,
  GENERAL_BATCH_VERSION_V2,
  GENERAL_CLEARING_CLEARED_CANDIDATE_ID_OFFSET_V1,
  GENERAL_CLEARING_CLEARED_SLOT_OFFSET_V1,
  GENERAL_CLEARING_FILLED_LOTS_OFFSET_V1,
  GENERAL_CLEARING_LIVE_ORDER_COUNT_OFFSET_V1,
  GENERAL_CLEARING_MOVE_MERGE_V1,
  GENERAL_CLEARING_MOVE_MINT_V1,
  GENERAL_CLEARING_MOVE_NONE_V1,
  GENERAL_CLEARING_PRICES_OFFSET_V1,
  GENERAL_CLEARING_RESERVED_MOVE_BYTES_V1,
  GENERAL_CLEARING_RESERVED_MOVE_OFFSET_V1,
  GENERAL_CLEARING_RESERVED_TAIL_BYTES_V1,
  GENERAL_CLEARING_RESERVED_TAIL_OFFSET_V1,
  GENERAL_CLEARING_SETS_MOVE_OFFSET_V1,
  GENERAL_CLEARING_SETS_QUANTITY_OFFSET_V1,
  GENERAL_CLEARING_TAIL_COUNT_V1,
  GENERAL_CLEARING_TAIL_STRIDE_V1,
} from './generated/generalClearingPriceV1';
import {
  GENERAL_ORDER_BATCH_ID_OFFSET_V2,
  GENERAL_ORDER_CLAIMS_PER_LOT_OFFSET_V2,
  GENERAL_ORDER_GENERATION_OFFSET_V2,
  GENERAL_ORDER_HEADER_BYTES_V2,
  GENERAL_ORDER_MAGIC_BYTES_V2,
  GENERAL_ORDER_MAGIC_OFFSET_V2,
  GENERAL_ORDER_MAGIC_V2,
  GENERAL_ORDER_MARKET_OFFSET_V2,
  GENERAL_ORDER_MAX_LOTS_OFFSET_V2,
  GENERAL_ORDER_MAX_QUOTE_DEBIT_PER_LOT_OFFSET_V2,
  GENERAL_ORDER_MIN_QUOTE_CREDIT_PER_LOT_OFFSET_V2,
  GENERAL_ORDER_NONCE_OFFSET_V2,
  GENERAL_ORDER_OUTCOME_COUNT_OFFSET_V2,
  GENERAL_ORDER_OUTCOME_HI_OFFSET_V2,
  GENERAL_ORDER_OUTCOME_LO_OFFSET_V2,
  GENERAL_ORDER_OWNER_ID_OFFSET_V2,
  GENERAL_ORDER_PHASE_OFFSET_V2,
  GENERAL_ORDER_PHASE_V2,
  GENERAL_ORDER_RESERVED_BYTES_V2,
  GENERAL_ORDER_RESERVED_OFFSET_V2,
  GENERAL_ORDER_RESERVED_SHAPE_BYTES_V2,
  GENERAL_ORDER_RESERVED_SHAPE_OFFSET_V2,
  GENERAL_ORDER_RESERVED_STATE_BYTES_V2,
  GENERAL_ORDER_RESERVED_STATE_OFFSET_V2,
  GENERAL_ORDER_RESERVED_STATE_TAIL_BYTES_V2,
  GENERAL_ORDER_RESERVED_STATE_TAIL_OFFSET_V2,
  GENERAL_ORDER_RESERVED_TAIL_BYTES_V2,
  GENERAL_ORDER_RESERVED_TAIL_OFFSET_V2,
  GENERAL_ORDER_ROW_BASE_V2,
  GENERAL_ORDER_ROW_STRIDE_V2,
  GENERAL_ORDER_SIDE_BUY_V2,
  GENERAL_ORDER_SIDE_OFFSET_V2,
  GENERAL_ORDER_SIDE_SELL_V2,
  GENERAL_ORDER_STATE_ADMITTED_SLOT_OFFSET_V2,
  GENERAL_ORDER_STATE_PHASE_OFFSET_V2,
  GENERAL_ORDER_STATE_RELEASED_SLOT_OFFSET_V2,
  GENERAL_ORDER_VALID_UNTIL_SLOT_OFFSET_V2,
  GENERAL_ORDER_VERSION_OFFSET_V2,
  GENERAL_ORDER_VERSION_V2,
} from './generated/generalOrderV2';
import {
  GENERAL_BATCH_CANCELLED_COUNT_OFFSET_V2,
  GENERAL_BATCH_CLOSED_ROOT_REVISION_OFFSET_V2,
  GENERAL_BATCH_MAGIC_OFFSET_V2,
  GENERAL_BATCH_COLLECTION_CLOSE_SLOT_OFFSET_V2,
  GENERAL_BATCH_COMMITTED_QUOTE_RESERVE_OFFSET_V2,
  GENERAL_BATCH_CONFIG_ID_OFFSET_V2,
  GENERAL_BATCH_GENERATION_OFFSET_V2,
  GENERAL_BATCH_MARKET_OFFSET_V2,
  GENERAL_BATCH_MAX_ORDERS_OFFSET_V2,
  GENERAL_BATCH_OPENED_ROOT_REVISION_OFFSET_V2,
  GENERAL_BATCH_ORDER_COUNT_OFFSET_V2,
  GENERAL_BATCH_OUTCOME_COUNT_OFFSET_V2,
  GENERAL_BATCH_PHASE_OFFSET_V2,
  GENERAL_BATCH_PHASE_V2,
  GENERAL_BATCH_PRICE_SCALE_OFFSET_V2,
  GENERAL_BATCH_PRODUCT_ID_OFFSET_V2,
  GENERAL_BATCH_SEQUENCE_OFFSET_V2,
  GENERAL_BATCH_SETTLEMENT_CLOSE_SLOT_OFFSET_V2,
  GENERAL_BATCH_STATUS_OFFSET_V2,
  GENERAL_BATCH_VERSION_OFFSET_V2,
  GENERAL_ORDER_STATE_CANCELLED_V2,
  GENERAL_ORDER_STATE_PLACED_V2,
  GENERAL_ORDER_STATE_RELEASED_V2,
} from './generated/generalSuccessorV5';

/**
 * The joint clearing's two client-facing records (cohort-18,
 * `MECHANISM_JOINT_CLEARING_2026_09_04.md`).
 *
 * NOT ONE COORDINATE BELOW IS HAND-KEPT. The family shipped these two wires
 * with Lean emitters that targeted Rust only, so this file spelled every
 * magic, tag and offset itself and the SDK's ABI-coverage baseline grew three
 * rows to record it. `EmitGeneralOrderV2AbiTs.lean` and
 * `EmitClearingPriceV1AbiTs.lean` are the second backend, so the same Lean
 * objects the Rust reads now print the browser's module too:
 *
 * - the ORDER record is Lean's entire (`GeneralOrderV2Abi` → `abi:general-order-v2`);
 * - the BATCH record's magic, version, status tags and clearing tail from byte
 *   224 on are Lean's (`ClearingPriceV1Abi` → `abi:general-clearing-v1`);
 * - the batch's V1 PREFIX (bytes 12..224), its phase byte, and the order's
 *   three escrow-state tags are not: no Lean object states the prefix's field
 *   sequence, so `GeneralBatchLayoutV2` in `collection_v1.rs` is their author
 *   and `abi:general-v5` scrapes it. Promoting the prefix to a field list is
 *   the named exit, and it moves Rust rather than the browser.
 *
 * The decode RULES are still this file's own reading of `GeneralBatchV2::decode`
 * and `GeneralOrderV2::decode`; only the numbers have one author.
 */

/**
 * ONE VERSION PER RECORD, from that record's own author.
 *
 * The joint clearing moved both digests in one cohort and both Lean modules
 * emit `2`, and `collection_v1.rs` asserts exactly that agreement
 * (`VERSION == order_wire::ORDER_VERSION_V2 && VERSION == clearing_wire::BATCH_VERSION_V2`).
 * Reading one record's version to check the other would make that agreement an
 * assumption of this file instead of a fact the crate proves.
 */


export type GeneralBatchStatusTagV2 = 'collecting' | 'closed' | 'cleared';
export type GeneralClearingSetsMoveV1 = 'none' | 'mint' | 'merge';
export type GeneralOrderSideV2 = 'buy' | 'sell';
export type GeneralOrderPhaseV2 = 'placed' | 'cancelled' | 'released';

/** The clearing tail, present only once a batch's `status` is `cleared`. */
export type GeneralClearingV1 = Readonly<{
  clearedCandidateId: string;
  clearedSlot: bigint;
  setsMove: GeneralClearingSetsMoveV1;
  setsQuantity: bigint;
  filledLots: bigint;
  liveOrderCount: number;
  prices: ReadonlyArray<bigint>;
  residual: ReadonlyArray<bigint>;
}>;

export type GeneralBatchRecordV2 = Readonly<{
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
  status: GeneralBatchStatusTagV2;
  orderCount: number;
  openedRootRevision: bigint;
  closedRootRevision: bigint;
  committedQuoteReserve: bigint;
  cancelledCount: number;
  liveOrderCount: number;
  clearing: GeneralClearingV1 | null;
}>;

export type GeneralOrderRowV2 = Readonly<{ receive: bigint; deliver: bigint }>;

export type GeneralOrderRecordV2 = Readonly<{
  outcomeCount: number;
  nonce: bigint;
  minQuoteCreditPerLot: bigint;
  ownerId: string;
  market: string;
  batchId: string;
  generation: bigint;
  maxLots: bigint;
  maxQuoteDebitPerLot: bigint;
  validUntilSlot: bigint;
  side: GeneralOrderSideV2;
  outcomeLo: number;
  outcomeHi: number;
  claimsPerLot: bigint;
  phase: GeneralOrderPhaseV2;
  admittedSlot: bigint;
  releasedSlot: bigint;
  rows: ReadonlyArray<GeneralOrderRowV2>;
}>;

/** The derived simplex view of one cleared batch's price vector. */
export type GeneralClearingPricesV1 = Readonly<{
  scale: bigint;
  prices: ReadonlyArray<bigint>;
  fractions: ReadonlyArray<number>;
  byConstruction: true;
}>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function readU32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

/**
 * The twelve-byte prologue both records carry, read at EACH RECORD'S OWN
 * coordinates rather than at one record's borrowed from the other. The two
 * happen to agree — `RuntimeWireV2.prologueFields` is why — and a shared
 * reader that assumed it would be right for the wrong reason, which is the
 * mis-wiring `generate-general-successor-v5.mjs`'s `layoutOffsets` refuses in
 * the other direction.
 */
type PrologueV2 = Readonly<{ magic: Uint8Array; magicOffset: number; magicBytes: number; version: number; versionOffset: number; phaseOffset: number; phase: number; reservedOffset: number; reservedBytes: number }>;

const BATCH_PROLOGUE_V2: PrologueV2 = {
  magic: GENERAL_BATCH_MAGIC_V2, magicOffset: GENERAL_BATCH_MAGIC_OFFSET_V2, magicBytes: GENERAL_BATCH_MAGIC_V2.length,
  version: GENERAL_BATCH_VERSION_V2, versionOffset: GENERAL_BATCH_VERSION_OFFSET_V2, phaseOffset: GENERAL_BATCH_PHASE_OFFSET_V2, phase: GENERAL_BATCH_PHASE_V2,
  // The batch prefix's scrape emits no reserved field, so the canonical zero
  // is the byte between the phase and the first field the scrape does emit.
  reservedOffset: GENERAL_BATCH_PHASE_OFFSET_V2 + 1, reservedBytes: GENERAL_BATCH_OUTCOME_COUNT_OFFSET_V2 - (GENERAL_BATCH_PHASE_OFFSET_V2 + 1),
};

const ORDER_PROLOGUE_V2: PrologueV2 = {
  magic: GENERAL_ORDER_MAGIC_V2, magicOffset: GENERAL_ORDER_MAGIC_OFFSET_V2, magicBytes: GENERAL_ORDER_MAGIC_BYTES_V2,
  version: GENERAL_ORDER_VERSION_V2, versionOffset: GENERAL_ORDER_VERSION_OFFSET_V2, phaseOffset: GENERAL_ORDER_PHASE_OFFSET_V2, phase: GENERAL_ORDER_PHASE_V2,
  reservedOffset: GENERAL_ORDER_RESERVED_OFFSET_V2, reservedBytes: GENERAL_ORDER_RESERVED_BYTES_V2,
};

function requireHeader(bytes: Uint8Array, prologue: PrologueV2, label: string): void {
  if (!same(slice(bytes, prologue.magicOffset, prologue.magicBytes), prologue.magic)
      || u16(bytes, prologue.versionOffset) !== prologue.version
      || bytes[prologue.phaseOffset] !== prologue.phase) {
    throw new Error(`${label} body is not exact V2`);
  }
  requireZero(bytes, prologue.reservedOffset, prologue.reservedBytes, `${label} header reserved`);
}

function id32(bytes: Uint8Array, offset: number, field: string): string {
  const value = slice(bytes, offset, 32);
  requireNonzero(value, field);
  return hex(value);
}

/**
 * An ACCOUNT ADDRESS, base58. `id32` is for content DIGESTS, which are hex.
 * The tree splits the two everywhere (`generalPlanV5.ts`'s `pubkeyHex` for
 * `market` and `owner`, its `idHex` for `batchId`, `productId`, `configId`),
 * and a reader that renders an address as hex hands the operator a string no
 * explorer, wallet or RPC will take.
 */
function address32(bytes: Uint8Array, offset: number, field: string): string {
  return pubkey(slice(bytes, offset, 32), field);
}

function clearingSetsMove(tag: number): GeneralClearingSetsMoveV1 {
  if (tag === GENERAL_CLEARING_MOVE_NONE_V1) return 'none';
  if (tag === GENERAL_CLEARING_MOVE_MINT_V1) return 'mint';
  if (tag === GENERAL_CLEARING_MOVE_MERGE_V1) return 'merge';
  throw new Error('General batch V2 clearing carries an unknown sets-move tag');
}

/**
 * Hostile-decode one exact `296 + 16N` General batch record: the V1 prefix
 * (`GeneralBatchLayoutV2`) followed by the clearing tail `ClearingPriceV1Abi`
 * appends at byte 224.
 *
 * The tail is VACANT while the batch collects or is closed, and a clearing —
 * a candidate, a slot, a canonical complete-set move, prices on the simplex,
 * a residual only where the price is zero — once `status` is `cleared`;
 * nothing else decodes (`ClearingPriceV1Abi.tailAdmissible`,
 * `GeneralBatchV2::decode` / `validate_clearing_tails` in `collection_v1.rs`).
 */
export function decodeGeneralBatchV2(bytes: Uint8Array): GeneralBatchRecordV2 {
  if (bytes.length < GENERAL_BATCH_V1_BYTES) throw new Error('General batch V2 body is truncated before its clearing tail');
  requireHeader(bytes, BATCH_PROLOGUE_V2, 'General batch V2');
  // The prefix's three canonical gaps, DERIVED from the coordinates either
  // side rather than typed: a `u32` bound followed by a `u64`, a `u8` status
  // followed by a `u32`, and everything between the last counter and the
  // clearing tail.
  const afterMaxOrders = GENERAL_BATCH_MAX_ORDERS_OFFSET_V2 + Uint32Array.BYTES_PER_ELEMENT;
  const afterStatus = GENERAL_BATCH_STATUS_OFFSET_V2 + Uint8Array.BYTES_PER_ELEMENT;
  const afterCancelledCount = GENERAL_BATCH_CANCELLED_COUNT_OFFSET_V2 + Uint32Array.BYTES_PER_ELEMENT;
  requireZero(bytes, afterMaxOrders, GENERAL_BATCH_SETTLEMENT_CLOSE_SLOT_OFFSET_V2 - afterMaxOrders, 'General batch V2 admission-bound tail');
  requireZero(bytes, afterStatus, GENERAL_BATCH_ORDER_COUNT_OFFSET_V2 - afterStatus, 'General batch V2 status tail');
  requireZero(bytes, afterCancelledCount, GENERAL_BATCH_V1_BYTES - afterCancelledCount, 'General batch V2 counters tail');

  const outcomeCount = readU32(bytes, GENERAL_BATCH_OUTCOME_COUNT_OFFSET_V2);
  const sequence = u64(bytes, GENERAL_BATCH_SEQUENCE_OFFSET_V2);
  const generation = u64(bytes, GENERAL_BATCH_GENERATION_OFFSET_V2);
  const market = address32(bytes, GENERAL_BATCH_MARKET_OFFSET_V2, 'batch Market');
  const productId = id32(bytes, GENERAL_BATCH_PRODUCT_ID_OFFSET_V2, 'batch Product');
  const configId = id32(bytes, GENERAL_BATCH_CONFIG_ID_OFFSET_V2, 'batch config');
  const priceScale = u64(bytes, GENERAL_BATCH_PRICE_SCALE_OFFSET_V2);
  const collectionCloseSlot = u64(bytes, GENERAL_BATCH_COLLECTION_CLOSE_SLOT_OFFSET_V2);
  const maxOrders = readU32(bytes, GENERAL_BATCH_MAX_ORDERS_OFFSET_V2);
  const settlementCloseSlot = u64(bytes, GENERAL_BATCH_SETTLEMENT_CLOSE_SLOT_OFFSET_V2);

  if (outcomeCount === 0 || priceScale === 0n || maxOrders === 0 || generation === 0n) {
    throw new Error('General batch V2 opening carries a zero outcome count, price scale, max orders, or generation');
  }
  if (settlementCloseSlot <= collectionCloseSlot) {
    throw new Error('General batch V2 settlement window does not close after its collection window');
  }

  const expectedLength = GENERAL_CLEARING_PRICES_OFFSET_V1 + GENERAL_CLEARING_TAIL_COUNT_V1 * GENERAL_CLEARING_TAIL_STRIDE_V1 * outcomeCount;
  if (bytes.length !== expectedLength) throw new Error('General batch V2 runtime width does not match its outcome count');

  requireZero(bytes, GENERAL_CLEARING_RESERVED_MOVE_OFFSET_V1, GENERAL_CLEARING_RESERVED_MOVE_BYTES_V1, 'General batch V2 sets-move tail');
  requireZero(bytes, GENERAL_CLEARING_RESERVED_TAIL_OFFSET_V1, GENERAL_CLEARING_RESERVED_TAIL_BYTES_V1, 'General batch V2 live-order-count tail');

  const statusByte = bytes[GENERAL_BATCH_STATUS_OFFSET_V2];
  const status: GeneralBatchStatusTagV2 | null =
    statusByte === GENERAL_BATCH_STATUS_COLLECTING_V2 ? 'collecting'
      : statusByte === GENERAL_BATCH_STATUS_CLOSED_V2 ? 'closed'
        : statusByte === GENERAL_BATCH_STATUS_CLEARED_V2 ? 'cleared' : null;
  if (status === null) throw new Error('General batch V2 status is unknown');

  const orderCount = readU32(bytes, GENERAL_BATCH_ORDER_COUNT_OFFSET_V2);
  const openedRootRevision = u64(bytes, GENERAL_BATCH_OPENED_ROOT_REVISION_OFFSET_V2);
  const closedRootRevision = u64(bytes, GENERAL_BATCH_CLOSED_ROOT_REVISION_OFFSET_V2);
  const committedQuoteReserve = u64(bytes, GENERAL_BATCH_COMMITTED_QUOTE_RESERVE_OFFSET_V2);
  const cancelledCount = readU32(bytes, GENERAL_BATCH_CANCELLED_COUNT_OFFSET_V2);

  if (orderCount > maxOrders || cancelledCount > orderCount) {
    throw new Error('General batch V2 admission counters exceed their bound');
  }
  if (openedRootRevision === 0n) throw new Error('General batch V2 carries a zero opened root revision');
  if (status === 'collecting' ? closedRootRevision !== 0n : closedRootRevision <= openedRootRevision) {
    throw new Error('General batch V2 lifecycle revision is noncanonical for its status');
  }

  const liveOrderCount = orderCount - cancelledCount;

  const clearedCandidateIdBytes = slice(bytes, GENERAL_CLEARING_CLEARED_CANDIDATE_ID_OFFSET_V1, 32);
  const clearedSlot = u64(bytes, GENERAL_CLEARING_CLEARED_SLOT_OFFSET_V1);
  const setsMoveByte = bytes[GENERAL_CLEARING_SETS_MOVE_OFFSET_V1];
  const setsQuantity = u64(bytes, GENERAL_CLEARING_SETS_QUANTITY_OFFSET_V1);
  const filledLots = u64(bytes, GENERAL_CLEARING_FILLED_LOTS_OFFSET_V1);
  const clearingLiveOrderCount = readU32(bytes, GENERAL_CLEARING_LIVE_ORDER_COUNT_OFFSET_V1);

  const tailBytesAreZero = isZero(slice(bytes, GENERAL_CLEARING_PRICES_OFFSET_V1, GENERAL_CLEARING_TAIL_COUNT_V1 * GENERAL_CLEARING_TAIL_STRIDE_V1 * outcomeCount));
  const tailIsVacant = isZero(clearedCandidateIdBytes) && clearedSlot === 0n && setsMoveByte === GENERAL_CLEARING_MOVE_NONE_V1
    && setsQuantity === 0n && filledLots === 0n && clearingLiveOrderCount === 0 && tailBytesAreZero;

  const opening = {
    outcomeCount, sequence, generation, market, productId, configId, priceScale,
    collectionCloseSlot, maxOrders, settlementCloseSlot,
  } as const;
  const counters = {
    status, orderCount, openedRootRevision, closedRootRevision, committedQuoteReserve, cancelledCount, liveOrderCount,
  } as const;

  if (status !== 'cleared') {
    if (!tailIsVacant) throw new Error('General batch V2 clearing tail is not vacant while uncleared');
    return Object.freeze({ ...opening, ...counters, clearing: null });
  }

  if (isZero(clearedCandidateIdBytes) || clearedSlot === 0n || clearingLiveOrderCount === 0) {
    throw new Error('General batch V2 clearing is missing its candidate, slot, or live order count');
  }
  const setsMove = clearingSetsMove(setsMoveByte);
  const canonicalMove = setsMove === 'none' ? setsQuantity === 0n : setsQuantity !== 0n;
  if (!canonicalMove) throw new Error('General batch V2 clearing sets-move disagrees with its quantity');
  if (clearingLiveOrderCount !== liveOrderCount) {
    throw new Error('General batch V2 clearing live order count disagrees with admitted minus cancelled');
  }

  const residualBase = GENERAL_CLEARING_PRICES_OFFSET_V1 + GENERAL_CLEARING_TAIL_STRIDE_V1 * outcomeCount;
  const prices: bigint[] = [];
  const residual: bigint[] = [];
  let total = 0n;
  for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
    const price = u64(bytes, GENERAL_CLEARING_PRICES_OFFSET_V1 + GENERAL_CLEARING_TAIL_STRIDE_V1 * outcome);
    const strand = u64(bytes, residualBase + GENERAL_CLEARING_TAIL_STRIDE_V1 * outcome);
    if (price !== 0n && strand !== 0n) throw new Error('General batch V2 clearing strands a residual behind a priced outcome');
    total += price;
    prices.push(price);
    residual.push(strand);
  }
  if (total !== priceScale) throw new Error('General batch V2 clearing prices do not sum to the price scale');

  return Object.freeze({
    ...opening, ...counters,
    clearing: Object.freeze({
      clearedCandidateId: hex(clearedCandidateIdBytes), clearedSlot, setsMove, setsQuantity, filledLots,
      liveOrderCount: clearingLiveOrderCount, prices: Object.freeze(prices), residual: Object.freeze(residual),
    }),
  });
}

/**
 * PRICES SUM TO ONE BY CONSTRUCTION. This is a read of a conjunct
 * {@link decodeGeneralBatchV2} already proved, not a check performed here:
 * `ClearingPriceV1Abi.tailAdmissible` refuses any cleared batch whose prices
 * do not sum to `priceScale` (the Lean theorem
 * `a_cleared_tail_is_on_the_simplex`), so a decoded record that carries a
 * non-null `clearing` has already been held to the simplex. Returns `null`
 * for an uncleared batch — there is no price vector to view yet.
 */
export function clearingPricesV1(decoded: GeneralBatchRecordV2): GeneralClearingPricesV1 | null {
  if (decoded.clearing === null) return null;
  const { prices } = decoded.clearing;
  const scale = decoded.priceScale;
  const scaleNumber = Number(scale);
  const fractions = prices.map((price) => Number(price) / scaleNumber);
  return Object.freeze({ scale, prices, fractions: Object.freeze(fractions), byConstruction: true as const });
}

/**
 * Hostile-decode one exact `216 + 16N` General order record
 * (`GeneralOrderLayoutV2` / `generated_order_v2.rs`). The rows at byte 216
 * are DERIVED transport: a buy receives `claimsPerLot` on its interval and
 * delivers nothing, a sell the reverse, and nothing moves off the interval
 * (`GeneralOrderV2Abi.Shape.row`); `decode` refuses a record whose rows
 * disagree with the shape its own header carries.
 */
export function decodeGeneralOrderV2(bytes: Uint8Array): GeneralOrderRecordV2 {
  if (bytes.length < GENERAL_ORDER_ROW_BASE_V2) throw new Error('General order V2 body is truncated before its rows');
  requireHeader(bytes, ORDER_PROLOGUE_V2, 'General order V2');

  const outcomeCount = readU32(bytes, GENERAL_ORDER_OUTCOME_COUNT_OFFSET_V2);
  const nonce = u64(bytes, GENERAL_ORDER_NONCE_OFFSET_V2);
  const minQuoteCreditPerLot = u64(bytes, GENERAL_ORDER_MIN_QUOTE_CREDIT_PER_LOT_OFFSET_V2);
  const ownerId = address32(bytes, GENERAL_ORDER_OWNER_ID_OFFSET_V2, 'order owner');
  const market = address32(bytes, GENERAL_ORDER_MARKET_OFFSET_V2, 'order Market');
  const batchId = id32(bytes, GENERAL_ORDER_BATCH_ID_OFFSET_V2, 'order Batch');
  const generation = u64(bytes, GENERAL_ORDER_GENERATION_OFFSET_V2);
  const maxLots = u64(bytes, GENERAL_ORDER_MAX_LOTS_OFFSET_V2);
  const maxQuoteDebitPerLot = u64(bytes, GENERAL_ORDER_MAX_QUOTE_DEBIT_PER_LOT_OFFSET_V2);
  const validUntilSlot = u64(bytes, GENERAL_ORDER_VALID_UNTIL_SLOT_OFFSET_V2);

  // THE BROWSER IS THE FINER OF THE TWO READERS HERE, and that is a finding
  // about the program rather than a licence for this file.
  //
  // An unknown side byte gets its own reason below. The program folds it:
  // `collection_v1.rs`'s `read_order_header` writes
  // `OrderSideV2::decode(...).ok_or(GeneralCollectionErrorV1::ShapeNotInterval)`,
  // so a byte that is neither buy nor sell refuses under a code whose own doc
  // says "the shape is not a nonempty interval inside the width moving a
  // positive number of claims per lot" -- three conjuncts, none of which is the
  // one that failed. That is AGENTS.md's `map_err(|_| Coarse)`: a located
  // defect turned into a search, and the causes are not one accusation.
  // Splitting it is a Trading-crate change with a refusal variant and a frame
  // capture, so it is recorded here rather than done from a browser lane.
  const sideByte = bytes[GENERAL_ORDER_SIDE_OFFSET_V2];
  const side: GeneralOrderSideV2 | null = sideByte === GENERAL_ORDER_SIDE_BUY_V2 ? 'buy' : sideByte === GENERAL_ORDER_SIDE_SELL_V2 ? 'sell' : null;
  if (side === null) throw new Error('General order V2 side tag is unknown');

  const outcomeLo = readU32(bytes, GENERAL_ORDER_OUTCOME_LO_OFFSET_V2);
  const outcomeHi = readU32(bytes, GENERAL_ORDER_OUTCOME_HI_OFFSET_V2);
  const claimsPerLot = u64(bytes, GENERAL_ORDER_CLAIMS_PER_LOT_OFFSET_V2);

  if (outcomeCount === 0 || maxLots === 0n || generation === 0n) {
    throw new Error('General order V2 header carries a zero outcome count, max lots, or generation');
  }
  if (bytes.length !== GENERAL_ORDER_ROW_BASE_V2 + GENERAL_ORDER_ROW_STRIDE_V2 * outcomeCount) {
    throw new Error('General order V2 runtime width does not match its outcome count');
  }
  if (outcomeLo > outcomeHi || outcomeHi >= outcomeCount || claimsPerLot === 0n) {
    throw new Error('General order V2 shape is not a nonempty interval inside its width');
  }

  requireZero(bytes, GENERAL_ORDER_RESERVED_SHAPE_OFFSET_V2, GENERAL_ORDER_RESERVED_SHAPE_BYTES_V2, 'General order V2 shape tail');
  requireZero(bytes, GENERAL_ORDER_RESERVED_TAIL_OFFSET_V2, GENERAL_ORDER_RESERVED_TAIL_BYTES_V2, 'General order V2 interval tail');
  requireZero(bytes, GENERAL_ORDER_RESERVED_STATE_OFFSET_V2, GENERAL_ORDER_RESERVED_STATE_BYTES_V2, 'General order V2 state tail');
  requireZero(bytes, GENERAL_ORDER_RESERVED_STATE_TAIL_OFFSET_V2, GENERAL_ORDER_RESERVED_STATE_TAIL_BYTES_V2, 'General order V2 state-window tail');

  const phaseByte = bytes[GENERAL_ORDER_STATE_PHASE_OFFSET_V2];
  const phase: GeneralOrderPhaseV2 | null =
    phaseByte === GENERAL_ORDER_STATE_PLACED_V2 ? 'placed'
      : phaseByte === GENERAL_ORDER_STATE_CANCELLED_V2 ? 'cancelled'
        : phaseByte === GENERAL_ORDER_STATE_RELEASED_V2 ? 'released' : null;
  if (phase === null) throw new Error('General order V2 state phase is unknown');

  const admittedSlot = u64(bytes, GENERAL_ORDER_STATE_ADMITTED_SLOT_OFFSET_V2);
  const releasedSlot = u64(bytes, GENERAL_ORDER_STATE_RELEASED_SLOT_OFFSET_V2);
  if (phase === 'placed' ? releasedSlot !== 0n : releasedSlot < admittedSlot) {
    throw new Error('General order V2 state carries a noncanonical release slot for its phase');
  }

  const rows: GeneralOrderRowV2[] = [];
  for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
    const offset = GENERAL_ORDER_ROW_BASE_V2 + GENERAL_ORDER_ROW_STRIDE_V2 * outcome;
    const receive = u64(bytes, offset);
    const deliver = u64(bytes, offset + 8);
    const covers = outcome >= outcomeLo && outcome <= outcomeHi;
    const expectedReceive = covers && side === 'buy' ? claimsPerLot : 0n;
    const expectedDeliver = covers && side === 'sell' ? claimsPerLot : 0n;
    if (receive !== expectedReceive || deliver !== expectedDeliver) {
      throw new Error('General order V2 rows disagree with the shape they derive');
    }
    rows.push(Object.freeze({ receive, deliver }));
  }

  return Object.freeze({
    outcomeCount, nonce, minQuoteCreditPerLot, ownerId, market, batchId, generation,
    maxLots, maxQuoteDebitPerLot, validUntilSlot, side, outcomeLo, outcomeHi, claimsPerLot,
    phase, admittedSlot, releasedSlot, rows: Object.freeze(rows),
  });
}

/** The order's content identity: `sha256` of its 184-byte signed header alone. */
export async function generalOrderIdV2(bytes: Uint8Array): Promise<string> {
  return hex(await sha256(slice(bytes, GENERAL_ORDER_MAGIC_OFFSET_V2, GENERAL_ORDER_HEADER_BYTES_V2)));
}
