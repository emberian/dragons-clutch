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

### 4.1 The live tree's uncommitted work

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

The same seven domains under the map's own recipe — every tracked
`.rs/.lean/.ts/.tsx/.mjs/.py/.sh/.md` file under the domain, which is what the
map's §0 figures were — at 330bbfaba and at 1ef14c5a1:

| domain | 330bbfaba | 1ef14c5a1 | delta |
| --- | ---: | ---: | ---: |
| programs | 308,362 | 277,403 | −30,959 |
| crates | 534,000 | 515,896 | −18,104 |
| formal | 73,280 | 66,967 | −6,313 |
| tools | 352,059 | 315,551 | −36,508 |
| apps | 105,671 | 59,114 | −46,557 |
| packages | 75,958 | 74,287 | −1,671 |
| docs | 115,341 | 133,580 | +18,239 |
| the seven together | 1,579,029 | 1,445,994 | −133,035 (−8.4 %) |

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

The eight converged links' full digests: accelerator
`a8cb8c8ab35fecd70615ba7bc0451bac95d6e7d4d61547154194d9b092be8f82`, claims
`38196b22d71d2814452ee751cb9263ac97c75a8f7be90b695541f2e64438a7c2`, core
`236e92ae9345af56476509b3b89c9442fb7980c36d672aca788d4c3d1f298986`, custody
`fd3b123a938ec376bf612025b753133b3ec97fcc52110d7dd2f889d79afc1ded`, registry
`98a1b45308d050378769a6d56eb1913bfe434452788d6b318c5db93e5c076d98`, rent
`2f145149ae575ba2180a0fa8d118dd098597620e333597bd5509cb4afe4fbcfc`,
resolution-proof
`f8bfca35ab8efaebfbce148368a1bb98ae9a58e378abadd671e5a8754b08b38f`, trading
`46757cedce83691cfd3fd85b1f551ca8d7d8dc7dcaab008b1ac64fa888ed4a01`. The base
set's full digests lived in the convergence scratchpad, which the host's fault
of 2026-09-05 cleared with the rest of `/private/tmp`; the twelve-hex prefixes
above stand, and the set is rebuilt from 330bbfaba by the same recipe. The
eight converged ELFs are the release recipe's output and the cohort-16
candidate builds them again from the cut. hbox's build of the same tree
(Linux x86_64, the same cargo-build-sbf 4.0.0 / platform-tools v1.53 / rustc
1.89.0) links a Trading ELF of `2753e3bf08a5…`, not the laptop's
`46757cedce83…`: the host reaches the ELF, which is why the release builder is
one named artifact and these digests are this host's.

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
4f31eb3df, 6398fc298; the wrapper's workflow commit c21451344; then
05af32c39 and f7a834022 for the evidence and the ledger, and the wrapper's
170d913ff (the six `run.sh` mentions its prose kept name `tools/gate`). The
final cut follows CONVERGE-2's commits.

### 7.2 The gates again at 1ef14c5a1, and what each red was

Measured by CONVERGE-2 on 2026-09-05 in a clean detached worktree at
1ef14c5a1 (the tree after the evidence and the ledger landed; no Rust moved
between 9f5e0aa70 and it), warm target, one gate at a time.

