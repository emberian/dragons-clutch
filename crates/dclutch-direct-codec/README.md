# dClutch compiled Direct codec

This safe, `no_std`, `no_alloc` crate encodes and strictly decodes the compact
Direct intent and controller instruction owned by
`formal/dclutch-semantics/DClutchSemantics/DirectControllerCodec.lean`. Each
maker signs the canonical Market identity directly; there is no parallel
execution-profile account or codec.

The registered successor adds a Lean-specialized 232-byte state, 152-byte
creation request, 32-byte fill request, 24-byte terminal request, 16-byte
claim-owner requests, and 168-byte lifecycle program. The Rust implementation
is not a second semantic authority.
Its tests compare the seller intent, buyer intent, enclosing inline controller,
registered state, fill, cancel, and expiry encoders byte-for-byte with exact
Lean output. The Solana controller, claim owner, host operator, and frontend
consume these types; none should carry a parallel handwritten layout.
