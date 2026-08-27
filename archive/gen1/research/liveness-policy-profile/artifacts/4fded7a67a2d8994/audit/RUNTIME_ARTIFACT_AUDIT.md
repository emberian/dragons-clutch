# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `d77d670922930273b09f015ed0eb1f46ad066102`. This is local build, static
artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary — in place, per the build-path protocol amendment

- **Canonical build location:** `/Users/ember/dev/dragons-clutch` itself.
  Per `docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md` the ELF identity is
  same-path-reproducible only (Cargo folds the absolute workspace path into
  every path-dependency's `-C metadata`, and hash-sorted symbol ties order by
  those hashes), so the canonical identity is defined at the canonical
  checkout path, where every bank log's fixture binds. Before the build the
  tree was verified exactly HEAD: `git status --porcelain` empty over the
  declared closure and `git diff HEAD` empty. No detached worktree is used for
  the canonical build; the one cross-path build below is the relocation probe.
- Exact Git archive: 27,351,040 bytes, SHA-256
  `baabb0025acda92cd6c3489682eebcb03c47452c5f2e331a8dbab9341e6b2b4b`
- Declared SBF closure: **109 files**, SHA-256
  `410893b484fd65820f95e6bf0ac1c91b85f6746638fc59666b0ce604c39b9d56`
- `d77d670` is `966ee2c` (the TerminalClosure merge for the general clearing
  plane) plus exactly one closure-neutral commit: a `GOAL.md` log commit
  (`d77d670`, `GOAL.md` only, outside the declared closure). Nothing in the SBF
  closure differs from the merge.
- The closure grows 108 → 109 files against the `e8ba31d5…` seal's
  declaration. The one addition is exactly
  `programs/clutch-sbf/program/src/instructions/orders_batch/terminal_closure.rs`.
  The declared `source_paths` themselves are unchanged — no path entered or
  left the closure declaration.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Pass 1/pass 2 stripped ELF, both built in place with fresh
  `CARGO_TARGET_DIR`s: byte-identical SHA-256
  `4fded7a67a2d8994f4dc2b82c533b978d14d6107f28de7cbbe7674ecdcedf6cb`,
  1,979,512 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `37407c582be162b3ec7c79680cac8fcfc183609cc48434499ca5d8c6524856e0`,
  2,158,840 bytes

### The two relocation probes

**Cross-path build probe: disposition `PATH_TIED_SYMBOL_ORDER`, observed
digest list — the equality claim is retired.** One build of the same commit in
a detached worktree at
`/Users/ember/jobs/dragons-clutch-r1-d77d670-xpath-worktree` (fresh target
directory, same recipe) produced stripped SHA-256
`d33bab44e679de5109c078aff2e504df6924a5cc32693f4f426c5321519317a0` — same
1,979,512 bytes, **different bytes**. The divergence is exactly the tied-pair
signature the root-cause note predicts and nothing else: **5 `.text` bytes at 4
sites** (`+0x624` `0x68↔0x80`, `+0xa2c` `0x80↔0x68`, `+0xa22bc`, and two
adjacent bytes at `+0xa2dfc`), with `.rodata`, `.rel.dyn`, `.data.rel.ro`,
`.dynstr`, `.dynsym`, `.dynamic`, and `.shstrtab` all byte-identical. The
unstripped cross-path ELF also differs
(`3268993a0bfa943828a1c83c8bf45a79d7fc5f50fef3dbe22ec4ac322a9b8ffc`).

  The `e8ba31d5…` seal recorded a cross-path build that happened to come back
  byte-identical and read that as a property of the artifact. **It was a
  one-sample coincidence, and it does not generalize.** The V3 measurement
  campaign at that same source observed two further distinct digests
  (`7fc8ba9f…` and `47c011d2…`) at two further paths, each differing from the
  seal in the same 486 `.text` bytes and 6 `.rel.dyn` bytes; this seal observes
  a third distinct path-digest at its own source. The evidence convention is
  therefore the **observed-digest list** — `artifact_reproducibility
  .cross_path_builds`, an array of `{path, sha256, bytes}` — and
  `policy.py::check_artifact_binding` now *refuses* both the old scalar
  `cross_path_build` field and any list entry equal to the canonical digest,
  so a future coincidence cannot be re-read as path independence.
  `docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md` governs.

