import { PublicKey } from '@solana/web3.js';

import { isZero, requireZero, slice, u16, u64 } from './bytes';
import { SYSTEM_PROGRAM_ID } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * Exact MakerReplayRootV1 authority mirrored from
 * `crates/dclutch-trading/src/successor.rs` and its generated ABI.
 */
export const DIRECT_MAKER_REPLAY_BYTES_V1 = 160;
/**
 * The pre-`fee_owed` width, still readable.
 *
 * `MakerReplayRootV1::decode` accepts both and reads `fee_owed = 0` at the
 * legacy width, so an exterior reader never has to know which one it holds.
 */
export const DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1 = 152;
export const DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1 = new TextEncoder().encode('dclutch:direct-maker:v1');

const DIRECT_MAKER_MAGIC_V1 = new TextEncoder().encode('DCLTDMR1');
const DIRECT_MAKER_VERSION_V1 = 1;
const DIRECT_MAKER_BUMP_OFFSET_V1 = 10;
const DIRECT_MAKER_RESERVED_OFFSET_V1 = 11;
const DIRECT_MAKER_MARKET_OFFSET_V1 = 16;
const DIRECT_MAKER_GENERATION_OFFSET_V1 = 48;
const DIRECT_MAKER_IDENTITY_OFFSET_V1 = 56;
const DIRECT_MAKER_NEXT_NONCE_OFFSET_V1 = 88;
const DIRECT_MAKER_LIVE_COUNT_OFFSET_V1 = 96;
const DIRECT_MAKER_MINIMUM_LIVE_NONCE_OFFSET_V1 = 104;
const DIRECT_MAKER_RENT_OWNER_OFFSET_V1 = 112;
const DIRECT_MAKER_RENT_PRINCIPAL_OFFSET_V1 = 144;
const DIRECT_MAKER_FEE_OWED_OFFSET_V1 = 152;
const U64_MAX = 0xffff_ffff_ffff_ffffn;

const authenticatedDirectMakerNonceV1: unique symbol = Symbol('authenticated Direct maker nonce V1');
const authenticatedDirectMakerNoncePairV1: unique symbol = Symbol('authenticated Direct maker nonce pair V1');

export type AuthenticatedDirectMakerNonceV1 = Readonly<{
  address: string;
  tradingProgram: string;
  market: string;
  generation: bigint;
  maker: string;
  observedSlot: string;
  nextNonce: bigint;
  /** Unsettled Direct fee this maker owes; a legacy-width record reads zero. */
  feeOwed: bigint;
  state: 'vacant' | 'existing';
  [authenticatedDirectMakerNonceV1]: true;
}>;

export type DirectMakerNonceRequestV1 = Readonly<{
  tradingProgram: string;
  market: string;
  generation: bigint;
  maker: string;
}>;

export type AuthenticatedDirectMakerNoncePairV1 = readonly [
  AuthenticatedDirectMakerNonceV1,
  AuthenticatedDirectMakerNonceV1,
] & Readonly<{ [authenticatedDirectMakerNoncePairV1]: true }>;

