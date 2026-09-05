# SIMPLIFY-DOCS — what moved, what was rewritten, what was deleted

Branch `simplify/docs`, base `330bbfaba` (main at 2026-09-04). Domain:
`docs/**` and the root documents. `docs/reference/**` is generated and was not
hand-edited. Every measurement below names the tree it was taken in:
`/private/tmp/claude-501/-Users-ember-dev-dragons-clutch/ef2920a4-77e9-4597-99e3-94569deb51f7/scratchpad/simplify-docs`.

## Line counts, base → HEAD

| what | before | after |
| --- | ---: | ---: |
| `GOAL.md` | 4,992 | 100 (an index) |
| `WAVE.md` | 7,112 | 11 (a tombstone) |
| `AGENTS.md` | 348 | 201 |
| `docs/OMISSION_INDEX.md` | 106 | 65 |
| `README.md` | 226 | 240 |
| `docs/` excluding `docs/ledger/` | 119,471 | 101,204 |
| `docs/ledger/` (verbatim history, dated) | 0 | 31,782 |
| `docs/` total | 119,471 | 132,986 |
| `docs/design/` | 29,803 | 28,341 |

The `docs/` total rises because 31,782 lines of history that lived at the
root (`GOAL.md`, `WAVE.md`, `SESSION_STATE.md`, the previous `AGENTS.md`) and
under `docs/` top level (letters, backlog, board archive) now live under
`docs/ledger/`, verbatim. Root and top-level `docs/` prose a lane is expected
to read went from 13,676 lines (`GOAL`+`WAVE`+`AGENTS`+`README`+the seven
top-level docs) to 1,596.

## One index, several stores

