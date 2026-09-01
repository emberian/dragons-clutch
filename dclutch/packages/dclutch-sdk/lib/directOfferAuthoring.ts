import { PublicKey } from '@solana/web3.js';

import {
  encodeCompactIntentSigningMessageV2,
  type CompactIntentV2Input,
  type SignedDirectIntentV3,
} from './directInlineV3';
import {
  requireAuthenticatedDirectMakerNonceV1,
  type AuthenticatedDirectMakerNonceV1,
} from './directMakerReplay';
import {
  decodeDirectIntentTicketV1,
  encodeDirectIntentTicketV1,
} from './directTicket';
import { type DirectSellerReadinessV1 } from './directParticipant';

const U64_MAX_V1 = BigInt('18446744073709551615');

export type ReadyDirectSellerV1 = Extract<DirectSellerReadinessV1, Readonly<{ status: 'ready' }>>;

export type DirectSellOfferDraftV1 = Readonly<{
  intent: CompactIntentV2Input;
  signingMessage: Uint8Array;
  observedSlot: bigint;
  availableClaims: bigint;
  collateralPrestate: 'vacant' | 'initialized';
}>;

export type AuthoredDirectSellOfferV1 = Readonly<{
  ticket: SignedDirectIntentV3;
  text: string;
  signingMessage: Uint8Array;
}>;

function exactAddress(value: string, field: string): string {
  let canonical: string;
  try {
    canonical = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (canonical !== value) throw new Error(`${field} must be canonical base58 text`);
  return canonical;
}

function exactSlot(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} is not canonical unsigned decimal text`);
  const slot = BigInt(value);
  if (slot > U64_MAX_V1) throw new Error(`${field} exceeds u64`);
  return slot;
}

/**
 * Bind human-selected sell terms to two authenticated chain observations.
 *
 * Seller readiness owns the Claims balance and canonical destination token
 * account. Maker replay owns the one next nonce. Neither is accepted as a
 * caller-assembled DTO, and the resulting signing preimage is produced by the
 * canonical Direct codec before this function returns.
 */
export function composeDirectSellOfferV1(input: Readonly<{
  route: Readonly<{
    market: string;
    generation: bigint;
    outcomeCount: number;
    priceScale: bigint;
    feeBasisPoints: number;
    tradingProgram: string;
  }>;
  maker: string;
  seller: ReadyDirectSellerV1;
  replay: AuthenticatedDirectMakerNonceV1;
  outcome: number;
  maximumFill: bigint;
  limitPrice: bigint;
  lifecycle: 0 | 1;
  durationSlots: bigint;
}>): DirectSellOfferDraftV1 {
  const maker = exactAddress(input.maker, 'offer maker');
  const routeMarket = exactAddress(input.route.market, 'offer Market');
  exactAddress(input.route.tradingProgram, 'Trading program');
  if (!Number.isSafeInteger(input.route.outcomeCount) || input.route.outcomeCount <= 0) {
    throw new Error('offer route has no exact positive outcome count');
  }
  if (!Number.isSafeInteger(input.outcome) || input.outcome < 0
      || input.outcome >= input.route.outcomeCount) {
    throw new Error('selected outcome is outside this Market');
  }
  if (input.seller.owner !== maker || input.seller.market !== routeMarket
      || input.seller.generation !== input.route.generation) {
    throw new Error('seller readiness belongs to another maker, Market, or generation');
  }
  if (input.seller.positionBalances.length !== input.route.outcomeCount) {
    throw new Error('seller Claims Position has another outcome width');
  }
  const availableClaims = input.seller.positionBalances[input.outcome];
  if (availableClaims === undefined || availableClaims < BigInt(0) || availableClaims > U64_MAX_V1) {
    throw new Error('seller Claims balance is outside the protocol u64 width');
  }
  if (input.maximumFill <= BigInt(0) || input.maximumFill > U64_MAX_V1) {
    throw new Error('offer size must be one positive u64 amount of claims');
  }
  if (input.maximumFill > availableClaims) {
    throw new Error('offer size exceeds the claims this seller holds for the selected outcome');
  }
  if (input.route.priceScale <= BigInt(0) || input.route.priceScale > U64_MAX_V1
      || input.limitPrice <= BigInt(0) || input.limitPrice > input.route.priceScale) {
    throw new Error('offer price is outside this Market\'s exact price scale');
  }
  if (!Number.isSafeInteger(input.route.feeBasisPoints)
      || input.route.feeBasisPoints < 0 || input.route.feeBasisPoints > 10_000) {
    throw new Error('offer route fee is outside 0 through 10000 basis points');
  }
  if (input.lifecycle !== 0 && input.lifecycle !== 1) {
    throw new Error('offer lifecycle must be fill-or-kill or immediate-or-cancel');
  }
  if (input.durationSlots <= BigInt(0) || input.durationSlots > U64_MAX_V1) {
    throw new Error('offer duration must be one positive u64 number of slots');
  }

  const nonce = requireAuthenticatedDirectMakerNonceV1(input.replay, {
    tradingProgram: input.route.tradingProgram,
    market: routeMarket,
    generation: input.route.generation,
    maker,
  });
  const observedSlot = [
    exactSlot(input.seller.observedSlot, 'seller observation slot'),
    exactSlot(input.replay.observedSlot, 'maker replay observation slot'),
  ].reduce((largest, slot) => slot > largest ? slot : largest, BigInt(0));
  if (input.durationSlots > U64_MAX_V1 - observedSlot) {
    throw new Error('offer duration would carry its valid-through slot outside u64');
  }

  const intent: CompactIntentV2Input = Object.freeze({
    side: 0,
    lifecycle: input.lifecycle,
    outcome: input.outcome,
    market: routeMarket,
    generation: input.route.generation,
    nonce,
    validFrom: observedSlot,
    validThrough: observedSlot + input.durationSlots,
    maximumFill: input.maximumFill,
    limitPrice: input.limitPrice,
    feeBasisPoints: input.route.feeBasisPoints,
    collateralAccount: exactAddress(input.seller.coordinates.collateral, 'seller Direct token account'),
  });
  const signingMessage = encodeCompactIntentSigningMessageV2(intent);
  return Object.freeze({
    intent,
    signingMessage,
    observedSlot,
    availableClaims,
    collateralPrestate: input.seller.collateralPrestate,
  });
}

/**
 * Seal one detached wallet signature into the canonical portable ticket and
 * immediately decode the authored text through the same decoder takers use.
 * "Authored" means exact and well-formed; only the chain verifies it.
 */
export function sealDirectSellOfferV1(
  maker: string,
  draft: DirectSellOfferDraftV1,
  signature: Uint8Array,
): AuthoredDirectSellOfferV1 {
  const signed: SignedDirectIntentV3 = Object.freeze({
    maker: exactAddress(maker, 'offer maker'),
    signature: new Uint8Array(signature),
    intent: draft.intent,
  });
  const text = encodeDirectIntentTicketV1(signed);
  const ticket = decodeDirectIntentTicketV1(text);
  const message = encodeCompactIntentSigningMessageV2(ticket.intent);
  if (message.length !== draft.signingMessage.length
      || message.some((byte, index) => byte !== draft.signingMessage[index])) {
    throw new Error('authored ticket did not preserve the exact wallet signing message');
  }
  return Object.freeze({ ticket, text, signingMessage: new Uint8Array(message) });
}
