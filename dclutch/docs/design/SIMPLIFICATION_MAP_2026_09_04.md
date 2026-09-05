# The simplification map — dClutch at a third the size

Status: **architect's map for the 2026-09-04 simplification swarm** (lane
SIMPLIFY-ARCHITECT). Written from reading, `cargo metadata`, `rg`, `wc`,
`filecmp` and `git log`; nothing here was built. Every number below was
measured in the worktree `/private/tmp/…/simplify-architect` at
**`330bbfaba`** (`main`, 2026-09-04) unless a line says otherwise. Ten domain
makers are cutting in parallel and will not wait for this; the convergence
lane reads §2 and §3 when it merges them.

The mandate, verbatim (ember): *"review this project and change anything they
wanna change rewrite anything they wanna rewrite delete whatever they wanna
delete. Can we somehow make this system way better and way simpler?"*

The constraint this map holds constant: **decision 0031's six mechanism notes
are the future** — cohort-17 lands joint clearing, the scoring Dealer and the
ensemble; the batch spine, the founder bond and conditional markets are
designed and unscheduled. A simplification that deletes what a mechanism note
names as *reused as-is* is a regression, not a cut (§1.7 lists them).

## 0. The tree today, in the units that matter

| unit | count | lines | the measurement |
| --- | --- | --- | --- |
| programs | 12 (register; band 7 `dclutch-dealer-sbf` deleted 09-02) | 308k (Trading 148k of it: 103k src + 43k program-test) | `find programs -name '*.rs'` |
| crates | 94 in the root workspace (108 members with test-support); **30 carry a generation suffix in their name; 17 are `*-operator`** | 534k | biggest five: `operator` 55k, `general-adapter-contract` 52k, `direct-codec` 35k, `claims-svm` 26k, `account-profile-contract` 24k |
| tests | 55 program-test / test-program crates + the harness | 86k + 22k | nested workspaces under `programs/*/` and `crates/dclutch-svm-harness` |
| tools | 37 top-level entries | 352k (`local-validator` 184k — all of it the successor driver, 74 files; `gauntlet` 46k, 31k of it lockfiles; `release` 34k Python, 22 of its scripts unreferenced; `relayer` 14k) | |
| apps | 1 (`dclutch-web`) | 138k as `wc` counts it — but 81k of that is **eight `.wasm` binaries (3.9 MB) checked in**, 24.6k components, and 33.8k byte-identical to the SDK | |
| packages | 2 (`dclutch-sdk` 72k, `dclutch-cli` 14.6k) | 76k | |
| formal | 4 dirs | 69k Lean in `dclutch-semantics` (143 modules, all reachable from the root; 105 `Emit*.lean`; 100 generated files from 94 emitters, 177 guard rows, all guarded) + three `qedsvm-*` captured-proof dirs | `emission_guard.py` COVERAGE.md |
| docs | 115k lines | `evidence` 41k (104 files), `design` 30k, `decisions` 10k; `GOAL.md` 4,992, `WAVE.md` 7,112 | |
| Cargo workspaces / lockfiles | **55 / 71** | **272,509 lines of `Cargo.lock` — 17% of the tree** | `find . -name Cargo.lock \| xargs cat \| wc -l` |
| `#[path = "../…"]` cross-links | 84 | | the workspace count made visible in source |
| wire magics | **255** distinct 8-byte literals in programs+crates; **14 families carry two generation digits at once** | | `rg -o 'b"DC…"'` |
| refusal codes | 357, **76 observed firing** on a real cluster; `TradingSbfError::Content` has **1,818 raise sites** | | `docs/reference/refusals.md`; `rg -o 'TradingSbfError::…'` |
| hand-written wire literals outside programs/crates | successor **423** (32 files), web `lib/` **374** (86 files), SDK `lib/` **299** (87 files), gauntlet 175, release 104 | | `rg -o 'DC[A-Z]{2,4}[A-Z0-9]{2,4}'`, generated dirs excluded |
| web ↔ SDK twins | **129 files byte-identical (33,842 lines)**, 33 differing; 22 of 23 shared `generated/` modules identical; 25 vs 27 near-identical `generate-*.mjs`; meanwhile 52 web files *already* import `@dclutch/sdk` | | `filecmp` over `lib/` |
| frame ratchet | 12 links = the 12 program ELFs, **1,973 frames** (Trading 917) | | `tools/frameguard/baseline.json` at `272fb867` |

The shape of the bloat is not "too many features". It is **generations kept
side by side** (V1 wrapped by V2 wrapped by V3, each with its own crate, magic,
emitter, band offset and test workspace), **one fact with several authors**
(the browser, the SDK, the successor and the operator each spelling the same
wire by hand), and **one job with several tools** (a gate per script, a
workspace per test, a runbook per cohort). A third of the size is what is left
when each of those becomes one — and a sixth of the tree is lockfiles.

## 1. What dClutch is at a third the size

### 1.1 Programs: 7