**Relocated-Cargo-home probe: `PATH_SENSITIVE` — the `e8ba31d5…` seal's
`INDEPENDENT` finding does not survive this artifact.** The relocated-home
build produced
`6302d3ee9c98d87550361502be2620103f9edfd8b3517f0fb33c9f488c498ae8` at
**1,980,064 bytes**, 552 bytes larger, with 35,405 differing byte positions
over the common prefix. The mechanism is measured, not guessed: `.rodata`
grows by exactly those 552 bytes and contains exactly **three** absolute
registry paths under the relocated home that the canonical build does not
contain at all —
`…/registry/src/index.crates.io-1949cf8c6b5b557f/solana-address-2.6.1/src/syscalls.rs`,
`…/solana-program-entrypoint-3.1.1/src/lib.rs`, and
`…/solana-account-info-3.1.1/src/lib.rs`. These are `core::panic::Location`
strings; under `$HOME/.cargo` they render relative (the canonical `.rodata`
holds `src/syscalls.rs` and no host-local absolute path at all — its only three
absolute strings are the platform-tools **build machine's** `/Users/runner/…`
std paths, baked into the pinned Rust distribution). `.text`, `.rel.dyn`,
`.data.rel.ro`, and `.dynamic` all shift with the `.rodata` growth. This is
exactly the sensitivity the `d6929549…` seal recorded and the `e8ba31d5…` seal
believed superseded; on this evidence the supersession was the anomaly, and the
narrower claim is restored. Single-host observation, not a cross-host claim.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was unused. No
network, RPC, signing, deployment, submission, or external-state mutation
occurred.

## Final-LTO and direct-frame gate

- 36 backend diagnostic lines naming 28 unique symbols — `backend-stack-
  diagnostics.txt` is **byte-identical** to the `e8ba31d5…` seal's.
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostics by lines/symbols: `clutch_batch` 11/8,
  `clutch_batch_policy_identity` 18/13, `clutch_solana_layout` 2/2, and
  `clutch_solana_reference` 5/5. Every one is nonresident.
- 999 resident text symbols at 997 addresses; all 997 addresses were
  disassembled.
- 60,135 direct `r10` references; maximum offset 4,096; zero invalid positive,
  zero, or greater-than-4,096 references. The deepest direct reference sits in
  `claim_truth::observe_outcome_mints`, unchanged.
- 905 first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `first-party-frame-audit.txt`
  (retained evidence).

**TerminalClosure handler frame verification.** The eight new intents (tags
60–67) and the creation-side ledger writer, measured in this exact artifact:

| function | tag | max direct `r10` | references |
| --- | --- | ---: | ---: |
| `terminal_closure::release_terminal_reservation` | 60 | 4,096 | 193 |
| `terminal_closure::close_general_receipt` | 61 | 4,096 | 40 |
| `terminal_closure::close_general_reservation` | 62 | 200 | 82 |
| `terminal_closure::close_general_page` | 63 | 4,096 | 177 |
| `terminal_closure::close_general_pot` | 64 | 4,096 | 66 |
| `terminal_closure::close_general_candidate` | 65 | 4,096 | 95 |
| `terminal_closure::close_general_clear_work` | 66 | 4,096 | 239 |
| `terminal_closure::close_general_epoch` | 67 | 4,096 | 152 |
| `terminal_closure::create_funding_ledger` | (creation side) | 4,096 | 91 |
| `terminal_closure::close_ledgered_group` | (shared close) | 88 | 38 |
| `terminal_closure::load_selected_feed` | (helper) | 1,024 | 160 |
| `terminal_closure::load_bound_frozen_page` | (helper) | 600 | 95 |
| `terminal_closure::read_funding_ledger` | (helper) | 280 | 53 |
| `terminal_closure::live_rank_of_order` | (helper) | 4,096 | 46 |
| `terminal_closure::load_terminal_epoch` | (helper) | 160 | 42 |
| `terminal_closure::require_pages_absent` / `credit` / `release_bytes` / `require_absent` | (helpers) | 136 / 8 / 8 / 0 | 21 / 4 / 1 / 0 |

