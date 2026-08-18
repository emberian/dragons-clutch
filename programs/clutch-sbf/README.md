# `clutch-sbf` — bring-up SBF program

A deployable SBF program with a routed instruction set: Split, Merge,
Materialize, Dematerialize, CreateMarket, FeedAdvance, evidence-gated
Resolve, RedeemInternal, and PlaceOrder are implemented, each mirroring the
offline reference adapter with byte-level differential tests; the SVM harness
drives all reference-oracled families through a real bank byte-exactly.

This is **bring-up evidence, not a finished program**. It is not complete,
not audited, and not authorization to deploy anywhere. `Resolve` currently
exceeds the per-transaction compute ceiling (measured; the terms facts API
fix is in flight). `CancelOrder` and `SettlePage` are honest refusals with
recorded findings (cancellation is unrepresentable in the frozen page format;
the batch relation does not fit an SBF frame pending the streaming verifier).
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
- `harness/` — host binary that builds one deterministic fixture, computes the
  expected post-state with the offline reference adapter, and emits genesis
  account dumps and unsigned transactions. It signs nothing and holds no key
  material.
- `scripts/run_bringup.sh` — the gate: builds the ELF twice, compares hashes,
  runs a loopback `solana-test-validator`, and diffs the SVM post-state against
  the reference post-state.
- `vendor/` — one verbatim third-party crate, present only because this host has
  its source but not its `.crate` archive. See `vendor/PROVENANCE.md`.

Full write-up, including the ladder of harnesses tried, the deferred-check list,
and honest claim language: [`docs/implementation/SBF_BRINGUP.md`](../../docs/implementation/SBF_BRINGUP.md).

```sh
programs/clutch-sbf/scripts/run_bringup.sh
```