| role | today | target | why |
| --- | --- | --- | --- |
| `registry` | `dclutch-registry-sbf` | **stays** | release lineage authority; 11 routes, devnet-witnessed |
| `core` | `dclutch-core-sbf` | **stays, absorbs Rent** | the Market root and its lifecycle |
| `rent` | `dclutch-rent-sbf` (1,471 lines, 4 routes, band 0x2, 24 frames) | **not a program** — its `LifecycleRentInstructionV2` routes (Create/Sweep/Close) become a Core family under a Core magic; the credit account becomes Core-owned | the credit is *Market-generation-scoped* (its own header says so) and Core owns the generation; today Core `found.rs:825` has to check that the credit's *owner is another program*, which is the seam the fold deletes. Decision 0030 already made the persisted fact a RATE in the funding ledger header, so nothing about the credit is Rent-program-specific any more. One fewer deploy, band, frame link, program id in every manifest and web table. Rides **cohort-16**, whose Core digest already moves (MCC C-07). Band 0x2 withdrawn, never reused (decision 0007). The successor's `rent-credit` subcommand becomes a `core` one |
| `custody` | `dclutch-custody-sbf` | **stays** | collateral namespace owner (decision 0008) |
| `resolution` | `dclutch-resolution-proof-sbf` | **stays** | the funded ladder (0027) and the ensemble's fragment routes (cohort-17) both live here |
| `claims` | `dclutch-claims-sbf` | **stays** | the economic owner; founder bond (0033) lands here |
| `trading` | `dclutch-trading-sbf` | **stays** | the interpreter waist and every capability family |
| `general-accelerator` | deployed beside the seven in cohorts 14 and 15 with a Registry pin | **becomes `dclutch-accelerator-sbf`**, the ONE readonly stateless evaluator | see below |
| `dealer-accelerator` | `SHIPPED_LINKS: true` in the successor's `upgrade.rs:284` yet `blocked.json` says *out-of-release-set: no tier deploys it*; never on a chain; C-06's 31/31 was witnessed through Trading's checkpoint chain (`dealer-checkpoint-programtest`), not through it; its only consumer is its own 12,062-line program-test | **folded into the one accelerator** as a family arm | cohort-17's scoring Dealer replaces its selector-9 evaluation (131,790 CU) with a 39k–93k CU participation check *inside the General batch* (`MECHANISM_SCORING_DEALER` §2, §6) — the standalone evaluator has no future either way |
| `series-shadow` | out-of-release-set, both routes blocked; 2,034 src lines behind `dclutch-shadow-accelerator-auth-v4` (shared with Trading) | **folded into the one accelerator** as the Series arm | decision 0029 item 1 keeps the Series family (BUILD A) and names *"dispatch and shadow derivation"* as what it still owes; the shadow is an evaluator with the accelerator's exact contract. It does not need its own program id to be that |
| `direct-aot` | `SHIPPED_LINKS: false`; 392 lines; its campaign is its own program-test of a Direct **V2** descriptor the live Direct route (`hot_v3` inline) no longer uses | **deleted**, with `dclutch-direct-aot-contract`, `dclutch-direct-aot-v3-contract`, `tools/gauntlet/aot-cu`, `tools/direct-translation-validator` | a measurement lineage of an unshipped program. Band 0xA withdrawn |
| `product-runtime-v2` | `SHIPPED_LINKS: false`; 345 lines; one route, blocked out-of-release-set; its own header says Core and Claims *repeat these checks* through `dclutch-product-runtime-v2-svm-reader` | **deleted** | an adapter whose consumers already re-derive what it persists. Band 0x9 withdrawn |

