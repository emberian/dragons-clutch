import { readFileSync } from 'node:fs';

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
  instructionMagicV1,
  magicIsAmbiguousV1,
  type CompiledProtocolInstructionV1,
} from '@dclutch/sdk/routeSelector';
import { INSTRUCTION_MAGICS, PREDICATE_SELECTED_ROUTES } from '@dclutch/sdk/generated/routeCensus';
import { CAPABILITY_ACTIONS_V1 } from '@dclutch/sdk/capabilityModel';
import {
  CORE_REQUEST_ACTION_OFFSET,
  compileCoreFoundTransactionV2,
  compileLifecycleRentCreateTransactionV2,
} from '@dclutch/sdk/coreFound';
import { compileDealerEquityTransactionV3, type DealerEquityHotRouteV3 } from '@dclutch/sdk/dealerEquityV3';
import {
  encodeDirectInlineOrdinaryRequestV3,
  type DirectHotAccountMetaV3,
  type SignedDirectIntentV3,
} from '@dclutch/sdk/directInlineV3';
import { encodeClaimsCustodyReplayRequestV1 } from '@dclutch/sdk/claimsCustodyReplay';
import { encodeWalletTerminalPayoutRequestV3 } from '@dclutch/sdk/walletTerminalPayoutV3';
import { compileRegistryReauthenticationTransaction, compileRegistryRoleActivationTransaction } from '@dclutch/sdk/releaseRegistry';
import { CORE_ACTION_FOUND_TAG, CORE_FOUND_ACCOUNT_COUNT_V3 } from '@dclutch/sdk/generated/coreFound';
import {
  DEALER_EQUITY_HEADER_BYTES_V3,
  DEALER_EQUITY_REQUEST_MAGIC_V3,
  DEALER_LP_POSITION_PDA_DOMAIN_V3,
  DEALER_OBLIGATION_PDA_DOMAIN_V3,
} from '@dclutch/sdk/generated/dealerEquityV3';
import { GENERAL_REQUEST_MAGIC_V3 } from '@dclutch/sdk/generated/generalSuccessorV5';
import {
  DIRECT_EXECUTION_REQUEST_MAGIC_V3,
  HOT_FAMILY_REQUEST_OFFSET_V3,
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_RENT_SYSVAR_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
} from '@dclutch/sdk/generated/directInlineV3';

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
 * WHEN ONE MAGIC SELECTS SEVERAL ROUTES the derivation's answer is a candidate
 * SET, and a declaration naming one member of it is a claim the leading bytes
 * do not settle. Two acts are in that position and they are treated alike: the
 * set is what the derivation returns, and the declaration is then either
 * checked against a SECOND coordinate the builder writes, or is empty. Core is
 * the first case -- eleven routes behind `DCLTCRQ2`, separated by an `Action`
 * tag `coreFound.ts` writes at a known offset -- and Rent's three lifecycle
 * arms are the second, separated by a variant this browser never writes.
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
 * Every route one program's dispatch selects for these leading bytes, read
 * from the census's own two tables.
 *
 * Read from `generated/routeCensus.ts` and not through `routeSelector.ts`,
 * which is the subject: an expectation the subject computed would agree with
 * it however wrong it was.
 */
function censusCandidatesV1(program: string, magic: string): ReadonlyArray<string> {
  return [...new Set([...INSTRUCTION_MAGICS, ...PREDICATE_SELECTED_ROUTES]
    .filter((entry) => entry.program === program && entry.magic === magic)
    .map((entry) => entry.routeId))].sort();
}

/**
 * The `variant X::Y` selectors the census publishes for each route.
 *
 * `INSTRUCTION_MAGICS` carries a magic and no selectors column, so what
 * separates eleven routes sharing one magic is not readable from the
 * TypeScript tables at all -- only from the census's own publication, which
 * `capabilityPhaseGate.test.ts` already reads the same way. OWED, and one line
 * of a generator: the inventory HAS these selectors and
 * `generate-route-census.mjs` keeps them for unselected entry routes while
 * dropping them for magic-selected ones. The day it carries them, this reader
 * is deleted and the two tests below read the table instead.
 */
const ROUTE_VARIANT_SELECTORS_V1 = new Map<string, ReadonlyArray<string>>(
  [...readFileSync(new URL('../../../docs/reference/routes.md', import.meta.url), 'utf8')
    .matchAll(/^\| `([^`]+)` \| (?:entry|action) \| ([^|]*)\|/gm)]
    .map(([, route, selectors]) => [route!, [...selectors!.matchAll(/variant `([^`]+)`/g)].map(([, name]) => name!)]),
);

