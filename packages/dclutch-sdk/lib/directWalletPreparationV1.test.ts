import {
  AddressLookupTableAccount,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { type DirectHotRouteInspectionV3 } from './directHotChain';
import {
  canonicalDirectInlineLookupAddressesV3,
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
import { type DirectParticipantReadinessV1 } from './directParticipant';
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
const SELLER_COLLATERAL = key(3);
const WALLET = key(4);
const TAKER_COLLATERAL = key(5);
const AGGREGATE = key(60);
const SELLER_POSITION = key(61);
const TAKER_POSITION = key(62);

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
  const market = key(10);
  const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => account(key(20 + index)));
  fixed[HOT_MARKET_ACCOUNT_V3] = account(market);
  fixed[HOT_ROOT_ACCOUNT_V3] = account(key(11), true);
  fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = account(key(12), false, true);
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
        [0, deriveDirectMakerReplayAddressV1(key(12), market, 19n, SELLER).address],
        [3, deriveDirectMakerReplayAddressV1(key(12), market, 19n, WALLET).address],
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
    tradingProgram: key(12),
    market,
    releaseSet: new Uint8Array(32).fill(31),
    generation: 19n,
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
  });
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

function participant(
  candidate: DirectInlineHotRouteV3,
  owner: string,
  collateral: string,
  side: 'seller' | 'taker',
): DirectParticipantReadinessV1 {
  const positionBalances = Array.from({ length: candidate.outcomeCount }, (_, index) => index === 0 ? 2_000n : 0n);
  return Object.freeze({
    status: 'ready' as const,
    observedSlot: '1002',
    market: candidate.market,
    generation: candidate.generation,
    owner,
    coordinates: Object.freeze({
      aggregate: AGGREGATE,
      position: side === 'seller' ? SELLER_POSITION : TAKER_POSITION,
      admission: key(side === 'seller' ? 63 : 64),
      collateral,
      custodyAuthority: key(65),
    }),
    collateralMint: key(66),
    tokenProgram: key(67),
    positionRevision: 4n,
    positionBalances: Object.freeze(positionBalances),
    collateralAtoms: 10_000n,
    delegatedCollateralAtoms: 10_000n,
    spendableCollateralAtoms: 10_000n,
    reason: 'authenticated test participant',
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
    sellerParticipant: participant(candidate, seller, SELLER_COLLATERAL, 'seller'),
    takerParticipant: participant(candidate, wallet, TAKER_COLLATERAL, 'taker'),
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
});
