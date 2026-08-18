# SPDX-License-Identifier: AGPL-3.0-or-later
from __future__ import annotations

import json
import unittest
from dataclasses import replace
from hashlib import sha256
from pathlib import Path

from model import (
    ACCOUNT_EXTENSIONS,
    DIGEST_BYTES,
    DREGG_MINT,
    DREGG_MINT_TEXT,
    EXTENSION_REFUSAL_REASON,
    LEGACY_TOKEN_PROGRAM,
    MINT_EXTENSIONS,
    PARENT_PROFILE_BYTES,
    PARENT_PROFILE_DOMAIN,
    PARENT_PROFILE_MAGIC,
    PARENT_PROFILE_RESERVED_BYTES,
    PARENT_PROFILE_SCHEMA_VERSION,
    PROFILE_DOMAIN,
    PROFILE_RESERVED_BYTES,
    PROFILE_SCHEMA_VERSION,
    PROTOCOL_ACCOUNT_EXTENSION_CEILING,
    SUBFIELD_COLLATERAL_POLICY,
    U64_MAX,
    CurrencyRef,
    ExtensionType,
    MintSnapshot,
    ModelError,
    ProfileIdentity,
    RealmCollateralProfile,
    RefusalCode,
    TOKEN_2022_PROGRAM,
    TokenAccountSnapshot,
    compose_profile_identity,
    decode_base58_pubkey,
    dregg_dogfood_profile,
    extension_mask,
    validate_hoard_account,
    validate_mint,
    verify_profile_identity,
)


OWNER = bytes.fromhex("ab" * 32)
MINT = bytes.fromhex("cd" * 32)


def token_2022_profile() -> RealmCollateralProfile:
    collateral = CurrencyRef.spl(TOKEN_2022_PROGRAM, MINT, 6)
    return RealmCollateralProfile(
        collateral=collateral,
        fee_currency=collateral,
        liveness_currency=CurrencyRef.native_sol(),
        max_supply_atoms=1_000_000_000_000_000,
    )


def good_mint(profile: RealmCollateralProfile) -> MintSnapshot:
    return MintSnapshot(
        token_program=profile.collateral.token_program,
        mint=profile.collateral.mint,
        initialized=True,
        decimals=profile.collateral.decimals,
        supply_atoms=1_000_000,
        mint_authority=None,
        freeze_authority=None,
    )


def good_account(profile: RealmCollateralProfile) -> TokenAccountSnapshot:
    return TokenAccountSnapshot(
        token_program=profile.collateral.token_program,
        mint=profile.collateral.mint,
        owner_authority=OWNER,
        initialized=True,
        frozen=False,
        amount_atoms=123,
        delegate=None,
        close_authority=None,
    )


