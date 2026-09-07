# MERGE NOTES — family `recovery-capture`

Branch `build/recovery-capture` at `c6e84d9fc`, rebased onto `main` at
`1775f134d` (the merge queue's own tip after ten landings and the wave's
evidence document). 18 commits, 60 files, +7,478/−680.

## 1. READY

**READY.**

Everything this family touches compiles and its tests run. Measured at
`c6e84d9fc`, with one `CARGO_TARGET_DIR` private to this worktree:

| what | result |
|---|---|
| `cargo check -p dclutch-source -p dclutch-resolution-proof-sbf --tests` | green |
| `cargo test -p dclutch-source --lib` | **306 passed**, 0 failed |
| `cargo test -p dclutch-resolution-proof-sbf --lib` (filtered: `tests::each_ensemble`, `tests::legacy_v1`, `market_admission_v1`, two `core_effect` rows) | 18 passed |
| `cargo check -p dclutch-core-sbf --tests`, `-p dclutch-svm-harness --tests` | green |
| `cargo test -p dclutch-journey-campaign -- provider::tests` | 3 passed |
| `cargo test -p dclutch-local-successor-bootstrap -- pyth_lab_publication` | 6 passed |
| `cargo check -p dclutch-ladder-campaign` | green |
| `lake build DClutchSemantics` | green, 147 jobs; the only `sorry`s are `ScoringRuleV1`'s two, which the package README declares |
| `crates/dclutch-source/check-generated.sh` | exit 0 |

Gates, at this HEAD:

| gate | verdict |
|---|---|
| `selftest` `census` `emission` `citations` `budgets` `fmt` `locks` `release` `journey` `reference` | **PASS** |
| `commands` | NOT RUN — `dclutch` is declared by `tools/dclutch-cli` and is neither built under `target/` nor on `PATH`. Not this branch's. |
| `seam` | **FAILED, and this branch adds nothing to it.** Measured against a clean `git worktree` at `main`: main reports 28 findings, this HEAD reports the same 28, byte-identical. §5 has the arithmetic. |
| `clippy` | **FAILED, and this branch REMOVES one of its reds.** Five packages red outside `clippy-debt.tsv` — `dclutch-claims`, `dclutch-market`, `dclutch-pre-market-funding-test-caller-sbf`, `dclutch-resolution-core-v3-operator`, `dclutch-route-census` — and no file of any of them is touched here. `dclutch-source` was a sixth on main and is green here (§5). |

**Not run, and this branch cannot run them**: `frames`, `programs`,
`sbfcontracts`, `suites` (all need `cargo build-sbf`), `abi` (needs
`wasm-bindgen`), `web`, `guards` in full, `root-targets`, `sbom`, `witness`
(devnet RPC), `workspaces`. `guards` was run once and reported two failures,
both main's: `crates/dclutch-market/check.sh` (a clippy
`assertions_on_constants` in `protocol_parameters/tests.rs:430`) and
`tools/direct-translation-validator/check.sh` (which was `dclutch-source`'s
clippy red, and is repaired here).

**The one thing `frames` will say.** Two new `#[inline(never)]` symbols enter
the Resolution link — `process_ensemble_fold` and `process_reclaim_member_seat`
— and neither has a row in `tools/gates/frames-baseline.json`. The gate "names
the commits that owe rows"; this branch owes two. Both are written in the
file's own frame discipline (`plan_and_encode_ensemble_fold` and
`commit_ensemble_fold` are separate `#[inline(never)]` frames, exactly as the
failure walk's and the crank's are, and for the reason those doc comments give)
but that is a claim about the source, not a measurement of the artifact, and
only `cargo build-sbf` settles it.

## 2. SHARED FILES — for the merge lane

Nothing here is a fight: this is the twelfth family and the queue is otherwise
empty, so every shared file below was **applied in this branch** rather than
handed on. They are listed because they are regenerated artifacts or
tree-wide registers, and a later landing moves them again.

1. `tools/gates/emission-coverage.md` — regenerated (`tools/gate emission
   --write`). 101 generated files, 101 guarded. If anything lands after this,
   regenerate; do not hand-merge.
