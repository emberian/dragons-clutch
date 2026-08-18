# Collateral-profile laboratory

Status: deterministic, host-only policy experiment (2026-08-18).

This laboratory makes the V1 Realm collateral policy explicit. It accepts both
legacy SPL Token and base Token-2022 mints, but it is intentionally conservative
about extensions: no mint extension is admitted, and only `ImmutableOwner` is
admitted on Token-2022 Hoard accounts. Unknown extensions and future
discriminants fail closed.

The model defines:

- a 266-byte canonical Realm profile and domain-separated SHA-256 digest;
- independent collateral, fee, and liveness currency identities;
- immutable checks for mint identity, token-program owner, decimals, supply
  ceiling, mint authority, freeze authority, and extension sets;
- Hoard-account checks for state, owner authority, delegate, close authority,
  and account extensions;
- a DREGG dogfood constructor using the same generic profile type; and
- golden encodings plus adversarial decision vectors.

Run the complete deterministic suite from this directory:

```sh
python3 -m unittest -v
python3 run_lab.py
```

Or from the repository root:

```sh
python3 -m unittest discover -s research/collateral-profiles -p 'test_*.py'
python3 research/collateral-profiles/run_lab.py
```

These programs have no RPC, wallet, key, signing, submission, deployment, CPI,
or dependency-install path. The snapshots are typed test inputs, not Solana
account parsers. Passing them does not establish routeability, runtime safety,
or chain readiness. See [the implementation note](../../docs/implementation/COLLATERAL_PROFILES.md)
and [the pinned source inventory](SOURCES.md).