class CanonicalProfileTests(unittest.TestCase):
    def test_exact_round_trip_and_digest(self) -> None:
        profile = token_2022_profile()
        encoded = profile.canonical_bytes()
        self.assertEqual(len(encoded), 266)
        self.assertEqual(RealmCollateralProfile.from_canonical_bytes(encoded), profile)
        self.assertEqual(
            profile.digest_hex(),
            "aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c32",
        )

    def test_nonzero_reserved_bytes_refused(self) -> None:
        raw = bytearray(token_2022_profile().canonical_bytes())
        raw[-PROFILE_RESERVED_BYTES] = 1
        with self.assertRaisesRegex(ModelError, "reserved bytes"):
            RealmCollateralProfile.from_canonical_bytes(bytes(raw))

    def test_wrong_length_and_magic_refused(self) -> None:
        raw = token_2022_profile().canonical_bytes()
        with self.assertRaisesRegex(ModelError, "exactly 266"):
            RealmCollateralProfile.from_canonical_bytes(raw[:-1])
        with self.assertRaisesRegex(ModelError, "magic"):
            RealmCollateralProfile.from_canonical_bytes(b"x" + raw[1:])

    def test_currency_roles_are_explicit_and_digest_bound(self) -> None:
        profile = token_2022_profile()
        sol_fee = replace(profile, fee_currency=CurrencyRef.native_sol())
        self.assertNotEqual(profile.canonical_bytes(), sol_fee.canonical_bytes())
        self.assertNotEqual(profile.digest(), sol_fee.digest())
        self.assertEqual(profile.fee_currency, profile.collateral)
        self.assertNotEqual(profile.liveness_currency, profile.collateral)

    def test_realm_cannot_expand_protocol_extension_ceiling(self) -> None:
        profile = token_2022_profile()
        with self.assertRaisesRegex(ModelError, "cannot expand"):
            replace(
                profile,
                allowed_mint_extensions=extension_mask(
                    {ExtensionType.METADATA_POINTER}
                ),
            )
        with self.assertRaisesRegex(ModelError, "cannot expand"):
            replace(
                profile,
                allowed_account_extensions=extension_mask({ExtensionType.CPI_GUARD}),
            )

    def test_realm_cannot_weaken_strict_policy_or_add_unprofiled_currencies(
        self,
    ) -> None:
        profile = token_2022_profile()
        with self.assertRaisesRegex(ModelError, "cannot weaken"):
            replace(profile, flags=0)
        other = CurrencyRef.spl(TOKEN_2022_PROGRAM, bytes.fromhex("ef" * 32), 6)
        with self.assertRaisesRegex(ModelError, "fee currency"):
            replace(profile, fee_currency=other)
        with self.assertRaisesRegex(ModelError, "liveness currency"):
            replace(profile, liveness_currency=profile.collateral)


class MintValidationTests(unittest.TestCase):
    def test_base_token_2022_mint_is_accepted(self) -> None:
        profile = token_2022_profile()
        self.assertEqual(
            validate_mint(profile, good_mint(profile)).code, RefusalCode.ACCEPT
        )

    def test_authority_supply_and_decimal_refusals(self) -> None:
        profile = token_2022_profile()
        cases = (
            (replace(good_mint(profile), decimals=9), RefusalCode.WRONG_DECIMALS),
            (replace(good_mint(profile), supply_atoms=0), RefusalCode.ZERO_SUPPLY),
            (
                replace(good_mint(profile), supply_atoms=profile.max_supply_atoms + 1),
                RefusalCode.SUPPLY_EXCEEDS_PROFILE,
            ),
            (
                replace(good_mint(profile), mint_authority=bytes.fromhex("11" * 32)),
                RefusalCode.MINT_AUTHORITY_PRESENT,
            ),
            (
                replace(good_mint(profile), freeze_authority=bytes.fromhex("22" * 32)),
                RefusalCode.FREEZE_AUTHORITY_PRESENT,
            ),
        )
        for snapshot, expected in cases:
            with self.subTest(expected=expected):
                self.assertEqual(validate_mint(profile, snapshot).code, expected)

    def test_identity_and_initialization_refusals(self) -> None:
        profile = token_2022_profile()
        cases = (
            (
                replace(good_mint(profile), token_program=LEGACY_TOKEN_PROGRAM),
                RefusalCode.WRONG_PROGRAM,
            ),
            (
                replace(good_mint(profile), mint=bytes.fromhex("98" * 32)),
                RefusalCode.WRONG_MINT,
            ),
            (replace(good_mint(profile), initialized=False), RefusalCode.UNINITIALIZED),
        )
        for snapshot, expected in cases:
            with self.subTest(expected=expected):
                self.assertEqual(validate_mint(profile, snapshot).code, expected)

    def test_every_current_mint_extension_is_refused(self) -> None:
        profile = token_2022_profile()
        self.assertGreaterEqual(len(MINT_EXTENSIONS), 19)
        for extension in MINT_EXTENSIONS:
            with self.subTest(extension=extension.name):
                result = validate_mint(
                    profile, replace(good_mint(profile), extensions=(int(extension),))
                )
                self.assertEqual(result.code, RefusalCode.EXTENSION_NOT_ALLOWED)
                self.assertEqual(result.detail, EXTENSION_REFUSAL_REASON[extension])

    def test_unknown_duplicate_and_wrong_location_extensions_refused(self) -> None:
        profile = token_2022_profile()
        self.assertEqual(
            validate_mint(
                profile, replace(good_mint(profile), extensions=(65_535,))
            ).code,
            RefusalCode.UNKNOWN_EXTENSION,
        )
        self.assertEqual(
            validate_mint(profile, replace(good_mint(profile), extensions=(1, 1))).code,
            RefusalCode.MALFORMED_EXTENSION_SET,
        )
        self.assertEqual(
            validate_mint(
                profile,
                replace(
                    good_mint(profile), extensions=(int(ExtensionType.IMMUTABLE_OWNER),)
                ),
            ).code,
            RefusalCode.WRONG_EXTENSION_LOCATION,
        )