2. `docs/reference/*`, `docs/reference/abi/*`,
   `packages/dclutch-sdk/lib/generated/{refusalRegistryV1,routeCensus,marketPhaseAdmissionV1}.ts`
   — regenerated to the fixpoint (`tools/gate reference --converge`, pass 2
   moved nothing; `--check --converge` then reports this revision IS the
   fixpoint). They carry `EnsembleMember 0x8029`, `EnsembleQuorum 0x802A`,
   `EnsembleQuorumMet 0x802B` and both new routes. **Regenerate again after any
   later landing rather than merging these by hand.**
3. `tools/gauntlet/blocked.json` — two `structural` rows added, one per new
   route, each saying why it is unreachable and telling the reader to delete it
   the day a founding writes an ensemble material.
4. `tools/seam-audit/baseline.json` — exactly two rows added, both for this
   family's own code, both taking the verdict their existing sibling carries
   (`hazard-privilege-pin` for `validate_relay_frame_with_tail_v1`, beside
   `validate_relay_frame_v1`; `inventory-guard-present` for
   `ensemble_v1.rs`). Nothing else in that file was touched, and in particular
   main's 28 outstanding findings were NOT absorbed.
5. `crates/dclutch-source/src/relay/decode.rs:359` — **one line of main's, not
   this family's.** The collapsible `if` that landed with the native venue made
   the whole `dclutch-source` package clippy-red, and a red package hides every
   package above it, so nothing in this family's crate could be measured while
   it stood. Collapsed here.

**Codes taken: `0x8029`, `0x802A`, `0x802B`,** in that order
(`EnsembleMember`, `EnsembleQuorum`, `EnsembleQuorumMet`), contiguous behind
`RelayedVenueKind = 0x8028`. The wave's plan was that this family "renumber
around" `deploy-lifecycle`'s `0x801E`; **that is not possible for any branch in
isolation**, and the reason is worth recording. `pin_refusal_band!` asserts the
discriminants are the *contiguous run from the band base* — a hole is a compile
error, so a branch cannot leave `0x801E` free and take `0x801F` before the
branch that fills `0x801E` has landed. The only workable order is the one the
queue used: land, then renumber against what is actually there. This branch was
rebased after ten landings and took the first free run.

## 3. What this lane completed, and what it ruled

### Completed

