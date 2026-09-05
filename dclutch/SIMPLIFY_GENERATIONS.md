# SIMPLIFY-GENERATIONS — superseded generations deleted whole

Branch `simplify/generations`, four commits on `330bbfaba`:
`aef29f74c` (nine modules), `c4fbb9e10` (eight tools), `281fc7233` (two
kernels), `cb9f0c1dc` (the direct-aot cluster, band 10 retired).
**113 files, +301 / −40,761.** Every deletion is a whole file, directory or
crate; the only in-file edits are the `mod`/`pub use` lines, register rows
and count literals a whole-unit deletion forces, and one test file's
artifact section (named in `aef29f74c`).

## Method (three read-only passes, then one join per unit)

1. **Crate level.** `cargo metadata` reverse-deps on the root workspace,
   then every `Cargo.toml` in all 55 workspaces and every
   `.rs/.ts/.mjs/.sh/.py/.lean/.json/.yml` for each name in hyphen and
   underscore form. Zero-dependent crates that survived this pass: 11;
   only two (the kernels) had no consumer outside the root member list and
   one generated register.
2. **Item level.** The tree tokenized once — 1,047,885 `file:token` rows
   over programs/, crates/, tools/, apps/, packages/, formal/, fixtures/ —
   joined against 17,490 `pub` definitions. A symbol counts as a unit's
   own only if it is defined nowhere else; a unit is dead when no non-test
   file outside it, and no `lib.rs`/`mod.rs` statement other than
   `mod`/`pub use`, mentions any own symbol or the module name. 31 files
   came out with zero non-test external consumers; each was then read.
3. **File level.** Every `.rs` under crates/programs/tools checked for a
   `mod`, `#[path]` or `include!` naming it: **0 orphans.** Every
   `DClutchSemantics/*.lean` checked against every `import`: **143/143
   imported, 0 orphans.**

Same-directory generation pairs (27 stems with two suffixes, 9 stems with
an unsuffixed sibling) were joined one by one: **every older generation is
a dependency of its successor** (`founding_v5` writes the V4 magic,
`hot_v6` reads `hot_v3`'s constants, `release_v5` calls `release_v4`,
`selected_*_v6` calls `_v5`, `continuation_v2` wraps `_v1`, …). The map's
two-digit-magic verdict — stacking, not death — holds at file level too;
nothing was taken from a suffix.

## Census: deleted

