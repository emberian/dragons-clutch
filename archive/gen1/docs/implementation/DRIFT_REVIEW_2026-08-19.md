# Drift review — 2026-08-19 close (waves 3/4: dragons-clutch + degg-research)

Closing review of the wave-3/4 swarm output: dragons-clutch `245c965..6e494a0`
(the `d60ccf3..f671156` sub-range was already deep-reviewed by
`DRIFT_REVIEW_2026-08-18.md` and got a light re-touch only; the deep pass here
is the post-`5b38578` work) and degg-research commits after `429192a`
(`1a4da56`, `7a101ab`, `a1a57c1`, `728b215`, `0c22ae7`; the live typst lane
and everything uncommitted were skipped). Two GOAL.md-only commits (`35da3a9`,
`68c3f67`) landed during the review and carry no code.

Method: actual diffs, code, and test bodies read — never lane summaries. Core
paths (evidence gate, SBF processor, vendored crate, byte vectors, layout
closure, P2 closures) read directly by the reviewer; docs/manifest coherence
and the degg night commits were read by scoped sub-reviewers reporting
file:line, with every load-bearing claim spot-verified against the tree.
Suites re-run locally, all green, all sub-3-minutes: layout 38, reference 20,
kernel 16, accumulator 24+2, batch 44, vertical 19, sbf-harness 4, econ 83,
collateral-profiles 28, cost-lab unittest 28, static-client npm 11, degg
independent-oracle workspace 7+16+11+8 and toy 9 — plus the full 300M-case
differential sweep, reproduced in 7.4 s with counts matching the committed
addendum pair-for-pair.

This review applied seven small wording/status fixes (listed below) and made
no code or structural change. Nothing here is a claim upgrade: both repos
remain tested, not verified.

## Summary

| Dim | Scope | Verdict |
|---|---|---|
| A | Semantic ownership (evidence gate, derive_payout, SBF, SupplyLedger) | **PASS** — one flagged-debt finding (digest dual-impl, P3-4) |
| B | Refusal integrity (evidence-absent, sealed flag, obligation 18, DarkTarget) | **PASS** |
| C | Vacuous-test hunt (15+ bodies across five suites) | **PASS** — byte vectors independently derived; nits recorded |
| D | Claim vocabulary (new docs, both repos) | **PASS after fixes** — three degg memos breached the authenticated line (fixed) |
| E | Cross-doc coherence (adapter doc, GOAL, manifest vs handoff) | **PASS** — coverage gap for the SBF lane (P2-2) |
| F | Vendored crate (checksum, verbatimness, license) | **PASS** — checksum and byte-identity verified; no license text file (P3-5) |
| G | Morning decisions | consolidated below |

Worst finding severity: **P2** (no correctness or refusal regressions found;
both prior-review P2s were addressed by the swarm — see below).

## Prior-review P2 closure check

1. **Econ Python defaults (P2-1): CLOSED by `7ca01ad`.** All four named sites
   are now required arguments with P0-5 rationale in the docstring
   (`allocate_fee` model.py:570, `run_fee_schedule` model.py:1767,
   `WeightedBook.open` model.py:1001, `enumerate_weighted_traces`
   model.py:1940 — remaining defaults there are search bounds, not policy
   selectors). Verified by AST scan of every remaining default across the lab:
   the residue is experiment parameterization (`exp_fee_*` sweep sizes,
   `carry=0` accumulator seeds), not family selection. 83/83 green.
2. **VERTICAL_MODEL.md coupled section (P2-2): CLOSED by `7ca01ad`.** The new
   §"Coupled clearing path" (VERTICAL_MODEL.md:71-195) is accurate against
   the code as reviewed on 08-18, records both flagged deviations (trace name,
   R-b unexercised), states the P1-B contrast and the `VirtualLegNotHosted`
   refusal, and closes with an explicit no-claim-upgrade paragraph.

## Findings by severity

### P2