/**
 * The Core `Action` variants this browser can encode, by the name the census
 * selects on.
 *
 * One row, because a Found is the only Core `Action` a browser builds. The TAG
 * comes from the generated ABI; only the NAME is written here, and only
 * because no generated module carries the discriminant's name -- Rust's
 * `ACTION_FOUND_TAG` reaches the browser as a number and the census publishes
 * `variant Action::Found` as text, so joining them needs the pair stated once.
 * A second copy of the number would be the defect; a name that no route
 * carries fails in `narrowedByActionTagV1` below rather than passing quietly.
 */
const CORE_ACTION_VARIANTS_V1: Readonly<Record<number, string>> = Object.freeze({
  [CORE_ACTION_FOUND_TAG]: 'Found',
});

/** The candidates whose published variant selector is the one these bytes carry. */
function narrowedByActionTagV1(candidates: ReadonlyArray<string>, request: Uint8Array): ReadonlyArray<string> {
  const variant = CORE_ACTION_VARIANTS_V1[request[CORE_REQUEST_ACTION_OFFSET]!];
  if (variant === undefined) return [];
  return candidates.filter((route) => (ROUTE_VARIANT_SELECTORS_V1.get(route) ?? []).includes(`Action::${variant}`));
}

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

describe('what one magic cannot decide alone, and the coordinate that decides it', () => {
  it('derives Core’s whole candidate set for a Found, because eleven routes share one magic', () => {
    // This assertion USED TO READ `toEqual([])`, beside a comment saying that
    // Core's request magic appeared in no census selector at all. That was
    // never a fact about Core's wire -- every Core instruction opens with
    // those eight bytes -- but about WHERE the check was written: inside
    // `Request::decode`, which the census's walk treats as terminal, rather
    // than in the dispatch it reads. With the guard moved, Core is the tree's
    // most AMBIGUOUS magic instead of its missing one, and an empty answer
    // here would now be a broken instrument reporting `no route`.
    const { core, compiled } = coreFoundFixture();
    const instructions = protocolInstructionsV1(compiled.transaction, { [core]: 'core' });
    expect(instructions).toHaveLength(1);
    const magic = instructionMagicV1(instructions[0]!.data);
    expect(magic).not.toBeNull();

    const derived = censusRouteIdsForInstructionsV1(instructions);
    expect(derived).toEqual(censusCandidatesV1('core', magic!));
    expect(magicIsAmbiguousV1('core', magic!)).toBe(true);
    // A set, not an answer: strictly more routes than the act declares.
    expect(derived.length).toBeGreaterThan(act('market.found').routes.length);
  });

  it('narrows that set by the Action tag the builder writes, which is what the act declares', () => {
    // The missing coordinate, derived the way the Hot family is derived below:
    // from the builder's own compiled bytes. The census separates these
    // candidates by a decoded `Action`; `coreFound.ts` writes that tag at
    // `CORE_REQUEST_ACTION_OFFSET`; so `market.found` declares the one
    // candidate whose published variant selector is the one its bytes carry,
    // and that is checked here rather than trusted.
    const { core, compiled } = coreFoundFixture();
    const request = protocolInstructionsV1(compiled.transaction, { [core]: 'core' })[0]!.data;
    expect(request[CORE_REQUEST_ACTION_OFFSET]).toBe(CORE_ACTION_FOUND_TAG);
    const candidates = censusCandidatesV1('core', instructionMagicV1(request)!);
    expect(narrowedByActionTagV1(candidates, request)).toEqual(act('market.found').routes);
  });

  it('does not narrow every Core variant to one route, so narrowing to one says something', () => {
    // The negative control, in both directions. A selector column this reader
    // parsed into nothing, or a filter that kept whatever it was handed, would
    // pass the case above and mean nothing.
    const { core, compiled } = coreFoundFixture();
    const request = protocolInstructionsV1(compiled.transaction, { [core]: 'core' })[0]!.data;
    const candidates = censusCandidatesV1('core', instructionMagicV1(request)!);

    const routesPerVariant = new Map<string, string[]>();
    for (const route of candidates) {
      for (const variant of ROUTE_VARIANT_SELECTORS_V1.get(route) ?? []) {
        routesPerVariant.set(variant, [...(routesPerVariant.get(variant) ?? []), route]);
      }
    }
    // The census parsed into something, and the tag is a NARROWING rather than
    // a bijection: some Core variant reaches several of these routes, and an
    // act declaring one of those would owe the set or a third coordinate.
    expect(routesPerVariant.size).toBeGreaterThan(0);
    expect([...routesPerVariant.values()].some((routes) => routes.length > 1)).toBe(true);

    // And a tag this browser has no name for narrows to NOTHING, never to
    // everything: an unnamed variant is an unanswered question.
    const unnamed = Uint8Array.from(request);
    unnamed[CORE_REQUEST_ACTION_OFFSET] = Math.max(...Object.keys(CORE_ACTION_VARIANTS_V1).map(Number)) + 1;
    expect(narrowedByActionTagV1(candidates, unnamed)).toEqual([]);
  });

  it('returns Rent’s whole candidate set for the RentCredit leg of a founding', () => {
    // The other ambiguous magic, and the other answer. Three lifecycle arms
    // separated by a decoded variant this browser never writes -- the packet
    // is a Create and nothing in these bytes says so at an offset the census
    // publishes -- so the founding's Rent leg is a set with no second
    // coordinate, and `market.found` declares no Rent route at all. Declaring
    // one of the three would publish two gates the act never reaches.
    const rent = address(41);
    const compiled = compileLifecycleRentCreateTransactionV2({
      payer: address(40), refundWallet: address(42), market: address(43),
      releaseSet: new Uint8Array(32).fill(5), generation: 7n, rentProgram: rent,
      recentBlockhash: address(44),
    });
    const instructions = protocolInstructionsV1(compiled.transaction, { [rent]: 'rent' });
    const magic = instructionMagicV1(instructions[0]!.data);
    const derived = censusRouteIdsForInstructionsV1(instructions);
    expect(derived).toEqual(censusCandidatesV1('rent', magic!));
    expect(derived).toEqual([
      'rent/process_close_v2#Close',
      'rent/process_create_v2#Create',
      'rent/process_sweep_v2#Sweep',
    ]);
    expect(magicIsAmbiguousV1('rent', magic!)).toBe(true);
    expect(act('market.found').routes.filter((route) => derived.includes(route))).toEqual([]);
  });
});

