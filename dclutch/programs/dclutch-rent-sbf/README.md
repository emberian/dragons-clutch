# dclutch-rent-sbf

Standalone successor SBF adapter for the SDK-free `dclutch-rent-contract`.
It owns SVM observations and physical effects, while the contract remains the
sole instruction, account-role, PDA-seed, and exact-balance semantic owner.

Every route is `LifecycleRentInstructionV2`, behind
`LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2`. Account frames are exact:

- Create: payer (writable signer), vacant lifecycle RentCredit PDA (writable),
  canonical executable System Program, canonical Rent sysvar.
- Sweep: lifecycle RentCredit PDA (writable), the credit's own immutable refund
  wallet (writable), canonical Rent sysvar. Permissionless: the destination is
  fixed by the credit, not by the caller.
- Close: eight accounts, or nine when a Registry continuation rides along.
  Requires a completely retired Market and the current Core close authority as
  signer.

The permanent per-authority RentCredit V1 Create and Withdraw routes were
deleted on 2026-08-27 as superseded by the Market-generation-scoped lifecycle
credit that tier 1 exercises. Their real-SBF campaign
(`tests/program_test.rs`) went with them; a copy is under
`~/dev/dclutch-legacy/dclutch-rent-credit-v1-routes/`, and git history is the
record. Anything that is not a V2 request is now refused as `Instruction`.

Focused host gates:

```text
cargo test --manifest-path programs/dclutch-rent-sbf/Cargo.toml --lib
cargo clippy --manifest-path programs/dclutch-rent-sbf/Cargo.toml --all-targets --all-features -- -D warnings
```

There is no real-SBF ProgramTest campaign here at present. The lifecycle V2
routes are exercised through the Registry continuation and Core retirement
paths that reach them; a Sweep campaign is JRNY-1's.
