# dClutch Product payoff codec

This standalone crate is the safe `no_std`, `no_alloc` translation target for
the Lean-owned fixed Product-payoff ABI. It hostile-decodes one exact 432-byte
value and evaluates constant, ramp-up, ramp-down, and tent terms over the sole
Product-owned knot sequence.

The V1 physical profile holds at most 16 knots and 16 terms. These are bounded
storage/profile choices, not mathematical restrictions in
`ProductPayoff.lean`. Unused capacity bytes must be zero. Knots are strictly
increasing, term shapes use normalized knot indices, and shape keys are
strictly increasing so a term set has one admitted order and no duplicate
shape.

`payoff_interpolation_floor` is the only rounding boundary. Rust widens one
`u64 × u64` interpolation numerator to `u128`, divides once, and floors exactly
as Lean does. It does not introduce a second rounding convention. Admission
also checked-sums term amplitudes into the deliberately conservative liability
bound; it neither calls that bound minimal nor admits overflow.

Run the complete generation and differential gate:

```sh
crates/dclutch-product-payoff-codec/check.sh
```

The gate:

- recompiles the new Lean ABI module without modifying the package aggregate;
- requires `EmitProductPayoffRust.lean` to reproduce `src/generated.rs`
  byte-for-byte;
- hostile-decodes 6 canonical programs, all 2,592 single-byte mutations, 2,598
  truncation/padding cases, and 23 explicit structural hostiles;
- compares 38 exact Lean/Rust evaluations, including a multiplication that
  exceeds `u64` before division; and
- compares 18 conservative collateral decisions, then runs Rust formatting,
  tests, and strict all-target Clippy.

## Evidence boundary

This is finite executable differential agreement plus named Lean theorems
about the semantic evaluator and conservative liability bound. It is not a
universal source-refinement theorem and not an SBF claim. The Lean/Rust
compilers, generator source list, corpus parser, and host runtime are trusted.
Product content identity, release authorization, Solana account/PDA ownership,
transaction rollback, compiler lowering, loader behavior, and runtime compute
cost remain unchecked here.