function canonicalKey(value: string, field: string): PublicKey {
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

function u64Text(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} is not canonical unsigned decimal text`);
  const parsed = BigInt(value);
  if (parsed > U64_MAX) throw new Error(`${field} exceeds u64`);
  return parsed;
}

function generationBytes(generation: bigint): Uint8Array {
  if (generation < 0n || generation > U64_MAX) throw new Error('Direct Market generation is outside u64');
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, generation, true);
  return bytes;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

/** Derive the sole canonical per-maker replay coordinate under Trading. */
export function deriveDirectMakerReplayAddressV1(
  tradingProgramText: string,
  marketText: string,
  generation: bigint,
  makerText: string,
): Readonly<{ address: string; bump: number }> {
  const tradingProgram = canonicalKey(tradingProgramText, 'Trading program');
  const market = canonicalKey(marketText, 'Direct Market');
  const maker = canonicalKey(makerText, 'Direct maker');
  const identities = [tradingProgram.toBase58(), market.toBase58(), maker.toBase58()];
  if (new Set(identities).size !== identities.length) throw new Error('Trading program, Market, and maker identities must not alias');
  const [address, bump] = PublicKey.findProgramAddressSync([
    DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1,
    market.toBytes(),
    generationBytes(generation),
    maker.toBytes(),
  ], tradingProgram);
  if (identities.includes(address.toBase58())) throw new Error('derived maker replay address aliases one of its authority coordinates');
  return Object.freeze({ address: address.toBase58(), bump });
}

function authenticated(input: Omit<AuthenticatedDirectMakerNonceV1, typeof authenticatedDirectMakerNonceV1>): AuthenticatedDirectMakerNonceV1 {
  const output = { ...input } as AuthenticatedDirectMakerNonceV1;
  Object.defineProperty(output, authenticatedDirectMakerNonceV1, { value: true, enumerable: false });
  return Object.freeze(output);
}

function decodeExistingMakerReplayV1(
  account: RpcAccount,
  expected: Readonly<{
    address: string;
    bump: number;
    tradingProgram: PublicKey;
    market: PublicKey;
    generation: bigint;
    maker: PublicKey;
    observedSlot: string;
  }>,
): AuthenticatedDirectMakerNonceV1 {
  if (account.owner !== expected.tradingProgram.toBase58()) throw new Error('maker replay root is not owned by the selected Trading program');
  if (account.executable) throw new Error('maker replay root is executable');
  const width = account.data.length;
  if (account.space !== width
      || (width !== DIRECT_MAKER_REPLAY_BYTES_V1 && width !== DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1)) {
    throw new Error(
      `maker replay root must be exactly ${DIRECT_MAKER_REPLAY_BYTES_V1}`
      + ` or ${DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1} bytes`,
    );
  }
  u64Text(account.lamports, 'maker replay root lamports');
  if (!same(slice(account.data, 0, 8), DIRECT_MAKER_MAGIC_V1)
      || u16(account.data, 8) !== DIRECT_MAKER_VERSION_V1) {
    throw new Error('maker replay root has the wrong exact V1 magic or version');
  }
  if (account.data[DIRECT_MAKER_BUMP_OFFSET_V1] !== expected.bump) throw new Error('maker replay root stores a noncanonical PDA bump');
  requireZero(account.data, DIRECT_MAKER_RESERVED_OFFSET_V1, 5, 'maker replay root');
  if (!same(slice(account.data, DIRECT_MAKER_MARKET_OFFSET_V1, 32), expected.market.toBytes())
      || u64(account.data, DIRECT_MAKER_GENERATION_OFFSET_V1) !== expected.generation
      || !same(slice(account.data, DIRECT_MAKER_IDENTITY_OFFSET_V1, 32), expected.maker.toBytes())) {
    throw new Error('maker replay root substitutes another Market, generation, or maker');
  }
  const nextNonce = u64(account.data, DIRECT_MAKER_NEXT_NONCE_OFFSET_V1);
  const liveCount = u64(account.data, DIRECT_MAKER_LIVE_COUNT_OFFSET_V1);
  const minimumLiveNonce = u64(account.data, DIRECT_MAKER_MINIMUM_LIVE_NONCE_OFFSET_V1);
  const rentOwner = slice(account.data, DIRECT_MAKER_RENT_OWNER_OFFSET_V1, 32);
  const rentPrincipal = u64(account.data, DIRECT_MAKER_RENT_PRINCIPAL_OFFSET_V1);
  const feeOwed = width === DIRECT_MAKER_REPLAY_BYTES_V1
    ? u64(account.data, DIRECT_MAKER_FEE_OWED_OFFSET_V1)
    : 0n;
  if (liveCount > nextNonce) throw new Error('maker replay live count exceeds its next nonce');
  if (minimumLiveNonce > nextNonce) throw new Error('maker replay minimum-live nonce exceeds its next nonce');
  if (isZero(rentOwner) || rentPrincipal === 0n) throw new Error('maker replay root has an invalid RentCredit beneficiary or rent principal');
  if (nextNonce === U64_MAX) throw new Error('maker replay nonce is saturated; no new signed intent can be admitted');
  return authenticated({
    address: expected.address,
    tradingProgram: expected.tradingProgram.toBase58(),
    market: expected.market.toBase58(),
    generation: expected.generation,
    maker: expected.maker.toBase58(),
    observedSlot: expected.observedSlot,
    nextNonce,
    feeOwed,
    state: 'existing',
  });
}

function projectMakerReplayObservationV1(
  account: RpcAccount | null,
  expected: Readonly<{
    address: string;
    bump: number;
    tradingProgram: PublicKey;
    market: PublicKey;
    generation: bigint;
    maker: PublicKey;
    observedSlot: string;
  }>,
): AuthenticatedDirectMakerNonceV1 {
  if (account === null) {
    return authenticated({
      address: expected.address,
      tradingProgram: expected.tradingProgram.toBase58(),
      market: expected.market.toBase58(),
      generation: expected.generation,
      maker: expected.maker.toBase58(),
      observedSlot: expected.observedSlot,
      nextNonce: 0n,
      feeOwed: 0n,
      state: 'vacant',
    });
  }
  if (account.owner === SYSTEM_PROGRAM_ID) {
    u64Text(account.lamports, 'vacant maker replay lamports');
    if (account.executable || account.space !== 0 || account.data.length !== 0) {
      throw new Error('maker replay PDA is not an exact data-free System-owned vacant account');
    }
    return authenticated({
      address: expected.address,
      tradingProgram: expected.tradingProgram.toBase58(),
      market: expected.market.toBase58(),
      generation: expected.generation,
      maker: expected.maker.toBase58(),
      observedSlot: expected.observedSlot,
      nextNonce: 0n,
      feeOwed: 0n,
      state: 'vacant',
    });
  }
  return decodeExistingMakerReplayV1(account, expected);
}

/**
 * Read the next taker nonce from the sole canonical replay PDA at one
 * finalized floor. An absent or exact data-free System-owned PDA is first use
 * and therefore nonce zero; every material account is hostile-decoded.
 */
export async function inspectDirectMakerNonceV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'accountInfo'>,
  request: DirectMakerNonceRequestV1,
): Promise<AuthenticatedDirectMakerNonceV1> {
  const tradingProgram = canonicalKey(request.tradingProgram, 'Trading program');
  const market = canonicalKey(request.market, 'Direct Market');
  const maker = canonicalKey(request.maker, 'Direct maker');
  const derived = deriveDirectMakerReplayAddressV1(
    tradingProgram.toBase58(), market.toBase58(), request.generation, maker.toBase58(),
  );
  const floor = await client.finalizedSlot();
  const floorValue = u64Text(floor, 'maker replay finalized floor');
  const observation = await client.accountInfo(derived.address, floor);
  const observedSlot = u64Text(observation.slot, 'maker replay observation slot');
  if (observedSlot < floorValue) throw new Error('maker replay observation regressed below its finalized floor');
  return projectMakerReplayObservationV1(observation.account, {
    address: derived.address,
    bump: derived.bump,
    tradingProgram,
    market,
    generation: request.generation,
    maker,
    observedSlot: observation.slot,
  });
}

/**
 * Reacquire both makers' replay roots in one finalized account snapshot.
 *
 * A Direct Hot request consumes seller and buyer nonces atomically. Reading
 * them through two independent RPC observations would let a client assemble
 * a packet from states that never coexisted, so the wallet caller uses this
 * exact pair reader immediately before it asks for a transaction signature.
 */
export async function inspectDirectMakerNoncePairV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  requests: readonly [DirectMakerNonceRequestV1, DirectMakerNonceRequestV1],
): Promise<AuthenticatedDirectMakerNoncePairV1> {
  const first = requests[0];
  const second = requests[1];
  if (first.tradingProgram !== second.tradingProgram
      || first.market !== second.market
      || first.generation !== second.generation) {
    throw new Error('maker replay pair does not share one Trading program, Market, and generation');
  }
  if (first.maker === second.maker) throw new Error('maker replay pair must contain two distinct makers');
  const tradingProgram = canonicalKey(first.tradingProgram, 'Trading program');
  const market = canonicalKey(first.market, 'Direct Market');
  const prepared = requests.map((request, index) => {
    const maker = canonicalKey(request.maker, `Direct maker ${index}`);
    const derived = deriveDirectMakerReplayAddressV1(
      tradingProgram.toBase58(), market.toBase58(), request.generation, maker.toBase58(),
    );
    return Object.freeze({ request, maker, derived });
  }) as unknown as readonly [
    Readonly<{ request: DirectMakerNonceRequestV1; maker: PublicKey; derived: Readonly<{ address: string; bump: number }> }>,
    Readonly<{ request: DirectMakerNonceRequestV1; maker: PublicKey; derived: Readonly<{ address: string; bump: number }> }>,
  ];
  if (prepared[0].derived.address === prepared[1].derived.address) {
    throw new Error('maker replay pair aliases one derived replay address');
  }
  const floor = await client.finalizedSlot();
  const floorValue = u64Text(floor, 'maker replay finalized floor');
  const addresses = prepared.map((entry) => entry.derived.address);
  const observation = await client.multipleAccounts(addresses, floor);
  const observedSlot = u64Text(observation.slot, 'maker replay observation slot');
  if (observedSlot < floorValue) throw new Error('maker replay observation regressed below its finalized floor');
  if (observation.accounts.length !== 2
      || observation.accounts.some((entry, index) => entry.address !== addresses[index])) {
    throw new Error('maker replay RPC result substituted or reordered the requested pair');
  }
  const decoded = prepared.map((entry, index) => projectMakerReplayObservationV1(
    observation.accounts[index]?.account ?? null,
    {
      address: entry.derived.address,
      bump: entry.derived.bump,
      tradingProgram,
      market,
      generation: entry.request.generation,
      maker: entry.maker,
      observedSlot: observation.slot,
    },
  ));
  const pair = decoded as unknown as AuthenticatedDirectMakerNoncePairV1;
  Object.defineProperty(pair, authenticatedDirectMakerNoncePairV1, { value: true, enumerable: false });
  return Object.freeze(pair);
}

/** Refuse a caller-assembled tuple and return only one same-snapshot nonce pair. */
export function requireAuthenticatedDirectMakerNoncePairV1(
  pair: AuthenticatedDirectMakerNoncePairV1,
): readonly [AuthenticatedDirectMakerNonceV1, AuthenticatedDirectMakerNonceV1] {
  if (pair === null || !Array.isArray(pair)
      || pair.length !== 2 || pair[authenticatedDirectMakerNoncePairV1] !== true) {
    throw new Error('maker nonce pair was not acquired from the authenticated pair reader');
  }
  const first = pair[0];
  const second = pair[1];
  if (first.observedSlot !== second.observedSlot
      || first.tradingProgram !== second.tradingProgram
      || first.market !== second.market
      || first.generation !== second.generation
      || first.maker === second.maker
      || first.address === second.address) {
    throw new Error('authenticated maker nonce pair does not share one exact distinct-maker snapshot');
  }
  return pair;
}

/** Runtime-check an opaque chain observation and bind it to one crossing. */
export function requireAuthenticatedDirectMakerNonceV1(
  observation: AuthenticatedDirectMakerNonceV1,
  expected: Readonly<{ tradingProgram?: string; market: string; generation: bigint; maker: string }>,
): bigint {
  if (observation === null || typeof observation !== 'object'
      || observation[authenticatedDirectMakerNonceV1] !== true) {
    throw new Error('taker nonce was not acquired from the authenticated maker replay reader');
  }
  if ((expected.tradingProgram !== undefined && observation.tradingProgram !== expected.tradingProgram)
      || observation.market !== expected.market
      || observation.generation !== expected.generation
      || observation.maker !== expected.maker) {
    throw new Error('authenticated taker nonce belongs to another Trading program, Market, generation, or maker');
  }
  if (observation.nextNonce < 0n || observation.nextNonce >= U64_MAX) {
    throw new Error('authenticated taker nonce is outside the admissible unsaturated u64 range');
  }
  return observation.nextNonce;
}
