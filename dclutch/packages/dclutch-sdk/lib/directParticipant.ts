import { PublicKey, SystemProgram } from '@solana/web3.js';

import { fromHex, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { type DirectCrossingPlanV1 } from './directTicket';
import * as Abi from './generated/directParticipantV1';
import { inspectMarketDetailV1 } from './marketDetail';
import {
  decodeClaimsPositionV2,
  deriveCustodyAuthorityAddressV1,
} from './marketCoreV2';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/** Chain-derived coordinates belonging to one ordinary participant. */
export type DirectParticipantCoordinatesV1 = Readonly<{
  aggregate: string;
  position: string;
  admission: string;
  collateral: string;
  custodyAuthority: string;
}>;

export type DirectParticipantReadinessV1 =
  | Readonly<{
    status: 'ready';
    observedSlot: string;
    market: string;
    generation: bigint;
    owner: string;
    coordinates: DirectParticipantCoordinatesV1;
    collateralMint: string;
    tokenProgram: string;
    positionRevision: bigint;
    positionBalances: ReadonlyArray<bigint>;
    collateralAtoms: bigint;
    delegatedCollateralAtoms: bigint;
    spendableCollateralAtoms: bigint;
    reason: string;
  }>
  | Readonly<{
    status: 'incomplete';
    observedSlot: string;
    market: string;
    generation: bigint;
    owner: string;
    coordinates: DirectParticipantCoordinatesV1;
    missing: ReadonlyArray<'Position and admission' | 'collateral account'>;
    reason: string;
  }>
  | Readonly<{ status: 'refused'; reason: string }>;

export type DirectParticipantRequestV1 = Readonly<{
  market: string;
  owner: string;
  coreProgram: string;
  registryProgram: string;
  claimsProgram: string;
  tradingProgram: string;
  custodyProgram: string;
  rentProgram: string;
}>;

/**
 * Chain-derived coordinates belonging to the SELLER half of a Direct crossing.
 *
 * There is no admission here, and the collateral is a different address in a
 * different derivation family, because the seller's collateral destination is
 * not an admission-created participant account. It is the role-separated PDA
 * `direct_token_setup_v1` derives under Trading and creates permissionlessly:
 * `find_program_address(["dclutch:direct-token:v1", market, generation_le,
 * seller, [Seller]], trading)`
 * (`DirectTokenAccountSeedsV1::as_slices`, `authenticate_semantics`).
 */
export type DirectSellerCoordinatesV1 = Readonly<{
  aggregate: string;
  position: string;
  collateral: string;
  custodyAuthority: string;
}>;

/**
 * Which of the two prestates the chain admits the seller's Direct token
 * account is currently in.
 *
 * `vacant` is what `direct_token_setup_v1` requires to create it
 * (`seller_account.owner != &system_program::ID || seller_account.data_len()
 * != 0` refuses); `initialized` is what the trade route requires to execute
 * against it (`direct_inline_route_v3.rs` refuses unless
 * `seller_token.state == AccountState::Initialized`). Nothing else is admitted
 * by either site, so nothing else is admitted here.
 */
export type DirectSellerCollateralPrestateV1 = 'vacant' | 'initialized';

export type DirectSellerReadinessV1 =
  | Readonly<{
    status: 'ready';
    observedSlot: string;
    market: string;
    generation: bigint;
    owner: string;
    coordinates: DirectSellerCoordinatesV1;
    collateralMint: string;
    tokenProgram: string;
    positionRevision: bigint;
    positionBalances: ReadonlyArray<bigint>;
    collateralPrestate: DirectSellerCollateralPrestateV1;
    reason: string;
  }>
  | Readonly<{
    status: 'incomplete';
    observedSlot: string;
    market: string;
    generation: bigint;
    owner: string;
    coordinates: DirectSellerCoordinatesV1;
    missing: ReadonlyArray<'Claims Position'>;
    reason: string;
  }>
  | Readonly<{ status: 'refused'; reason: string }>;

const U64_MAX = 0xffff_ffff_ffff_ffffn;

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function readU32(bytes: Uint8Array, offset: number): number {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset + 4 > bytes.length) throw new Error('u32 field is outside its exact account');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function lamports(account: RpcAccount, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(account.lamports)) throw new Error(`${field} lamports are not canonical unsigned decimal text`);
  const value = BigInt(account.lamports);
  if (value > U64_MAX) throw new Error(`${field} lamports exceed u64`);
  return value;
}

