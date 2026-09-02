import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  encodeCompactIntentV2,
  encodeDirectInlineOrdinaryRequestV3,
  type CompactIntentV2Input,
  type SignedDirectIntentV3,
} from './directInlineV3';
import * as DirectAbi from './generated/directInlineV3';
import { decodeCompactIntentV2, decodeDirectInlineOrdinaryFillV3 } from './marketActivity';

/**
 * The decoders, held against the encoders they invert.
 *
 * There is exactly one authority for these coordinates —
 * `lib/generated/directInlineV3.ts`, emitted from the Rust that writes them —
 * and two consumers of it, an encoder that has existed since the trade stepper
 * and this reader. So the test that matters is the ROUND TRIP: anything the
 * encoder writes, the reader reads back identically, which is a property no
 * amount of restating offsets in an expectation could establish. If the Rust
 * moves a field, the generated module moves, and both directions move with it;
 * this case fails only when the two directions disagree, which is the only
 * failure it is here to catch.
 *
 * Then the adversarial half, because a decoder that accepts anything is not a
 * decoder: a short wire, a foreign magic, a wrong version, a self-crossing and
 * a pair whose sides are the wrong way round each refuse by name.
 */

const MARKET = new PublicKey('6t3ZnmRuxVKsB4NGrpiQurEwK52xSKVyNqY3tF1ner15').toBase58();
const SELLER = new PublicKey('FBYW95Fo3fyHHkd2ff55Zqr5HvjzGctTacSAgcvkQJ3Q').toBase58();
const BUYER = new PublicKey('BVBriJDjsN7ZhGsJoJ3PET5FdkSKbcn7iDMAjA5tB6ZV').toBase58();
const SELLER_TOKEN = new PublicKey('3ir66Yi6LsLdoJD68msEeBh7xaVJ5zWUPMMRgmcRVqFU').toBase58();
const BUYER_TOKEN = new PublicKey('HJBvqz8qoUPemqDBwucnK7UgLYKsF978YNUxhqrNKkku').toBase58();

function intent(side: 0 | 1, collateralAccount: string): CompactIntentV2Input {
  return Object.freeze({
    side,
    lifecycle: 0,
    outcome: 3,
    market: MARKET,
    generation: 2n,
    nonce: 7n,
    validFrom: 492_090_724n,
    validThrough: 492_290_924n,
    maximumFill: 200n,
    limitPrice: 1_000_000n,
    feeBasisPoints: 50,
    collateralAccount,
  });
}

function signed(maker: string, side: 0 | 1, collateralAccount: string, fill: number): SignedDirectIntentV3 {
  return Object.freeze({
    maker,
    signature: new Uint8Array(64).fill(fill),
    intent: intent(side, collateralAccount),
  });
}

/** The envelope `compileDirectInlineTransactionV3` puts a family request in. */
function hotWire(request: Uint8Array): Uint8Array {
  const bytes = new Uint8Array(DirectAbi.HOT_EXECUTION_ENVELOPE_BYTES_V3 + request.length);
  const view = new DataView(bytes.buffer);
  bytes.set(DirectAbi.HOT_EXECUTION_MAGIC_V3, 0);
  view.setUint16(8, DirectAbi.HOT_EXECUTION_VERSION_V3, true);
  view.setUint16(10, DirectAbi.HOT_EXECUTION_PROFILE_V3, true);
  view.setUint32(12, request.length, true);
  bytes.set(request, DirectAbi.HOT_EXECUTION_ENVELOPE_BYTES_V3);
  return bytes;
}