class AccountValidationTests(unittest.TestCase):
    def test_base_and_immutable_owner_accounts_are_accepted(self) -> None:
        profile = token_2022_profile()
        account = good_account(profile)
        self.assertEqual(
            validate_hoard_account(profile, account, OWNER).code, RefusalCode.ACCEPT
        )
        immutable = replace(account, extensions=(int(ExtensionType.IMMUTABLE_OWNER),))
        self.assertEqual(
            validate_hoard_account(profile, immutable, OWNER).code, RefusalCode.ACCEPT
        )

    def test_all_other_current_account_extensions_are_refused(self) -> None:
        profile = token_2022_profile()
        denied = ACCOUNT_EXTENSIONS - PROTOCOL_ACCOUNT_EXTENSION_CEILING
        self.assertGreaterEqual(len(denied), 8)
        for extension in denied:
            with self.subTest(extension=extension.name):
                result = validate_hoard_account(
                    profile,
                    replace(good_account(profile), extensions=(int(extension),)),
                    OWNER,
                )
                self.assertEqual(result.code, RefusalCode.EXTENSION_NOT_ALLOWED)
                self.assertEqual(result.detail, EXTENSION_REFUSAL_REASON[extension])

    def test_account_authority_and_state_refusals(self) -> None:
        profile = token_2022_profile()
        cases = (
            (replace(good_account(profile), frozen=True), RefusalCode.FROZEN_ACCOUNT),
            (
                replace(
                    good_account(profile), owner_authority=bytes.fromhex("12" * 32)
                ),
                RefusalCode.WRONG_ACCOUNT_OWNER,
            ),
            (
                replace(good_account(profile), delegate=bytes.fromhex("13" * 32)),
                RefusalCode.DELEGATE_PRESENT,
            ),
            (
                replace(
                    good_account(profile), close_authority=bytes.fromhex("14" * 32)
                ),
                RefusalCode.CLOSE_AUTHORITY_PRESENT,
            ),
        )
        for snapshot, expected in cases:
            with self.subTest(expected=expected):
                self.assertEqual(
                    validate_hoard_account(profile, snapshot, OWNER).code, expected
                )

    def test_account_identity_and_initialization_refusals(self) -> None:
        profile = token_2022_profile()
        cases = (
            (
                replace(good_account(profile), token_program=LEGACY_TOKEN_PROGRAM),
                RefusalCode.WRONG_PROGRAM,
            ),
            (
                replace(good_account(profile), mint=bytes.fromhex("97" * 32)),
                RefusalCode.WRONG_MINT,
            ),
            (
                replace(good_account(profile), initialized=False),
                RefusalCode.UNINITIALIZED,
            ),
        )
        for snapshot, expected in cases:
            with self.subTest(expected=expected):
                self.assertEqual(
                    validate_hoard_account(profile, snapshot, OWNER).code, expected
                )


