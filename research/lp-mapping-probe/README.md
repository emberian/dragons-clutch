# `lp-mapping-probe` — PROPOSED research probe

Status: **PROPOSED research artifact.** Not a shipped crate, not a member of any
workspace, not on any on-chain path, and not an evidence claim. It exists to
make the claims in
[`docs/implementation/OPTIMALITY_CERTIFICATE_MAPPING.md`](../../docs/implementation/OPTIMALITY_CERTIFICATE_MAPPING.md)
falsifiable against real code rather than against prose. It uses only the public
API of `clutch-batch` and modifies nothing.

```
cargo run --release
```

Four experiments, all referenced by name from the mapping document:

| id | question | result |
| --- | --- | --- |
| E1 | does `derive_canonical`'s closed-form flow equal the brute-forced LP maximum of score component 1? | agree 11 / 11 ticks, disagree 0 |
| E2 | is the canonical allocation the argmax of the frozen `ScoreV1` order *within one tick*? | **no** — a feasible rival ties components 1 and 3, beats component 4 by 2 owners, and is refused `CandidateMismatch` |
| E3 | does allocation policy A's `StrictUnderfill` refusal remove the flow-maximal tick from the searched grid? | **yes** — A's grid argmax is 4 % below B's |
| E4 | how far does largest-remainder allocation deviate from the fractional pro-rata point? | worst `‖f − x*‖₁ = 3.0` at `n = 6, D = 3`, exactly the predicted `2D(n−D)/n`; worst `‖·‖_∞ = 0.833 < 1` |

E2 and E3 are counterexamples and are candidates for promotion into the crate's
named falsifier suite (mapping document §6, obligations 4 and 5).
