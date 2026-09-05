# SIMPLIFY-CRATES

Branch `simplify/crates`, cut from `main` at `330bbfaba`. Domain: `crates/`
except the seventeen `*-operator` crates (another maker's). Every count below
is measured in this worktree at the named commit, over `git ls-files` and
`wc -l` of `crates/**/*.rs`.

## Counts

| | before (`330bbfaba`) | after (branch head) |
| --- | --- | --- |
| crates under `crates/` | 94 | 46 |
| of which in this lane's domain | 77 | 29 |
| Rust lines under `crates/` | 529,894 | 522,751 |
| root workspace members | 106 | 58 |

The line count moves only by the deletions (−7,143): a merge moves bytes, it
does not remove them. The crate count is where the merges land: 66 of the
domain's 77 crates became 9 authorities, 3 were deleted, and the 29 that
remain are the 9 authorities, 8 `cdylib`s and 3 host crates that are the
operator maker's, and 9 that stay one crate on purpose (below).

## Deletions, each with its census control

| what | lines | control |
| --- | --- | --- |
| `dclutch-resolution-policy-kernel` | 895 | zero path dependents across all 55 workspaces (`cargo metadata` reverse-deps plus every `Cargo.toml`); zero mentions of `categorical_pyth_v1` or `MAX_PRICE_CELLS` outside its directory; no TS mirror. The "live Pyth policy" of the 08-27 carve-out never acquired a caller. |
| `dclutch-economic-kernel` | 1,808 | same census; kept twice before as the Lean refinement witness of `DClutchSemantics.EconomicKernel`, which the ASPIRATION_LEDGER had already named as a model "the programs no longer implement". **For the Lean lane:** `emit-economic-vectors` and `vectors/economic-kernel-v1.txt` now have no Rust reader and `vectors/MANIFEST.md` still says the crate "must" decode them; `formal/` is untouched here. |
| `dclutch-direct-aot-v3-contract` + `tools/gauntlet/aot-cu/twin-v3` + `harness/tests/measure_v3.rs` + the README section | 2,104 + twin | one consumer, a measurement ELF the README says "cannot be reproduced from a bare checkout" (the crate never compiled for `target_os = "solana"` without an uncommitted gate). The measurement lives in `docs/evidence/DIRECT_HOT_AOT_MEASUREMENT_2026-08-31.md`; the relation's authority is the Lean-authored program in `direct-codec/src/generated_ordinary_v3.rs`. Three prose citations now name that program. |
| `dclutch-market::capability_manifest::readiness_instruction` | 348 | module and every item name grepped over `.rs/.ts/.mjs/.sh` in all trees: zero; magic `DCLTRDY1` appears nowhere else. |
| `dclutch-claims::lbv2_terminal_v2` | 927 | same; `DCLBTR02`/`DCLBTE02` appear nowhere else. |
| `dclutch-claims::founding_v4` | 1,123 | one line outside the module: a hostile test in `claims-sbf/founding_v5.rs` copying the V4 request magic to prove V5 refuses it. The test now flips one byte of the V5 magic and asserts the same refusal. (The architect's map read that site as "one wire, two module names" and asked for a merge; it is a refusal test, and nothing decodes V4.) |
| `dclutch-trading::general::runtime_candidate` | 247, kept | one consumer, `runtime_settlement`'s test module; it is now `#[cfg(test)]` rather than a public API with no caller. |

No program linked the three crates and the modules were unreachable from every
program, so the expected ELF control for those commits is identical size with
`.strtab` hash moves only.

## Merges: one crate per authority at one layer

Target from the architect's map §1.2; mechanism `tools/simplify/merge_crates.py`,
spec-driven and replayable per group on any branch (`--resweep` re-runs only
the text-reference sweep; `product-svm-reader` is the fold-into-existing-target
form). Layout rule: the constituent the family already reaches most stays at
the crate root -- so the two Lean-emitted Direct layouts that carry `$crate::`
macro paths move untouched -- and every other constituent is `pub mod <name>`
with a `/// Formerly the `<crate>` crate.` line. A consumer changes exactly the
crate segment of a path (`dclutch_custody::token_svm::X` -> `dclutch_custody::token_svm::X`).

| target | root constituent | modules (former crates) |
| --- | --- | --- |
| `dclutch-sbf-runtime` | `sbf-bump-heap` | `cu_checkpoint` |
| `dclutch-custody` | `custody-contract` | `token_svm` (its `program-test` workspace and PROVENANCE travel with it) |
| `dclutch-registry` | `registry-contract` | `svm`, `record`, `release_set`, `activation_auth_v1` (behind `svm`) |
| `dclutch-vm` | `transition-vm` | `effect`, `account_profile`, `request_profile`, `capability_seal` |
| `dclutch-market` | `market-core-codec` | `capability_manifest`, `capability_program`, `capability_activation` (host-only, `cfg(not(target_os = "solana"))`), `execution_strategy` (with its `alloc` feature), `realm`, `rent`, `protocol_parameters` |
| `dclutch-product` | `product-runtime-v2` | `contract`, `admission`, `payoff`, `economic_slice`, `svm_reader` (behind `svm`) |
| `dclutch-source` | `source-contract` | `resolution`, `relay`, `pyth` (with `synthetic-local-fixture`) |
| `dclutch-claims` | `claims-svm` | `conservation`, `fractional_kernel`, `fractional`, `fractional_lowering`, `rational_kernel`, `rational_request`, `rational`, `rational_lifecycle`, `composition`, `bearer`, `structured_kernel`, `structured`, `position_admission`, `product_representation_reader_v3` (behind `svm`) |
| `dclutch-trading` | `direct-codec` | `dealer_scenario`, `dealer`, `general_codec`, `general_config`, `general`, `series`, `shadow_accelerator_auth` (behind `svm`) |

Per-group control: `cargo check --tests` on the target and on its direct
consumers (named in each commit), every workspace lock regenerated offline,
and at the end `cargo check --workspace --all-targets --offline` green on the
root workspace.

Deviations from the map, each forced by a cycle or a layer:

- `capability-seal-contract` -> `dclutch-vm`, not `dclutch-market`: every
  waist constituent depends on the seal and the Market's capability program
  depends on the waist.
- `execution-strategy-contract` -> `dclutch-market`, not `dclutch-trading`:
  Claims' Fractional contract depends on it while Trading depends on Claims.
- `core-contract` stays its own crate: Registry depends on it and Market
  depends on Registry, so it cannot live in Market.
- `product-runtime-v2-svm-reader`'s `representation_v3.rs` moved to Claims
  (`product_representation_reader_v3`) before the reader could join Product:
  it reached the Rational kernels, which reach the payoff codec. The reader's
  two private helpers it used are public with docs; the reader's integration
  test takes Claims as a dev-dependency (a dev-cycle cargo permits).
- `product-compiler` stays its own crate: it is host code (`std`, `Vec`,
  sha2) with two host consumers, and folding it into a `no_std` authority does
  not compile. The layer rule sends it to the operator merge.
- `direct-ticket` stays: serde, keypairs, signers -- host layer, the operator
  merge's.
- `liability-basis-v2-kernel` stays: the option-D ruling wires it as a
  dev-dependency precisely so the differential reference reaches no ELF;
  folding it in would delete that enforcement.
- SDK-dependent constituents inside an SDK-free authority sit behind a `svm`
  feature; consumers that used them ask for `features = ["svm"]`.

## What moves a program's ELF bytes

No executable line changed. Every merge renames symbol paths in every ELF
that links the crate (`dclutch_custody::…` -> `dclutch_custody::…`),
which moves `.strtab` and the frameguard baseline's symbol names and nothing
else. `tools/frameguard/baseline.json` is deliberately the pre-merge capture
so the convergence recapture shows renames only; the per-ELF `sha256` and the
census recipe's control (identical size, every differing byte a `.strtab`
hash) are the convergence build's to print. One deliberate change of reach:
`dclutch-market::capability_activation` is `cfg(not(target_os = "solana"))`,
the guard the General adapter's host-only dependency used to stand in for.

## Regenerated, not edited

`tools/emission-guard/COVERAGE.md` (101 generated files, 101 guarded, 78
guards; `--verify` PASS) and `fixpoint-debt.tsv` (one recorded row:
`protocol_parameters/generated.rs` was never in the census before);
`tools/seam-audit/baseline.json` (verdicts carried across the moved paths;
109 `DOMAIN_RAW_RESTATEMENT` findings the merge *created* are recorded under
the existing `debt-derivation-restatement` verdict -- the rule's owner is a
crate, and `ACTIVATION_PDA_DOMAIN_V1`'s owner now exports seed constructors it
never exported as `capability-contract`; a per-domain owner in `seam_rules.py`
would retire those rows, and is the gate maker's); `tools/sbom/SBOM.md` and
`NOTICES.md`; every `Cargo.lock`; `apps/dclutch-web/lib/generated/**` and
`packages/dclutch-sdk/lib/generated/**` from their generators (whose Rust
inputs were re-pointed). `docs/reference/**` is genref's (convergence).
`tools/ci/root-targets.tsv` rows follow the renamed test targets; the one
target that had never had a row (`capability_manifest__funded_rent_v1`) is
wired at its measured time.

## Left on purpose

- `dclutch-direct-aot-contract`: goes with the unshipped `direct-aot` program
  (map §1.4(a)); the programs and deletion makers own that pair.
- `dclutch-fractional-cubic-life-evidence`: lives with `tools/fractional-exterior`,
  its one campaign.
- `dclutch-svm-harness`: the real-ELF harness, its own workspace, stays.
- The 55 nested workspaces: the root `Cargo.toml` is this lane's; collapsing
  the program-test, gauntlet and driver workspaces crosses the programs, gate
  and successor columns (§2.5) and is theirs.
- `map_err(|_| X)` in contracts (~2,000 sites in the domain) and the
  `MAX_OUTCOMES` four-author bound: named, not done. The economic-kernel
  author of `MAX_OUTCOMES` is gone with the crate; the remaining authors are
  the realm ABI (Lean) and the two codec re-exports.
- Frameguard recapture, `docs/reference` regeneration and `genref --converge`:
  convergence, once, after every maker lands.

## Branch

Fifteen commits from `330bbfaba`; the merge commits after `6fed44545` are
unsigned (1Password refused the signature while this lane ran unattended).
