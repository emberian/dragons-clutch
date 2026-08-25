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

That campaign uses no native processor or mock token implementation. It proves
only the named runtime executions and rollback observations, not a complete
Direct lifecycle or a Solana runtime theorem.
