# dClutch compiled Direct codec

This safe, `no_std`, `no_alloc` crate encodes and strictly decodes the compact
Direct intent, controller instruction, and experimental execution profile owned
by `formal/dclutch-semantics/DClutchSemantics/DirectControllerCodec.lean`.

The Rust implementation is not a second semantic authority. Its tests compare
all three encoders byte-for-byte with the exact vectors emitted by Lean. The
Solana controller and host operator consume these types; neither should carry a
parallel handwritten layout.