function requiredAccount(account: RpcAccount | null, owner: string, field: string): RpcAccount {
  if (account === null) throw new Error(`${field} is absent`);
  if (account.owner !== owner || account.executable) throw new Error(`${field} is not nonexecutable state owned by ${owner}`);
  if (account.space !== account.data.length) throw new Error(`${field} RPC space differs from its exact data length`);
  lamports(account, field);
  return account;
}

function coptionAddress(bytes: Uint8Array, offset: number, field: string): string | null {
  const tag = readU32(bytes, offset);
  if (tag === 0) return null;
  if (tag !== 1) throw new Error(`${field} has a noncanonical option tag`);
  const tagBytes = Uint32Array.BYTES_PER_ELEMENT;
  const addressBytes = Abi.TOKEN_ACCOUNT_OWNER_OFFSET_V1 - Abi.TOKEN_ACCOUNT_MINT_OFFSET_V1;
  return key(new PublicKey(slice(bytes, offset + tagBytes, addressBytes)).toBase58(), field).toBase58();
}

async function deriveParticipantCollateralV1(
  market: PublicKey,
  owner: PublicKey,
  releaseSet: Uint8Array,
): Promise<string> {
  const preimage = new Uint8Array(
    Abi.DIRECT_PARTICIPANT_COLLATERAL_SEED_DOMAIN_V1.length + 32 + 32 + 32,
  );
  let offset = 0;
  for (const part of [
    Abi.DIRECT_PARTICIPANT_COLLATERAL_SEED_DOMAIN_V1,
    market.toBytes(),
    owner.toBytes(),
    releaseSet,
  ]) {
    preimage.set(part, offset);
    offset += part.length;
  }
  const seed = hex(await sha256(preimage)).slice(0, 32);
  return (await PublicKey.createWithSeed(owner, seed, key(Abi.TOKEN_2022_PROGRAM_ID_V1, 'Token-2022 program'))).toBase58();
}

/**
 * Derive the seller's Direct token account exactly as Trading does.
 *
 * The on-chain authority is `direct_token_setup_v1::authenticate_semantics`,
 * which builds `DirectTokenAccountSeedsV1::new(request.market,
 * request.generation, position.owner, DirectTokenAccountRoleV1::Seller)` and
 * takes `Pubkey::find_program_address(&seeds.as_slices(), program_id)` under
 * the TRADING program -- not under Token-2022, and not `create_with_seed` from
 * the owner. The seeds are the domain, the Market, the generation as eight
 * little-endian bytes, the owner, and the one-byte role.
 */
export function deriveDirectSellerTokenAddressV1(
  tradingProgram: string,
  market: string,
  generation: bigint,
  sellerOwner: string,
): string {
  if (generation < 0n || generation > U64_MAX) throw new Error('Market generation is outside u64');
  const generationBytes = new Uint8Array(8);
  new DataView(generationBytes.buffer).setBigUint64(0, generation, true);
  return PublicKey.findProgramAddressSync([
    Abi.DIRECT_TOKEN_ACCOUNT_PDA_DOMAIN_V1,
    key(market, 'Market').toBytes(),
    generationBytes,
    key(sellerOwner, 'seller owner').toBytes(),
    Uint8Array.of(Abi.DIRECT_TOKEN_ACCOUNT_SELLER_ROLE_V1),
  ], key(tradingProgram, 'Trading program'))[0].toBase58();
}