- **`process_ensemble_fold`** — the physical outer the whole family was written
  against and which did not exist. It authenticates the Source state, the
  Market, five Source records, the whole frame (fixed prefix plus a `2k` tail
  whose width is the authenticated material's own `k`), each member seat at its
  own derived address, the Product graph, the manifest and the ledger; plans
  through `plan_ensemble_fold_v1`; encodes both records so a shape either
  Lean-owned schema would refuse never reaches an account; then writes the
  terminal certificate, the fold receipt, the debited ledger and every consumed
  member's bounty to the captor its own fragment named — all in one transaction
  or none of it. `plan_ensemble_fold_v1`, `plan_member_bounty_releases_v1`,
  `validate_relay_frame_with_tail_v1`, `ensemble_fold_tail_v1`,
  `ENSEMBLE_FOLD_FRAME_PREFIX_V1`, `EnsembleFragmentSeatSeedsV1`,
  `EnsembleFoldReceiptSeatSeedsV1` and `EnsembleFoldInstructionV1` all had no
  caller anywhere in the tree; they have one now.
- **`process_reclaim_member_seat`** — the second undispatched action. It is
  where `EnsembleMember` means exactly what its doc says: a member byte at or
  above the material's `k` is refused *before* an address is derived from it.
- **The five call-site seams** (C-1, P-2..P-5) and the two `E0004` matches.
- **The ensemble's laws as tests**, by the exact names `EnsembleResolutionV1.lean`
  proves them under (§4).
- **The ladder tier runs both arms** and no witness of either arm is evaluated
  against the other's transcript (§4, and R7 below).
- **Two workspaces outside the root one** — `tools/relayer` and
  `tools/gauntlet/relayed-vertical` — matched exhaustively over
  `RelayAccountNameV1` and were red on the three variants this family added.
  Only the `journey` gate compiles them; the BUILD doc named neither.

### Two frames grew, and why

The seams' frames could not serve the routes they were written for.

- `ENSEMBLE_FOLD_FRAME_PREFIX_V1`, 25 → **29**: it carried no primary
  `SourceSpecV1` pair and no `StatisticSpecV1` pair, so the fold had no way to
  authenticate member zero's route (the primary provider release) or the
  source-to-result shift the median reaches the selector on. Without them both
  would have had to come from the caller, which is a caller choosing the
  outcome.
- `RECLAIM_MEMBER_SEAT_FRAME_V1`, 6 → **10**: it carried no Core pair and no
  material pair, so `k` was unknowable and `EnsembleMember`'s stated conjunct —
  refuse the member byte *before* deriving a seat address — could not be
  enforced at all.

### Rulings (provisional; ember rules by reversal)

- **R7 stands, in a stronger form.** The tier runs both arms
  (`--walk both`, the default, one arm at a time against one port block — the
  same default `run-relayed-vertical.sh` already carries). Beyond that, the
  witnesses are now **arm-scoped files**, so a witness is never *evaluated*
  against the walk it is not about. The old shape was worse than R7 assumed in
  both directions: the two capture witnesses said `if .walk != "capture" then
  <the expected value>` and went green on the arm that never asked their
  question, and the failure arm's three said in their own provenance that they
  are red on the capture walk *on purpose*. A green that cannot tell a silent
  instrument from a silent chain is not evidence; a red nobody is meant to read
  trains a reader to ignore reds.
- **R9 (new). `observed_fragments = 0` from every existing frame is EXACT
  today, not optimistic, and it has a named trigger.** The crank/fold
  exclusivity conjunct (`require_primary_is_the_ladders`) takes a count of
  observed fragments, and every existing frame passes zero. That is correct
  while **no route writes a fragment**: the only producer of a kind-1
  certificate is the provider execute, and `select_rung`
  (`programs/dclutch-resolution-proof-sbf/src/provider_v3.rs`) refuses
  `(member, Primary)` by name with `SourceLadder` — a member capture is exactly
  that pair; the relay's `ConsumeRecord` gained a `source_index` byte for the
  same declaration and `process_consume` does not read it. **The moment a
  member-capture route exists, `ensemble_seat_tail_v1` must be threaded onto
  the crank's and the failure walk's frames in the same change**, or
  `EnsembleQuorumMet` becomes a code with a raiser that can never fire and the
  crank can steal the decision from a met quorum. `ensemble_seat_tail_v1` is
  written and, deliberately, still has no caller.
- **R8 stands** (the rung's records are derived by content identity because the
  founding's evidence map does not publish them) and now says so at the site:
  `tools/local-validator/bootstrap/successor/src/market.rs`, beside
  `recovery_policy_record`.
- **The fold's `Fragment` refusal maps to `EnsembleMember`.** That code's
  documented conjunct is "a member byte the material does not declare"; it now
  also carries "member `m`'s seat is not this market's fragment for member
  `m`". Both are one accusation with one place to look — seat `m` — which is
  why they are one code and not two.

## 4. The tests, and what each proves

**The ensemble's laws** (`crates/dclutch-source/src/source_resolution_v2.rs`,
under the Lean names `EnsembleResolutionV1.lean` proves them by). All six were
run BROKEN first — one expected cell and one expected refusal flipped turns two
of them red — so the assertions are the measurement and not decoration.

| test | what it proves |
|---|---|
| `an_attacker_below_the_bound_cannot_move_the_cell` | three honest of five hold cell zero against two manipulated readings at either end (`2·2 < 5`) |
| `exactly_half_can_move_the_cell_up_and_not_down` | at exactly half the bound is one-directional: `[50,60,200,300]` → 200 in cell 1, `[50,60,−5,−7]` → 50 in cell 0. The asymmetry is `exact_median_by`'s rank, `count / 2`, taking the upper of an even count's two middles |
| `an_even_quorum_is_not_foundable` | where that asymmetry is refused: at the founding, while the decoder still admits every `1 ≤ q ≤ k ≤ 5`, because the theorems hold for all of them |
| `fewer_than_the_quorum_engages_the_ladder` | short of its quorum an ensemble WITH a rung is the ladder's, and the crank enters slot `k − 1` — the first rung after the members, never slot zero; with the quorum answered the crank refuses `EnsembleQuorumMet` rather than complaining about a deadline |
| `the_fold_never_stalls` | exhaustively over every `n` a `k=5, q=3` market can observe: below the quorum exactly the fallback fires and the fold refuses; at or above it exactly the fold decides and the fallback refuses. Never both, never neither |
| `a_single_source_market_folds_to_todays_selection` | the fold refuses a market that declared one source, and today's primary transition still answers it — the ensemble route cannot re-decide a market that never declared one |

**The wire and the frame** (`crates/dclutch-source/src/relay/`):

- `an_unknown_action_refuses` was **RED and nobody had run it**: it put action
  byte `9`, which `ReclaimMemberSeat` took, so it was decoding a well-formed
  action of the wrong width and reading `InvalidLength` as its refusal. It now
  asks `RelayActionV1::decode` which bytes are actions and puts every byte that
  is not one.
- `the_ensemble_fold_and_the_seat_reclaim_round_trip_as_their_own_actions`.
- `a_consumption_that_names_no_source_index_is_the_primarys_own_bytes` — the
  claim that made `source_index` safe to add to a live wire, executed: a
  primary consumption and a member's differ in exactly one byte, at exactly the
  offset the reserved span gave up, and the five bytes after it still refuse.
- `a_fold_frame_is_its_prefix_and_two_positions_per_declared_member`,
  `a_member_seat_passed_twice_cannot_answer_twice` (a seat aliased into two
  member positions, and a captor aliased onto the funding ledger),
  `a_writable_member_seat_and_a_read_only_captor_both_refuse`.
- `each_frame_accepts_exactly_its_own_shape` gains `AdvanceRecovery` and
  `ReclaimMemberSeat`; the crank's frame had never been in that loop.

**The program** (`programs/dclutch-resolution-proof-sbf/src/tests.rs`):
`each_ensemble_refusal_reaches_its_own_published_code` — every variant of the
pure fold's error maps to a DISTINCT published code (the coarse-refusal rule,
executed), which is what gives `EnsembleMember` and `EnsembleQuorum` their
first raisers, and `FundedWalkErrorV1::QuorumMet` reaches `EnsembleQuorumMet`
rather than the deadline's `Transition`.

