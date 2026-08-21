# `clutch-sbf/svm-tests` — the Token-2022 leg, against a real bank

Status: **program-test evidence.** Not a cluster, not an audit, not
authorization to deploy anywhere. The workspace was founded 2026-08-18 with
two test files; it now carries 27. The scenario table below still describes
only the token leg — see [Provenance and
regeneration](#provenance-and-regeneration) for what the rest is pinned by.

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

All six live in `tests/token_leg.rs`.

| scenario | claim |
| --- | --- |
| `e1_materialize_mints_exactly_q_and_the_aggregate_reconciles` | `Materialize` of *q* raises the outcome mint's supply by exactly *q*, credits the destination exactly *q*, lowers the internal term by exactly *q*, and leaves the market-wide observed cache equal to the authoritative mint supply |
| `e1_dematerialize_burns_exactly_and_the_aggregate_reconciles` | the exact inverse, through a real `Burn` |
| `a_direct_burn_is_synchronized_and_the_market_stays_live` | a holder burning outcome tokens *outside* the program lowers the mint's supply; the next claim transition recognizes the lower supply as a safe liability donation, retains Hoard backing, and continues |
| `an_extension_on_the_outcome_mint_is_refused_at_instruction_time` | `TransferFeeConfig` and `MintCloseAuthority` on the mint are refused when the instruction runs, not only when a market is founded |
| `the_derived_hoard_authority_cannot_be_signed_for_by_a_wallet` | a deposit into an account owned by `seeds::hoard_authority_pda` needs only the depositor's signature; taking it out with the same signature is `TokenError::OwnerMismatch` |
| `the_incomplete_mint_vector_is_refused_and_moves_nothing` | the transitional optional-token-leg hole is **closed**: `CreateMarket` now creates the mints, so the ten-account plane is `AccountCount` and after the refusal the two truths are still equal because nothing moved |

The third and sixth rows changed meaning at the 2026-08-19 single-truth
cutover (`docs/implementation/TOKEN2022_EXTERNAL_TRUTH_V1.md`): actual
Token-2022 mint supply is now authoritative, `external_supply` survives only
as a last-observed cache, and `ShadowSupplyMismatch` keeps its one remaining
correct use — a *higher* supply, impossible without the program's
mint-authority PDA.

## What it does not establish

An in-process bank is not a cluster. Transaction replay, durable nonces,
instruction duplication inside one transaction, batch retries, fee payment,
rent collection over time, and program upgrade are all outside what
`solana-program-test` can show. `TOKEN2022_PLAN.md` §4 lists them, and nothing
here closes them.

The `E5` post-CPI rollback obligation *is* now reachable without a
fault-injection variant: `ACTOR_COLLATERAL` is deliberately smaller than the
founding position's free cash, so a `Split` exists that the kernel admits and
the token program then refuses (`tests/collateral_leg.rs:64-68`). The
rollback cases live in `collateral_leg.rs`
(`late_create_market_refusal_rolls_back_state_and_token_construction`,
`duplicate_withdrawal_transaction_rolls_back_the_first_token_cpi`,
`a_failed_endow_leaves_ledger_replay_and_tokens_unchanged`).

## Provenance and regeneration

`evidence/svm_run.txt` is a **verbatim capture, not a generated artifact**. It
was written at `5c88505` and last refreshed at `50c6e35` (2026-08-18), when
this workspace contained exactly two test files — so it is the whole-workspace
transcript *of that era*, and it is **one regeneration stale**, as the Realm
report records
(`docs/decisions/REPORT_realm-admission-and-token2022_2026-08-20.md`, §5 and
the §8 cost table). Concretely, against the current tree it names four tests
that no longer exist (`e1_materialize_mints_exactly_q_and_the_shadow_reconciles`,
`e1_dematerialize_burns_exactly_and_the_shadow_reconciles`,
`a_supply_that_drifted_outside_the_program_is_refused`,
`the_ten_account_plane_is_refused_and_moves_nothing`), records 9 collateral-leg
tests where there are now 17, and omits 25 test files.

**Regenerate only at the canonical checkout.** The ELF identity is
same-path-reproducible only — Cargo folds the absolute workspace path into
every path-dependency's `-C metadata`, and hash-sorted symbol ties order by
those hashes — so the canonical artifact identity is defined at
`/Users/ember/dev/dragons-clutch` and nowhere else
(`docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md`; the cycle-E audit states it
as "no detached worktree is used for the canonical build",
`research/liveness-policy-profile/artifacts/4fded7a67a2d8994/audit/RUNTIME_ARTIFACT_AUDIT.md`).
A capture taken in a worktree attributes real bank results to a
relocation-probe artifact, which is worse than a stale file.

**The live regeneration surface is the manifest gate, not a hand capture.**
`sbf.token2022_program_test` and
`sbf.token2022_program_test_non_production_mock` run `run_svm_tests.sh` under
both profiles during every baseline-manifest emission
(`scripts/baseline_manifest.py`). The default gate's own note says the
manifest captures "stable suite totals and the required refusal line, while
variable per-test nocapture/CU text stays under the separate same-ELF evidence
seal" — `svm_run.txt` *is* that separate seal, which is why it has to be
refreshed by the same cycle that emits the manifest, at the same path, against
the same ELF.

So the regeneration is a **reseal-cycle item**, and the capture must record
the profile line, the ELF digest and byte count `run_svm_tests.sh` prints, and
the source commit — the way the existing file records its own toolchain, ELF
digest and source tree.

One gap worth closing in the same pass: the default gate's key pattern
`^[0-9a-f]{64}\s+.*clutch_sbf\.so$` does not match anything
`run_svm_tests.sh` emits (the script prints `elf_sha256=<hex>`), so unlike the
mock-source gate, **the default profile's ELF identity is not captured in any
manifest key line**.
