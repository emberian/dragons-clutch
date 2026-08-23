"""Adversarial properties for the quotient-risk ScoreV2 proposal."""

from __future__ import annotations

import itertools
import unittest

from model import (
    MAX_OUTCOMES,
    U64_MAX,
    ExecutedLeg,
    ModelError,
    RiskObjectiveV2,
    SelectionKeyV2,
    Side,
    aggregate_vectors,
    direct_flow_from_buy_side,
    direct_flow_from_sell_side,
    indistinguishable_owner_worlds,
    objective_from_padded_flow,
    owner_normalized_direct_flow,
    price_weighted_gini_numerator,
    quotient_representative,
    score_v1_primary,
)


class QuotientRiskProperties(unittest.TestCase):
    def test_exhaustive_complete_set_shift_and_complement_invariance(self) -> None:
        for outcomes in range(2, 6):
            for flow in itertools.product(range(4), repeat=outcomes):
                score = RiskObjectiveV2.from_direct_flow(flow)
                for shift in range(5):
                    shifted = tuple(value + shift for value in flow)
                    self.assertEqual(RiskObjectiveV2.from_direct_flow(shifted), score)
                ceiling = max(flow)
                complement = tuple(ceiling - value for value in flow)
                self.assertEqual(RiskObjectiveV2.from_direct_flow(complement), score)

    def test_zero_exactly_identifies_complete_set_flow(self) -> None:
        for outcomes in range(2, 7):
            for quantity in range(8):
                self.assertEqual(
                    RiskObjectiveV2.from_direct_flow((quantity,) * outcomes),
                    RiskObjectiveV2(0),
                )
            for outcome in range(outcomes):
                flow = [0] * outcomes
                flow[outcome] = 1
                self.assertEqual(
                    RiskObjectiveV2.from_direct_flow(flow), RiskObjectiveV2(1)
                )

    def test_range_is_the_minimum_decomposition_risk_lower_bound(self) -> None:
        # R(sum_k a_k) <= sum_k R(a_k).  Grouping the aggregate as one vector
        # attains equality, so the bound is exact without choosing identities.
        for outcomes in range(2, 5):
            vectors = tuple(itertools.product(range(4), repeat=outcomes))
            for left in vectors:
                left_score = RiskObjectiveV2.from_direct_flow(left)
                for right in vectors:
                    right_score = RiskObjectiveV2.from_direct_flow(right)
                    aggregate = aggregate_vectors((left, right))
                    aggregate_score = RiskObjectiveV2.from_direct_flow(aggregate)
                    self.assertLessEqual(
                        aggregate_score.certified_risk_flow_atoms,
                        left_score.certified_risk_flow_atoms
                        + right_score.certified_risk_flow_atoms,
                    )

    def test_scaling_is_exact_until_the_u64_admission_boundary(self) -> None:
        flow = (0, 3, 7, 2)
        base = RiskObjectiveV2.from_direct_flow(flow)
        for scalar in range(17):
            scaled = tuple(scalar * value for value in flow)
            self.assertEqual(
                RiskObjectiveV2.from_direct_flow(scaled).certified_risk_flow_atoms,
                scalar * base.certified_risk_flow_atoms,
            )

    def test_payoff_preserving_state_refinement_is_invariant(self) -> None:
        for flow in ((0, 7), (3, 8, 1), (9, 9, 2, 4)):
            expected = RiskObjectiveV2.from_direct_flow(flow)
            for index in range(len(flow)):
                refined = flow[: index + 1] + (flow[index],) + flow[index + 1 :]
                self.assertEqual(RiskObjectiveV2.from_direct_flow(refined), expected)

    def test_inactive_zero_padding_is_excluded_from_the_quotient(self) -> None:
        active = (7, 7)
        padded = active + (0,) * (MAX_OUTCOMES - len(active))
        self.assertEqual(
            objective_from_padded_flow(padded, len(active)),
            RiskObjectiveV2(0),
        )
        with self.assertRaises(ModelError):
            objective_from_padded_flow(active + (1,) + (0,) * 13, len(active))

    def test_partial_fill_splitting_preserves_the_aggregate_objective(self) -> None:
        whole = (19, 0, 7)
        fragments = ((4, 0, 2), (6, 0, 1), (9, 0, 4))
        self.assertEqual(aggregate_vectors(fragments), whole)
        self.assertEqual(
            RiskObjectiveV2.from_direct_flow(aggregate_vectors(fragments)),
            RiskObjectiveV2.from_direct_flow(whole),
        )

    def test_virtual_complete_set_translations_leave_direct_flow_exact(self) -> None:
        direct = (9, 0, 4)
        for translation in range(8):
            buys = tuple(value + translation for value in direct)
            sells = tuple(value + translation for value in direct)
            self.assertEqual(direct_flow_from_buy_side(buys, translation), direct)
            self.assertEqual(direct_flow_from_sell_side(sells, translation), direct)
            self.assertEqual(
                RiskObjectiveV2.from_direct_flow(
                    direct_flow_from_buy_side(buys, translation)
                ),
                RiskObjectiveV2.from_direct_flow(direct),
            )

    def test_tail_and_midpoint_claims_receive_equal_quantity_score(self) -> None:
        quantity = 91
        for outcome in range(4):
            flow = [0, 0, 0, 0]
            flow[outcome] = quantity
            self.assertEqual(
                RiskObjectiveV2.from_direct_flow(flow), RiskObjectiveV2(quantity)
            )

        # ScoreV1 suppresses the one-percent tail relative to the midpoint.
        self.assertGreater(
            score_v1_primary((quantity, 0), (5_000, 5_000), 10_000),
            score_v1_primary((quantity, 0), (100, 9_900), 10_000),
        )

    def test_price_is_not_smuggled_into_the_risk_objective(self) -> None:
        flow = (13, 0)
        score = RiskObjectiveV2.from_direct_flow(flow)
        for prices in ((0, 10_000), (100, 9_900), (5_000, 5_000), (9_000, 1_000)):
            self.assertEqual(RiskObjectiveV2.from_direct_flow(flow), score)
            # The observational Gini moves, including to zero at the boundary.
        self.assertEqual(price_weighted_gini_numerator(flow, (0, 10_000), 10_000), 0)
        self.assertGreater(
            price_weighted_gini_numerator(flow, (5_000, 5_000), 10_000), 0
        )

    def test_no_multiplication_or_rounding_is_needed_at_the_integer_boundary(
        self,
    ) -> None:
        alternating = tuple(
            U64_MAX if index % 2 else 0 for index in range(MAX_OUTCOMES)
        )
        self.assertEqual(
            RiskObjectiveV2.from_direct_flow(alternating),
            RiskObjectiveV2(U64_MAX),
        )
        self.assertEqual(
            quotient_representative((U64_MAX,) * MAX_OUTCOMES),
            (0,) * MAX_OUTCOMES,
        )

    def test_admission_refuses_width_sign_type_and_u64_overflow(self) -> None:
        for bad in ((1,), (0,) * 17, (-1, 0), (U64_MAX + 1, 0), (True, 0)):
            with self.assertRaises(ModelError):
                RiskObjectiveV2.from_direct_flow(bad)
        with self.assertRaises(ModelError):
            direct_flow_from_buy_side((0, 1), 1)
        with self.assertRaises(ModelError):
            direct_flow_from_sell_side((0, 1), 1)


