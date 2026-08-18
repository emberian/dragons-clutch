# Drift review — 2026-08-18 overnight swarm (dragons-clutch + degg-research)

Closing review of the night's ten dragons-clutch commits (`fa4efb4..f671156`)
and eight degg-research commits (`7005548..429192a`). Method: actual diffs and
code/test bodies read, never lane summaries; all suites re-run (kernel 16,
batch 44, accumulator 10, solana-reference 10, vertical 19, econ 83, dark-fba
9 — all green, all sub-3s); `scripts/check.sh` in degg green after fixes.
This review applied seven small doc/wording fixes (listed below) and made no
structural change. Nothing here is a claim upgrade: the repos remain tested,
not verified.

## Summary

| Dim | Scope | Verdict |
|---|---|---|
| A | Semantic ownership (dragons-clutch) | **PASS** |
| B | Policy canonization (P0-5) | **PASS (Rust)** — findings in the Python econ lab |
| C | Refusals not weakened | **PASS** |
| D | Vacuous-test hunt | **PASS** — oracle counts independently re-derived |
| E | Claim vocabulary (both repos) | **PASS** |
| F | Cross-doc coherence (dragons-clutch) | **PASS after fixes** — one doc-coverage gap remains |
| G | Fixture contract | **PASS** |
| H | Degg ledger coherence | **PASS after fixes** |

Worst finding severity: P2 (no correctness or refusal regressions found).

## Findings by severity

### P2

1. **Econ lab defaults silently select fee-policy variants (dim B).**
   `research/economics/model.py` carries Python default parameters inside
   exactly the P0-5 families:
   - `run_fee_schedule` (model.py:1746-1753): `domain=CarryDomain.INTENT`,
     `close_policy=CarryClose.TERMINAL_CEIL`,
     `side_arm=FeeSideArm.PER_INTENT_BOTH_SIDES`;
   - `allocate_fee` (model.py:570-575): 60/15/25 split, `executor_cap=None`;
   - `WeightedBook.open` (model.py:987) and `enumerate_weighted_traces`
     (model.py:1916): `policy=PayoutPolicy.KERNEL_BASELINE` (defensible — the
     landed arm, documented as a contrast arm — but still a default).
   Most call sites name every selector explicitly; the ones that actually rely
   on defaults are `test_fee_policy.py:53` and `:79` (`run_fee_schedule` with
   no domain/close/side named) and `fixtures.py:1285` (`allocate_fee` at the
   60/15/25 defaults). The Rust side is fully clean (no `Default` impl
   anywhere; `FrozenPolicyV1` doc forbids canonization by omission,
   relation_v1.rs:227-231). Lab-only exposure, but the defaults are exactly
   the shape P0-5 exists to prevent. Recommendation: strip the defaults or
   mark them PROPOSED at the definition site — structural, not applied here.

2. **`docs/implementation/VERTICAL_MODEL.md` is silent on the coupled path
   (dim F).** The vertical model gained a second, ~2,200-line clearing path
   (`clear_relation_v1` / `settle_relation_receipt`, `golden/coupled.trace`)
   and its architecture document does not mention it. Additionally, commit
   f671156's message said the trace-name deviation from design §14.3 and the
   unexercised R-b boundary were "flagged as design notes" — those notes
   existed nowhere in the tree. I recorded both in
   `BATCH_RELATION_V1_DESIGN.md` §18 (fix 4 below); the VERTICAL_MODEL.md
   coupled-path section itself is a real writing task and is left as this
   finding.

### P3

3. **Oracle counts are pinned only in prose (dim D).** The three exhaustive
   oracles assert only lower bounds in-test
   (relation_v1_tests.rs:791 `checked > 1000`, :884 `checked > 500`,
   :2171-2180 `accepted > 1000` / `refused > 100`) while the exact figures
   3,255 / 1,072 / 2,592×9 live in design §18 and the commit message. I
   re-derived all three counts combinatorially from the enumeration code and
   they are exact (4^6 box × 7 conservation-filtered conversion combos − 1
   empty book = 3,255; 3^8 × 5 combos − 1 = 1,072; 1,296 shapes × 2 owner
   layouts × (3 ticks × 3 imbalances) = 2,592 × 9). If a box is ever widened,
   §18 goes silently stale; consider pinning the exact counts in the asserts.

