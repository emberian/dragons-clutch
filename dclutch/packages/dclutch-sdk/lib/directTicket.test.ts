import { createHash, createPrivateKey, sign } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';

import { Keypair, PublicKey } from '@solana/web3.js';
import { beforeAll, describe, expect, it } from 'vitest';

import {
  decodeDirectIntentTicketV1,
  encodeDirectIntentTicketV1,
  largestAdmissibleFillV1,
  planDirectCrossingV1,
} from './directTicket';
import { encodeCompactIntentSigningMessageV2, type CompactIntentV2Input, type SignedDirectIntentV3 } from './directInlineV3';
import {
  DIRECT_MAKER_REPLAY_BYTES_V1,
  deriveDirectMakerReplayAddressV1,
  inspectDirectMakerNonceV1,
  type AuthenticatedDirectMakerNonceV1,
} from './directMakerReplay';

const MAKER = Keypair.fromSeed(new Uint8Array(32).fill(61)).publicKey.toBase58();
const TAKER = Keypair.fromSeed(new Uint8Array(32).fill(62)).publicKey.toBase58();
const MARKET = Keypair.fromSeed(new Uint8Array(32).fill(63)).publicKey.toBase58();
const MAKER_COLLATERAL = Keypair.fromSeed(new Uint8Array(32).fill(64)).publicKey.toBase58();
const TAKER_COLLATERAL = Keypair.fromSeed(new Uint8Array(32).fill(65)).publicKey.toBase58();
const TRADING = Keypair.fromSeed(new Uint8Array(32).fill(66)).publicKey.toBase58();
const RENT_OWNER = Keypair.fromSeed(new Uint8Array(32).fill(67)).publicKey.toBase58();

const ROUTE = Object.freeze({
  tradingProgram: TRADING,
  market: MARKET,
  generation: 2n,
  outcomeCount: 4,
  priceScale: 1_000_000n,
  feeBasisPoints: 25,
});

let TAKER_REPLAY: AuthenticatedDirectMakerNonceV1;

beforeAll(async () => {
  const { bump } = deriveDirectMakerReplayAddressV1(TRADING, MARKET, ROUTE.generation, TAKER);
  const data = new Uint8Array(DIRECT_MAKER_REPLAY_BYTES_V1);
  data.set(new TextEncoder().encode('DCLTDMR1'), 0);
  new DataView(data.buffer).setUint16(8, 1, true);
  data[10] = bump;
  data.set(new PublicKey(MARKET).toBytes(), 16);
  new DataView(data.buffer).setBigUint64(48, ROUTE.generation, true);
  data.set(new PublicKey(TAKER).toBytes(), 56);
  new DataView(data.buffer).setBigUint64(88, 7n, true);
  data.set(new PublicKey(RENT_OWNER).toBytes(), 112);
  new DataView(data.buffer).setBigUint64(144, 2_000_000n, true);
  TAKER_REPLAY = await inspectDirectMakerNonceV1({
    finalizedSlot: async () => '100',
    accountInfo: async () => Object.freeze({
      slot: '100',
      account: Object.freeze({ data, executable: false, lamports: '2000000', owner: TRADING, space: data.length }),
    }),
  }, { tradingProgram: TRADING, market: MARKET, generation: ROUTE.generation, maker: TAKER });
});

function sellerTicket(overrides?: Partial<CompactIntentV2Input>): SignedDirectIntentV3 {
  return Object.freeze({
    maker: MAKER,
    signature: new Uint8Array(64).fill(9),
    intent: Object.freeze({
      side: 0 as const,
      lifecycle: 1 as const,
      outcome: 2,
      market: MARKET,
      generation: 2n,
      nonce: 0n,
      validFrom: 10n,
      validThrough: 500n,
      maximumFill: 10_000n,
      limitPrice: 500_000n,
      feeBasisPoints: 25,
      collateralAccount: MAKER_COLLATERAL,
      ...overrides,
    }),
  });
}