function decodeAdmissionV2(
  account: RpcAccount,
  expected: Readonly<{
    market: PublicKey;
    owner: PublicKey;
    releaseSet: Uint8Array;
    productRecord: Uint8Array;
    semanticBasis: Uint8Array;
    generation: bigint;
    outcomeCount: number;
    claimsProgram: PublicKey;
    tradingProgram: PublicKey;
    rentProgram: PublicKey;
  }>,
): Readonly<{ positionRent: bigint; admissionRent: bigint }> {
  const bytes = account.data;
  if (bytes.length !== Abi.PROTOCOL_POSITION_ADMISSION_BYTES_V2) {
    throw new Error(`Claims admission is ${bytes.length} bytes; the exact current width is ${Abi.PROTOCOL_POSITION_ADMISSION_BYTES_V2}`);
  }
  if (!same(bytes.subarray(
    Abi.PROTOCOL_POSITION_ADMISSION_MAGIC_V2.byteOffset,
    Abi.PROTOCOL_POSITION_ADMISSION_MAGIC_V2.length,
  ), Abi.PROTOCOL_POSITION_ADMISSION_MAGIC_V2)
      || u16(bytes, Abi.PROTOCOL_POSITION_ADMISSION_MAGIC_V2.length) !== Abi.PROTOCOL_POSITION_WIRE_VERSION_V2
      || bytes[Abi.POSITION_ADMISSION_STATUS_OFFSET_V2] !== 1
      || bytes[Abi.POSITION_ADMISSION_OWNER_KIND_OFFSET_V2] !== Abi.PROTOCOL_POSITION_USER_OWNER_KIND_V2) {
    throw new Error('Claims admission has the wrong exact persisted-user header');
  }
  requireZero(bytes, Abi.POSITION_ADMISSION_RESERVED_HEADER_OFFSET_V2, Abi.POSITION_ADMISSION_RESERVED_HEADER_BYTES_V2, 'Claims admission header');
  requireZero(bytes, Abi.POSITION_ADMISSION_RESERVED_TAIL_OFFSET_V2, Abi.POSITION_ADMISSION_RESERVED_TAIL_BYTES_V2, 'Claims admission tail');
  for (const [offset, value, field] of [
    [Abi.POSITION_ADMISSION_RELEASE_SET_OFFSET_V2, expected.releaseSet, 'release set'],
    [Abi.POSITION_ADMISSION_MARKET_OFFSET_V2, expected.market.toBytes(), 'Market'],
    [Abi.POSITION_ADMISSION_OWNER_OFFSET_V2, expected.owner.toBytes(), 'owner'],
    [Abi.POSITION_ADMISSION_PRODUCT_RECORD_OFFSET_V2, expected.productRecord, 'Product record'],
    [Abi.POSITION_ADMISSION_SEMANTIC_BASIS_OFFSET_V2, expected.semanticBasis, 'semantic basis'],
    [Abi.POSITION_ADMISSION_CLAIMS_PROGRAM_OFFSET_V2, expected.claimsProgram.toBytes(), 'Claims program'],
    [Abi.POSITION_ADMISSION_TRADING_PROGRAM_OFFSET_V2, expected.tradingProgram.toBytes(), 'Trading program'],
    [Abi.POSITION_ADMISSION_RENT_PROGRAM_OFFSET_V2, expected.rentProgram.toBytes(), 'Rent program'],
  ] as const) {
    if (!same(slice(bytes, offset, 32), value)) throw new Error(`Claims admission substitutes another ${field}`);
  }
  for (const [offset, field] of [
    [Abi.POSITION_ADMISSION_LINKED_BASIS_OFFSET_V2, 'linked basis'],
    [Abi.POSITION_ADMISSION_PARENT_REQUEST_OFFSET_V2, 'parent request'],
    [Abi.POSITION_ADMISSION_REQUEST_DIGEST_OFFSET_V2, 'request digest'],
    [Abi.POSITION_ADMISSION_RENT_CREDIT_OFFSET_V2, 'RentCredit'],
  ] as const) requireNonzero(slice(bytes, offset, 32), `Claims admission ${field}`);
  if (!isZero(slice(bytes, Abi.POSITION_ADMISSION_CAPABILITY_DESCRIPTOR_OFFSET_V2, 32))
      || readU32(bytes, Abi.POSITION_ADMISSION_CAPABILITY_OUTCOME_OFFSET_V2) !== 0) {
    throw new Error('ordinary user admission carries a Claims-capability coordinate');
  }
  if (u64(bytes, Abi.POSITION_ADMISSION_GENERATION_OFFSET_V2) !== expected.generation
      || readU32(bytes, Abi.POSITION_ADMISSION_OUTCOME_COUNT_OFFSET_V2) !== expected.outcomeCount) {
    throw new Error('Claims admission substitutes another Market generation or outcome width');
  }
  const before = u64(bytes, Abi.POSITION_ADMISSION_MARKET_REVISION_BEFORE_OFFSET_V2);
  const after = u64(bytes, Abi.POSITION_ADMISSION_MARKET_REVISION_AFTER_OFFSET_V2);
  if (before !== after || u64(bytes, Abi.POSITION_ADMISSION_POSITION_REVISION_OFFSET_V2) !== 0n) {
    throw new Error('Claims admission does not preserve its Market revision and zero initial Position revision');
  }
  const positionRent = u64(bytes, Abi.POSITION_ADMISSION_POSITION_RENT_OFFSET_V2);
  const admissionRent = u64(bytes, Abi.POSITION_ADMISSION_RECORD_RENT_OFFSET_V2);
  const recordedPositionLamports = u64(bytes, Abi.POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2);
  const recordedAdmissionLamports = u64(bytes, Abi.POSITION_ADMISSION_RECORD_LAMPORTS_OFFSET_V2);
  if (positionRent === 0n || admissionRent === 0n
      || recordedPositionLamports < positionRent || recordedAdmissionLamports < admissionRent
      || lamports(account, 'Claims admission') < admissionRent) {
    throw new Error('Claims admission rent principals or observed lamports are invalid');
  }
  return Object.freeze({ positionRent, admissionRent });
}