Every TerminalClosure handler and helper is at or under the 4,096-byte line and
the backend emits no diagnostic for any of them. The wave's creation-side
widening leaves every prior handler under the line as well: `init_epoch`
4,096/345 (from 393 references), `freeze_epoch` 1,264/336, `submit_candidate`
4,096/205, `seal_candidate` 4,096/233, `finalize_selection` 4,096/172,
`freeze_entitlement` 4,096/212 (from 166), `entitle_slice` 4,096/169,
`entitle_single_slice` 4,096/123, `entitle_portfolio_pair` 4,096/221,
`settle_portfolio_pair` 4,096/308, `settle_page` 4,096/185, `init_clear_work`
4,096/82, `advance_clear_work` 4,096/247, `advance_clear_slices` 4,096/64,
`complete_clear_work` 4,096/91, and the Tier 0 restructurings of the ten former
opt-z overflowers (`place_order` 4,096/422, `recorded_redeem` 4,096/364,
`prepare_direct_v4_economics` 4,096/236, `authenticate_settlement` 1,432/423,
`prepare_selection_commit` 4,096/200, `resolve_global` 4,096/620,
`apply_native_market_resolution` 4,096/270, `commit_observed_supplies`
1,080/67, `apply_legacy_market_resolution` 4,096/261, `settle_page` above).

The backend-survivor check is authoritative alongside these direct offsets; an
offset at or below 4,096 alone is not evidence that a nested-call warning is
safe.

## ELF shape and the unchanged import surface

ELF shape passes: three load segments, no writable-executable segment,
1,799,952-byte `.text`, entrypoint `0xF9338`, and exactly ten undefined
imports: `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memmove_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address`.

**The TerminalClosure wave adds no syscall.** `.dynstr` is byte-identical to
the `e8ba31d5…` seal (163 bytes, the same ten names); `.dynsym` is the same
312 bytes with different symbol values, since the entrypoint and every defined
address moved with the wave's code growth (`0xEB9D0` → `0xF9338`). The
audit gate's exact-surface predicate — which refused `sol_memmove_` on the
prior cycle until it was reviewed, and whose hostile self-check still rejects a
second hash syscall — passed unmodified on the first run of this cycle.

Loader-v3 Program/Buffer/ProgramData sizing is 36/1,979,549/1,979,557 bytes,
with 8,506,203 bytes of data-length headroom.

## Exact comparison with the superseded `e8ba31d5…` seal

This is a **materially different artifact**. The stripped ELF grows from
1,914,432 to 1,979,512 bytes (+65,080) and 921,724 byte positions differ over
the common prefix.

| section | e8ba31d5… | 4fded7a6… | verdict |
| --- | ---: | ---: | --- |
| `.text` | 1,738,176 | 1,799,952 | different |
| `.rodata` | 107,673 | 107,761 | different |
| `.rel.dyn` | 47,008 | 49,120 | different |
| `.data.rel.ro` | 19,976 | 21,080 | different |
| `.dynstr` | 163 | 163 | **identical** |
| `.dynsym` | 312 | 312 | different (values only) |
| `.dynamic` | 176 | 176 | different |
| `.shstrtab` | 72 | 72 | **identical** |

Stripped-ELF instruction disassembly grows 212,770 → 220,245 instruction lines
(both measured on the stripped artifacts). Exact section digests and both
disassemblies are retained evidence (`comparison-e8ba-vs-4fde.txt`). No CU row,
stack row, frame row, or ELF-shape row from the `e8ba31d5…` seal is carried
forward; every current row in the liveness profile was remeasured against exact
`4fded7a6…`.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified crates.io
archives/unpacked trees, and one vendored package. `dependencies.tsv`,
`registry-source-verification.tsv`, and `vendor.diff` are byte-identical to the
`e8ba31d5…` seal's — the TerminalClosure wave changed no external pin.

