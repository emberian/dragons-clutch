# dClutch SVM harness

This is a standalone, real-SVM integration harness. It deliberately loads
compiled successor role-program ELFs through `solana-program-test`; it never
registers a native processor or mocks an adapter.

Build the intended SBF artifacts first, then run from this crate with an
explicit artifact directory:

```sh
SBF_OUT_DIR=../../target/deploy cargo test --test resolution_core_v3_lifecycle
```

Each campaign fails early with an honest prerequisite message naming the ELF it
could not read.

Every campaign that loaded the gen-2 monolith `dclutch_sbf.so`, or one of the
gen-2 measurement programs, was banished with those programs on 2026-08-27; the
deleted files are listed in `~/dev/dclutch-legacy/svm-harness-tests/` and remain
in git history. The `RelayedMainnetStateV1` campaign was not banished: its
adapter moved into the Resolution role program and `relayed_mainnet_state.rs`
now loads `dclutch_resolution_proof_sbf.so`.

The successor Resolution campaign executes the compiled Registry and
Resolution ELFs against the provenance-pinned local-validator projection of
the captured Pyth receiver/router programs and account bodies. It covers one
primary success and replay refusal, then funded recovery, funded exhaustion,
and explicit Product failure on one initially fresh Source. Each deterministic
certificate PDA is prepaid through a real System transfer with exact rent plus
tolerated dust and allocated/assigned by the compiled Resolution program. A
second fresh Source reaches exhaustion before an occupied final certificate
proves late transaction-wide rollback across Source, certificate, funding, and
worker state:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-registry-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
cargo build-sbf \
  --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
SBF_OUT_DIR=../../target/deploy \
  cargo test --test resolution_successor -- --nocapture
```

An exact `git archive b0e515f` build produced a 210,528-byte optimized
Resolution V3 ELF with SHA-256
`f684b845a60a25e661dee334e2866895d830956aedba74c8e1bf705d5abee2e7`
and an 89,760-byte Registry ELF with SHA-256
`b7d6634a23de84cb1b1f0a3368493b9008d88278c460f90e26b522af5e9a6e39`.
The clean ProgramTest campaign observed these instruction costs:

| Transition | Compute units |
| --- | ---: |
| Registry Resolution-role reauthentication | 128,793 |
| Primary Pyth success plus certificate creation | 242,546 |
| Primary replay refusal | 7,916 |
| Under-rent certificate refusal at final gate | 239,835 |
| Funded recovery plus certificate creation | 294,002 |
| Funded exhaustion plus certificate creation | 292,213 |
| Explicit Product failure plus certificate creation | 290,172 |
| Rollback lineage recovery plus certificate creation | 295,502 |
| Rollback lineage exhaustion plus certificate creation | 293,713 |
| Occupied failure certificate late refusal | 287,477 |

This is local real-SVM evidence. The captured update is synthetic-local, and
the campaign is not provider availability, devnet, deployment, or mainnet
evidence.

The experimental Direct successor has a separate four-ELF physical campaign.
Build `dclutch_claims_proof_sbf.so`, `dclutch_controller_proof_sbf.so`, and
`dclutch_custody_proof_sbf.so` from their program manifests. Build
`spl_token.so` from the pinned official source named in
`docs/evidence/PHYSICAL_DIRECT_COMPOSITION_2026_08_25.md`, then run:

```sh
SBF_OUT_DIR=../../target/deploy \
  cargo test --test physical_direct_composition -- --nocapture
```

That campaign uses no native protocol processor or mock token implementation.
It also drives the official address lookup table program through create,
extend, next-slot activation, an actual signed v0 physical fill, deactivate,
the full SlotHashes cooldown, and close. It proves only the named runtime
executions and rollback observations, not a complete Direct lifecycle or a
Solana runtime theorem.

An ignored transport campaign takes the same 990-byte v0 fill across a separate
`solana-test-validator` process and JSON-RPC boundary:

```sh
SBF_OUT_DIR=../../target/deploy \
SOLANA_TEST_VALIDATOR=/path/to/solana-test-validator \
cargo test --test physical_direct_composition \
  compiled_direct_crosses_the_local_validator_rpc_boundary \
  -- --ignored --nocapture
```

It loads the three first-party ELFs, uses the validator's canonical Token and
address lookup table programs, checks physical claim and custody mutations, and
deactivates the table. All fixture accounts are imported into a temporary local
genesis; this is not deployment/bootstrap, devnet, or mainnet evidence. The
temporary ledger is removed on exit. The ordinary ProgramTest campaign covers
the table's full 512-slot cooldown and close.