function decodeParticipantTokenV1(
  account: RpcAccount,
  expected: Readonly<{ mint: PublicKey; owner: PublicKey; custodyAuthority: PublicKey }>,
): Readonly<{ amount: bigint; delegated: bigint }> {
  if (account.data.length !== Abi.TOKEN_ACCOUNT_BYTES_V1) {
    throw new Error(`participant collateral is ${account.data.length} bytes; the exact base Token-2022 width is ${Abi.TOKEN_ACCOUNT_BYTES_V1}`);
  }
  const bytes = account.data;
  if (!same(slice(bytes, Abi.TOKEN_ACCOUNT_MINT_OFFSET_V1, 32), expected.mint.toBytes())
      || !same(slice(bytes, Abi.TOKEN_ACCOUNT_OWNER_OFFSET_V1, 32), expected.owner.toBytes())) {
    throw new Error('participant collateral substitutes another Realm Mint or wallet owner');
  }
  if (bytes[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] !== Abi.TOKEN_ACCOUNT_INITIALIZED_STATE_V1) {
    throw new Error(bytes[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] === 2
      ? 'participant collateral is frozen'
      : 'participant collateral is not initialized');
  }
  const delegate = coptionAddress(bytes, Abi.TOKEN_ACCOUNT_DELEGATE_OFFSET_V1, 'participant collateral delegate');
  if (delegate !== expected.custodyAuthority.toBase58()) {
    throw new Error('participant collateral is not delegated to this Market’s Custody authority');
  }
  if (coptionAddress(bytes, Abi.TOKEN_ACCOUNT_CLOSE_AUTHORITY_OFFSET_V1, 'participant collateral close authority') !== null
      || readU32(bytes, Abi.TOKEN_ACCOUNT_NATIVE_OFFSET_V1) !== 0) {
    throw new Error('participant collateral is native-wrapped or has a separate close authority');
  }
  const amount = u64(bytes, Abi.TOKEN_ACCOUNT_AMOUNT_OFFSET_V1);
  const delegated = u64(bytes, Abi.TOKEN_ACCOUNT_DELEGATED_AMOUNT_OFFSET_V1);
  if (delegated > amount) throw new Error('participant collateral delegation exceeds its current balance');
  return Object.freeze({ amount, delegated });
}

/**
 * Reacquire the complete participant prestate from chain state alone.
 *
 * A local admission report may explain history, but it is not authority here:
 * the current Market, Realm, aggregate, Position, admission and Token-2022
 * account all have to join at one finalized floor before coordinates escape.
 */
