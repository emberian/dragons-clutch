# dClutch compiled Direct codec

This safe, `no_std`, `no_alloc` crate encodes and strictly decodes the compact
Direct intent and controller instruction owned by
`formal/dclutch-semantics/DClutchSemantics/DirectControllerCodec.lean`. Each
maker signs the canonical Market identity directly; there is no parallel
execution-profile account or codec.

The Rust implementation is not a second semantic authority. Its tests compare
the seller intent, buyer intent, and enclosing controller encoders byte-for-byte
with the exact vectors emitted by Lean. The Solana controller, host operator,
and frontend consume these types; none should carry a parallel handwritten
layout.
