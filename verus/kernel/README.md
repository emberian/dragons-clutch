# Verus kernel shadow

`lib.rs` names the first proof obligations for the exact source in
`crates/clutch-kernel`: ceiling-liability arithmetic and complete-split
solvency. It is a proof shadow, not an executable replacement or an adapter.

The current workspace does not provide a `verus` binary, so this file has not
been checked by Verus in this spike. No formal-verification claim is made. A
future proof run must pin Verus, vstd, Z3, and the Rust frontend, record their
versions and source digests, and close the remaining bounded-array and
checked-u64 refinement obligations before this artifact is promoted.

