# Direct inline ordinary V1 vector

- Semantic source: `DClutchSemantics.Examples.frame`
- Plan source: `DClutch.Direct.effectPlan`
- Encoder source: `DClutch.Codec.encodePlan`
- Toolchain: `leanprover/lean4:v4.30.0`
- Generation command: `lake exe emit-vectors`
- Encoding: lowercase hexadecimal, one final newline
- Encoded bytes: 120
- Header bytes: 8
- Effect records: 7 × 16 bytes
- Differential consumer:
  `dclutch-direct-contract::tests::lean_effect_plan_matches_inline_ordinary_reference`

The vector is a reproducible semantic fixture. It is not a deployment artifact
or evidence about an SBF program.

## Multiprogram physical vectors

`DClutch.Direct.Physical.physicalPlan` derives two disjoint physical plans from
the same semantic frame:

| File | Generator | Bytes | Records |
|---|---|---:|---:|
| `direct-inline-ordinary-claims-v1.hex` | `lake exe emit-claim-vector` | 72 | 4 claim/replay effects |
| `direct-inline-ordinary-custody-v1.hex` | `lake exe emit-custody-vector` | 40 | 2 indivisible transfers |

The claim plan is ordinary Effect V1. The custody plan uses `DCCP`, version 1,
an eight-byte header, and two fixed sixteen-byte records. One custody record is
source party, destination party, six zero reserved bytes, then a little-endian
`u64` amount. The two plans recombine to the exact high-level Direct post-state;
the corresponding Lean theorems are in `DClutchSemantics.Physical`.

## Compiled transition program

`direct-inline-ordinary-program-v1.hex` is the 568-byte output of
`lake exe emit-direct-program`. `DClutchSemantics.DirectProgram.program`
contains 35 fixed sixteen-byte instructions over 41 scalar registers and four
abstract identity registers. It does not accept gross or fee as caller facts:
it derives exact quote divisibility, the quotient, and the named floor fee into
output registers after checking the Direct admission relations. Native
signature evidence and the refinement from abstract identity equality to exact
32-byte public keys remain adapter obligations.