1. **Three degg memos described the accumulator as combining "authenticated
   observations" (dim D) — fixed.** `data-q12-q18-data-quality.md:94-95`,
   `definitions-q5-narrow-based-index-status.md:88`, and
   `definitions-q15-reference-integrity.md:80-81` presented authentication as
   implemented, while the artifact's own header says "Nothing here
   authenticates anything" (`crates/clutch-accumulator/src/lib.rs:53-55`).
   The memos' "tested, not formally verified, not deployed" ceilings did not
   cover this gap. This is the one place tonight crossed the
   evidence-gated ≠ authenticated line. All three sentences now state that
   source authentication is an assumed input contract the prototype does not
   implement (fixes 3-5). The Draft 6 filings themselves are clean — the word
   appears once, as design description, not artifact claim.

2. **The manifest's 28/28 gates do not cover the newest lane (dim E).**
   `MANIFEST.baseline.json` covers every command CODEX_HANDOFF §5 names (plus
   the coupled trace as `5-extended`) — nothing in §5 is missing. But
   `programs/clutch-sbf` (4 harness tests, the reproducible-ELF check,
   clippy/doc), `benchmarks/tests` (28 unit tests), and `cost_lab.py
   abi-audit` are documented commands (`benchmarks/README.md:10-14`,
   SBF_BRINGUP §Reproducing) that appear in neither §5 nor the gate
   inventory, and CODEX_HANDOFF.md never mentions clutch-sbf at all. The
   clean-tree emit's green is real but does not witness the SBF lane.
   Structural: add the gates and a handoff §5 entry — not applied here.

### P3

3. **abi-audit's identifier values are hardcoded (dim C/E).**
   `benchmarks/cost_lab.py:1901-1909` (`RUST_IDENTIFIER_VALUES`) pins
   `MAX_OUTCOMES=16`, `ORDER_RECORD_BYTES=99`, etc. by hand. The audit
   re-derives all 15 `account_len` constants from the real
   `programs/solana-layout/src/lib.rs` and refuses expression-level drift
   both directions (:1957-2021), but a Rust change to an identifier's
   *definition* with unchanged `account_len` expressions re-derives the old
   widths and passes; the codec-digest mismatch is a printed note, not a
   failure (:1966-1973) — demonstrated live against the current tree. The
   drift refusal is real for expression drift only.

4. **The parent collateral digest has two implementations and no executed
   cross-language pin (dim A).** `dragons-clutch/profile/v1` is computed in
   `programs/solana-layout/src/lib.rs:271` and re-implemented in
   `research/collateral-profiles/model.py:633`; `identity_vectors.json` has
   no Rust consumer (only `test_profiles.py` and the manifest digest it).
   The layout test (lib.rs:4897) pins the domain string but not a golden
   vector. This is *flagged* debt — SOLANA_REFERENCE_ADAPTER.md obligation 19
   already declares the Rust golden vectors unwritten — but the plan's
   "cross-language golden vectors" phrasing should not be read as executed.

5. **The vendored crate ships no license text (dim F).** Verified:
   `vendor/solana-define-syscall-5.1.0` is byte-identical to the
   cargo-verified unpack (only `.cargo-ok` absent, exactly as PROVENANCE.md
   states) and the claimed sha256
   `21e14a4f…d581d` matches the sparse-index `cksum` for 5.1.0 exactly. But
   the upstream package carries no LICENSE file (SPDX `license = "Apache-2.0"`
   metadata only), so the repo redistributes Apache-2.0 code without the
   license text §4 asks for. If the crate stays after sign-off, add
   `vendor/LICENSE-APACHE` beside (not inside) the verbatim tree.

6. **GOAL.md's "Attestation survey" entry has no artifact (dim E).**
   GOAL.md:59-66 records survey conclusions (parsing-lane grades, four named
   gaps, provenance red flags) but no commit in either repo contains the
   survey; `35da3a9`/`68c3f67` are GOAL-only. The entry honestly lacks
   "; pushed", but "landed" in the commit subject overstates. It needs a home
   or a pointer to one.