4. **DRAFT5 ledger's re-pin observed mid-flight state (dim H, degg).** The
   ledger's evidence re-pin section describes clutch-batch `relation_v1` as
   "uncommitted in-progress work" (19 extra tests) — true at 02:45 when
   written, committed eleven minutes later as f7caf04 with 33 falsifiers. The
   section already says it will be re-pinned once on filing day, so no edit
   was needed; noted so the filing-day re-pin isn't surprised.

5. **Commit a23c7e9 says "cross-language fixtures"; only the Python consumer
   exists (dims A/G).** `fixtures/economics/README.md` is accurate ("a future
   Rust consumer … is the other"); the commit message slightly overstates.
   No file wrong; no action.

### Nits (recorded, no action)

- `clear_batch_inner` (vertical lib.rs:1396-1399) computes
  `fee_revenue.checked_add(fee)` and discards the result before `add_fee`
  does it again — harmless pre-check inside a staged transaction.
- `materialize`/`dematerialize` (kernel lib.rs:460, :494) re-run an identical
  `check_invariants()` mid-function; documented as the supply-neutral analogue
  of the prospective check, but it is literally the same call as at entry.
- `transfer_internal`'s u128 equal-and-opposite check (kernel
  lib.rs:770-777) is arithmetically unreachable defense-in-depth; the commit
  message calls it a "post-condition", which is fair, but it proves nothing a
  test could not — same status as `redeem_complete_set`'s documented
  unreachable remainder branch.
- econ `experiments.py` uses "verified_cells" as a variable name for
  exhaustively enumerated cells — loose vocabulary internal to a falsifier.

## Dimension detail

**A — semantic ownership: PASS.** The coupled settlement path moves claims
only through `transfer_claim` → kernel `transfer_internal`
(vertical lib.rs:870-893, :1662-1668) and derives every draw from the frozen
`PairingWitnessV1`: `pair_plan` (lib.rs:541-566) and the settlement plan read
`pairing.slices` / `settled_by_slice` exclusively — there is no model-side
pairing choice on that path. The scalar path's model-owned pairing is
retained deliberately and documented as the permanent regression lab (module
doc, lib.rs:14-27). `relation_v1` does not duplicate kernel arithmetic (it
clears in price units; virtual split/merge counts are relation bookkeeping,
not kernel transitions); the outcome bound is restated but guarded by a
compile-time equality assert (lib.rs:99). The vertical model's fee stays the
model's own by an explicit `FeeBaseV1::None` selection with the
one-fee-owner rationale written at the site (lib.rs:718-720). The econ lab's
kernel mirror is labeled as a mirror, pinned to the kernel via hand-authored
fixtures, and promotes no constant.

**B — policy canonization: PASS (Rust), findings above (Python).** Every
Rust selection is explicit: no `Default` impl in any crate; kernel
`TransferPhasePolicy` is a required argument at every call; the vertical
model names all eleven families at one construction site with per-selection
rationale and PROPOSED marking (`proposed_relation_policy`, lib.rs:690-738),
plus `PROPOSED_SEARCH_BOUNDS` and T-a named-as-PROPOSED at both settlement
call sites (lib.rs:1259-1264, :2057-2059, :1844-1847). The vector spine's
"decided default proposed above; none is applied" (G2) is a proposal to a
human, not a silent selection.

**C — refusals: PASS.** solana-reference `Resolve`/`RedeemInternal` refuse
unconditionally at the match arm (programs/solana-reference/src/lib.rs:770-772);
the program was untouched tonight (only `programs/README.md` changed) and its
10 tests pass. `run_verus.sh` retains exit 2 / 3 / 4 refusals (not installed
/ off-pin version or frontend / tampered source) and `exec`s the real result.
`verify_ignoring_claimed_aggregates` is documented as **not** an acceptance
entry point (relation_v1.rs:2372-2379), is called only from tests and from
`canonical_candidate`, which then calls the full `verify` before returning
(relation_v1.rs:2564-2567); `propose_best_valid` compares only candidates
that passed full `verify`. The vertical model refuses 1b-free at clear time
(before any charge) and P-b / N-c refuse as unimplemented variants. degg's
DarkTarget refusal test passes. Both AGENTS.md changes relax only local-commit
gating per the user's 2026-08-18 direction; push/publish/production gates and
all refusal language are intact.

