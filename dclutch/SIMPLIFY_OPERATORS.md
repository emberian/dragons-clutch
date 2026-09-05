# SIMPLIFY-OPERATORS — the host operator crates, 2026-09-04

Branch `simplify/operators` from `330bbfaba`; the crate merge is parked on
`simplify/operators-merge-wip` (§6). Every measurement below is on this
worktree at the commit named beside it.

## 1. The crate table

Seventeen host-only crates, 105,178 source lines at the base. Every one has a
non-test dependent except `dclutch-structured-v2-operator` (dev-dependency of
`claims-sbf` only), which the map's §1.4(c) keeps as a named producer-missing
reader (decision 0029 item 7). None was deleted; the merge of all seventeen
into `dclutch-operator` is scripted and parked (§6).

| crate | src lines | discards before → after | carrying variants added |
| --- | ---: | ---: | ---: |
| dclutch-operator | 55,178 → 53,205 | 894 → 160 | 233 |
| dclutch-resolution-core-v3-operator | 8,864 | 224 → 62 | 17 |
| dclutch-fractional-claim-operator | 6,705 | 179 → 39 | 35 |
| dclutch-product-runtime-v2-operator | 4,244 | 97 → 32 | 17 |
| dclutch-market-retirement-v1-operator | 1,733 | 61 → 7 | 15 |
| dclutch-representation-composition-v3-operator | 2,339 | 60 → 14 | 9 |
| dclutch-rational-representation-v2-operator | 2,065 | 59 → 24 | 11 |
| dclutch-bearer-v2-operator | 8,980 | 45 → 44 | 0 (already carried ten; the 44 are `TryFromIntError`/`ContentId::new`) |
| dclutch-provider-transport-v3-operator | 2,362 | 44 → 0 | 12 |
| dclutch-structured-v2-operator | 1,628 | 35 → 8 | 8 |
| dclutch-market-open-v1-operator | 1,014 | 21 → 6 | 7 |
| dclutch-market-founding-v1-operator | 673 | 14 → 2 | 4 |
| dclutch-wallet-terminal-payout-operator | 2,329 | 8 → 8 | 0 (string errors carry a message) |
| dclutch-source-readiness-operator | 2,654 | 6 → 6 | 0 (same) |
| dclutch-versioned-message-operator | 791 | 3 → 3 | 0 (`InstructionError`, `CompileError`: not `Copy`) |
| dclutch-general-successor-operator | 1,555 | 2 → 2 | 0 (struct-variant refusals) |
| dclutch-wallet-terminal-input-operator | 1,690 | 0 → 0 | 0 |
| **total** | **105,178 → 103,581** | **1,750 → 417** | **368** |

## 2. The discard class — 1,750 → 417 sites, 1,452 carrying

Method: the CAUSES lane's type oracle, scripted. Rewrite every `map_err(|_|`
to `map_err(|_: ()|` in one crate, `cargo check -p`, read every E0631's
"expected closure signature `fn(T) -> _`", restore. 1,724 of 1,752 sites
resolved to a type; the 28 unresolved are `Result<_, ()>` callees
(`decode_rent`, `decode_clock`) and `TryFrom` slice conversions.

The rule, applied by `gen.py` per crate and read by hand per enum:

- a site whose callee returns a typed contract/kernel/codec/program error maps
  through a variant of its operator enum carrying that type — one variant per
  source type per enum, named for the authority that refused (`Registry`,
  `RegistrySvm`, `MarketCore`, `Capability`, `ProductBasis`,
  `LiabilityBasisState`, `GeneralCandidate`, `RuntimeVerify`, ...); an
  existing variant already carrying the type is reused; a name taken by a
  surviving unit variant gets the long form (`RegistryContract`,
  `ObservationError`);
- a callee that returns the operator enum itself propagates with `?`;
- a coarse unit variant is deleted when nothing references it any more
  (`ChildFrame`, `ArtifactMismatch`, `Encoding`, `Request`, `Intent`,
  `Selection`, `InvalidGraph`, the seven `FractionalSelectedArtifactErrorV4`
  stages, ...) and kept where a site still has nothing typed to carry;