7. **Static-client "kernel-true terms" is a vocabulary gate, not a checked
   equality (dim D/E).** The client's release integrity is real (embedded
   mirror held equal by vm-executed deep-equality + digest recompute,
   runtime Web Crypto recheck, strict CSP with no inline escapes, honest
   stubs). But the *kernel* linkage is a hand-transcribed string in
   `apps/static-client/terms.json:9-10` plus a regex gate
   (`test/smoke.mjs:66-77` forbids floor/truncate/round, requires
   exact/refuse/complete-set). No test reads `crates/clutch-kernel`; a kernel
   semantics change would not turn the client red.
   `docs/implementation/STATIC_CLIENT.md:71-72` describes the gate honestly;
   only the commit-message phrase oversells.

8. **degg DRAFT5 ledger's Draft 6 hash rows went stale (dim E) — annotated.**
   The typography commit `0c22ae7` rebuilt all four PDFs (+1 page each on the
   long three) without re-pinning `DRAFT5_CLAIM_LEDGER.md`'s "Draft 6
   artifacts" hashes, making "currently in `output/pdf/`" false at HEAD. A
   dated status note now records this (fix 6); the row is deliberately not
   re-pinned while the typst lane is live — the filing-day freeze (ledger
   gate 4) re-pins once at the frozen commit.

### Nits (recorded, no action)

- `mirrored_bounds_match_their_owning_crates` (layout lib.rs) asserts
  restated literals — the honest comment says so, but the name suggests an
  import comparison that no dependency edge allows.
- `page_set_closure_accepts_one_dense_ordered_frozen_set` asserts mostly
  constructor facts; the falsifying work lives in its sibling
  (`page_set_refuses_gap_duplicate_reorder_and_post_freeze_mutation`), which
  is genuinely adversarial (gap, cross-page duplicate, reorder, post-freeze
  mutation with digest-repair attempt, broken chain link, thawed page).
- The 0c22ae7 commit *title* says "content byte-preserved"; the body's own
  claim (character multisets modulo repeated table headers at new page
  breaks) is the accurate one and was verified mechanically.
- `MANIFEST.baseline.json` `check` can never green against a moving HEAD
  (baseline.commit/tree_hash drift by construction); the 37 digests still
  match today. BASELINE_MANIFEST.md's "several minutes" for `--run-gates`
  ran in 8 s warm — conservative-direction error.
- The manifest claims block is not literally all-false: the three
  achievement flags (`verified`/`deployed`/`release`) are false, and
  `reviewed_offline_checks_recorded: true` is backed by the 28 recorded gate
  runs. The commit's phrasing named only the three flags; accurate.

## Dimension detail

**A — semantic ownership: PASS.** `apply_inner`
(programs/solana-reference/src/lib.rs:967) owns no economics: every balance
move is a `pure_market` kernel call (`split`/`materialize`/`dematerialize`/
`resolve`/`redeem_internal`), the fold is the accumulator's own
`open → observe → witness_feed_cursor → seal → result` machine
(fold_window_evidence, lib.rs:1152), and `derive_payout` lives in exactly one
place (src/resolution.rs) — `resolve_from_evidence` calls it and refuses
`PayoutIndexMismatch` unless the request names exactly the derived index.
The SupplyLedger creates no second truth: `apply_inner` rewrites the ledger
terms from the same kernel-produced position aggregates every transition, and
`validate_aggregate_closure` (lib.rs:1589) — run on entry *and* before
encoding — is the only join, checking the two-term sum against
`kernel.total_supply` plus the single-position identification, with the
multi-position replacement obligation stated in its doc. The clutch-sbf
processor (programs/clutch-sbf/program/src/processor.rs) re-implements
hostile validation by design and calls `clutch_kernel` for the split; the
collateral-cap and cash-debit pre-checks are mirrored adapter semantics, in
the reference's order, and SBF_BRINGUP §Layering names this independence as
what makes the differential "two adapters over one kernel rather than a
function with itself" — a linker-map check proves the offline `apply` is
absent from the ELF. The SBF program has no supply-ledger account at all
(nine accounts, closure checked directly); now recorded as deferred check 13
(fix 2). Finding P3-4 (digest dual-impl) is the one ownership seam.