| gate | verdict | seconds | what the red was, and what followed |
| --- | --- | ---: | --- |
| `tools/gate cheap` | 8 of 9 green | 72 | selftest 9, census 4, emission 0, budgets 0, fmt 3, locks 20, seam 28, release 4 (green inside the tier since 9f5e0aa70); `commands` FAILED in 0 — the `dclutch-terminal --help` row, as at main |
| `reference --check --converge` | fixpoint on the first pass | 40 | 1ef14c5a1 is the fixpoint |
| `sbom` | PASS | 17 | no drift after 67b13378a's lock line |
| `guards` | PASS | 323 | the full tier; the nine §3 repaired hold |
| `twins` | FAILED | 0 | "no test files found": the tier ran `lib/twinIdentity.test.ts` against `tools/twins/classification.mjs`, both deleted by the clients maker with the twin arrangement (dcaba4770, replacement none). The tier goes — 15c2f20b6 |
| `clippy` | FAILED | 112 | `dclutch-product` red outside the debt table: four `indexing_slicing` sites in the merged svm reader's bump pairs, and three more in its tests behind the lib errors. The record index is typed and the tests follow — c666a9976; the package green under the gate's flags; the tier again: PASS in 8 s at 5e7276b2b — 43 members: 29 clean, 4 red on their debt rows (claims-sbf, custody-sbf, trading, resolution-proof-sbf), 10 never reached behind those four. Reaching the tree took four never-reached packages in a chain (product → source → the resolution operator → claims: c666a9976, b2d6660a0, 67750c22a, 5e7276b2b), and the ratchet's own rule deleted three debt rows for packages it cannot reach (chain-bundle-builder, operator, trading-sbf) — a measured row is written when each is reached |
| `web` | FAILED | 40 | the web vitest: 3 failed in 4 files, 1,028 passed, 29 skipped (`capabilitySelectedGate` at load, one `capabilityPhaseGate` expectation, `explorerCoverage`'s record survey, `tradeFlowRefusals`' wall wording — the two explorer route rows 9f5e0aa70 repaired are green); the SDK vitest 859 passed / 20 skipped / 0 failed; the liveness check 2 of 2 |
| `abi` | PASS | 117 | |
| `journey` | FAILED | 184 | two workspaces. The journey campaign's test target: three 9-argument calls made with 8 and `ProviderExecuteSnapshotV3` without `recovery_ladder` — the ladder's own commits (6a3079454, 61706bc9a, 8875255a5; RECOVERY-4, on main after the base), the journey's `#[path]` mirror of the successor untouched since. `tools/fractional-exterior`: the one regeneration resolved `solana-keypair` 3.1.2 and `solana-signature` 3.5.2 beside 2.1.0 where the base's stale lock had neither, and five8's `DecodeError` has no `std::error::Error` under the features that graph enables — not a feature: the workspace pins `solana-sdk =2.1.0` beside the operator's 3.0 line, the two lines share `five8` 1.0.0, and the lock binds it to the 2.1 line's `five8_core` 0.1.2 (no `Error` impl) while `solana-keypair` 3.1.2 needs 1.0.0's; the resolver refuses to rebind it under the 2.1 pin. At the base its lock did not resolve under `--locked` at all (the SBOM census of 2026-09-04 lists it), so no gate has compiled it since. Fractional-exterior on the 3.0 line is the fix — the drivers column |
| `root-targets` | FAILED | 0 | three orphaned rows — trading's `dealer_scenario_profile_vector` and `dealer_v3_composer`, the operator's `series_projected_outer_packet_v2`, all deleted units — and two unwired ladder tests (`funded_rent_recovery_v1` 5 passed, `funding_ledger_rent_parameter_v1` 4 passed; 0.00 s each). The rows follow — 52b84b369. The tier then found three targets that did not compile or did not pass: the claims program's test module had lost the crate-root `identity` helper 2418e6173 deleted with the Core-effect route (92905bd34: the helper is the test module's own); the bundle-builder's span test compared against a `Vec::new()` whose element type serde_json made ambiguous once the regeneration put it in that graph, and the two browser wire vectors read the web fixtures the clients maker deleted as twins (e907315ae: `Vec::<u32>`, and the SDK fixtures the emission gate pins). PASS in 396 s at e907315ae — build 121 s, 73 targets executed in 237 s inside the 280 s backstop |
| `frames` | PASS | 318 | at 679484ea1; nothing the links read moved after |

### 7.3 The gauntlet, on hbox

Each tier at the named commit under `swarm-build` (MemoryMax 32 G) on hbox —
Linux x86_64, cargo-build-sbf 4.0.0, platform-tools v1.53, rustc 1.89.0 — the
host the Token-2022 fixture is canonical on.

