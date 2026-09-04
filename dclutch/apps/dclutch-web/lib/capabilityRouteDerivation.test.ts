import {
  AddressLookupTableAccount,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  type VersionedTransaction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  censusRouteIdsForInstructionsV1,
  magicIsAmbiguousV1,
  type CompiledProtocolInstructionV1,
} from '@dclutch/sdk/routeSelector';
import { CAPABILITY_ACTIONS_V1 } from './capabilityModel';
import { compileCoreFoundTransactionV2, compileLifecycleRentCreateTransactionV2 } from './coreFound';
import { compileDealerEquityTransactionV3, type DealerEquityHotRouteV3 } from './dealerEquityV3';
import { type DirectHotAccountMetaV3 } from './directInlineV3';
import { encodeClaimsCustodyReplayRequestV1 } from './claimsCustodyReplay';
import { encodeWalletTerminalPayoutRequestV3 } from './walletTerminalPayoutV3';
import { compileRegistryReauthenticationTransaction, compileRegistryRoleActivationTransaction } from './releaseRegistry';
import { CORE_FOUND_ACCOUNT_COUNT_V3 } from './generated/coreFound';
import {
  DEALER_EQUITY_HEADER_BYTES_V3,
  DEALER_LP_POSITION_PDA_DOMAIN_V3,
  DEALER_OBLIGATION_PDA_DOMAIN_V3,
} from './generated/dealerEquityV3';
import {
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_RENT_SYSVAR_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
} from './generated/directInlineV3';

/**
 * The routes an act declares, checked against the routes its builder EMITS.
 *
 * WHAT WAS WRONG. `CAPABILITY_ACTIONS_V1` declares census route ids by hand,
 * and the only check on them was that `routes.md` publishes a row with that
 * name (`capabilityPhaseGate.test.ts`). A name that exists passes — so an act
 * declaring a route its transaction never reaches, and an act reaching a route
 * it declares nothing about, both read as correct. The second is the dangerous
 * one: an undeclared route is an unread phase gate, and an unread gate renders
 * as READY TO PREFLIGHT.
 *
 * WHAT IS HERE. For every act whose BUILDER AUTHORS THE INSTRUCTION BYTES, the
 * builder is run against a fixture and the compiled instruction is put through
 * `censusRouteIdsForInstructionsV1`, which reads the census's own selector
 * tables. The declaration must contain every route those bytes select. Nothing
 * in this file names a magic; the fixtures name programs and inputs, and the
 * magic comes out of the builder.
 *
 * WHICH ACTS THAT IS, and why it is not all of them. Seven acts do not author
 * their own bytes at all: a Rust planner does, and the browser deserializes
 * the message it produced and re-checks its geometry (`market.join`,
 * `source.create-fund`, `source.ready`, `source.provider`,
 * `source.admit-terminal`, `source.close-fund`) or hostile-decodes a
 * transaction a person pasted in (`general.consider`, `general.settle`,
 * `general.close`). Running the derivation over those fixtures would measure
 * the fixture's placeholder bytes, not the protocol's — so their declarations
 * are cited to the planner instead, in `capabilityModel.ts`, and named here as
 * what they are.
 */

const CLAIMS = '11111111111111111111111111111114';
const TOKEN = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';

function address(seed: number): string {
  const bytes = new Uint8Array(32);
  new DataView(bytes.buffer).setUint32(0, seed, true);
  bytes[31] = 1;
  return new PublicKey(bytes).toBase58();
}

function hex(byte: number): string {
  return byte.toString(16).padStart(2, '0').repeat(32);
}

const act = (id: string) => {
  const found = CAPABILITY_ACTIONS_V1.find((one) => one.id === id);
  if (found === undefined) throw new Error(`no act ${id}`);
  return found;
};

/**
 * The protocol instructions of a compiled transaction, by census program.
 *
 * A transaction carries ComputeBudget, System and Ed25519 instructions that
 * belong to no dClutch program; they are dropped by not being in the map,
 * which is the only way a caller can be sure it did not silently classify one.
 */
function protocolInstructionsV1(
  transaction: VersionedTransaction,
  programs: Readonly<Record<string, string>>,
): ReadonlyArray<CompiledProtocolInstructionV1> {
  const keys = transaction.message.staticAccountKeys.map((one) => one.toBase58());
  const output: CompiledProtocolInstructionV1[] = [];
  for (const compiled of transaction.message.compiledInstructions) {
    const program = programs[keys[compiled.programIdIndex] ?? ''];
    if (program === undefined) continue;
    output.push({ program, data: Uint8Array.from(compiled.data) });
  }
  return output;
}