**B — refusal integrity: PASS.** Evidence-absent `Resolve`/`RedeemInternal`
still produce `ResolutionEvidenceUnavailable` as a missing code path — `apply`
takes no evidence parameter (lib.rs:937), and both regression bodies are real:
`signer_cannot_bypass_missing_resolution_evidence` (lib.rs:2429) tries an
arbitrary signer and then owner-signed redemption against internally coherent
forged resolved bytes; `adapter_refuses_resolution_and_redemption_without_typed_evidence`
(lib.rs:2471) pins the pre-evidence fixture for both actions and refuses
`UnexpectedEvidence` for a layout intent with evidence attached. No
caller-supplied sealed flag exists anywhere on the path: `WindowResult` has
private fields, exactly one constructor (`WindowAccumulator::result`,
crates/clutch-accumulator/src/window.rs:631, reachable only after `seal()`
enforces completeness and maturity), and a `compile_fail` doctest blocks
handing a bare `Summary` to a settlement-shaped function. Obligation 18: there
is no boundary-table input to refuse — the frozen `TermsAccount` has no field
that can carry one, `ResolutionTerms` is built only by `from_market_terms`
(ordinal partition, derived, never caller-supplied), and every non-pinned
registry value the terms *can* express refuses (`TermsMalformed`, exercised in
`unimplemented_policies_and_inadmissible_statistics_refuse`, lib.rs:3017). So
nothing mis-resolves against committed semantics; the committed semantics are
exactly the ordinal ones, and threshold semantics are inexpressible — the
plan-annotation wording now says this precisely (fix 1). degg: DarkTarget
refuses in both oracles — original
(`experiments/dark-fba/src/lib.rs:250`, `DarkBackendAbsent`, tested at
:867-877) and independent
(`experiments/dark-fba-independent/oracle/src/admit.rs:199-200`,
`DarkTargetUnavailable` as the *first* screen check, three tests) — all four
tests run green.

**C — vacuous-test hunt: PASS.** Read in full: seven reference bodies (the
two evidence-absent regressions, the split byte vector, the full lifecycle
byte vector, forged-resolved redemption, unimplemented-policies, prefix-seal
partial), four layout bodies plus the `verify_page_set` implementation, the
four sbf harness bodies, and the degg differ's compare/report path. The
reference happy-path byte vector
(`evidence_gated_resolution_and_redemption_have_exact_byte_vectors`,
lib.rs:3569) **independently derives** its expectations: full-array equality
against copies of the pre-state mutated only at hand-named offsets with
semantically derived values (lifecycle byte 131, kernel phase/payout 34/35,
resolution record fields from named constants, redemption zeroing, cash back
to 100) — not copied output — and its pre-state anchor is itself pinned by
the independently hand-built split vector (lib.rs:2264). The layout page-set
closure is two-sided: `verify_page_set` independently sums per-page counts
against the head's committed total and recomputes the set digest, and the
mutation suite proves recomputing a page digest cannot repair the set
commitment. The sbf harness tests are known-vector encoding tests (base58
against the pinned CLI as oracle, RFC base64 vectors, short-vec u16); the
real processor test is the SVM differential, which SBF_BRINGUP honestly
names as the only test of `process` (deferred check 12) and which is
demonstrated falsifiable (one-byte expectation mutation goes red on exactly
that account). The static-client smoke executes the embedded mirror in a vm
realm and deep-equals plus digest-recomputes — not string-contains. The degg
differ generates inputs from its own domain enumerators and compares
field-by-field; neither side's output feeds the other's expectation, and the
in-suite bounded domains (~866K cases) are backed by the committed 7-second
full-sweep binary whose counts reproduce the addendum exactly. Weak spots are
the nits above plus P3-3.

