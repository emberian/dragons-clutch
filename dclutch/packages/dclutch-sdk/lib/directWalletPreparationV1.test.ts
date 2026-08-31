import {
  AddressLookupTableAccount,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import bumpHintVector from '../fixtures/direct-hot-bump-hints.json';
import { activationCacheFixtureV1 } from '../fixtures/hotBumpHintSource';
import { type DirectHotRouteInspectionV3 } from './directHotChain';
import { type DirectHotBumpHintSourceV3 } from './directHotBumpHintsV1';
import {
  canonicalDirectInlineLookupAddressesV3,
  encodeHotBumpHintsV1,
  HOT_BUMP_HINTS_OFFSET_V1,
  HOT_BUMP_HINT_COUNT_V1,
  DIRECT_INLINE_CURRENT_LOOKUP_ADDRESSES_V3,
  DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3,
  DIRECT_INLINE_CURRENT_WIRE_BYTES_V3,
  projectDirectInlineSealedExecutionRouteV3,
  type DirectHotAccountMetaV3,
  type DirectInlineHotRouteV3,
  type SignedDirectIntentV3,
} from './directInlineV3';
import {
  deriveDirectMakerReplayAddressV1,
  inspectDirectMakerNoncePairV1,
} from './directMakerReplay';
import {
  deriveDirectSellerTokenAddressV1,
  type DirectParticipantReadinessV1,
  type DirectSellerReadinessV1,
} from './directParticipant';
import { planDirectCrossingV1 } from './directTicket';
import {
  prepareDirectWalletTransactionV1,
  type DirectWalletChainContextV1,
  type DirectWalletPreparationInputV1,
} from './directWalletPreparationV1';
import {
  CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_RENT_SYSVAR_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
} from './generated/directInlineV3';

const MAX_U64 = 0xffff_ffff_ffff_ffffn;

function key(seed: number): string {
  return new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
}

const SELLER = key(2);
const WALLET = key(4);
const TAKER_COLLATERAL = key(5);
const AGGREGATE = key(60);
const SELLER_POSITION = key(61);
const TAKER_POSITION = key(62);
const MARKET = key(10);
const TRADING = key(12);
const GENERATION = 19n;
/**
 * The seller's collateral is DERIVED here, not invented, because that is what
 * it is on chain: the role-separated PDA `direct_token_setup_v1` derives under
 * Trading and creates. A fixture that named an arbitrary key would still pass
 * every join in this file while agreeing with nothing the chain does.
 */
const SELLER_COLLATERAL = deriveDirectSellerTokenAddressV1(TRADING, MARKET, GENERATION, SELLER);
/** The address the participant `create_with_seed` derivation produces instead. */
const PARTICIPANT_SHAPED_SELLER_COLLATERAL = key(3);

function account(address: string, isWritable = false, executable = false, isSigner = false): DirectHotAccountMetaV3 {
  return Object.freeze({ address, isSigner, isWritable, executable });
}

function runtimeProfile(): Uint8Array {
  const fixedAccounts = 5 + DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3;
  const predicateBytes = 16;
  const profileHeader = 48 + predicateBytes;
  const output = new Uint8Array(profileHeader + fixedAccounts * 16);
  output.set(new TextEncoder().encode('DCLTAP02'), 0);
  const view = new DataView(output.buffer);
  view.setUint16(8, 2, true);
  view.setUint16(10, 14, true);
  view.setUint16(12, fixedAccounts, true);
  view.setUint16(20, 1, true);
  view.setUint16(42, 1, true);
  output[48] = 1;
  output[56] = 0x41;
  const writableRules = new Set([0, 5, 6, 7, 8, 12, 28, 29, 33, 35, 36, 41]);
  const executableRules = new Set([9, 10, 21, 22, 24, 26, 38, 43]);
  for (let index = 0; index < fixedAccounts; index += 1) {
    output[profileHeader + index * 16] = (index === 6 ? 1 : 0)
      | (writableRules.has(index) ? 2 : 0)
      | (executableRules.has(index) ? 4 : 0);
    view.setUint32(profileHeader + index * 16 + 8, 1, true);
  }
  return output;
}

function route(payer: string): DirectInlineHotRouteV3 {
  const market = MARKET;
  const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => account(key(20 + index)));
  fixed[HOT_MARKET_ACCOUNT_V3] = account(market);
  fixed[HOT_ROOT_ACCOUNT_V3] = account(key(11), true);
  fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = account(TRADING, false, true);
  fixed[HOT_RENT_SYSVAR_ACCOUNT_V3] = account(SYSVAR_RENT_PUBKEY.toBase58());
  fixed[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = account(SYSVAR_INSTRUCTIONS_PUBKEY.toBase58());
  const runtimeFixed = new Map([
    [8, 37], [9, 31], [10, 32], [11, 33], [12, 35], [13, 28], [14, 0],
    [15, 22], [16, 27], [17, 25], [18, 26], [21, 23], [22, 24],
  ]);
  const writable = new Set([0, 1, 2, 3, 7, 23, 24, 28, 30, 31, 36]);
  const executable = new Set([4, 5, 16, 17, 19, 21, 33, 38]);
  const runtimeAccounts = Object.freeze(Array.from(
    { length: DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3 },
    (_, index) => {
      const joined = new Map<number, string>([
        [0, deriveDirectMakerReplayAddressV1(TRADING, market, GENERATION, SELLER).address],
        [3, deriveDirectMakerReplayAddressV1(TRADING, market, GENERATION, WALLET).address],
        [7, AGGREGATE],
        [23, SELLER_POSITION],
        [24, TAKER_POSITION],
        [30, TAKER_COLLATERAL],
        [31, SELLER_COLLATERAL],
      ]);
      return account(
      index === 1 ? payer : joined.get(index) ?? (runtimeFixed.has(index) ? fixed[runtimeFixed.get(index)!]!.address : key(100 + index)),
      writable.has(index), executable.has(index), index === 1,
      );
    },
  ));
  const projected = projectDirectInlineSealedExecutionRouteV3(Object.freeze({
    payer,
    tradingProgram: TRADING,
    market,
    releaseSet: new Uint8Array(32).fill(31),
    generation: GENERATION,
    rootPrestateDigest: new Uint8Array(32).fill(32),
    outcomeCount: 51,
    priceScale: 1_000_000n,
    feeBasisPoints: 25,
    accountProfile: runtimeProfile(),
    selectedProgramSchema: CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
    selectedProgram: new Uint8Array(32).fill(33),
    observedSlot: 1_000n,
    fixedAccounts: Object.freeze(fixed),
    strategyAccounts: Object.freeze([]),
    runtimeAccounts,
    recentBlockhash: key(92),
    blockhashObservedSlot: 1_001n,
    lastValidBlockHeight: 2_000n,
    lookupTableCreationSlot: 700n,
    lookupTables: Object.freeze([]),
    outerEvidence: Object.freeze({
      status: 'checked' as const,
      tradingArtifactRelease: '11'.repeat(32),
      checkedManifestDigest: '12'.repeat(32),
    }),
  }));
  const lookupTable = new AddressLookupTableAccount({
    key: new PublicKey(key(90)),
    state: {
      deactivationSlot: MAX_U64,
      lastExtendedSlot: 800,
      lastExtendedSlotStartIndex: 0,
      authority: undefined,
      addresses: [...canonicalDirectInlineLookupAddressesV3(projected)],
    },
  });
  return Object.freeze({ ...projected, lookupTables: Object.freeze([lookupTable]) });
}

function routeInspection(candidate: DirectInlineHotRouteV3): DirectHotRouteInspectionV3 {
  if (candidate.outerEvidence.status !== 'checked') throw new Error('test route must be checked');
  return Object.freeze({
    observedSlot: candidate.observedSlot.toString(),
    route: candidate,
    selectedProgramSchema: '21'.repeat(32),
    selectedProgramDigest: '22'.repeat(32),
    programSetDigest: '23'.repeat(32),
    accountProfileDigest: '24'.repeat(32),
    strategyDigest: '25'.repeat(32),
    transitionDigest: '26'.repeat(32),
    capabilitySealDigest: '27'.repeat(32),
    checkedOuter: candidate.outerEvidence,
    bumpHintSource: bumpHintSource(candidate),
  });
}

/**
 * The three finalized bodies a real inspection keeps so the wallet path can
 * mine its hint block.
 *
 * The Market state and root header are the canonical encodings emitted by
 * `crates/dclutch-operator/tests/browser_bump_hint_vector.rs`; nothing in this
 * file depends on WHICH identities they carry, because the two slots they feed
 * are derived from their own bytes rather than from the route. What this file
 * does depend on is that mining runs at all on a route this builder accepts,
 * and that the two lifecycle slots agree with the replay accounts the runtime
 * frame already carries -- which is the join `mineDirectInlineHotBumpHintsV3`
 * refuses on.
 */
function bumpHintSource(candidate: DirectInlineHotRouteV3): DirectHotBumpHintSourceV3 {
  return Object.freeze({
    coreProgram: bumpHintVector.coreProgram,
    marketCoreState: fromHex(bumpHintVector.marketCoreStateHex),
    capabilityRootHeader: fromHex(bumpHintVector.capabilityRootHeaderHex),
    activationCache: activationCacheFixtureV1(new Uint8Array(candidate.releaseSet), { custody: key(70) }),
  });
}

function fromHex(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

async function noncePair(candidate: DirectInlineHotRouteV3, seller = SELLER, taker = WALLET) {
  return inspectDirectMakerNoncePairV1({
    finalizedSlot: async () => '1002',
    multipleAccounts: async (addresses) => Object.freeze({
      slot: '1002',
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: null }))),
    }),
  }, [
    { tradingProgram: candidate.tradingProgram, market: candidate.market, generation: candidate.generation, maker: seller },
    { tradingProgram: candidate.tradingProgram, market: candidate.market, generation: candidate.generation, maker: taker },
  ]);
}

