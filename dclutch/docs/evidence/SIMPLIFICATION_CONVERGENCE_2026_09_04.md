# The simplification convergence — 2026-09-04

Lane CONVERGE. Eleven branches off `main@330bbfaba`, merged onto `main` in the
map's order (`docs/design/SIMPLIFICATION_MAP_2026_09_04.md` §3), one
regeneration, one build-and-gate pass. Every number below was measured in the
convergence worktree at the commit named beside it; the recipe for each count
is stated where the count is. The public cut is `tools/cut.sh` after every
merge that was green.

## 1. The merge order, with each merge's conflicts and their resolutions

The rule for every conflict: read both sides; a deletion a maker made stays
deleted; a file a main-lane also touched keeps both facts; a generated file is
regenerated, never hand-merged. Every merge ended with `cargo check --workspace
--offline` green before its commit.

| # | branch (head) | merge commit | cut | conflicts | resolution |
| --- | --- | --- | --- | --- | --- |
| 1 | `simplify/generations` (ed11ebbca) | fe2071c7a | ca97a0c27 | 1: `tools/frameguard/baseline.json` | main's 3616e5e5 capture rows and commit; the branch's link count (11, direct-aot gone). Redone once on the orchestrator's goal commit that landed during the check. |
| 2 | `simplify/trading` (11a962cf7) | d01d6ef06 | 79c51f743 | 1: `dealer/mod.rs` | trading's rename beside generations' deletion of `v3_lifecycle`. |
| 3 | `simplify/programs` (c1a689692) | 3bee5f3f1 | 6618974d8 | 20 | band table = programs' emission (superset of generations' band-10 retirement); `Cargo.toml` members = both removals + the accelerator; `Cargo.lock` regenerated; baseline = main's rows for the seven links programs keeps, count 7 (the accelerator unbaselined until the recapture); blocked.json / substrates.json / census TARGETS / SHIPPED_LINKS / the release role tables = programs' eight-program rows over trading's and generations' deletions with main's ladder entries; the deleted checkpoint chain's program-test driver and the dealer-checkpoint tier stay deleted at the paths programs had renamed them to. One rename-merge gap: the new accelerator's selector still imported selector 9's magic from the old module names — re-pathed, the magic dropped from the family match. |
| — | finding (c) | 6f2d1fc64 | 293c09f4c | — | the strict release gate's five frame diagnostics in `execute_authenticated_hot_v3` under `hot-cu-profile`: measured 3,840 plain / 3,904 profiled with `-Zemit-stack-sizes`; the projection keys boxed in an `#[inline(never)]` stage (plain 3,776, profiled 3,840, zero diagnostics), the profile's `{:?}` formatting moved out of the frame, the two loggers `#[inline(never)]`, the ranges logged one per call. |
| 4 | `simplify/crates` (2eed5ef31) | 73499583b | 0f382e900 | 97 | 56 modify/delete pairs where an earlier branch deleted a unit crates had moved: the deletion stands; twelve Rust files and four manifests crates had only re-pathed: main's side, then the crates tool's own sweep (`merge_crates.py`: rewrite_rust / rewrite_manifests / rewrite_path_strings per group — 83 Rust files, 16 manifests, 15 path strings); the root `Cargo.toml` = crates' members; every generated mirror and nested lock = crates', regenerated at the end; main's new `submit_candidate_clause_v3.rs` kept where directory-rename detection carried it. |
| 5 | `simplify/operators` (5cc4a261d) | bee9db34c | ad7fbf710 | 10 | trading's and operators' deletions stand; four files operators rewrote and crates re-pathed: operators' side + the sweep; five references to Trading's Dealer modules by their old generation names re-pathed to trading's concept names. |
| 6 | the parked merge (`simplify/operators-merge-wip` 32e9ed2cb), applied by its script | ef7e1c868 | a7fc68186 | — | thirteen of the nineteen absorbed; six stay their own crates (below). Reds 1–4 the WIP named, discharged. |
| 7 | `simplify/formal` (3e5b220c2) | d60d625dd | abb946956 | 13 | trading's three DealerScenario emitters stay deleted; the root module list regenerated (135); the lakefile and README formal's; three moved guard tests and two check scripts take formal's normalising body at crates' paths; the seam baseline minus the qedsvm rows formal moved; the General V2 codec's promoted import re-pathed; the capability-manifest guard rewired to the unified emitter (formal had not). |
| 8 | `simplify/gates` (8e9ebe280) | 73b99e911 | b0562e467 | 13 | the shim, the moved baseline (7 links carried), the tier table carrying run.sh's earlier edits (the accelerator runners, the deleted release suites), `EXPECTED_LINK_COUNT` 8 (programs' change lived in the deleted frameguard.py). |
| 9 | `simplify/drivers` (364ed9546) | 7080f404e | e287f651d | 7 | five deletions stand; `steps.tsv` = drivers' eleven-column table with main's route-witness fact; the successor's lock regenerated. |
| — | RECOVERY-4 (a), (b) | ad4131b14 | a26d5686b | — | the founding boundary derives its headroom (the DCLTGMF3 census had refused every founding at HEAD); `LocalMarketShapeV1.terminal_max_age_seconds` (default None, byte-identical); blocked.json's AdvanceRecovery entry names the field as landed. |
| 10 | `simplify/clients` (a2a883c61) | bc5ef31bd | 4f31eb3df | 68 | 62 deletions stand; package.json ×2 and the web capability surface clients' + the sweeps; the old genref driver's two edits carried into `tools/gates/reference.py`. |
| 11 | `simplify/docs` (4060a387e) | c9d01564b | — | 2 | AGENTS.md = docs' 201 lines + four gate facts main/gates landed; GOAL.md = the 104-line index; main's 161 appended lines verbatim in `docs/ledger/2026-09-04.md`, three rows. |
| 12 | `simplify/architect` (123faad77) | c5fdf74db | 6398fc298 | 0 | the map at the path the index cites. |

