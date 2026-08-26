# dclutch-rent-sbf

Standalone successor SBF adapter for the SDK-free `dclutch-rent-contract`.
It owns SVM observations and physical effects, while the contract remains the
sole instruction, account-role, PDA-seed, and exact-balance semantic owner.

V1 account frames are exact:

- Create: payer (writable signer), vacant RentCredit PDA (writable), canonical
  executable System Program, canonical Rent sysvar.
- Withdraw: permanent RentCredit PDA (writable), immutable authority (signer),
  data-empty System recipient (writable), canonical Rent sysvar. Authority may
  equal recipient only with the contract-defined privilege union.

The RentCredit is permanent. There is deliberately no close action.

Focused host gates:

```text
cargo test --manifest-path programs/dclutch-rent-sbf/Cargo.toml --lib
cargo clippy --manifest-path programs/dclutch-rent-sbf/Cargo.toml --all-targets --all-features -- -D warnings
```

The real-SBF campaign first builds the ELF, then supplies its output directory
to the isolated ProgramTest workspace:

```text
cargo build-sbf --manifest-path programs/dclutch-rent-sbf/Cargo.toml --sbf-out-dir /tmp/dclutch-rent-sbf-out
SBF_OUT_DIR=/tmp/dclutch-rent-sbf-out cargo test --manifest-path programs/dclutch-rent-sbf/Cargo.toml --test program_test
```
