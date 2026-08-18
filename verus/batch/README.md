# Batch relation proof scaffold

This directory is an intentionally uncompiled Verus scaffold for the pure
arithmetic boundary in `crates/clutch-batch`. It is not a proof, and the Rust
crate does not claim formal verification. No `assume`, `admit`, axiom, or
`external_body` is used to make a claim appear closed.

Verus is installed and pinned (see
[`toolchain/PINNED_PROOF_TOOLS.md`](../../toolchain/PINNED_PROOF_TOOLS.md)).
Run directly against the pinned binary, `batch.rs` **fails** to compile:
`cannot find macro 'verus' in this scope`, because the file has no
`use vstd::prelude::*;`. It has never compiled and no proof log exists.

`batch.rs` additionally contains four `proof fn`s whose entire postcondition
is `ensures true`. These are vacuous placeholders, self-labelled `TODO`, and
would prove nothing even if the file compiled — they must never be counted in
any theorem or assumption inventory for this repository.

The first obligations are:

1. `allocate_conserves`: largest-remainder allocation sums to the target and
   every fill is bounded by its order quantity.
2. `choose_tick_deterministic`: the tie rule returns one grid index for every
   valid fixed grid.
3. `relation_conserves`: verified buy and sell fills are equal to the selected
   matched quantity.
4. `canonical_padding_zero`: inactive fixed-array entries cannot affect a
   successful relation result.

The executable facts are the tests and checked arithmetic in the Cargo crate;
the theorem names below are proposed proof targets only until a pinned Verus
toolchain verifies this exact source digest.