**D — vacuous-test hunt: PASS.** Read: all 16 kernel bodies, all 19 vertical
bodies, 8 batch falsifier bodies plus the three oracles in full, econ
`test_alignment.py` in full plus two experiment bodies. Tests assert exact
expected values, full-prestate equality after refusals, contrastive arms, and
non-vacuity counters. Oracles 1-2 are two-sided (constructor completes ⟺
feasibility inequality; on completion the witness must re-verify and be
deterministic; on refusal the table must be infeasible and the error is
pinned to `ConstructorStalled`); oracle 3 checks every accepted candidate
against full `verify`, independently recomputed conservation identities, and
constructor pairability. Domain enumeration is exhaustive, not sampled, and
the counted domains match the §18 claims exactly (derivation under P3-3).
The P1-B contrast test at the model boundary is real: scalar arm charges
fee 1 on unpairable volume then refuses settlement; coupled arm clears
volume 0, fee 0 (vertical lib.rs:3291-3342).

**E — claim vocabulary: PASS.** dragons-clutch: no new achievement language;
"verified" appears as the relation's `verify` verb with the module-header
disclaimer ("not a verified claim in any proof-assistant sense"); the
"best valid submitted candidate" discipline is uniform across relation_v1.rs,
the design doc, and the vertical model; README/crates/programs status lines
were honestly updated (installed-and-pinned ≠ proof closed). degg:
LEGAL_ANALYSIS.md labels every paragraph VERIFIED/SOURCED/INFERRED/PROPOSED
with a §9 per-citation ledger (28 verified, 2 flagged unverified and not
relied on); GUARDED_EVENT_FOUNDATIONS.md keeps its body label-free and
resolves all 25 claims in Appendix A to evidence class + exact path:line at
pinned commits — I verified C-18's kernel line refs are exact at the pinned
`fa4efb4` (they would be wrong at HEAD; the commit table makes them right).