The staged bank fixture was verified as exact `4fded7a6…` before every suite.
Current `d77d670` tests pass — **24 default-feature targets, 101 tests, plus
three further independent runs of the Direct V3 suite (9 more)**: artifact
transport 6/6, blank-bank 2/2, candidate selection 5/5, clear lifecycle 2/2,
clear walk 3/3, clear-work creation 5/5, collateral 13/13, coupled authority
2/2, coupled settlement 2/2, DirectSelectionV2 2/2, entitled clearing 4/4,
general epoch 3/3, joined lifecycle 3/3, native full lifecycle 0/0 (mock-only,
correctly empty under the default feature), native resolution 15/15, native
window preflight 4/4, funded orders 2/2, prefund/source gate 5/5,
ResolutionWork 4/4, batched folds 2/2, source-archive host 9/9, source ingest
0/0 (mock-only), **TerminalClosure 2/2**, token leg 6/6.

**CU drift against the `e8ba31d5…` seal is at most ±0.005% on every promoted
route.** Every ResolutionWork route (Begin, Fold(1)–Fold(4), Finalize, Abort)
and every FoldBatch(2/4/8/12) moves by exactly +1 to +12 CU — `FoldBatch(12)`
929,561 → 929,573 (+0.001%) — Direct V2 Select 226,445 → 226,444, Direct V2
Freeze 357,879 → 357,876, the monolithic V4 row is unchanged at 182,859, and
every occupation-v4 / native-point resolve and retry row is bit-for-bit
unchanged (only the three external bearer-redeem rows move, −7 CU each,
−0.005%). No selected limit moves a quantum and no admission flips.

The comparison below covers the 104 rows the two seals share **outside the
Direct V3 venue**. The `direct_v3` family is excluded from the drift window on
purpose and not silently: its rows are *not reproducible between runs*, so
run-to-run and seal-to-seal deltas there measure the fixture, not the code.
Each of its 23 rows is sealed as a fresh three-run spread, and the observed
seal-to-seal movement is exactly the documented 1,500-CU PDA-bump quantum —
`LapseUnselectedDirectV3` moves the most, 164,539 → 178,051 (+8.2%, nine
quanta), `FreezeDirectEpochV4` 390,272 → 382,784 (−1.9%, five quanta),
`SubmitDirectCandidateV3` max 209,440 → 202,097. **Everything in that family
that is *not* keypair-dependent is byte-identical to the superseded seal**: all
nine close routes with every `pre_close` balance and every recipient delta, all
four rollback observations, the closed-row-to-route map, and all three strand
figures — re-derived independently from three new logs, not carried forward.

**Seven of the 104 compared rows exceed the ±1% window; every one of them is in
an UNPROMOTED family or in the family from which no projection quote derives:**

- `entitled_clearing` (UNPROMOTED): SettlePage entitled portfolio full pair
  225,739 → 234,735 (+4.0%), EntitleSlice single 204,577 → 210,607 (+3.0%),
  SettlePage entitled direct slice 54,834 → 53,330 (−2.7%), EntitleSlice
  portfolio pair 246,173 → 243,518 (−1.1%). The wave adds an optional funding
  ledger to the entitlement creation path, which is exactly where these rows
  move.
- `clear_walk` (UNPROMOTED): the hottest pass-1 slot observation 400,428 →
  391,428 (−2.25%).
- `general_epoch` (UNPROMOTED): portfolio placement 191,350 → 194,345
  (+1.6%).
- blank-bank `create_market` v2 195,057 → 192,048 (−1.5%); v3 and v4 move
  −0.004%. The creation flow has been the drift-heavy family since the
  custom-heap wave and reverses direction between seals; the byte-exactness and
  rollback assertions of the same suite gate its semantics unchanged, and no
  projection quote derives from the create_market rows.

