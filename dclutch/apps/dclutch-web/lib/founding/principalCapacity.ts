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

import {
  BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
  CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
  CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
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
