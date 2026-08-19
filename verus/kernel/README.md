# Verus kernel work

There are two deliberately separate artifacts here.

## Exact production transfer arithmetic

[`run_transfer_refinement.sh`](run_transfer_refinement.sh) verifies the actual
checked-in body of
[`crates/clutch-kernel/src/transfer_arithmetic.rs`](../../crates/clutch-kernel/src/transfer_arithmetic.rs),
which `MarketState::transfer_internal` calls in production.  The runner copies
that source to a private temporary directory and performs only three mechanical
proof-facing edits:

1. name the function's return value so a Verus postcondition can refer to it;
2. turn inner documentation comments into ordinary comments because the source
   is nested in the generated proof module; and
3. insert the reviewed `requires`/`ensures` contract at the unique named anchor.

The executable function body is otherwise the production body, not a second
implementation or a mathematical toy.  Both the helper source and the precise
`transfer_internal` call/write seam are digest-pinned.  Drift refuses the run
until the proof and seam are reviewed together.

Pinned Verus checks this contract under `quantity <= from`:

- success subtracts exactly `quantity` from the sender;
- success adds exactly `quantity` to the receiver;
- the two-owner mathematical sum is conserved;
- receiver overflow returns `TransferArithmeticError::Overflow`; and
- underflow and the defensive conservation refusal are unreachable.

The runner then changes addition to subtraction and inverts the conservation
guard in two separate temporary mutants.  Both must fail specifically at the
postcondition or the gate fails.  Run it with:

```sh
sh verus/kernel/run_transfer_refinement.sh
cargo test --manifest-path crates/clutch-kernel/Cargo.toml --offline
```

This is a narrow executable refinement result, not end-to-end verification.
The contract injector and the reviewed call-site digest are part of the trust
boundary.  Verus/vstd, its Rust frontend, bundled Z3, the host Rust compiler,
the rest of `MarketState::transfer_internal`, all other kernel transitions,
serialization, accounts, Token-2022 CPI, Solana runtime behaviour, and SBF
code generation are outside the theorem.  In particular, it proves no owner
identity, phase, collateral, account-authentication, rollback, or deployed-code
claim.

There are no project `assume`, `admit`, axiom, `external_body`,
`assume_specification`, `unsafe`, or proof-only executable branches in this
path.  Tool versions and binary provenance are recorded in
[`toolchain/PINNED_PROOF_TOOLS.md`](../../toolchain/PINNED_PROOF_TOOLS.md).

## Older mathematical shadow

[`lib.rs`](lib.rs) names ceiling-liability and complete-split obligations.  It
remains a hand-written mathematical shadow and is not the production transfer
result above.  It also still fails under the pinned Verus release: after its
two `Seq::subrange` type errors, its unfinished division proof has open
obligations.  It must not be counted as a passing theorem or as correspondence
to executable Rust.
