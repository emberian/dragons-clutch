import { PublicKey } from '@solana/web3.js';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { LIVE, liveRpcAccount, mutate } from '../fixtures/liveOpenMarket';
import { hex, sha256 } from './bytes';
import {
  admitDirectParticipantCrossingV1,
  deriveDirectSellerTokenAddressV1,
  inspectDirectParticipantReadinessV1,
  inspectDirectSellerReadinessV1,
  type DirectParticipantReadinessV1,
} from './directParticipant';
import { type DirectCrossingPlanV1 } from './directTicket';
import * as Abi from './generated/directParticipantV1';
import {
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  LIABILITY_BASIS_MARKET_BASIS_OFFSET,
} from './generated/coreFound';
import { inspectMarketDetailV1 } from './marketDetail';
import { deriveCustodyAuthorityAddressV1 } from './marketCoreV2';
import { decodeRealmRecordV1 } from './realmRecord';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

vi.mock('./marketDetail', () => ({ inspectMarketDetailV1: vi.fn() }));

const SLOT = '101';
const MARKET = LIVE.market.address;
const OWNER = LIVE.founder;
const CORE = LIVE.programs.core;
const REGISTRY = LIVE.programs.registry;
const CLAIMS = LIVE.programs.claims;
const TRADING = LIVE.programs.trading;
const CUSTODY = LIVE.programs.custody;
const RENT = LIVE.programs.rent;
const AGGREGATE = LIVE.claimsAggregate.address;
const RELEASE_SET = LIVE.market.data.slice(CORE_STATE_SELECTED_RELEASE_SET_OFFSET, CORE_STATE_SELECTED_RELEASE_SET_OFFSET + 32);
const PRODUCT = LIVE.market.data.slice(CORE_STATE_PRODUCT_RECORD_OFFSET, CORE_STATE_PRODUCT_RECORD_OFFSET + 32);
const BASIS = LIVE.claimsAggregate.data.slice(LIABILITY_BASIS_MARKET_BASIS_OFFSET, LIABILITY_BASIS_MARKET_BASIS_OFFSET + 32);
const GENERATION = new DataView(LIVE.market.data.buffer, LIVE.market.data.byteOffset).getBigUint64(CORE_STATE_GENERATION_OFFSET, true);
const REALM = decodeRealmRecordV1(LIVE.realmRecord.data);

function rpcAccount(owner: string, data: Uint8Array, lamports = '2000000'): RpcAccount {
  return Object.freeze({ owner, data, executable: false, lamports, space: data.length });
}

