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
