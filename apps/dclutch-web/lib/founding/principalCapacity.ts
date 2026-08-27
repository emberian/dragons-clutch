/**
 * κ — the founding capacity bound, computed in the browser.
 *
 * The predicate is `total_principal ≤ κ · manipulation_cost_lower_bound`, where
 * the lower bound is the venue's observed manipulation floor at founding. It is
 * evaluated cross-multiplied — `principal · denominator ≤ numerator · floor` —
 * so there is no division and no rounding anywhere in it, and the answer is
 * exact rather than conservative.
 *
 * WHAT THIS IS NOT. **No on-chain route calls this predicate.** It is proven in
 * `formal/dclutch-semantics/SourcePrincipalCapacityV1.lean` (`admit_sound`,
 * `admit_complete`, `overflow_is_exact`) and implemented in
 * `crates/dclutch-source-contract/src/principal_capacity_v1.rs`, and its only
 * non-test caller in the whole tree is the off-chain gauntlet driver. Found
 * sees the Source and not the principal; Claims FoundingV5 sees the reverse.
 * A wizard that showed this verdict as "the chain will refuse" would be lying,
 * so `PrincipalCapacityVerdictV1.enforcement` carries the truth instead, and
 * the copy that renders it must keep saying so until WAVE.md's κ-enforcement
 * row lands the cap on the Market root.
 *
 * The second, subtler half of that gap, recorded here because a UI is exactly
 * where someone would forget it: even once wired, **a founding-only check is
 * not a cap**, because principal grows on every complete-set split. The number
 * below bounds what a founding may open with, not what a Market may hold.
 */

import { ascii, hex, isZero, requireZero, slice, u16, u64 } from '../bytes';
import {
  BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
  CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
  CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
  MANIPULATION_FLOOR_V1_ADAPTER_CONFIG_OFFSET,
  MANIPULATION_FLOOR_V1_BASIS_OFFSET,
  MANIPULATION_FLOOR_V1_BYTES,
  MANIPULATION_FLOOR_V1_COLLATERAL_UNIT_OFFSET,
  MANIPULATION_FLOOR_V1_CURVE_DERIVED_TAG,
  MANIPULATION_FLOOR_V1_DERIVATION_RELEASE_OFFSET,
  MANIPULATION_FLOOR_V1_FLOOR_ATOMS_OFFSET,
  MANIPULATION_FLOOR_V1_MAGIC,
  MANIPULATION_FLOOR_V1_MAGIC_OFFSET,
  MANIPULATION_FLOOR_V1_OBSERVED_DEPTH_TAG,
  MANIPULATION_FLOOR_V1_RESERVED_BYTES,
  MANIPULATION_FLOOR_V1_RESERVED_OFFSET,
  MANIPULATION_FLOOR_V1_SCHEMA_VERSION,
  MANIPULATION_FLOOR_V1_SOURCE_SPEC_OFFSET,
  MANIPULATION_FLOOR_V1_TAIL_RESERVED_BYTES,
  MANIPULATION_FLOOR_V1_TAIL_RESERVED_OFFSET,
  MANIPULATION_FLOOR_V1_VERSION_OFFSET,
} from '../generated/principalCapacityV1';

const MAX_U32 = 0xffff_ffffn;
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_U128 = (1n << 128n) - 1n;

/** κ as the Source states it: a `u32 / u32` ratio, or nothing at all. */
export type PrincipalCapacityV1 =
  | Readonly<{ kind: 'unstated' }>
  | Readonly<{ kind: 'bounded'; numerator: bigint; denominator: bigint }>;

export const DEFAULT_CHAIN_STATE_CAPACITY_V1: PrincipalCapacityV1 = Object.freeze({
  kind: 'bounded',
  numerator: BigInt(CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1),
  denominator: BigInt(CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1),
});

export const DEFAULT_VENUE_FLOOR_LAMPORTS_V1 = BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1;

/**
 * The refusal names `principal_capacity_v1.rs` raises, kept distinguishable.
 *
 * The Rust returns a plain enum with no numeric discriminants, so these never
 * reach a wire and cost nothing to name. An operator who is over the bound and
 * an operator whose Source never stated κ have completely different next
 * actions, and collapsing them into "refused" would hide that.
 */
export type PrincipalCapacityRefusalV1 =
  | 'PrincipalCapacityUnstated'
  | 'NonCanonicalCapacity'
  | 'ZeroCapacity'
  | 'PrincipalExceedsCapacity';

