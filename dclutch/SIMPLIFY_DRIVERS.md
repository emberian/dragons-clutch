# SIMPLIFY-DRIVERS — the host drivers

Branch `simplify/drivers` off `main` (`330bbfaba`). This lane owns the host
drivers: the successor bootstrap, the release tool, the cohort runbook, the
lifecycle runners. Three commits: the runbook (`b56fa755b`), the release-tool
deletion (`d213873c1`), a one-line lock catch-up. Every deletion below carries
the census control the map's §2.1 asks for — the reverse-dependency read that
found zero, and the count that did not change — and one deletion was reverted
inside its own batch when the read found a keeper on the far end.

| | before | after |
| --- | ---: | ---: |
| cohort runbooks | 3 directories, 2,918 lines | 1 directory, 1,987 lines (39 are the two frozen fixtures) |
| stage scripts a cohort hand-writes | 33 | 0 — generated (32 for cohort-15, 0 absolute paths, 0 credentials) |
| `tools/release` (py + sh + mjs) | 32,121 lines | 14,002 |
| successor (`src/*.rs`) | 182,947 lines, 95 commands | 175,533 lines, 89 commands |
| `devnet-reconcile` (tool + suite) | 2,593 + 2,047 lines, 58 tests | 1,255 + 700, 23 tests, all green |
| controls | — | successor `cargo check --offline` exit 0; `tools/cohort/test.sh` 23/23; reconcile 23/23 |

---

## 1. ONE RUNBOOK — landed (`b56fa755b`)

**`tools/cohort14/` and `tools/cohort15/` are deleted.** Their two forked
checkers, two preflights and two READMEs are gone; their `steps.tsv` files are
kept as fixtures under `tools/cohort/frozen/`, and `check-steps.py
--prove-frozen` is the standing proof that the one table still reproduces both
byte for byte — the proof the prompt asked to keep as a fixture, not two
directories. That proof is what let two columns be added and six rows appended
in the same commit as the deletion: cohort-14's and cohort-15's six-column
views are provably unchanged.

### The 134 flags and four shapes are two columns

The COHORT15 evidence's owed list named the corpus: **33 hand-written stage
scripts, 82 absolute paths, 134 flags the rows did not carry, four structural
shapes.** `steps.tsv` grew `shape` and `args`:

| shape | what the hand scripts open-coded |
| --- | --- |
| `once` / `per-role` | a straight-line invocation, or the list once per role |
| `attempts` | the plan-then-sign loop, a fresh output path per attempt |
| `wait:capture` / `wait:settle` | the bounded chain-clock wait, the guard that EXITS rather than falls through |
| `journal` | rerun one durable action per pass; an expired Planned entry is preserved, never re-signed |
| `commit` / `-` | a source commit; a row whose args are not captured yet |

`args` carries every flag as a `{field}` against the manifest or a
`{market.field}` per market — never a literal — with a driver vocabulary
(`bootstrap` / `bootstrap-public` / `bootstrap-offline` / `solana` / `script` /
`simulator` / `sh`), loop prefixes (`@roles`, `@owned_roles`, `@participants`),
`?` (skip when the output exists), `*` (the looped act, optionally
`*[FILE:field,field]` naming what says it is done), and `{pubkey:keys/x.json}`
/ `{stage:key}`. The fourth shape, peer-chaining — a script grepping another's
log for `SETTLE_LANDED` — is the `blocks` column now: every emitted script
refuses at its first line until each blocker left a `GREEN` marker, written
only on a zero exit.

`generate-stage-scripts.py` emits the family from those columns: 32 scripts for
cohort-15, per-market rows fanned once per market, a market fact the manifest
lacks refused BY NAME with nothing left on disk, the value test (no absolute
path, no credential, or the directory is emptied) applied to every script.

### The rows the runbook did not carry

Six verbs cohort-15 ran by hand off-runbook (recovered from its job scripts and
`HOLD_STATE.md`) are rows since 16, each `replaces`-chained so the frozen proof
holds: `deploy-roles`, `prepare` (moved AHEAD of the ladder — a genesis cohort
installs the candidate directly), `fund-payer`, `administration` (replaces
`ladder`), `checked-execution-release`, `seal-general`. The since-17
`refund-scale-seated` / `escrow-seated` got the README headings a cohort-17
gate needs. The runbook also names the defect it inherited: the frozen
`admit-terminal` row's driver column said `terminal-sequence` while its command
said `sponsored-push --action admit-terminal`; the `args` column now says what
cohort-15 actually ran.

### What is deliberately left

- Six rows carry `-` args (`redeploy`, `ladder`, `accelerator-release`,
  `refound-general`, `route-witness`, `re-admit`): retired rows whose
  successors carry the args, or rows with no host-runnable command. The gate
  prints the count; it is the migration's ledger.
- CI wiring: `tools/ci/run.sh` is the GATE maker's column (§2.5) and has never
  named the runbook; wiring `tools/cohort/test.sh` in is that column's one line.