Main moved once under the convergence (540ada0d9, the orchestrator's goal
commit) and the first merge was redone on it; every later merge checked main's
head before committing.

## 2. The six operator crates that stay, and why

`crates/dclutch-svm-harness` and `programs/dclutch-claims-sbf/program-test/affine-batch`
are held on solana-program-test =4.2.1 (their manifests name the runtime panic
the bump buys), and that cluster cannot resolve `dclutch-operator`'s pins. With
all nineteen absorbed, `cargo metadata --offline` refused the address-table
interface at =3.2.0; with that dependency optional under a feature gating the
twenty-two-module reach-closure of its users, it refused direct-ticket's
solana-signature; with that optional too, compute-budget-interface's
solana-instruction-error. A crate those workspaces link cannot live inside
`dclutch-operator`, so the five planners they consume — market-open,
market-retirement, resolution-core, product-runtime, provider-transport — stay
out, and so does versioned-message, which provider-transport reaches under its
own feature and which inside the operator would be a cycle. The script
(`tools/lane/merge-operator-crates.py`) states this at its absorb list.

## 3. What the merge could not see, carried by hand

- trading's Dealer renames into the new accelerator's selector and into the
  operators branch's new carrying variants;
- formal's promotion import at the old crate root; the capability-manifest
  guard formal had not rewired; `EmitMarketCoreRust`'s doc naming a crate that
  no longer exists;
- clients' package.json predating formal's emitter unification (five SDK
  scripts rewired to the one emitter per record, `ts` selected);
- the tier table transcribed from run.sh at the base: the accelerator's two
  runners, the deleted release suites, the frame gate's link count;
- the stale pins three guards carried (BAND_COUNT 25, a raw line count of 95,
  a compare path at the crate root) and the seventeen clippy sites in two
  absorbed market constituents clippy had never reached;
