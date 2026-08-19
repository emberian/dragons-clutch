"""Print the bounded wrapper resource comparison and lot example."""

from model import (
    external_vault_estimate,
    internal_position_estimate,
    position_only_estimate,
    resolved_redemption_lot,
    universal_redemption_lot,
)


def sol(lamports: int) -> str:
    return f"{lamports / 1_000_000_000:.9f}"


print("outcomes external-vault-SOL internal-position-SOL position-only-SOL external-accts/CPIs")
for outcomes in (2, 4, 8, 16):
    external = external_vault_estimate(outcomes)
    internal = internal_position_estimate(outcomes)
    position = position_only_estimate(outcomes)
    print(
        f"{outcomes:>8} {sol(external.infrastructure_lamports):>18} "
        f"{sol(internal.infrastructure_lamports):>21} "
        f"{sol(position.infrastructure_lamports):>17} "
        f"{external.wrap_accounts:>2}/{external.wrap_cpis:<2}"
    )

coefficients = (1, 2, 4)
weights = (1, 4, 1)
denominator = 6
print()
print(f"coefficients: {coefficients}")
print(f"universal exact redemption lot: {universal_redemption_lot(coefficients, denominator)}")
print(
    "resolved exact redemption lot: "
    f"{resolved_redemption_lot(coefficients, weights, denominator)}"
)