**The rung capture** (`tools/gauntlet/journey/src/provider.rs`):

- `the_captured_publication_is_the_pinned_fixture_byte_for_byte` — the only
  thing that makes parameterizing the transport safe. The journey's primary
  walk sends exactly the bytes it always sent, and the journey itself cannot
  say so, because a transport that quietly sent different bytes would still
  resolve a market and still go green.
- `a_rung_capture_and_a_primary_capture_do_not_share_a_stage_label`.
- `a_rungs_records_are_addressed_by_both_their_schema_and_their_body` — the
  separation property that makes deriving a rung's records by content identity
  safe where the founding's evidence map does not publish them.

**Lean**: `the_members_are_staggered_by_one_second`, the witness for the
corrected `membersShareTheWindow` (see §5, L-4).

**The ladder tier's witnesses**, now three files:

- `witnesses.json` — arm-independent only, plus a new
  `no-crank-was-refused-before-it-was-built`: `drive_crank` treats the
  too-early refusal as the tier's own hostile satisfied and every other
  preflight refusal as a real stop, and nothing counted the second kind.
- `witnesses-exhaust.json` — new. `this-transcript-is-the-exhaust-arm` (the
  positive control), `the-exhaust-arm-cranked-onto-the-rung-and-then-off-it`
  (sequences 2 and 3; deliberately NOT that the second crank landed, because a
  leg past `--max-wait-seconds` is reported not-yet-due by design),
  `the-exhaust-arm-answered-nothing`, and the failure arm's three moved in.
- `witnesses-capture.json` — the two rung witnesses with their `.walk`
  disjuncts REMOVED, behind the arm pin.

## 5. What a reviewer should distrust in `BUILD_recovery-capture.md`

Every load-bearing claim was checked. These are the ones that did not hold.

