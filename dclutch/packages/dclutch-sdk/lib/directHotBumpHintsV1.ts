import { PublicKey } from '@solana/web3.js';

import { slice, u16 } from './bytes';
import {
  HOT_BUMP_HINT_COUNT_V1,
  HOT_BUMP_HINT_SLOT_NAMES_V1,
} from './generated/hotBumpHintSlotsV1';
import { DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1 } from './directMakerReplay';
import { activatedRoleProgramV1 } from './releaseRegistry';
import {
  CALLER_AUTHORITY_PDA_DOMAIN_V1,
  CUSTODY_REPLAY_PDA_DOMAIN_V1,
  EXECUTION_ROLE_TRADING_V1,
} from './generated/claimsCustodyReplayV1';
import {
  CORE_STATE_BYTES,
  CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_PRODUCT_ID_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RESOLUTION_POLICY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
  MARKET_CORE_STATE_PDA_DOMAIN_V2,
} from './generated/coreFound';
import {
  CAPABILITY_EXECUTION_SELECTION_BYTES_V1,
  CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_MAGIC_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_MAGIC_V1,
  CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_PROFILE_V1,
  CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET,
  CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_V1,
  CAPABILITY_ROOT_GENERATION_OFFSET,
  CAPABILITY_ROOT_HEADER_BYTES_V1,
  CAPABILITY_ROOT_MARKET_OFFSET,
  CAPABILITY_ROOT_PDA_DOMAIN_V1,
  CAPABILITY_ROOT_SELECTION_OFFSET,
} from './generated/directInlineV3';

/**
 * The eight caller-mined bump hints the V3 hot envelope carries, and the
 * browser-side miner that fills them.
 *
 * Mirrors `HotBumpHintsV1` and `direct_inline_hot_bump_hints_v1` in
 * `crates/dclutch-market/src/capability_program/hot_v3.rs` and
 * `crates/dclutch-operator/src/direct_inline_v3.rs`.
 *
 * # Why a browser should mine them
 *
 * A PDA bump is `Geometric(1/2)` in the participant key and
 * `find_program_address` costs the PROGRAM 1,500 CU per rejected candidate. A
 * route that still searches therefore has no compute ceiling: its cost is a
 * property of whose key is trading, and a fraction of strangers draw deep
 * enough to be refused for a reason they can neither see nor fix. Off chain the
 * same search is free, so mining here converts a per-key lottery into a
 * constant. The unhinted wire is still correct -- zero means absent and the
 * program searches exactly as it used to -- it is only more expensive, and its
 * expense varies with the trader.
 *
 * # Why a hint is never an authority
 *
 * Trading rebuilds each address's seeds ITSELF, reproduces the address with
 * `create_program_address`, and refuses unless the result equals the account
 * its frame was handed. A wrong hint names a different address and the route
 * refuses; the derivation IS the check. Nothing here can steer an execution,
 * which is why taking these eight bytes from a stranger is safe.
 *
 * # Which slots this miner fills
 *
 * Six of eight, and the two it leaves are the same two the Rust builder leaves.
 * `child_caller` seeds end in a digest over each child's PROJECTED request,
 * which only the selected Transition/Effect interpreters can produce;
 * `build_direct_inline_hot_v4` therefore takes those two bytes as a PARAMETER
 * from `derive_direct_inline_child_authorities_v3` rather than deriving them,
 * and so does this function. A caller with no projection passes `[0, 0]`, which
 * is correct and merely slower.
 */

/**
 * The block's geometry and its slot names, from the Rust that owns each.
 *
 * All three were spelled out here, in both twins, until 2026-09-03 -- and the
 * names had drifted. This file glossed the family-neutral slots with Direct
 * account names ("lifecycle 0 (seller maker replay)") three lines under a
 * comment saying the slots are roles in any hot execution and never Direct
 * accounts, so a Rational or Dealer reader shown one of those names was being
 * told something false about the route in front of it. The producer,
 * `dclutch-hot-bump-miner-v1`, names them `lifecycle[0]` and the rest, and it
 * is the authority for every host-side and browser-side miner there is.
 *
 * Re-exported rather than imported-and-forwarded by hand so nothing that reads
 * these from this module has to move.
 */