describe('an act declares the routes its own builder emits', () => {
  it('release.activate reaches the Registry route its instruction selects', () => {
    const registry = address(11);
    const activation = compileRegistryRoleActivationTransaction({
      payer: address(12), registryProgram: registry, recentBlockhash: address(13), computeUnitLimit: 200_000,
      cache: address(14), releaseSetRecord: address(15), releaseSetStaging: address(16), role: 'trading',
      addresses: { record: address(17), staging: address(18), program: address(19), programData: address(20) },
    });
    const reauth = compileRegistryReauthenticationTransaction({
      payer: address(12), registryProgram: registry, recentBlockhash: address(13), computeUnitLimit: 200_000,
      cache: address(14), role: 'trading', program: address(19), programData: address(20),
    });
    const derived = censusRouteIdsForInstructionsV1([
      ...protocolInstructionsV1(activation.transaction, { [registry]: 'registry' }),
      ...protocolInstructionsV1(reauth.transaction, { [registry]: 'registry' }),
    ]);
    expect(derived).toEqual(['registry/record_v1::dispatch']);
    for (const route of derived) expect(act('release.activate').routes).toContain(route);
  });

  it('dealer.liquidity reaches the Trading hot route its envelope selects', async () => {
    const value = dealerFixture();
    const plan = await compileDealerEquityTransactionV3(value.route, value.request);
    const derived = censusRouteIdsForInstructionsV1(
      protocolInstructionsV1(plan.transaction, { [value.route.tradingProgram]: 'trading' }),
    );
    // The route this browser has been building since `/liquidity` shipped, and
    // declaring nothing about: an undeclared route is an unread gate.
    expect(derived).toEqual(['trading/hot_v3::process_hot_execution_v3']);
    for (const route of derived) expect(act('dealer.liquidity').routes).toContain(route);
  });

  it('claims.replay reaches the Claims route its request selects', () => {
    const derived = censusRouteIdsForInstructionsV1([
      { program: 'claims', data: encodeClaimsCustodyReplayRequestV1(address(21)) },
    ]);
    expect(derived).toEqual(['claims/custody_replay_v1::process']);
    expect(act('claims.replay').routes).toEqual(derived);
  });

  it('claims.redeem reaches the Claims route its request selects', () => {
    const derived = censusRouteIdsForInstructionsV1([
      { program: 'claims', data: encodeWalletTerminalPayoutRequestV3(payoutRequest()) },
    ]);
    expect(derived).toEqual(['claims/terminal_settlement_v3::process']);
    expect(act('claims.redeem').routes).toEqual(derived);
  });
});

describe('what the derivation cannot decide, and says so instead of guessing', () => {
  it('names no route for Core Found, because Core dispatches on a variant', () => {
    // `market.found` declares `core/found::process#Found`, whose census
    // selector is `tag Action::Found` plus a length — not a magic. The
    // instruction this builder emits starts with Core's request magic, which
    // appears in no census selector at all, so the derivation is EMPTY and the
    // declaration stands on the census's variant selector instead. Asserted so
    // that a Core magic arriving in the census later fails here first.
    const core = address(31);
    const accounts = Array.from({ length: CORE_FOUND_ACCOUNT_COUNT_V3 }, (_, index) => address(100 + index));
    accounts[0] = address(30);
    accounts[1] = address(32);
    accounts[25] = core;
    const compiled = compileCoreFoundTransactionV2({
      payer: accounts[0]!, coreProgram: core, market: accounts[1]!, generation: 7n,
      recentBlockhash: address(33), accountAddresses: accounts,
      lookupTable: new AddressLookupTableAccount({
        key: new PublicKey(address(34)),
        state: {
          deactivationSlot: 18_446_744_073_709_551_615n, lastExtendedSlot: 800,
          lastExtendedSlotStartIndex: 0, authority: undefined,
          addresses: accounts.map((one) => new PublicKey(one)),
        },
      }),
    });
    const instructions = protocolInstructionsV1(compiled.transaction, { [core]: 'core' });
    expect(instructions).toHaveLength(1);
    expect(censusRouteIdsForInstructionsV1(instructions)).toEqual([]);
    expect(act('market.found').routes).toEqual(['core/found::process#Found']);
  });

  it('returns Rent’s whole candidate set for the RentCredit leg of a founding', () => {
    // One magic, three lifecycle arms, separated by a decoded variant this
    // derivation has no offset for. The founding's Rent leg is therefore a set
    // and not an answer, which is why `market.found` declares no Rent route:
    // declaring all three would publish two gates the act never reaches.
    const rent = address(41);
    const compiled = compileLifecycleRentCreateTransactionV2({
      payer: address(40), refundWallet: address(42), market: address(43),
      releaseSet: new Uint8Array(32).fill(5), generation: 7n, rentProgram: rent,
      recentBlockhash: address(44),
    });
    const derived = censusRouteIdsForInstructionsV1(protocolInstructionsV1(compiled.transaction, { [rent]: 'rent' }));
    expect(derived).toEqual([
      'rent/process_close_v2#Close',
      'rent/process_create_v2#Create',
      'rent/process_sweep_v2#Sweep',
    ]);
    expect(magicIsAmbiguousV1('rent', 'DCLRNCI2')).toBe(true);
    expect(act('market.found').routes).not.toContain('rent/process_create_v2#Create');
  });
});