- left as a name, not a flattening: `TryFromIntError` (202), `ContentId::new`
  (one cause, 45), `TryFromSliceError`, `PubkeyError` (17), `ProgramError`
  (20), `InstructionError` (7), `solana_message::CompileError` (6) — the last
  four are not `Copy` and every operator error is; `dclutch_release_tool::Error`
  (6, not `Copy`); the seven sites that map into the Trading program's
  `SeriesOperatorErrorV3::Content` (a program enum, not this lane's to widen);
  the `format!`/`Error::new` sites of the two wire crates, which already carry
  a sentence; and 40 struct-variant refusals that already name their conjunct
  (`DirectInlineFinalizationRefusalV3::{DescriptorDecode, PoststateWidth, ...}`).

The three collapses the census named: `GeneralHotOperatorErrorV3::ChainState`
(55 sites, six enums) → eleven carrying variants; `ResolutionCoreOperatorErrorV3::
Encoding` (54) → `MarketCore`/`Resolution`/`ReleaseSet`, `Encoding` kept for the
`()` and `TryFromIntError` sites; `TerminalRetirementErrorV1::Projection` (42)
→ `Capability`/`MarketCore`/`ReleaseSet`/`RetirementReplayHandoff`/
`LifecycleRent`/`CustodyContract`, `Projection` kept for six `TryFromIntError`s.

Two enums stop deriving `Copy` to carry a non-`Copy` cause
(`ClaimCheckCompactionOperatorErrorV1`); `Display for DirectInlineRouteErrorV3`
prints a carrying variant's cause; the sealed-report projection maps the new
`DirectInlineTransaction` cause to the `Instruction` discriminant its site
published before.

Tests: 64 assertions across seven crates asserted the coarse code and now
assert the printed conjunct, each rewritten from the value the run printed
(`Capability(InvalidDependency)` where `Manifest` was, `Registry(Registry(
ElfDigestMismatch))` where `Registry(InvalidDeployment)` was, ...). Green:
dclutch-operator 274/274, and the eleven crates the rewrite touched. The 22
references to these enums outside the operator crates (svm-harness tests,
program-tests, the successor) all name variants that survive.

Commits: `f1f70de79`, `31f7ef75e`, `a742a42f2`, `0c9811bc6`.

## 3. Deletions (`a1cd3a5ef`, −2,200 lines)

Control: every `.rs`/`.py`/`.ts`/`.mjs`/`.sh`/`.json` read for each symbol;
the only references were the definitions.

| deleted | lines | why it was dead |
| --- | ---: | --- |
| `operator/src/series_projected_v2.rs` + its bound test | 1,152 + 300 | the compact projected Series Consume wire; no campaign reaches it, the Series lanes drive `series_hot_v3` and route 4 |
| `operator/src/general_invocation_v1.rs` | 393 | the durable General caller; the fifteen-action release and the successor session are the path; the contract half keeps one consumer and stays |
| `operator/src/direct_successor.rs` | 165 | Direct V2 signing plans; the ticket crate authors the intent, the V3 route consumes it |
| `operator/src/registry/hot_continuation_v2.rs` | 138 | the headerless Registry continuation builder; the Registry program's route restates its constants and is the authority |
| `operator/src/capability_program_set_v2.rs` | 126 | a wrapper over `encode_program_set_v2`, which its 24 real callers call directly |
| `rational_selected_actions_v1()`, `structured_selected_actions_v1()` | 12 | getters over constants their callers import |
| 13 constants (`DIRECT_*_RECORD_LABEL_V1` ×7, `CLAIM_CHECK_COMPACTION_*_ACCOUNT_V1` ×4, `REGISTRY_REAUTHENTICATE_ACCOUNT_COUNT_V1`, `TERMINAL_PAYOUT_INPUT_FORMAT_V1`), the `lifecycle_rent_v2` re-export alias | 30 | zero readers; the last was an alias of `INPUT_FORMAT`, which stays the one author |

The seam register drops the two `UNSET_GUARD_PRESENT` inventory rows whose
files are gone; the audit's NEW/GONE set on this branch is the live tree's
(35 NEW / 4 GONE, pre-existing) minus exactly those two.

## 4. Kept and named, not deleted

- `dealer_lp_hot_v4` — no caller; the LP campaign runs through the program-test
  bundle builder, which can never submit. The module doc now says so.
- `product-runtime`'s `graded_basis_v3` — decision 0029 item 2 keeps curvature;
  the reader waits for its producer. Doc says so.
- `dealer_equity_hot_v3`, the three Series Hot builders,
  `dclutch-structured-v2-operator` — map §1.4(e).
- `registry/hot_continuation_v1`'s builder has no caller either; its two
  constants are read by a Trading program-test, so the module stays whole.

## 5. Constants restated from programs (not moved — the owner is not a host crate)

`PROVIDER_SUBMIT_ACCOUNT_COUNT_V3 = 38` / `PROVIDER_RECLAIM_ACCOUNT_COUNT_V3 = 18`
have three authors (resolution-core operator, provider-transport operator, the
Resolution program); `INITIALIZE_INFRASTRUCTURE_ACCOUNT_COUNT_V2 = 21` two
(operator, Core program); `PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1` two (operator,
Resolution program); the 64 account-lock limit three names
(`SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1`, `DIRECT_INLINE_DEVNET_ACCOUNT_LOCK_LIMIT_V3`,
`FRACTIONAL_DEVNET_MAX_ACCOUNT_LOCKS_V3`). The operator cannot import a program
crate for a count; the one author belongs in the contract crate that owns the
frame (crates maker). The merge collapses the two operator copies of the
provider counts to one.

## 6. The merge — scripted, parked

`tools/lane/merge-operator-crates.py` (`d100d5991`) performs the map's §1.2
host merge: nineteen crates (the sixteen operators, `rational-lifecycle-hot-v3`,
`hot-bump-miner-v1`, `fractional-cubic-life-evidence`) become
`crates/dclutch-operator/src/<authority>.rs` modules named without generation
suffixes (`bearer`, `fractional`, `resolution_core`, `provider_transport`, ...),
every consumer manifest re-pointed, the root member list −19, features carried
(`successor`, `test-fixtures`). Applied on `simplify/operators-merge-wip`
(`32e9ed2cb`): the merged crate checks green with default features and the
root workspace resolves. Not green, named in that commit: `bincode` used
outside the `successor` gate; two inference errors in a moved test; two
consumer workspaces (`svm-harness`, `affine-batch`) pin
`solana-address-lookup-table-interface` differently from the union; ~30
non-Rust readers of the old source paths (SDK/web emitters, the parity test's
`cargo run -p`, `root-targets.tsv`, `fmt-baseline.txt`, `clippy-debt.tsv`,
three release scripts) and one generated table (`OPERATOR_CRATES_V1`) to
re-emit. It belongs after the program makers land (map §3 step 3), which is
why it is not on this branch.

## 7. What was run, and what was not

Run: `cargo check -p` for every touched crate and the six root-workspace
dependents (five wasm crates, general-successor); `cargo test -p` for the
eleven rewritten crates (no ELF, no validator); `cargo metadata --offline` on
the root; the seam audit against the live tree. Not run: the consumer
workspaces' compiles (successor, gauntlet, program-tests, svm-harness — each
its own target directory under a shared CPU and a 31 GiB disk), the wasm32
target (`getrandom` refuses a bare `cargo check --target wasm32` regardless of
this branch), and any program-test — the 22 external references were checked
by reading. No SBF link moves: every touched crate is host-only.

## 8. Ledger

Operator crates 17 → 17 (merge parked); source lines 105,178 → 103,581; test
lines −300; discard sites 1,750 → 417 (1,452 carrying); carrying variants
+368; coarse unit variants deleted 28; deletions 2,200 lines across 18 files.
Branch head: see `git log --oneline 330bbfaba..simplify/operators`.
