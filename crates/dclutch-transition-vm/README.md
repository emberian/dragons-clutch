# dClutch transition VM

This `no_std`, `no_alloc`, safe Rust crate executes canonical `DCTV` programs
emitted by Lean. V1 retains the initial 64-scalar/16-identity compatibility
profile. V2 declares exact scalar and identity widths in the program header,
uses borrowed caller-owned input/scratch/output banks, and therefore does not
compile an outcome, family, or register-bank width into the interpreter.

Both versions hostile-decode their complete instruction streams and use
checked arithmetic. V1 commits its fixed register copy only after acceptance.
V2 copies an accepted scratch candidate into output only after every operation
succeeds; a late refusal can alter scratch but leaves input and output
byte-for-byte unchanged. The V2 instruction count and register indices are
`u16` physical-representation bounds. Capacity profiles may impose smaller
measured chain limits, but those limits are not VM semantics.

The VM does not parse Solana accounts, inspect signatures, select a Product or
Realm, or decide which program bytes are authorized. An adapter must populate
the registers from authenticated sources and pin the exact Lean-emitted
program. Identity instructions expose only equality and inequality, allowing
Lean's abstract identifiers to refine to exact public-key bytes without a hash
or integer surrogate.