describe('the counterparty ticket', () => {
  it('round-trips exactly through its portable JSON form', () => {
    const ticket = sellerTicket();
    const text = encodeDirectIntentTicketV1(ticket);
    const decoded = decodeDirectIntentTicketV1(text);
    expect(decoded.maker).toBe(MAKER);
    expect(Array.from(decoded.signature)).toEqual(Array.from(ticket.signature));
    expect(decoded.intent).toEqual(ticket.intent);
  });

  it('refuses a tampered kind, signature, or noncanonical field by name', () => {
    const text = encodeDirectIntentTicketV1(sellerTicket());
    expect(() => decodeDirectIntentTicketV1(text.replace('dclutch/direct-intent-ticket/v1', 'x/y'))).toThrow('ticket kind');
    const zeroSig = JSON.parse(text) as { signature: string };
    zeroSig.signature = '0'.repeat(128);
    expect(() => decodeDirectIntentTicketV1(JSON.stringify(zeroSig))).toThrow('nonzero');
    const badFill = JSON.parse(text) as { intent: { maximumFill: string } };
    badFill.intent.maximumFill = '-5';
    expect(() => decodeDirectIntentTicketV1(JSON.stringify(badFill))).toThrow('canonical unsigned decimal');
  });

  it('floors a fill to the largest size exactly representable at the price scale', () => {
    // price 500_000 over scale 1_000_000 needs even fills.
    expect(largestAdmissibleFillV1(999n, 500_000n, 1_000_000n)).toBe(998n);
    expect(largestAdmissibleFillV1(1n, 500_000n, 1_000_000n)).toBe(0n);
    // price 250_000 needs multiples of 4.
    expect(largestAdmissibleFillV1(11n, 250_000n, 1_000_000n)).toBe(8n);
    // a price equal to the scale admits any fill.
    expect(largestAdmissibleFillV1(7n, 1_000_000n, 1_000_000n)).toBe(7n);
  });

  it('crosses a buy against a seller ticket at the maker signed price with exact arithmetic', () => {
    const plan = planDirectCrossingV1({
      route: ROUTE,
      ticket: sellerTicket(),
      takerAddress: TAKER,
      takerReplay: TAKER_REPLAY,
      takerCollateralAccount: TAKER_COLLATERAL,
      desiredFill: 2_000n,
      clockSlot: 100n,
    });
    expect(plan.takerSide).toBe('buy');
    expect(plan.fill).toBe(2_000n);
    expect(plan.executionPrice).toBe(500_000n);
    expect(plan.preview.grossCollateral).toBe(1_000n);
    expect(plan.preview.buyerCollateralDebit).toBe(1_002n);
    expect(plan.preview.sellerNetCollateralCredit).toBe(998n);
    expect(plan.taker.side).toBe(1);
    expect(plan.taker.lifecycle).toBe(0);
    expect(plan.taker.outcome).toBe(2);
    expect(plan.taker.nonce).toBe(7n);
    expect(plan.note).toContain('exact debit 1002');
  });

  it('honors a fill-or-kill ticket exactly and refuses a smaller ask', () => {
    const fok = sellerTicket({ lifecycle: 0 as const, maximumFill: 4_000n });
    expect(() => planDirectCrossingV1({
      route: ROUTE, ticket: fok, takerAddress: TAKER, takerReplay: TAKER_REPLAY, takerCollateralAccount: TAKER_COLLATERAL,
      desiredFill: 2_000n, clockSlot: 100n,
    })).toThrow('fill-or-kill');
    const plan = planDirectCrossingV1({
      route: ROUTE, ticket: fok, takerAddress: TAKER, takerReplay: TAKER_REPLAY, takerCollateralAccount: TAKER_COLLATERAL,
      desiredFill: 4_000n, clockSlot: 100n,
    });
    expect(plan.fill).toBe(4_000n);
  });

  it('refuses a ticket for another Market, an expired window, a foreign fee, or a self-cross', () => {
    const base = { route: ROUTE, takerAddress: TAKER, takerReplay: TAKER_REPLAY, takerCollateralAccount: TAKER_COLLATERAL, desiredFill: 100n, clockSlot: 100n };
    expect(() => planDirectCrossingV1({ ...base, ticket: sellerTicket({ market: TAKER }) })).toThrow('not this Market');
    expect(() => planDirectCrossingV1({ ...base, ticket: sellerTicket(), clockSlot: 501n })).toThrow('expired');
    expect(() => planDirectCrossingV1({ ...base, ticket: sellerTicket({ feeBasisPoints: 30 }) })).toThrow('fee rate differs');
    expect(() => planDirectCrossingV1({ ...base, ticket: sellerTicket(), takerAddress: MAKER })).toThrow('two distinct makers');
  });
});

/**
 * THE TWO-SIDED TICKET VECTOR.
 *
 * Until 2026-08-30 the browser trade panel was the ONLY code in this repository
 * that could WRITE a Direct ticket; every tool could only read one, which is why
 * `devnet-direct-trade-produce-v1` demanded `--seller-ticket` and
 * `--buyer-ticket` as inputs nothing under `tools/` could produce. The Rust
 * author in `crates/dclutch-direct-ticket` is the second writer — reached as
 * `dclutch ticket author` in the released CLI and as
 * `direct-intent-ticket-author-v1` in the operator binary, ONE author behind
 * both — and this vector is how the two languages are held to one wire.
 *
 * TypeScript emits and Rust reproduces, because TypeScript is the incumbent:
 * its bytes are the ones a chain has already been asked to accept. If this test
 * changes the fixture, the Rust tests in
 * `crates/dclutch-direct-ticket/tests/portable_ticket_v1.rs` and
 * `tools/dclutch-cli/tests/ticket_v1.rs` go red next
 * — which is the correct order, and the reason the fixture is not written from
 * the Rust side.
 *
 * The 172-byte SIGNING MESSAGE underneath is owned by neither language but by
 * `formal/dclutch-semantics/EmitDirectIntentV2Rust.lean`, whose emitted Rust
 * constants `lib/generated/directInlineV3.ts` mirrors. `signatureHex` is
 * therefore a real check on that message: Ed25519 is deterministic (RFC 8032),
 * so node's `crypto.sign` here and `ed25519-dalek` there can only agree if both
 * sides built the same bytes.
 *
 * Regenerate with `DCLUTCH_WRITE_TICKET_VECTOR=1` and only when the wire
 * deliberately moved. Every intent field below is distinct and none is a
 * default, so a swapped pair of offsets in either encoder cannot survive.
 */