function positionBalancesFor(candidate: DirectInlineHotRouteV3): ReadonlyArray<bigint> {
  return Object.freeze(Array.from({ length: candidate.outcomeCount }, (_, index) => index === 0 ? 2_000n : 0n));
}

function participant(
  candidate: DirectInlineHotRouteV3,
  owner: string,
  collateral: string,
): DirectParticipantReadinessV1 {
  return Object.freeze({
    status: 'ready' as const,
    observedSlot: '1002',
    market: candidate.market,
    generation: candidate.generation,
    owner,
    coordinates: Object.freeze({
      aggregate: AGGREGATE,
      position: TAKER_POSITION,
      admission: key(64),
      collateral,
      custodyAuthority: key(65),
    }),
    collateralMint: key(66),
    tokenProgram: key(67),
    positionRevision: 4n,
    positionBalances: positionBalancesFor(candidate),
    // The balance is a floor the debit must fit under; the delegation is an
    // equality. These crossings debit 1,002 atoms, and a single-use
    // authorization for any other amount is a state the chain refuses, so the
    // fixture cannot hold a round 10,000 delegation and still describe a
    // participant a packet could be prepared for.
    collateralAtoms: 10_000n,
    delegatedCollateralAtoms: 1_002n,
    spendableCollateralAtoms: 1_002n,
    reason: 'authenticated test participant',
  });
}

