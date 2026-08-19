//! Mathematical Verus shadow for the scalar relation in
//! `crates/clutch-batch/src/lib.rs`.
//!
//! This unit proves properties of the integer model below.  It does not import
//! or verify the production Rust body.  `run_batch_proofs.sh` pins both this
//! file and the exact production seams to which each model predicate is
//! compared; `BATCH_ASSUMPTIONS.md` records the remaining refinement boundary.

use vstd::arithmetic::div_mod::{
    lemma_div_pos_is_pos, lemma_mod_multiples_basic, lemma_multiply_divide_le,
    lemma_multiply_divide_lt, lemma_remainder_lower,
};
use vstd::arithmetic::mul::{
    lemma_mul_inequality, lemma_mul_inequality_converse,
    lemma_mul_is_commutative, lemma_mul_is_distributive_add_other_way,
    lemma_mul_nonnegative, lemma_mul_strict_inequality,
};
use vstd::prelude::*;

verus! {

pub open spec fn max_orders() -> nat { 64 }
pub open spec fn max_grid_ticks() -> nat { 64 }

/// Sum the first `len` mathematical integer entries of a fixed-array shadow.
pub open spec fn prefix_sum(values: Seq<int>, len: nat) -> int
    recommends len <= values.len()
    decreases len
{
    if len == 0 {
        0
    } else {
        prefix_sum(values, (len - 1) as nat) + values[(len - 1) as int]
    }
}

pub open spec fn count_selected(selected: Seq<bool>, len: nat) -> int
    recommends len <= selected.len()
    decreases len
{
    if len == 0 {
        0
    } else {
        count_selected(selected, (len - 1) as nat)
            + if selected[(len - 1) as int] { 1int } else { 0int }
    }
}

/// The quotient assigned by the production allocator's first loop.
pub open spec fn floor_fill(quantity: int, target: int, total: int) -> int
    recommends 0 <= quantity, 0 <= target, 0 < total
{
    quantity * target / total
}

pub open spec fn allocated_fill(
    quantities: Seq<int>,
    selected: Seq<bool>,
    target: int,
    total: int,
    index: int,
) -> int
    recommends
        quantities.len() == selected.len(),
        0 <= index < quantities.len(),
        0 <= target <= total,
        0 < total,
{
    floor_fill(quantities[index], target, total)
        + if selected[index] { 1int } else { 0int }
}

pub open spec fn floor_sum(
    quantities: Seq<int>,
    target: int,
    total: int,
    len: nat,
) -> int
    recommends len <= quantities.len(), 0 <= target, 0 < total
    decreases len
{
    if len == 0 {
        0
    } else {
        floor_sum(quantities, target, total, (len - 1) as nat)
            + floor_fill(quantities[(len - 1) as int], target, total)
    }
}

pub open spec fn allocation_sum(
    quantities: Seq<int>,
    selected: Seq<bool>,
    target: int,
    total: int,
    len: nat,
) -> int
    recommends
        quantities.len() == selected.len(),
        len <= quantities.len(),
        0 <= target <= total,
        0 < total,
    decreases len
{
    if len == 0 {
        0
    } else {
        allocation_sum(quantities, selected, target, total, (len - 1) as nat)
            + allocated_fill(quantities, selected, target, total, (len - 1) as int)
    }
}

proof fn allocation_sum_decomposes(
    quantities: Seq<int>,
    selected: Seq<bool>,
    target: int,
    total: int,
    len: nat,
)
    requires
        quantities.len() == selected.len(),
        len <= quantities.len(),
        0 <= target <= total,
        0 < total,
    ensures
        allocation_sum(quantities, selected, target, total, len)
            == floor_sum(quantities, target, total, len) + count_selected(selected, len),
    decreases len
{
    if len > 0 {
        allocation_sum_decomposes(
            quantities,
            selected,
            target,
            total,
            (len - 1) as nat,
        );
    }
}

proof fn floor_sum_scaled_le(
    quantities: Seq<int>,
    target: int,
    total: int,
    len: nat,
)
    requires
        len <= quantities.len(),
        0 <= target,
        0 < total,
        forall|i: int| 0 <= i < len ==> 0 <= quantities[i],
    ensures
        floor_sum(quantities, target, total, len) * total
            <= prefix_sum(quantities, len) * target,
    decreases len
{
    if len > 0 {
        let index = (len - 1) as int;
        floor_sum_scaled_le(quantities, target, total, (len - 1) as nat);
        lemma_mul_nonnegative(quantities[index], target);
        lemma_remainder_lower(quantities[index] * target, total);
        lemma_mul_is_distributive_add_other_way(
            total,
            floor_sum(quantities, target, total, (len - 1) as nat),
            floor_fill(quantities[index], target, total),
        );
        lemma_mul_is_distributive_add_other_way(
            target,
            prefix_sum(quantities, (len - 1) as nat),
            quantities[index],
        );
    }
}

proof fn floor_sum_is_at_most_target(
    quantities: Seq<int>,
    target: int,
    total: int,
)
    requires
        0 <= target,
        0 < total,
        forall|i: int| 0 <= i < quantities.len() ==> 0 <= quantities[i],
        prefix_sum(quantities, quantities.len()) == total,
    ensures
        floor_sum(quantities, target, total, quantities.len()) <= target,
{
    floor_sum_scaled_le(quantities, target, total, quantities.len());
    lemma_mul_is_commutative(total, target);
    assert(
        floor_sum(quantities, target, total, quantities.len()) * total
            <= target * total
    );
    lemma_mul_inequality_converse(
        floor_sum(quantities, target, total, quantities.len()),
        target,
        total,
    );
}

proof fn floor_fill_is_bounded(quantity: int, target: int, total: int)
    requires
        0 <= quantity,
        0 <= target <= total,
        0 < total,
    ensures
        0 <= floor_fill(quantity, target, total) <= quantity,
{
    lemma_mul_inequality(target, total, quantity);
    lemma_mul_is_commutative(quantity, target);
    lemma_div_pos_is_pos(quantity * target, total);
    lemma_multiply_divide_le(quantity * target, total, quantity);
}

proof fn selected_floor_has_room(quantity: int, target: int, total: int)
    requires
        0 <= quantity,
        0 <= target <= total,
        0 < total,
        (quantity * target) % total > 0,
    ensures
        floor_fill(quantity, target, total) < quantity,
{
    if target == total {
        lemma_mod_multiples_basic(quantity, total);
        assert((quantity * target) % total == 0);
        assert(false);
    }
    assert(target < total);
    assert(quantity > 0) by {
        if quantity == 0 {
            lemma_mod_multiples_basic(0, total);
            assert((quantity * target) % total == 0);
        }
    }
    lemma_mul_strict_inequality(target, total, quantity);
    lemma_mul_is_commutative(quantity, target);
    lemma_multiply_divide_lt(quantity * target, total, quantity);
}

/// Decompose quotient floors plus a caller-supplied one-shot selection mask,
/// and bound each modeled fill.  The positive-remainder condition on selected
/// entries is a premise; this theorem does not prove production dust-loop
/// progress, its completed selection count, or its choice of those entries.
pub proof fn allocation_decomposes_and_bounds(
    quantities: Seq<int>,
    selected: Seq<bool>,
    target: int,
    total: int,
)
    requires
        quantities.len() == selected.len(),
        quantities.len() <= max_orders(),
        0 < total,
        0 <= target <= total,
        forall|i: int| 0 <= i < quantities.len() ==> 0 <= quantities[i],
        prefix_sum(quantities, quantities.len()) == total,
        forall|i: int| 0 <= i < selected.len() && #[trigger] selected[i]
            ==> (quantities[i] * target) % total > 0,
    ensures
        floor_sum(quantities, target, total, quantities.len()) <= target,
        allocation_sum(quantities, selected, target, total, quantities.len())
            == floor_sum(quantities, target, total, quantities.len())
                + count_selected(selected, selected.len()),
        forall|i: int| 0 <= i < quantities.len()
            ==> 0 <= #[trigger] allocated_fill(quantities, selected, target, total, i)
                <= quantities[i],
{
    floor_sum_is_at_most_target(quantities, target, total);
    allocation_sum_decomposes(
        quantities,
        selected,
        target,
        total,
        quantities.len(),
    );
    assert forall|i: int| 0 <= i < quantities.len() implies
        0 <= #[trigger] allocated_fill(quantities, selected, target, total, i)
            <= quantities[i] by {
        floor_fill_is_bounded(quantities[i], target, total);
        if selected[i] {
            selected_floor_has_room(quantities[i], target, total);
        }
    };
}

/// Lexicographic score used by `FixedBook::choose_tick`: greater volume,
/// smaller imbalance, then greater tick index.
pub open spec fn at_least_as_good(
    volumes: Seq<int>,
    imbalances: Seq<int>,
    left: int,
    right: int,
) -> bool
    recommends
        volumes.len() == imbalances.len(),
        0 <= left < volumes.len(),
        0 <= right < volumes.len(),
{
    volumes[left] > volumes[right]
        || (volumes[left] == volumes[right]
            && (imbalances[left] < imbalances[right]
                || (imbalances[left] == imbalances[right] && left >= right)))
}

proof fn score_total(
    volumes: Seq<int>,
    imbalances: Seq<int>,
    left: int,
    right: int,
)
    requires
        volumes.len() == imbalances.len(),
        0 <= left < volumes.len(),
        0 <= right < volumes.len(),
    ensures
        at_least_as_good(volumes, imbalances, left, right)
            || at_least_as_good(volumes, imbalances, right, left),
{
}

proof fn score_transitive(
    volumes: Seq<int>,
    imbalances: Seq<int>,
    first: int,
    second: int,
    third: int,
)
    requires
        volumes.len() == imbalances.len(),
        0 <= first < volumes.len(),
        0 <= second < volumes.len(),
        0 <= third < volumes.len(),
        at_least_as_good(volumes, imbalances, first, second),
        at_least_as_good(volumes, imbalances, second, third),
    ensures
        at_least_as_good(volumes, imbalances, first, third),
{
}

proof fn score_antisymmetric(
    volumes: Seq<int>,
    imbalances: Seq<int>,
    left: int,
    right: int,
)
    requires
        volumes.len() == imbalances.len(),
        0 <= left < volumes.len(),
        0 <= right < volumes.len(),
        at_least_as_good(volumes, imbalances, left, right),
        at_least_as_good(volumes, imbalances, right, left),
    ensures
        left == right,
{
}

proof fn select_best_prefix(
    volumes: Seq<int>,
    imbalances: Seq<int>,
    len: nat,
) -> (best: int)
    requires
        volumes.len() == imbalances.len(),
        0 < len <= volumes.len(),
    ensures
        0 <= best < len,
        forall|i: int| 0 <= i < len
            ==> at_least_as_good(
                volumes,
                imbalances,
                best,
                i,
            ),
    decreases len
{
    if len == 1 {
        0
    } else {
        let previous = select_best_prefix(volumes, imbalances, (len - 1) as nat);
        let challenger = (len - 1) as int;
        if at_least_as_good(volumes, imbalances, challenger, previous) {
            assert forall|i: int| 0 <= i < len implies
                at_least_as_good(volumes, imbalances, challenger, i) by {
                if i < challenger {
                    score_transitive(volumes, imbalances, challenger, previous, i);
                }
            };
            challenger
        } else {
            score_total(volumes, imbalances, previous, challenger);
            assert forall|i: int| 0 <= i < len implies
                at_least_as_good(volumes, imbalances, previous, i) by {
                if i == challenger {
                    score_total(volumes, imbalances, previous, challenger);
                }
            };
            previous
        }
    }
}

pub open spec fn tick_winner(
    volumes: Seq<int>,
    imbalances: Seq<int>,
    tick: int,
) -> bool {
    volumes.len() == imbalances.len()
        && 0 <= tick < volumes.len()
        && forall|i: int| 0 <= i < volumes.len()
            ==> at_least_as_good(volumes, imbalances, tick, i)
}

/// A nonempty bounded score grid has exactly one winner under the frozen tie
/// rule, and the recursive scan returns it.
pub proof fn choose_tick_deterministic(
    volumes: Seq<int>,
    imbalances: Seq<int>,
) -> (chosen: int)
    requires
        volumes.len() == imbalances.len(),
        0 < volumes.len() <= max_grid_ticks(),
        forall|i: int| 0 <= i < volumes.len()
            ==> 0 <= volumes[i] && 0 <= imbalances[i],
    ensures
        tick_winner(
            volumes,
            imbalances,
            chosen,
        ),
        forall|left: int, right: int|
            tick_winner(volumes, imbalances, left)
                && tick_winner(volumes, imbalances, right)
                ==> left == right,
{
    let chosen = select_best_prefix(volumes, imbalances, volumes.len());
    assert forall|left: int, right: int|
        tick_winner(volumes, imbalances, left)
            && tick_winner(volumes, imbalances, right)
            implies left == right by {
        score_antisymmetric(volumes, imbalances, left, right);
    };
    chosen
}

pub open spec fn side_sum(
    buy_side: Seq<bool>,
    fills: Seq<int>,
    len: nat,
    want_buy: bool,
) -> int
    recommends buy_side.len() == fills.len(), len <= fills.len()
    decreases len
{
    if len == 0 {
        0
    } else {
        let index = (len - 1) as int;
        side_sum(buy_side, fills, (len - 1) as nat, want_buy)
            + if buy_side[index] == want_buy { fills[index] } else { 0 }
    }
}

proof fn side_sums_partition(
    buy_side: Seq<bool>,
    fills: Seq<int>,
    len: nat,
)
    requires
        buy_side.len() == fills.len(),
        len <= fills.len(),
    ensures
        side_sum(buy_side, fills, len, true)
            + side_sum(buy_side, fills, len, false)
            == prefix_sum(fills, len),
    decreases len
{
    if len > 0 {
        side_sums_partition(buy_side, fills, (len - 1) as nat);
    }
}

/// Premises extracted from the successful verifier seam: the claimed
/// `matched` field and both recomputed side folds have already passed their
/// production equality checks.  This predicate is not a derived theorem.
pub open spec fn accepted_side_equalities(
    buy_side: Seq<bool>,
    fills: Seq<int>,
    len: nat,
    matched: int,
) -> bool {
    buy_side.len() == fills.len()
        && len <= fills.len()
        && 0 <= matched
        && (forall|i: int| 0 <= i < len ==> 0 <= fills[i])
        && side_sum(buy_side, fills, len, true) == matched
        && side_sum(buy_side, fills, len, false) == matched
}

/// Given the two accepted side equalities as premises, derive only the
/// whole-fill partition identity and its `2 * matched` consequence.
pub proof fn accepted_sides_partition_whole_fill(
    buy_side: Seq<bool>,
    fills: Seq<int>,
    len: nat,
    matched: int,
)
    requires accepted_side_equalities(buy_side, fills, len, matched)
    ensures
        prefix_sum(fills, len)
            == side_sum(buy_side, fills, len, true)
                + side_sum(buy_side, fills, len, false),
        prefix_sum(fills, len) == 2 * matched,
{
    side_sums_partition(buy_side, fills, len);
}

pub open spec fn canonical_padding(values: Seq<int>, active_len: nat) -> bool {
    active_len <= values.len()
        && forall|i: int| active_len <= i < values.len() ==> values[i] == 0
}

proof fn zero_padding_does_not_change_sum(
    values: Seq<int>,
    active_len: nat,
    upto: nat,
)
    requires
        canonical_padding(values, active_len),
        active_len <= upto <= values.len(),
    ensures
        prefix_sum(values, upto) == prefix_sum(values, active_len),
    decreases upto - active_len
{
    if upto > active_len {
        zero_padding_does_not_change_sum(values, active_len, (upto - 1) as nat);
        assert(values[(upto - 1) as int] == 0);
    }
}

/// Under the `canonical_padding` zero-suffix premise, a full-array fold is
/// identical to the active-prefix fold.  Production validation is not proved.
pub proof fn canonical_padding_fold_identity(values: Seq<int>, active_len: nat)
    requires
        values.len() <= max_orders(),
        canonical_padding(values, active_len),
    ensures
        prefix_sum(values, values.len()) == prefix_sum(values, active_len),
{
    zero_padding_does_not_change_sum(values, active_len, values.len());
}

}