class DreggDogfoodTests(unittest.TestCase):
    def test_dregg_is_a_plain_legacy_profile_instance(self) -> None:
        profile = dregg_dogfood_profile(decimals=6, max_supply_atoms=10**15)
        self.assertEqual(profile.collateral.token_program, LEGACY_TOKEN_PROGRAM)
        self.assertEqual(profile.collateral.mint, DREGG_MINT)
        self.assertEqual(decode_base58_pubkey(DREGG_MINT_TEXT), DREGG_MINT)
        self.assertEqual(profile.fee_currency, profile.collateral)
        self.assertEqual(profile.liveness_currency, CurrencyRef.native_sol())
        self.assertEqual(
            profile.digest_hex(),
            "ef63ccd0c5e1616c1570dd96a985ef9924f622d44c246f5aa88e1b9545f54343",
        )

    def test_dregg_arguments_are_validated_not_asserted_as_chain_facts(self) -> None:
        with self.assertRaises(ModelError):
            dregg_dogfood_profile(decimals=-1, max_supply_atoms=10**15)
        with self.assertRaises(ModelError):
            dregg_dogfood_profile(decimals=6, max_supply_atoms=0)


class CheckedVectorTests(unittest.TestCase):
    def test_checked_vectors(self) -> None:
        vectors = json.loads((Path(__file__).parent / "vectors.json").read_text())
        profile = token_2022_profile()
        baseline = good_mint(profile)
        for vector in vectors["mint_vectors"]:
            extensions = tuple(vector.get("extensions", ()))
            snapshot = replace(
                baseline,
                decimals=vector.get("decimals", baseline.decimals),
                supply_atoms=vector.get("supply_atoms", baseline.supply_atoms),
                mint_authority=(
                    bytes.fromhex("44" * 32) if vector.get("mint_authority") else None
                ),
                freeze_authority=(
                    bytes.fromhex("55" * 32) if vector.get("freeze_authority") else None
                ),
                extensions=extensions,
            )
            result = validate_mint(profile, snapshot)
            self.assertEqual(result.code.name, vector["expected"], vector["name"])

        baseline_account = good_account(profile)
        for vector in vectors["account_vectors"]:
            snapshot = replace(
                baseline_account,
                frozen=vector.get("frozen", False),
                delegate=(bytes.fromhex("66" * 32) if vector.get("delegate") else None),
                close_authority=(
                    bytes.fromhex("77" * 32) if vector.get("close_authority") else None
                ),
                extensions=tuple(vector.get("extensions", ())),
            )
            result = validate_hoard_account(profile, snapshot, OWNER)
            self.assertEqual(result.code.name, vector["expected"], vector["name"])

    def test_every_pinned_extension_has_one_location_and_reason(self) -> None:
        self.assertEqual(
            MINT_EXTENSIONS | ACCOUNT_EXTENSIONS,
            set(ExtensionType) - {ExtensionType.UNINITIALIZED},
        )
        self.assertFalse(MINT_EXTENSIONS & ACCOUNT_EXTENSIONS)
        self.assertEqual(set(EXTENSION_REFUSAL_REASON), set(ExtensionType))


IDENTITY_BUILDERS = {
    "generic-token-2022": lambda: token_2022_profile(),
    "dregg-dogfood": lambda: dregg_dogfood_profile(
        decimals=6, max_supply_atoms=10**15
    ),
    "legacy-sol-fee": lambda: legacy_sol_fee_profile(),
}


def legacy_sol_fee_profile() -> RealmCollateralProfile:
    collateral = CurrencyRef.spl(LEGACY_TOKEN_PROGRAM, bytes.fromhex("ab" * 32), 9)
    return RealmCollateralProfile(
        collateral=collateral,
        fee_currency=CurrencyRef.native_sol(),
        liveness_currency=CurrencyRef.native_sol(),
        max_supply_atoms=U64_MAX,
        allowed_account_extensions=0,
    )


def identity_vectors() -> dict:
    return json.loads((Path(__file__).parent / "identity_vectors.json").read_text())