| unit | generation / what it was | superseded by | non-test reverse deps | control |
| --- | --- | --- | --- | --- |
| `crates/dclutch-general-config-contract/src/activation.rs` (490) | General V2 activation request | `general-adapter-contract::activation_bundle_v1` (`DCGNACT1`, 2026-08-30) | none of 6 pub items | join; cargo check |
| `…/general-config-contract/src/root_v3.rs` (497) | `activate_general_owned_v3`, `plan_general_activation_v3` | `dclutch-operator::general_activation_v3` (decision 0006 §8.1); last callers removed by `06be6ed29` | none (one comment in a program-test) | join; git log -S |
| `crates/dclutch-operator/src/direct_successor.rs` (165) | second producer of the `domain‖CompactIntentV2` signing message | `direct-codec::intent_v2::signed_preimage` (+ SDK `directInlineV3.ts:385`) | none | join |
| `crates/dclutch-operator/src/capability_program_set_v2.rs` (126) | generic `build_capability_program_set_v2` | per-family set encoders (General, Rational v5/v6, Dealer v4) | none | join |
| `programs/dclutch-trading-sbf/src/dispatch_v3.rs` (80) | CapabilityProgramV3 content dispatcher | `hot_v3::authenticate_descriptor_root_selection` → `CapabilityProgramV4::validate_selection` | none | join; cargo check |
| `programs/dclutch-trading-sbf/src/dealer/v3_lifecycle.rs` (533) | Dealer activation/retirement staging, 4 pub fns | the common state-lifecycle executor + V4 releases | none | join; seam row removed |
| `crates/dclutch-claims-svm/src/lbv2_terminal_v2.rs` (927) | `DCLBTR02`/`DCLBTE02` terminal wire | `product_basis_terminal_v3` (same day, emits `SignedDeltaPlanV3`) | none; no program decodes the magic | join; seam row removed |
| `crates/dclutch-product-contract/src/terminal.rs` (322) | `TerminalResultV1` (`DCLTEND1`) | resolution's terminal facts in `dclutch-source-contract` V2 | none; magic appears nowhere else | join |
| `crates/dclutch-fractional-claim-contract/src/artifacts.rs` (~600) | V1 artifact bundle, owed by `53d73d4ee` | V4 in the operator | `lib.rs` re-exports + own test only | join; the test's artifact section removed |
| `tools/release/{lifecycle-chaos,private_validator_upgrade,devnet-flight}/`, `devnet-recycle.sh`, `devnet-observe.sh`, `devnet-demo-pulse.sh`(+test), `tools/sbf-footprint.py`, `tools/atomic-generate/` (5,613) | tools nothing runs | successor `observe`/cohort runbook; `sbf-frame-sizes.py`; `AGENTS.md`'s atomic-replace rule + each `check-generated.sh` | runner/code referrers: none beyond their own tests in `run.sh` | referrer census by category; `bash -n run.sh` |
| `crates/dclutch-economic-kernel` (1,808), `crates/dclutch-resolution-policy-kernel` (895) | Lean refinement witness / old Pyth policy | (Lean module kept) / `source-contract` V2 policy | root member list + `capabilitySurfaceV1.ts` (regenerated) | cargo metadata + all-manifest grep; cargo check; **reverses two KEEP rulings** (board-archive :11172, ledger :2520) on the map's and coordinator's instruction — separate commit, one revert restores |
| `programs/dclutch-direct-aot-sbf` (573), `tools/gauntlet/direct/`, `tools/gauntlet/aot-cu/`, `crates/dclutch-direct-aot-v3-contract` (2,104) | unshipped program (`SHIPPED_LINKS` false), its self-witness campaign, its CU twins, the V3 contract only the twins read | the live Direct route is `hot_v3` inline | v3-contract: aot-cu only; program: its campaign + registers | every register moved in the same commit; band 10 retired by theorem; registers re-emitted from built Lean |

