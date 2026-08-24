# `clutch-sbf` — bring-up SBF program

A deployable SBF program with a routed instruction set: Split, Merge,
Materialize, Dematerialize, CreateMarket, FeedAdvance, evidence-gated
Resolve, and RedeemInternal are implemented, each mirroring the offline
reference adapter with byte-level differential tests, and the SVM harness
drives all eight through a real bank byte-exactly. PlaceOrder and
CancelOrder (v4 tombstones) are implemented with host tests only — the
reference adapter models no order family, so they have no oracle and no
SVM leg.

This is **bring-up evidence, not a finished program**. It is not complete,
not audited, and not authorization to deploy anywhere. `Resolve` once
exceeded the per-transaction compute ceiling; the terms facts API fix
landed and it now executes at 536 123 CU (38% of the ceiling).
`SettlePage` is an honest refusal with a recorded finding (on-chain
settlement awaits the streaming verifier's integration).
A refusal reads no account, writes no byte, and reports no success.

- `program/` — the SBF program. No semantic or economic logic: it authenticates
  hostile `AccountInfo` metadata, derives and checks program addresses, decodes
  through `clutch-solana-layout`, transitions through `clutch-kernel`, and
  writes back. The PDA seed schema in `program/src/seeds.rs` is a **proposal**,
  not a frozen ABI.
  - `src/accounts.rs` — the shared validation and account plane: metadata
    authentication, address comparison, one decoder per account, and the
    CLO-DELTA-V1 closure primitives.
  - `src/dispatch.rs` — decodes the request envelope and routes on the action
    tag. One arm per instruction family.
  - `src/instructions/` — one module per family, one lane per module. The
    ownership table is in `docs/implementation/SBF_BRINGUP.md`.
  - `src/token.rs` — Token-2022 observation, admission against the frozen
    collateral bitsets, and CPI construction. No economics: every quantity is a
    parameter the kernel already decided.
- `harness/` — host binary that builds one deterministic fixture, computes the
  expected post-state with the offline reference adapter, and emits genesis
  account dumps and unsigned transactions. It signs nothing and holds no key
  material.
- `svm-tests/` — a **separate** Cargo workspace that drives the real ELF against
  the real Token-2022 program on an in-process Agave bank: `Materialize` mints
  and `Dematerialize` burns by CPI, with exact post-CPI delta checks and the
  shadow/supply reconciliation. It carries its own 1.93.1 toolchain pin because
  the Agave runtime cannot be built by this repository's 1.89.0 host pin; see
  its README and `docs/implementation/TOKEN2022_PLAN.md` §1.2.
- `scripts/run_bringup.sh` — the gate: builds the ELF twice, compares hashes,
  runs a loopback `solana-test-validator`, and diffs the SVM post-state against
  the reference post-state. All external-validator launchers default to the
  repository's provenance-checked patched Agave binary and retain exact-PID
  listener probes before and after protocol traffic; stock Agave is refused.
- `vendor/` — one verbatim third-party crate, present only because this host has
  its source but not its `.crate` archive. See `vendor/PROVENANCE.md`.
- `source-profiles/` — provisional, machine-readable source observations and an
  offline consistency gate. These records are not compiled registry entries or
  release identities; see `source-profiles/README.md`.

Full write-up, including the ladder of harnesses tried, the deferred-check list,
and honest claim language: [`docs/implementation/SBF_BRINGUP.md`](../../docs/implementation/SBF_BRINGUP.md).

The separately identified `non-production-product-series-lab` profile admits
only typed immutable Product/Series artifact publication in addition to the
full profile. Artifact kinds `32..=40` are refused by ordinary profiles. Its
real-SBF bank gate publishes all nine exact bodies; the largest 2,352-byte
basis crosses 13 ordered writes. It exercises hostile context/digest/cursor,
nonzero sequence, a self-hashed malformed body, and incomplete-seal rollback;
checks exact program ownership and rent; and proves a second upload converges
on the same content PDA:

```sh
cd programs/clutch-sbf/svm-tests
./run_svm_tests.sh --non-production-product-series-lab \
  product_series_artifact_catalog_is_real_resumable_and_fail_closed
```

This profile does not register or activate a Series, hold prepaid funding, or
create a Market. Those routes remain blocked on SourcePlane V3, authenticated
registry selectors, full-width Instance identity, and mutable component-owned
funding state.

Structured custody is joining the single
`profile-successor-chain-attached-dev` product rather than retaining a separate
laboratory identity. The wrapper and base share one exact account-count
contract for current actions 1/3/5/6/7/8 and authenticate their own checked
ProgramData/ELF release manifests before value observation or mutation.
Actions 2/4 remain withdrawn.

The staged handlers hostile-decode the current Realm collateral chain, Product
lineage, Hoard V2, ClaimLedger V3, Position V3 and Replay V3 owners, descriptor
root, wrapper/base Token-2022 deployments, FundingTerms-selected neutral sink,
and exact collateral release receipts. Value-moving compositions preserve the
named rent-principal and donation boundaries and bind their hostile-reloaded
poststates. The unified profile remains an implementation checkpoint until
every required family tuple and observed-positive collateral/claim release row
is present.

SourceSeries `77/v2` actions 1 through 12 remain compiled but not callable at
this checkpoint. Their action handlers and per-generation close are concrete,
but Product has not yet called the private
`retire_source_funding_custody_v1` whole-lifecycle drain. Enabling Source before
that exact Product retirement join would allow prepaid custody to be founded
without a callable final refund/donation disposition. Failure `78/v1` actions
10 through 13 are independently callable because their current session
resolve/archive and Product slot-10 transition are already joined.

No build, measurement, SVM, or validator evidence exists yet for the unified
profile.

```sh
# one-time source/build preparation; subsequent builds are offline
tools/agave-loopback-validator/fetch-source.sh
tools/agave-loopback-validator/build.sh --allow-network

programs/clutch-sbf/scripts/run_bringup.sh
```