class SelectionProperties(unittest.TestCase):
    @staticmethod
    def key(
        flow: tuple[int, ...],
        split: int = 0,
        merge: int = 0,
        digest_byte: int = 0,
    ) -> SelectionKeyV2:
        return SelectionKeyV2.from_candidate(
            flow, split, merge, bytes([digest_byte]) * 32
        )

    def test_risk_precedes_every_representation_tie(self) -> None:
        high = self.key((8, 0), split=9, digest_byte=255)
        low = self.key((7, 0), digest_byte=0)
        self.assertTrue(high.is_better_than(low))

    def test_min_zero_representative_wins_a_complete_set_shift_tie(self) -> None:
        canonical = self.key((8, 0), digest_byte=255)
        shifted = self.key((13, 5), digest_byte=0)
        self.assertEqual(canonical.objective, shifted.objective)
        self.assertTrue(canonical.is_better_than(shifted))

    def test_empty_candidate_beats_a_pure_complete_set_wash(self) -> None:
        empty = self.key((0, 0), digest_byte=255)
        wash = self.key((10, 10), digest_byte=0)
        self.assertEqual(empty.objective, wash.objective)
        self.assertTrue(empty.is_better_than(wash))

    def test_lower_churn_then_smaller_full_digest_break_ties(self) -> None:
        low_churn = self.key((3, 0), split=1, digest_byte=255)
        high_churn = self.key((3, 0), split=2, digest_byte=0)
        self.assertTrue(low_churn.is_better_than(high_churn))

        small_digest = self.key((3, 0), split=1, digest_byte=4)
        large_digest = self.key((3, 0), split=1, digest_byte=5)
        self.assertTrue(small_digest.is_better_than(large_digest))
        self.assertEqual(small_digest.compare(small_digest), 0)

    def test_churn_overflow_and_noncanonical_digest_refuse(self) -> None:
        with self.assertRaises(ModelError):
            SelectionKeyV2.from_candidate((1, 0), U64_MAX, 1, bytes(32))
        with self.assertRaises(ModelError):
            SelectionKeyV2.from_candidate((1, 0), 0, 0, bytes(16))


