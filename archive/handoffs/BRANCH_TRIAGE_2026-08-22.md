# Branch and worktree triage — 2026-08-22

Read-only archaeology against `main` at `1d9d4e4` (2026-08-22 13:27 -0400).
Nothing in this survey was merged, deleted, checked out, or stashed. §7 is the
copy-paste cleanup script; it is not run here.

---

## 0. Headline

**Every one of the 16 unmerged branches is superseded. Zero HARVEST. One KEEP,
and that KEEP is a *merged* branch pinned by live docs.**

| Disposition | Count | What |
| --- | --- | --- |
| **KEEP** | 1 | `r2-caps-rebase-trial` — merged, but three live docs cite its tip hash |
| **HARVEST** | 0 | nothing on any branch is absent from `main` |
| **DELETE** | 27 | 16 unmerged (all superseded) + 11 merged leftovers |
| worktree registrations to remove | 34 | ~65 GB on disk, 0 currently prunable |

The strongest single fact: **not one file touched by any unmerged branch is
missing from `main`.** Every path exists today. Across all 16 branches, ≥97 % of
branch-added lines appear in `main` *verbatim*, and every residual line is dated
status prose, a stale digest, or a pre-refactor identifier.

---

## 1. Method, and why "probably superseded" became "provably superseded"

Four tests, run per branch:

1. **`git log main..<branch>` + diffstat** — the raw claim.
2. **`git cherry -v main <branch>`** — patch-id equivalence. A `-` mark means an
   equivalent patch already sits in `main`'s history (the rebase-merge signature).
3. **File-existence sweep** — for every path in `merge-base..branch`, does
   `main:<path>` resolve? (Answer, for all 16 branches: yes, always.)
4. **Line-level residual** — of the lines the branch *added* relative to its own
   merge-base, how many are absent from `main`'s current version of that file?
   This is the sharp test: it excludes `main`'s own later deletions, which a
   naive `git diff main <branch>` would mis-report as branch-unique work.

Test 4 is what turns the survey's "probably" into evidence. A branch can be
patch-equivalent and still have drifted; a branch can look 234 lines ahead in
`solana-layout/src/lib.rs` and in fact be 234 lines *behind* a refactor `main`
did afterward. Test 4 separates those.

Merge-base spread: 11 of 16 branches fork at `414d6e4`, `6743b9d`, `9fd1ef1`,
`b5da74f`, or `1326c9d` — all 2026-08-19. `main` is **370 commits** past
2026-08-19 and 354–440 commits ahead of each branch tip.

---

## 2. The unmerged 16 — per-branch disposition

### 2.1 The Direct V3 family (3 branches) — DELETE