- three SDK generators reading what other branches deleted or moved
  (general-v5 → the Lean emission's V2 facts; dealer-v3 → the concept names, no
  selector 9; state-machines → no Dealer scenario machines);
- the browser side of two deleted units, finished: selector 9's profile
  mirror, fixture, vector test and the web tier's cargo-test step; the
  product-runtime-v2 admission generator's read of the deleted program, the SDK
  module's instruction half, the Studio's step 03 and the record-PDA helper
  only it used;
- the web's `@dclutch/sdk` resolves through a relative symlink: from a
  worktree's symlinked node_modules it reaches the LIVE tree's SDK. Every
  client verdict here was taken after the worktree's node_modules were rebuilt
  with `@dclutch/sdk` pointing at the converged package.

## 4. Restored

Nothing a maker deleted was restored. The one thing kept against a maker's
intent is structural, not a restoration: the six operator crates above.

## 5. The live tree's uncommitted work

The live tree carried another lane's uncommitted work through the convergence
(SERIES-5's two test files). Each fast-forward that would have overwritten a
generated lock restored that lock to HEAD (every one is regenerated below);
the two test files' hunks were re-applied with the crates branch's renamed
paths (`dclutch_capability_program_contract::` → `dclutch_market::capability_program::`,
`dclutch_release_set_contract::` → `dclutch_registry::release_set::`) and are
dirty in the live tree again, for that lane. The untracked
`registered_terminal_artifacts_v4.rs` moved, still untracked, from the
direct-codec crate's directory to `crates/dclutch-trading/src/` beside its
tracked siblings; its `use` paths are the old crate names.

## 5. The line counts, before → after

One recipe on both sides, over `git ls-files` at the named commit: programs
and crates count `.rs`; formal counts `.lean`; tools counts every tracked file
minus `Cargo.lock`; apps and packages count `.ts/.tsx/.mjs/.js`; docs counts
`.md`. The map's §0 figures (308k / 534k / 73k / 352k / 106k / 76k / 115k) are
the same tree measured by hand a few hours earlier; the recipe is stated so the
two columns below are one measurement.

| domain | 330bbfaba | b41786d95 | delta |
| --- | ---: | ---: | ---: |
| programs (`.rs`) | 305,806 | 274,619 | −31,187 |
| crates (`.rs`) | 529,894 | 511,828 | −18,066 |
| formal (`.lean`) | 72,146 | 66,725 | −5,421 |
| tools (tracked, no locks) | 386,896 | 348,024 | −38,872 |
| apps (`.ts/.tsx/.mjs/.js`) | 107,883 | 61,326 | −46,557 |
| packages (`.ts/.tsx/.mjs/.js`) | 75,727 | 74,058 | −1,669 |
| docs (`.md`) | 115,341 | 130,074 | +14,733 |
| every `Cargo.lock` | 272,509 | 246,594 | −25,915 |
| every tracked file | 1,993,491 | 1,826,062 | −167,429 |

docs grows because 31,782 lines of root and top-level narrative now live
verbatim under `docs/ledger/` (the index that replaced them is 104 lines), and
because the map and this document are new. The counts that are units, not
lines: programs 12 → 8; crates under `crates/` 94 → 33; root workspace members
108 → 45; lockfiles 71 → 61; Lean emitters 105 → 92, the lakefile 423 → 11
lines; `GOAL.md` 4,992 → 104; `AGENTS.md` 348 → 202; routes 164 → 148, refusal
codes 357 → 344, registered bands 26 → 22; the web's files 547 → 348, its
twins with the SDK 202 → 3. The crates line count moves only by the deletions
(a merge moves bytes, it does not remove them); the map's 0.5–0.6 M target is
the rewrites' — the successor as a caller of the operator above all — which
no maker started (their reports say why).

## 6. The ELF table

Both sets built with the release recipe — `cargo build-sbf --manifest-path
programs/<pkg>/Cargo.toml -- --locked` — on this host (cargo-build-sbf 4.0.0,
platform-tools v1.53, rustc 1.89.0), the pre-swarm set in a detached worktree at
`330bbfaba` (twelve links, 0 frame diagnostics), the converged set at
`018ea525f` (eight links, 0 frame diagnostics). No link is byte-identical: the
crates branch's renames alone would have moved `.strtab`, and every link's
`.text` also moved — the band macro, the split, the folds, the feature lines
and the crates' layout all reach every program through the shared crates. So
every link owes rows, and cohort-16 carries every redeploy (cohort-16 has not
deployed). The frame rows are the recapture in §7.

| link | base sha256 (330bbfaba) | converged sha256 (018ea525f) | bytes | disposition |
| --- | --- | --- | ---: | --- |
| `dclutch_claims_sbf.so` | 38633f4de349…3ffc22 | 38196b22d71d… | 1,449,416 → 1,421,800 | moved (the Core-effect route gone, the band macro, the crate layout); cohort-16 |
| `dclutch_core_sbf.so` | 64702fcf08cd… | 236e92ae9345… | 1,189,464 → 1,186,824 | moved (the band macro, the layout); cohort-16 |
| `dclutch_custody_sbf.so` | 176f8007b002… | fd3b123a938e… | 573,576 → 431,304 | moved (the Dealer reservation route gone); cohort-16 |
| `dclutch_registry_sbf.so` | 83c9b0e89b21… | 98a1b45308d0… | 239,816 → 237,704 | moved (the band macro, the layout); cohort-16 |
| `dclutch_rent_sbf.so` | 738d847981f6… | 2f145149ae57… | 143,736 → 143,152 | moved (the band macro, the layout); cohort-16 |
| `dclutch_resolution_proof_sbf.so` | 7b1aab8f527a… | f8bfca35ab8e… | 851,096 → 846,656 | moved (the alias collapse, the band macro, the layout); cohort-16 |
| `dclutch_trading_sbf.so` | 654aeb599e69… | 46757cedce83… | 2,335,920 → 2,179,824 | moved (the checkpoint chain and selector 9 gone, the split, the boxed stage); cohort-16 |
| `dclutch_accelerator_sbf.so` | — | a8cb8c8ab35f… | — → new | new link (the three accelerators folded); cohort-16 seals its eight-entry Dealer set and its Registry pin |
| `dclutch_dealer_accelerator_sbf.so` | e343889667c7… | — | deleted link | never on a chain |
| `dclutch_direct_aot_sbf.so` | 5ad10944924a… | — | deleted link | never shipped |
| `dclutch_general_accelerator_sbf.so` | d36ed8b4f838… | — | folded | its band and codes byte-identical inside the accelerator |
| `dclutch_product_runtime_v2_sbf.so` | 345cf85e4fc5… | — | deleted link | never shipped |
| `dclutch_series_shadow_sbf.so` | 548dae10fb82… | — | folded | never on a chain |

The full digests are in the convergence scratchpad's `sbf-base.sha256` and
`sbf-after.sha256`; the eight converged ELFs are the release recipe's output
and the cohort-16 candidate builds them again from the cut.

## 7. The gates, with their seconds

Measured in the convergence worktree; the tree is 9f5e0aa70 (the frames
baseline at 018ea525f, after which no Rust moved).

| gate | verdict | seconds | note |
| --- | --- | ---: | --- |
| `cargo check --workspace --offline` after each merge | green ×12 | 128 (cold) … 2–17 | 38 warnings, the pre-existing never-used items the makers named |
| `cargo metadata --locked --offline`, every workspace | green, 61 of 61 | 74 (the `locks` tier) | regenerated once |
| `lake build` | green | 51 cold, 3 incremental | the band-population theorem corrected first |
| `tools/gate emission` | PASS | 2 | 91 generated / 91 guarded / 71 guards, 0 drift; pins settled |
| `tools/gate guards` (every guard for real) | 62 of 71 ok in the one full run; the 9 reds repaired and each re-run green alone | 230 | §3 names them; the full tier was not re-run after |
| `tools/gate fmt` | PASS | 6 | 367 swept files formatted with the pinned rustfmt; 8 baseline lines dropped |
| `tools/gate seam` | PASS | 53 | 30 rows carried, 68 retired |
| `tools/gate census` (`--check-unique`) | PASS | 8 warm / 44 cold | 148 routes, 344 codes, 22 bands, 0 unclassified |
| `tools/gate budgets`, `selftest` | PASS | 0, 21 | |
| `tools/gate release` | red in the tier's last run (one fixture path), the suite green alone after | 12 | `test_successor_campaign_pack.py` |
| `tools/gate commands` | FAILED | 1 | 18 rows: `dclutch-terminal --help` names none of its verbs in a fresh checkout (its `dist/` unbuilt — the gates report found the same at main); the four rows naming the deleted runner were repaired |
| `tools/gate reference --check --converge` | fixpoint on the first pass | 54 | at b41786d95 |
| `tools/gate frames` | PASS | 318 | against the recaptured baseline (captures 212 s and 244 s, identical) |
| SDK: `tsc`, `eslint`, vitest | clean, clean, 892 passed / 20 skipped / 0 failed | 66 | 87 files |
| web: `tsc`, `eslint`, vitest | clean, clean, 1,037 passed / 29 skipped / 4 failed in 4 files | 301 | §8 names them |
| the gauntlet tiers | not run | — | §8 |

The cuts, one per green merge: ca97a0c27, 79c51f743, 6618974d8, 293c09f4c,
0f382e900, ad7fbf710, a7fc68186, abb946956, b0562e467, e287f651d, a26d5686b,
4f31eb3df, 6398fc298; the wrapper's workflow commit c21451344; the final cut
follows this document.

## 8. What is owed

- **The gauntlet tiers** (tier 1, the Dealer campaign, General hot, the claims
  real-ELF suites) were not run by this lane: the convergence's budget went to
  the merges, the regeneration and the instruments; each is a campaign of tens
  of minutes on a validator. `tools/gate suites` and `tools/gauntlet/run.sh`
  are the commands; every runner they name exists and every nested workspace
  resolves under `--locked`.
- **Four web tests stay red, each with its owner**: `capabilitySelectedGate.test.ts` and one `capabilityPhaseGate` expectation (the census publishes no selected gate behind the Direct crosscheck — `ROUTE_SELECTED_GATES_V1` is empty in the clients branch's census too; the gates and clients columns); `explorerCoverage`'s record survey reads `DCLTRIX1` off the SDK's protocol-constants table as a record while the explorer renders it as the Registry instruction it is (the instrument's classification is the clients column's); `tradeFlowRefusals`' wall wording against the SDK's copies of the modules it reads (clients). The rest of that paragraph:
- **The census publishes no selected gate** behind the Direct crosscheck
  (`ROUTE_SELECTED_GATES_V1` is empty in the clients branch's census too), so
  `capabilitySelectedGate.test.ts` and one `capabilityPhaseGate` expectation
  stay red — the gates and clients columns.
- **`sourceReadinessWasmParity.test.ts`** compiles the operator's parity binary
  inside its own 30-second timeout on a cold target; green on a warm one.
- **Twelve magics the SDK's protocol-constants table declares** and the
  explorer does not decode are exempted with the clients column named; two
  routes the census enumerates and the explorer does not name (the Claims
  conservation route, the Series arm) likewise.
- **The parked operator merge's six crates** (§2): a program-test cluster
  bump (solana-program-test =4.3.0-beta.2) that does not panic would let them
  join; measured not to be that day.
- **Rent into Core**, the Series flatten, the Dealer `release` merge, the
  successor as a caller of the operator, the wire-literal census, the 45-clause
  refusal split — each maker's report names its own; none started here.