export type PrincipalCapacityVerdictV1 = Readonly<{
  admitted: boolean;
  refusal: PrincipalCapacityRefusalV1 | null;
  /** `numerator · floor`, the right-hand side, or null when κ is unstated. */
  bound: bigint | null;
  /** `principal · denominator`, the left-hand side, or null when κ is unstated. */
  scaled: bigint | null;
  /** The largest principal this κ and floor admit, or null when none is. */
  largestAdmittedPrincipal: bigint | null;
  /**
   * Whether a chain refuses over this bound today. It does not.
   *
   * Kept as a field rather than a comment so no caller can render the verdict
   * without having had the chance to render this beside it.
   */
  enforcement: 'off-chain-only';
}>;

function u32(value: bigint, field: string): bigint {
  if (typeof value !== 'bigint' || value < 0n || value > MAX_U32) throw new Error(`${field} is outside u32`);
  return value;
}

function refuse(refusal: PrincipalCapacityRefusalV1, bound: bigint | null, scaled: bigint | null, largest: bigint | null): PrincipalCapacityVerdictV1 {
  return Object.freeze({ admitted: false, refusal, bound, scaled, largestAdmittedPrincipal: largest, enforcement: 'off-chain-only' });
}

/**
 * Decide the founding predicate against one venue floor.
 *
 * Mirrors `PrincipalCapacityV1::admit` arm for arm, including the asymmetry in
 * its two overflow paths: the right-hand side is `u32 × u64` and cannot exceed
 * `u128`, so its overflow is unreachable, while a left-hand side that overflows
 * `u128` is a genuine *refusal* rather than an error — the Lean's
 * `overflow_is_exact` proves the bound stays below `2^96`, so anything that
 * overflows really is larger. JavaScript BigInt does not overflow, so the
 * width has to be checked rather than observed.
 */
export function admitPrincipalCapacityV1(
  capacity: PrincipalCapacityV1,
  floorAtoms: bigint,
  totalPrincipalAtoms: bigint,
): PrincipalCapacityVerdictV1 {
  if (typeof floorAtoms !== 'bigint' || floorAtoms < 0n || floorAtoms > MAX_U64) throw new Error('venue floor is outside u64');
  if (typeof totalPrincipalAtoms !== 'bigint' || totalPrincipalAtoms < 0n || totalPrincipalAtoms > MAX_U128) throw new Error('total principal is outside u128');
  if (capacity.kind !== 'bounded') return refuse('PrincipalCapacityUnstated', null, null, null);
  u32(capacity.numerator, 'kappa numerator');
  u32(capacity.denominator, 'kappa denominator');
  if (capacity.denominator === 0n) return refuse('NonCanonicalCapacity', null, null, null);

  const bound = capacity.numerator * floorAtoms;
  const largest = bound === 0n ? null : bound / capacity.denominator;
  if (totalPrincipalAtoms === 0n) return refuse('ZeroCapacity', bound, 0n, largest);
  if (bound === 0n) return refuse('PrincipalExceedsCapacity', bound, null, null);

  const scaled = totalPrincipalAtoms * capacity.denominator;
  if (scaled > MAX_U128 || scaled > bound) return refuse('PrincipalExceedsCapacity', bound, scaled > MAX_U128 ? null : scaled, largest);
  return Object.freeze({ admitted: true, refusal: null, bound, scaled, largestAdmittedPrincipal: largest, enforcement: 'off-chain-only' });
}

/**
 * The largest principal a κ and floor admit, for a UI that wants a headroom bar.
 *
 * This is `floor(numerator · floor / denominator)` and it is only ever a
 * *display* of the predicate above; nothing decides admission from it, because
 * a division introduced for presentation must not become the thing that
 * answers the question.
 */
export function largestAdmittedPrincipalV1(capacity: PrincipalCapacityV1, floorAtoms: bigint): bigint | null {
  return admitPrincipalCapacityV1(capacity, floorAtoms, 1n).largestAdmittedPrincipal;
}

/** Render κ as the ratio an operator reads, without ever dividing to decide. */
export function formatCapacityV1(capacity: PrincipalCapacityV1): string {
  return capacity.kind === 'bounded' ? `${capacity.numerator}/${capacity.denominator}` : 'unstated';
}


/**
 * A venue's derived cost floor for forcing the observation a Market resolves on.
 *
 * The record carries no Market, no generation and no principal: it is the same
 * immutable derivation for every Market founded against that Source. What it
 * does carry is three identities, and they are the whole reason the record is
 * worth authenticating rather than trusting — a floor derived for a different
 * Source, a different venue configuration, or a different collateral unit is a
 * number in the wrong denomination, and it would bound nothing.
 */
export type ManipulationFloorV1 = Readonly<{
  basis: 'curve-derived' | 'observed-depth';
  sourceSpecId: string;
  adapterConfigId: string;
  collateralUnitId: string;
  derivationReleaseId: string;
  floorAtoms: bigint;
}>;

