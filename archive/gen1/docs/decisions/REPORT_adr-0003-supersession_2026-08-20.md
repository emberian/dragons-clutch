# Decision report: `adr-0003-supersession` (register H1)

Status: **ANALYSIS / PROPOSED.** Standalone report for register entry H1
(`docs/decisions/DECISION_REGISTER_2026-08-20.md:961-989`), prepared
2026-08-20 against worktree HEAD `6c37177`. It decides nothing; the
appendix is a superseding ADR ready for ember to adopt. Every count below
was re-measured in this worktree, not recalled.

## 1. The decision

`docs/adr/0003-verus-first-shadow-models.md` (status: experimental) made
Verus "the V1 executable-kernel gate," Rocq the Rust-independent shadow and
extraction oracle, and confined Lean to a seam, warning that Lean must not
become "mandatory by inertia." Reality inverted all three assignments with
no superseding record (`docs/reviews/PLANNED_VS_BUILT_2026-08-19.md:69-78`,
ranked #1 among quietly-superseded). The choice: **(a)** author ADR-0005
ratifying Lean as the proof substrate of record, Verus retained for narrow
checked-Rust-subset results, the Rocq role retired; **(b)** restore
ADR-0003's intent by re-staffing Verus/Rocq and porting the Lean corpus;
**(c)** a recorded hybrid.

## 2. The evidence

### What each substrate actually holds (measured at `6c37177`)

**Lean** (`lean/DragonsClutch/`): **212 theorem/lemma declarations** outside
comments — 184 at bare line start plus 28 attribute-prefixed (`@[simp]
theorem` …), which is why the register says "184+". Per file: BSpline 126,
Transitions 34, Basic 32, Solvency 13, Basis 7 (Kernel.lean and Vectors.lean
are definitions and `#guard` vectors). Zero `sorry`, zero project axioms,
zero `native_decide`, zero `unsafe`/`implemented_by` (grep per
`lean/README.md:73`). `lake build` reruns green in this worktree ("Build
completed successfully", 10 jobs, no network, `"packages": []`). The axiom
audit reruns clean: `P_SOLV_01_resolution_bound` depends on exactly
`[propext, Classical.choice, Quot.sound]`. The claim discipline is already
ADR-shaped: correspondence to `crates/clutch-kernel` is declared "manual,
unproved, and bounded only by the semantic vectors both evaluate"
(`lean/README.md`); no theorem claims to reach the Rust, the ELF, or a
deployment.

**Verus** (`verus/`): four artifacts, two alive. Alive: the transfer
refinement — the repo's only production-Rust proof — verifying the actual
checked-in body of `crates/clutch-kernel/src/transfer_arithmetic.rs` with
conservation/overflow postconditions and two required-red mutants (narrow
CHECKED-RUST-SUBSET, `verus/kernel/README.md`); and the scalar batch shadow
(`verus/batch/batch.rs`), "28 verified, 0 errors" plus five required red
mutants — a *mathematical shadow* of the scalar `FixedBook`, not an
executable-body refinement. Failing: the older kernel shadow
(`verus/kernel/lib.rs`, two `Seq::subrange` type errors then open division
obligations) and the accumulator shadow (4× `E0308`; "no proof log records
a pass"). Against `docs/VERIFICATION.md:96-110`'s eleven "Verus is expected
to prove" bullets, coverage is the register's **~1.5 of 11**; against
`docs/EVIDENCE_MATRIX.md`'s table, ~1.5 of 17 property IDs.

**Rocq** (`rocq/ClutchKernel.v`): **zero theorems** — the obligations are
`Definition … : Prop`, never `Theorem`, never `Admitted` (the file says so
itself). One conjunct of `successful_transition_is_well_formed` (`:426`) is
machine-checked *vacuous*: `resolve s o = Some s` needs the output to equal
an input that the Active-guard and Resolved-output make unequal — recorded
in the manifest's own gate note (`MANIFEST.baseline.json:1781`, gate
`proof.rocq_check`, "BLOCKER: the Rocq definition typecheck is not proof
content"). The toolchain is installed and pinned (Rocq 9.2.0); the
substrate produced nothing in the months it had.

### What the batch shadow's excluded-source discipline shows

`verus/batch/BATCH_ASSUMPTIONS.md` is the decisive exhibit about the
*correspondence-review model*, which is substrate-independent: the runner
SHA-256-pins the production sources and states plainly that "those digests
make the following review stable, but do not turn it into a machine-checked
refinement," while `relation_v1`/`relation_v1_stream` are recorded as
**excluded sources** — "these theorems are not proofs of the coupled
outcome-conservation, owner-pairing, AON-mask, portfolio, or streaming
relations bearing those names." That is: outside one 30-line
helper, Verus in this tree operates in exactly the mode Lean does —
mathematical model + digest-pinned human correspondence + named exclusions.
Its distinguishing advantage (proofs *of the executable Rust*) materialized
once; everywhere else the claim shape is identical and the tiebreak —
theorem throughput, ergonomics, ecosystem — is 212 checked theorems to 28
obligations, not close.

### What the ADR feared vs what happened

The ADR feared Lean "becoming mandatory by inertia." What happened is not
inertia: Lean became primary **by delivering** — including four corrections
to the design's own proof sketch and an unstated u128 bricking hazard
(`GOAL.md:975-983`) — while the designated substrates stalled (Rocq) or
narrowed (Verus), and the ADR's *underlying values* stayed honored: no
second production implementation, correspondence disclosed in every README,
no proof claiming to reach the adapter/runtime/ELF. And every live road on
the formal-methods horizon runs through Lean: the house AIR-in-Lean rule
(`docs/design/SUCCINCT_CLEARING_FEASIBILITY.md:20-23` — a STARK over a
Lean-emitted AIR); the Solana Foundation's Lean 4 sBPF semantics
(`docs/research/VERIFIED_BYTECODE_PATHS.md` §6/§8, the solanalib scoping
item); the Aeneas/Charon route with its Lean backend and our unusually
Aeneas-friendly kernel (`GOAL.md:995-997`). Option (b) would port 212
theorems into a substrate with no advocate, no output, and no future road,
to vindicate a record instead of correcting it.

## 3. Downstream cleanups a supersession unlocks

The register's H1 "blocked on it" is real: promotion criteria are written
against the dead architecture and are unsatisfiable-as-written, not pending.

1. **`docs/FEE_GEOMETRY.md` §7 rewrite** (`:236`): "Verus and Rocq close
   translation, homogeneity, complete-set invariance, bounded arithmetic,
   carry conservation, and partition-refinement invariance" — plus §6's
   tail (`:229-230`). The fee-base report's finding 3 already stages this
   as "rides H1" (`REPORT_fee-base-selection_2026-08-20.md:315-327`): keep
   the six properties' *content*, change only the prover. Until then no
   fee base can ever be promoted, including zero-fee's successor.
2. **Promotion-gate and policy language elsewhere** (the swept inventory):
   `docs/EVIDENCE_MATRIX.md:8-11` ("V1 is Verus-first …") and its per-row
   Verus/Rocq/Lean columns; `docs/VERIFICATION.md:76` ("Verus is the V1
   executable-kernel gate") plus its Rocq-model and rocq-of-rust sections;
   `docs/ENGINEERING_PLAN.md:489`; `docs/PARTITION_ALGEBRA.md:175`;
   `docs/ARCHITECTURE.md:37`; `docs/SPECIALIZED_BATCH_RELATION.md:287`;
   `docs/ACCUMULATOR_PLAN.md:203`;
   `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md:773`;
   `docs/SWARM_ROADMAP_2026-08-19.md` R5's "Install/pin Rocq … and prove"
   bullet; and the `docs/adr/README.md` index. Each is a one-line edit once
   ADR-0005 exists; none is editable honestly before it.
3. **H-cluster carried rulings** get their natural closure vehicle:
   **H3** — the native_decide ban, today an audit convention
   (`lean/README.md:73`) — becomes a written rule inside ADR-0005 (the
   appendix includes it). **H2(a)** — the E0/Verus probe posture — is
   subsumed: the recorded probe failure and E1 NO-GO stand as the
   documented reason the Verus lane stays narrow; re-author the probe only
   if a new checked-subset target is commissioned. H2(b)/(c) (vector-spine
   G1/G2, VM-INT trace naming) are substrate-independent and stay open.
4. **Not** unlocked, and worth saying: the `proof.rocq_check` gate is
   honest as written (typecheck-only, blocker note) and need not move in
   any reseal; `rocq/` stays in-tree as a historical specification —
   deleting it buys nothing and costs the record.

## 4. The assurance-ladder note

Ember's directive orders the next wave maturation → sophistication →
optimization → assurance, "formal verification deliberately last"
(`GOAL.md:34-46`). Honestly: **this decision changes nothing about that
ordering, and should not be sold as changing it** — ADR-0005 is a
governance-record correction, not a verification investment; it advances no
proof and gives no reason to pull assurance earlier. What it changes is the
*destination*: when the wave reaches assurance, the agenda is already scoped
in Lean's terms — `VERIFIED_BYTECODE_PATHS.md` §8's ranked list (the
account/authorization plane in Lean, where 100% of our P0s lived; the SVM
differential pointed at that model; solanalib tracking; the bounded Aeneas
spike) — instead of `VERIFICATION.md`'s eleven Verus bullets and a Rocq
extraction oracle that will never exist. It also stops interim waves from
minting *new* criteria against the dead architecture mid-flight, which is
how FEE_GEOMETRY §7 happened. One record fix now, so the last phase starts
true.

## 5. Recommendation and counterargument

**Recommendation: option (a).** Adopt ADR-0005 (appendix): Lean is the
proof substrate of record; Verus retained for narrow executable-body
contract results; the Rocq role retired with `rocq/` kept as a historical
specification; the Aeneas/Charon spike and solanalib scoping carried as the
named Rust-correspondence road; the native_decide ban written into the
record. Then execute the §3 cleanup list, FEE_GEOMETRY §7 first (it gates
cluster B).

**The counterargument that deserves an answer:** Verus is the only tool
that has verified *actual production Rust* here; Lean's 212 theorems are
about a hand-written model with manual correspondence. Ratifying
Lean-primary risks institutionalizing model-world comfort — proofs
accumulate fastest in the substrate farthest from the artifact.
*Answer:* the draft ADR concedes the premise and binds it: the claim
vocabulary already refuses the conflation (PROVED-MODEL vs
CHECKED-RUST-SUBSET), the Verus lane is retained precisely for
executable-body wins, and the Aeneas spike is the named test of whether Lean
can acquire the refinement arrow — with the recorded fallback that if it
fails, checked-subset growth happens in Verus, never by relabeling model
theorems. What the counterargument cannot justify is (b): re-staffing a
substrate whose entire output is a typechecked vacuous conjunct.

## 6. Execution cost

One documentation commit, no program source, no reseal: adopt the appendix
as `docs/adr/0005-lean-proof-substrate-of-record.md`, set ADR-0003's status
to `superseded by 0005`, add the `docs/adr/README.md` index line. The §3
one-liners plus the FEE_GEOMETRY §7 rewrite are a second small lane. Total:
well under a day of lane time, zero effect on any evidence identity.

---

## Appendix: draft ADR-0005 (ready to adopt)

```markdown
# ADR-0005: Lean is the proof substrate of record

Status: proposed (supersedes ADR-0003 on adoption)

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
