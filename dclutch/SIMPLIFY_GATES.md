# SIMPLIFY-GATES — the gate tools, before and after

Branch `simplify/gates` from `main` at `330bbfaba`. Domain: the instruments that
decide what is true — `tools/{ci,gauntlet/{run.sh,census,devnet-witness},
frameguard,emission-guard,genref,doc-commands,twins,lib,lane.sh}`.

## The shape

One entry point, `tools/gate`. One tier table (`tools/gates/tiers.py`) that
`--list`, the dispatcher and the README all read. One verdict vocabulary
(0 pass / 1 defect / 2 nothing proven / 64 usage), one clean-revision export
(`gates/common.py: archived`, `git archive | tar -xm`), one subprocess wrapper.
Every tier runs every row and reports every row; a missing prerequisite is 2,
never a pass; `--require` makes it 1. The verdict prints each gate's measured
seconds, so cost is written where it runs.

Instruments a caller drives directly: `census`, `emission`, `frames`,
`reference`, `witness`, `budgets`, `commands`, `twins`, `lane`, `archive`,
`selftest`. Tiers that are not instruments run a tool another directory owns
(`tools/seam-audit`, `tools/sbom`, `tools/release`, the program-test runners)
against a register kept in `tools/gates/`.

## The tool table: before → after

| before | lines | after | lines | note |
|---|---:|---|---:|---|
| `tools/ci/run.sh` (19 tiers, bash, ~60% prose) | 2,840 | `tools/gates/tiers.py` + `cli.py` + `common.py` | 913 + 126 + 267 | every tier transcribed; `--dry-run` prints each tier's commands |
| `tools/ci/README.md` | 417 | `tools/gates/README.md` | 107 | one paragraph per instrument: what it refuses |
| `tools/ci/never-run-tests.py`, `clippy-census.py`, `wire-vector-pins.py` | 236 + 214 + 130 | folded into `tiers.py` (root-targets, clippy) and `emission.py` (pins) | — | one 0/1/2 convention; the old pins/never-run tools had no 2 |
| `tools/ci/*.tsv`, `fmt-baseline.txt` | 245 | `tools/gates/*.tsv`, `fmt-baseline.txt` | 245 | moved, byte-identical |
| `tools/frameguard/{run.sh,frameguard.py,test-runner.sh,test_frameguard.py,README}` | 369 + 726 + 209 + 400 + 100 | `tools/gates/frames.py` + `tests/test_frames.py` | 559 + 261 | `EXPECTED_LINK_COUNT` pinned once, not in two files; one export instead of archive-then-worktree |
| `tools/frameguard/baseline.json` | 11,493 | `tools/gates/frames-baseline.json` | 11,493 | moved, byte-identical; `owed` accepts both paths in history |
| `tools/emission-guard/{emission_guard.py,README,pre-push,install-hooks.sh}` | 762 + 177 + 76 + 47 | `tools/gates/emission.py` + `tests/test_emission.py` | 474 + 75 | the pre-push hook never fired: this repository has no remote |
| `tools/emission-guard/COVERAGE.md` | 206 | `tools/gates/emission-coverage.md` | 200 | regenerated; tables identical to the old tool's output at HEAD, prose cut to one paragraph |
| `tools/doc-commands/{doc_commands.py,README,negative-control.sh}` | 546 + 124 + 168 | `tools/gates/commands.py` + `tests/test_commands.py` | 546 + 87 | logic verbatim; the control ported to unittest |
| `tools/genref/{generate.sh,test.sh}` | 298 + 327 | `tools/gates/reference.py` + `tests/test_reference.py` | 177 + 114 | `generate.mjs`, `substrate-control.mjs`, `render-site.mjs` stay (renderers; 38 banners name them) |
| `tools/genref/generate.sh` | — | 3-line shim → `tools/gate reference` | 5 | named by every generated banner and `tools/release/final-generated-convergence.py` |
| `tools/ci/run.sh` | — | 17-line shim → `tools/gate` with old tier names mapped | 17 | the public wrapper's workflows call it; delete after they call `tools/gate` |
| `tools/gauntlet/devnet-witness/corroborate.py` | 899 | `tools/gates/witness.py` | 899 | moved; `main(argv)` |
| `tools/gauntlet/run.sh --mode census` + stages tool/inventory/census | 834 | `tools/gates/census.py`; `run.sh` delegates (808) | 152 | the census is built, tested, inventoried and reported by one instrument; the campaign runner keeps elf/campaign/observe |
| `tools/gauntlet/board-staleness.sh` | 385 | deleted | — | no caller; a heuristic over a free-text log that decided nothing |
| `tools/lane.sh guard-script` | 98 | deleted | — | no caller |
| — | — | `tools/gates/budgets.py` | 99 | new: the CU register's shape rules ran nowhere before a campaign evaluated them |
| — | — | `tools/gates/twins.py`, `selftest.py`, `archive.py`, `lane.py` | 39 + 43 + 36 + 11 | the twin gate, the self-tests, the export helper and the lane wrapper, reachable from the one entry point |
| census crate: six file walks, six parses | 9,516 | `sources.rs`: one walk, one parse | 9,518 | inventory byte-identical (see controls); enumeration CPU 19.3s → 5.9s |