| tier | verdict | seconds | note |
| --- | --- | ---: | --- |
| `tools/gate suites` at 1ef14c5a1 | 11 of 15 green | 1,929 | green: custody, claims, claims-position, claims-fractional, sparse-chain, affine-batch, signed-delta, userposition, registry, fee2tx, postjoin. The four reds are below. |
| tier 1, `tools/gauntlet/run.sh --mode full` at 1ef14c5a1 | NOT green | 979 | 24 witnesses checked, 0 failed; 36 routes executed, 1 refused-only, 7 of 344 refusal codes observed; the census refused to record coverage over two binding problems that are one fact: the founding's pre-fund transaction has been labelled "pre-fund the founding's program-allocated accounts" since 266c1d687 added the escrow, and `tier1/bindings.json` still said "…five…". The binding follows its producer — cccf4d721. Again at cccf4d721: green at cccf4d721 (1,075 s): 24 witnesses checked, 0 failed; 36 routes executed, 1 refused-only, 7 of 344 refusal codes observed; the census records its coverage with no binding problem |
| General hot, `run-general-hot.sh --at` | red at 1ef14c5a1, green at cccf4d721 | 154, then 216 | every test read `dclutch_general_accelerator_sbf.so` from `SBF_OUT_DIR`, the link the fold renamed, though the runner has built `dclutch_accelerator_sbf` since. The five names follow the link (cccf4d721): 5 of 5 — open-batch 662,882 CU, close-batch-seal 602,147, close-batch 641,231, second-open-batch 658,382, submit-candidate-assembled 691,110, submit-candidate-seal 711,026; the foreign entry refused 0x4015 at 116,988, the out-of-sequence close 0x4002 at 40,727 |
| hot-cu, `run-hot-cu.sh --probe` (a diagnostic by the runner's own banner, not release evidence) | 0 of 20 at 1ef14c5a1 | 168 | the continuation floor: twenty seeds at 91,039 / 94,039 / 97,039 / 100,039 / 106,039, every residual exactly 554 below the pinned 91,593 on the clean 3,000 grid, zero jitter — the outer got cheaper with the split and the boxed stage. The floor moves — 44e0cf880. Again at 44e0cf880: PASS 20 of 20, probe MEAN 1,201,700 CU (292 s) — a probe, with no checked all-13 provenance, so not an M-61 quote |

The four suite reds at 1ef14c5a1, with their controls (the same runner at
330bbfaba, and at 3bee5f3f1 — after the programs merge, before the crates
merge):

- **core** — five Series tests in `found_program_test.rs`
  (`series_permit_expiry_uses_only_the_authenticated_successor_profile`,
  `series_consume_accepts_258_outcomes_and_commits_found_with_permit`,
  `series_consume_late_hoard_refusal_rolls_back_found_and_all_replay_state`,
  `series_consume_refuses_to_consume_the_same_ticket_twice`,
  `a_strangers_lamport_cannot_strand_a_scheduled_series_occurrence`) refused
  `CoreSbfError::Reference` (0x3003, the Realm/Product/result-domain/Market
  identity linkage) at instruction 0. At 330bbfaba and at 3bee5f3f1: 15 passed,
  5 failed, 2 ignored — the same five. Main's, before the swarm.
- **claims-lifecycle** —
  `real_token_2022_lifecycle_refuses_ata_substitution_and_rolls_back_every_late_failure`:
  Claims refuses 0x103004 and the lifecycle then 0x5216
  (`RationalLifecycleSbfErrorV2::Token`) where the test asserts acceptance. No
  control: the runner at 330bbfaba and at 3bee5f3f1 refused `--locked` (that
  workspace's lock was stale until the one regeneration).
- **dealer** —
  `real_elf_forwards_a_geometry_complete_frame_into_accelerator_authentication`:
  0xC006 `GeneralAcceleratorSbfErrorV3::HeapFrameNotRequested` where 0xC101
  `DealerAcceleratorSbfErrorV4::InvalidInvocation` was expected — the folded
  link's dispatcher refuses the frame before the Dealer arm sees its content.
  At 330bbfaba the old Dealer accelerator's runner refused `--locked`; at
  3bee5f3f1: the same test, the same 0xC006 for 0xC101 (1 passed, 1 failed). The red is the fold's — the programs branch — and precedes the crates merge.
- **general** — `a_nonvacant_product_staging_cursor_refuses`:
  `ProductGraphObservation(InvalidRecord)` where `Product` was expected. At
  330bbfaba the old General accelerator's own suite: 13 of 13; at 3bee5f3f1:
  13 of 13 (freeze 3, hot_instruction_v3 10), this test among them; at 1ef14c5a1 hot_instruction_v3 is 9 of 10. The one commit that touches the suite between the two is the crates merge (73499583b), and the refusal is the merged Product reader's. The red is the crates merge's.

### 7.1 The frames, function by function

The recaptured baseline against the pre-swarm one (330bbfaba's
`tools/frameguard/baseline.json`: twelve links, 1,888 functions, 1,973 frames;
018ea525f's `tools/gates/frames-baseline.json`: eight links, 1,661 functions,
1,743 frames), every function keyed by its demangled symbol (`rustfilt`, the
capture's `<hash>` placeholders substituted; no two symbols demangled alike).
Of the 1,888:

- **1,322 keep their name and their frame.**
- **312 are renamed with the same frame** — the crates maker's claim, measured:
  152 by trading's `hot_v3` split into its family modules, 82 by its Dealer
  modules taking concept names (`dealer::v3_release` → `dealer::release`, …),
  24 by the accelerator fold (`dclutch_general_accelerator_sbf::` →
  `dclutch_accelerator_sbf::general::`, the Series shadow likewise), the rest
  by the crate merges (`dclutch_market_core_codec`, `dclutch_rent_contract`,
  `dclutch_capability_program_contract`, `dclutch_execution_strategy_contract`
  → `dclutch_market`; `dclutch_registry_svm`, `dclutch_record_contract` →
  `dclutch_registry`; `dclutch_product_*` → `dclutch_product`;
  `dclutch_series_v3_kernel` → `dclutch_trading`; `dclutch_claims_svm` →
  `dclutch_claims`; `dclutch_source_contract` → `dclutch_source`) and their
  trait impls. Eight more the matcher could not pair by name are renames too:
  trading's `dealer::v4_{equity,lp}_accelerator_accounts::*` became
  `dealer::{equity,lp}_accelerator::*` with their frames unchanged (1,856,
  1,216, 64, 384, 256, 384), and `v3_admitted::encode_register_bank` 64 became
  `encode_bank` 64 in each.
- **Exactly one grew**: `dclutch_claims_sbf::process_generic_plan` 1,344 →
  1,728, taking the dispatch that `process_non_fractional_instruction` (2,240
  → 64) no longer does once the Core-effect route is gone.
- **Five shrank**: that `process_non_fractional_instruction`; Custody's
  `process_instruction` 3,904 → 3,840 (the reservation route gone); Trading's
  `process_instruction` 256 → 192 and
  `series::artifacts_v3::series_base_request_digest_v3` 192 → 128; the
  accelerator's `<T as SpecFromElem>::from_elem` 64 → 0.
- **Four renamed and moved**: `hot_v3::execute_authenticated_hot_v3` 3,840 →
  3,776 as `hot_v3::execute::execute_authenticated_hot_v3` (finding (c), the
  boxed stage — its 128-byte `logical_projection_keys_boxed_v3` is the one
  genuinely new Trading function); `dealer::v3_release::encode_dealer_global_program_set_v3`
  1,088 → 1,024 as `dealer::release::…`; the Dealer accelerator's
  `process_instruction` 1,024 → 64 as the fold's `process_instruction` (its
  body is now `dealer::process` 1,024) and `evaluate_selected_family_v4` 384 →
  320 under `dealer::`.
- **209 deleted**, each a unit a maker deleted whole: Custody's
  `dealer_reservation_v1` (31) and `authenticate_reservation_frame_v1`; Claims'
  Core-effect route (9: `authenticate_core_market`, `authenticate_core_authority`,
  `authenticate_economic_accounts`, `authenticate_releases`, the foundational
  split pair, …); Trading's 161 — `dealer_scenario_checkpoint_v1` (the
  checkpoint chain), the V2 Dealer (`DealerProfileV2`, `interpret_projected_v2`,
  …), `dealer::v3_trade`, `v3_trade_artifacts`, `v3_composer`, `v3_lifecycle`,
  `v3_route`, `v3_admitted`, the `v3_accelerator_accounts` scenario half,
  `series::execute_v3`'s composers, `series::projector`, `dispatch_v3`'s three
  program authenticators; the accelerator's 7 (the three constituents'
  entrypoints and `verify_cause`, two `RawVec` growers). And the two deleted
  links' 19 (direct-aot 10, product-runtime-v2 9).