**Lines deleted per domain:** crates −8,966; programs −4,497; tools
−27,208 (incl. the two deleted nested workspaces' lockfiles); root −69
(workspace members + seam/frameguard rows); formal −15/+12 (the band row
and two theorems); apps/packages −6/+6 (re-emitted band twins,
regenerated capability surface). **Crates deleted: 3** (economic-kernel,
resolution-policy-kernel, direct-aot-v3-contract) **+ 1 program + 5 nested
workspace crates** (aot-cu twin/twin-v3/harness, gauntlet/direct producer).

## Emitters and Lean follow-ups (named, not touched — FORMAL maker)

- `EmitGeneralConfigAbiRust.lean`: its `GENERAL_ACTIVATION_*_V2` /
  `ACTIVATION_*_OFFSET` block lost its only reader (`activation.rs`); 21
  `never used` warnings in `general-config-contract/src/generated.rs` until
  the block is deleted and re-emitted.
- `EmitEconomicVectors.lean`, lakefile exe `emit-economic-vectors`,
  `vectors/economic-kernel-v1.txt` and its `vectors/MANIFEST.md` row: no
  Rust reader since `281fc7233`.
- No emitter's generated file was deleted (direct-aot-v3-contract had
  none; `direct-aot-contract`'s stays); `emission_guard --verify` was
  already STOP at HEAD for an unrelated protocol-parameters emission.

## Re-emissions owed at convergence (not hand-edited here)

`docs/reference/{routes,route-witnesses,refusals,programs}.md` via
`genref --converge` (routes 164 → 163, refusals minus band 10's four);
`lib/generated/{routeCensus,refusalRegistryV1}.ts` in apps and packages;
`tools/sbom/SBOM.md` (`--locked`); root `Cargo.lock` (regenerated, never
merged — left uncommitted on this branch); the frameguard §3.4 recapture
(the baseline's direct-aot link was removed and `link_count` set to 11 so
`upgrade.rs:6592`'s gate agrees with `SHIPPED_LINKS`; `commit` left as
captured).

## Deliberately left, with the invariant each teaches

- **`crates/dclutch-general-adapter-contract/src/plan.rs` (1,016 + tests)** —
  the V2 plan layer, dead by join and superseded per decision 0006 §5, but
  `MECHANISM_BATCH_SPINE` §(b) and §(d)(i) cite `plan.rs:453`
  (`require_certificate`) and `:414-430` as live enforcement sites. A note
  citing a dead layer is the finding; the deletion is one re-citation
  away (the reachable conjuncts live in `runtime_settlement.rs`,
  `runtime_selection.rs` and `GeneralTransitionV3.lean`).
- **`trading-sbf/src/dealer/v3_route.rs`** — only consumer is a real
  assertion in `tests/dealer_v3_multi_lp.rs:433-451`; deleting it edits a
  test's meaning.
- **Series: `execute_v3.rs`, `shadow_operator.rs`,
  `operator/series_projected_v2.rs`, the three Series builders** — the
  live family's route-missing half (D7 pending; five Series commits this
  week).
- **`operator/general_invocation_v1.rs` + `adapter-contract/invocation_v1.rs`**
  and **`capability-contract/readiness_instruction.rs`** — producer-missing
  readers named by the C16 rehearsal, not generations.
- **`claims-svm/src/request_layout.rs`** — dead by join, but has no
  successor to name; its custody twin is live.
- **`bearer-v2-operator/open_{selected,structured}_transaction_v3.rs`**,
  **dealer `v4_lp_operator`/`v4_scenario_operator`/`v4_scenario_release`** —
  builders whose only callers are the real-ELF program-tests (the accepted
  campaigns), not superseded.
- **Dealer equity builder, registered-Direct V4 artifacts** — map §1.4(e).
- **`crates/dclutch-direct-aot-contract`, `tools/direct-translation-validator`**
  — the checked release's translation evidence (`CheckedTranslationValidationV1`
  hashes the contract's `lib.rs`/`generated.rs`; CI runs `check.sh`).
- **`programs/dclutch-product-runtime-v2-sbf`** — deferred: the web's
  `generate-product-runtime-v2-admission.mjs` reads its `lib.rs` to emit the
  browser's admission mirror, `ProductV2Studio.tsx` names it as the
  admission program, and the SDK's `productRuntimeV2Admission.ts` calls it
  the authority. Banishing it is a client rewrite (CLIENTS + PROGRAMS).
- **`tools/lineage-loopback`** — invoked by `gauntlet/lineage/run-lineage.sh:52`;
  a witnessed campaign, not dead. **The other 14 of the map's "22" release
  scripts** — each has a runner, code or runbook consumer (`c4fbb9e10`).
- **The eight `.wasm` blobs** — each has a live `lib/*V1.ts` loader; they
  are checked-in build outputs, a CI/CLIENTS decision, not a generation.
  **The 129 web/SDK twins** — CLIENTS' column (`tools/twins`).
- `transition-vm::v2`, `walletTerminalPayoutV1.ts`, every same-directory
  older generation — live as dependencies of their successors.

## Gates as found

seam-audit and doc-citations were red at `330bbfaba` (39 and 24
pre-existing rows); after this branch their finding lists are identical /
contain no deleted path. `cargo check --workspace --offline` green after
every batch. No program was built; no ELF byte was measured, and the
commits say so. One `git stash` of `upgrade.rs` happened by mistake during
`cb9f0c1dc` and was popped immediately; the two older stash entries are
not mine and were not touched.