Domain total (excluding lockfiles): **36,505 → 31,937 lines**, of which
11,493 is the frame baseline in both. Code and prose excluding baselines and
data: **~24,000 → ~19,300**; prose retelling incidents: gone (git history and
`docs/evidence/` hold it).

## Deletions, with the control each one showed

| deleted | lines | control |
|---|---:|---|
| `tools/ci/run.sh` prose and per-tier `--commit` boilerplate (8 copies) | ~1,900 | `tools/gate --dry-run all --commit HEAD` prints every tier's command lines; read against the old script tier by tier |
| `tools/ci/never-run-tests.py`, `clippy-census.py`, `wire-vector-pins.py` | 580 | logic transcribed into `tiers.py`/`emission.py`; `emission --pins` on the 5 pinned fixtures matches |
| `tools/frameguard/run.sh` + `test-runner.sh` | 578 | `tests/test_frames.py` re-proves: clean capture names HEAD, diagnostic red, dirty capture refused, `--at` measures the named commit, worktree removed, bad rev is 2, `owed` follows the two-edge closure |
| `tools/emission-guard/pre-push`, `install-hooks.sh` | 123 | this repository has no remote (AGENTS.md); the hook never fired; `tools/gate emission` is on `cheap` |
| `tools/genref/generate.sh` loop + `test.sh` | 625 | `tests/test_reference.py` re-proves: dirty refusal and both escapes, fixpoint by pass 3, no-fixpoint refused, undeclared emitter refused, `--check --converge` measures the commit not the tree |
| `tools/doc-commands/negative-control.sh` | 168 | `tests/test_commands.py`: clean runbook, three defect classes, unprobed is 2, subcommand descent |
| `tools/gauntlet/board-staleness.sh` | 385 | `grep -rn board-staleness` finds only the board archive |
| `lane.sh guard-script` + its tests and README section | ~150 | `grep -rn guard-script` finds only the board archive; `tools/lane/test.sh` 44/44 |
| census: five per-scanner walks (`index_constants`, `index_admissions`, `bands::sweep`, `magics::sweep`, `preimages::sweep`) and the two inside `enumerate()` | ~120 | `inventory.json` at `330bbfaba` byte-identical before → after except the new `bands` key; 86/86 crate tests |
| genref's `lib.rs` regex for band bases; the two route-census generators' `generated_bands.rs` regex | ~75 | all three read `inventory.bands`; `routeCensus.ts` regenerated (its only diff is `main`'s staleness, below) |

## The duplicates the brief named

- **Two registers that disagreed**: `tools/genref/generate.mjs` regexed
  `crates/dclutch-refusal-registry/src/lib.rs` for `*_BASE` constants that moved
  to `generated_bands.rs` on 2026-09-02, so `docs/reference/refusals.md` shipped an
  **empty band-allocation table** while `generate-route-census.mjs` (web + SDK)
  regexed the right file. Now the census inventory carries `bands` and all three
  read it. The table has 26 rows again.
- **The census's three scanners** (six, counted): one `Sources` load.
- **The three schema readers**: `tools/lib/rust_schema.py` is the author and
  has three importers; `preflight.py`'s own `rust_str_const` and the two test
  oracles (`test_chaos.py`, `test_reconcile.py`) were left — the first is the
  release maker's file and states its independence as the point, the other two
  are test oracles of the reader. Named, not merged (see "left").

## Byte-identity controls (all at `330bbfaba`, this checkout)

| artifact | result |
|---|---|
| `inventory.json` old census → new census | identical except `bands` (26 rows) appended |
| `fmt` tier findings old → new | identical: the same 7 files |
| emission census old → new | identical counts (101 generated / 95 emitters / 78 guards) and the same fixpoint hazard |
| `emission-coverage.md` tables vs old `COVERAGE.md` tables | identical rows; only the prose differs |
| `docs/reference/**`, `lib/generated/routeCensus.ts`, `marketPhaseAdmissionV1.ts` | regenerated by `tools/gate reference --converge` (fixpoint by pass 3); the diff is `main` being one route behind its own reference (`claims/claims_conservation_v1::process`, decision 0023 confirmed, provenance lines moved) plus the band table filling in |
| `frames-baseline.json`, the four `.tsv`, `fmt-baseline.txt` | moved unchanged |
| seam findings, stale locks | the old tools in a throwaway worktree of `main` report the same (see the findings below) |

## The verbs, with cost

`tools/gate --list` is the table. Measured on 2026-09-04 with seven makers
building on the machine: `tools/gate cheap` = **6m22s wall** for selftest,
census, emission, budgets, fmt, locks, seam, commands, release; the verdict
now prints each gate's seconds, so the next run writes the split. Heavy tiers
carry the dates and figures the old runner recorded; none were run here (the
CPU is a shared lock, per the brief).

`cheap` = selftest census emission budgets fmt locks seam commands release.
`all` = cheap + reference twins clippy sbom sbfcontracts web abi guards frames
journey root-targets programs suites witness. `workspaces` stays outside `all`.

## Findings at `main`, not port defects (each controlled against the old tool)

- `emission`: `crates/dclutch-market/src/protocol_parameters/generated.rs` is
  raw-compared and rustfmt reflows it; one `lane.sh fmt` from red. The debt file
  is empty; the owning lane repairs the guard or the emitter.
- `fmt`: 7 files outside the baseline (claims-svm, economic-slice-kernel,
  series-v3-kernel, four under claims-sbf).
- `locks`: 13 workspace lockfiles do not resolve under `--locked --offline`.
- `seam`: 39 new findings against the baseline (DOMAIN_RAW_RESTATEMENT and
  siblings in resolution/provider operators, core_effect.rs, market.rs); the old
  tool at `main` reports 40 (the one difference is under the census crate this
  branch rewrote).
- `commands`: `dclutch-terminal --help` names none of its verbs in a fresh
  checkout (the launcher's `dist/` is unbuilt), and `dclutch` is unbuilt — 2,
  and 1 for the terminal.
- `reference`: `main` was not at its fixpoint (the diff above).
- `emission-coverage.md` at `main` was stale (100 → 101 generated files).

## The architect's map, and where this branch deviates

`SIMPLIFICATION_MAP.md` §1.5 targets one Rust `dclutch-gate`. This branch
delivers the same shape — one entry point, one tier table, every row runs and
reports, one export, one exit vocabulary — in Python, and stops there, for three
reasons: (1) the wrapper's cheap lane must run on a runner with no Rust
toolchain (`checks.yml` states that as its contract), and a gate that must be
built before it can gate breaks it; (2) the two instruments that read Rust are
already Rust (the census) or read an ELF with the standard library
(`sbf-frame-sizes.py`); (3) a Rust port of ~5k lines of Python/JS gate logic
with byte-identity controls is its own swarm. The entry point, the tier table
and the verdict are language-independent and a later Rust port adopts them as
they are. `seam-audit`, `sbom`, `doc-citations`, `cohort*` and the live
quarter of `release/` were left where they are and called by name.

§3.4's build-and-gate pass, in this tree's spellings:
2. `tools/gate frames --at <commit> --capture <file>` (twice) then
   `tools/gate frames accept --first A --second B --output tools/gates/frames-baseline.json`;
3. `tools/gate emission` then `tools/gate guards`;
4. `tools/gate reference --converge` from a detached worktree at HEAD, then
   `tools/gate reference --check --converge`;
5. `tools/gate census` (prints routes and refusal codes);
6. `tools/gauntlet/run.sh --mode full` (its census stages call `tools/gate census`);
7. `tools/gate web` / `tools/gate abi` / `tools/gate twins`.

## Left deliberately, and why

- `tools/lib/rust_schema.py`, `tools/twins/classification.mjs`: already the one
  author each; their importers spell their paths (`run.py:913` hashes the path
  into a manifest).
- `tools/genref/{generate.mjs,substrate-control.mjs,render-site.mjs}`: the
  renderer and the site builder, named by 38 generated banners and the wrapper's
  `pages.yml`.
- `tools/gauntlet/run.sh` full mode, the family runners, `check-witnesses.sh`:
  campaign runners (heavy tiers) — census stages now delegate to `tools/gate
  census`; the 18 hand-copied `ledger_lock` functions in family runners can move
  to `tools/gate census observe` (which holds the lock) one runner at a time.
- `preflight.py`'s second `&str` reader and the two test oracles (above).
- `witness.py`'s regex magic reader beside the census's syn one: the census
  does not export the `decode_action` tag table; adding it to the inventory is
  the fix, and a census-crate change of its own.
- The two shims (`tools/ci/run.sh`, `tools/genref/generate.sh`): each has live
  callers outside this tree; delete with the callers.
- `tools/gauntlet/{TIERS,DESIGN,CU_BUDGETS}.md`: campaign documentation, pointers
  updated only.
- AGENTS.md: only the four lines naming replaced commands changed.

## The commits

`f8494986b` the package; `c4d4294e6` the census merge; `aa44e325e` deletions,
shims, delegation, band-table author; then the README, the regenerated
reference, per-gate seconds, the entry point's help arm, and this file.