export {
  HOT_BUMP_HINTS_OFFSET_V1,
  HOT_BUMP_HINT_COUNT_V1,
  HOT_BUMP_HINT_SLOT_NAMES_V1,
  hotBumpHintSlotNameV1,
} from './generated/hotBumpHintSlotsV1';

export type HotBumpHintsV1 = Readonly<{
  /** Core Market state PDA bump. Relayed to every child that reads the Market. */
  market: number;
  /** Trading capability root PDA bump. */
  root: number;
  /** Lifecycle-created account bumps, in lifecycle materialization order. */
  lifecycle: readonly [number, number];
  /** Trading caller-authority bumps, in child-route order. */
  childCaller: readonly [number, number];
  /** Bumps a child derives internally, relayed in its request, in route order. */
  childRelay: readonly [number, number];
}>;

/** The all-zero block: every reader searches, exactly as it used to. */
export const HOT_BUMP_HINTS_ABSENT_V1: HotBumpHintsV1 = Object.freeze({
  market: 0,
  root: 0,
  lifecycle: Object.freeze([0, 0] as const),
  childCaller: Object.freeze([0, 0] as const),
  childRelay: Object.freeze([0, 0] as const),
});

function exactBump(value: number, field: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 255) {
    throw new Error(`${field} bump hint is not one byte`);
  }
  return value;
}

/** Write the block in canonical slot order. */
export function encodeHotBumpHintsV1(hints: HotBumpHintsV1): Uint8Array {
  const slots = [
    hints.market,
    hints.root,
    hints.lifecycle[0],
    hints.lifecycle[1],
    hints.childCaller[0],
    hints.childCaller[1],
    hints.childRelay[0],
    hints.childRelay[1],
  ];
  if (slots.length !== HOT_BUMP_HINT_COUNT_V1) throw new Error('hot bump hint block is not eight slots');
  return Uint8Array.from(slots.map((value, index) => exactBump(value, HOT_BUMP_HINT_SLOT_NAMES_V1[index] as string)));
}

/** Read the block in canonical slot order. */
export function decodeHotBumpHintsV1(bytes: Uint8Array): HotBumpHintsV1 {
  if (bytes.length !== HOT_BUMP_HINT_COUNT_V1) {
    throw new Error(`hot bump hint block is ${bytes.length} bytes, not the exact ${HOT_BUMP_HINT_COUNT_V1}`);
  }
  const at = (index: number): number => exactBump(bytes[index] as number, HOT_BUMP_HINT_SLOT_NAMES_V1[index] as string);
  return Object.freeze({
    market: at(0),
    root: at(1),
    lifecycle: Object.freeze([at(2), at(3)] as const),
    childCaller: Object.freeze([at(4), at(5)] as const),
    childRelay: Object.freeze([at(6), at(7)] as const),
  });
}

/** Whether no hint is set, so every reader on this execution searches. */
export function hotBumpHintsAreAbsentV1(hints: HotBumpHintsV1): boolean {
  return encodeHotBumpHintsV1(hints).every((value) => value === 0);
}

/**
 * The finalized bytes a caller already read, kept rather than thrown away.
 *
 * Exactly the inputs `direct_inline_hot_bump_hints_v1` mines from: the Core
 * Market state, the Trading capability-root header, and the Registry activation
 * cache that names the release set's Custody deployment. A route inspection has
 * all three in hand and currently discards them, which is the only reason the
 * browser could not mine.
 */
export type DirectHotBumpHintSourceV3 = Readonly<{
  coreProgram: string;
  marketCoreState: Uint8Array;
  capabilityRootHeader: Uint8Array;
  activationCache: Uint8Array;
}>;

