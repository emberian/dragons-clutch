import { PublicKey } from '@solana/web3.js';

import {
  previewDirectInlineV3,
  type CompactIntentV2Input,
  type DirectInlineEconomicPreviewV3,
  type DirectInlineHotRouteV3,
  type SignedDirectIntentV3,
} from './directInlineV3';
import {
  requireAuthenticatedDirectMakerNonceV1,
  type AuthenticatedDirectMakerNonceV1,
} from './directMakerReplay';

/**
 * The counterparty ticket.
 *
 * A Direct inline fill settles two SIGNED intents from two distinct makers in
 * one transaction. There is no order book and no indexer — the brief is
 * explicit that resting orders may only ever be an untrusted projection — so
 * the counterparty's half arrives as a portable, self-describing ticket: the
 * maker, their detached Ed25519 signature, and every field of the intent the
 * signature covers. Anyone can carry it (a maker service, a chat message, a
 * file); nothing about it is trusted until the builder re-derives the signing
 * message and the chain verifies the signature natively.
 *
 * The ticket deliberately contains nothing but what the signature already
 * binds. A tampered field changes the signing message and dies at the Ed25519
 * program, so the honest failure mode is a refused transaction, never a
 * different trade.
 */

export const DIRECT_TICKET_KIND_V1 = 'dclutch/direct-intent-ticket/v1';
const MAX_U64 = 18_446_744_073_709_551_615n;