**D — claim vocabulary: PASS after fixes.** dragons-clutch: no achievement
language in SBF_BRINGUP.md (whose "Correct description" section is a model of
the discipline), BASELINE_MANIFEST.md, the COST_LAB addendum, or the
static-client docs; CU discipline verified at the data level — the only CU
figures are three pinned protocol constants with source URLs, the `_cu`
column is empty in all 56 landed and 12 differential rows, `cost_lab.py`
refuses `_cu` keys in landed arms, and the one measured CU (72,869) is a real
measurement labeled as one fixture, "not an envelope". The
"evidence-gated is not authenticated" sentence appears verbatim at the top of
resolution.rs, in the adapter doc, and in the plan — held everywhere in
dragons-clutch. degg: held everywhere except the three memo sentences of
finding P2-1 (fixed); LANDSCAPE's 11 comments all carry retrievable docket
identifiers with paraphrase visibly distinct from quotation and the one
unretrieved comment flagged rather than characterized; the eight committee
memos carry the blanket status header (per-claim VERIFIED/SOURCED labels
would be the stricter house style — recorded, not required). PROVENANCE.md is
complete and, per dim F, exactly true.

**E — cross-doc coherence: PASS.** SOLANA_REFERENCE_ADAPTER.md matches the
landed code exactly: the eleven-step gate order is the code's order
(validate_evidence_metadata → signer → active → bind_terms → payout-set →
bind_resolution → from_market_terms → fold → check_domain → derive_payout →
index equality), the refusal-taxonomy table is one-to-one with the `Error`
enum, the V1 pin table matches `from_market_terms`, and the "Exact byte
evidence" offsets match the test bodies byte-for-byte. The
RESOLUTION_EVIDENCE_PLAN and ADVERSARIAL_REVIEW annotations correctly
supersede their stale status prose (the promotion-gate annotation hedges
"authenticated sources remain a future SVM obligation" — right resolution).
GOAL.md's done-log maps to real commits with matching content in both repos,
"pushed" is true against origin, and the one unbacked entry is finding P3-6.
Manifest vs handoff §5 is finding P2-2. Cost-lab annotations (261-row update,
1,819-byte page note with commit ref) are accurate. (2026-08-19: the page is
now 3,883 bytes — sixteen 228-byte tag-discriminated order slots, OrderPage v3,
commit da2fbf7 — and the cost lab was re-pinned to it; the note above is
accurate as of the commit it cites.) degg's night commits are
consistent with the consolidated ledger — six load-bearing Draft 6 claims
spot-checked against V-17/V-38/N-1..5/V-37/V-33 all hold verbatim, forbidden
phrases appear only negated — except the stale hash rows (finding P3-8,
annotated).

**F — vendored crate: PASS.** `diff -r` against the cargo-verified unpack:
byte-identical, `.cargo-ok` alone absent, exactly as PROVENANCE.md claims.
The claimed sha256 recomputed from the sparse-index cache entry for 5.1.0:
exact match. No `.crate` archive exists on this host and the panamax mirror
is absent — consistent with PROVENANCE's stated reason for vendoring at all —
so the archive-level chain of custody is cargo's own checksum-verified unpack
(which wrote the `.cargo-ok` marker). The workaround is honestly framed
(build plumbing, not a fork; deletion instructions included). Remaining item
is the license text (finding P3-5) and the user sign-off itself (decision 4).

**G — morning decisions** — every open decision across both repos:

1. **Policy freezes, now with executed evidence** (carried from 08-18, all
   still open): residual-settlement variant (1a / 1b-canonical / 1c;
   1b-free refused at clear time); fractional-payout candidate (a1/b1/c +
   the landed complete-set primitive; cross-boundary set-assembly question;
   lot-gating under (b)); transfer phase T-a vs T-b (needs the §14.2
   epoch/resolution ordering rule); fee-policy arms (terminal-ceil vs
   dropped-carry, κ, 60/15/25, executor cap — all unpromoted). The econ
   default-parameter cleanup from the 08-18 list is done (`7ca01ad`).
2. **Obligation-18 TermsAccount revision.** V1 pins ordinal cells; a
   threshold or TWAP-family market is inexpressible until a revision carries
   statistic id, ambiguity policy id, coverage parameter, source/evaluator
   versions, and a boundary table + payout map inside the digest. Decide
   whether to author it (and STAT-05's 256-bit comparison question rides
   along) or leave the family closed.
