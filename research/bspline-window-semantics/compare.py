#!/usr/bin/env python3
"""Small exact, reproducible comparison of the candidate path semantics."""

from model import BasisSpec, compare_path_modes


def main() -> None:
    spec = BasisSpec(2, (0, 16, 32), 64)
    path = ((0, 1, 4), (1, 1, 28))
    result = compare_path_modes(spec, path)
    print("basis: degree=2 knots=(0,16,32) D=64")
    print("path: 1*4, 1*28; exact TWAP=16")
    print("evaluate-at-TWAP:          ", result.evaluate_at_twap)
    print("quantized-basis occupation:", result.quantized_basis_occupation)
    print("exact-basis occupation:    ", result.exact_basis_occupation)


if __name__ == "__main__":
    main()
