
# ADR-0005: Lean is the proof substrate of record

Status: adopted 2026-08-20 (supersedes ADR-0003; adoption recorded in docs/decisions/ADOPTED_2026-08-20.md)

## Context

ADR-0003 designated Verus the V1 executable-kernel gate, Rocq the
independent mathematical shadow, and Lean an optional seam, warning against
Lean becoming mandatory by inertia. As of 2026-08-20 (`6c37177`): Lean
carries 212 checked theorem/lemma declarations, zero sorry, zero project
axioms, no native_decide, building offline with no dependencies; Verus holds
one narrow production-body refinement (transfer arithmetic, mutation-gated)
plus a 28-obligation scalar mathematical shadow whose correspondence is a
digest-pinned human review with named excluded sources; Rocq holds zero
theorems and one machine-checked vacuous conjunct. Lean became primary by
delivering, not by inertia; the house AIR-in-Lean rule, the Lean 4 sBPF
semantics (solanalib), and the Aeneas/Charon Lean backend all point the
project's formal road through Lean.

## Decision

Lean is the proof substrate of record: new theorem obligations, promotion
criteria, and verification-facing design language name Lean unless a named
exception applies. Verus is retained solely for checked-Rust-subset results
verifying actual executable bodies under digest-pinned contracts; its
mathematical-shadow mode is deprecated in favor of Lean. The Rocq shadow
role is retired; `rocq/ClutchKernel.v` remains a historical specification
and its manifest typecheck gate stays labeled non-proof-content.
`native_decide` is banned in the Lean tree (it places Lean's compiler in
the TCB); the standing forbidden-construct audit and the Lean-only axiom
set (`propext`, `Classical.choice`, `Quot.sound`) are the rule of this
ADR, not a convention.

## Consequences

- Promotion criteria written as "Verus and Rocq close X" are rewritten to
  name Lean with unchanged property content (first: FEE_GEOMETRY §7).
- Lean remains a hand-written model: correspondence to Rust is manual,
  disclosed, and bounded by shared vectors until a mechanical arrow exists.
  No Lean theorem is ever cited as verification of the Rust, the ELF, or a
  deployment; PROVED-MODEL and CHECKED-RUST-SUBSET stay distinct claims.
- The Aeneas/Charon spike (one pure kernel function, bounded, with a kill
  criterion) is the named test of closing the model-to-source arrow in
  Lean; solanalib sBPF scoping is the named runtime-plane road. If the
  spike fails, executable-body growth continues in Verus, never by
  relabeling model theorems.
- The transfer-arithmetic Verus result remains evidence; the failing
  kernel/accumulator shadows remain recorded failures.

## Rejected alternatives

Re-staffing Verus/Rocq to ADR-0003's assignments (no output, no advocate,
no road; porting 212 theorems yields no new knowledge). An unrecorded
hybrid (the present state — a false governance record).

## Verification impact

None on existing evidence identities; no reseal; CURRENT_TRUTH §1's claim
vocabulary unchanged. A successful proof still never implies the
adapter/runtime/ELF is proved.

## Authority impact

None. Verification does not close Gate L0 or authorize a deployment.

## Evidence

lean/ build + audits at 6c37177; verus/kernel/TRANSFER_ASSUMPTIONS.md;
verus/batch/BATCH_ASSUMPTIONS.md; rocq/ClutchKernel.v and the
proof.rocq_check gate note; docs/reviews/PLANNED_VS_BUILT_2026-08-19.md;
docs/decisions/REPORT_adr-0003-supersession_2026-08-20.md.
```

---

*Report compiled read-only except for this file; counts, builds, and axiom
audits rerun in the worktree at `6c37177`.*