**No account width moved.** The offline probe re-run at `d77d670` reproduces
every one of the 38 probed rows byte-for-byte and rent-for-rent against the
`e8ba31d5…` seal. The wave's one new persistent family is post-probe pinned
from the layout crate: **`general.funding_ledger`, 85 bytes / 1,482,480
lamports** (`clearing.rs::GENERAL_FUNDING_LEDGER_BYTES = 2 + 32 + 32 + 8 + 8 +
1 + 1 + 1`, account tag `GENERAL_FUNDING_LEDGER_TAG = 26`) — one optional
sibling per created group, at `seeds::general_funding_pda`, recording the exact
post-prefund payer outlay and the monotone donation floor. It closes as a
member of its own group, so the sealed conservation covers it. The terminal
inventory grows to **48 rows and 15 blocking ids**.

## TerminalClosure: what the seal establishes, and what it does not

The general clearing plane has a complete close DAG for the first time — tags
60–67, `orders_batch/terminal_closure.rs`, driven end to end on a real bank in
`logs/bank/terminal_closure.log` and sealed as the new UNPROMOTED measurement
family `terminal_closure`:

- **Cleared epoch.** The machinery held 531,652,377 lamports across 27 accounts
  (epoch, window, page, pot, checkpoint, two candidate record/feed pairs, three
  receipts, six reservations, and nine funding ledgers). **531,639,600 were
  reclaimed to the exact recorded payers**, 12,777 burned at the frozen
  incinerator — exactly the two injected donations, asserted as such — and the
  residual is exactly **1,336,320 lamports**: the sealed 64-byte batch-policy
  artifact, which is the row's own `artifact.batch_policy.final` rent and is
  already classified `PERMANENT_INFRA`. Conservation is asserted in the suite
  and re-checked by `policy.py`: inventory = reclaimed + burned, to the lamport.
- **Lapsed epoch.** 47,167,920 lamports held, 47,167,920 reclaimed, 0 burned —
  and the deliberately unledgered candidate pair stands at **47,738,640
  lamports by design**, unclosable, with the epoch root closing past it.
- Every close is refuse-before-any-byte-moves, pays exactly the recorded
  principal to the exact recorded payer, and routes all surplus to the frozen
  sink. The hostile battery inside the same two tests executes double-close,
  close-before-economic-zero, wrong-payer, wrong-sink, wrong-DAG-order,
  filled/consumed-reservation release, and non-owner release refusals on the
  same bank.

**No terminal row is reclassified `REFUNDABLE_TRANSIENT` by this seal, and the
reason is structural, not evidentiary.** The close routes exist, are driven,
and conserve exactly; what does not hold is the *unconditional* form of the two
properties `terminal_admission` requires of a refundable row:

