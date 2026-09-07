import { hex, isZero, pubkey, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';

/**
 * The joint clearing's two client-facing records (cohort-18,
 * `MECHANISM_JOINT_CLEARING_2026_09_04.md`).
 *
 * There is no generated TypeScript ABI companion for either wire: the Lean
 * emitters under `formal/dclutch-semantics/` target Rust only
 * (`EmitGeneralOrderV2AbiRust.lean`, `EmitClearingPriceV1AbiRust.lean`), so
 * the byte coordinates below are hand-kept against their authorities —
 * `crates/dclutch-trading/src/general/generated_order_v2.rs`,
 * `generated_clearing_price_v1.rs`, and the decode rules in
 * `crates/dclutch-trading/src/general/collection_v1.rs`
 * (`GeneralBatchV2::decode`, `GeneralOrderV2::decode`) — the same way this
 * package hand-keeps a handful of other local offsets beside its generated
 * modules (see `dealerEquityV3.ts`, `dealerAccountProfileV3.ts`).
 */

const RECORD_VERSION_V2 = 2;

// Batch record (`DCGBAT02`) — `generated_clearing_price_v1.rs` for the
// clearing tail at and after byte 224, `GeneralBatchLayoutV2` in
// `collection_v1.rs` for the V1 prefix. The whole record is `296 + 16N`.
const BATCH_MAGIC_V2 = new TextEncoder().encode('DCGBAT02');
const BATCH_PHASE_V2 = 20;
const BATCH_OUTCOME_COUNT = 12;
const BATCH_SEQUENCE = 16;
const BATCH_GENERATION = 24;
const BATCH_MARKET = 32;
const BATCH_PRODUCT_ID = 64;
const BATCH_CONFIG_ID = 96;
const BATCH_PRICE_SCALE = 128;
const BATCH_COLLECTION_CLOSE_SLOT = 136;
const BATCH_MAX_ORDERS = 144;
const BATCH_SETTLEMENT_CLOSE_SLOT = 152;
const BATCH_STATUS = 160;
const BATCH_ORDER_COUNT = 164;
const BATCH_OPENED_ROOT_REVISION = 168;
const BATCH_CLOSED_ROOT_REVISION = 176;
const BATCH_COMMITTED_QUOTE_RESERVE = 184;
const BATCH_CANCELLED_COUNT = 192;
const BATCH_V1_BYTES = 224;
const BATCH_CLEARED_CANDIDATE_ID = 224;
const BATCH_CLEARED_SLOT = 256;
const BATCH_SETS_MOVE = 264;
const BATCH_SETS_QUANTITY = 272;
const BATCH_FILLED_LOTS = 280;
const BATCH_LIVE_ORDER_COUNT = 288;
const BATCH_PRICES_BASE = 296;
const BATCH_TAIL_STRIDE = 8;

const BATCH_STATUS_COLLECTING = 1;
const BATCH_STATUS_CLOSED = 2;
const BATCH_STATUS_CLEARED = 3;
const CLEARING_MOVE_NONE = 0;
const CLEARING_MOVE_MINT = 1;
const CLEARING_MOVE_MERGE = 2;

// Order record (`DCGORD02`) — `generated_order_v2.rs`. The header (the
// identity preimage) is the first 184 bytes; the state window follows at
// 184; the derived `(receive, deliver)` rows begin at 216, stride 16.
const ORDER_MAGIC_V2 = new TextEncoder().encode('DCGORD02');
const ORDER_PHASE_V2 = 21;
const ORDER_OUTCOME_COUNT = 12;
const ORDER_NONCE = 16;
const ORDER_MIN_QUOTE_CREDIT_PER_LOT = 24;
const ORDER_OWNER_ID = 32;
const ORDER_MARKET = 64;
const ORDER_BATCH_ID = 96;
const ORDER_GENERATION = 128;
const ORDER_MAX_LOTS = 136;
const ORDER_MAX_QUOTE_DEBIT_PER_LOT = 144;
const ORDER_VALID_UNTIL_SLOT = 152;
const ORDER_SIDE = 160;
const ORDER_OUTCOME_LO = 164;
const ORDER_OUTCOME_HI = 168;
const ORDER_CLAIMS_PER_LOT = 176;
const ORDER_HEADER_BYTES = 184;
const ORDER_STATE_PHASE = 184;
const ORDER_STATE_ADMITTED_SLOT = 192;
const ORDER_STATE_RELEASED_SLOT = 200;
const ORDER_ROW_BASE = 216;
const ORDER_ROW_STRIDE = 16;

const ORDER_SIDE_BUY = 1;
const ORDER_SIDE_SELL = 2;
const ORDER_STATE_PLACED = 1;
const ORDER_STATE_CANCELLED = 2;
const ORDER_STATE_RELEASED = 3;

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

function requireHeader(bytes: Uint8Array, magic: Uint8Array, phase: number, label: string): void {
  if (!same(slice(bytes, 0, 8), magic) || u16(bytes, 8) !== RECORD_VERSION_V2 || bytes[10] !== phase || bytes[11] !== 0) {
    throw new Error(`${label} body is not exact V2`);
  }
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
  if (tag === CLEARING_MOVE_NONE) return 'none';
  if (tag === CLEARING_MOVE_MINT) return 'mint';
  if (tag === CLEARING_MOVE_MERGE) return 'merge';
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
  if (bytes.length < BATCH_V1_BYTES) throw new Error('General batch V2 body is truncated before its clearing tail');
  requireHeader(bytes, BATCH_MAGIC_V2, BATCH_PHASE_V2, 'General batch V2');
  requireZero(bytes, 148, 4, 'General batch V2 admission-bound tail');
  requireZero(bytes, 161, 3, 'General batch V2 status tail');
  requireZero(bytes, 196, 28, 'General batch V2 counters tail');

  const outcomeCount = readU32(bytes, BATCH_OUTCOME_COUNT);
  const sequence = u64(bytes, BATCH_SEQUENCE);
  const generation = u64(bytes, BATCH_GENERATION);
  const market = address32(bytes, BATCH_MARKET, 'batch Market');
  const productId = id32(bytes, BATCH_PRODUCT_ID, 'batch Product');
  const configId = id32(bytes, BATCH_CONFIG_ID, 'batch config');
  const priceScale = u64(bytes, BATCH_PRICE_SCALE);
  const collectionCloseSlot = u64(bytes, BATCH_COLLECTION_CLOSE_SLOT);
  const maxOrders = readU32(bytes, BATCH_MAX_ORDERS);
  const settlementCloseSlot = u64(bytes, BATCH_SETTLEMENT_CLOSE_SLOT);

  if (outcomeCount === 0 || priceScale === 0n || maxOrders === 0 || generation === 0n) {
    throw new Error('General batch V2 opening carries a zero outcome count, price scale, max orders, or generation');
  }
  if (settlementCloseSlot <= collectionCloseSlot) {
    throw new Error('General batch V2 settlement window does not close after its collection window');
  }

  const expectedLength = BATCH_PRICES_BASE + 2 * BATCH_TAIL_STRIDE * outcomeCount;
  if (bytes.length !== expectedLength) throw new Error('General batch V2 runtime width does not match its outcome count');

  requireZero(bytes, BATCH_SETS_MOVE + 1, 7, 'General batch V2 sets-move tail');
  requireZero(bytes, BATCH_LIVE_ORDER_COUNT + 4, 4, 'General batch V2 live-order-count tail');

  const statusByte = bytes[BATCH_STATUS];
  const status: GeneralBatchStatusTagV2 | null =
    statusByte === BATCH_STATUS_COLLECTING ? 'collecting'
      : statusByte === BATCH_STATUS_CLOSED ? 'closed'
        : statusByte === BATCH_STATUS_CLEARED ? 'cleared' : null;
  if (status === null) throw new Error('General batch V2 status is unknown');

  const orderCount = readU32(bytes, BATCH_ORDER_COUNT);
  const openedRootRevision = u64(bytes, BATCH_OPENED_ROOT_REVISION);
  const closedRootRevision = u64(bytes, BATCH_CLOSED_ROOT_REVISION);
  const committedQuoteReserve = u64(bytes, BATCH_COMMITTED_QUOTE_RESERVE);
  const cancelledCount = readU32(bytes, BATCH_CANCELLED_COUNT);

  if (orderCount > maxOrders || cancelledCount > orderCount) {
    throw new Error('General batch V2 admission counters exceed their bound');
  }
  if (openedRootRevision === 0n) throw new Error('General batch V2 carries a zero opened root revision');
  if (status === 'collecting' ? closedRootRevision !== 0n : closedRootRevision <= openedRootRevision) {
    throw new Error('General batch V2 lifecycle revision is noncanonical for its status');
  }

  const liveOrderCount = orderCount - cancelledCount;

  const clearedCandidateIdBytes = slice(bytes, BATCH_CLEARED_CANDIDATE_ID, 32);
  const clearedSlot = u64(bytes, BATCH_CLEARED_SLOT);
  const setsMoveByte = bytes[BATCH_SETS_MOVE];
  const setsQuantity = u64(bytes, BATCH_SETS_QUANTITY);
  const filledLots = u64(bytes, BATCH_FILLED_LOTS);
  const clearingLiveOrderCount = readU32(bytes, BATCH_LIVE_ORDER_COUNT);

  const tailBytesAreZero = isZero(slice(bytes, BATCH_PRICES_BASE, 2 * BATCH_TAIL_STRIDE * outcomeCount));
  const tailIsVacant = isZero(clearedCandidateIdBytes) && clearedSlot === 0n && setsMoveByte === CLEARING_MOVE_NONE
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

  const residualBase = BATCH_PRICES_BASE + BATCH_TAIL_STRIDE * outcomeCount;
  const prices: bigint[] = [];
  const residual: bigint[] = [];
  let total = 0n;
  for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
    const price = u64(bytes, BATCH_PRICES_BASE + BATCH_TAIL_STRIDE * outcome);
    const strand = u64(bytes, residualBase + BATCH_TAIL_STRIDE * outcome);
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
  if (bytes.length < ORDER_ROW_BASE) throw new Error('General order V2 body is truncated before its rows');
  requireHeader(bytes, ORDER_MAGIC_V2, ORDER_PHASE_V2, 'General order V2');

  const outcomeCount = readU32(bytes, ORDER_OUTCOME_COUNT);
  const nonce = u64(bytes, ORDER_NONCE);
  const minQuoteCreditPerLot = u64(bytes, ORDER_MIN_QUOTE_CREDIT_PER_LOT);
  const ownerId = address32(bytes, ORDER_OWNER_ID, 'order owner');
  const market = address32(bytes, ORDER_MARKET, 'order Market');
  const batchId = id32(bytes, ORDER_BATCH_ID, 'order Batch');
  const generation = u64(bytes, ORDER_GENERATION);
  const maxLots = u64(bytes, ORDER_MAX_LOTS);
  const maxQuoteDebitPerLot = u64(bytes, ORDER_MAX_QUOTE_DEBIT_PER_LOT);
  const validUntilSlot = u64(bytes, ORDER_VALID_UNTIL_SLOT);

  const sideByte = bytes[ORDER_SIDE];
  const side: GeneralOrderSideV2 | null = sideByte === ORDER_SIDE_BUY ? 'buy' : sideByte === ORDER_SIDE_SELL ? 'sell' : null;
  if (side === null) throw new Error('General order V2 side tag is unknown');

  const outcomeLo = readU32(bytes, ORDER_OUTCOME_LO);
  const outcomeHi = readU32(bytes, ORDER_OUTCOME_HI);
  const claimsPerLot = u64(bytes, ORDER_CLAIMS_PER_LOT);

  if (outcomeCount === 0 || maxLots === 0n || generation === 0n) {
    throw new Error('General order V2 header carries a zero outcome count, max lots, or generation');
  }
  if (bytes.length !== ORDER_ROW_BASE + ORDER_ROW_STRIDE * outcomeCount) {
    throw new Error('General order V2 runtime width does not match its outcome count');
  }
  if (outcomeLo > outcomeHi || outcomeHi >= outcomeCount || claimsPerLot === 0n) {
    throw new Error('General order V2 shape is not a nonempty interval inside its width');
  }

  requireZero(bytes, ORDER_SIDE + 1, 3, 'General order V2 shape tail');
  requireZero(bytes, ORDER_OUTCOME_HI + 4, 4, 'General order V2 interval tail');
  requireZero(bytes, ORDER_STATE_PHASE + 1, 7, 'General order V2 state tail');
  requireZero(bytes, ORDER_STATE_RELEASED_SLOT + 8, 8, 'General order V2 state-window tail');

  const phaseByte = bytes[ORDER_STATE_PHASE];
  const phase: GeneralOrderPhaseV2 | null =
    phaseByte === ORDER_STATE_PLACED ? 'placed'
      : phaseByte === ORDER_STATE_CANCELLED ? 'cancelled'
        : phaseByte === ORDER_STATE_RELEASED ? 'released' : null;
  if (phase === null) throw new Error('General order V2 state phase is unknown');

  const admittedSlot = u64(bytes, ORDER_STATE_ADMITTED_SLOT);
  const releasedSlot = u64(bytes, ORDER_STATE_RELEASED_SLOT);
  if (phase === 'placed' ? releasedSlot !== 0n : releasedSlot < admittedSlot) {
    throw new Error('General order V2 state carries a noncanonical release slot for its phase');
  }

  const rows: GeneralOrderRowV2[] = [];
  for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
    const offset = ORDER_ROW_BASE + ORDER_ROW_STRIDE * outcome;
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
  return hex(await sha256(slice(bytes, 0, ORDER_HEADER_BYTES)));
}