/** The acts whose instruction bytes are authored outside this browser. */
const PLANNER_AUTHORED_V1: Readonly<Record<string, string>> = Object.freeze({
  'market.join': 'crates/dclutch-user-position-admission-wasm',
  'source.create-fund': 'crates/dclutch-operator/src/source_readiness.rs',
  'source.ready': 'crates/dclutch-operator/src/source_readiness.rs',
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

/**
 * WHICH FAMILY an act's Hot request belongs to, from the bytes it writes.
 *
 * `trading/hot_v3::process_hot_execution_v3` is ONE route and four families:
 * Direct, General, Dealer and Series all arrive on `DCLTHOT3`, and each
 * family's prelude returns a non-error for every request that is not its own
 * before it reads anything. So the route is not enough to decide what a given
 * act is subject to -- five acts declare it, and the Direct root's `Open` set
 * behind `hot_v3::prepare_direct_inline_hot_crosscheck_v3` binds exactly one
 * of them. The family is the missing coordinate, and it is derived the same
 * way the route is: from the builder's own compiled bytes.
 *
 * WHERE THE DISCRIMINANT IS. `HotExecutionEnvelopeV3::split_instruction` cuts
 * the instruction at `HOT_FAMILY_REQUEST_OFFSET_V3`, and everything past that
 * is the family request, which opens with its own codec's magic. Nothing below
 * writes a magic: each is imported from the generated ABI module of the crate
 * that owns that family's wire, so a family renamed in Rust reaches this
 * derivation by regeneration.
 */
const FAMILY_REQUEST_MAGICS_V1: Readonly<Record<string, Uint8Array>> = Object.freeze({
  // Series is absent on purpose and it is not an oversight: no browser module
  // encodes or decodes `DCLTSIX3`, because no act offers a Series Expire. It
  // is stated as an empty case below rather than left to look covered.
  Direct: DIRECT_EXECUTION_REQUEST_MAGIC_V3,
  Dealer: DEALER_EQUITY_REQUEST_MAGIC_V3,
  General: GENERAL_REQUEST_MAGIC_V3,
});

const HOT_ROUTE_V1 = 'trading/hot_v3::process_hot_execution_v3';

/** The family whose codec owns the magic these request bytes open with. */
function familyOfRequestV1(request: Uint8Array): string | null {
  for (const [family, magic] of Object.entries(FAMILY_REQUEST_MAGICS_V1)) {
    if (magic.every((byte, index) => request[index] === byte)) return family;
  }
  return null;
}

function directIntent(seed: number, side: 0 | 1): SignedDirectIntentV3 {
  return Object.freeze({
    maker: address(seed),
    signature: new Uint8Array(64).fill(seed),
    intent: Object.freeze({
      side, lifecycle: 0 as const, outcome: 0, market: address(3), generation: 7n,
      nonce: BigInt(seed), validFrom: 1n, validThrough: 1_000n,
      maximumFill: 10n, limitPrice: 5n, feeBasisPoints: 0,
      collateralAccount: address(seed + 60),
    }),
  });
}

describe('an act declares the family its own request bytes belong to', () => {
  it('dealer.liquidity is the Dealer family, split out of the envelope it compiles', async () => {
    const value = dealerFixture();
    const plan = await compileDealerEquityTransactionV3(value.route, value.request);
    const [hot] = protocolInstructionsV1(plan.transaction, { [value.route.tradingProgram]: 'trading' });
    expect(hot).toBeDefined();
    // The same cut the program makes: envelope, then family request. The
    // offset is the generated one, so a wider envelope moves both together.
    const family = familyOfRequestV1(hot!.data.subarray(HOT_FAMILY_REQUEST_OFFSET_V3));
    expect(family).toBe('Dealer');
    expect(act('dealer.liquidity').families).toEqual([family]);
  });

  it('direct.inline is the Direct family, from the request its builder authors', () => {
    // `compileDirectInlineTransactionV3` needs a whole checked hot route --
    // two signed intents, a Profile14 account profile, a canonical lookup
    // table -- and none of that decides the family. The family request does,
    // and this browser AUTHORS it: `validateDirectInlineInstructionSequenceV3`
    // then refuses any sequence whose bytes at the same offset are not these.
    const request = encodeDirectInlineOrdinaryRequestV3(directIntent(11, 0), directIntent(12, 1), 4n, 5n);
    const family = familyOfRequestV1(request);
    expect(family).toBe('Direct');
    expect(act('direct.inline').families).toEqual([family]);
  });

  it('tells the two families apart, so deriving one proves something', () => {
    // The negative control. A `familyOfRequestV1` that returned the first key
    // whatever the bytes would pass both cases above.
    const dealer = dealerFixture();
    expect(familyOfRequestV1(dealer.request)).toBe('Dealer');
    expect(familyOfRequestV1(encodeDirectInlineOrdinaryRequestV3(directIntent(11, 0), directIntent(12, 1), 4n, 5n)))
      .not.toBe('Dealer');
    expect(familyOfRequestV1(new Uint8Array(64))).toBeNull();
  });

  it('pins which acts derived a family and which declare none', () => {
    // Neither list may move silently. An act that gains a family gains a
    // compile above, and an act that gains the Hot route without one has to
    // appear in `PLANNER_AUTHORED_V1` and say who wrote its bytes.
    expect(CAPABILITY_ACTIONS_V1.filter((one) => one.families.length > 0).map((one) => one.id))
      .toEqual(['direct.inline', 'dealer.liquidity']);
    expect(CAPABILITY_ACTIONS_V1
      .filter((one) => one.routes.includes(HOT_ROUTE_V1) && one.families.length === 0)
      .map((one) => one.id))
      .toEqual(['general.consider', 'general.settle', 'general.close']);
    for (const one of CAPABILITY_ACTIONS_V1) {
      if (one.families.length > 0 || !one.routes.includes(HOT_ROUTE_V1)) continue;
      expect(one.id in PLANNER_AUTHORED_V1, `${one.id} declares the Hot route, no family, and no author`).toBe(true);
    }
  });

  it('claims no General family, though the browser holds that magic', () => {
    // The empty case that is easiest to get wrong. `generalPlanV5.ts` DECODES
    // `DCGREQ03` and would recognise a General request instantly -- but the
    // three General acts hand back a transaction a reader pasted in, so this
    // browser compiles no General request and derives no General family from a
    // fixture's placeholder bytes. The magic being available is not the same
    // fact as an act's own builder emitting it.
    expect(FAMILY_REQUEST_MAGICS_V1.General).toBeDefined();
    for (const id of ['general.consider', 'general.settle', 'general.close']) {
      expect(act(id).families).toEqual([]);
      expect(act(id).routes).toContain(HOT_ROUTE_V1);
    }
  });

  it('claims no Series family, because no act offers a Series Expire', () => {
    // Said out loud rather than left as an absence: the second selected gate
    // the census publishes (`series-ticket: Prepared`) is answered for NOBODY
    // today, and will be the moment an act declares this family.
    expect(CAPABILITY_ACTIONS_V1.some((one) => one.families.includes('Series'))).toBe(false);
    expect(Object.keys(FAMILY_REQUEST_MAGICS_V1)).not.toContain('Series');
  });

  it('gives every act off the Hot route an empty family by construction', () => {
    for (const one of CAPABILITY_ACTIONS_V1) {
      if (one.routes.includes(HOT_ROUTE_V1)) continue;
      expect(one.families, `${one.id} declares a Hot family and no Hot route`).toEqual([]);
    }
  });
});

// ---------------------------------------------------------------- fixtures

/** One Core Found, compiled by the builder `/found` runs. */
function coreFoundFixture() {
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
  return { core, compiled };
}

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