- **The accelerator's other new names** are the fold's dispatcher
  (`top_level_family_magic` 192, `dealer_family_selected` 64) and the
  constituents' bodies re-homed with their old frames (`general::process`
  2,176, `dealer::process` 1,024, `series::process` 64), plus `general::content`
  and `dealer::content` at 128 (the Dealer's was 256) and
  `general::candidate_cause` 64.

The deepest frame is unchanged: `authenticate_strategy_from_sealed_boxed_v3`
at 3,968 under the 4,096 bound. The diff's recipe: demangle both baselines,
match exact names, then names modulo crate root, then `Type::method`, then a
bare method name only where it is unique on both sides.

## 8. What is owed

Each item names its column. Nothing here is hidden behind a green.

- **The gauntlet's three suite reds that are the convergence's** (§7.3):
  `dealer`'s geometry-complete frame refused 0xC006 before the Dealer arm —
  the fold's dispatcher (programs); `general`'s nonvacant staging cursor
  refused `ProductGraphObservation(InvalidRecord)` where `Product` was due —
  the crates merge's Product reader (crates); `claims-lifecycle`'s
  ATA-substitution walk refused 0x103004 then `RationalLifecycleSbfErrorV2::Token`
  where it expects acceptance — no control could run (its lock was stale at
  the base), so the column is claims until one does. Each is a program-test
  on a real ELF and none is a route cohort-16 has deployed.
- **`core`'s five Series tests** refuse `CoreSbfError::Reference` at the
  base too: main's, the Series column's, before the swarm.
- **tier 1**: green again at cccf4d721 with the binding following its producer (§7.3); the 111 routes tier 1 never executes (37 blocked, 74 unclaimed) are the census's standing count, not this convergence's
- **`journey`**: the journey campaign's test target does not compile against
  the ladder (`recovery_ladder`, the 9-argument crank calls: RECOVERY-4's
  6a3079454, 61706bc9a, 8875255a5), because the campaign's `#[path]` mirror
  of the successor was not moved with it — the recovery column;
  `tools/fractional-exterior`: compiles only once it leaves the `solana-sdk =2.1.0` line (§7.2) — the drivers column.
- **`root-targets`**: the census agrees with the table (73 cheap targets, all
  wired — 52b84b369), and the tier then says PASS (§7.2): 73 targets, all wired, all green, after three repairs (92905bd34, e907315ae).
- **`clippy`**: the two merged crates repaired (c666a9976, b2d6660a0); the
  tier again at HEAD: PASS (8 s; §7.2). The four debt reds and the ten packages behind them are the standing register, each row with its owner.
- **`commands`**: the `dclutch-terminal --help` row, red in a fresh checkout
  because its `dist/` is unbuilt (the gates report found the same at main) —
  the clients column: either the probe builds it or the runbook says so.
- **Four web tests stay red, each with its owner**: `capabilitySelectedGate`
  and one `capabilityPhaseGate` expectation — the census publishes no
  selected gate behind the Direct crosscheck (`ROUTE_SELECTED_GATES_V1` is
  empty in the clients branch's census too; the gates and clients columns);
  `explorerCoverage`'s record survey reads `DCLTRIX1` off the SDK's
  protocol-constants table as a record while the explorer renders it as the
  Registry instruction it is (clients); `tradeFlowRefusals`' wall wording
  against the SDK's copies of the modules it reads (clients). Twelve magics
  the protocol-constants table declares and the explorer does not decode are
  exempted with the clients column named; two routes the census enumerates
  and the explorer does not name (the Claims conservation route, the Series
  arm) likewise.
- **The hot-tail sweep is a probe**: `run-hot-cu.sh` quotes M-61 only with
  `--checked-gate` and its sha256; the 20 of 20 at 44e0cf880 and its mean are
  diagnostics until cohort-16's release pack supplies both.
- **The base ELF set's full digests** were lost with the scratchpad (§6); the
  twelve-hex prefixes stand and the set is one `cargo build-sbf` at 330bbfaba
  from being whole again.
- **The parked operator merge's six crates** (§2): a program-test cluster
  bump (solana-program-test =4.3.0-beta.2) that does not panic would let them
  join; measured not to be that day.
- **Rent into Core**, the Series flatten, the Dealer `release` merge, the
  successor as a caller of the operator, the wire-literal census, the 45-clause
  refusal split — each maker's report names its own; none started here.

---

## ADDENDUM, 2026-09-05, lane SUITES-2: §7.3's "11 of 15" was fifteen coin tosses

§7.3's suites row reads `11 of 15 green` and §8 hangs four reds off it. **That
row is not a measurement of fifteen suites. It is one draw of each**, and
`tools/gate suites` at the time ran each row exactly once, so every verdict in
it — green and red alike — was a single toss of whatever distribution that row
has.

For at least one row the distribution was near-even. `claims-lifecycle`'s
`real_token_2022_lifecycle_refuses_ata_substitution_and_rolls_back_every_late_failure`
passed about half its draws from `d6c4dea63` (2026-08-27) onward, against one
binary and one ELF: the honest `RetireCoordinate` is byte-identical to the
nonzero-supply hostile that precedes it, so on the same recent blockhash it
signed to the same signature and the bank refused it as `AlreadyProcessed`
without running the program, and `ProgramTest` registers blockhashes on a
wall-clock timer. Which side of that race the row landed on decided this table's
cell. `docs/evidence/CLAIMS_LIFECYCLE_LAYOUT_WALL_2026_09_05.md`, second
addendum, carries the draws.

So: **the count stands as a count of one draw each, and stands for nothing
more.** No cell of it distinguishes "green" from "green about half the time",
and §8's four reds were each also one toss — three of them were nonetheless
convicted at their authors by fb36992d7 with controls at two commits, so those
survive; the `claims-lifecycle` cell does not.

The instrument is fixed rather than the number restated. `tools/gate suites`
now draws each row `DCLUTCH_GATE_SUITE_DRAWS` times (3 by default), calls a row
green only if every draw is, and reports a row whose draws disagree as
**NONDETERMINISTIC by name** — never folded into green, never into red.

Re-measured on the same host, three draws per row: **15 of 15 green, 45 runs,
every one of them a pass** — 16m59s for the thirteen rows that need no extra
fixture, plus 72s for `claims-position` and `claims-fractional` with
`TOKEN_2022_SO` supplied. Nothing is NONDETERMINISTIC and nothing is red.

The measured tree is a `git archive` export of `9fe4506f2` carrying this lane's
own changes and nothing else — `5c534b016`, `b03b1c26e` and `cda85ef47`'s files
copied in — not a clean export of `cda85ef47`, whose ancestry includes three
other lanes' commits from the same afternoon. It is exactly the tree the row
under repair was measured on before and after, and no file another lane
committed that day is on any of these fifteen runners' paths; a clean export of
`cda85ef47` is what a cut would measure, and this is not that.