1. **`rent_principal_recorded`.** The `GeneralFundingLedgerV1` sibling is
   **optional** at every creating instruction of the family — each accepts
   `accounts.len() == N || N + 1` (`general_epoch.rs:145`, `selection.rs:196`,
   `entitlement.rs:262`, `genesis.rs:897`). Every close runs through
   `close_ledgered_group`, which requires the ledger at its canonical PDA, so an
   account created without it refuses to close forever and no payer is ever
   guessed. The landing commit says exactly this ("an account created without
   the ledger keeps the unowned-refund blocker and stands forever, by design")
   and the sealed lapsed walk **proves the state is reachable**. The residual
   keeps its existing, correct id: `RENT.ACCOUNT_REFUND_UNOWNED`.
2. **`expiry_or_reaper`.** `release_terminal_reservation` (tag 60) is the only
   signer-gated step in the whole DAG — tags 61–67 are permissionless. A
   zero-fill or lapsed ACTIVE reservation can only be released by its owner, and
   `CloseGeneralPage` (63) requires every live record's reservation RELEASED or
   CONSUMED, `CloseGeneralPot` (64) requires every page absent, and
   `CloseGeneralEpoch` (67) requires both. Owner abandonment therefore holds the
   page, the pot, and the epoch root open at recorded rent cost, and the design
   explicitly declines to invent a sweep right. New blocking id:
   **`GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`**. It does not reach
   `epoch.receipt`, whose close gates only on the receipt being exhausted by
   permissionless settlement.

What the seal *does* retire is the reason those rows carried
`PROFILE.STORAGE_INVENTORY_INCOMPLETE` — *no close path existed*. That generic
id is replaced on `epoch.window`, `epoch.final_pot`, and `epoch.receipt` by the
two precise residuals above, and it is kept only on the four `legacy.*` rows,
where it is now carrying a different and still-true weight: their cardinality is
UNADMITTED, and `terminal_admission` refuses a refundable row whose instance
count is unbounded. `policy.py::require_terminal_closure_evidence` welds both
halves so neither the classification nor the evidence can drift alone; it
refuses a general-plane row that quietly became refundable, evidence that stops
declaring the ledger optional or the release edge owner-signed, a walk that did
not conserve exactly, a cleared residual that is not the permanent artifact's
own rent row, and either residual id vanishing from the global set.

The suite prints no per-route CU label for tags 60–67, so **no CU row is
invented for any close route** — the family declares
`per_route_cu: NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED`, and its evidence is exact
lamport conservation plus the executed refusal battery.

The settlement blocker ledger moves with it: `SETTLEMENT_BLOCKERS` is now
exactly `[PartialFillLedger, VirtualPot]`, and `RETIRED_SETTLEMENT_BLOCKERS`
grows to six with `GeneralReservationSetClosure` and `TerminalClosure` joining
`FrozenPolicyPreimage`, `FullWidthRelationDomain`, `CandidateWindowClosure`, and
`EntitlementFreeze`.

## The V3 order-page strand, now named

The sealed V3 campaign's third stranded family gets its own blocking id at this
seal: **`DIRECT.ORDER_PAGE_RENT_PERSISTS`**. `init_direct_v4_order_page`
(`genesis.rs`) creates the 4,012-byte V4 page with
`create_pda_account_full_principal` — no funding-ledger sibling, no recorded
payer — and no V3 route closes it; all three bank runs re-measure **28,814,401
lamports** still held after both settle and lapse (`DirectV3 STRAND …
order.page`, identical across runs). The general plane's instance of the same
row *does* close (tag 63, driven in the cleared walk), which is why the row
stays an honest STOP rather than moving either way: the row's scope covers both
planes and only one of them closes. The honest per-epoch V3 structural strand
is unchanged at the corrected **35,941,440 lamports**, and
`require_v3_close_evidence` now also refuses a stranding row that fails to carry
its own id.

## Unpromoted standing is unchanged

The general clearing plane, including this close wave, is **SBF-executed bank
evidence only, deliberately unpromoted**. Five UNPROMOTED measurement families
(`general_epoch`, `clear_walk`, `candidate_selection`, `entitled_clearing`,
`terminal_closure`) plus the two Direct V3 families (`direct_v3`,
`direct_v3_close`) — eighteen same-ELF families in all, nineteen bank logs. No
admission, quote, or reward row is derived for any tag-49–67 route, no live
flag moves, `live_v3` stays false, and the reference adapter refuses tags 49–67
with `UnsupportedIntent`. Admission-policy treatment of the plane is ember's
decision, not this seal's; `decision_owner` is `ember` on every one.

Default Endow still refuses with `0x79`
(`ClutchError::SourceReleaseUnavailable = 0x0079`, asserted by the suite
together with byte-and-lamport-exact rollback) because no production source
release is registered. No CU value is invented for that log, and no
mock-feature ELF evidence is mixed here.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-d77d670-reseal-evidence/artifact-4fded7a67a2d8994f4dc2b82c533b978d14d6107f28de7cbbe7674ecdcedf6cb`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-d77d670-reseal-evidence/authoritative-audit-console.log`
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-d77d670-reseal-evidence/source-d77d670-baabb0025acda92cd6c3489682eebcb03c47452c5f2e331a8dbab9341e6b2b4b.tar`
- First-party frame table: `first-party-frame-audit.txt`; account probe:
  `account-probe-d77d670.txt`; section/probe comparison:
  `comparison-e8ba-vs-4fde.txt` and `probe-compare/`; cross-path build log:
  `sbf-build-crosspath.log` (sealed in-root)
- Evidence checksum ledger: `SHA256SUMS` (mirrored here as
  `upstream-SHA256SUMS`)