class SybilAndWashFalsifiers(unittest.TestCase):
    def test_score_v1_rewards_the_binary_midpoint_complete_set_wash(self) -> None:
        quantity = 4
        flow = (quantity, quantity)
        self.assertGreater(score_v1_primary(flow, (5_000, 5_000), 10_000), 0)
        self.assertEqual(RiskObjectiveV2.from_direct_flow(flow), RiskObjectiveV2(0))

    def test_owner_and_order_fragmentation_do_not_enter_score_v2(self) -> None:
        # These are three representations of the same admitted aggregate flow.
        whole_owner = {"alice": ((9, 0, 2),)}
        fragmented_orders = {"alice": ((4, 0, 1), (5, 0, 1))}
        fragmented_keys = {"alice-1": ((4, 0, 1),), "alice-2": ((5, 0, 1),)}

        def aggregate(
            representation: dict[str, tuple[tuple[int, ...], ...]]
        ) -> tuple[int, ...]:
            return aggregate_vectors(
                vector
                for owner_vectors in representation.values()
                for vector in owner_vectors
            )

        expected = RiskObjectiveV2.from_direct_flow((9, 0, 2))
        for representation in (whole_owner, fragmented_orders, fragmented_keys):
            self.assertEqual(
                RiskObjectiveV2.from_direct_flow(aggregate(representation)), expected
            )

    def test_v1_owner_normalization_itself_is_not_sybil_neutral(self) -> None:
        same_key = (
            ExecutedLeg("actor", 0, Side.BUY, 7),
            ExecutedLeg("actor", 0, Side.SELL, 7),
        )
        split_keys = (
            ExecutedLeg("actor-a", 0, Side.BUY, 7),
            ExecutedLeg("actor-b", 0, Side.SELL, 7),
        )
        self.assertEqual(owner_normalized_direct_flow(2, same_key), (0, 0))
        self.assertEqual(owner_normalized_direct_flow(2, split_keys), (7, 0))
        self.assertNotEqual(
            RiskObjectiveV2.from_direct_flow(owner_normalized_direct_flow(2, same_key)),
            RiskObjectiveV2.from_direct_flow(
                owner_normalized_direct_flow(2, split_keys)
            ),
        )

    def test_nonconstant_wash_is_observationally_identical_to_honest_trade(
        self,
    ) -> None:
        public_keys = ("key-a", "key-b")
        self.assertTrue(indistinguishable_owner_worlds(public_keys, public_keys))
        self.assertGreater(
            RiskObjectiveV2.from_direct_flow((7, 0)).certified_risk_flow_atoms,
            0,
        )
        # No deterministic score over this identical transcript can know
        # whether the keys have one controller or two.  This test preserves the
        # counterexample instead of claiming wash-proofness.


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
