# Verus kernel shadow

`lib.rs` names the first proof obligations for the exact source in
`crates/clutch-kernel`: ceiling-liability arithmetic and complete-split
solvency. It is a proof shadow, not an executable replacement or an adapter.

Verus is installed and pinned (see
[`toolchain/PINNED_PROOF_TOOLS.md`](../../toolchain/PINNED_PROOF_TOOLS.md)).
This shadow currently **fails** to verify: run directly with the pinned
binary, `lib.rs` reports 2x `E0308` (`Seq::subrange` expects `int`, got `nat`)
at lines 31 and 32. No formal-verification claim is made. The remaining
bounded-array and checked-u64 refinement obligations, plus these type errors,
must be closed before this artifact is promoted.