**F — cross-doc coherence: PASS after fixes.** All six documented deviations
in the relation module docs are real in code: V4-before-V3 refusal precedence
(verify_inner), `ConstructorStalled` instead of the §8.4 slack floor (oracle-
pinned), debits-up/credits-down with the named pot (relation_v1.rs:2053-2087),
AON 2b honored-at-full-size (:1503), dispersion-weighted score subtraction,
`ReceiptFloor` per filled leg (:1934-1946). §18 correctly scopes the two that
correct the design text. POLICY_ANALYSIS ↔ ECONOMICS_LAB addendum are
consistent; the one genuine refinement (complete-set primitive must be
lot-gated under candidate (b), addendum finding 1 vs §1.5's "composes with
every candidate") now carries a pointer (fix 2). TOOLCHAIN_SPIKE addendum,
PINNED_PROOF_TOOLS.md, and the README status line agree. The `run_lab.sh`
residual was resolved as a wording-only fix (fix 1): the `compatibility=`
token grammar was deliberately preserved because TOOLCHAIN_SPIKE.md:78 pins
the historical transcript; only the stale informational `verus=` /
`verus_probe=` lines changed, and no refusal exists on that path to weaken.

**G — fixture contract: PASS.** Expectations in `fixtures.py` are
hand-authored literals ("authored by hand from the policy analysis, not
generated from the model"); the model functions imported there are used only
by the replay/consumer helpers, so the lab is graded against the vectors, not
the other way around — no circularity. Regeneration is deterministic
serialization of the same literals, compatible with
"failing fixture is a finding, never edited." TRC-001's `kernel_baseline` arm
was checked against the actual kernel: both single-outcome redemptions refuse
`remainder_required`, `redeem_complete_set(1)` pays exactly 1, final state
all-zero — byte-for-byte the behavior pinned by the kernel's
`complete_set_redemption_exits_the_fractional_trap` test.

**H — degg ledger coherence: PASS after fixes.** Supersession pointers are
present at the top of all three DRAFT4 ledgers; the consolidated map covers
V-01…V-38 with the V-22+ collision renumbered and a read-through rule; the
Draft 5 verdict uses consolidated IDs consistently (V-29/V-34 appear only
consolidated). Every JOHN_REVIEW_PACKET judgment row resolves into an
existing LEGAL_ANALYSIS section (§1-§9; "§7.3" = §7 item 3, routing). Three
self-stale status notes were found and fixed (fixes 5-7): the ledger's
cross-repo note about the dragons-clutch README (corrected the same night by
a23c7e9), and LA §8 items 1-2 plus the John-memo defect row, which described
as "queued"/"being aligned" two fixes that landed in the very commit that
flagged them. Typst bodies and PDFs were not touched, per instruction; hedge
preservation was reviewed through the audit verdict's row-level trail, not
re-derived line-by-line.

## Fixes applied (all wording/status; no code semantics, no refusals touched)

1. `/Users/ember/dev/dragons-clutch/toolchain/scripts/run_lab.sh` — the two
   informational Verus status branches now point at `run_verus.sh` /
   `PINNED_PROOF_TOOLS.md` instead of the stale "not installed" / "release
   not yet selected" wording; `compatibility=` token grammar unchanged;
   `sh -n` clean.
2. `/Users/ember/dev/dragons-clutch/docs/implementation/POLICY_ANALYSIS_LOTS_FEES.md`
   §1.5 — executed-correction note: under candidate (b) the complete-set
   primitive must itself be lot-gated (ECONOMICS_LAB addendum finding 1).
3. `/Users/ember/dev/dragons-clutch/docs/implementation/VECTOR_SPINE_PROPOSAL.md`
   R7 — dated note: resolved in code by d60ccf3 (check-before-write is now
   structural and stated in rustdoc); the `post_state_on_error` manifest field
   remains the per-surface pin.
4. `/Users/ember/dev/dragons-clutch/docs/implementation/BATCH_RELATION_V1_DESIGN.md`
   §18 — vertical-model integration record: landed trace name
   `golden/coupled.trace` deviates from §14.3's `relation_v1.trace`; R-b's
   rounding pot is carried but never drawn on that path (recorded, not
   exercised).
5. `/Users/ember/dev/degg-research/docs/regulatory/DRAFT5_CLAIM_LEDGER.md` —
   cross-repo README note now records the same-night fix (a23c7e9).
6. `/Users/ember/dev/degg-research/docs/regulatory/LEGAL_ANALYSIS.md` §8 —
   items 1-2 marked applied-in-the-same-change-set.
7. `/Users/ember/dev/degg-research/docs/regulatory/JOHN_REVIEW_PACKET.md` —
   defect row: "fix queued"/"being aligned" → fixed before the memo was
   finalized.

## Morning decisions (deliberately left open by the night's work)

1. **Residual-settlement freeze (1a / 1b-canonical / 1c).** All three are
   implemented and tested end-to-end (vertical model, six-permutation
   idempotence, per-variant diagnostics); 1b-free is refused at clear time
   for its documented strand hazard and would need a terminal sweep
   authority. Evidence is now executed, not prospective.
2. **Fractional-payout candidate — (a1) one-hot / (b1) lots / (c) credit,
   plus the landed complete-set primitive.** All four arms executed with
   exit-liveness (not just solvency) walks; the primitive alone exits P1-A;
   TRC-003 shows one materialized leg of a fractional set strands permanently
   even with the primitive — any freeze must say whether a set may be
   assembled across the internal/external boundary; under (b) the primitive
   must be lot-gated (addendum finding 1).
3. **Transfer phase T-a vs T-b.** Kernel implements both; both models select
   T-a as PROPOSED at named call sites; T-b's behavior is pinned by kernel
   test. Freezing T-a requires the epoch/resolution ordering rule §14.2
   leaves open.
4. **Fee-policy arms.** Terminal-ceil vs dropped-carry executed (dropped
   collects zero on dust while volume is positive; wash-sign matrix strict
   under terminal-ceil); κ=4/1000, 60/15/25, executor cap all remain
   unpromoted experimental arms. Also decide the econ-lab default-parameter
   cleanup (finding P2-1).
5. **VM-INT flagged items (commit f671156).** (a) accept or rename the
   coupled golden trace vs design §14.3's `relation_v1.trace`; (b) R-b
   price-unit/atom rounding boundary is recorded per ledger but not
   exercised — needs an exercising test before any R-b freeze.
6. **Vector-spine gates G1-G5** (taxonomy shape; rulings on R1-R11 — R7 is
   now resolved in code, R8's merge check order remains a live intentionality
   ruling pinned by regression test; encoding rules; comparison rules;
   ownership/direction).
7. **E0/Verus probe posture.** The pinned Verus rejects the pinned probe
   (missing vstd prelude); fixing it changes the pinned digest. Decide:
   author a new reviewed probe + digest, or keep the recorded failure. E1
   remains NO-GO either way.
8. **VERTICAL_MODEL.md coupled-path section** (finding P2-2) — assign a
   writing lane.
9. **degg filing week:** John's four one-minute questions (the Q1 sentence,
   signature block, one-route-or-two, risk-register sanity); the filing-day
   single re-pin of the evidence section at a frozen commit; docket re-checks
   before Aug 24 / Aug 27 deadlines.