export async function inspectDirectParticipantReadinessV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: DirectParticipantRequestV1,
): Promise<DirectParticipantReadinessV1> {
  try {
    const market = key(request.market, 'Market');
    const owner = key(request.owner, 'participant owner');
    const coreProgram = key(request.coreProgram, 'Core program');
    const registryProgram = key(request.registryProgram, 'Registry program');
    const claimsProgram = key(request.claimsProgram, 'Claims program');
    const tradingProgram = key(request.tradingProgram, 'Trading program');
    const custodyProgram = key(request.custodyProgram, 'Custody program');
    const rentProgram = key(request.rentProgram, 'Rent program');
    if (new Set([market, owner, coreProgram, registryProgram, claimsProgram, tradingProgram, custodyProgram, rentProgram]
      .map((value) => value.toBase58())).size !== 8) throw new Error('participant and protocol authority coordinates alias');

    const detail = await inspectMarketDetailV1(client, {
      address: market.toBase58(),
      coreProgramId: coreProgram.toBase58(),
      registryProgramId: registryProgram.toBase58(),
      claimsProgramId: claimsProgram.toBase58(),
      custodyProgramId: custodyProgram.toBase58(),
    });
    if (detail.card.status !== 'decoded') throw new Error(`Market refused: ${detail.card.refusal}`);
    const card = detail.card;
    const brokenBinding = card.bindings.find((binding) => !binding.ok);
    if (brokenBinding !== undefined) throw new Error(`Market ${brokenBinding.label} refused: ${brokenBinding.detail}`);
    if (card.collateral.status !== 'bound') throw new Error(`Market Realm collateral refused: ${card.collateral.reason}`);
    if (card.liability.status !== 'bound') throw new Error(`Market Claims aggregate refused: ${card.liability.reason}`);
    if (card.collateral.tokenProgram !== Abi.TOKEN_2022_PROGRAM_ID_V1) {
      throw new Error('Direct participant admission requires the Realm’s exact Token-2022 profile');
    }
    const releaseSet = fromHex(card.identity.selectedReleaseSetId, 'Market release set');
    const aggregate = key(card.liability.aggregateAddress, 'Claims aggregate');
    const position = PublicKey.findProgramAddressSync([
      Abi.PROTOCOL_POSITION_STATE_SEED_V2, aggregate.toBytes(), owner.toBytes(),
    ], claimsProgram)[0];
    const admission = PublicKey.findProgramAddressSync([
      Abi.PROTOCOL_POSITION_ADMISSION_SEED_V2, aggregate.toBytes(), owner.toBytes(),
    ], claimsProgram)[0];
    const custodyAuthority = key(
      deriveCustodyAuthorityAddressV1(custodyProgram.toBase58(), market.toBase58(), card.identity.selectedReleaseSetId),
      'Custody authority',
    );
    const collateral = key(await deriveParticipantCollateralV1(market, owner, releaseSet), 'participant collateral');
    const coordinates = Object.freeze({
      aggregate: aggregate.toBase58(),
      position: position.toBase58(),
      admission: admission.toBase58(),
      collateral: collateral.toBase58(),
      custodyAuthority: custodyAuthority.toBase58(),
    });
    if (new Set([market, owner, coreProgram, registryProgram, claimsProgram, tradingProgram, custodyProgram, rentProgram,
      aggregate, position, admission, collateral, custodyAuthority].map((value) => value.toBase58())).size !== 13) {
      throw new Error('derived participant, Market, and program coordinates alias');
    }

    const observation = await client.multipleAccounts([
      position.toBase58(), admission.toBase58(), collateral.toBase58(),
    ], detail.floorSlot);
    if (BigInt(observation.slot) < BigInt(detail.floorSlot)) throw new Error('participant observation regressed below its finalized Market floor');
    const byAddress = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
    const positionAccount = byAddress.get(position.toBase58()) ?? null;
    const admissionAccount = byAddress.get(admission.toBase58()) ?? null;
    const collateralAccount = byAddress.get(collateral.toBase58()) ?? null;
    if ((positionAccount === null) !== (admissionAccount === null)) {
      throw new Error('only one of the atomic Claims Position and admission record exists');
    }
    const missing: Array<'Position and admission' | 'collateral account'> = [];
    if (positionAccount === null) missing.push('Position and admission');
    if (collateralAccount === null) missing.push('collateral account');
    if (missing.length > 0) {
      return Object.freeze({
        status: 'incomplete', observedSlot: observation.slot, market: market.toBase58(),
        generation: BigInt(card.generation), owner: owner.toBase58(), coordinates,
        missing: Object.freeze(missing),
        reason: `Your wallet is missing ${missing.join(' and ')} for this Market. The devnet admission command can create them; this browser does not invent or sign that caller’s transaction.`,
      });
    }

    const currentPosition = requiredAccount(positionAccount, claimsProgram.toBase58(), 'Claims Position');
    const currentAdmission = requiredAccount(admissionAccount, claimsProgram.toBase58(), 'Claims admission');
    const currentCollateral = requiredAccount(collateralAccount, Abi.TOKEN_2022_PROGRAM_ID_V1, 'participant collateral');
    const positionView = decodeClaimsPositionV2(position.toBase58(), currentPosition.data);
    if (positionView.aggregate !== aggregate.toBase58() || positionView.owner !== owner.toBase58()
        || positionView.liabilityBasisId !== card.liability.liabilityBasisId
        || positionView.claimCount !== card.liability.claimCount) {
      throw new Error('Claims Position substitutes another aggregate, owner, basis, or outcome width');
    }
    const recordedRent = decodeAdmissionV2(currentAdmission, {
      market, owner, releaseSet,
      productRecord: fromHex(card.identity.productRecordId, 'Market Product record'),
      semanticBasis: fromHex(card.liability.liabilityBasisId, 'Claims semantic basis'),
      generation: BigInt(card.generation), outcomeCount: card.liability.claimCount,
      claimsProgram, tradingProgram, rentProgram,
    });
    if (lamports(currentPosition, 'Claims Position') < recordedRent.positionRent) {
      throw new Error('Claims Position is below its admission-recorded rent principal');
    }
    const token = decodeParticipantTokenV1(currentCollateral, {
      mint: key(card.collateral.collateralMint, 'Realm collateral Mint'), owner, custodyAuthority,
    });
    const spendable = token.amount < token.delegated ? token.amount : token.delegated;
    return Object.freeze({
      status: 'ready', observedSlot: observation.slot, market: market.toBase58(),
      generation: BigInt(card.generation), owner: owner.toBase58(), coordinates,
      collateralMint: card.collateral.collateralMint, tokenProgram: card.collateral.tokenProgram,
      positionRevision: BigInt(positionView.revision),
      positionBalances: Object.freeze(positionView.balances.map((value) => BigInt(value))),
      collateralAtoms: token.amount,
      delegatedCollateralAtoms: token.delegated,
      spendableCollateralAtoms: spendable,
      reason: `Your Position, admission record, and delegated collateral account join this Market at finalized slot ${observation.slot}.`,
    });
  } catch (error) {
    return Object.freeze({
      status: 'refused',
      reason: error instanceof Error ? error.message : 'participant readiness refused without a usable reason',
    });
  }
}