function canonicalKey(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`${field} is missing`);
  let key: string;
  try {
    key = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (key !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

function exactU64Text(value: unknown, field: string): bigint {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be one canonical unsigned decimal string`);
  const parsed = BigInt(value);
  if (parsed > MAX_U64) throw new Error(`${field} exceeds u64`);
  return parsed;
}

function exactUnsigned(value: unknown, field: string, maximum: number): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > maximum) {
    throw new Error(`${field} is not an exact unsigned integer within its width`);
  }
  return value;
}

export function encodeDirectIntentTicketV1(signed: SignedDirectIntentV3): string {
  if (signed.signature.length !== 64 || signed.signature.every((byte) => byte === 0)) {
    throw new Error('ticket requires one nonzero 64-byte detached signature');
  }
  const intent = signed.intent;
  return JSON.stringify({
    kind: DIRECT_TICKET_KIND_V1,
    maker: signed.maker,
    signature: Array.from(signed.signature, (byte) => byte.toString(16).padStart(2, '0')).join(''),
    intent: {
      side: intent.side,
      lifecycle: intent.lifecycle,
      outcome: intent.outcome,
      market: intent.market,
      generation: intent.generation.toString(),
      nonce: intent.nonce.toString(),
      validFrom: intent.validFrom.toString(),
      validThrough: intent.validThrough.toString(),
      maximumFill: intent.maximumFill.toString(),
      limitPrice: intent.limitPrice.toString(),
      feeBasisPoints: intent.feeBasisPoints,
      collateralAccount: intent.collateralAccount,
    },
  }, null, 2);
}

export function decodeDirectIntentTicketV1(text: string): SignedDirectIntentV3 {
  if (text.length === 0 || text.length > 4_096) throw new Error('ticket text is empty or above its explicit 4096-byte bound');
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error('ticket is not valid JSON');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error('ticket must be one JSON object');
  const candidate = value as Record<string, unknown>;
  if (candidate.kind !== DIRECT_TICKET_KIND_V1) throw new Error(`ticket kind is not ${DIRECT_TICKET_KIND_V1}`);
  if (typeof candidate.signature !== 'string' || !/^[0-9a-f]{128}$/.test(candidate.signature) || /^0+$/.test(candidate.signature)) {
    throw new Error('ticket signature must be one nonzero 64-byte lowercase-hex Ed25519 signature');
  }
  if (candidate.intent === null || typeof candidate.intent !== 'object' || Array.isArray(candidate.intent)) throw new Error('ticket intent must be one JSON object');
  const raw = candidate.intent as Record<string, unknown>;
  const side = exactUnsigned(raw.side, 'ticket side', 1) as 0 | 1;
  const lifecycle = exactUnsigned(raw.lifecycle, 'ticket lifecycle', 1) as 0 | 1;
  const intent: CompactIntentV2Input = Object.freeze({
    side,
    lifecycle,
    outcome: exactUnsigned(raw.outcome, 'ticket outcome', 0xffff_ffff),
    market: canonicalKey(raw.market, 'ticket Market'),
    generation: exactU64Text(raw.generation, 'ticket generation'),
    nonce: exactU64Text(raw.nonce, 'ticket nonce'),
    validFrom: exactU64Text(raw.validFrom, 'ticket valid-from slot'),
    validThrough: exactU64Text(raw.validThrough, 'ticket valid-through slot'),
    maximumFill: exactU64Text(raw.maximumFill, 'ticket maximum fill'),
    limitPrice: exactU64Text(raw.limitPrice, 'ticket limit price'),
    feeBasisPoints: exactUnsigned(raw.feeBasisPoints, 'ticket fee basis points', 10_000),
    collateralAccount: canonicalKey(raw.collateralAccount, 'ticket collateral account'),
  });
  return Object.freeze({
    maker: canonicalKey(candidate.maker, 'ticket maker'),
    signature: Uint8Array.from(candidate.signature.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16)),
    intent,
  });
}

/**
 * The largest fill not above `desired` whose fill × price is exactly
 * representable at the immutable price scale. Zero means no admissible fill
 * exists at or below the desired size.
 */
export function largestAdmissibleFillV1(desired: bigint, executionPrice: bigint, priceScale: bigint): bigint {
  if (desired < 0n || executionPrice <= 0n || priceScale <= 0n) throw new Error('fill, price, and scale must be positive exact integers');
  const divisor = gcd(executionPrice, priceScale);
  const step = priceScale / divisor;
  return (desired / step) * step;
}

function gcd(left: bigint, right: bigint): bigint {
  let a = left < 0n ? -left : left;
  let b = right < 0n ? -right : right;
  while (b !== 0n) {
    const next = a % b;
    a = b;
    b = next;
  }
  return a;
}

export type DirectCrossingPlanV1 = Readonly<{
  ticket: SignedDirectIntentV3;
  taker: CompactIntentV2Input;
  fill: bigint;
  executionPrice: bigint;
  preview: DirectInlineEconomicPreviewV3;
  takerSide: 'buy' | 'sell';
  note: string;
}>;

/**
 * Cross the connected wallet against one counterparty ticket.
 *
 * The execution price is the MAKER's signed limit — the counterparty gets
 * exactly the price they signed and the taker takes it or leaves it, which is
 * the only crossing that needs no negotiation round-trip. The taker's own
 * intent is built here to mirror the route and the ticket exactly, sized to
 * the largest admissible fill at or below what the taker asked for.
 */
export function planDirectCrossingV1(input: Readonly<{
  route: Pick<DirectInlineHotRouteV3, 'market' | 'generation' | 'outcomeCount' | 'priceScale' | 'feeBasisPoints'>;
  ticket: SignedDirectIntentV3;
  takerAddress: string;
  /** Opaque next nonce returned by the finalized MakerReplayRootV1 reader. */
  takerReplay: AuthenticatedDirectMakerNonceV1;
  takerCollateralAccount: string;
  desiredFill: bigint;
  clockSlot: bigint;
  validThroughSlots?: bigint;
}>): DirectCrossingPlanV1 {
  const ticket = input.ticket;
  const takerAddress = canonicalKey(input.takerAddress, 'taker address');
  if (takerAddress === ticket.maker) throw new Error('the connected wallet is the ticket maker; a Direct fill needs two distinct makers');
  const takerNonce = requireAuthenticatedDirectMakerNonceV1(input.takerReplay, {
    market: input.route.market,
    generation: input.route.generation,
    maker: takerAddress,
  });
  if (ticket.intent.market !== input.route.market) throw new Error(`ticket is for Market ${ticket.intent.market}, not this Market`);
  if (ticket.intent.generation !== input.route.generation) throw new Error('ticket generation differs from the current Market generation');
  if (ticket.intent.feeBasisPoints !== input.route.feeBasisPoints) throw new Error('ticket fee rate differs from the immutable Direct config');
  if (ticket.intent.outcome >= input.route.outcomeCount) throw new Error('ticket outcome is outside this Product width');
  if (input.clockSlot < ticket.intent.validFrom) throw new Error(`ticket becomes valid at slot ${ticket.intent.validFrom}, after the current finalized slot`);
  if (input.clockSlot > ticket.intent.validThrough) throw new Error(`ticket expired at slot ${ticket.intent.validThrough}`);
  const executionPrice = ticket.intent.limitPrice;
  // A fill-or-kill ticket admits exactly its signed maximum, nothing else.
  const fill = ticket.intent.lifecycle === 0
    ? ticket.intent.maximumFill
    : (() => {
      const bounded = input.desiredFill < ticket.intent.maximumFill ? input.desiredFill : ticket.intent.maximumFill;
      return largestAdmissibleFillV1(bounded, executionPrice, input.route.priceScale);
    })();
  if (fill === 0n) throw new Error('no admissible fill exists at or below the requested size at this exact price scale');
  if (ticket.intent.lifecycle === 0 && input.desiredFill < fill) {
    throw new Error(`the ticket is fill-or-kill for exactly ${ticket.intent.maximumFill}; a smaller fill is not admissible`);
  }
  const takerSide: 'buy' | 'sell' = ticket.intent.side === 0 ? 'buy' : 'sell';
  const taker: CompactIntentV2Input = Object.freeze({
    side: (ticket.intent.side === 0 ? 1 : 0) as 0 | 1,
    lifecycle: 0,
    outcome: ticket.intent.outcome,
    market: input.route.market,
    generation: input.route.generation,
    nonce: takerNonce,
    validFrom: input.clockSlot,
    validThrough: input.clockSlot + (input.validThroughSlots ?? 150n),
    maximumFill: fill,
    limitPrice: executionPrice,
    feeBasisPoints: input.route.feeBasisPoints,
    collateralAccount: canonicalKey(input.takerCollateralAccount, 'taker collateral account'),
  });
  const seller = ticket.intent.side === 0 ? { intent: ticket.intent } : { intent: taker };
  const buyer = ticket.intent.side === 1 ? { intent: ticket.intent } : { intent: taker };
  const preview = previewDirectInlineV3(input.route, seller, buyer, fill, executionPrice, input.clockSlot);
  return Object.freeze({
    ticket,
    taker,
    fill,
    executionPrice,
    preview,
    takerSide,
    note: takerSide === 'buy'
      ? `Buying ${fill} claim atoms of outcome ${ticket.intent.outcome} at the maker's signed price ${executionPrice} (scale ${input.route.priceScale}); exact debit ${preview.buyerCollateralDebit} collateral atoms including ${preview.buyerFee} fee.`
      : `Selling ${fill} claim atoms of outcome ${ticket.intent.outcome} at the maker's signed price ${executionPrice} (scale ${input.route.priceScale}); exact credit ${preview.sellerNetCollateralCredit} collateral atoms after ${preview.sellerFee} fee.`,
  });
}