const VECTOR_PATH = new URL('../fixtures/direct-intent-ticket.json', import.meta.url);
const PKCS8_ED25519_PREFIX = '302e020100300506032b657004220420';
const MAKER_SEED_FILL = 61;
const MARKET_SEED_FILL = 63;
const COLLATERAL_SEED_FILL = 64;

function seededAddress(fill: number): string {
  return Keypair.fromSeed(new Uint8Array(32).fill(fill)).publicKey.toBase58();
}

function ed25519Sign(seedFill: number, message: Uint8Array): Uint8Array {
  const key = createPrivateKey({
    key: Buffer.concat([Buffer.from(PKCS8_ED25519_PREFIX, 'hex'), Buffer.alloc(32, seedFill)]),
    format: 'der',
    type: 'pkcs8',
  });
  return new Uint8Array(sign(null, Buffer.from(message), key));
}

function vectorIntent(): CompactIntentV2Input {
  return Object.freeze({
    side: 0 as const,
    lifecycle: 1 as const,
    outcome: 3,
    market: seededAddress(MARKET_SEED_FILL),
    generation: 7n,
    nonce: 9n,
    validFrom: 11n,
    validThrough: 4_294_967_295n,
    maximumFill: 100_000_000n,
    limitPrice: 500_000n,
    feeBasisPoints: 50,
    collateralAccount: seededAddress(COLLATERAL_SEED_FILL),
  });
}

describe('the portable ticket wire the Rust author has to match', () => {
  it('emits the exact fixture both languages are pinned to', () => {
    const intent = vectorIntent();
    const maker = seededAddress(MAKER_SEED_FILL);
    const signature = ed25519Sign(MAKER_SEED_FILL, encodeCompactIntentSigningMessageV2(intent));
    const ticketText = encodeDirectIntentTicketV1(Object.freeze({ maker, signature, intent }));
    const produced = {
      format: 'dclutch/direct-intent-ticket-vector/v1',
      note: 'Two-sided vector for the portable Direct intent ticket. Emitted by directTicket.test.ts through the SAME encodeDirectIntentTicketV1 the browser trade panel calls, and reproduced byte-for-byte by the Rust author in crates/dclutch-direct-ticket, which both `dclutch ticket author` (the released CLI) and direct-intent-ticket-author-v1 (the operator binary) call — one author, two invocations. TypeScript is the incumbent producer and therefore the authority for the JSON envelope; the 172-byte signing message underneath is owned by neither language but by formal/dclutch-semantics/EmitDirectIntentV2Rust.lean, whose emitted Rust constants lib/generated/directInlineV3.ts mirrors. The three seed fills are 32-byte Ed25519 seeds supplied as INPUTS; maker, market, collateralAccount, signatureHex, ticketText and ticketSha256 are what the encoders produced from them. ticketText carries no trailing newline because JSON.stringify(value, null, 2) emits none.',
      makerSeedFill: MAKER_SEED_FILL,
      marketSeedFill: MARKET_SEED_FILL,
      collateralSeedFill: COLLATERAL_SEED_FILL,
      side: intent.side,
      lifecycle: intent.lifecycle,
      outcome: intent.outcome,
      generation: Number(intent.generation),
      nonce: Number(intent.nonce),
      validFrom: Number(intent.validFrom),
      validThrough: Number(intent.validThrough),
      maximumFill: Number(intent.maximumFill),
      limitPrice: Number(intent.limitPrice),
      feeBasisPoints: intent.feeBasisPoints,
      maker,
      market: intent.market,
      collateralAccount: intent.collateralAccount,
      signatureHex: Array.from(signature, (byte) => byte.toString(16).padStart(2, '0')).join(''),
      ticketText,
      ticketSha256: createHash('sha256').update(ticketText, 'utf8').digest('hex'),
    };
    const body = `${JSON.stringify(produced, null, 2)}\n`;
    if (process.env.DCLUTCH_WRITE_TICKET_VECTOR === '1') writeFileSync(VECTOR_PATH, body);
    expect(readFileSync(VECTOR_PATH, 'utf8')).toBe(body);
    expect(ticketText.endsWith('\n')).toBe(false);
    expect(decodeDirectIntentTicketV1(ticketText).intent).toEqual(intent);
  });
});
