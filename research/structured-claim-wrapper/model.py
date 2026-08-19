"""Executable accounting model for an atomic structured-claim wrapper.

This module is deliberately not a Solana program.  It isolates the arithmetic
and state-machine questions that must be settled before a Token-2022 wrapper
can be admitted.  All quantities are integer atoms and every transition is
validate-before-mutate.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from functools import reduce
from hashlib import sha256
from math import gcd
from typing import Iterable, Mapping, Sequence


MAX_OUTCOMES = 16
U64_MAX = (1 << 64) - 1
U128_MAX = (1 << 128) - 1
NATIVE_BASIS_SEMANTICS = "native-open-clamped-bspline-v1"
COMPATIBILITY_SEMANTICS = "categorical-compatibility-lowering-v1"


class Refusal(ValueError):
    """A deterministic refusal of hostile or inadmissible input."""


class AssetKind(Enum):
    NATIVE_BASIS_EGG = "native-basis-egg"
    STRUCTURED_WRAPPER = "structured-wrapper"


def _bytes32(value: bytes, name: str) -> bytes:
    if not isinstance(value, bytes) or len(value) != 32:
        raise Refusal(f"{name} must be exactly 32 bytes")
    return value


def _u64(value: int, name: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0 or value > U64_MAX:
        raise Refusal(f"{name} is not a u64")
    return value


def _checked_add(left: int, right: int, name: str) -> int:
    value = left + right
    if value > U64_MAX:
        raise Refusal(f"{name} overflows u64")
    return value


def _checked_mul(left: int, right: int, name: str) -> int:
    value = left * right
    if value > U64_MAX:
        raise Refusal(f"{name} overflows u64")
    return value


def _lcm(left: int, right: int) -> int:
    return left // gcd(left, right) * right


@dataclass(frozen=True)
class BasisIdentity:
    """Consensus identity of one native payout basis.

    The terms digest is expected to bind the knot vector, degree, edge and
    ambiguity policies, denominator, evaluator version, source/window identity,
    and rounding rule.  This model does not pretend to reconstruct those terms.
    """

    base_program: bytes
    market: bytes
    terms_digest: bytes
    degree: int
    denominator: int
    outcome_count: int
    semantics: str = NATIVE_BASIS_SEMANTICS

    def validate_native(self) -> None:
        _bytes32(self.base_program, "base program")
        _bytes32(self.market, "market")
        _bytes32(self.terms_digest, "terms digest")
        if self.semantics != NATIVE_BASIS_SEMANTICS:
            raise Refusal("a compatibility lowering is not a native basis")
        if self.degree not in range(4):
            raise Refusal("native degree must be in 0..=3")
        if self.outcome_count < 2 or self.outcome_count > MAX_OUTCOMES:
            raise Refusal("outcome count must be in 2..=16")
        if self.denominator <= 0 or self.denominator > U64_MAX:
            raise Refusal("denominator must be a nonzero u64")

    def canonical_bytes(self) -> bytes:
        self.validate_native()
        return b"".join(
            (
                b"dragons-clutch:native-basis:v1\x00",
                self.base_program,
                self.market,
                self.terms_digest,
                bytes((self.degree, self.outcome_count)),
                self.denominator.to_bytes(8, "little"),
            )
        )


@dataclass(frozen=True)
class UnderlyingAsset:
    """One proposed backing asset, used to make nesting refusal executable."""

    kind: AssetKind
    basis_digest: bytes
    outcome: int


def basis_digest(basis: BasisIdentity) -> bytes:
    return sha256(basis.canonical_bytes()).digest()


def canonical_underlyings(basis: BasisIdentity) -> tuple[UnderlyingAsset, ...]:
    digest = basis_digest(basis)
    return tuple(
        UnderlyingAsset(AssetKind.NATIVE_BASIS_EGG, digest, outcome)
        for outcome in range(basis.outcome_count)
    )


def _validate_underlyings(
    basis: BasisIdentity, underlyings: Sequence[UnderlyingAsset]
) -> None:
    if tuple(underlyings) != canonical_underlyings(basis):
        if any(asset.kind is AssetKind.STRUCTURED_WRAPPER for asset in underlyings):
            raise Refusal("wrapper nesting is forbidden")
        raise Refusal("backing must be the canonical ordered native basis Eggs")


@dataclass(frozen=True)
class ClaimDescriptor:
    """Canonical primitive coefficient claim over one native basis.

    One wrapper atom is backed by exactly ``coefficients[i]`` atoms of native
    basis Egg ``i``.  Requested proportional vectors canonicalize to a primitive
    gcd-one vector; ``display_scale`` tells the caller how many primitive wrapper
    atoms reproduce one requested display lot and is not part of mint identity.
    """

    basis: BasisIdentity
    coefficients: tuple[int, ...]
    digest: bytes

    @classmethod
    def compile(
        cls,
        basis: BasisIdentity,
        requested_coefficients: Sequence[int],
        underlyings: Sequence[UnderlyingAsset] | None = None,
    ) -> tuple["ClaimDescriptor", int]:
        basis.validate_native()
        if len(requested_coefficients) != basis.outcome_count:
            raise Refusal("coefficient count does not match the basis")
        requested = tuple(
            _u64(value, f"coefficient[{index}]")
            for index, value in enumerate(requested_coefficients)
        )
        nonzero = tuple(value for value in requested if value != 0)
        if not nonzero:
            raise Refusal("the zero claim has no wrapper product value")
        divisor = reduce(gcd, nonzero)
        primitive = tuple(value // divisor for value in requested)
        if sum(value != 0 for value in primitive) < 2:
            raise Refusal("a single-Egg wrapper only fragments the native mint")
        if len(set(primitive)) == 1:
            raise Refusal("a constant complete-set wrapper should merge to collateral")
        _validate_underlyings(
            basis,
            canonical_underlyings(basis) if underlyings is None else underlyings,
        )
        preimage = bytearray(b"dragons-clutch:structured-claim:v1\x00")
        preimage.extend(basis.canonical_bytes())
        for coefficient in primitive:
            preimage.extend(coefficient.to_bytes(8, "little"))
        digest = sha256(preimage).digest()
        return cls(basis, primitive, digest), divisor

    @property
    def maximum_payout_atoms(self) -> int:
        return max(self.coefficients)


def universal_redemption_lot(coefficients: Sequence[int], denominator: int) -> int:
    """Least quantity making payout integral for every integer simplex vector."""

    if not coefficients:
        raise Refusal("empty coefficient vector")
    denominator = _u64(denominator, "denominator")
    if denominator == 0:
        raise Refusal("zero denominator")
    base = _u64(coefficients[0], "coefficient[0]")
    lot = 1
    for index, coefficient in enumerate(coefficients[1:], start=1):
        coefficient = _u64(coefficient, f"coefficient[{index}]")
        required = denominator // gcd(denominator, abs(coefficient - base))
        lot = _lcm(lot, required)
    return lot


def resolved_redemption_lot(
    coefficients: Sequence[int], weights: Sequence[int], denominator: int
) -> int:
    """Least exact quantity after one particular resolution vector is frozen."""

    if len(coefficients) != len(weights):
        raise Refusal("weight count mismatch")
    denominator = _u64(denominator, "denominator")
    if denominator == 0:
        raise Refusal("zero denominator")
    checked_weights = tuple(_u64(value, "weight") for value in weights)
    if sum(checked_weights) != denominator:
        raise Refusal("weights do not sum to the denominator")
    numerator = sum(
        _u64(coefficient, "coefficient") * weight
        for coefficient, weight in zip(coefficients, checked_weights)
    )
    if numerator > U128_MAX:
        raise Refusal("payout numerator overflows u128")
    return denominator // gcd(denominator, numerator)


class WrapperMachine:
    """Small exact state machine for one canonical wrapper mint.

    ``basis_balances`` model ordinary owner-held native basis Eggs.
    ``other_basis`` models Eggs elsewhere in the market.  ``vault`` is owned by
    a wrapper PDA.  Actual Token-2022 mint supply, not a program shadow, is
    represented by ``wrapper_supply``.
    """

    def __init__(
        self,
        descriptor: ClaimDescriptor,
        basis_balances: Mapping[str, Sequence[int]],
        hoard_atoms: int,
        other_basis: Sequence[int] | None = None,
    ) -> None:
        descriptor.basis.validate_native()
        self.descriptor = descriptor
        n = descriptor.basis.outcome_count
        self.basis_balances = {
            owner: [
                _u64(value, f"{owner}.basis[{index}]")
                for index, value in enumerate(values)
            ]
            for owner, values in basis_balances.items()
        }
        if any(len(values) != n for values in self.basis_balances.values()):
            raise Refusal("owner basis balance width mismatch")
        self.other_basis = [0] * n if other_basis is None else list(other_basis)
        if len(self.other_basis) != n:
            raise Refusal("other basis width mismatch")
        self.other_basis = [
            _u64(value, f"other_basis[{index}]")
            for index, value in enumerate(self.other_basis)
        ]
        self.vault = [0] * n
        self.wrapper_balances = {owner: 0 for owner in self.basis_balances}
        self.wrapper_supply = 0
        self.hoard_atoms = _u64(hoard_atoms, "hoard atoms")
        self.resolved_weights: tuple[int, ...] | None = None
        self.retired = False
        self.assert_invariants()

    @property
    def total_basis_supply(self) -> tuple[int, ...]:
        totals = []
        for index in range(self.descriptor.basis.outcome_count):
            total = self.other_basis[index] + self.vault[index]
            total += sum(values[index] for values in self.basis_balances.values())
            if total > U64_MAX:
                raise AssertionError("model basis supply exceeds u64")
            totals.append(total)
        return tuple(totals)

    def _terminal_numerator(self, quantities: Sequence[int] | None = None) -> int:
        if self.resolved_weights is None:
            raise Refusal("market is unresolved")
        vector = self.total_basis_supply if quantities is None else quantities
        value = sum(
            quantity * weight for quantity, weight in zip(vector, self.resolved_weights)
        )
        if value > U128_MAX:
            raise AssertionError("terminal numerator exceeds u128")
        return value

    def assert_invariants(self) -> None:
        n = self.descriptor.basis.outcome_count
        if len(self.vault) != n:
            raise AssertionError("vault width")
        if any(value < 0 or value > U64_MAX for value in self.vault):
            raise AssertionError("vault amount range")
        if self.wrapper_supply != sum(self.wrapper_balances.values()):
            raise AssertionError("Token-2022 mint supply is not holder-balance sum")
        if self.wrapper_supply < 0 or self.wrapper_supply > U64_MAX:
            raise AssertionError("wrapper supply range")
        for index, coefficient in enumerate(self.descriptor.coefficients):
            required = self.wrapper_supply * coefficient
            if required > U64_MAX or self.vault[index] < required:
                raise AssertionError("wrapper is not exactly overcollateralized by basis Eggs")
        totals = self.total_basis_supply
        if self.resolved_weights is None:
            # Partition of unity makes max_i(T_i) the full-simplex liability.
            if self.hoard_atoms < max(totals, default=0):
                raise AssertionError("unresolved base market is insolvent")
        else:
            numerator = self._terminal_numerator(totals)
            # Compare exact rationals without silently rounding a payout.
            if self.hoard_atoms * self.descriptor.basis.denominator < numerator:
                raise AssertionError("resolved base market is insolvent")
        if self.retired and (self.wrapper_supply != 0 or any(self.vault)):
            raise AssertionError("retired wrapper retains claims")

    def snapshot(self) -> tuple[object, ...]:
        return (
            tuple((owner, tuple(values)) for owner, values in sorted(self.basis_balances.items())),
            tuple(sorted(self.wrapper_balances.items())),
            tuple(self.other_basis),
            tuple(self.vault),
            self.wrapper_supply,
            self.hoard_atoms,
            self.resolved_weights,
            self.retired,
        )

    def _owner(self, owner: str) -> None:
        if owner not in self.basis_balances:
            self.basis_balances[owner] = [0] * self.descriptor.basis.outcome_count
            self.wrapper_balances[owner] = 0

    def merge_components(self, owner: str, quantity: int) -> None:
        """Escrow ``quantity * coefficients`` and mint ``quantity`` wrappers."""

        if self.retired:
            raise Refusal("wrapper is retired")
        quantity = _u64(quantity, "quantity")
        if quantity == 0:
            raise Refusal("zero quantity")
        self._owner(owner)
        required = [
            _checked_mul(quantity, coefficient, "component quantity")
            for coefficient in self.descriptor.coefficients
        ]
        if any(
            self.basis_balances[owner][index] < amount
            for index, amount in enumerate(required)
        ):
            raise Refusal("insufficient native basis Eggs")
        new_supply = _checked_add(self.wrapper_supply, quantity, "wrapper supply")
        new_wrapper_balance = _checked_add(
            self.wrapper_balances[owner], quantity, "wrapper balance"
        )
        new_vault = [
            _checked_add(self.vault[index], amount, "vault balance")
            for index, amount in enumerate(required)
        ]
        for index, amount in enumerate(required):
            self.basis_balances[owner][index] -= amount
        self.vault = new_vault
        self.wrapper_supply = new_supply
        self.wrapper_balances[owner] = new_wrapper_balance
        self.assert_invariants()

    def split_components(self, owner: str, quantity: int) -> None:
        """Burn wrappers and release the exact native basis Egg basket."""

        if self.retired:
            raise Refusal("wrapper is retired")
        quantity = _u64(quantity, "quantity")
        if quantity == 0:
            raise Refusal("zero quantity")
        self._owner(owner)
        if self.wrapper_balances[owner] < quantity:
            raise Refusal("insufficient wrapper balance")
        released = [
            _checked_mul(quantity, coefficient, "component quantity")
            for coefficient in self.descriptor.coefficients
        ]
        if any(self.vault[index] < amount for index, amount in enumerate(released)):
            raise Refusal("vault coverage failure")
        new_basis = [
            _checked_add(self.basis_balances[owner][index], amount, "owner basis balance")
            for index, amount in enumerate(released)
        ]
        self.wrapper_balances[owner] -= quantity
        self.wrapper_supply -= quantity
        for index, amount in enumerate(released):
            self.vault[index] -= amount
        self.basis_balances[owner] = new_basis
        self.assert_invariants()

    def transfer_wrapper(self, source: str, destination: str, quantity: int) -> None:
        """Model an ordinary Token-2022 bearer transfer."""

        quantity = _u64(quantity, "quantity")
        if quantity == 0:
            raise Refusal("zero quantity")
        self._owner(source)
        self._owner(destination)
        if self.wrapper_balances[source] < quantity:
            raise Refusal("insufficient wrapper balance")
        destination_after = _checked_add(
            self.wrapper_balances[destination], quantity, "destination wrapper balance"
        )
        self.wrapper_balances[source] -= quantity
        self.wrapper_balances[destination] = destination_after
        self.assert_invariants()

    def direct_burn_wrapper(self, owner: str, quantity: int) -> None:
        """Ordinary holder burn: a donation that leaves surplus backing."""

        quantity = _u64(quantity, "quantity")
        if quantity == 0:
            raise Refusal("zero quantity")
        self._owner(owner)
        if self.wrapper_balances[owner] < quantity:
            raise Refusal("insufficient wrapper balance")
        self.wrapper_balances[owner] -= quantity
        self.wrapper_supply -= quantity
        self.assert_invariants()

    def donate_components(self, owner: str, amounts: Sequence[int]) -> None:
        """Transfer arbitrary native Eggs into the PDA vault without minting."""

        if len(amounts) != self.descriptor.basis.outcome_count:
            raise Refusal("donation width mismatch")
        self._owner(owner)
        checked = [
            _u64(value, f"donation[{index}]") for index, value in enumerate(amounts)
        ]
        if any(
            self.basis_balances[owner][index] < amount
            for index, amount in enumerate(checked)
        ):
            raise Refusal("insufficient donation balance")
        new_vault = [
            _checked_add(self.vault[index], amount, "vault donation")
            for index, amount in enumerate(checked)
        ]
        for index, amount in enumerate(checked):
            self.basis_balances[owner][index] -= amount
        self.vault = new_vault
        self.assert_invariants()

    def compact_surplus(self) -> tuple[int, ...]:
        """Burn every vault atom not needed to cover actual wrapper supply.

        No caller receives the surplus.  It is an underlying-claim donation and
        can only reduce the base market's liability.  A production adapter must
        authenticate post-CPI mint supply and exact vault deltas.
        """

        surplus = tuple(
            self.vault[index] - self.wrapper_supply * coefficient
            for index, coefficient in enumerate(self.descriptor.coefficients)
        )
        for index, amount in enumerate(surplus):
            self.vault[index] -= amount
        # The modeled Token-2022 Egg burns reduce aggregate basis supply because
        # total_basis_supply is computed directly from the owner/vault ledgers.
        self.assert_invariants()
        return surplus

    def resolve(self, weights: Sequence[int]) -> None:
        if self.resolved_weights is not None:
            raise Refusal("market already resolved")
        checked = tuple(_u64(value, "weight") for value in weights)
        if len(checked) != self.descriptor.basis.outcome_count:
            raise Refusal("weight width mismatch")
        if sum(checked) != self.descriptor.basis.denominator:
            raise Refusal("weights do not sum to denominator")
        self.resolved_weights = checked
        self.assert_invariants()

    def redeem_terminal(self, owner: str, quantity: int) -> int:
        """Optional atomic aggregate redemption, exact-or-refuse.

        This requires a base-program aggregate redemption CPI that does not yet
        follow merely from the wrapper model.  It is included to exercise the
        arithmetic and to show why silent flooring is unnecessary.
        """

        if self.resolved_weights is None:
            raise Refusal("market is unresolved")
        quantity = _u64(quantity, "quantity")
        if quantity == 0:
            raise Refusal("zero quantity")
        self._owner(owner)
        if self.wrapper_balances[owner] < quantity:
            raise Refusal("insufficient wrapper balance")
        consumed = [
            _checked_mul(quantity, coefficient, "component redemption")
            for coefficient in self.descriptor.coefficients
        ]
        numerator_per_wrapper = sum(
            coefficient * weight
            for coefficient, weight in zip(
                self.descriptor.coefficients, self.resolved_weights
            )
        )
        numerator = quantity * numerator_per_wrapper
        if numerator > U128_MAX:
            raise Refusal("redemption numerator overflows u128")
        denominator = self.descriptor.basis.denominator
        if numerator % denominator != 0:
            raise Refusal("redemption quantity is not an exact lot")
        payout = numerator // denominator
        if payout > self.hoard_atoms:
            raise Refusal("insufficient Hoard collateral")
        if any(self.vault[index] < amount for index, amount in enumerate(consumed)):
            raise Refusal("vault coverage failure")
        self.wrapper_balances[owner] -= quantity
        self.wrapper_supply -= quantity
        for index, amount in enumerate(consumed):
            self.vault[index] -= amount
        self.hoard_atoms -= payout
        self.assert_invariants()
        return payout

    def unauthorized_mint(self, owner: str, quantity: int) -> None:
        del owner, quantity
        raise Refusal("only the canonical wrapper-authority PDA may mint")

    def unauthorized_vault_withdrawal(self, amounts: Sequence[int]) -> None:
        del amounts
        raise Refusal("the wrapper vault has no delegate or external authority")

    def retire(self) -> None:
        if self.wrapper_supply != 0 or any(self.vault):
            raise Refusal("nonempty wrapper cannot retire")
        self.retired = True
        self.assert_invariants()


# Resource estimates use Solana's documented/default Rent parameters.  They are
# design estimates, not measurements of a compiled instruction.
ACCOUNT_STORAGE_OVERHEAD = 128
DEFAULT_LAMPORTS_PER_BYTE_YEAR = 3_480
DEFAULT_EXEMPTION_THRESHOLD = 2
WRAPPER_DESCRIPTOR_BYTES = 272
TOKEN_2022_MINT_BYTES = 82
IMMUTABLE_OWNER_TOKEN_ACCOUNT_BYTES = 170
BASE_POSITION_BYTES = 220
BASE_REPLAY_BYTES = 84
CLAIM_POSITION_BYTES = 112


def rent_exempt_lamports(space: int) -> int:
    if space < 0:
        raise Refusal("negative account space")
    return (
        (ACCOUNT_STORAGE_OVERHEAD + space)
        * DEFAULT_LAMPORTS_PER_BYTE_YEAR
        * DEFAULT_EXEMPTION_THRESHOLD
    )


@dataclass(frozen=True)
class ResourceEstimate:
    outcomes: int
    support: int
    infrastructure_lamports: int
    holder_lamports: int
    wrap_accounts: int
    wrap_cpis: int


def external_vault_estimate(outcomes: int, support: int | None = None) -> ResourceEstimate:
    """Per-mint Token-2022 escrow accounts, using TransferChecked per component."""

    if outcomes < 2 or outcomes > MAX_OUTCOMES:
        raise Refusal("outcomes must be in 2..=16")
    support = outcomes if support is None else support
    if support < 2 or support > outcomes:
        raise Refusal("support must be in 2..=outcomes")
    infrastructure = rent_exempt_lamports(WRAPPER_DESCRIPTOR_BYTES)
    infrastructure += rent_exempt_lamports(TOKEN_2022_MINT_BYTES)
    infrastructure += support * rent_exempt_lamports(IMMUTABLE_OWNER_TOKEN_ACCOUNT_BYTES)
    return ResourceEstimate(
        outcomes=outcomes,
        support=support,
        infrastructure_lamports=infrastructure,
        holder_lamports=rent_exempt_lamports(IMMUTABLE_OWNER_TOKEN_ACCOUNT_BYTES),
        # actor, descriptor, wrapper mint, wrapper account, Token-2022 program,
        # then source account + mint + vault for each nonzero component.
        wrap_accounts=5 + 3 * support,
        # support TransferChecked CPIs and one MintToChecked/BurnChecked CPI.
        wrap_cpis=support + 1,
    )


def internal_position_estimate(outcomes: int, support: int | None = None) -> ResourceEstimate:
    """One dedicated base Position/Replay vault plus an ordinary wrapper mint."""

    if outcomes < 2 or outcomes > MAX_OUTCOMES:
        raise Refusal("outcomes must be in 2..=16")
    support = outcomes if support is None else support
    if support < 2 or support > outcomes:
        raise Refusal("support must be in 2..=outcomes")
    infrastructure = sum(
        rent_exempt_lamports(space)
        for space in (
            WRAPPER_DESCRIPTOR_BYTES,
            TOKEN_2022_MINT_BYTES,
            BASE_POSITION_BYTES,
            BASE_REPLAY_BYTES,
        )
    )
    return ResourceEstimate(
        outcomes=outcomes,
        support=support,
        infrastructure_lamports=infrastructure,
        holder_lamports=rent_exempt_lamports(IMMUTABLE_OWNER_TOKEN_ACCOUNT_BYTES),
        # Stable upper-level estimate: actor, descriptor, wrapper mint/account,
        # two programs, market, two Position/Replay pairs, and authority.
        wrap_accounts=12,
        # One base atomic-vector-transfer CPI plus one Token-2022 mint/burn CPI.
        wrap_cpis=2,
    )


def position_only_estimate(outcomes: int, support: int | None = None) -> ResourceEstimate:
    """Program-owned atomic claim ledger, with no Token-2022 wrapper mint."""

    if outcomes < 2 or outcomes > MAX_OUTCOMES:
        raise Refusal("outcomes must be in 2..=16")
    support = outcomes if support is None else support
    if support < 2 or support > outcomes:
        raise Refusal("support must be in 2..=outcomes")
    infrastructure = sum(
        rent_exempt_lamports(space)
        for space in (WRAPPER_DESCRIPTOR_BYTES, BASE_POSITION_BYTES, BASE_REPLAY_BYTES)
    )
    return ResourceEstimate(
        outcomes=outcomes,
        support=support,
        infrastructure_lamports=infrastructure,
        holder_lamports=rent_exempt_lamports(CLAIM_POSITION_BYTES),
        wrap_accounts=8,
        wrap_cpis=1,
    )


def simplex_vectors(parts: int, total: int) -> Iterable[tuple[int, ...]]:
    """Enumerate small integer simplex vectors for executable theorem tests."""

    if parts <= 0 or total < 0:
        raise Refusal("invalid simplex dimensions")
    if parts == 1:
        yield (total,)
        return
    for first in range(total + 1):
        for rest in simplex_vectors(parts - 1, total - first):
            yield (first,) + rest
