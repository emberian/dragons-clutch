"""Invariant and adversarial tests for the structured-claim wrapper model."""

from __future__ import annotations

import copy
import random
import sys
import unittest
from itertools import product
from pathlib import Path


sys.path.insert(0, str(Path(__file__).resolve().parent))

from model import (  # noqa: E402
    AssetKind,
    BasisIdentity,
    ClaimDescriptor,
    COMPATIBILITY_SEMANTICS,
    Refusal,
    UnderlyingAsset,
    U64_MAX,
    WrapperMachine,
    basis_digest,
    canonical_underlyings,
    external_vault_estimate,
    internal_position_estimate,
    position_only_estimate,
    rent_exempt_lamports,
    resolved_redemption_lot,
    simplex_vectors,
    universal_redemption_lot,
)


def identity(
    outcomes: int = 3,
    denominator: int = 6,
    degree: int = 2,
    marker: int = 1,
    semantics: str = "native-open-clamped-bspline-v1",
) -> BasisIdentity:
    return BasisIdentity(
        bytes((marker,)) * 32,
        bytes((marker + 1,)) * 32,
        bytes((marker + 2,)) * 32,
        degree,
        denominator,
        outcomes,
        semantics,
    )


def descriptor(coefficients=(1, 2, 4), **kwargs):
    return ClaimDescriptor.compile(identity(**kwargs), coefficients)[0]


class DescriptorTests(unittest.TestCase):
    def test_proportional_coefficients_have_one_canonical_mint(self):
        basis = identity()
        left, left_scale = ClaimDescriptor.compile(basis, (10, 20, 40))
        right, right_scale = ClaimDescriptor.compile(basis, (1, 2, 4))
        self.assertEqual(left.coefficients, (1, 2, 4))
        self.assertEqual(left.digest, right.digest)
        self.assertEqual((left_scale, right_scale), (10, 1))

    def test_basis_identity_is_load_bearing(self):
        one, _ = ClaimDescriptor.compile(identity(marker=1), (1, 2, 4))
        other, _ = ClaimDescriptor.compile(identity(marker=7), (1, 2, 4))
        cubic, _ = ClaimDescriptor.compile(identity(marker=1, degree=3), (1, 2, 4))
        self.assertNotEqual(one.digest, other.digest)
        self.assertNotEqual(one.digest, cubic.digest)

    def test_categorical_compatibility_lowering_cannot_pose_as_native(self):
        lowered = identity(semantics=COMPATIBILITY_SEMANTICS)
        with self.assertRaisesRegex(Refusal, "not a native basis"):
            ClaimDescriptor.compile(lowered, (1, 2, 4))

    def test_nesting_cross_market_and_noncanonical_order_are_refused(self):
        basis = identity()
        eggs = list(canonical_underlyings(basis))
        nested = list(eggs)
        nested[1] = UnderlyingAsset(
            AssetKind.STRUCTURED_WRAPPER, basis_digest(basis), 1
        )
        with self.assertRaisesRegex(Refusal, "nesting"):
            ClaimDescriptor.compile(basis, (1, 2, 4), nested)
        foreign = list(eggs)
        foreign[1] = UnderlyingAsset(
            AssetKind.NATIVE_BASIS_EGG, basis_digest(identity(marker=9)), 1
        )
        with self.assertRaisesRegex(Refusal, "canonical ordered"):
            ClaimDescriptor.compile(basis, (1, 2, 4), foreign)
        swapped = [eggs[1], eggs[0], eggs[2]]
        with self.assertRaisesRegex(Refusal, "canonical ordered"):
            ClaimDescriptor.compile(basis, (1, 2, 4), swapped)

    def test_redundant_or_hostile_claims_refuse(self):
        basis = identity()
        for values in ((0, 0, 0), (0, 7, 0), (7, 7, 7)):
            with self.subTest(values=values), self.assertRaises(Refusal):
                ClaimDescriptor.compile(basis, values)
        with self.assertRaises(Refusal):
            ClaimDescriptor.compile(basis, (1, 2))
        with self.assertRaises(Refusal):
            ClaimDescriptor.compile(basis, (1, -1, 2))
        with self.assertRaises(Refusal):
            ClaimDescriptor.compile(basis, (1, 2, U64_MAX + 1))