1. **§2 seam L-2 — "`formal/dclutch-semantics/README.md` module census:
   `EnsembleFoldReceiptV1Abi`". There is no module census.** That README is a
   prose taxonomy; its "ABI modules (`*Abi.lean`, …)" bullet already covers the
   new module generically, and the only modules it names individually are the
   five mechanism ones, of which `EnsembleResolutionV1` is already there.
   Nothing to add. Seam reclassified as a no-op, not applied.
2. **§2 seam T-6 — "the ladder's conservation ledger declares
   `LamportClaimV1::inapplicable` at the crank boundary, so L7 does not hold
   there". False.** The only two `inapplicable` claims in `ladder.rs` are at the
   FOUNDING boundary, and each already carries its reason. Both crank
   boundaries use `LamportClaimV1::fees(...)`, and the file says in its own
   words why L7 holds exactly there: the worker who cranks is inside the
   watched set (`ledger.watch("ladder_crank_worker", …)`), so the bounty's
   payer and payee are both watched.
3. **§5 — "`source_material_v3.rs`: … every clause refuses by name" and the
   receipt's corpus tests were reported green; they are, and I ran them. But
   the same sweep left `relay::instruction::tests::an_unknown_action_refuses`
   RED**, on a byte the family itself had claimed. A family that adds an action
   must re-run the wire tests, not only the ones it wrote.
4. **§7 — "the five errors" reproduced exactly**, all five, at the branch
   point. That claim holds.
5. **§1.6b FINDING 1** (`journey.rs:698` red at the branch point) held and is
   now moot: main fixed it at `6400193b7`, and the rebase resolved to main's
   line with its comment.
6. **§1.6b FINDING 2** (the pre-advance hostile cannot reach the conjunct it is
   named after, and records the refusal verbatim) held on inspection and is
   preserved: `a-capture-before-the-rung-is-due-refuses-by-name` asserts the
   outcome word, never a discriminant.
7. **The BUILD doc names no seam for `tools/relayer` or
   `tools/gauntlet/relayed-vertical`**, and both were red on the three new
   `RelayAccountNameV1` variants. Neither is in the root workspace; the
   `journey` gate is the only thing that compiles them.
8. **The BUILD doc's frames were under-specified for their own routes** (§3).
   Read the frame tables against what the route must authenticate, not against
   the doc.

**And two facts about `main`, not about this family**, both measured:

- `tools/gate seam` is red on `main` with **28 findings** — measured in a clean
  `git worktree` at `main`, then diffed against this HEAD's: identical. Most
  are the ten landed families' (`dclutch-claims-sbf`, `dclutch-operator`,
  `dclutch-trading/scoring_rule`, `dclutch-custody`,
  `dclutch-resolution-core-v3-operator`, `derived_transport_v1.rs`). They want
  one triage pass by whoever owns the wave, not twelve.
- `tools/gate clippy` is red on `main` in six packages outside the debt list.
  This branch repairs one of them (`dclutch-source`, §2 item 5) and touches no
  file of the other five.

## 6. What is still owed — named, so it is debt and not silence

The ensemble is complete on the CONTRACT and complete as a ROUTE, and is
unreachable on any chain. Two producers are missing, and each is its own piece
of work rather than a missing test. Both are now `structural` rows in
`tools/gauntlet/blocked.json`, which the route census prints.

1. **No founding writes an ensemble material.** `SourceMaterialV3::with_ensemble`
   sets the three bytes and `EnsembleSpecV1::validate_foundable` states the
   odd-quorum conjunct, and Core's founding calls neither; the successor's
   market compiler has no input that declares `k`. Every material on any chain
   reads `EnsembleSpecV1::SINGLE`, so the fold's first conjunct refuses.
2. **No route writes a fragment** — see R9.

Also never started, and named as such in the BUILD doc's §1.7 and §1.8: the
operator, the successor driver, the `tools/cohort/steps.tsv` row for
`found-ensemble`, and the TypeScript decoder for the fold receipt.

## 7. One sentence the merge lane most needs

**Take the codes as they stand (`0x8029`–`0x802B`) and regenerate
`docs/reference/` plus the three SDK mirrors after landing rather than merging
them by hand** — everything else in this branch is self-contained, and the two
gates that are red here are red on `main` for reasons no file of this family
touches.