/**
 * The seller half as the chain models it: an aggregate, a Position, a Direct
 * token account, a Custody authority -- and NO admission coordinate, because
 * `direct_token_setup_v1` names no admission account at any of its
 * twenty-three indices and the founder of a Market has none.
 */
function sellerReadiness(
  candidate: DirectInlineHotRouteV3,
  collateral: string = SELLER_COLLATERAL,
  collateralPrestate: 'vacant' | 'initialized' = 'initialized',
): DirectSellerReadinessV1 {
  return Object.freeze({
    status: 'ready' as const,
    observedSlot: '1002',
    market: candidate.market,
    generation: candidate.generation,
    owner: SELLER,
    coordinates: Object.freeze({
      aggregate: AGGREGATE,
      position: SELLER_POSITION,
      collateral,
      custodyAuthority: key(65),
    }),
    collateralMint: key(66),
    tokenProgram: key(67),
    positionRevision: 4n,
    positionBalances: positionBalancesFor(candidate),
    collateralPrestate,
    reason: 'authenticated test seller',
  });
}

async function fixture(payer: 'wallet' | 'operator' = 'wallet'): Promise<DirectWalletPreparationInputV1> {
  const wallet = WALLET;
  const seller = SELLER;
  const candidate = route(payer === 'wallet' ? wallet : key(91));
  const replayPair = await noncePair(candidate);
  const sellerReplay = replayPair[0];
  const takerReplay = replayPair[1];
  const signedSeller: SignedDirectIntentV3 = Object.freeze({
    maker: seller,
    signature: new Uint8Array(64).fill(11),
    intent: Object.freeze({
      side: 0,
      lifecycle: 0,
      outcome: 0,
      market: candidate.market,
      generation: candidate.generation,
      nonce: sellerReplay.nextNonce,
      validFrom: 900n,
      validThrough: 1_200n,
      maximumFill: 2_000n,
      limitPrice: 500_000n,
      feeBasisPoints: candidate.feeBasisPoints,
      collateralAccount: SELLER_COLLATERAL,
    }),
  });
  const crossingPlan = planDirectCrossingV1({
    route: candidate,
    ticket: signedSeller,
    takerAddress: wallet,
    takerReplay,
    takerCollateralAccount: TAKER_COLLATERAL,
    desiredFill: 2_000n,
    clockSlot: 1_002n,
  });
  const signedTaker: SignedDirectIntentV3 = Object.freeze({
    maker: wallet,
    signature: new Uint8Array(64).fill(12),
    intent: crossingPlan.taker,
  });
  const chain: DirectWalletChainContextV1 = Object.freeze({
    rpcEndpoint: 'https://rpc.example.test/',
    genesisHash: key(200),
  });
  return Object.freeze({
    routeInspection: routeInspection(candidate),
    ticketInspection: signedSeller,
    crossingPlan,
    sellerParticipant: sellerReadiness(candidate),
    takerParticipant: participant(candidate, wallet, TAKER_COLLATERAL),
    noncePair: replayPair,
    signedSeller,
    signedTaker,
    context: Object.freeze({
      route: chain,
      sellerParticipant: chain,
      takerParticipant: chain,
      noncePair: chain,
      planning: Object.freeze({ ...chain, connectedWallet: wallet }),
      current: Object.freeze({
        ...chain,
        connectedWallet: wallet,
        finalizedSlot: 1_002n,
        blockHeight: 1_500n,
      }),
    }),
  });
}