function admission(): Uint8Array {
  const bytes = new Uint8Array(Abi.PROTOCOL_POSITION_ADMISSION_BYTES_V2);
  bytes.set(Abi.PROTOCOL_POSITION_ADMISSION_MAGIC_V2, 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(8, Abi.PROTOCOL_POSITION_WIRE_VERSION_V2, true);
  bytes[Abi.POSITION_ADMISSION_STATUS_OFFSET_V2] = 1;
  bytes[Abi.POSITION_ADMISSION_OWNER_KIND_OFFSET_V2] = Abi.PROTOCOL_POSITION_USER_OWNER_KIND_V2;
  bytes.set(RELEASE_SET, Abi.POSITION_ADMISSION_RELEASE_SET_OFFSET_V2);
  bytes.set(new PublicKey(MARKET).toBytes(), Abi.POSITION_ADMISSION_MARKET_OFFSET_V2);
  bytes.set(new PublicKey(OWNER).toBytes(), Abi.POSITION_ADMISSION_OWNER_OFFSET_V2);
  bytes.set(PRODUCT, Abi.POSITION_ADMISSION_PRODUCT_RECORD_OFFSET_V2);
  bytes.set(BASIS, Abi.POSITION_ADMISSION_SEMANTIC_BASIS_OFFSET_V2);
  for (const offset of [
    Abi.POSITION_ADMISSION_LINKED_BASIS_OFFSET_V2,
    Abi.POSITION_ADMISSION_PARENT_REQUEST_OFFSET_V2,
    Abi.POSITION_ADMISSION_REQUEST_DIGEST_OFFSET_V2,
    Abi.POSITION_ADMISSION_RENT_CREDIT_OFFSET_V2,
  ]) bytes.fill(offset / 16 + 1, offset, offset + 32);
  bytes.set(new PublicKey(RENT).toBytes(), Abi.POSITION_ADMISSION_RENT_PROGRAM_OFFSET_V2);
  bytes.set(new PublicKey(CLAIMS).toBytes(), Abi.POSITION_ADMISSION_CLAIMS_PROGRAM_OFFSET_V2);
  bytes.set(new PublicKey(TRADING).toBytes(), Abi.POSITION_ADMISSION_TRADING_PROGRAM_OFFSET_V2);
  view.setBigUint64(Abi.POSITION_ADMISSION_GENERATION_OFFSET_V2, GENERATION, true);
  view.setUint32(Abi.POSITION_ADMISSION_OUTCOME_COUNT_OFFSET_V2, 4, true);
  view.setBigUint64(Abi.POSITION_ADMISSION_POSITION_RENT_OFFSET_V2, 1n, true);
  view.setBigUint64(Abi.POSITION_ADMISSION_RECORD_RENT_OFFSET_V2, 1n, true);
  view.setBigUint64(Abi.POSITION_ADMISSION_MARKET_REVISION_BEFORE_OFFSET_V2, 7n, true);
  view.setBigUint64(Abi.POSITION_ADMISSION_MARKET_REVISION_AFTER_OFFSET_V2, 7n, true);
  view.setBigUint64(Abi.POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2, 1n, true);
  view.setBigUint64(Abi.POSITION_ADMISSION_RECORD_LAMPORTS_OFFSET_V2, 1n, true);
  return bytes;
}

function token(owner: string, custody: string, amount = 20_000n, delegated = 12_000n): Uint8Array {
  const bytes = new Uint8Array(Abi.TOKEN_ACCOUNT_BYTES_V1);
  const view = new DataView(bytes.buffer);
  bytes.set(new PublicKey(REALM.collateralMint).toBytes(), Abi.TOKEN_ACCOUNT_MINT_OFFSET_V1);
  bytes.set(new PublicKey(owner).toBytes(), Abi.TOKEN_ACCOUNT_OWNER_OFFSET_V1);
  view.setBigUint64(Abi.TOKEN_ACCOUNT_AMOUNT_OFFSET_V1, amount, true);
  view.setUint32(Abi.TOKEN_ACCOUNT_DELEGATE_OFFSET_V1, 1, true);
  bytes.set(new PublicKey(custody).toBytes(), Abi.TOKEN_ACCOUNT_DELEGATE_OFFSET_V1 + 4);
  bytes[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] = Abi.TOKEN_ACCOUNT_INITIALIZED_STATE_V1;
  view.setBigUint64(Abi.TOKEN_ACCOUNT_DELEGATED_AMOUNT_OFFSET_V1, delegated, true);
  return bytes;
}

async function coordinates() {
  const market = new PublicKey(MARKET);
  const owner = new PublicKey(OWNER);
  const aggregate = new PublicKey(AGGREGATE);
  const position = PublicKey.findProgramAddressSync([Abi.PROTOCOL_POSITION_STATE_SEED_V2, aggregate.toBytes(), owner.toBytes()], new PublicKey(CLAIMS))[0];
  const admissionAddress = PublicKey.findProgramAddressSync([Abi.PROTOCOL_POSITION_ADMISSION_SEED_V2, aggregate.toBytes(), owner.toBytes()], new PublicKey(CLAIMS))[0];
  const digest = await sha256(new Uint8Array([...Abi.DIRECT_PARTICIPANT_COLLATERAL_SEED_DOMAIN_V1, ...market.toBytes(), ...owner.toBytes(), ...RELEASE_SET]));
  const collateral = await PublicKey.createWithSeed(owner, hex(digest).slice(0, 32), new PublicKey(Abi.TOKEN_2022_PROGRAM_ID_V1));
  const custody = deriveCustodyAuthorityAddressV1(CUSTODY, MARKET, hex(RELEASE_SET));
  return { position: position.toBase58(), admission: admissionAddress.toBase58(), collateral: collateral.toBase58(), custody };
}

function request() {
  return { market: MARKET, owner: OWNER, coreProgram: CORE, registryProgram: REGISTRY, claimsProgram: CLAIMS, tradingProgram: TRADING, custodyProgram: CUSTODY, rentProgram: RENT } as const;
}

async function client(overrides: ReadonlyMap<string, RpcAccount | null> = new Map()): Promise<SolanaRpcClient> {
  const c = await coordinates();
  const accounts = new Map<string, RpcAccount | null>([
    [c.position, liveRpcAccount(LIVE.founderPosition)],
    [c.admission, rpcAccount(CLAIMS, admission())],
    [c.collateral, rpcAccount(Abi.TOKEN_2022_PROGRAM_ID_V1, token(OWNER, c.custody))],
    ...overrides,
  ]);
  return {
    finalizedSlot: async () => SLOT,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: SLOT,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
  } as unknown as SolanaRpcClient;
}

beforeEach(() => {
  vi.mocked(inspectMarketDetailV1).mockResolvedValue({
    floorSlot: '99',
    card: {
      status: 'decoded', generation: GENERATION.toString(), bindings: [],
      identity: { selectedReleaseSetId: hex(RELEASE_SET), productRecordId: hex(PRODUCT) },
      collateral: { status: 'bound', collateralMint: REALM.collateralMint, tokenProgram: REALM.tokenProgram },
      liability: { status: 'bound', aggregateAddress: AGGREGATE, liabilityBasisId: hex(BASIS), claimCount: 4 },
    },
  } as unknown as Awaited<ReturnType<typeof inspectMarketDetailV1>>);
});

describe('Direct participant readiness', () => {
  it('joins the Position, atomic admission, and deterministic delegated collateral at one finalized floor', async () => {
    const result = await inspectDirectParticipantReadinessV1(await client(), request());
    expect(result.status).toBe('ready');
    if (result.status !== 'ready') throw new Error(result.reason);
    expect(result.coordinates.position).toBe(LIVE.founderPosition.address);
    expect(result.collateralAtoms).toBe(20_000n);
    expect(result.delegatedCollateralAtoms).toBe(12_000n);
    expect(result.positionBalances).toHaveLength(4);
    expect(result.reason).toContain(`finalized slot ${SLOT}`);
  });

  it('names missing resources but refuses a torn atomic Position/admission pair', async () => {
    const c = await coordinates();
    const absent = await inspectDirectParticipantReadinessV1(await client(new Map([
      [c.position, null], [c.admission, null], [c.collateral, null],
    ])), request());
    expect(absent).toMatchObject({ status: 'incomplete', missing: ['Position and admission', 'collateral account'] });
    const torn = await inspectDirectParticipantReadinessV1(await client(new Map([[c.admission, null]])), request());
    expect(torn).toMatchObject({ status: 'refused' });
    expect(torn.reason).toContain('only one of the atomic');
  });

  it('refuses dirty admission padding and substituted or unsafe Token-2022 state', async () => {
    const c = await coordinates();
    const dirty = mutate(admission(), Abi.POSITION_ADMISSION_RESERVED_HEADER_OFFSET_V2, 1);
    const dirtyResult = await inspectDirectParticipantReadinessV1(await client(new Map([[c.admission, rpcAccount(CLAIMS, dirty)]])), request());
    expect(dirtyResult.reason).toContain('reserved bytes');

    const frozen = token(OWNER, c.custody);
    frozen[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] = 2;
    const frozenResult = await inspectDirectParticipantReadinessV1(await client(new Map([[c.collateral, rpcAccount(Abi.TOKEN_2022_PROGRAM_ID_V1, frozen)]])), request());
    expect(frozenResult.reason).toContain('frozen');

    const overdelegated = token(OWNER, c.custody, 9n, 10n);
    const overResult = await inspectDirectParticipantReadinessV1(await client(new Map([[c.collateral, rpcAccount(Abi.TOKEN_2022_PROGRAM_ID_V1, overdelegated)]])), request());
    expect(overResult.reason).toContain('delegation exceeds');
  });
});

/**
 * The seller's Direct token account exactly as Trading creates it.
 *
 * `direct_token_setup_v1` calls `initialize_account3(token_program, resource,
 * collateral_mint, seller_owner)` and then asserts the result byte-for-byte
 * against `TokenAccount::initialized_base_bytes(mint, owner)` -- mint, owner,
 * Initialized, and every other field zero. There is NO delegate, which is the
 * whole reason the buyer's participant model cannot be pointed at it.
 */
function sellerToken(owner: string, amount = 0n): Uint8Array {
  const bytes = new Uint8Array(Abi.TOKEN_ACCOUNT_BYTES_V1);
  const view = new DataView(bytes.buffer);
  bytes.set(new PublicKey(REALM.collateralMint).toBytes(), Abi.TOKEN_ACCOUNT_MINT_OFFSET_V1);
  bytes.set(new PublicKey(owner).toBytes(), Abi.TOKEN_ACCOUNT_OWNER_OFFSET_V1);
  view.setBigUint64(Abi.TOKEN_ACCOUNT_AMOUNT_OFFSET_V1, amount, true);
  bytes[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] = Abi.TOKEN_ACCOUNT_INITIALIZED_STATE_V1;
  return bytes;
}

const SELLER_TOKEN = deriveDirectSellerTokenAddressV1(TRADING, MARKET, GENERATION, OWNER);

describe('Direct seller readiness', () => {
  /**
   * The founder shape, which is market19's seller: it holds every claim through
   * the founding campaign, so it has a canonical Claims Position and was never
   * admitted by `devnet-user-position-admission-v1` -- no admission record, and
   * no participant collateral account.
   *
   * The buyer's model cannot describe it. The seller's model is the chain's:
   * `direct_token_setup_v1` names no admission account at any of its
   * twenty-three indices and derives the collateral itself.
   */
  it('is ready on the founder shape the participant model refuses, with a vacant Direct token account', async () => {
    const c = await coordinates();
    // Both readings of the founder, because the participant-shaped collateral
    // address is the one thing certainly vacant -- nothing on chain ever creates
    // it for a seller -- while whether an admission record accompanies the
    // founding Position decides only WHICH participant refusal the panel showed.
    for (const [label, chain, participantStatus, fragment] of [
      [
        'Position and admission, no participant collateral',
        await client(new Map([[c.collateral, null]])),
        'incomplete',
        'missing collateral account',
      ],
      [
        'Position alone',
        await client(new Map([[c.admission, null], [c.collateral, null]])),
        'refused',
        'only one of the atomic Claims Position and admission record exists',
      ],
    ] as const) {
      const asParticipant = await inspectDirectParticipantReadinessV1(chain, request());
      expect(asParticipant.status, label).toBe(participantStatus);
      expect(asParticipant.reason, label).toContain(fragment);

      const asSeller = await inspectDirectSellerReadinessV1(chain, request());
      expect(asSeller.status, label).toBe('ready');
      if (asSeller.status !== 'ready') throw new Error(asSeller.reason);
      expect(asSeller.coordinates.collateral, label).toBe(SELLER_TOKEN);
      expect(asSeller.coordinates, label).not.toHaveProperty('admission');
      expect(asSeller.collateralPrestate, label).toBe('vacant');
      expect(asSeller.positionBalances, label).toHaveLength(4);
      expect(asSeller.reason, label).toContain('still vacant');
    }
  });

  /**
   * The single line this lane exists for: the account Trading BUILDS for the
   * seller is refused by the model the panel was applying to it, because
   * `initialize_account3` leaves it with no delegate and the participant decoder
   * requires a Custody delegate. Wrong address AND wrong shape, independently.
   */
  it('accepts the live account Trading creates, which the participant decoder refuses for having no Custody delegate', async () => {
    const c = await coordinates();
    const live = new Map([
      [c.admission, null],
      [c.collateral, null],
      [SELLER_TOKEN, rpcAccount(Abi.TOKEN_2022_PROGRAM_ID_V1, sellerToken(OWNER, 4_000n))],
    ]);
    const asSeller = await inspectDirectSellerReadinessV1(await client(live), request());
    expect(asSeller).toMatchObject({ status: 'ready', collateralPrestate: 'initialized' });

    const asParticipant = await inspectDirectParticipantReadinessV1(
      await client(new Map([[c.collateral, rpcAccount(Abi.TOKEN_2022_PROGRAM_ID_V1, sellerToken(OWNER))]])),
      request(),
    );
    expect(asParticipant.status).toBe('refused');
    expect(asParticipant.reason).toContain('not delegated to this Market');
  });

  it('refuses a seller with no Claims Position, and every prestate neither on-chain site admits', async () => {
    const c = await coordinates();
    const noPosition = await inspectDirectSellerReadinessV1(
      await client(new Map([[c.position, null], [c.admission, null], [c.collateral, null]])),
      request(),
    );
    expect(noPosition).toMatchObject({ status: 'incomplete', missing: ['Claims Position'] });

    const frozen = sellerToken(OWNER);
    frozen[Abi.TOKEN_ACCOUNT_STATE_OFFSET_V1] = 2;
    const substituted = sellerToken(MARKET);
    for (const [label, account, fragment] of [
      ['frozen', rpcAccount(Abi.TOKEN_2022_PROGRAM_ID_V1, frozen), 'frozen'],
      ['substituted owner', rpcAccount(Abi.TOKEN_2022_PROGRAM_ID_V1, substituted), 'substitutes another Realm Mint or seller owner'],
      ['System-owned with data', rpcAccount('11111111111111111111111111111111', new Uint8Array(8)), 'System-owned but carries data'],
      ['a foreign program', rpcAccount(CLAIMS, sellerToken(OWNER)), 'neither the System program nor Token-2022'],
    ] as const) {
      const refused = await inspectDirectSellerReadinessV1(await client(new Map([[SELLER_TOKEN, account]])), request());
      expect(refused.status, label).toBe('refused');
      expect(refused.reason, label).toContain(fragment);
    }
  });

  /**
   * A cross-language control on the derivation itself, against market19's real
   * coordinates and the address the Rust test
   * `the_seller_direct_token_pda_is_not_the_participant_collateral_address`
   * pins in `direct_trade_producer.rs`. Two independent implementations, one
   * address -- and it is not the one the 2026-08-31 ticket was authored with.
   */
  it('reproduces the Rust seller Direct token PDA for market19, and it is not the participant address', () => {
    const derived = deriveDirectSellerTokenAddressV1(
      '5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk',
      '6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4',
      2n,
      'B6qxQCSwVeSfgcFyhNx38mcHs6FrTqRYpuDyQ4TVJ7cs',
    );
    expect(derived).toBe('2xGo6Cxtfb41HJCrrqWBf73TSaJrbjUmFqr2urDi91q8');
    expect(derived).not.toBe('HxwXjVqB9aFgNkcxpVHRycB9NqkTvVM3EPx3dxATyDcJ');
  });
});

function crossing(participant: DirectParticipantReadinessV1, side: 'buy' | 'sell', required: bigint): DirectCrossingPlanV1 {
  if (participant.status !== 'ready') throw new Error(participant.reason);
  return {
    takerAddress: participant.owner,
    taker: { market: participant.market, generation: participant.generation, collateralAccount: participant.coordinates.collateral, outcome: 0 },
    takerSide: side,
    fill: required,
    preview: { buyerCollateralDebit: required },
  } as unknown as DirectCrossingPlanV1;
}

describe('Direct crossing participant admission', () => {
  /**
   * The delegation is a single-use authorization, so `validate_collateral` in
   * `dclutch-direct-codec`'s `inline_candidate_v2.rs` requires
   * `delegated_amount == debit` and refuses either direction. This fixture
   * holds 20,000 collateral atoms with 12,000 delegated, so exactly one debit
   * is admissible and both neighbours must refuse for their own stated reason.
   */
  it('admits only the buy whose debit equals the delegation exactly, and sales against exact outcome claims', async () => {
    const participant = await inspectDirectParticipantReadinessV1(await client(), request());
    if (participant.status !== 'ready') throw new Error(participant.reason);
    expect(participant.collateralAtoms).toBe(20_000n);
    expect(participant.delegatedCollateralAtoms).toBe(12_000n);

    expect(admitDirectParticipantCrossingV1(participant, crossing(participant, 'buy', 12_000n)))
      .toMatchObject({ resource: 'collateral allowance', requiredAtoms: 12_000n, availableAtoms: 12_000n });

    // Under the standing delegation. A ceiling would have admitted this, and
    // the chain would then have refused the reader's packet.
    expect(() => admitDirectParticipantCrossingV1(participant, crossing(participant, 'buy', 11_000n)))
      .toThrow('more than the debit');
    expect(() => admitDirectParticipantCrossingV1(participant, crossing(participant, 'buy', 11_000n)))
      .toThrow('approve exactly 11000 atoms');

    // Over it, but still inside the balance: the delegation is what refuses.
    expect(() => admitDirectParticipantCrossingV1(participant, crossing(participant, 'buy', 13_000n)))
      .toThrow('less than the debit');

    // Past the balance: the balance refuses on its own terms, not as allowance.
    expect(() => admitDirectParticipantCrossingV1(participant, crossing(participant, 'buy', 20_001n)))
      .toThrow('your finalized token balance is 20000');

    expect(() => admitDirectParticipantCrossingV1(participant, { ...crossing(participant, 'buy', 1n), takerAddress: MARKET })).toThrow('another participant');
    if (participant.status !== 'ready') throw new Error(participant.reason);
    const available = participant.positionBalances[0] ?? 0n;
    expect(admitDirectParticipantCrossingV1(participant, crossing(participant, 'sell', available))).toMatchObject({ resource: 'claim balance', availableAtoms: available });
    expect(() => admitDirectParticipantCrossingV1(participant, crossing(participant, 'sell', available + 1n))).toThrow('current finalized Position');
  });
});