class ProfileIdentityTests(unittest.TestCase):
    """The P1-G parent/child join: the collateral digest is a subfield, not the ID."""

    def test_parent_bytes_round_trip_and_embed_the_child_digest(self) -> None:
        profile = token_2022_profile()
        identity = compose_profile_identity(profile)
        raw = identity.canonical_bytes()
        self.assertEqual(len(raw), PARENT_PROFILE_BYTES)
        self.assertEqual(raw[:8], PARENT_PROFILE_MAGIC)
        self.assertEqual(raw[8:10], PARENT_PROFILE_SCHEMA_VERSION.to_bytes(2, "little"))
        self.assertEqual(raw[10:12], bytes(2))
        self.assertEqual(
            raw[12:14], SUBFIELD_COLLATERAL_POLICY.to_bytes(2, "little")
        )
        self.assertEqual(raw[14:16], PROFILE_SCHEMA_VERSION.to_bytes(2, "little"))
        self.assertEqual(raw[16:48], profile.digest())
        self.assertEqual(raw[48:], bytes(PARENT_PROFILE_RESERVED_BYTES))
        self.assertEqual(ProfileIdentity.from_canonical_bytes(raw), identity)
        self.assertEqual(
            identity.digest(), sha256(PARENT_PROFILE_DOMAIN + raw).digest()
        )
        self.assertTrue(identity.binds(profile))

    def test_the_collateral_digest_is_not_the_profile_id(self) -> None:
        profile = token_2022_profile()
        identity = compose_profile_identity(profile)
        self.assertNotEqual(identity.digest(), profile.digest())
        self.assertEqual(len(identity.digest()), DIGEST_BYTES)

    def test_every_collateral_field_change_moves_the_parent_identity(self) -> None:
        base = token_2022_profile()
        base_identity = compose_profile_identity(base).digest()
        variants = [
            replace(base, max_supply_atoms=base.max_supply_atoms - 1),
            replace(
                base,
                collateral=CurrencyRef.spl(
                    TOKEN_2022_PROGRAM, base.collateral.mint, 7
                ),
                fee_currency=CurrencyRef.spl(
                    TOKEN_2022_PROGRAM, base.collateral.mint, 7
                ),
            ),
            replace(base, fee_currency=CurrencyRef.native_sol()),
            replace(base, allowed_account_extensions=0),
        ]
        seen = {base_identity}
        for variant in variants:
            digest = compose_profile_identity(variant).digest()
            self.assertNotIn(digest, seen)
            seen.add(digest)

    def test_malformed_parent_identities_are_refused(self) -> None:
        digest = token_2022_profile().digest()
        with self.assertRaises(ModelError):
            ProfileIdentity(collateral_policy_digest=bytes(DIGEST_BYTES))
        with self.assertRaises(ModelError):
            ProfileIdentity(collateral_policy_digest=digest[:31])
        with self.assertRaises(ModelError):
            ProfileIdentity(collateral_policy_digest=digest, subfield_tag=2)
        with self.assertRaises(ModelError):
            ProfileIdentity(collateral_policy_digest=digest, subfield_schema_version=2)
        with self.assertRaises(ModelError):
            ProfileIdentity(collateral_policy_digest=digest, schema_version=2)
        with self.assertRaises(ModelError):
            ProfileIdentity(collateral_policy_digest=digest, flags=1)

    def test_a_wellformed_parent_can_still_bind_the_wrong_policy(self) -> None:
        generic = token_2022_profile()
        other = dregg_dogfood_profile(decimals=6, max_supply_atoms=10**15)
        foreign = compose_profile_identity(other)
        # It decodes: well-formedness is not evidence of the right subfield.
        self.assertEqual(
            ProfileIdentity.from_canonical_bytes(foreign.canonical_bytes()), foreign
        )
        self.assertFalse(foreign.binds(generic))
        with self.assertRaises(ModelError):
            verify_profile_identity(generic, foreign.canonical_bytes())
        self.assertEqual(
            verify_profile_identity(other, foreign.canonical_bytes()), foreign
        )


