# `clutch-sbf` — bring-up SBF program

A deployable SBF program with a routed instruction set of which exactly one
instruction, `Split`, is implemented, so that the account-facing half of
Dragon's Clutch can be executed by a real SVM rather than only reasoned about
offline.

This is **bring-up evidence, not a program**. It is not complete, not audited,
and not authorization to deploy anywhere. `Resolve` and `RedeemInternal` refuse
here exactly as they refuse in `programs/solana-reference`, and the four
unimplemented instruction families are honest stubs: they read no account, write
no byte, and report no success.

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