describe('the signed intent, read back', () => {
  it('round-trips every field the encoder writes', () => {
    for (const side of [0, 1] as const) {
      const original = intent(side, side === 0 ? SELLER_TOKEN : BUYER_TOKEN);
      expect(decodeCompactIntentV2(encodeCompactIntentV2(original))).toEqual(original);
    }
  });

  it('refuses a wire of the wrong width', () => {
    const bytes = encodeCompactIntentV2(intent(0, SELLER_TOKEN));
    expect(() => decodeCompactIntentV2(bytes.slice(0, bytes.length - 1))).toThrow(/bytes and the ABI declares/);
  });

  it('refuses a wire that is not a compact intent at all', () => {
    const bytes = encodeCompactIntentV2(intent(0, SELLER_TOKEN));
    bytes[0] ^= 0xff;
    expect(() => decodeCompactIntentV2(bytes)).toThrow(/compact-intent magic/);
  });

  it('refuses a version this reader was not written for', () => {
    const bytes = encodeCompactIntentV2(intent(0, SELLER_TOKEN));
    new DataView(bytes.buffer).setUint16(8, DirectAbi.COMPACT_INTENT_VERSION_V2 + 1, true);
    expect(() => decodeCompactIntentV2(bytes)).toThrow(/exact compact-intent V2/);
  });
});

describe('one InlineOrdinary crossing, read off its own instruction', () => {
  const seller = signed(SELLER, 0, SELLER_TOKEN, 1);
  const buyer = signed(BUYER, 1, BUYER_TOKEN, 2);
  const wire = hotWire(encodeDirectInlineOrdinaryRequestV3(seller, buyer, 200n, 1_000_000n));

  it('reads both makers, both signed intents and the crossing itself', () => {
    const terms = decodeDirectInlineOrdinaryFillV3(wire);
    expect(terms.seller).toBe(SELLER);
    expect(terms.buyer).toBe(BUYER);
    expect(terms.sellerIntent).toEqual(seller.intent);
    expect(terms.buyerIntent).toEqual(buyer.intent);
    expect(terms.fillAtoms).toBe(200n);
    expect(terms.executionPrice).toBe(1_000_000n);
  });

  it('refuses an instruction that is not the exact InlineOrdinary width', () => {
    expect(() => decodeDirectInlineOrdinaryFillV3(wire.slice(0, wire.length - 8)))
      .toThrow(/an InlineOrdinary fill is/);
  });

  it('refuses an instruction that is not a Hot V3 envelope', () => {
    const foreign = Uint8Array.from(wire);
    foreign[0] ^= 0xff;
    expect(() => decodeDirectInlineOrdinaryFillV3(foreign)).toThrow(/canonical Hot V3 envelope/);
  });

  it('refuses a Hot envelope carrying some other family request', () => {
    const foreign = Uint8Array.from(wire);
    foreign[DirectAbi.HOT_FAMILY_REQUEST_OFFSET_V3] ^= 0xff;
    expect(() => decodeDirectInlineOrdinaryFillV3(foreign)).toThrow(/Direct InlineOrdinary request/);
  });

  it('refuses a Hot envelope carrying another Direct action', () => {
    const foreign = Uint8Array.from(wire);
    new DataView(foreign.buffer).setUint32(
      DirectAbi.HOT_FAMILY_REQUEST_OFFSET_V3 + DirectAbi.DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3,
      DirectAbi.DIRECT_INLINE_ORDINARY_ACTION_V3 + 1,
      true,
    );
    expect(() => decodeDirectInlineOrdinaryFillV3(foreign)).toThrow(/Direct InlineOrdinary request/);
  });

  it('refuses a crossing whose two sides are one identity', () => {
    const aliased = hotWire(encodeDirectInlineOrdinaryRequestV3(
      seller,
      { ...buyer, maker: BUYER },
      200n,
      1_000_000n,
    ));
    // The encoder itself refuses an identical maker, so the alias is written
    // into the wire afterwards: this is a hostile wire, not a hostile call.
    aliased.set(
      new PublicKey(SELLER).toBytes(),
      DirectAbi.HOT_FAMILY_REQUEST_OFFSET_V3
        + DirectAbi.DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3
        + DirectAbi.DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
    );
    expect(() => decodeDirectInlineOrdinaryFillV3(aliased)).toThrow(/one identity on both sides/);
  });

  it('refuses a crossing whose two signed intents are not a Sell and then a Buy', () => {
    const inverted = hotWire(encodeDirectInlineOrdinaryRequestV3(
      signed(SELLER, 1, SELLER_TOKEN, 1),
      signed(BUYER, 0, BUYER_TOKEN, 2),
      200n,
      1_000_000n,
    ));
    expect(() => decodeDirectInlineOrdinaryFillV3(inverted)).toThrow(/one Sell and one Buy/);
  });
});