**The one accelerator.** Three programs today share one contract — literally:
`AcceleratorRequestV2`, `AdmittedAcceleratorRequestV2` and
`AcceleratorOutputPageRequestV3` live in
`crates/dclutch-execution-strategy-contract/src/v2.rs`, and all three (plus
`direct-aot`) depend on that crate and on `dclutch-sbf-bump-heap`. Their
`lib.rs` headers describe the same machine in almost the same words: *readonly,
stateless, admitted-AOT; receives the canonical admitted frame from Trading,
rebuilds the authenticated input bank from Trading-owned scratch pages,
evaluates one transition, returns exactly one typed candidate chunk; never
writes, never CPIs, owns no state.* The admitted frame already carries the
family; the accelerator dispatches on it. One program id, one band (the
general accelerator's 0xC; 0xB and 0xD withdrawn), one frame link, one
program-test harness in place of three (12,062 + 7,743 + 417 lines), one
Registry deployment pin instead of three. This is a **cohort-16 or cohort-17
change**, not a convergence change: it moves an ELF and its band, so it lands
the way every program change lands (§2.2), and it lands *before* the scoring
Dealer needs the Dealer arm to be a batch participant.

**Series exists.** Not as a program (above) and not as a fifth generation: the
`series/` module in Trading carries `artifacts_v3` → `artifacts_v4` →
`release_v4` → `release_v5` → `activation_bundle_v1` as a chain of wrappers
(`release_v5` is referenced only by `activation_bundle_v1`; `release_v4` only
by `release_v5` and `lifecycle_policy_v5`; `artifacts_v3` by eleven siblings).
The family that cohort-16 deploys is the one whose digest moved in
`2cf96117a`; the generation names are history and become one module per
concept (`artifacts`, `release`, `funding`) when the Trading maker flattens
them — a rename with a byte-identical ELF as the control (§2.1).

### 1.2 Crates: ~18, grouped by authority, never by generation

The rule: **a crate is one authority at one layer.** The layers are `no_std`
kernel/contract (compiled into an ELF), host operator (constructs unsigned
transactions), and browser boundary (a `cdylib`). Each authority gets at most
one crate per layer; a generation suffix in a crate name (`-v1-`, `-v2-`,
`-v3-`, `-v4`) names a merge, not a crate. Today **30 of 94** carry one.

| target crate | layer | absorbs (today's crates) |
| --- | --- | --- |
| `dclutch-registry` | no_std | `registry-contract`, `registry-svm`, `registry-activation-auth-v1`, `record-contract`, `release-set-contract` |
| `dclutch-market` | no_std | `core-contract`, `market-core-codec`, `capability-contract`, `capability-program-contract`, `capability-seal-contract`, `capability-activation-codec`, `realm-contract`, `rent-contract`, `protocol-parameters-contract` |
| `dclutch-product` | no_std | `product-contract`, `product-compiler`, `product-runtime-v2`, `product-runtime-v2-admission`, `product-runtime-v2-svm-reader`, `product-payoff-v2-codec`, `liability-basis-v2-kernel`, `economic-kernel`, `economic-slice-kernel`, `resolution-policy-kernel` |
| `dclutch-custody` | no_std | `custody-contract`, `token-svm` |
| `dclutch-source` | no_std | `source-contract`, `resolution-codec`, `relay-contract`, `pyth-svm` |
| `dclutch-claims` | no_std | `claims-svm`, `claims-conservation-contract`, `fractional-claim-contract`, `fractional-claim-kernel`, `fractional-claims-kernel`, `rational-representation-v2-{contract,request-contract,kernel,lifecycle-contract}`, `representation-composition-v3-kernel`, `bearer-v2-contract`, `structured-v2-{contract,kernel}`, `user-position-admission-contract` |
| `dclutch-trading` | no_std | `direct-codec`, `direct-ticket`, `dealer-codec`, `dealer-scenario-kernel`, `general-codec`, `general-config-contract`, `general-adapter-contract`, `series-v3-kernel`, `shadow-accelerator-auth-v4`, `execution-strategy-contract` |
| `dclutch-vm` | no_std | the Lean-owned interpreter waist every family shares: `transition-vm`, `effect-kernel`, `account-profile-contract`, `request-profile-contract` |
| `dclutch-sbf-runtime` | no_std | `sbf-bump-heap`, `cu-checkpoint` |
| `dclutch-refusal-registry`, `dclutch-sha256-adapter` | no_std | stay as they are (one authority each, already) |
| `dclutch-operator` | host | `operator` + the sixteen other `*-operator` crates: `market-{founding,open,retirement}-v1-operator`, `resolution-core-v3-operator`, `provider-transport-v3-operator`, `source-readiness-operator`, `product-runtime-v2-operator`, `general-successor-operator`, `fractional-claim-operator`, `rational-representation-v2-operator`, `representation-composition-v3-operator`, `rational-lifecycle-hot-v3`, `bearer-v2-operator`, `structured-v2-operator`, `wallet-terminal-{input,payout}-operator`, `versioned-message-operator`, `hot-bump-miner-v1`, `fractional-cubic-life-evidence` |
| `dclutch-release-tool` | host | stays (it is the C-14 authority) |
| `dclutch-wasm` | browser | ONE `cdylib` with feature-gated exports in place of eight: `partition-quality-wasm`, `product-payoff-v2-wasm`, `rational-open-wasm`, `source-provider-wasm`, `source-readiness-wasm`, `user-position-admission-wasm`, `wallet-terminal-input-wasm`, `wallet-terminal-payout-wasm` — and one `.wasm` in the client tree instead of eight (3.9 MB of binaries today) |
| `dclutch-svm-harness` | test | stays; the real-ELF harness |
| program-test support | test | `chain-bundle-builder`, `direct-hot-program-test-support`, `program-test-evidence` — one crate |

That is 15 named crates plus test support: **~18**. The merges are mechanical
(`pub mod` per absorbed crate, `pub use` for the old paths during the cycle,
`cargo metadata --locked` as the control); the *deletions* inside them are
not, and §1.4 names those separately.

**Why the operator merge is safe and the wasm merge is the one that pays.**
Sixteen operator crates exist so that each `*-wasm` cdylib could depend on a
narrow slice without pulling `solana-sdk`. With one `dclutch-wasm` behind
feature flags the reason evaporates, and the operator can be one crate with
one `Cargo.toml` — today the successor, the CLI and the gauntlet each name a
different subset of them by path (84 `#[path = "../…"]` links in the tree).

### 1.3 Clients: one package

`packages/dclutch-sdk` (`@dclutch/sdk`) is the client. `apps/dclutch-web`
**imports it** — and already does: the `file:` dependency exists and 52 web
files import from it — while 129 files of `lib/` still carry a byte copy, 33
more drift, two near-identical `scripts/` directories of generators exist, and
22 of the 23 shared `generated/` modules are identical.
`tools/twins/classification.mjs` exists to *classify* the drift, which is the
tool a tree needs only while it has two copies. The absorption is half done;
finishing it deletes the 129, re-homes the 33 as SDK-owned, empties the
classification table and the `twinIdentity` test with it. `packages/dclutch-cli`
imports the SDK (decision 0029 item 8 already renames the binary). The web
keeps only what is browser-coupled: `walletStandard.ts`, components, pages,
the explorer.

The hand-written wire in the browser goes with the twin. `abi-coverage.baseline.json`
lists **26 magics and a dozen seed domains the browser states in its own
words** (`coreFound.ts`, `dealerEquityChain.ts`, `directHotChain.ts`,
`infrastructure.ts`, `releaseRegistry.ts`, …). Each becomes an import from a
generated module; the baseline goes to `[]` and the script becomes a gate that
refuses a non-empty list.

### 1.4 The dead generations — a census, not an opinion

A generation is dead when its **producer or its consumer is gone**. The tree
has four kinds, each with its own census control — and a fifth kind that looks
dead and is not.

**(a) Programs no cohort deploys** — `direct-aot`, `product-runtime-v2`
(`SHIPPED_LINKS: false`), and the two accelerators/shadows that are `true` in
`SHIPPED_LINKS` but *out-of-release-set* in `blocked.json` (the two files
disagree, which is itself a finding: one table, in the release tool, and
`blocked.json` derives from it). Control: `tools/cohort/cohorts/{14,15}.json`
name seven roles + `general_accelerator`, nothing else.

**(b) Older wire generations still decoded beside their successor.** Fourteen
magic families carry two digits. Read each before cutting — two of the three
checked here were not deletions:

| family | old / new | where the old one lives | disposition |
| --- | --- | --- | --- |
| `DCLTHOT` / `DCLTHOT2` / `DCLTHOT3` | Trading hot request | `hot_v3.rs:17327-17328` — **refused by name in a test, nothing else** | already the right end state; nothing to delete |
| `DCLFDC04`+`DCLFDR04` / `05` | Claims founding | `claims-svm/founding_v4.rs` (1,123 lines) — and **`founding_v5.rs:2591` writes the V4 request magic**: one wire, two module names | **merge** into one `founding` module; not a delete |
| `DCGREQ02` / `DCGREQ03` | **General's two 64-byte request generations** — seven actions validate one, eight the other (`GOAL.md` 2026-09-04 17:15) | `general-codec/successor_request_v2.rs`, `general-adapter-contract/artifacts_v3.rs` | **unify** (§4 item 3); rides cohort-16 |
| `DCGBAT01` / `02`, `DCGORD01` / `02` | General batch and order | `general-adapter-contract/collection_v1.rs:72-74` — the collection route's records, named in bindings and driven by the successor | **live**; a layer under the v2 records — merge |
| `DCLTRP02` / `03` | request profile V2 / V3 | `request-profile-contract/v2.rs:16` — V2 has eight crate consumers | **live**; merge |
| `DCLTAP02` / `03` | account profile | `account-profile-contract` (`v2.rs` 5,372 + `v2/encode.rs` 5,537 beside `lifecycle_v3.rs` 6,678 and `v3.rs`) — V2 has eight crate consumers | **merge**, not delete |
| `DCLRPFQ1`+`DCLRPFR1` / `2` | Resolution pre-market funding | **`resolution-proof-sbf/pre_market_funding_abort_v1.rs:559` writes the V1 magic** into the abort's legacy packet | **live** — the "legacy packet" thread (`GOAL.md` 09-01 21:30); a rename when that thread closes |
| `DCRLHT03` / `06`, `DCLTSIX1` / `3`, `DCLTDRB1` / `2`, `DCLPCL01` / `02` | one constant each, no test-only sites | Rational lifecycle hot; source instruction; the Dealer checkpoint's rollback; the projected-custody lock receipt | **live**: each pair is two *records* of one family (request/receipt, lock/rollback), not two generations |
| `DCLPPR01` / `02` | claims position | test sites only | nothing to delete |

**Read, the verdict is: the two-digit census finds STACKING, not death.**
Every one of the fourteen families is either refused-by-name (the right end
state), a live wire under a stale name, or a request/receipt pair. The
generation-deletion maker takes **nothing** from this table; the domain
makers take *merges* from it (§4 item 6). Death in this tree is found by the
consumer census — (a), (c), (d) — and by a builder with no caller, never by a
digit in a magic.

Control for a merge from this table: the old constant survives as a
`pub const` alias for one cycle, the executing arm is one function, and the
ELF is byte-identical (§2.2).

**(c) Crates with no non-test consumer.** From the tree-wide reverse-dep
census over all 55 workspaces (every `path = "…"` in every `Cargo.toml`):

| crate | consumers | disposition |
| --- | --- | --- |
| `dclutch-economic-kernel` (1,808) | none | *Bounded execution refinement for `DClutchSemantics.EconomicKernel`* — a Lean twin with no Rust reader. Delete, or make it the one reader of `EconomicKernel.lean` that Claims actually calls. Not both |
| `dclutch-resolution-policy-kernel` (895) | none | same class; the live policy lives in `source-contract` |
| `dclutch-liability-basis-v2-kernel` (7,798) | `product-payoff-v2-codec` **dev only** | decision 0029 item 2 KEEPS curvature. A **producer-missing reader**: named as debt in `dclutch-product`'s crate doc, not deleted, not hidden |
| `dclutch-structured-v2-operator` (4,997) | `claims-sbf` **dev only** | 0029 item 7 says K=3 Structured IS the product, yet no host driver constructs it. Producer-missing; named |
| `dclutch-fractional-cubic-life-evidence` (1,000) | `tools/fractional-exterior` only | an evidence bridge for one campaign; lives with that campaign |
| `dclutch-direct-aot-contract`, `dclutch-direct-aot-v3-contract` | the unshipped program and `aot-cu/twin-v3` | go with (a) |

**(d) Tools nothing runs.** No reference from `tools/ci`, `tools/gauntlet`,
`tools/cohort`, `tools/lane` or `.github`, with the citation census
(`rg -l tools/<name> docs`, `GOAL.md`, `WAVE.md`) beside each:

| tool | lines | cited in docs / GOAL+WAVE | disposition |
| --- | --- | --- | --- |
| `lineage-loopback` | 579 | 0 / 0 | delete — the cleanest case in the tree |
| `atomic-generate`, `sbf-footprint.py` | 132, — | 1 / 0 each; last touched 08-25 | delete; the emission guard and `sbf-frame-sizes.py` own their jobs |
| `activity-properties`, `lamport-ledger`, `pyth-sponsored-push-audit`, `economic-lifecycle-ledger` | 1.4k, 2.7k, 1.1k, 2.8k | 2–3 / 0; last touched 08-28..31 | their outputs are in dated evidence docs; the evidence stays, the tool goes — or the tool becomes a `dclutch-gate` subcommand if a cohort runbook row still calls it |
| `devnet-activity`, `devnet-scenarios` | 12.2k, 12.5k | 5 / 0 | fold into the host driver's `observe` — they are the successor's reads written a second time |
| `branch-census`, `doc-citations`, `ticket-board` | 0.3k, 0.5k, 2.2k | 1 / 1 each | `dclutch-gate` subcommands or deleted; `ticket-board` is a Direct product surface and goes to the host driver |
| `tools/release`: 22 scripts with **zero** references (`lifecycle-chaos` 2,733, `private_validator_upgrade` 1,501, `devnet-flight` 450, `devnet-recycle.sh`, `devnet-observe.sh`, …) | ≈7k | — | the same rule; `checked-release-candidate.sh` (10 refs), `artifact_provenance.py`, `compose-mixed-gate.py`, `check-all-workspaces.py` are the live release gate and become `dclutch-gate release` |

`relayer`, `load-simulator`, `cut.sh`, `dclutch-cli` have no CI reference and
are **live** (a product component, ember's standing deliverable, the
publication path, the operator door) — absence of a runner is not the test;
absence of a *consumer* is.

**(e) What looks dead and is NOT a deletion.** These are producer-missing
readers with a ruling or a C-row behind them; the generation-deletion maker
leaves them, and their crate doc names the campaign they wait for:

- the registered-Direct V4 artifacts — `crates/dclutch-direct-codec/src/registered_*_v4.rs`,
  **7,999 lines** whose only gate is `hot_v3.rs:5372` refusing every Direct
  kind but `InlineOrdinary`; MCC says *"the gate must become a DISPATCH, not a
  deletion"* and C-04 wants every one of them. Only ember's batch-spine ruling
  deletes them, and it is not made;
- the Dealer equity builder (the browser's `compileDealerEquityTransactionV3`
  is its only submit path — deleting the Rust makes the mirror the last
  authority, PARSIMONY 09-03);
- the three Series Hot builders (`build_series_{prepare,consume,expire}_hot_v3`,
  no caller, waiting on the dispatched Shadow callback — C-07 blocker (c));
- `liability-basis-v2-kernel` and `structured-v2-operator` (above).

### 1.5 Tools: one gate, one host driver, and the two products

| target | today | rule |
| --- | --- | --- |
| **`dclutch-gate`** — one Rust CLI, one workspace | `gauntlet/{census,journey,ladder,relayed-vertical,…}` (6 workspaces), `frameguard` (Python), `emission-guard` (Python), `genref` (mjs), `seam-audit` (Python), `twins` (mjs), `doc-commands`, `doc-citations`, `ci/*.py`, `cohort`+`cohort14`+`cohort15` (three runbooks), the live quarter of `release/` (34k), `sbom`, `sbf-frame-sizes.py` | subcommands `census`, `frames`, `emission`, `genref`, `witnesses`, `cohort <manifest>`, `release`, `seams`, `ci`. One language where the gate reads Rust (census, frames) and the same one everywhere else — five languages of gate today is five ways to have a `set -e` that stops at the first row |
| **the host driver** — the successor | `tools/local-validator/bootstrap/successor` (184k, 74 files: `market.rs` 18,597, `upgrade.rs` 15,767, `terminal_sequence.rs` 13,327; seven role subcommands `registry core claims trading resolution custody rent-credit`), `tools/release/{private-validator-lifecycle,devnet_direct_lifecycle,successor_campaign_pack}.py`, `devnet-reconcile`, `devnet-scenarios`, `devnet-activity` | one binary; every family a module that calls `dclutch-operator` and **spells no wire byte itself** — 423 hand-written magic literals in 32 files today |
| the user door — `@dclutch/cli` | `tools/dclutch-cli` (Rust, 4.3k; commands `market`, `capability`, `fractional-retirement-next`, `general`, `ticket` — user acts, not operator ones) beside `packages/dclutch-cli` (TS, 14.6k); decision 0029 item 8 already says two binaries named `dclutch` are lethal | one CLI over the SDK; the Rust one's user-facing commands move there, its operator-facing ones (`general` session driving) to the host driver; `ticket-board` goes with `ticket` |
| `relayer` | 14k Rust | stays; a product component (the keeper) |
| `load-simulator` | 11.6k | stays; part of every devnet deliverable by ember's grant |
| `lane.sh`, `cut.sh` | | stay |

### 1.6 Formal: one library, one emitter per record

`formal/dclutch-semantics` is already one library with every module reachable
from the root (143/143). What is not one is the **emitter**: 105 `Emit*.lean`
for 100 generated files, five records with a Rust *and* a TypeScript emitter
written separately (`RefusalBandsV1`, `RealmPositionAbi`,
`RationalTerminalHotV3`, `DirectProgram`, `CapabilityManifestV1Abi`), 24
hand-written `check-generated.sh` scripts beside the cargo-test and
`package.json` guards, and a second generator technology — 25 `generate-*.mjs`
scripts in the web (27 in the SDK) that read Rust source with regexes — beside
the Lean one. Target: one ABI module per record, one `Emit<Record>.lean` that
prints both targets through the shared `TsEmit`/Rust printers, `lean-emit.mjs`
as the only script, and `dclutch-gate emission` as the only guard. The three
`qedsvm-*` dirs are captured proof artifacts (`.pcs`, a lifted `.lean`, a
README) and belong under `docs/evidence/`, not beside the library.

### 1.7 What must survive — the cohort-17 list

From the six notes' *reused as-is* sections and §3.2 of the batch spine:

- `GeneralClearing.lean`, `GeneralTransitionV3.lean`, `GeneralConfigV3Abi.lean`,
  `GeneralControllerAbi.lean` — every mechanism keys on them.
- `crates/dclutch-general-adapter-contract/src/{runtime_verify,runtime_settlement,escrow_v1}.rs`
  and `programs/dclutch-general-accelerator-sbf/src/lib.rs` — joint clearing's
  four new `RuntimeVerifyErrorV2` codes land here.
- `crates/dclutch-claims-svm/src/{founding_v5,claim_check_conservation_v1,product_basis_terminal_v3}.rs`
  — the founder bond's compartment.
- `crates/dclutch-product-compiler`, `crates/dclutch-source-contract`,
  `programs/dclutch-core-sbf/src/found.rs`, Resolution's derived provider —
  conditional markets.
- `DealerLiquidity.lean`, `DealerLiquidityAbi.lean`, `DealerTradingProfile.lean`
  — the LP machine survives every ruling; only the scenario checkpoint chain
  (`DealerScenario*.lean`, `dealer_scenario_checkpoint_v1`) is conditional on
  ember's batch-spine ruling, which is **not** made.
- `Series*.lean` unchanged under every note.
- `tools/gauntlet/journey/src/ledger.rs:1004-1012` — the census laws L1–L8;
  every note says which law it moves and none asks for a tenth compartment.

A merge may rename any of these; a deletion of any of them is out of scope for
this swarm.

## 2. The rules the domain makers should have followed

The convergence lane enforces these on every branch it merges. They are the
parsimony attractor (`GOAL.md` closeouts of 09-01 and 09-03) made checkable.

### 2.1 A deletion shows its census control

Every deleted file, route, crate, magic or tool comes with, in the commit
message: (i) the reverse-dependency read that found zero consumers — for a
crate, every `Cargo.toml` in all 55 workspaces; for a magic, `rg` over the
seven trees named in §1.4(b); for a route, `docs/reference/route-witnesses.md`
and every `bindings.json`; (ii) the **count that did not change** — routes
164, refusal codes 357 minus exactly the deleted band's, frameguard links 12
or 12 minus the deleted ELF's, `emission_guard.py --verify` green; (iii) for
a program: the band withdrawn in `RefusalBandsV1.lean` (never reused), the
`SHIPPED_LINKS` row, `blocked.json`'s rule, the web deployment table, the
cohort manifest schema — **the same commit** (`AGENTS.md`: banishing is not
finished at the Rust boundary).

A deletion the reader cannot reproduce from the message is reverted at merge.
A deletion of anything in §1.4(e) or §1.7 is reverted at merge regardless.

### 2.2 A rewrite preserves the ratchet, the records, and the Lean-first order

**Which changes move a deployed program's bytes.** Every crate compiled into
an SBF link — the link's whole path-dependency closure, not its program crate.
The twelve links are the twelve program ELFs, 1,973 frames; touching
`dclutch-vm`'s constituents (`transition-vm`, `effect-kernel`,
`account-profile-contract`, `request-profile-contract`) moves Trading, Claims,
Core, Custody and both accelerators at once. A merge of crates that changes
only `Cargo.toml` paths and `pub use` lines produces a **byte-identical ELF**,
and that identity is the control: `sha256` of each ELF before and after,
printed in the message.

**What cohort-16 needs.** Devnet is disposable (decision 0012, ember's standing
grant): every cohort is a full redeploy from a commit, so no *program id* is
sacred. What is sacred is (i) the **frame ratchet** — every link's
`tools/frameguard/baseline.json` rows captured in the same commit, or the
message says it leaves the ratchet red and names who owes; (ii) the **persisted
record layouts** a market carries — `MarketCore`, `CapabilityManifestV1`,
`FundingLedgerV2`, the Series `TemplateV3`, `SourceResolutionStateV2`,
`RecoveryPolicyV2`, the General selection and order records — which move
**Lean-first** (the ABI module, its emitter, the generated Rust *and* TS, the
web decoder) or not at all; a layout that moved on one side is the failure
mode the emission guard exists for; (iii) the **CU budgets**: a row in
`CU_BUDGETS.json` whose campaign the rewrite touched is re-drawn, not carried.

**What cohort-17 may move.** Everything in §1.7 — but by the mechanism lanes,
after this swarm, under decisions 0032–0034. A simplification branch that
"tidies" a General order record or a Dealer profile is doing cohort-17's work
without its Lean and is refused.

### 2.3 A merge carries three things

1. **The frame baseline.** A merged crate's link owes rows exactly as a
   rewritten one does; `frameguard.py owed` names the debtor, and the Lane
   trailer is what makes that name mean something (`DCLUTCH_LANE` exported).
2. **The emission guards.** Every generated file the merge moves keeps a
   guard that re-runs its emitter and compares (`emission_guard.py --verify`
   at 100/100 today; a merge that lands 99/100 is refused). Generated files
   are moved by moving the emitter's output path, never by hand.
3. **The twin classification.** Until the web imports the SDK for everything,
   every web file touched is re-classified in `tools/twins/classification.mjs`
   or the `twinIdentity` test goes red. The clients maker's absorption commit
   is what deletes both; every other maker's commit must keep them green.

### 2.4 The two rules that are behaviour, not code

- **One author per fact.** A maker who finds a fact spelled twice (a width, a
  magic, a seed domain, a count) does not fix the second spelling; it deletes
  it and imports the first. The five Series sites of `2cf96117a` are the model:
  private fields, one constructor, the derived value unspellable elsewhere.
- **A producer-missing reader is debt named, not hidden.** A reader with no
  producer (§1.4(e)) is not deleted to make the census clean; it is named in
  its crate doc with the campaign that would produce it. Deleting it would make
  the mirror the last authority (`GOAL.md` PARSIMONY 09-03).

### 2.5 Per-maker scope, and the files two makers will both touch

What each branch is expected to contain — a branch that reaches outside its
column is coordinating with the column it reached into, or is rebased out of it.

| maker | expected scope | must not touch |
| --- | --- | --- |
| Trading program | `programs/dclutch-trading-sbf/src/**` (flatten `series/` v3→v5 and `dealer/` v3/v4, split `hot_v3.rs`), its program-tests | `crates/**` layouts; the accelerators' band |
| other programs | `programs/{core,claims,custody,resolution,registry,rent}-sbf`, the accelerator fold (§1.1) if it lands now | Trading's `hot_v3.rs` |
| kernel/contract crates | the no_std merges of §1.2, `Cargo.toml` at the root | ELF bytes (byte-identical control) |
| operator crates | the host merge into `dclutch-operator`, `dclutch-wasm` | wire layouts |
| Lean | one emitter per record, `TsEmit`/Rust printers, `qedsvm-*` → evidence | any `*Abi.lean` field (that is cohort-17's) |
| gate tools | `dclutch-gate`, `tools/{gauntlet,frameguard,emission-guard,genref,seam-audit,twins,ci,doc-*}` | `blocked.json` semantics without the programs maker |
| successor / release / cohort | the host driver, `tools/release`, `tools/cohort*`, `devnet-*` | `SHIPPED_LINKS` without the programs maker |
| clients | `apps/dclutch-web`, `packages/*` | `lib/generated/**` by hand (regenerated) |
| docs | `docs/**`, `GOAL.md`, `WAVE.md`, `README.md`, `ARCHITECTURE.md` | `docs/reference/**` (generated) |
| generation deletion | §1.4 (a)–(d) tree-wide | §1.4(e), §1.7 |

The files more than one column will edit, and who owns the merge of each:
`programs/*/src/lib.rs` `mod` lists (programs ← deletion); the root
`Cargo.toml`/`Cargo.lock` (crates ← everyone; resolved by regenerating, never
by hand-merge); `crates/dclutch-refusal-registry/src/generated_bands.rs` and
`RefusalBandsV1.lean` (Lean ← programs ← deletion; re-emitted, not merged);
`tools/gauntlet/blocked.json` (gate ← programs); `tools/frameguard/baseline.json`
(nobody — recaptured once, §3.4); `tools/local-validator/bootstrap/successor/src/upgrade.rs`
`SHIPPED_LINKS` (successor ← programs); `apps/dclutch-web/lib/generated/**`
and `packages/dclutch-sdk/lib/generated/**` (re-emitted); `docs/reference/**`
(re-generated); `docs/decisions/README.md` index (docs).

## 3. The order of convergence

**The tree-wide generation deletion merges first**, then the in-domain
rewrites, then the merges of crates, then the clients — because each step
shrinks the surface the next one has to reconcile:

1. **Generation deletion (tree-wide).** Deletes are the cheapest merges: a
   file absent on both sides has no conflict. Merging it first means every
   in-domain rewrite is rebased onto a tree where the unshipped programs, the
   dead crates, the unrun tools and the old executing arms are already gone,
   and the rewrite's own diff is against the live generation only. Expected
   conflicts: `lib.rs` `mod` lists in Trading, Claims and `claims-svm`;
   `RefusalBandsV1.lean`; `blocked.json`; the web deployment table.
2. **Program makers** (Trading; the other programs) rebased on 1. These move
   ELF bytes, so their frame rows are recaptured *once* at the end (§3.4),
   not per branch — three correct recaptures were each invalidated within
   minutes on 09-02 (`AGENTS.md`). Expected conflicts: `hot_v3.rs` (20,276
   lines, 347 functions — any two branches touching it conflict), the
   `series/` and `dealer/` module lists, `entrypoint_adapter.rs`.
3. **Crate makers** (kernel/contract; operator; Lean) rebased on 2. The crate
   merges of §1.2 change every `Cargo.toml` in the tree, so they go after the
   program branches whose manifests they rewrite. Expected conflicts: every
   `Cargo.toml`, `Cargo.lock` (71 of them — this is where the 55-workspace
   count is also cut, §4 item 7), `#[path]` links (84), emitter output paths.
4. **Gate and successor makers** rebased on 3, since they read the crate paths.
5. **Clients** last: the SDK absorption touches 129 files whose *content* no
   other maker changes but whose generated modules every Lean and crate change
   re-emits.
6. **Docs** in parallel with all of the above and merged last; the only shared
   files are `docs/reference/*` (regenerated, never hand-merged) and the
   decision index.

### 3.4 The single build-and-gate pass

Run once, on the converged tree, at a commit, in this order — each stage
the control of the one before:

1. `cargo metadata --locked` in every remaining workspace (the manifest gate).
2. `cargo check` for the root workspace; `cargo build-sbf` for the seven
   programs; **`sha256` of each ELF against the pre-swarm ELFs** — every link
   that is byte-identical needs no frame rows, and every link that moved gets
   `tools/frameguard/run.sh --at <commit>` exactly once.
3. `emission_guard.py --verify` then `tools/ci/run.sh emission` (the real
   guards, 86–195 s).
4. `tools/genref/generate.sh --converge` from a detached worktree at HEAD;
   `--check` must be a fixpoint in ≤ 3 passes.
5. `dclutch-route-census inventory --check-unique`; route count and refusal
   count printed and compared to the ledger of deletions (§2.1 ii).
6. `tools/gauntlet/run.sh` tier 1 (19m33s measured 09-03) and the family
   tiers whose routes any branch touched — filtered, never the whole suite.
7. `npm test` in the SDK and the web (`twinIdentity`, `abi:*:verify`, the
   liveness gate) — after the clients merge, the twin test is gone and the
   coverage baseline is `[]`.
8. `tools/cut.sh` dry run: the published tree equals HEAD.

Only then a cohort-16 manifest is written from the converged commit.

## 4. The ten things that make the system WAY better, not only smaller

Each is a class of defect this tree has paid for more than once, and the one
change that removes the class rather than the instance.

1. **The browser keeps hand-written twins of the wire.** 26 magics, a dozen
   seed domains, the Dealer equity envelope (`compileDealerEquityTransactionV3`
   — the *only* submit path for a route whose Rust builder is therefore
   producer-missing), 374 literals across 86 files, and a 129-file byte-copy of
   the SDK beside them. *The change:* the web imports the SDK for everything
   and imports every wire constant from a Lean-emitted module; `abi-coverage`
   refuses a non-empty list. The class removed: a deletion on the Rust side
   that leaves the browser as the last authority with nothing going red.
2. **Coarse refusals.** `TradingSbfError::Content` has 1,818 raise sites;
   `SubmitCandidate` hid 45 clauses behind one code and the first inference
   from it was wrong; `map_err(|_| Coarse)` was the most expensive idiom in the
   tree three times in one day. *The change:* a census gate — no `#[repr(u32)]`
   variant may have more than *N* raise sites unless it is a `From<Inner>`
   wrapper that keeps the cause; `clippy-census.py` reds `map_err(|_|` on an
   `*Error` return. The class removed: a located defect converted into a search.
3. **General ships two 64-byte request generations** (`DCGREQ02` for seven
   actions, `DCGREQ03` for eight) and each action's profile revalidates exactly
   one; seven actions were nearly built in the wrong wire. *The change:* one
   request record with the action kind in it, fifteen profiles over one
   decoder, the old magic decoded only to be refused by name. Rides cohort-16.
4. **The successor is a second interpreter.** `market.rs` alone is 18,597
   lines; the driver spells 423 wire literals itself and rebuilds frames the
   operator already builds. *The change:* the successor calls
   `dclutch-operator` for every instruction and asserts nothing about bytes it
   did not derive; a literal in `tools/` is a census red. The class removed:
   the harness agreeing with itself rather than with the chain (the refusal
   registry's own words).
5. **One fact, several authors.** The Series config identity had six sites and
   five of them agreed with each other and not with the Registry (SERIES-4);
   `REQUEST_BYTES` is declared five times with four values (WITNESS-3);
   `SHIPPED_LINKS` and `blocked.json` disagree about the Dealer accelerator
   today. *The change:* derived values are unspellable — private fields, one
   constructor, `const` re-exports; and one table for what ships, in the
   release tool, from which `blocked.json`, the cohort schema and the web
   deployment table are generated.
6. **Generations stacked, not replaced.** `release_v4` under `release_v5`,
   `artifacts_v3` under `artifacts_v4`, `founding_v4`'s magic written by
   `founding_v5`, account-profile `v2` beside `lifecycle_v3` beside `v3`,
   `dealer/v3_*` under `v4_*`, fourteen two-digit magic families, thirty
   generation-named crates. Each generation is a crate name, a Lean module, an
   emitter, a band offset and a test workspace. *The change:* the rule of §1.2
   — one module per concept, the newest generation takes the name, the older
   is deleted with its census or refused by name. The class removed: a change
   that must be made in three generations to land in one.
7. **55 workspaces, 71 lockfiles, 272k lines of lock.** Four lockfiles could
   not resolve under `--locked` at committed HEAD; the successor did not
   compile at HEAD for a day because nothing builds that workspace between
   program commits; a `CARGO_TARGET_DIR` override linked one crate twice.
   *The change:* one host workspace (programs, crates, program-tests, the
   gate, the successor) and `cargo metadata --locked` as a CI gate over
   exactly one lockfile; the SBF builds already come from the root workspace.
   The class removed: a commit that compiles for whoever holds the dirty file
   and for nobody else — and a sixth of the tree.
8. **Five gate languages and three cohort runbooks.** Bash (`run.sh` 834
   lines), Python (`frameguard`, `emission-guard`, `seam-audit`, `release/*`),
   mjs (`genref`, `twins`, 25 generators), Rust (`census`, `journey`) and TSV
   (`steps.tsv` × 3). A `set -e` runner reported one failure when the true
   figure was ten; a `timeout … | head` reported head's exit. *The change:*
   `dclutch-gate`, one binary that runs every row and reports every row, with
   `failed` distinct from `never ran`; one cohort runbook parameterized by the
   manifest (the 09-03 attractor's item 3, half-built as `tools/cohort`).
9. **The accelerator is three programs with one contract.** Three bands, three
   program-tests, three deployment pins, a shadow that has never been on a
   chain and a Dealer evaluator whose replacement is already designed. *The
   change:* one accelerator with a family arm; the scoring Dealer becomes a
   participant of the batch it already sits beside.
10. **The ledger is written four times.** Each finding exists as commit →
    lane report → `WAVE.md` entry → coordinator reply; `GOAL.md` is 4,992 lines
    doing three jobs; `docs/evidence` is 104 dated files whose facts are also
    in job directories. *The change:* `GOAL.md` an index of dated deltas;
    `WAVE.md` frozen at its last line; evidence generated from the job dir's
    machine-readable witnesses with prose only for findings; `docs/reference`
    the only register. The class removed: forty-seven stale claims found by one
    rehearsal.

## 5. The counts, before and after

| | today | target |
| --- | --- | --- |
| programs | 12 | **7** (registry, core+rent, custody, resolution, claims, trading, accelerator) |
| crates | 94 | **~18** |
| client packages | 2 + a web `lib/` that is a third package in all but name | **1** (`@dclutch/sdk`; the web and the CLI import it) |
| tools | 37 entries, 55 workspaces, 5 languages | **6** (`dclutch-gate`, the host driver, `relayer`, `load-simulator`, `lane.sh`, `cut.sh`), 1 workspace |
| Lean | 1 library, 105 emitters, 2 generator technologies, 3 guard kinds | 1 library, one emitter per record, 1 technology, 1 guard |
| lines | 1.56 M | ≈ 0.5–0.6 M |

**The deletion ledger — what needs no rewrite at all** (≈ 0.44 M lines):

| what | lines | needs |
| --- | --- | --- |
| 70 of 71 `Cargo.lock` | ≈ 268k | one workspace (§4 item 7) |
| eight checked-in `.wasm` binaries | 81k as `wc` counts them (3.9 MB) | one `dclutch-wasm`, built not committed |
| the web's byte-copy of the SDK and its drifted twins | 39k | the absorption (§1.3) |
| unshipped programs and their tests, the AOT lineage | ≈ 20k | §1.4(a) |
| tools nothing runs, `tools/release`'s unreferenced scripts | ≈ 30k | §1.4(d) |
| dead crates, old executing arms of two-digit magic families | ≈ 5–15k | §1.4(b)(c), read first |

The rest of the way — from ≈ 1.1 M to ≈ 0.55 M — is the rewrites: the
successor as a caller of the operator (184k → a fraction), the Trading
generations flattened (103k src), the crate merges shedding their per-crate
scaffolding, the docs ledger becoming an index.

## History

- 2026-09-04, first cut at `330bbfaba` (`d706c08b5`): written in the first
  hour from the censuses named in §0.
- 2026-09-04, deepened: the frameguard baseline is the twelve ELFs (1,973
  frames), not "eleven plus one"; `DCLTHOT`/`DCLTHOT2` are refused by name
  only and need nothing; `founding_v5` writes the V4 request magic, so
  `founding_v4` is a merge, not a delete; the registered-Direct V4 artifacts
  (7,999 lines) are producer-missing under an open C-04 wall, not dead —
  §1.4(e) added so the deletion maker leaves them; the lockfile count
  (272,509 lines) and the release-script census added; §2.5 per-maker scope
  and shared-file ownership added.
- 2026-09-04, third pass: the ten remaining two-digit magic families read —
  all live (the Resolution abort route writes `DCLRPFQ1`; `DCGBAT01` is the
  collection route the successor drives; the rest are request/receipt pairs),
  so §1.4(b) now says the digit census finds stacking, not death, and the
  deletion maker takes nothing from it. All 71 lockfiles and all 8 `.wasm`
  are tracked (`git ls-files`), so the ledger's two largest rows are real.
  `hot_v3.rs` splits along its phases (35 `require_*`, 31 `authenticate_*`,
  13 `commit_*`, 7 `execute_*`), not its families. `tools/dclutch-cli`'s five
  commands are user acts and go to `@dclutch/cli`, not the host driver.
