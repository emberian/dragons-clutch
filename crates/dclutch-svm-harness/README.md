# dClutch SVM harness

This is a standalone, real-SVM integration harness. It deliberately loads the
compiled `dclutch_sbf.so` ELF through `solana-program-test`; it never registers
a native processor or mocks the adapter.

Build the intended SBF artifact first, then run from this crate with an
explicit artifact directory:

```sh
SBF_OUT_DIR=../../target/deploy cargo test --test failure_route
```

`SBF_OUT_DIR` must contain `dclutch_sbf.so`. The tests fail early with an
honest prerequisite message when it is missing. The exercised slices are
immutable collateral-Realm creation through a real System CPI and the
permissionless, body-free failure route. The harness does not yet test a
provider price update.

The successor Resolution campaign executes the compiled Registry and
Resolution ELFs against the provenance-pinned local-validator projection of
the captured Pyth receiver/router programs and account bodies. It covers one
primary success, one funded recovery, exhausted Product failure, exact bounty
credit, certificate sequencing, and a late transaction-wide refusal:

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

This is local real-SVM evidence. The captured update is synthetic-local, and
the campaign is not provider availability, devnet, deployment, or mainnet
evidence.

Run Realm creation evidence with:

```sh
SBF_OUT_DIR=../../target/deploy cargo test --test realm_creation
```

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