Three branches carry overlapping snapshots of the Direct V3 line. `main`
absorbed all of it via **`codex/r3-direct-v3-successor` (`733a6c8`, "Re-measure
the CU table on the rebased tree")**, which *is* an ancestor of `main`.

| Branch | Ahead | `git cherry` | Branch-added lines absent from `main` |
| --- | --- | --- | --- |
| `backup/r3-direct-v3-successor-prerebase` | 26 | 25 `-`, 1 `+` | **122 / 11,982 (1.0 %)** |
| `codex/r3-direct-v3` | 13 | 12 `-`, 1 `+` | **91 / 6,050 (1.5 %)** |
| `fable/v3-settle-port` | 15 | 14 `-`, 1 `+` | **112 / 6,969 (1.6 %)** |

**The single `+` commit is the same one on all three:** `fe40bac` "Add dedicated
Direct V3 request envelope". It is not orphaned — `main` carries **`582f9ad`**,
same subject, and `582f9ad` is *exactly* `fe40bac` **minus the
`programs/solana-reference/Cargo.lock` hunk** (verified by diffing the two
patches: lines 1–138 of `fe40bac`'s patch, the entire Cargo.lock portion, are
the only delta; the `lib.rs` half is byte-identical). The rebase dropped that
hunk because the lock content had already landed — `main`'s
`programs/solana-reference/Cargo.lock` contains `clutch-batch-policy-identity`
at line 29 today.

`main` is a strict superset of the branch code in every V3 file:

| file | prerebase | `main` |
| --- | --- | --- |
| `direct_selection_v3/common.rs` | 569 | 571 |
| `direct_selection_v3/staged.rs` | 964 | 964 |
| `direct_selection_v3/terminal.rs` | 1,406 | 1,413 |
| `direct_selection_v3/freeze_abort.rs` | 486 | 490 |
| `instructions/orders_batch.rs` | 3,406 | **4,013** |
| `solana-layout/src/direct_selection_v3.rs` | 3,109 | 3,161 |
| `svm-tests/tests/direct_selection_v3.rs` | 2,361 | **2,859** |
| `research/batch-policy-identity/src/direct_lifecycle_v3.rs` | 5,905 | 6,071 |

Every symbol that the branch-vs-`main` diff shows as "removed" is still present
in `main` — it moved, it did not die: `DIRECT_RESERVATION_V2_BYTES` (31 hits),
`DIRECT_EPOCH_V4_BYTES` (45), `read_epoch_v4_boxed` (12),
`read_reservation_v2_boxed` (14), `DIRECT_CANDIDATE_STATUS_REVERIFIED` (13),
`DirectReservationV2Account` (21), `frozen_pair` (8).

`codex/r3-direct-v3`'s 34-line residual in
`programs/clutch-sbf/program/src/instructions/direct_selection_v3.rs` is the
*self-described dead end*:

> `//! Unrouted Direct V3 lifecycle adapter.`
> `//! This module is compiled and host-tested, but [crate::dispatch] does not`
> `//! route any Direct V3 tag to it. … Freeze, staged verification, settlement,`
> `//! and lapse are still runtime STOPs.`
> `_ => Err(ClutchError::NotYetImplemented.into()),`

`main` has the routed, complete family — the whole `direct_selection_v3/`
subdirectory with `staged.rs`, `terminal.rs`, `freeze_abort.rs`. This is an
abandoned checkpoint whose successor shipped.

> **DELETE** all three. Superseded by `733a6c8` (merged) and `582f9ad`.

---

### 2.2 The R2 pull-profile / source-provider family (3 branches) — DELETE

| Branch | Ahead | `git cherry` | Residual |
| --- | --- | --- | --- |
| `fable/r2-runtime-capabilities` | 1 | 1 `-` | **0 lines** |
| `codex/r2-production-source-rwv8` | 6 | 6 `-` | **5 lines** |
| `codex/manifest-v2-catalog` | 10 | 9 `-`, 1 `+` | **29 lines** |

`fable/r2-runtime-capabilities` (`f9045a0`) is the cleanest case in the whole
survey: **zero** branch-added lines absent from `main`. Its two decoders —
`loader_state.rs` (739 lines) and `instructions_sysvar.rs` (1,035 lines) — were
rebased to `01a004b` on `r2-caps-rebase-trial` and merged to `main` by
**`5a94c1d` "Merge the two R2 pull-profile runtime capabilities" (2026-08-21)**.
`git diff r2-caps-rebase-trial main -- <both decoders>` is empty.

`codex/r2-production-source-rwv8`'s 5 residual lines are 08-19 status prose that
`main` has since falsified — e.g. *"it remains model-only, unmerged, with the
default source registry empty"*, written before `5a94c1d` and before
`329d777` ("Pin the R2 pull identity in one const module and decode
PriceUpdateV2"). `research/source-profile-v1/{auth_v2,crossing_v1,spec_v2}.rs`
are all in `main`.

`codex/manifest-v2-catalog`'s one `+` commit is **`715648e` "Decide
evidence-only failure recovery economics"** — and its four substantive files are
**byte-identical to `main`**:

```
docs/implementation/FAILURE_PAYOUT_DECISION_V1.md  IDENTICAL
research/failure-payout-v1/src/lib.rs              IDENTICAL
research/failure-payout-v1/README.md               IDENTICAL
research/failure-payout-v1/Cargo.toml              IDENTICAL
```

Only the narrative co-edits (`CURRENT_TRUTH.md`, `docs/ECONOMICS.md`,
`docs/SWARM_ROADMAP_2026-08-19.md`) differ, and those are dated 08-19 claims
`main`'s 08-21 `CURRENT_TRUTH.md` supersedes.

> **DELETE** all three.

---

### 2.3 The research-integration / manifest-v2 family (5 branches) — DELETE

| Branch | Ahead | `git cherry` | Residual |
| --- | --- | --- | --- |
| `codex/research-integration-20260819` | 12 | 9 `-`, 3 `+` | 54 lines |
| `codex/manifest-v2-baseline` | 5 | 5 `-` | 42 lines |
| `codex/manifest-v2-diagnostic` | 1 | 1 `+` | 35 lines |
| `codex/final-seal-1326` | 1 | 1 `+` | 6 lines |
| `codex/glass-r1-evidence` | 1 | 1 `-` | **2 lines** |

The `+` commits here are all **evidence snapshots that a later reseal replaced**:

* `6b6e2b6` / `c740797` — both "Record checked schema v2 baseline manifest",
  both touching only `MANIFEST.baseline.json`. `main` is already
  `dragons-clutch/baseline-manifest/v2` and **144,497 bytes** against the
  branches' 139,601 / 139,835. Seven seal cycles have run since. A generated
  artifact from 08-19 cannot supersede the 08-22 one.
* `3fc3e65` "docs: reconcile integrated research boundaries" — pure handoff-doc
  co-edit. `main`'s `CURRENT_TRUTH.md` is status-dated **2026-08-21**; the
  branch's is 08-19 and still says *"the sealed default ELF remains on `0x79`"*
  and *"no registry entry or runtime codec/parser exists"* — both now false
  (`source_v2/`, `source_archive_v2.rs`, `5a94c1d`, `329d777`).
* `d1d3883` "Record resealed baseline diagnostic state" — the file
  `docs/implementation/BASELINE_MANIFEST_DIAGNOSTIC_2026-08-19.md` exists in
  `main` and is **strictly richer**. The branch stops at the `bd20711b…` ELF and
  an "Exact next gate sequence" TODO; `main`'s version already records the
  `7931e23` reseal to `af6bb79c…`, the 94/94 result, the Persvati attestation
  (40/40 portable gates, 528 files checked twice), and a "Re-emission claim"
  section closing at 98/98. `main` was later modified again by `ecfd552`.

`codex/manifest-v2-baseline`'s 14 residual lines in `scripts/baseline_manifest.py`
are 08-19 gate notes — *"37 deterministic liveness-profile tests"*, *"the sealed
1,228,192-byte default ELF and 23 committed"*. `main` has the newer counts, the
newer sealed ELF, and a **`STRICT_DOC_CRATES`** mechanism plus a
`cargo_doc.batch_policy_identity` gate that the branch **lacks entirely**. The
branch is behind, not ahead.

`codex/glass-r1-evidence`: 2 residual lines across two generated evidence blobs.

> **DELETE** all five.

---

### 2.4 The Verus / formal-refinement pair (2 branches) — DELETE

| Branch | Ahead | `git cherry` | Residual |
| --- | --- | --- | --- |
| `codex/verus-batch-proof-v1` | 3 | 3 `-` | 52 lines |
| `codex/formal-refinement-sol-…kRPVQc` | 4 | 4 `-` | 18 lines |

Decisive, from `verus/batch/BATCH_REFINEMENT.json`:

| | branch | `main` |
| --- | --- | --- |
| `result` | `PASS: … 20 verified, 0 errors` | `PASS: … **28 verified**, 0 errors` |
| `checked_date` | 2026-08-19 | **2026-08-22** |
| `statement_digest_sha256` | `835578e7…` | `3d6d27c5…` |

`main`'s proof set is a strict advance. The 3 residual lines in `batch.rs` are
`vstd` lemma imports (`lemma_div_pos_is_pos`, `lemma_multiply_divide_le`, …)
that `main` reorganized. The residual `run_batch_proofs.sh` lines are stale
`*_SHA256_PIN` digests bound to the retired 20-lemma statement.

*(Substrate note, per the house rule: this is Verus over a scalar mathematical
shadow, not Lean-authored AIR, and it was already that way on `main` before this
triage. Nothing here changes the substrate; flagging only so the disposition is
not read as an endorsement of the Rust-side proof shape.)*

> **DELETE** both.

---

### 2.5 The one-commit repair branches (3 branches) — DELETE

| Branch | Ahead | `git cherry` | Residual |
| --- | --- | --- | --- |
| `codex/solana-lock-v1` | 1 | 1 `-` | **0 lines** |
| `codex/cost-abi-v2-repair` | 1 | 1 `-` | 30 lines |
| `codex/r1-truth-v7` | 1 | 1 `-` | 83 lines |

`codex/solana-lock-v1` (`df3f8a4` "Repair Solana reference lock graph"): **zero**
residual. Fully absorbed.

`codex/cost-abi-v2-repair` is a textbook stale ABI snapshot. Side by side:

| field | branch (08-19) | `main` (08-22) |
| --- | --- | --- |
| `snapshot_date` | 2026-08-19 | **2026-08-22** |
| `abi_landed.commit_subject` | "feat(sbf): route resumable occupation resolution" | "Retire VirtualMergeCredit: deliver, then burn, then pay" |
| `epoch.bytes` | 328 | **329** |
| `candidate_record.bytes` / `schema_version` | 305 / v2 | **337 / v3** |
| `clear_work.bytes` | 48,750 | **50,054** (adds `CLEAR_WORK_INTERNER_BYTES`) |
| `max_intent_bytes` | 310 | **402** (`InitSourceSpecV2`'s 368-byte body) |

Adopting any of this would *regress* the ABI audit.

`codex/r1-truth-v7` (`916ff47` "Seal final R1 truth boundary"): its 83 residual
lines are the 08-19 truth paragraph — sealed ELF `a5725a3d…` at 1,228,192 bytes,
runtime source `7e8f6b1`, *"The checked-in `MANIFEST.baseline.json` remains a
historical schema-v1 manifest."* `main` is at the TerminalClosure seal, schema
v2, status date 08-21. Every claim on this branch is a retired one.

> **DELETE** all three.

---

## 3. `r2-caps-rebase-trial` — the runbook's seed — **KEEP**

**State:** `01a004b`, **0 ahead of `main`**, 198 behind. It is *merged*, which is
why it never appeared in `git branch --no-merged main`.

**Verification of `R2_PHASE0_RUNBOOK.md`'s claims, clause by clause:**

| Runbook claim | Verdict |
| --- | --- |
| §1.1 "Tip `01a004be…`, parented on main `e5b0503d…`" | ✅ holds |
| §1.2 "`loader_state.rs` 739 lines, `instructions_sysvar.rs` 1,035, `lib.rs` +4, at the canonical path" | ✅ holds — and `git diff r2-caps-rebase-trial main` over both decoders is **empty** |
| §1.4 floor test "expect 42 passed, 0 failed" | ✅ holds — `main` has 18 `#[test]` in `loader_state.rs` + 24 in `instructions_sysvar.rs` = **42** |
| §1.4 "`git branch r2-phase0 r2-caps-rebase-trial` / `git rebase main r2-phase0`" | ⚠️ **now a no-op.** The content is already in `main`, so this yields a branch identical to `main`. |
| §1.5 "Branch work forces no reseal; only the merge does" / §7 "merge rides E3's reseal" | ⚠️ **already executed.** `5a94c1d` (2026-08-21 07:30) merged it: *"The trial rebase measured clean against recent main; this is that merge."* |
| §1.4 "Leave `r2-caps-rebase-trial` in place as the report's cited artifact" | ✅ **the reason to KEEP** |

**Disposition: KEEP.** Three live docs pin its tip by hash —
`REPORT_r2-cutover-and-registry-flip_2026-08-20.md` (§3 line 37, §E4 line 512,
line 580), `DECISION_PACKET_2026-08-20.md` (line 31), and
`NEXT_WAVE_ROADMAP_2026-08-20.md` (line 25), plus `GOAL.md` lines 214 and 373.
Deleting the ref would strand a cited artifact.

**Its future, and the doc drift worth ember's eyes:** the runbook was written
2026-08-20 assuming the merge was still pending behind E3. The merge landed
2026-08-21 via `5a94c1d`, and `329d777` has since built on it. So the branch has
retired from *seed* to *provenance marker*. Anyone opening the runbook today
should skip §1.4's seeding recipe and start a Phase-0 branch from `main` — the
42/42 floor test runs unchanged against `main`. Worth a one-line status head on
`R2_PHASE0_RUNBOOK.md` §1.4 saying so, and updating `GOAL.md:373`'s
"r2-caps-rebase-trial seeds Phase 0" to past tense.

---

## 4. Merged leftover branches (11) — DELETE

Fully contained in `main`; they exist only as stale worktree anchors.

| Branch | Tip | Note |
| --- | --- | --- |
| `codex/bringup-harness-repair-6743b9d` | `6743b9d` | |
| `codex/bringup-split-79` | `45eac1a` | worktree is **dirty** — see §5 |
| `codex/r3-direct-v3-successor` | `733a6c8` | the line that superseded §2.1 |
| `codex/research-convergence-final` | `8e7f827` | |
| `fable/deploy-economics-report` | `1b879d8` | |
| `fable/sha-syscall` | `6c25df4` | |
| `fee-plumbing-boundary` | `0b3561a` | no worktree |
| `worktree-agent-a139c165def6fcef2` | `a310df2` | no worktree |
| `worktree-agent-a8304fb312fd4cc66` | `86d1c72` | no worktree |
| `worktree-agent-a939cba1002e1f51b` | `5c95b6c` | no worktree |
| `worktree-agent-aa3453d7d8ca64298` | `49cf8ad` | no worktree |

---

## 5. Uncommitted work in worktrees — all three superseded

`git stash list` is **empty**. Three worktrees carry tracked modifications:

**(a) `/Users/ember/dev/dragons-clutch-bringup-split-79`** — 6 modified files,
including a 48-line uncommitted delta to `programs/clutch-sbf/harness/src/main.rs`
adding a `--default-source-refusal` mode split. **It landed.** `main` moved
`harness/src/main.rs` (now an 11-line shim) into `harness/src/lib.rs`, where the
same work lives:

```
harness/src/lib.rs:11327  "usage: clutch-sbf-harness <out-dir> [--committed|--general-clearing|--default-source-refusal]"
harness/src/lib.rs:11338  let default_source_refusal = mode.as_deref() == Some("--default-source-refusal");
harness/src/lib.rs:11448  if !default_source_refusal {
harness/src/lib.rs:5064   fn expect_default_source_refusals(cases: &mut [Case])
```

The `simulate.py` guard ("Reject plans that mix the default refusal and
mock-success claims") and the `baseline_manifest.py` `refuse endow` /
`accept endow` patterns are likewise in `main`
(`scripts/baseline_manifest.py:1278-1280`). `run_bringup.sh`: **0** unique lines.

**(b) `/private/tmp/dragons-clutch-direct-e0ac46e`** (detached `e0ac46e`, itself
in `main`) — 7 modified files. Residual after the line test: **1 line** in
`direct_selection.rs`, **1 line** in `direct_window_v1.rs`, **0** in
`svm-tests/Cargo.toml`. The one interesting untracked file,
`svm-tests/tests/direct_selection_v2.rs` (900 lines), is an earlier draft of
`main`'s **tracked** 917-line file; they differ by 16 lines, opening with a
docstring the successor edited: `"…and its measured STOP."` → `"…and its
measured cost."` The other 296 untracked entries are `target-sbf/` build output.

**(c) `/Users/ember/jobs/dc-idcheck-d2`** (detached `6e4702a`, in `main`) — no
tracked modifications; 242 untracked, all `t/` build output except
`o/clutch_sbf-keypair.json`, a locally-generated program keypair in a scratch
output dir. Ephemeral; note it only so its deletion is a conscious act.

> Nothing to rescue from any of the three.

---

## 6. Worktree registrations — 34 satellites, ~65 GB

**All 34 are currently on disk.** `git worktree prune` would remove **nothing**
today. The 18 under `/private/tmp` become prunable only after macOS reaps them —
and reaping frees the disk while *leaving* the registration and the branch ref
behind, which is how this backlog formed. Removing them explicitly now is
strictly better than waiting.

### Class A — holds a DELETE branch (20)

Must be `worktree remove`d before `git branch -D` will succeed.

| Path | Branch | Size | Dirty |
| --- | --- | --- | --- |
| `/private/tmp/claude-501/…/65da8d1f-…/scratchpad/f1f2-report` | `fable/deploy-economics-report` | 26 M | clean |
| `/private/tmp/claude-501/…/65da8d1f-…/scratchpad/r2-caps` | `fable/r2-runtime-capabilities` | 418 M | clean |
| `/private/tmp/claude-501/…/65da8d1f-…/scratchpad/sha-opt` | `fable/sha-syscall` | **6.3 G** | clean |
| `/private/tmp/dragons-clutch-bringup-repair.YFQpun` | `codex/bringup-harness-repair-6743b9d` | **4.6 G** | clean |
| `/private/tmp/dragons-clutch-cost-abi-v2-repair` | `codex/cost-abi-v2-repair` | 13 M | clean |
| `/private/tmp/dragons-clutch-final-seal-1326` | `codex/final-seal-1326` | **7.0 G** | clean |
| `/private/tmp/dragons-clutch-formal-refinement.kRPVQc` | `codex/formal-refinement-sol-…` | 36 M | clean |
| `/private/tmp/dragons-clutch-glass-r1` | `codex/glass-r1-evidence` | 12 M | clean |
| `/private/tmp/dragons-clutch-manifest-v2-316c` | `codex/manifest-v2-baseline` | 1.2 G | clean |
| `/private/tmp/dragons-clutch-manifest-v2-catalog` | `codex/manifest-v2-catalog` | 90 M | clean |
| `/private/tmp/dragons-clutch-manifest-v2-diagnostic` | `codex/manifest-v2-diagnostic` | 14 M | clean |
| `/private/tmp/dragons-clutch-research-convergence` | `codex/research-convergence-final` | **7.0 G** | clean |
| `/private/tmp/dragons-clutch-research-integration` | `codex/research-integration-20260819` | **6.8 G** | clean |
| `/private/tmp/dragons-clutch-verus-batch.0QYmIT` | `codex/verus-batch-proof-v1` | 36 M | clean |
| `/Users/ember/dev/dragons-clutch-bringup-split-79` | `codex/bringup-split-79` | 267 M | **6 tracked** |
| `/Users/ember/dev/dragons-clutch-lockfix` | `codex/solana-lock-v1` | 169 M | clean |
| `/Users/ember/dev/dragons-clutch-r1-truth-v7` | `codex/r1-truth-v7` | 12 M | clean |
| `/Users/ember/dev/dragons-clutch-r2-source-rwv8` | `codex/r2-production-source-rwv8` | 93 M | clean |
| `/Users/ember/jobs/dragons-clutch-r3-direct-successor.honest` | `codex/r3-direct-v3-successor` | **10 G** | clean |
| `/Users/ember/jobs/dragons-clutch-r3-direct.BDnrsh` | `codex/r3-direct-v3` | 1.3 G | clean |

### Class B — detached HEAD already in `main` (11)

No branch ref at risk; removal is pure disk reclaim.

| Path | HEAD | Size |
| --- | --- | --- |
| `/private/tmp/dragons-clutch-bringup-wt.DI07jm` | `cbbc2a0` | 158 M |
| `/private/tmp/dragons-clutch-direct-e0ac46e` | `e0ac46e` | 3.1 G — **dirty, §5(b)** |
| `/Users/ember/jobs/dc-idcheck-d2` | `6e4702a` | 60 M — 242 untracked, §5(c) |
| `/Users/ember/jobs/dragons-clutch-bank-fixture-161f530.9X9ZQY` | `161f530` | 11 M |
| `/Users/ember/jobs/dragons-clutch-r1-2d530d2-stack-audit-worktree` | `2d530d2` | 15 M |
| `/Users/ember/jobs/dragons-clutch-r1-6c25df4-stack-audit-worktree` | `d8c5034` | 17 M |
| `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-worktree.wacx0l` | `7e8f6b1` | 11 M |
| `/Users/ember/jobs/dragons-clutch-r1-83e124d-stack-audit-worktree` | `83e124d` | 12 M |
| `/Users/ember/jobs/dragons-clutch-r1-853fecb-reseal-worktree` | `853fecb` | 22 M |
| `/Users/ember/jobs/dragons-clutch-r1-c55f471-xpath-worktree` | `c55f471` | 37 M |
| `/Users/ember/jobs/dragons-clutch-r1-e58aef4-reseal-worktree` | `e58aef4` | 19 M |

### Class C — detached HEAD *not* in `main` (3)

Each is 1 commit ahead. All three verified superseded before listing them here.

| Path | HEAD | Verdict |
| --- | --- | --- |
| `/private/tmp/dragons-clutch-r4-failure.osf5ct` (63 M) | `17c7d5d` "Decide evidence-only failure recovery economics" | `git cherry` `+`, **but** all four substantive files byte-identical to `main` (same as `715648e`, §2.2). Only 08-19 narrative differs. |
| `/private/tmp/dragons-clutch-selection-v3.XzytSZ` (197 M) | `74cf7d1` "add direct selection v3 authority codecs" | `git cherry` `-` — equivalent in `main`. |
| `/Users/ember/dev/dragons-clutch-terminal-v2` (45 M) | `9dd03f1` "Add terminal lifecycle V2 research model" | `git cherry` `-` — and `research/terminal-lifecycle-v2/` + `docs/implementation/TERMINAL_LIFECYCLE_V2.md` are both in `main`. |

**Disk by root:** `/private/tmp` ≈ 37 G · `/Users/ember/jobs` ≈ 28 G ·
`/Users/ember/dev` satellites ≈ 587 M.

---

## 7. Cleanup script

Not run by this survey. Read §3 before running: **`r2-caps-rebase-trial` is
excluded on purpose** and must stay.

Order matters — `git branch -D` fails while a worktree holds the branch, so
worktrees come first.

```sh
#!/bin/sh
# Branch + worktree cleanup, dispositioned 2026-08-22.
# KEEP (never listed below): main, r2-caps-rebase-trial
set -eu
cd /Users/ember/dev/dragons-clutch

# --- Step 1: remove worktrees holding DELETE branches (Class A, 20) ---------
for w in \
  "/private/tmp/claude-501/-Users-ember-dev-dragons-clutch/65da8d1f-7994-4d66-b7d4-45c984839d9f/scratchpad/f1f2-report" \
  "/private/tmp/claude-501/-Users-ember-dev-dragons-clutch/65da8d1f-7994-4d66-b7d4-45c984839d9f/scratchpad/r2-caps" \
  "/private/tmp/claude-501/-Users-ember-dev-dragons-clutch/65da8d1f-7994-4d66-b7d4-45c984839d9f/scratchpad/sha-opt" \
  "/private/tmp/dragons-clutch-bringup-repair.YFQpun" \
  "/private/tmp/dragons-clutch-cost-abi-v2-repair" \
  "/private/tmp/dragons-clutch-final-seal-1326" \
  "/private/tmp/dragons-clutch-formal-refinement.kRPVQc" \
  "/private/tmp/dragons-clutch-glass-r1" \
  "/private/tmp/dragons-clutch-manifest-v2-316c" \
  "/private/tmp/dragons-clutch-manifest-v2-catalog" \
  "/private/tmp/dragons-clutch-manifest-v2-diagnostic" \
  "/private/tmp/dragons-clutch-research-convergence" \
  "/private/tmp/dragons-clutch-research-integration" \
  "/private/tmp/dragons-clutch-verus-batch.0QYmIT" \
  "/Users/ember/dev/dragons-clutch-lockfix" \
  "/Users/ember/dev/dragons-clutch-r1-truth-v7" \
  "/Users/ember/dev/dragons-clutch-r2-source-rwv8" \
  "/Users/ember/jobs/dragons-clutch-r3-direct-successor.honest" \
  "/Users/ember/jobs/dragons-clutch-r3-direct.BDnrsh" \
; do git worktree remove "$w" || echo "SKIP (inspect by hand): $w"; done

# Dirty; §5(a) proves the uncommitted delta is already in harness/src/lib.rs.
git worktree remove --force "/Users/ember/dev/dragons-clutch-bringup-split-79"

# --- Step 2: remove detached worktrees already in main (Class B, 11) --------
for w in \
  "/private/tmp/dragons-clutch-bringup-wt.DI07jm" \
  "/Users/ember/jobs/dragons-clutch-bank-fixture-161f530.9X9ZQY" \
  "/Users/ember/jobs/dragons-clutch-r1-2d530d2-stack-audit-worktree" \
  "/Users/ember/jobs/dragons-clutch-r1-6c25df4-stack-audit-worktree" \
  "/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-worktree.wacx0l" \
  "/Users/ember/jobs/dragons-clutch-r1-83e124d-stack-audit-worktree" \
  "/Users/ember/jobs/dragons-clutch-r1-853fecb-reseal-worktree" \
  "/Users/ember/jobs/dragons-clutch-r1-c55f471-xpath-worktree" \
  "/Users/ember/jobs/dragons-clutch-r1-e58aef4-reseal-worktree" \
; do git worktree remove "$w" || echo "SKIP (inspect by hand): $w"; done

# Dirty / untracked-heavy; §5(b), §5(c) show nothing is lost.
git worktree remove --force "/private/tmp/dragons-clutch-direct-e0ac46e"
git worktree remove --force "/Users/ember/jobs/dc-idcheck-d2"

# --- Step 3: remove detached worktrees not in main (Class C, 3) -------------
# Each is 1 commit ahead; §6 Class C shows all three superseded.
for w in \
  "/private/tmp/dragons-clutch-r4-failure.osf5ct" \
  "/private/tmp/dragons-clutch-selection-v3.XzytSZ" \
  "/Users/ember/dev/dragons-clutch-terminal-v2" \
; do git worktree remove "$w" || echo "SKIP (inspect by hand): $w"; done

git worktree prune -v
git worktree list      # expect exactly one line: /Users/ember/dev/dragons-clutch

# --- Step 4: delete branches -----------------------------------------------
# -D, not -d: unmerged tips are refused by -d by design. §2 is the evidence
# that -D is safe here. Each deletion prints the SHA; `git branch <name> <sha>`
# restores it until gc, and the reflog holds it for the usual 90 days.

# 4a. The 16 unmerged, all superseded (§2):
git branch -D \
  backup/r3-direct-v3-successor-prerebase \
  codex/r3-direct-v3 \
  fable/v3-settle-port \
  fable/r2-runtime-capabilities \
  codex/r2-production-source-rwv8 \
  codex/manifest-v2-catalog \
  codex/research-integration-20260819 \
  codex/manifest-v2-baseline \
  codex/manifest-v2-diagnostic \
  codex/final-seal-1326 \
  codex/glass-r1-evidence \
  codex/verus-batch-proof-v1 \
  "codex/formal-refinement-sol-dragons-clutch-formal-refinement.kRPVQc" \
  codex/solana-lock-v1 \
  codex/cost-abi-v2-repair \
  codex/r1-truth-v7

# 4b. The 11 merged leftovers (§4):
git branch -d \
  codex/bringup-harness-repair-6743b9d \
  codex/bringup-split-79 \
  codex/r3-direct-v3-successor \
  codex/research-convergence-final \
  fable/deploy-economics-report \
  fable/sha-syscall \
  fee-plumbing-boundary \
  worktree-agent-a139c165def6fcef2 \
  worktree-agent-a8304fb312fd4cc66 \
  worktree-agent-a939cba1002e1f51b \
  worktree-agent-aa3453d7d8ca64298

# 4c. NOT DELETED, on purpose — see §3:
#   main
#   r2-caps-rebase-trial   <- cited by hash in R2_PHASE0_RUNBOOK.md §1.1,
#                             REPORT_r2-cutover-…_2026-08-20.md §3/§E4,
#                             DECISION_PACKET_2026-08-20.md:31,
#                             NEXT_WAVE_ROADMAP_2026-08-20.md:25,
#                             GOAL.md:214,373

# --- Step 5: verify --------------------------------------------------------
git branch --format='%(refname:short)'    # expect: main, r2-caps-rebase-trial
git worktree list
```

**Two cautions.**

1. `origin` carries only `main` — nothing being deleted was ever pushed. Deletion
   is local-only and reflog-recoverable, not remotely recoverable. §2's residual
   line counts are the standing evidence that nothing is being lost; keep this
   file until the deletion is done.
2. The main working tree at `/Users/ember/dev/dragons-clutch` has **12 modified
   files** (11 `svm-tests/tests/*.rs` plus
   `research/batch-policy-identity/src/revenue_policy_v1.rs`). That is live work
   and the script never touches it. Do not `git stash` around any of this.

---

## 8. Follow-ups worth ember's attention

1. **`R2_PHASE0_RUNBOOK.md` §1.4 has drifted.** Its seeding recipe
   (`git branch r2-phase0 r2-caps-rebase-trial && git rebase main r2-phase0`) is
   now a no-op — `5a94c1d` merged the content on 08-21 and `329d777` built on
   it. The 42/42 floor test is unaffected and runs against `main` as written.
   One status line on §1.4, and `GOAL.md:373`'s "r2-caps-rebase-trial seeds
   Phase 0" to past tense, closes it.
2. **~65 GB reclaimable**, ~37 GB of it in `/private/tmp` where macOS will
   eventually free the bytes but leave the registrations and branch refs — the
   exact mechanism that produced this backlog. Explicit removal beats waiting.
3. **Zero genuinely-unique work across 16 branches is itself the finding.** The
   08-19 swarm's output reached `main` through rebase-merges and successor
   lines with no measurable loss: 97–100 % line-level absorption on every
   branch, and two branches (`codex/solana-lock-v1`,
   `fable/r2-runtime-capabilities`) at exactly 0 lines outstanding.
