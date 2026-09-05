# Lean-emitted vectors

Every file here is printed by one `Emit*.lean` at the package root and
consumed byte-for-byte by a Rust or TypeScript test. Regenerate one with

    lake build DClutchSemantics.<Module> && lake env lean --run Emit<Name>.lean > vectors/<file>

They are reproducible semantic fixtures, not deployment artifacts and not
evidence about an SBF program. Toolchain: `leanprover/lean4:v4.30.0`.

| File | Emitter | Bytes | Consumer |
|---|---|---:|---|
| `direct-inline-ordinary-v1.hex` | `EmitVectors.lean` | 120 | `dclutch-effect-kernel` decodes it in its tests |
| `direct-inline-ordinary-program-v1.hex` | `EmitDirectProgram.lean` | 568 | `dclutch-transition-vm` executes it in its tests |
| `direct-controller-v1.txt` | `EmitDirectControllerVectors.lean` | — | `dclutch-direct-codec`, `packages/dclutch-sdk`, `apps/dclutch-web` encode and strictly decode all four vectors |
| `economic-kernel-v1.txt` | `EmitEconomicVectors.lean` | — | `dclutch-economic-kernel` hostile-decodes each `DCES` pre-state, executes the command, and equals all three Lean outputs |

## Direct inline ordinary V1

Semantic source `DClutchSemantics.Examples.frame`; plan `DClutch.Direct.effectPlan`;
encoder `DClutch.Codec.encodePlan`. Lowercase hexadecimal, one final newline:
an 8-byte header and seven 16-byte effect records.

## Compiled transition program

`DClutchSemantics.DirectProgram.program` is 35 fixed sixteen-byte instructions
over 41 scalar registers and four abstract identity registers. It does not
accept gross or fee as caller facts: it derives exact quote divisibility, the
quotient, and the named floor fee into output registers after checking the
Direct admission relations. Native signature evidence and the refinement from
abstract identity equality to exact 32-byte public keys remain adapter
obligations.

## Direct controller ABI

The seller intent, buyer intent, complete controller instruction, and
experimental execution profile. The controller and real-SVM harness consume
the Rust crate instead of maintaining local layouts.

## Shared economic microkernel

Generated from the executable frames in `DClutchSemantics.EconomicExamples`.
Each operation has canonical `DCES` pre/post state, existing `DCEF` claim-plan,
and existing `DCCP` custody-plan hex. This is source-level differential
evidence; account authenticity, CPI, persistence, and SVM execution remain
outside it.

## Removed

`direct-inline-ordinary-claims-v1.hex` (72 bytes) and
`direct-inline-ordinary-custody-v1.hex` (40 bytes), the two multiprogram
physical plans, lost their only Rust consumer when the DCLTCAT1 stratum was
banished on 2026-08-27 and were deleted with their emitters on 2026-09-04. The
theorems about those plans -- custody conservation and exact recomposition to
the Direct post-state -- live in `DClutchSemantics.Physical` and did not move.