/** The acts whose instruction bytes are authored outside this browser. */
const PLANNER_AUTHORED_V1: Readonly<Record<string, string>> = Object.freeze({
  'market.join': 'crates/dclutch-user-position-admission-wasm',
  'source.create-fund': 'crates/dclutch-source-readiness-operator',
  'source.ready': 'crates/dclutch-source-readiness-operator',
  'source.provider': 'crates/dclutch-provider-transport-v3-operator',
  'source.admit-terminal': 'crates/dclutch-resolution-core-v3-operator',
  'source.close-fund': 'crates/dclutch-resolution-core-v3-operator',
  'general.consider': 'a transaction the reader pastes in; this browser only hostile-decodes it',
  'general.settle': 'a transaction the reader pastes in; this browser only hostile-decodes it',
  'general.close': 'a transaction the reader pastes in; this browser only hostile-decodes it',
});

describe('every act with a declared route says who authored its bytes', () => {
  it.each(CAPABILITY_ACTIONS_V1.filter((one) => one.routes.length > 0).map((one) => [one.id] as const))(
    '%s is compiled here or cited to a planner',
    (id) => {
      const compiledHere = ['release.activate', 'market.found', 'direct.inline', 'dealer.liquidity', 'claims.replay', 'claims.redeem'];
      // Neither list may grow silently: an act that gains a route gains a
      // derivation or a citation in the same change, or this fails.
      expect(compiledHere.includes(id) || id in PLANNER_AUTHORED_V1, `${id} declares routes with no author`).toBe(true);
    },
  );

  it('direct.inline is the one browser-authored act with no fixture here', () => {
    // Owed, and named rather than left to look covered. Its compiler needs a
    // whole checked hot route -- fixed, strategy and runtime metas, a
    // validated Profile14 account profile, a canonical lookup table and two
    // signed intents -- and `dealer.liquidity` above compiles the identical
    // envelope, so the `DCLTHOT3` derivation it relies on IS exercised.
    expect(act('direct.inline').routes).toEqual(['trading/hot_v3::process_hot_execution_v3']);
    expect(act('dealer.liquidity').routes).toEqual(act('direct.inline').routes);
  });
});

// ---------------------------------------------------------------- fixtures

function payoutRequest() {
  return {
    releaseSet: hex(1), market: address(50), realm: hex(2), parentContext: hex(3),
    productRecordDigest: hex(4), exposureId: hex(5), exposureDigest: hex(6),
    terminalRecordDigest: hex(7), owner: address(51), position: address(52),
    recipientOwner: address(51), recipient: address(53), claimsProgram: CLAIMS,
    custodyProgram: address(54), collateralMint: address(55), tokenProgram: TOKEN,
    semanticBasisId: hex(8), linkedBasisRecordDigest: hex(9), generation: '7',
    expectedMarketRevision: '1', expectedPositionRevision: '1', expectedCustodyRevision: '1',
    quantity: '1000', claimIndex: 0, transferIndex: 0,
  };
}

