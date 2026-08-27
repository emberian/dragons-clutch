#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Print deterministic collateral-profile evidence; never accesses a network."""

from model import (
    CurrencyRef,
    PARENT_PROFILE_BYTES,
    RealmCollateralProfile,
    TOKEN_2022_PROGRAM,
    compose_profile_identity,
    dregg_dogfood_profile,
)


def main() -> None:
    synthetic_mint = bytes.fromhex("cd" * 32)
    collateral = CurrencyRef.spl(TOKEN_2022_PROGRAM, synthetic_mint, 6)
    generic = RealmCollateralProfile(
        collateral=collateral,
        fee_currency=collateral,
        liveness_currency=CurrencyRef.native_sol(),
        max_supply_atoms=1_000_000_000_000_000,
    )
    dregg = dregg_dogfood_profile(decimals=6, max_supply_atoms=10**15)
    generic_identity = compose_profile_identity(generic)
    dregg_identity = compose_profile_identity(dregg)
    print(f"generic_profile_bytes={len(generic.canonical_bytes())}")
    print(f"generic_profile_digest={generic.digest_hex()}")
    print(f"dregg_dogfood_digest={dregg.digest_hex()}")
    print(f"parent_profile_bytes={PARENT_PROFILE_BYTES}")
    print(f"generic_parent_identity={generic_identity.digest_hex()}")
    print(f"dregg_parent_identity={dregg_identity.digest_hex()}")
    print("network_actions=0")


if __name__ == "__main__":
    main()