class WrapperTransitionTests(unittest.TestCase):
    def machine(self, coefficients=(1, 2, 4), amount=100) -> WrapperMachine:
        claim = descriptor(coefficients)
        return WrapperMachine(
            claim,
            {"alice": (amount,) * 3, "bob": (amount // 2,) * 3},
            hoard_atoms=amount + amount // 2,
        )

    def test_merge_split_and_transfer_preserve_exact_backing(self):
        machine = self.machine()
        original_supply = machine.total_basis_supply
        machine.merge_components("alice", 10)
        self.assertEqual(machine.vault, [10, 20, 40])
        machine.transfer_wrapper("alice", "bob", 4)
        machine.split_components("bob", 3)
        self.assertEqual(machine.wrapper_supply, 7)
        self.assertEqual(machine.vault, [7, 14, 28])
        self.assertEqual(machine.total_basis_supply, original_supply)
        machine.assert_invariants()

    def test_direct_wrapper_burn_is_a_donation_and_surplus_compacts_to_burns(self):
        machine = self.machine()
        machine.merge_components("alice", 10)
        before_base_supply = machine.total_basis_supply
        machine.direct_burn_wrapper("alice", 3)
        self.assertEqual(machine.wrapper_supply, 7)
        self.assertEqual(machine.vault, [10, 20, 40])
        burned = machine.compact_surplus()
        self.assertEqual(burned, (3, 6, 12))
        self.assertEqual(machine.vault, [7, 14, 28])
        self.assertEqual(
            machine.total_basis_supply,
            tuple(before_base_supply[i] - burned[i] for i in range(3)),
        )
        machine.assert_invariants()

    def test_arbitrary_component_donation_never_mints_or_pays_a_caller(self):
        machine = self.machine()
        machine.merge_components("alice", 5)
        supply = machine.wrapper_supply
        machine.donate_components("bob", (3, 0, 9))
        self.assertEqual(machine.wrapper_supply, supply)
        self.assertEqual(machine.vault, [8, 10, 29])
        self.assertEqual(machine.compact_surplus(), (3, 0, 9))
        self.assertEqual(machine.vault, [5, 10, 20])

    def test_failed_transitions_are_validate_before_mutate(self):
        machine = self.machine(amount=10)
        for operation in (
            lambda: machine.merge_components("alice", 11),
            lambda: machine.split_components("alice", 1),
            lambda: machine.transfer_wrapper("alice", "bob", 1),
            lambda: machine.donate_components("alice", (11, 0, 0)),
            lambda: machine.unauthorized_mint("alice", 1),
            lambda: machine.unauthorized_vault_withdrawal((1, 0, 0)),
        ):
            before = machine.snapshot()
            with self.assertRaises(Refusal):
                operation()
            self.assertEqual(machine.snapshot(), before)

    def test_overflow_refuses_before_mutation(self):
        claim = descriptor((1, 2, U64_MAX))
        machine = WrapperMachine(
            claim,
            {"alice": (U64_MAX, U64_MAX, U64_MAX)},
            hoard_atoms=U64_MAX,
        )
        before = machine.snapshot()
        with self.assertRaisesRegex(Refusal, "overflows"):
            machine.merge_components("alice", 2)
        self.assertEqual(machine.snapshot(), before)

    def test_unbacked_mint_and_vault_drain_are_detected(self):
        machine = self.machine()
        machine.merge_components("alice", 2)
        forged = copy.deepcopy(machine)
        forged.wrapper_supply += 1
        forged.wrapper_balances["alice"] += 1
        with self.assertRaisesRegex(AssertionError, "not exactly overcollateralized"):
            forged.assert_invariants()
        drained = copy.deepcopy(machine)
        drained.vault[2] -= 1
        with self.assertRaisesRegex(AssertionError, "not exactly overcollateralized"):
            drained.assert_invariants()

    def test_retirement_requires_zero_actual_supply_and_zero_vault(self):
        machine = self.machine()
        machine.merge_components("alice", 2)
        with self.assertRaises(Refusal):
            machine.retire()
        machine.direct_burn_wrapper("alice", 2)
        machine.compact_surplus()
        machine.retire()
        with self.assertRaises(Refusal):
            machine.merge_components("alice", 1)


class RedemptionTests(unittest.TestCase):
    def test_universal_lot_is_exact_and_minimal_over_small_simplexes(self):
        denominator = 6
        for coefficients in product(range(5), repeat=3):
            if coefficients == (0, 0, 0):
                continue
            lot = universal_redemption_lot(coefficients, denominator)
            vectors = tuple(simplex_vectors(3, denominator))
            self.assertTrue(
                all(
                    lot * sum(c * w for c, w in zip(coefficients, weights))
                    % denominator
                    == 0
                    for weights in vectors
                )
            )
            for smaller in range(1, lot):
                self.assertTrue(
                    any(
                        smaller
                        * sum(c * w for c, w in zip(coefficients, weights))
                        % denominator
                        != 0
                        for weights in vectors
                    ),
                    (coefficients, lot, smaller),
                )

    def test_resolution_specific_lot_and_exact_aggregate_redemption(self):
        claim = descriptor((1, 2, 4))
        machine = WrapperMachine(claim, {"alice": (60, 60, 60)}, hoard_atoms=60)
        machine.merge_components("alice", 12)
        weights = (1, 4, 1)
        machine.resolve(weights)
        self.assertEqual(resolved_redemption_lot((1, 2, 4), weights, 6), 6)
        before = machine.snapshot()
        with self.assertRaisesRegex(Refusal, "not an exact lot"):
            machine.redeem_terminal("alice", 1)
        self.assertEqual(machine.snapshot(), before)
        self.assertEqual(machine.redeem_terminal("alice", 6), 13)
        self.assertEqual(machine.wrapper_supply, 6)
        machine.assert_invariants()

    def test_redemption_pays_native_dot_product_not_a_categorical_cell(self):
        claim = descriptor((1, 5, 2), degree=3)
        machine = WrapperMachine(claim, {"alice": (40, 40, 40)}, hoard_atoms=40)
        machine.merge_components("alice", 6)
        machine.resolve((2, 3, 1))
        # 6 * (1*2 + 5*3 + 2*1) / 6 = 19.  No one-hot cell is selected.
        self.assertEqual(machine.redeem_terminal("alice", 6), 19)


class ResourceTests(unittest.TestCase):
    def test_default_rent_values_and_cost_table(self):
        self.assertEqual(rent_exempt_lamports(82), 1_461_600)
        self.assertEqual(rent_exempt_lamports(170), 2_074_080)
        expected_external = {
            2: 8_393_760,
            4: 12_541_920,
            8: 20_838_240,
            16: 37_430_880,
        }
        for outcomes, lamports in expected_external.items():
            with self.subTest(outcomes=outcomes):
                self.assertEqual(
                    external_vault_estimate(outcomes).infrastructure_lamports,
                    lamports,
                )
                self.assertEqual(
                    internal_position_estimate(outcomes).infrastructure_lamports,
                    8_143_200,
                )
                self.assertEqual(
                    position_only_estimate(outcomes).infrastructure_lamports,
                    6_681_600,
                )

    def test_external_vault_account_and_cpi_growth_is_visible(self):
        external = external_vault_estimate(16)
        internal = internal_position_estimate(16)
        self.assertEqual((external.wrap_accounts, external.wrap_cpis), (53, 17))
        self.assertEqual((internal.wrap_accounts, internal.wrap_cpis), (12, 2))
        self.assertGreater(external.infrastructure_lamports, 4 * internal.infrastructure_lamports)


class SequenceTests(unittest.TestCase):
    def test_deterministic_adversarial_sequences_preserve_invariants(self):
        rng = random.Random(0xD6E66)
        claim = descriptor((1, 2, 3))
        machine = WrapperMachine(
            claim,
            {"alice": (5_000, 5_000, 5_000), "bob": (5_000, 5_000, 5_000)},
            hoard_atoms=10_000,
        )
        owners = ("alice", "bob")
        refusals = 0
        for _ in range(5_000):
            actor = owners[rng.randrange(2)]
            other = owners[1 - owners.index(actor)]
            choice = rng.randrange(6)
            before = machine.snapshot()
            try:
                if choice == 0:
                    machine.merge_components(actor, rng.randrange(1, 12))
                elif choice == 1:
                    machine.split_components(actor, rng.randrange(1, 12))
                elif choice == 2:
                    machine.transfer_wrapper(actor, other, rng.randrange(1, 12))
                elif choice == 3:
                    machine.direct_burn_wrapper(actor, rng.randrange(1, 8))
                elif choice == 4:
                    machine.donate_components(
                        actor, tuple(rng.randrange(3) for _ in range(3))
                    )
                else:
                    machine.compact_surplus()
            except Refusal:
                refusals += 1
                self.assertEqual(machine.snapshot(), before)
            machine.assert_invariants()
        self.assertGreater(refusals, 0)


if __name__ == "__main__":
    unittest.main()