- **`GOAL.md`** is an index: what the project is, the standing goal, the
  attractor, and a table of dated deltas, one line each, linking the ledger
  entry (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#Lnnn`) and the store
  the fact lives in (a decision record, a cohort document, a design note).
  The narrative moved verbatim to `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md`,
  line numbers intact; all 201 `GOAL.md:NNN` citations in the tree were
  rewritten to that path and still land on the same line.
- **`WAVE.md`** → `docs/ledger/WAVE_2026-08-26_to_2026-09-02.md` verbatim
  (last commit 2026-09-02; nothing appended since). A one-paragraph tombstone
  at the root names where its live rows went (decision records, `AGENTS.md`,
  `blocked.json`, `GOAL.md`); 60 `WAVE.md:NNN` citations rewritten. Code
  comments that name `WAVE.md` bare still resolve to the tombstone.
- **`docs/ASPIRATION_LEDGER.md`** (a 2026-08-27 audit with dated amendments,
  not a live ledger) → `docs/evidence/ASPIRATION_LEDGER_2026_08_27.md`
  verbatim, with a tombstone at the old path so `M-`/`G-`/`N-` identifiers
  cited from code comments and decisions keep resolving.
- **`docs/evidence/DEBT_OWNERSHIP_LEDGER_2026_09_01.md`** was already a dated
  evidence document; left in place.
- Moved into `docs/ledger/` verbatim: `SESSION_STATE.md`
  (→ `SESSION_STATE_2026-08-31.md`), `docs/board-archive-2026-08-27.md`,
  `docs/START_HERE_2026_09_01.md`, `docs/HANDOFF_CODEX_2026_08_31.md`,
  `docs/LETTER_TO_CLAUDE_2026_09_01.md`, `docs/LETTER_TO_CODEX_2026_08_31.md`,
  `docs/VALIDATION_BACKLOG.md` (→ `VALIDATION_BACKLOG_2026-08-31.md`), the
  previous `AGENTS.md` (→ `AGENTS_2026-09-03.md`), and three cohort-9 design
  records (`COHORT9_PLAN_REVIEW_2026_08_31.md`,
  `COHORT9_CLOSEMAKER_RULINGS_2026_08_31.md`,
  `PROFILE_SUCCESSION_HANDOFF_2026_08_31.md`).
- Moved into `docs/evidence/` verbatim (dated measurements, not designs):
  `LITERAL_SWEEP_CONVICTIONS_2026_08_31.md`, `ORPHAN_DESIGNS_TRIAGE_2026_08_30.md`.
- **Deleted**: `docs/design/FIRST_VERTICAL_SLICE.md` (superseded by its own
  2026-08-27 banner; cited by nothing live).
- **`docs/OMISSION_INDEX.md`** keeps only what is deliberately not built: the
  O- rows and the P- rows, one line each with the record that owns them.
  The fifteen U- rows (a backlog whose statuses had not moved in four days,
  whose subjects are the contract's C-rows) are retired; P-005 and P-006 are
  closed and kept by identifier. Its narrative cells carried three of the
  rehearsals' stale claims (P-006 "zero `CloseSeal` occurrences", P-007's
  counts, P-008's `NEVER-EXECUTED`); they went with the cells.
- **`docs/MASTER_COMPLETION_CONTRACT.md`**: the work-queue paragraph that sent
  crews to "55 never-executed routes" (the register says the set is empty)
  now says the set is derived from the generated register; the decision
  register keeps one line per question and points every ruled one at its
  record (0018, 0024, 0025, 0026, 0027, devnet succession), with the
  registered-Direct row corrected to what `hot_v3.rs` does today.
- `INTENT.md` kept whole (ember's voice with provenance); two dead citations
  fixed. `PROJECT_METHOD.md` and `ARCHITECTURE.md` banners point at the
  decision records and the design notes instead of `WAVE.md`.

## Design notes

Heads rewritten to state the current truth, the prior text byte-identical
below a `## History` fold, and every line citation into the note from outside
`docs/ledger/` shifted by the head's length (so decision 0029's
`PACKET_LIMIT_2026_09_01.md:362-373` still lands on the options table):
`PACKET_LIMIT_2026_09_01` (+43), `BASIS_ABI_UNIFICATION_V1` (+29),
`CLAIM_CHECK_COMPACTION_V1` (+40), `CLIFF_DOCTRINE_V1` (+36),
`MAINNET_STATE_RELAY` (+38), `REGISTRY_FINALIZATION_OBSERVATION_2026_09_02`
(+41), `RELEASE_LINEAGE_MIGRATION_V1` (+41), `DEVNET_DEMO_DEPLOY` (+26, its
"PREPARATION ONLY — nothing executed" status replaced by what was executed
and what superseded it). The NOTES lane had already done
`DEALER_PARTIAL_REMOVE_COMPUTE`, `OBSERVATION_SCALE_AUTHORITY` and
`SPONSORED_WINDOW_ADMISSION`. The six mechanism notes are untouched.

## Stale claims (the two C-16 rehearsals)

In this domain, corrected: `README.md` (all five — the closed cohort's market
address, "updated in place, same addresses", "resolution runs in a test
harness", "none of the three driven on a validator", "does not enable devnet
trading"); the sixteen guide passages ("not live yet", "permanent addresses",
"one market open", "payout not open", "still to come: the first trade") in
`docs/guides/{README,reader,operator,trencher,client-developers}.md` and
`docs/operators/README.md`; the two documented commands that could not run
(`devnet-permanent-substrate-capture-v1` without its seven role flags; the
private-validator one-liner without `--seeds 1`, in `README.md` and
`reader.md`); the contract's decision register (six rows); the omission
index (seven rows); `client-developers.md`'s 360-byte `DCLTCOR3`;
`INTENT.md:216-217` and `:419-420`; `ARCHITECTURE.md`'s `DCLTCOR2` banner;
`WAVE.md`'s "Updated: 2026-08-26"; the entry list's denominators (a
supersession banner rather than new numbers, since they move daily). The
`ARCHITECT_SCHOLAR` verdicts the 2026-09-01 closeout said were owed an
addendum (§C2, §B4) have a dated one.

Not in this domain and left for their makers: the component READMEs
(`tools/ci`, `tools/gauntlet/tier2`, `tools/gauntlet/tier4`,
`programs/dclutch-claims-sbf`, `programs/dclutch-core-sbf`,
`packages/dclutch-sdk`, `tools/dclutch-cli`), the generated
`docs/reference/refusals.md` promise ("every error code"), and the missing
`apps/dclutch-web` README.

## Evidence

The three cohort documents that have a machine-readable witness
(`docs/evidence/witnesses/cohort-{13,14,15}-discovered.json`, plus
`cohort-13-founding.json`) now say at their head that the witness and the job
directory are the authority and the prose is a hand copy kept for its
findings. **Deliberately not done**: replacing each hand-copied number inside
7,000 lines of cohort prose with a reference — the witness files carry
signatures and slots, not the derived arithmetic the prose reasons with, and
a sentence-by-sentence rewrite of dated evidence would author a second copy
of the same facts. The attractor's evidence-from-witnesses shape is the
cohort runbook's to emit from now on, not a retrofit.

## Controls

- `tools/genref/generate.sh --converge --check` reaches its fixpoint on the
  branch at pass 3, exactly as at the base commit `330bbfaba`. At both the
  same eleven generated files move (`routeCensus`, `budgets`, `programs`,
  `refusals`, `routes`, `route-witnesses`, `README`, `decisions`, the client
  mirrors) — pre-existing census drift a convergence owner regenerates; the
  regenerated `decisions.md` at HEAD differs from the base's only by the
  aspiration ledger's new path in decision 0009's status sentence, so the
  markers the generator parses (`Status:` paragraphs, titles) are intact.
  `main` has since advanced to `9b9c31926`, where the check refuses earlier
  on `substrates.json`'s `ladder` row — not this branch's. No build
  directory was left under this worktree; the scratch worktree used for the
  comparison was removed.
- Inbound links: every `GOAL.md:NNN`, `WAVE.md:NNN`, `OMISSION_INDEX.md:NN`
  and moved-file reference outside `docs/ledger/` rewritten; relative
  markdown links that resolve to no tracked file are 20 at base and 20 at
  HEAD (none added). The three the moves introduced were fixed.
- `tools/doc-citations` judges Rust doc comments, not markdown; not run.

## Left deliberately

- `docs/ledger/*` is verbatim; historical text inside it still names old
  paths, on purpose.
- `docs/evidence/C16_ENTRY_LIST_2026_09_01.md` and
  `DEBT_OWNERSHIP_LEDGER_2026_09_01.md` are kept as dated evidence (the entry
  list with a banner) rather than corrected in place.
- `docs/design/EVIDENCE_REFRESH_V1.md` (1,883), `FLOWFUL_IA_V1.md` (1,215),
  `FAMILY_ROOT_TAILS_V1.md`, `FUNDED_CRANK_CAPABILITY_CLOSE_PATCH.md` (its
  second half is still unlanded), `dealer-v2-scenario-collateral.md`: no
  addenda and no record supersedes them; not rewritten.
- `docs/research/*`, `docs/compost/*`, `docs/recovered/*`: dated, cited,
  untouched.
- `docs/decisions/DECISION_PACKET_2026_08_30.md` sits beside the numbered
  records and is indexed by the generator; left.
- The simplification map is cited from `GOAL.md` at the path it will take on
  convergence (`docs/design/SIMPLIFICATION_MAP_2026_09_04.md`), not copied.