class ProfileIdentityVectorTests(unittest.TestCase):
    def test_positive_vectors_recompute_from_the_model(self) -> None:
        vectors = identity_vectors()
        self.assertEqual(vectors["schema"], "dragons-clutch/profile-identity-v1")
        self.assertEqual(vectors["manifest"]["network_actions"], 0)
        self.assertEqual(vectors["manifest"]["parent_bytes"], PARENT_PROFILE_BYTES)
        self.assertEqual(vectors["manifest"]["child_bytes"], 266)
        self.assertEqual(len(vectors["positive_vectors"]), len(IDENTITY_BUILDERS))
        for vector in vectors["positive_vectors"]:
            profile = IDENTITY_BUILDERS[vector["builder"]]()
            identity = compose_profile_identity(profile)
            self.assertEqual(
                profile.canonical_bytes().hex(),
                vector["collateral_profile_bytes"],
                vector["name"],
            )
            self.assertEqual(profile.digest_hex(), vector["child_digest"], vector["name"])
            self.assertEqual(
                identity.canonical_bytes().hex(), vector["parent_bytes"], vector["name"]
            )
            self.assertEqual(
                identity.digest_hex(), vector["parent_digest"], vector["name"]
            )
            embedded = bytes.fromhex(vector["parent_bytes"])[16:48]
            self.assertEqual(embedded.hex(), vector["child_digest"], vector["name"])
            self.assertNotEqual(vector["parent_digest"], vector["child_digest"])
        names = [vector["parent_digest"] for vector in vectors["positive_vectors"]]
        self.assertEqual(len(set(names)), len(names))

    def test_decode_refusal_vectors_all_fail_closed(self) -> None:
        vectors = identity_vectors()
        self.assertTrue(vectors["decode_refusals"])
        for vector in vectors["decode_refusals"]:
            raw = bytes.fromhex(vector["parent_bytes"])
            with self.assertRaises(ModelError, msg=vector["name"]):
                ProfileIdentity.from_canonical_bytes(raw)
            with self.assertRaises(ModelError, msg=vector["name"]):
                verify_profile_identity(token_2022_profile(), raw)

    def test_binding_refusal_vectors_decode_but_do_not_bind(self) -> None:
        vectors = identity_vectors()
        profile = token_2022_profile()
        self.assertTrue(vectors["binding_refusals"])
        for vector in vectors["binding_refusals"]:
            raw = bytes.fromhex(vector["parent_bytes"])
            identity = ProfileIdentity.from_canonical_bytes(raw)
            self.assertFalse(identity.binds(profile), vector["name"])
            with self.assertRaises(ModelError, msg=vector["name"]):
                verify_profile_identity(profile, raw)
            self.assertNotEqual(
                identity.digest_hex(),
                compose_profile_identity(profile).digest_hex(),
                vector["name"],
            )

    def test_domain_separation_vectors_stay_distinct(self) -> None:
        vectors = identity_vectors()
        profile = token_2022_profile()
        identity = compose_profile_identity(profile)
        parent_raw = identity.canonical_bytes()
        child_raw = profile.canonical_bytes()
        recomputed = {
            "child-domain-over-parent-bytes": sha256(
                PROFILE_DOMAIN + parent_raw
            ).hexdigest(),
            "parent-domain-over-child-bytes": sha256(
                PARENT_PROFILE_DOMAIN + child_raw
            ).hexdigest(),
            "undomained-parent-bytes": sha256(parent_raw).hexdigest(),
            "parent-domain-with-separator-byte": sha256(
                PARENT_PROFILE_DOMAIN + bytes(1) + parent_raw
            ).hexdigest(),
        }
        self.assertEqual(
            {vector["name"] for vector in vectors["domain_separation"]},
            set(recomputed),
        )
        forbidden = {identity.digest_hex(), profile.digest_hex()}
        seen = set()
        for vector in vectors["domain_separation"]:
            self.assertEqual(
                recomputed[vector["name"]], vector["digest"], vector["name"]
            )
            self.assertNotIn(vector["digest"], forbidden, vector["name"])
            self.assertNotIn(vector["digest"], seen, vector["name"])
            seen.add(vector["digest"])


if __name__ == "__main__":
    unittest.main()