/**
 * Classify the seller's Direct token account against the only two prestates
 * the chain admits, and refuse every third thing.
 *
 * `initialized` reproduces `direct_inline_route_v3.rs` exactly: the account is
 * owned by the token program, parses as the base 165-byte Token-2022 account,
 * carries this Realm's Mint and this seller as its owner, is Initialized, and
 * is not native-wrapped. It deliberately does NOT require a Custody delegate --
 * the trade route requires that of `buyer_token` and of nothing else, and
 * `initialize_account3` gives the seller's account no delegate at all, so the
 * participant model's delegation demand refuses the very account Trading
 * creates. It also does not require an empty close authority or a zero balance:
 * the trade route checks neither, and a seller's account holds its proceeds.
 */
function sellerCollateralPrestateV1(
  account: RpcAccount | null,
  expected: Readonly<{ mint: PublicKey; owner: PublicKey }>,
): DirectSellerCollateralPrestateV1 {
  if (account === null) return 'vacant';
  if (account.space !== account.data.length) throw new Error('seller Direct token account RPC space differs from its exact data length');
  lamports(account, 'seller Direct token account');
  if (account.executable) throw new Error('seller Direct token account is executable');
  if (account.owner === SystemProgram.programId.toBase58()) {
    if (account.data.length !== 0) {
      throw new Error('seller Direct token account is System-owned but carries data, so neither Trading setup nor the trade route admits it');
    }
    return 'vacant';
  }
  if (account.owner !== Abi.TOKEN_2022_PROGRAM_ID_V1) {
    throw new Error(`seller Direct token account is owned by ${account.owner}, neither the System program nor Token-2022`);
  }
  const bytes = account.data;
  if (bytes.length !== Abi.TOKEN_ACCOUNT_BYTES_V1) {
    throw new Error(`seller Direct token account is ${bytes.length} bytes; the exact base Token-2022 width is ${Abi.TOKEN_ACCOUNT_BYTES_V1}`);
  }
  if (!same(slice(bytes, Abi.TOKEN_ACCOUNT_MINT_OFFSET_V1, 32), expected.mint.toBytes())
      || !same(slice(bytes, Abi.TOKEN_ACCOUNT_OWNER_OFFSET_V1, 32), expected.owner.toBytes())) {
    throw new Error('seller Direct token account substitutes another Realm Mint or seller owner');
  }
  if (bytes[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] !== Abi.TOKEN_ACCOUNT_INITIALIZED_STATE_V1) {
    throw new Error(bytes[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] === 2
      ? 'seller Direct token account is frozen'
      : 'seller Direct token account is not initialized');
  }
  if (readU32(bytes, Abi.TOKEN_ACCOUNT_NATIVE_OFFSET_V1) !== 0) {
    throw new Error('seller Direct token account is native-wrapped');
  }
  return 'initialized';
}

