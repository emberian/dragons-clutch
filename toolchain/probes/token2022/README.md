# Token-2022 probe

Status: **feasibility probe, 2026-08-18**. Not a program, not a component of
the protocol, not evidence about Dragon's Clutch.

This directory answers one question and no others: *can the Token-2022 leg of
[`SOLANA_REFERENCE_ADAPTER.md`](../../../docs/implementation/SOLANA_REFERENCE_ADAPTER.md)
obligations 5-7 be built on this host, and does the V1 collateral matrix
actually refuse what it says it refuses when it is run against bytes a real
Token-2022 program wrote?*

It carries no clutch semantics. It depends on no `clutch-*` crate. It is a
standalone Cargo workspace precisely so that nothing in `programs/` or
`crates/` acquires an Agave runtime dependency because of it.

## Run it

```sh
toolchain/probes/token2022/run_probe.sh
```

Everything is local: an in-process Agave bank started by `solana-program-test`,
with the Token-2022 ELF that `solana-program-binaries` installs at genesis. No
RPC, no cluster, no wallet, no key material, no submission.

The recorded run is [`evidence/probe_run.txt`](evidence/probe_run.txt); the
integration plan it feeds is
[`docs/implementation/TOKEN2022_PLAN.md`](../../../docs/implementation/TOKEN2022_PLAN.md).

## What is in here

`src/lib.rs` is the one piece of the future adapter that cannot be written
offline: the predicate that turns raw Token-2022 mint and token-account bytes
into an accept-or-refuse decision under a V1 Realm collateral profile. Its
`RefusalCode` numbering is identical to `RefusalCode` in
`research/collateral-profiles/model.py`, so the offline decision vectors and
anything observed on chain are directly comparable. The Python model decides
over a hand-built snapshot; this decides over bytes the token program wrote.

`tests/token2022_probe.rs` drives six scenarios against the bank. Four are
positive or negative admission decisions, one is the mint/burn/transfer
lifecycle, and one deliberately admits a mint V1 forbids so that the refusals
are falsifiable rather than vacuous.

## Toolchain

`rust-toolchain.toml` pins 1.93.1, which is **not** the repository's host build
pin of 1.89.0. 1.89.0 cannot compile the Agave 4.2.1 runtime; the verbatim
failure is recorded in the plan document. That divergence is a real cost of
adopting `solana-program-test` and is the plan's first open decision.