export type DirectInlineBumpHintInputV3 = Readonly<{
  source: DirectHotBumpHintSourceV3;
  tradingProgram: string;
  market: string;
  generation: bigint;
  releaseSet: Uint8Array;
  sellerMaker: string;
  buyerMaker: string;
  /**
   * The two child caller-authority bumps, Claims then Custody.
   *
   * Their seeds end in a digest over each child's projected request, which no
   * exterior caller rebuilds; the Rust builder takes the same two bytes as a
   * parameter for the same reason. `[0, 0]` searches.
   */
  childCaller?: readonly [number, number];
  /**
   * The two maker-replay accounts the transaction's runtime frame already
   * carries, seller then buyer.
   *
   * Supplying them turns mining into a JOINT check rather than a second opinion
   * nobody compares: the frame's addresses were derived by the replay reader
   * from chain state, these are derived here from seeds, and a disagreement
   * means the hint would name an account the frame does not carry -- which
   * Trading refuses on chain, one round trip and one signature later.
   */
  expectedLifecycleAccounts?: readonly [string, string];
}>;

function exactKey(value: string, field: string): PublicKey {
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

function exactGeneration(generation: bigint): bigint {
  if (generation < 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('Market generation is outside u64');
  return generation;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

/**
 * The eight seeds the Trading capability root's own immutable header declares.
 *
 * The header is read rather than reconstructed for the same reason the operator
 * reads it: every seed past the Market is an activation-time selection the
 * caller has no other authority over, and a caller that could choose them could
 * name another Market's root.
 *
 * INERT NOTE FOR THE MARKET SITE, which has no equivalent here: on any Market
 * whose creator recorded its own bump the hint is never read -- the on-chain
 * reader spells its precedence `state.bumps.market.or(hint)`, so the record
 * outranks the wire and a caller cannot steer a Market that already knows its
 * own address. It is mined anyway because a pre-tail Market has no record and
 * still searches, and because a slot left zero on a route that could fill it is
 * indistinguishable from a slot nobody thought about.
 */
export function capabilityRootSeedsV1(capabilityRootHeader: Uint8Array): ReadonlyArray<Uint8Array> {
  if (capabilityRootHeader.length < CAPABILITY_ROOT_HEADER_BYTES_V1) {
    throw new Error('Trading capability root is shorter than its exact immutable header');
  }
  const selection = slice(capabilityRootHeader, CAPABILITY_ROOT_SELECTION_OFFSET, CAPABILITY_EXECUTION_SELECTION_BYTES_V1);
  if (!same(slice(selection, CAPABILITY_EXECUTION_SELECTION_MAGIC_OFFSET, 8), CAPABILITY_EXECUTION_SELECTION_MAGIC_V1)
      || u16(selection, CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET) !== CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_V1
      || u16(selection, CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET) !== CAPABILITY_EXECUTION_SELECTION_PROFILE_V1) {
    throw new Error('capability root selection has the wrong canonical header');
  }
  const entryIndex = new Uint8Array(2);
  new DataView(entryIndex.buffer).setUint16(0, u16(selection, CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET), true);
  return Object.freeze([
    CAPABILITY_ROOT_PDA_DOMAIN_V1,
    slice(capabilityRootHeader, CAPABILITY_ROOT_MARKET_OFFSET, 32),
    slice(capabilityRootHeader, CAPABILITY_ROOT_GENERATION_OFFSET, 8),
    slice(selection, CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, 32),
    entryIndex,
    slice(selection, CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET, 32),
    slice(selection, CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET, 32),
    slice(selection, CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET, 32),
  ]);
}

/**
 * The Custody deployment this Market's release set activated.
 *
 * Custody is not in the hot fixed frame; the Registry activation cache is, and
 * it names every role's current program. `direct_inline_hot_bump_hints_v1`
 * reaches the same program the same way.
 */
export function custodyProgramFromActivationCacheV1(activationCache: Uint8Array): string {
  return activatedRoleProgramV1(activationCache, 'custody');
}

/** One address this route reproduces from a hint, with the seeds it is built from. */
export type HotBumpHintSeedSiteV1 = Readonly<{
  /** Which of the eight wire slots carries this site's bump. */
  slot: number;
  /** The slot's canonical name, for a failure that names one. */
  name: string;
  /** The ordered seeds, excluding the bump the site's slot carries. */
  seeds: ReadonlyArray<Uint8Array>;
  /** The program the seeds are interpreted under. */
  programId: string;
  /** The address `find_program_address` reached, and its canonical bump. */
  address: string;
  bump: number;
}>;

/**
 * The seed order of every address a Direct InlineOrdinary caller can mine,
 * stated exactly once.
 *
 * The miner below maps over this, and the adversarial tests perturb it, so a
 * seed order that moved cannot move in one place and not the other. Six sites,
 * not eight: the two `child_caller` slots are supplied rather than derived --
 * their seeds end in a digest over each child's projected request.
 *
 * The seller replay comes before the buyer's because that is the order the
 * StateLifecyclePolicy materializes them in, and a slot IS its position in that
 * order. A swap is not a hazard, only a slower trade: the program reproduces
 * each slot against the account its frame supplies and a wrong one refuses.
 *
 * The two Custody relay sites are addresses Custody derives for ITSELF and can
 * carry from nowhere -- its replay, whose context is the buyer's maker-replay
 * root, and its transfer authority. Neither can be stored: the replay body is
 * exactly packed and its own bump would have to be written before the account
 * exists. Mined here and relayed inside the Custody child request.
 */
export function directInlineHotBumpSeedSitesV3(
  input: DirectInlineBumpHintInputV3,
): ReadonlyArray<HotBumpHintSeedSiteV1> {
  const trading = exactKey(input.tradingProgram, 'Trading program');
  const market = exactKey(input.market, 'Market');
  const seller = exactKey(input.sellerMaker, 'seller maker');
  const buyer = exactKey(input.buyerMaker, 'buyer maker');
  if (input.releaseSet.length !== 32) throw new Error('execution release set must be one 32-byte identity');
  if (new Set([trading.toBase58(), market.toBase58(), seller.toBase58(), buyer.toBase58()]).size !== 4) {
    throw new Error('Trading program, Market, and the two maker identities must not alias');
  }
  const custody = exactKey(
    custodyProgramFromActivationCacheV1(input.source.activationCache),
    'activated Custody program',
  );
  if (input.source.marketCoreState.length !== CORE_STATE_BYTES) {
    throw new Error(`Core Market state is ${input.source.marketCoreState.length} bytes, not the exact ${CORE_STATE_BYTES}`);
  }
  const state = input.source.marketCoreState;
  const generation = new Uint8Array(8);
  new DataView(generation.buffer).setBigUint64(0, exactGeneration(input.generation), true);

  const site = (slot: number, seeds: ReadonlyArray<Uint8Array>, programId: PublicKey): HotBumpHintSeedSiteV1 => {
    const [address, bump] = PublicKey.findProgramAddressSync([...seeds] as Uint8Array[], programId);
    return Object.freeze({
      slot,
      name: HOT_BUMP_HINT_SLOT_NAMES_V1[slot] as string,
      seeds: Object.freeze(seeds.map((seed) => new Uint8Array(seed))),
      programId: programId.toBase58(),
      address: address.toBase58(),
      bump: exactBump(bump, HOT_BUMP_HINT_SLOT_NAMES_V1[slot] as string),
    });
  };

  const replay = (slot: number, maker: PublicKey) => site(slot, [
    DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1, market.toBytes(), generation, maker.toBytes(),
  ], trading);
  const buyerMakerRoot = replay(3, buyer);

  return Object.freeze([
    site(0, [
      MARKET_CORE_STATE_PDA_DOMAIN_V2,
      slice(state, CORE_STATE_IDENTITY_REALM_OFFSET, 32),
      slice(state, CORE_STATE_PRODUCT_RECORD_OFFSET, 32),
      slice(state, CORE_STATE_PRODUCT_ID_OFFSET, 32),
      slice(state, CORE_STATE_RESOLUTION_POLICY_OFFSET, 32),
      slice(state, CORE_STATE_CAPABILITY_MANIFEST_OFFSET, 32),
      slice(state, CORE_STATE_SELECTED_RELEASE_SET_OFFSET, 32),
      slice(state, CORE_STATE_REGISTRY_PROGRAM_OFFSET, 32),
      slice(state, CORE_STATE_GENERATION_OFFSET, 8),
    ], exactKey(input.source.coreProgram, 'Core program')),
    site(1, capabilityRootSeedsV1(input.source.capabilityRootHeader), trading),
    replay(2, seller),
    buyerMakerRoot,
    site(6, [
      CUSTODY_REPLAY_PDA_DOMAIN_V1,
      market.toBytes(),
      input.releaseSet,
      Uint8Array.of(EXECUTION_ROLE_TRADING_V1),
      new PublicKey(buyerMakerRoot.address).toBytes(),
    ], custody),
    site(7, [CUSTODY_AUTHORITY_PDA_DOMAIN_V1, market.toBytes(), input.releaseSet], custody),
  ]);
}

/**
 * Mine every bump an off-chain InlineOrdinary caller can mine.
 *
 * Six of eight slots. The two `child_caller` bumps come in as a parameter for
 * the same reason `build_direct_inline_hot_v4` takes them as one: their seeds
 * end in a digest over each child's PROJECTED request, which only the selected
 * Transition and Effect interpreters can produce. `[0, 0]` searches.
 */
export function mineDirectInlineHotBumpHintsV3(input: DirectInlineBumpHintInputV3): HotBumpHintsV1 {
  const sites = new Map(directInlineHotBumpSeedSitesV3(input).map((entry) => [entry.slot, entry]));
  const bump = (slot: number): number => {
    const entry = sites.get(slot);
    if (entry === undefined) throw new Error(`slot ${slot} has no seed site to mine`);
    return entry.bump;
  };
  if (input.expectedLifecycleAccounts !== undefined) {
    for (const [slot, field] of [[2, 'seller'], [3, 'buyer']] as const) {
      if (sites.get(slot)?.address !== input.expectedLifecycleAccounts[slot - 2]) {
        throw new Error(`mined ${field} maker replay hint names another account than the route frame carries`);
      }
    }
  }
  const childCaller = input.childCaller ?? ([0, 0] as const);
  return Object.freeze({
    market: bump(0),
    root: bump(1),
    lifecycle: Object.freeze([bump(2), bump(3)] as const),
    childCaller: Object.freeze([
      exactBump(childCaller[0], 'Claims caller authority'),
      exactBump(childCaller[1], 'Custody caller authority'),
    ] as const),
    childRelay: Object.freeze([bump(6), bump(7)] as const),
  });
}

/**
 * The Trading caller-authority bump for one already-projected child request.
 *
 * Exported so a caller that DOES project its children -- the operator, a test
 * vector, a relayer that runs the interpreters -- can fill `childCaller`
 * without restating the seed order. The context is the parent request digest
 * for the Claims route and the buyer's maker-replay root for every Custody
 * route, exactly as `derive_child_authorities` spells it.
 */
export function mineChildCallerAuthorityBumpV1(
  tradingProgram: string,
  releaseSet: Uint8Array,
  market: string,
  context: Uint8Array,
  childRequestDigest: Uint8Array,
): number {
  for (const [value, field] of [[releaseSet, 'release set'], [context, 'caller context'], [childRequestDigest, 'child request digest']] as const) {
    if (value.length !== 32) throw new Error(`caller authority ${field} must be one 32-byte identity`);
  }
  return PublicKey.findProgramAddressSync([
    CALLER_AUTHORITY_PDA_DOMAIN_V1,
    releaseSet,
    exactKey(market, 'Market').toBytes(),
    Uint8Array.of(EXECUTION_ROLE_TRADING_V1),
    context,
    childRequestDigest,
  ], exactKey(tradingProgram, 'Trading program'))[1];
}