function identity(bytes: Uint8Array, offset: number, field: string): string {
  const value = slice(bytes, offset, 32);
  // `ContentId::new` refuses the all-zero sentinel, so a floor record that
  // named one would be vacuous in exactly the binding it exists to check.
  if (isZero(value)) throw new Error(`manipulation floor ${field} is the reserved all-zero identity`);
  return hex(value);
}

/**
 * Hostile-decode one exact canonical floor preimage.
 *
 * Mirrors `ManipulationFloorV1::decode` check for check: exact width, magic,
 * schema version, both reserved runs zero, a known basis tag, and four nonzero
 * identities. Nothing here is lenient — the bytes either are a floor record or
 * they are not, and a decoder that guessed would be inventing the bound.
 */
export function decodeManipulationFloorV1(bytes: Uint8Array): ManipulationFloorV1 {
  if (bytes.length !== MANIPULATION_FLOOR_V1_BYTES) {
    throw new Error(`manipulation floor is ${bytes.length} bytes, not the ${MANIPULATION_FLOOR_V1_BYTES} its schema declares`);
  }
  if (ascii(bytes, MANIPULATION_FLOOR_V1_MAGIC_OFFSET, MANIPULATION_FLOOR_V1_MAGIC.length) !== MANIPULATION_FLOOR_V1_MAGIC) {
    throw new Error('these bytes are not a ManipulationFloorV1 record');
  }
  if (u16(bytes, MANIPULATION_FLOOR_V1_VERSION_OFFSET) !== MANIPULATION_FLOOR_V1_SCHEMA_VERSION) {
    throw new Error('manipulation floor names an unsupported schema version');
  }
  requireZero(bytes, MANIPULATION_FLOOR_V1_RESERVED_OFFSET, MANIPULATION_FLOOR_V1_RESERVED_BYTES, 'manipulation floor reserved');
  requireZero(bytes, MANIPULATION_FLOOR_V1_TAIL_RESERVED_OFFSET, MANIPULATION_FLOOR_V1_TAIL_RESERVED_BYTES, 'manipulation floor tail reserved');
  const tag = bytes[MANIPULATION_FLOOR_V1_BASIS_OFFSET];
  const basis = tag === MANIPULATION_FLOOR_V1_CURVE_DERIVED_TAG ? 'curve-derived'
    : tag === MANIPULATION_FLOOR_V1_OBSERVED_DEPTH_TAG ? 'observed-depth'
    : null;
  if (basis === null) throw new Error(`manipulation floor names an unknown derivation basis ${tag}`);
  return Object.freeze({
    basis,
    sourceSpecId: identity(bytes, MANIPULATION_FLOOR_V1_SOURCE_SPEC_OFFSET, 'Source spec'),
    adapterConfigId: identity(bytes, MANIPULATION_FLOOR_V1_ADAPTER_CONFIG_OFFSET, 'adapter config'),
    collateralUnitId: identity(bytes, MANIPULATION_FLOOR_V1_COLLATERAL_UNIT_OFFSET, 'collateral unit'),
    derivationReleaseId: identity(bytes, MANIPULATION_FLOOR_V1_DERIVATION_RELEASE_OFFSET, 'derivation release'),
    // A floor of zero is representable and means "found nothing against this
    // Source". It is not an error; it admits no principal at all, which the
    // predicate reports as `PrincipalExceedsCapacity`.
    floorAtoms: u64(bytes, MANIPULATION_FLOOR_V1_FLOOR_ATOMS_OFFSET),
  });
}

/**
 * The whole chain-state founding admission, as one decision.
 *
 * Mirrors `admit_founding_principal`: bind the floor to the authenticated
 * Source and the Market's collateral unit FIRST, then apply §6.5. The order is
 * the point — a floor that binds to something else is not a weaker bound, it is
 * an answer to a different question, and applying §6.5 to it would produce a
 * verdict that looks exactly like a real one.
 */
export function admitFoundingPrincipalV1(
  capacity: PrincipalCapacityV1,
  floor: ManipulationFloorV1,
  binding: Readonly<{ sourceSpecId: string; adapterConfigId: string; collateralUnitId: string }>,
  totalPrincipalAtoms: bigint,
): PrincipalCapacityVerdictV1 {
  if (floor.sourceSpecId !== binding.sourceSpecId
      || floor.adapterConfigId !== binding.adapterConfigId
      || floor.collateralUnitId !== binding.collateralUnitId) {
    throw new Error('this manipulation floor was derived for another Source, venue configuration, or collateral unit');
  }
  return admitPrincipalCapacityV1(capacity, floor.floorAtoms, totalPrincipalAtoms);
}
