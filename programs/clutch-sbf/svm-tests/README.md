# `clutch-sbf/svm-tests` — the Token-2022 leg, against a real bank

Status: **program-test evidence, 2026-08-18.** Not a cluster, not an audit, not
authorization to deploy anywhere.

This workspace drives the **real** `clutch_sbf` ELF — the artifact
`cargo-build-sbf` produces — against the **real** Token-2022 program, on an
in-process Agave bank. There is no native processor, no model of the program,
and no model of the token program: the bank executes SBF bytecode and every
balance asserted below was computed by `spl_token_2022-10.0.0.so`.

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh
```

## Why it is its own workspace

Two independent reasons, both recorded in
[`docs/implementation/TOKEN2022_PLAN.md`](../../../docs/implementation/TOKEN2022_PLAN.md) §1.2:

* the Agave 4.2.1 runtime needs Rust ≥ 1.93 (`MaybeUninit::write_copy_of_slice`
  stabilizes there) and `programs/clutch-sbf` is built by the repository's
  1.89.0 host pin; and
* `cargo-build-sbf` runs `cargo metadata` for every target and needs a `.crate`
  archive for every package in the graph, which a 731-package Agave graph does
  not have on this host.

`rust-toolchain.toml` therefore pins 1.93.1, and `solana-program-test` is taken
with `agave-unstable-api`, a feature name that should be read as the stability
warning it is.

It **does** depend on `clutch-sbf` itself, on purpose: the seed prefixes, the
refusal codes, and the account-list indices come from the program rather than
from a second hand-kept copy. The only thing that cannot be shared is
off-chain program-address derivation, and the bank is what proves the two
agree — a mismatch is `WrongPda` on the first transaction.

## What the scenarios establish

| scenario | claim |
| --- | --- |
| `e1_materialize_mints_exactly_q_and_the_shadow_reconciles` | `Materialize` of *q* raises the outcome mint's supply by exactly *q*, credits the destination exactly *q*, lowers the internal term by exactly *q*, and leaves the market-wide external term equal to the mint's supply |
| `e1_dematerialize_burns_exactly_and_the_shadow_reconciles` | the exact inverse, through a real `Burn` |
| `a_supply_that_drifted_outside_the_program_is_refused` | a holder burning outcome tokens *outside* the program makes the two truths disagree, and the next seam instruction refuses `ShadowSupplyMismatch` |
| `an_extension_on_the_outcome_mint_is_refused_at_instruction_time` | `TransferFeeConfig` and `MintCloseAuthority` on the mint are refused when the instruction runs, not only when a market is founded |
| `the_derived_hoard_authority_cannot_be_signed_for_by_a_wallet` | a deposit into an account owned by `seeds::hoard_authority_pda` needs only the depositor's signature; taking it out with the same signature is `TokenError::OwnerMismatch` |
| `the_ten_account_plane_moves_only_the_shadow` | the transitional optional-token-leg hole, measured: the shadow-only plane is still accepted and produces exactly the divergence the token plane refuses to build on |

## What it does not establish

An in-process bank is not a cluster. Transaction replay, durable nonces,
instruction duplication inside one transaction, batch retries, fee payment,
rent collection over time, and program upgrade are all outside what
`solana-program-test` can show. `TOKEN2022_PLAN.md` §4 lists them, and nothing
here closes them.

The `E5` post-CPI rollback obligation is **not** covered: it needs a
deliberate fault-injection instruction variant, which does not exist.