describe('Direct wallet preparation V1', () => {
  it('returns one frozen, exact wallet-payer v0 plan without submitting it', async () => {
    const input = await fixture();
    const prepared = prepareDirectWalletTransactionV1(input);
    expect(prepared.status).toBe('wallet-preparable');
    expect(prepared.payerBranch).toBe('wallet-pays');
    expect(prepared.payer).toBe(input.context.current.connectedWallet);
    expect(Object.isFrozen(prepared)).toBe(true);
    expect(Object.isFrozen(prepared.binding)).toBe(true);
    expect(prepared.binding.routeObservedSlot).toBe('1000');
    expect(prepared.binding.seller.nonceAddress).toBe(input.noncePair[0].address);
    expect(prepared.binding.taker.participantObservedSlot).toBe('1002');
    if (prepared.status !== 'wallet-preparable') throw new Error('unreachable test branch');
    expect(prepared.transactionPlan.requiredSigners).toEqual([input.context.current.connectedWallet]);
    expect(prepared.transactionPlan.loadedAddresses).toBe(DIRECT_INLINE_CURRENT_LOOKUP_ADDRESSES_V3);
    expect(prepared.transactionPlan.wireBytes).toHaveLength(DIRECT_INLINE_CURRENT_WIRE_BYTES_V3);
  });

  it('compiles a MINED wallet wire, at the same 1,167 bytes as the unmined one', async () => {
    // The lane's lead claim: a browser-built trade carries hints. Six of the
    // eight slots are filled from the finalized bodies the route inspection
    // already read; the two child caller-authority slots stay zero because
    // their seeds end in a digest over a PROJECTED child request, which is the
    // same gap `build_direct_inline_hot_v4` has and takes as a parameter.
    const input = await fixture();
    const prepared = prepareDirectWalletTransactionV1(input);
    if (prepared.status !== 'wallet-preparable') throw new Error('unreachable test branch');
    const plan = prepared.transactionPlan;
    expect(plan.minedBumpHintSlots).toBe(6);
    expect(plan.bumpHints.childCaller).toEqual([0, 0]);
    for (const [field, bump] of [
      ['market', plan.bumpHints.market],
      ['root', plan.bumpHints.root],
      ['seller replay', plan.bumpHints.lifecycle[0]],
      ['buyer replay', plan.bumpHints.lifecycle[1]],
      ['Custody replay', plan.bumpHints.childRelay[0]],
      ['Custody authority', plan.bumpHints.childRelay[1]],
    ] as const) {
      expect(bump, `${field} slot mined to absent`).not.toBe(0);
    }
    const block = plan.hotInstructionBytes.slice(HOT_BUMP_HINTS_OFFSET_V1, HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1);
    expect([...block]).toEqual([...encodeHotBumpHintsV1(plan.bumpHints)]);
    // The two lifecycle slots are the replay accounts the runtime frame already
    // carries: mining derived them a second time, from seeds, and the miner
    // refuses if the two derivations disagree.
    expect(plan.bumpHints.lifecycle[0]).toBe(deriveDirectMakerReplayAddressV1(
      input.routeInspection.route.tradingProgram, input.routeInspection.route.market,
      input.routeInspection.route.generation, input.signedSeller.maker,
    ).bump);
    expect(plan.wireBytes).toHaveLength(DIRECT_INLINE_CURRENT_WIRE_BYTES_V3);
  });

  it('returns an honest operator handoff naming the exact route payer and does not compile', async () => {
    const input = await fixture('operator');
    const prepared = prepareDirectWalletTransactionV1(input);
    expect(prepared).toMatchObject({
      status: 'operator-required',
      payerBranch: 'operator-required',
      payer: input.routeInspection.route.payer,
    });
    expect(prepared.payer).not.toBe(input.context.current.connectedWallet);
    expect('transactionPlan' in prepared).toBe(false);
    if (prepared.status !== 'operator-required') throw new Error('unreachable test branch');
    expect(prepared.signedIntents).toEqual({ seller: input.signedSeller, buyer: input.signedTaker });
  });

  it('refuses account switches and mixed RPC or genesis acquisition contexts', async () => {
    const input = await fixture();
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      context: { ...input.context, current: { ...input.context.current, connectedWallet: key(6) } },
    })).toThrow(/wallet changed/);
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      context: {
        ...input.context,
        noncePair: { ...input.context.noncePair, rpcEndpoint: 'https://other-rpc.example.test/' },
      },
    })).toThrow(/another RPC endpoint/);
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      context: {
        ...input.context,
        sellerParticipant: { ...input.context.sellerParticipant, genesisHash: key(201) },
      },
    })).toThrow(/another genesis hash/);
  });

  it('refuses a payer substitution that no longer matches the authenticated signer account', async () => {
    const input = await fixture();
    const substitutedRoute = Object.freeze({ ...input.routeInspection.route, payer: key(91) });
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      routeInspection: Object.freeze({ ...input.routeInspection, route: substitutedRoute }),
    })).toThrow(/unexpected transaction co-signer/);
  });

  it('refuses stale or replayed signed nonces against both authenticated maker roots', async () => {
    const input = await fixture();
    const replayedSeller = Object.freeze({
      ...input.signedSeller,
      intent: Object.freeze({ ...input.signedSeller.intent, nonce: input.signedSeller.intent.nonce + 1n }),
    });
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      ticketInspection: replayedSeller,
      signedSeller: replayedSeller,
      crossingPlan: Object.freeze({ ...input.crossingPlan, ticket: replayedSeller }),
    })).toThrow(/stale, future, or already consumed/);

    const anotherMakerPair = await noncePair(input.routeInspection.route, key(8), WALLET);
    expect(() => prepareDirectWalletTransactionV1({ ...input, noncePair: anotherMakerPair })).toThrow(/another Trading program, Market, generation, or maker/);
  });

  it('refuses exact route joins and an expired blockhash before wallet compilation', async () => {
    const input = await fixture();
    const mismatchedRoute = Object.freeze({ ...input.routeInspection.route, feeBasisPoints: 26 });
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      routeInspection: Object.freeze({ ...input.routeInspection, route: mismatchedRoute }),
    })).toThrow(/Market, generation, outcome, price, fill, or fee/);
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      context: { ...input.context, current: { ...input.context.current, blockHeight: 2_001n } },
    })).toThrow(/blockhash expired/);
    const substitutedRuntime = [...input.routeInspection.route.runtimeAccounts];
    substitutedRuntime[23] = account(key(210), true);
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      routeInspection: Object.freeze({
        ...input.routeInspection,
        route: Object.freeze({ ...input.routeInspection.route, runtimeAccounts: Object.freeze(substitutedRuntime) }),
      }),
    })).toThrow(/substitutes the seller Position coordinate/);
  });

  /**
   * The fourth wall, from both sides.
   *
   * market19's seller is the founder: it holds every claim through the founding
   * campaign, so it has a Claims Position and NO admission record, and no
   * participant collateral account was ever created for it. The panel used to
   * derive the seller's collateral as `deriveParticipantCollateralV1` and demand
   * that owner be a READY participant, which is the buyer's shape -- so it
   * refused this route before reading a single economic fact.
   *
   * The chain's own seller preconditions are `direct_token_setup_v1`'s: own the
   * canonical Claims Position, and let Trading derive and create the Direct
   * token PDA. Both are met here, so preparation must reach the compiled wire.
   */
  it('prepares market19’s seller half: a Position, no admission record, and a Direct token account still vacant', async () => {
    const input = await fixture();
    const founder = sellerReadiness(input.routeInspection.route, SELLER_COLLATERAL, 'vacant');
    if (founder.status !== 'ready') throw new Error('unreachable test branch');
    expect(Object.keys(founder.coordinates)).not.toContain('admission');
    const prepared = prepareDirectWalletTransactionV1({ ...input, sellerParticipant: founder });
    expect(prepared.status).toBe('wallet-preparable');
    expect(prepared.binding.seller.collateralPrestate).toBe('vacant');
    expect(prepared.binding.seller.coordinates.collateral).toBe(SELLER_COLLATERAL);
    expect(prepared.binding.seller.coordinates).not.toHaveProperty('admission');
    if (prepared.status !== 'wallet-preparable') throw new Error('unreachable test branch');
    expect(prepared.transactionPlan.requiredSigners).toEqual([WALLET]);
  });

  it('still refuses a seller whose Claims Position or claim balance the chain would refuse', async () => {
    const input = await fixture();
    // No Position: `authenticate_seller_position` cannot decode one, so Trading
    // will not derive a Direct token account and there is nothing to sell.
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      sellerParticipant: Object.freeze({
        status: 'incomplete' as const,
        observedSlot: '1002',
        market: input.routeInspection.route.market,
        generation: input.routeInspection.route.generation,
        owner: SELLER,
        coordinates: Object.freeze({
          aggregate: AGGREGATE, position: SELLER_POSITION,
          collateral: SELLER_COLLATERAL, custodyAuthority: key(65),
        }),
        missing: Object.freeze(['Claims Position'] as const),
        reason: 'this seller holds no Claims Position for this Market',
      }),
    })).toThrow(/seller Direct state is incomplete, not ready/);

    const ready = sellerReadiness(input.routeInspection.route);
    if (ready.status !== 'ready') throw new Error('unreachable test branch');
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      sellerParticipant: Object.freeze({
        ...ready,
        positionBalances: Object.freeze(ready.positionBalances.map(() => 0n)),
      }),
    })).toThrow(/seller finalized Position does not cover the exact planned claim fill/);
  });

  it('refuses a seller ticket authored with the BUYER’s create_with_seed collateral derivation', async () => {
    // The exact ticket the 2026-08-31 session authored and the producer refused:
    // the participant address, which nothing on chain ever creates for a seller.
    // The panel now re-derives Trading's own seeds and names the disagreement,
    // rather than believing whichever address the readiness reported.
    const input = await fixture();
    const substitutedSeller = Object.freeze({
      ...input.signedSeller,
      intent: Object.freeze({ ...input.signedSeller.intent, collateralAccount: PARTICIPANT_SHAPED_SELLER_COLLATERAL }),
    });
    expect(() => prepareDirectWalletTransactionV1({
      ...input,
      ticketInspection: substitutedSeller,
      signedSeller: substitutedSeller,
      crossingPlan: Object.freeze({ ...input.crossingPlan, ticket: substitutedSeller }),
      sellerParticipant: sellerReadiness(input.routeInspection.route, PARTICIPANT_SHAPED_SELLER_COLLATERAL),
    })).toThrow(/not the Direct token account Trading derives/);
  });
});
