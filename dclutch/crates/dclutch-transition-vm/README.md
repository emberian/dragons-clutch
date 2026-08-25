# dClutch transition VM

This `no_std`, `no_alloc`, safe Rust crate executes the canonical `DCTV` V1
program emitted by Lean. Programs operate on fixed scalar and 32-byte identity
registers, refuse hostile encodings, derive checked arithmetic outputs, and
commit register changes only after every instruction succeeds.

The VM does not parse Solana accounts, inspect signatures, select a Product or
Realm, or decide which program bytes are authorized. An adapter must populate
the registers from authenticated sources and pin the exact Lean-emitted
program. Identity instructions expose only equality and inequality, allowing
Lean's abstract identifiers to refine to exact public-key bytes without a hash
or integer surrogate.