function meta(value: string, writable = false, executable = false): DirectHotAccountMetaV3 {
  return Object.freeze({ address: value, isSigner: false, isWritable: writable, executable });
}

function put64(output: Uint8Array, offset: number, value: bigint): void {
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true);
}

/**
 * One P0 Dealer contribution, in the shape `dealerEquityV3.test.ts` uses.
 *
 * The request header is hand-built because a Dealer request is authored by the
 * scenario kernel and handed to this browser; what is NOT hand-built is the
 * outer instruction, which `compileDealerEquityTransactionV3` writes and which
 * is the thing under test.
 */
function dealerFixture(): Readonly<{ route: DealerEquityHotRouteV3; request: Uint8Array }> {
  const trading = address(2);
  const market = address(3);
  const root = address(4);
  const lpOwner = address(5);
  const release = new Uint8Array(32).fill(6);
  const obligation = PublicKey.findProgramAddressSync(
    [DEALER_OBLIGATION_PDA_DOMAIN_V3, new PublicKey(root).toBytes()], new PublicKey(trading))[0];
  const lp = PublicKey.findProgramAddressSync(
    [DEALER_LP_POSITION_PDA_DOMAIN_V3, new PublicKey(root).toBytes(), new PublicKey(lpOwner).toBytes()],
    new PublicKey(trading))[0];
  const request = new Uint8Array(DEALER_EQUITY_HEADER_BYTES_V3);
  request.set(new TextEncoder().encode('DCLMEQ03'), 0);
  new DataView(request.buffer).setUint16(8, 2, true);
  new DataView(request.buffer).setUint16(10, 1, true);
  new DataView(request.buffer).setUint32(12, 3, true);
  for (const [offset, value] of [
    [16, release], [48, new PublicKey(market).toBytes()], [80, new PublicKey(root).toBytes()],
    [112, lp.toBytes()], [144, new PublicKey(lpOwner).toBytes()], [176, obligation.toBytes()],
    [208, new Uint8Array(32).fill(10)], [240, new Uint8Array(32).fill(11)], [272, new Uint8Array(32).fill(12)],
    [304, new Uint8Array(32).fill(13)], [336, new Uint8Array(32).fill(14)], [368, new Uint8Array(32).fill(15)],
  ] as const) request.set(value, offset);
  for (const [offset, value] of [
    [400, 1n], [408, 2n], [416, 3n], [424, 4n], [432, 7n], [440, 1_100n], [448, 5n], [456, 25n], [464, 10n],
  ] as const) put64(request, offset, value);

  const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => meta(address(100 + index)));
  fixed[HOT_MARKET_ACCOUNT_V3] = meta(market);
  fixed[HOT_ROOT_ACCOUNT_V3] = meta(root, true);
  fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = meta(trading, false, true);
  fixed[HOT_RENT_SYSVAR_ACCOUNT_V3] = meta(SYSVAR_RENT_PUBKEY.toBase58());
  fixed[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = meta(SYSVAR_INSTRUCTIONS_PUBKEY.toBase58());
  const strategy = Array.from({ length: 10 }, (_, index) => meta(address(200 + index), false, index === 6));
  const runtime = Array.from({ length: 50 }, (_, index) => meta(address(300 + index), index === 48 || index === 49));
  const addresses = [...fixed, ...strategy, ...runtime].map((entry) => new PublicKey(entry.address));
  const table = new AddressLookupTableAccount({
    key: new PublicKey(address(900)),
    state: {
      deactivationSlot: 18_446_744_073_709_551_615n, lastExtendedSlot: 800,
      lastExtendedSlotStartIndex: 0, authority: undefined, addresses,
    },
  });
  return Object.freeze({
    request,
    route: Object.freeze({
      payer: address(901), tradingProgram: trading, market, releaseSet: release, generation: 7n,
      rootPrestateDigest: new Uint8Array(32).fill(17), observedSlot: 1_000n,
      fixedAccounts: Object.freeze(fixed), strategyAccounts: Object.freeze(strategy),
      runtimeAccounts: Object.freeze(runtime), recentBlockhash: address(902),
      lookupTables: Object.freeze([table]),
      outerEvidence: Object.freeze({
        status: 'checked' as const,
        tradingArtifactRelease: '18'.repeat(32),
        checkedManifestDigest: '19'.repeat(32),
      }),
    }),
  });
}