- `sim-config.json` (the admissions row's input) is built by a hand-written
  `build-sim-config.py` in the job dir; the tree's `build_config_from_probe.py`
  is its home, and it is producer-missing (§3).

---

## 2. ONE DRIVER — the dead commands are gone; the rewrite is cross-column

**Deleted** (`d213873c1`): `private_activity.rs` (5,207) and
`private_lifecycle.rs` (2,169) and their six commands
(`local-private-validator-{activity-stage-completion, activity-manifest,
finalized-activity-capture, lifecycle-session, lifecycle-receipt,
direct-payout-schedule}-v1`). Reverse-dependency read: reachable from `main.rs`
only; named under `tools/` by nothing but the deleted lifecycle runner; the load
simulator's simlife layer drives eleven other owned-loopback commands and none
of these. 95 commands → 89.

**Cut and RESTORED in the same batch**: `terminal_exterior_pyth.rs` (2,996)
and its two `local-private-validator-pyth-*` commands. The census found a
keeper on the far end — the load simulator's local resolution names
`local-private-validator-pyth-vaa-provision-v1` as its prerequisite
(`simlife_drivers.py:1265`, its README step 3) and no other local Pyth
provisioner exists. Deleting it would have compiled green and stranded a
keeper's resolution: the per-file-green red umbrella. It stays.

**The rewrite this lane did not start.** The successor is still a **second
interpreter** of the operator's wire — `market.rs` 18,597 lines, 414
hand-written magic literals across the crate — rebuilding frames the operator
already builds. The map (§4 item 4) makes it a *caller* of `dclutch-operator`,
and that instruction surface is the OPERATORS maker's deliverable. Rewriting
`market.rs` against an API that does not exist yet builds a mirror of it (the
global CLAUDE.md's "ground-truth-first" lesson). The seam to agree: one
`build_<instruction>() -> UnsignedInstruction` per route on the operator; the
successor keeps the shell (args, RPC, journals, the cluster-origin policy) and
asserts on nothing it did not derive; a wire literal in `tools/` becomes a
census red. The three-authorities split the prompt names (the founding's frame
counts, the escrow identity, the ladder) is that rewrite, not a standalone edit.

**Named for the coordinator's call, not deleted**: `capability_seal_close.rs`
(536) — zero namers, but the only path to close one stranded pre-cohort-8
ZeroBump seal; `source-abort-interruption-audit-v1` and
`devnet-direct-trade-session-produce-v1` — dispatch arms nothing names, inside
modules that are otherwise live (an arm removal each, low value).

**Not dead though a command grep says so** (kept): the Series campaigns
(PRODUCER-MISSING, waiting on D7 per GOAL), `infrastructure_succession`
(`campaign.rs` calls the module), `evidence_refresh` and the General
plan/lookup-table/execute trio (now named by the runbook's `retire` and
`openbatch-refounded` rows), every `local-private-validator-*` arm the load
simulator or the gauntlet tiers drive.

---

## 3. ONE RELEASE TOOL — landed (`d213873c1`)

**The schema-reader merge was already done.** `tools/lib/rust_schema.py` is the
one author of "read a Rust `&str` const"; `run.py`, `chaos.py` and
`reconcile.py` were its three readers. Two of the three are gone with the
runner; the reconciler's devnet arms need no Rust constant, so its copy of the
loader went too. Nothing left to merge.

**The lifecycle runner that cannot found a market is deleted in favour of the
tier that does.** `tools/release/private-validator-lifecycle/` (`run.py` 7,599,
`preflight.py` 1,617, `chaos.py`, `watchdog.py`, three suites),
`lifecycle-chaos/`, `private_validator_upgrade/`, `devnet-flight/` — 18,119
lines. The control: `COLD_MACHINE_2026_09_03.md` §6/§8 — `run.py` founds once
at `48ad76992` and dies at admissions on an expired routing-table premise; its
twenty-seed mode and chaos matrix have never run on any host; the gauntlet tier
founds, opens and completes (201 transactions). The in-place Loader-v3
rehearsal was superseded on 2026-09-01 by the full-redeploy grant (no partial
or incremental deploys). `reconcile.py`'s `owned-loopback-captured` arm consumed
only the runner's artifacts and went with it (2,593 → 1,255 lines); its
`captured` / `follow` arms — the ones a cohort job directory reconciles through
— are untouched and 23/23 green, so the prompt's preserve rule ("cohort-15's job
dir must still reconcile") holds. `run.py`'s "6k lines by stage" is subsumed:
the disciplined move was to delete the runner, not to reshape a file the tier
replaces.

**The seams this lane does not own, named for the convergence commit:**

- `tools/ci/run.sh:2446-2450` — five `py_suites` rows now name deleted files
  (GATE column).
- `README.md:143,178`, `docs/guides/reader.md:68`,
  `tools/doc-commands/README.md:30` teach `run.py --through participant`; the
  tier is the replacement (DOCS column; the doc-commands baseline moves with it).
- `tools/load-simulator/build_config_from_probe.py` read the runner's
  `participant-handoff.json`; it is producer-missing until it reads the tier's
  evidence. Its README step 1 says so now (edited here; the simulator is a
  keeper with no column of its own).

**The small scripts stayed.** `devnet-observe.sh`, `devnet-recycle.sh`,
`devnet-sponsored-keeper.py`, `stage-story-market-exchange.py` are each the
subject of a live guide or a `///` doc-comment in the successor gated by
`doc-citations`; they are not orphaned, and they go with their guide or not at
all.

---

## 4. Rules this lane kept

- No commit reached into another column; every cross-column dependency is
  named with its line numbers above and in the commit messages.
- Every deletion was preceded by the reverse-dependency read and followed by
  the control that could have refuted it; the one that would have failed
  (`terminal_exterior_pyth`) was reverted before the commit.
- Nothing was built and no lifecycle was run: `cargo check --offline` in a
  private target dir, the Python suites, the runbook's own red proofs.
- Two of the three commits are unsigned — 1Password refused the agent — which
  is the signal the global instructions say it should be.
