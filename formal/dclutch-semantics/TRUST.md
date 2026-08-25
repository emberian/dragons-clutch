# Trust boundary

## Machine-checked in this package

For an `Admissible` inline ordinary Direct frame, Lean 4 checks that:

- the emitted plan has seven effects;
- the small effect interpreter produces the specified post-state;
- selected-outcome claims are conserved;
- collateral across buyer, seller, and venue is conserved;
- both replay nonces advance exactly once;
- every post-state field remains in `u64` range;
- quote equality is exact at the Product-owned scale;
- the fee uses the named floor boundary; and
- a refused frame exposes the unchanged pre-state.

Lean also checks the general encoded length equations for V1 headers, effect
records, and plans, plus the exact 120-byte hexadecimal encoding of the Direct
example.

`cumulativeFee_monotone` proves monotonicity of the concrete floor-fee function,
and `cumulative_floor_fee_fragmentation_independent` combines it with the
telescoping subtraction theorem. Matcher-selected fragmentation therefore
cannot change a resting order's final cumulative fee in the semantic model.

## Not yet connected

- generated safe-Rust and TypeScript clients;
- Ed25519 instruction authenticity;
- Solana account ownership, signer/writable flags, PDA derivation, CPI, sysvars,
  Token/Token-2022 semantics, rent, and transaction rollback;
- a machine-checked refinement from the Lean effect interpreter to the Rust
  microkernel (`dclutch-effect-kernel` currently supplies cross-language vector,
  round-trip, execution, hostile-parser, and late-rollback tests only);
- refinement from the executor's deployed ELF bytes to Lean's sBPF semantics;
- compute-unit, stack, ELF-size, and rent measurements;
- all Direct routes other than inline ordinary execution; and
- all other protocol families.

The package uses Lean 4.30.0. No theorem contains `sorry`, an axiom, an
`external_body`, or an assumed specification. Lean's kernel, toolchain, native
code used by `native_decide`, hardware, and operating system remain trusted for
the build. `native_decide` is used only for concrete regression examples, not
for the general conservation and refinement theorems.

## Provenance

The implementation is freshly authored for dClutch from the protocol invariants
recorded in this repository. It does not import, copy, or depend on leanuweave,
minidregg, breadstuffs, Dragon's Clutch, or another historical DREGG codebase.