/**
 * Reacquire the SELLER's complete Direct prestate from chain state alone.
 *
 * This is deliberately not `inspectDirectParticipantReadinessV1` with a
 * different owner. The two halves of a Direct crossing have different on-chain
 * preconditions and the chain's model wins: a BUYER spends collateral through
 * Custody, so the buyer needs an admission record and an admission-created,
 * Custody-delegated Token-2022 account. A SELLER spends CLAIMS, and its
 * collateral is only a destination -- one that `direct_token_setup_v1` creates
 * permissionlessly from the seller's Claims Position alone, with no admission
 * account at any of that route's twenty-three indices.
 *
 * So what is required here is exactly what the chain requires: the canonical
 * Claims aggregate and Position for this Market and owner
 * (`authenticate_seller_position`), and a Direct token account at the address
 * Trading derives, in one of the two prestates the chain admits.
 */
export async function inspectDirectSellerReadinessV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: DirectParticipantRequestV1,
): Promise<DirectSellerReadinessV1> {
  try {
    const market = key(request.market, 'Market');
    const owner = key(request.owner, 'seller owner');
    const coreProgram = key(request.coreProgram, 'Core program');
    const registryProgram = key(request.registryProgram, 'Registry program');
    const claimsProgram = key(request.claimsProgram, 'Claims program');
    const tradingProgram = key(request.tradingProgram, 'Trading program');
    const custodyProgram = key(request.custodyProgram, 'Custody program');
    // The Rent program takes no part in a seller's preconditions -- only the
    // admission record this route does not read names it -- but it stays in the
    // request so one caller-built coordinate set serves both halves, and it
    // still has to be distinct from everything else here.
    const rentProgram = key(request.rentProgram, 'Rent program');
    if (new Set([market, owner, coreProgram, registryProgram, claimsProgram, tradingProgram, custodyProgram, rentProgram]
      .map((value) => value.toBase58())).size !== 8) throw new Error('seller and protocol authority coordinates alias');

    const detail = await inspectMarketDetailV1(client, {
      address: market.toBase58(),
      coreProgramId: coreProgram.toBase58(),
      registryProgramId: registryProgram.toBase58(),
      claimsProgramId: claimsProgram.toBase58(),
      custodyProgramId: custodyProgram.toBase58(),
    });
    if (detail.card.status !== 'decoded') throw new Error(`Market refused: ${detail.card.refusal}`);
    const card = detail.card;
    const brokenBinding = card.bindings.find((binding) => !binding.ok);
    if (brokenBinding !== undefined) throw new Error(`Market ${brokenBinding.label} refused: ${brokenBinding.detail}`);
    if (card.collateral.status !== 'bound') throw new Error(`Market Realm collateral refused: ${card.collateral.reason}`);
    if (card.liability.status !== 'bound') throw new Error(`Market Claims aggregate refused: ${card.liability.reason}`);
    if (card.collateral.tokenProgram !== Abi.TOKEN_2022_PROGRAM_ID_V1) {
      throw new Error('Direct seller readiness requires the Realm’s exact Token-2022 profile');
    }
    const generation = BigInt(card.generation);
    const aggregate = key(card.liability.aggregateAddress, 'Claims aggregate');
    const position = PublicKey.findProgramAddressSync([
      Abi.PROTOCOL_POSITION_STATE_SEED_V2, aggregate.toBytes(), owner.toBytes(),
    ], claimsProgram)[0];
    const custodyAuthority = key(
      deriveCustodyAuthorityAddressV1(custodyProgram.toBase58(), market.toBase58(), card.identity.selectedReleaseSetId),
      'Custody authority',
    );
    const collateral = key(
      deriveDirectSellerTokenAddressV1(tradingProgram.toBase58(), market.toBase58(), generation, owner.toBase58()),
      'seller Direct token account',
    );
    const coordinates = Object.freeze({
      aggregate: aggregate.toBase58(),
      position: position.toBase58(),
      collateral: collateral.toBase58(),
      custodyAuthority: custodyAuthority.toBase58(),
    });
    if (new Set([market, owner, coreProgram, registryProgram, claimsProgram, tradingProgram, custodyProgram, rentProgram,
      aggregate, position, collateral, custodyAuthority].map((value) => value.toBase58())).size !== 12) {
      throw new Error('derived seller, Market, and program coordinates alias');
    }

    const observation = await client.multipleAccounts([position.toBase58(), collateral.toBase58()], detail.floorSlot);
    if (BigInt(observation.slot) < BigInt(detail.floorSlot)) throw new Error('seller observation regressed below its finalized Market floor');
    const byAddress = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
    const positionAccount = byAddress.get(position.toBase58()) ?? null;
    const collateralAccount = byAddress.get(collateral.toBase58()) ?? null;
    if (positionAccount === null) {
      return Object.freeze({
        status: 'incomplete', observedSlot: observation.slot, market: market.toBase58(),
        generation, owner: owner.toBase58(), coordinates,
        missing: Object.freeze(['Claims Position'] as const),
        reason: `This seller holds no Claims Position for this Market, so Trading cannot derive or create its Direct token account and there are no claims to sell.`,
      });
    }

    const currentPosition = requiredAccount(positionAccount, claimsProgram.toBase58(), 'Claims Position');
    const positionView = decodeClaimsPositionV2(position.toBase58(), currentPosition.data);
    if (positionView.aggregate !== aggregate.toBase58() || positionView.owner !== owner.toBase58()
        || positionView.liabilityBasisId !== card.liability.liabilityBasisId
        || positionView.claimCount !== card.liability.claimCount) {
      throw new Error('Claims Position substitutes another aggregate, owner, basis, or outcome width');
    }
    const collateralPrestate = sellerCollateralPrestateV1(collateralAccount, {
      mint: key(card.collateral.collateralMint, 'Realm collateral Mint'), owner,
    });
    return Object.freeze({
      status: 'ready', observedSlot: observation.slot, market: market.toBase58(),
      generation, owner: owner.toBase58(), coordinates,
      collateralMint: card.collateral.collateralMint, tokenProgram: card.collateral.tokenProgram,
      positionRevision: BigInt(positionView.revision),
      positionBalances: Object.freeze(positionView.balances.map((value) => BigInt(value))),
      collateralPrestate,
      reason: collateralPrestate === 'initialized'
        ? `This seller’s Claims Position and its live Direct token account join this Market at finalized slot ${observation.slot}.`
        : `This seller’s Claims Position joins this Market at finalized slot ${observation.slot}; its Direct token account is still vacant and the route’s permissionless Trading token setup creates it.`,
    });
  } catch (error) {
    return Object.freeze({
      status: 'refused',
      reason: error instanceof Error ? error.message : 'seller readiness refused without a usable reason',
    });
  }
}