3. **VM-INT flagged pair** (carried): accept `golden/coupled.trace` or
   rename to §14.3's `relation_v1.trace`; R-b's rounding boundary is
   recorded but unexercised — needs an exercising test before any R-b
   freeze.
4. **Vendored-crate sign-off**: accept `solana-define-syscall 5.1.0`
   (verified verbatim, checksum-matched) or drop it once the registry
   archive is reachable; if kept, add the Apache-2.0 license text beside the
   verbatim tree (P3-5).
5. **degg filing week**: read Draft 7 when the rebalance lands (LANDSCAPE:
   formal-methods ground is a corpus-wide zero; P-D5 crowded, P-R6
   preempted); John hand-off — the four one-minute questions (Q1 sentence,
   signature block, one-route-or-two, risk-register sanity); filing-day
   gates — the single evidence-section re-pin at a frozen commit *and* the
   ledger gate-4 PDF hash re-pin (rows currently annotated stale after the
   typography rebuild); docket re-checks before Aug 24 / Aug 27.
6. **Operatorless-agent IAC addendum**: the memo + addendum candidate is in
   flight; decide whether the addendum files. Adjacent queued decision from
   the attestation survey: one paid Bedrock session to produce the first
   D-grade provider-attested transcript.
7. **Manifest/handoff coverage** (P2-2): bless the SBF lane into
   CODEX_HANDOFF §5 and the manifest gate inventory (clutch-sbf tests,
   reproducible-ELF check, benchmarks unittest, abi-audit), and give the
   attestation survey an artifact home (P3-6).
8. **E0/Verus probe posture** (carried): the manifest now pins the probe
   expected-failing with the reason inline; decide re-author (new digest) vs
   keep the recorded failure. E1 remains NO-GO either way.
9. **Vector-spine gates G1-G5** (carried): taxonomy shape, R8 merge-order
   intentionality ruling, encoding/comparison rules, ownership/direction.

## Fixes applied (all wording/status; no code, no refusals touched)

1. `/Users/ember/dev/dragons-clutch/docs/implementation/RESOLUTION_EVIDENCE_PLAN.md`
   — top annotation: "refuses threshold boundary tables" → precise statement
   that a boundary table is inexpressible in the frozen TermsAccount and that
   every expressible non-pinned registry value refuses (obligation 18
   unchanged).
2. `/Users/ember/dev/dragons-clutch/docs/implementation/SBF_BRINGUP.md` —
   deferred check 13: the SupplyLedgerAccount has no on-chain counterpart;
   the nine-account set carries no supply PDA, closure is checked directly,
   and the six-account differential cannot observe supply-ledger drift.
3. `/Users/ember/dev/degg-research/docs/regulatory/research-memos/definitions-q5-narrow-based-index-status.md`
   — "combines authenticated observations" → combines supplied observations;
   source authentication named as an assumed input contract the prototype
   does not implement.
4. `/Users/ember/dev/degg-research/docs/regulatory/research-memos/definitions-q15-reference-integrity.md`
   — same correction ("over supplied observations (source authentication is
   an assumed input contract, not implemented)").
5. `/Users/ember/dev/degg-research/docs/regulatory/research-memos/data-q12-q18-data-quality.md`
   — same correction in the design-description sentence.
6. `/Users/ember/dev/degg-research/docs/regulatory/DRAFT5_CLAIM_LEDGER.md` —
   Draft 6 artifacts heading corrected ("identify the pre-typography build")
   plus a dated status note: 0c22ae7 rebuilt all four PDFs, hashes/page
   counts above are pre-typography, re-pin deferred to the filing-day freeze
   while the typst lane is live.
7. `/Users/ember/dev/degg-research/experiments/dark-fba-independent/README.md`
   — "third independent rule enumerator" → "third, deliberately naive rule
   enumerator" with its sharing of parameter constants and
   `required_reservation` stated (independent of the toy, not of this crate).