export type DirectParticipantCrossingAdmissionV1 = Readonly<{
  resource: 'collateral allowance' | 'claim balance';
  requiredAtoms: bigint;
  availableAtoms: bigint;
  note: string;
}>;

/** Bind one economic crossing preview to the connected participant's current assets. */
export function admitDirectParticipantCrossingV1(
  participant: DirectParticipantReadinessV1,
  plan: DirectCrossingPlanV1,
): DirectParticipantCrossingAdmissionV1 {
  if (participant.status !== 'ready') throw new Error(`participant state is ${participant.status}, not ready`);
  if (plan.takerAddress !== participant.owner
      || plan.taker.market !== participant.market || plan.taker.generation !== participant.generation
      || plan.taker.collateralAccount !== participant.coordinates.collateral) {
    throw new Error('crossing plan belongs to another participant, Market, generation, or collateral account');
  }
  if (plan.takerSide === 'buy') {
    const required = plan.preview.buyerCollateralDebit;
    if (participant.spendableCollateralAtoms < required) {
      throw new Error(`this buy needs ${required} delegated collateral atoms; your current finalized allowance is ${participant.spendableCollateralAtoms}`);
    }
    return Object.freeze({
      resource: 'collateral allowance', requiredAtoms: required,
      availableAtoms: participant.spendableCollateralAtoms,
      note: `Your current finalized collateral allowance covers ${required} atoms for this buy.`,
    });
  }
  const available = participant.positionBalances[plan.taker.outcome];
  if (available === undefined) throw new Error('crossing outcome is outside the participant Position width');
  if (available < plan.fill) {
    throw new Error(`this sale needs ${plan.fill} claim atoms; your current finalized Position holds ${available} at outcome ${plan.taker.outcome}`);
  }
  return Object.freeze({
    resource: 'claim balance', requiredAtoms: plan.fill, availableAtoms: available,
    note: `Your current finalized Position covers ${plan.fill} claim atoms for this sale.`,
  });
}
