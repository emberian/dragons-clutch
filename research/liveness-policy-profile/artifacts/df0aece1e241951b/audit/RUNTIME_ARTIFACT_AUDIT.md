# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `04acf6165376d3abfb3bc16877e2a0ba2bb47931`. This is local build, static
artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary — in place, per the build-path protocol amendment

- **Canonical build location:** `/Users/ember/dev/dragons-clutch` itself.
  Per `docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md` the ELF identity is
  same-path-reproducible only, so the canonical identity is defined at the
  canonical checkout path, where every bank log's fixture binds. Before the
  build the tree was verified exactly HEAD: `git status --porcelain` empty over
  the declared closure and `git diff HEAD` empty. No detached worktree is used
  for the canonical build; the cross-path build below is a relocation probe.
- Exact Git archive: 32,798,720 bytes, SHA-256
  `d78cec1eea9e04cba3eb59588d52d29d12bda2382c2b7376783ef5d5563f56df`
- Declared SBF closure: **111 files**, SHA-256
  `ceab59c6270501a2e2a3979611a32eb314c6facaf5841836c0a395a37e6429cb`
- `04acf61` is the `56ec1ed` fee-plumbing merge plus exactly one commit, and
  that commit is **inside** the closure rather than beside it: the cycle-F
  housekeeping wave (`04acf61`) repaired thirteen rustdoc warnings across nine
  in-closure files, moved four `PROPOSED` status comments to `FROZEN` per
  `docs/decisions/ADOPTED_2026-08-20.md` item 1, and crate-qualified six
  unresolved intra-doc links the fee wave had left in
  `research/batch-policy-identity/src/revenue_policy_v1.rs`. All of it is
  doc-comment and comment bytes; none of it changes a single executable
  statement. It forks the ELF identity anyway — precedent `9c371fe` — which is
  exactly why the roadmap held these three debts for a reseal-bearing wave
  instead of opening a drift window for a comment.
- The closure grows 109 → 111 files against the `4fded7a6…` seal's
  declaration. The two additions are exactly
  `programs/solana-layout/src/revenue.rs` and
  `research/batch-policy-identity/src/revenue_policy_v1.rs`. The declared
  `source_paths` themselves are unchanged — no path entered or left the
  closure declaration.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Every toolchain binary digest in `authoritative-audit-console.log` is
  unchanged from the `4fded7a6…` seal.
- Pass 1/pass 2 stripped ELF, both built in place with fresh
  `CARGO_TARGET_DIR`s: byte-identical SHA-256
  `df0aece1e241951b7b70521e20b15e35428c8b464ffa9ce02d9b9aed2141ae1b`,
  1,986,104 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `e4db7f0b9c68e2818df082a4adfbcd0495c10a4125483f73923c151f0d5f7b74`,
  2,167,536 bytes

### The cross-path probe: `PATH_TIED_SYMBOL_ORDER`, unchanged disposition

One build of the same commit in a detached worktree at
`/Users/ember/jobs/dragons-clutch-r1-04acf61-xpath-worktree` (fresh target
directory, same recipe) produced stripped SHA-256
`7cd3beb84aa973f81822aa292eaefb66b7299e49f718218cd453be22d378adb5` — same
1,986,104 bytes, **different bytes**. The divergence is exactly the tied-pair
signature and nothing else: **481 `.text` bytes at 195 contiguous sites**
(spanning `+0x659` to `+0xa48fc`) and **6 `.rel.dyn` bytes at 3 sites**, with
`.rodata`, `.data.rel.ro`, `.dynstr`, `.dynsym`, `.dynamic`, and `.shstrtab`
all byte-identical, and the disassembly the same 220,910 instruction lines. The
unstripped cross-path ELF also differs
(`01d9b46b8e1f20b50c07bab7eb20081db4d39dffeeee297edb571ca9d37f63f8`).

This is the same shape the V3 campaign observed at two other paths (486 `.text`
bytes, 6 `.rel.dyn` bytes) and a wider instance than the `4fded7a6…` seal's own
cross-path build (5 `.text` bytes at 4 sites). The evidence convention remains
the **observed-digest list** `artifact_reproducibility.cross_path_builds`, and
`policy.py::check_artifact_binding` still refuses both the retired scalar field
and any list entry equal to the canonical digest. Building the same source at
the same path with the two different published recipes — the audit script's
`--arch v0 --offline --locked` form and `run_svm_tests.sh`'s form — produced
byte-identical output at both the canonical path and the worktree path, so the
divergence is attributable to the path and not to the recipe.

### The relocated-Cargo-home probe: `PATH_SENSITIVE`, with the attribution corrected

The protocol probe (`programs/clutch-sbf/audit/audit_artifact.sh`, which
symlinks only `registry/index` and `registry/cache` into a fresh home so Cargo
must extract the crate sources itself) diverged again:
`dd20707f5d61bbfe2e697d60506d08d55b385c859172694f2c4bd3e11c52a562` at
**1,986,656 bytes**, 552 bytes larger, with 35,709 differing byte positions over
the common prefix. The mechanism re-derives **exactly** as it did at the
`4fded7a6…` seal, independently and to the byte: `.rodata` grows by exactly
those 552 bytes and contains exactly **three** absolute registry paths under the
relocated home that the canonical build does not contain at all — for
`solana-address-2.6.1/src/syscalls.rs`,
`solana-program-entrypoint-3.1.1/src/lib.rs`, and
`solana-account-info-3.1.1/src/lib.rs`. These are `core::panic::Location`
strings; the canonical `.rodata` renders the first as the relative
`src/syscalls.rs` and holds no host-local absolute path at all — its only three
absolute strings are the platform-tools **build machine's** `/Users/runner/…`
std paths, baked into the pinned Rust distribution. `.text`, `.rel.dyn`,
`.data.rel.ro`, and `.dynamic` all shift with the `.rodata` growth. The
disposition is therefore `PATH_SENSITIVE`, as at the last two seals.

**What is new is the attribution, and it narrows the claim.** The protocol
probe builds its relocated home under `$TMPDIR`, which on macOS is reached
through the `/var` → `/private/var` symlink (and, as `mktemp` composes it here,
with a doubled separator: `…/T//clutch-sbf-artifact-audit.xP0xzi/…`). Two
controls were run with the *same* recipe and the *same* fresh extraction, and
differing only in where the relocated home sits:

| probe | relocated `CARGO_HOME` | stripped SHA-256 |
| --- | --- | --- |
| protocol (`audit_artifact.sh`) | `/var/folders/…/T//clutch-sbf-artifact-audit.xP0xzi/cargo-home-relocated` | `dd20707f…` **diverged** |
| control B | `/Users/ember/jobs/dragons-clutch-r1-04acf61-reloc-probe-B` | `df0aece1…` **canonical** |
| control C | `/private/var/folders/…/T/clutch-reloc-probe-C.7Io2J5/cargo-home` | `df0aece1…` **canonical** |

Control C is the decisive one: it sits on the same temporary filesystem, in the
same directory, extracted just as freshly — and differs from the protocol probe
only in that its path is the resolved `/private/var` form. It reproduced the
canonical bytes exactly. So the divergence does **not** track "the Cargo home
moved"; it tracks **a `CARGO_HOME` whose path contains an unresolved symlink
component**, which defeats the relative-path computation Cargo otherwise
performs and hands rustc absolute crate-root paths that land in
`panic::Location`. The seal records the protocol probe's digest and its
`PATH_SENSITIVE` disposition — that is what the declared probe measured — and
`policy.py::check_artifact_binding` now **requires** a diverged probe to carry
these controls and to name what the divergence tracks, refusing an attribution
with no reproducing control behind it.

Two consequences worth stating plainly rather than burying. First, the
`d6929549…` and `4fded7a6…` seals' `PATH_SENSITIVE` findings are *reproduced*
here but were *attributed too widely*; the narrower statement is the one above.
Second, the protocol probe as written will report `PATH_SENSITIVE` on any macOS
host regardless of the artifact, because `$TMPDIR` is always the symlinked form
— so the probe currently measures the harness as much as the recipe. Amending
`audit_artifact.sh` to resolve its work directory (or to run the probe at both
forms) is a protocol change and is **not** made here; it is recorded as owed.
Single-host observation, not a cross-host claim.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was unused. No
network, RPC, signing, deployment, submission, or external-state mutation
occurred.

## Final-LTO and direct-frame gate

- 36 backend diagnostic lines naming 28 unique symbols — `backend-stack-
  diagnostics.txt` is **byte-identical** to the `4fded7a6…` and `e8ba31d5…`
  seals'.
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- 1,011 resident text symbols at 1,009 addresses; all 1,009 addresses were
  disassembled (from 999 at 997).
- **60,441** direct `r10` references; maximum offset 4,096; zero invalid
  positive, zero, or greater-than-4,096 references. The deepest direct
  reference still sits in `claim_truth::observe_outcome_mints`, unchanged.
- **924** first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `first-party-frame-audit.txt` (retained
  evidence). Zero regions of any provenance exceed 4,096.

**The revenue seams' frames, measured in this exact artifact.** These are the
wave's new first-party regions:

| function | max direct `r10` | references |
| --- | ---: | ---: |
| `genesis::init_revenue_policy_record` | 4,096 | 129 |
| `genesis::close_revenue_policy_record` (tag 68) | 4,096 | 58 |
| `genesis::write_realm` | 72 | 18 |
| `accounts::read_realm` | 132 | 13 |
| `seeds::revenue_policy_pda` | 88 | 15 |
| `seeds::realm_pda` | 88 | 15 |
| `clutch_solana_layout::revenue::RevenuePolicyRecordV1::decode` | 160 | 25 |
| `clutch_solana_layout::canonical_realm_id` | 280 | 35 |
| `clutch_solana_layout::RealmAccount::decode` | 68 | 14 |
| `clutch_batch_policy_identity::revenue_policy_v1::revenue_policy_digest` | 176 | 20 |
| `…::revenue_policy_v1::encode_revenue_policy` | 32 | 11 |
| `…::revenue_policy_v1::treasury_admits_fee_bearing` | 0 | 0 |

`RevenuePolicyRecordV1::{validate,encode}` and `RealmAccount::{validate,encode}`
carry zero direct references. The backend emits no diagnostic for any of them.

**The fee-bearing account tail is where the existing handlers moved.**
`orders_batch::general_epoch::init_epoch` goes 4,096/345 → **4,096/457**
references — the fee-bearing admission branch — and `general_epoch::freeze_epoch`
is unchanged at 1,264/336. **Every TerminalClosure handler and helper (tags
60–67) is byte-for-byte unchanged in this table**: `release_terminal_reservation`
4,096/193, `close_general_receipt` 4,096/40, `close_general_reservation` 200/82,
`close_general_page` 4,096/177, `close_general_pot` 4,096/66,
`close_general_candidate` 4,096/95, `close_general_clear_work` 4,096/239,
`close_general_epoch` 4,096/152, `create_funding_ledger` 4,096/91,
`close_ledgered_group` 88/38, `load_selected_feed` 1,024/160,
`load_bound_frozen_page` 600/95, `read_funding_ledger` 280/53,
`live_rank_of_order` 4,096/46, `load_terminal_epoch` 160/42, and the four small
helpers at 136/8/8/0. The entitlement and settlement handlers are likewise
unchanged (`entitle_slice` 4,096/169, `entitle_single_slice` 4,096/123,
`entitle_portfolio_pair` 4,096/221, `settle_portfolio_pair` 4,096/308,
`settle_page` 4,096/185, `advance_clear_work` 4,096/247, `advance_clear_slices`
4,096/64, `place_order` 4,096/422).

The backend-survivor check is authoritative alongside these direct offsets; an
offset at or below 4,096 alone is not evidence that a nested-call warning is
safe.

## ELF shape and the unchanged import surface

ELF shape passes: three load segments, no writable-executable segment,
1,805,656-byte `.text`, entrypoint `0xFB080`, and exactly ten undefined
imports: `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memmove_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address`.

**The fee wave adds no syscall, and did not move the ten-symbol surface.**
`.dynstr` is byte-identical to the `4fded7a6…` seal (163 bytes, the same ten
names, same digest `7b804b755d45b1a4…`); `.dynsym` is the same 312 bytes with
different symbol values, since the entrypoint and every defined address moved
with the wave's code growth (`0xF9338` → `0xFB080`). The audit gate's
exact-surface predicate passed unmodified on the first run of this cycle.

Loader-v3 Program/Buffer/ProgramData sizing is 36/1,986,141/1,986,149 bytes,
with 8,499,611 bytes of data-length headroom.

## Exact comparison with the superseded `4fded7a6…` seal

This is a **materially different artifact**. The stripped ELF grows from
1,979,512 to 1,986,104 bytes (+6,592) and 943,327 byte positions differ over the
common prefix.

| section | 4fded7a6… | df0aece1… | verdict |
| --- | ---: | ---: | --- |
| `.text` | 1,799,952 | 1,805,656 | different |
| `.rodata` | 107,761 | 107,929 | different |
| `.rel.dyn` | 49,120 | 49,696 | different |
| `.data.rel.ro` | 21,080 | 21,224 | different |
| `.dynstr` | 163 | 163 | **identical** |
| `.dynsym` | 312 | 312 | different (values only) |
| `.dynamic` | 176 | 176 | different |
| `.shstrtab` | 72 | 72 | **identical** |

Stripped-ELF instruction disassembly grows 220,245 → 220,910 instruction lines
(both measured on the stripped artifacts). Exact section digests and both
disassemblies are retained evidence (`comparison-4fde-vs-df0a.txt`). No CU row,
stack row, frame row, or ELF-shape row from the `4fded7a6…` seal is carried
forward; every current row in the liveness profile was remeasured against exact
`df0aece1…`.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified crates.io
archives/unpacked trees, and one vendored package. `dependencies.tsv`,
`registry-source-verification.tsv`, and `vendor.diff` are byte-identical to the
`4fded7a6…` seal's — the fee wave and the doc wave changed no external pin, and
`vendor.diff` is empty.

The staged bank fixture was verified as exact `df0aece1…` before every suite and
re-verified unchanged after the last one. Current `04acf61` tests pass —
**26 default-feature targets, 104 tests, plus three further independent runs of
the Direct V3 suite (9 more)**: artifact transport 6/6, blank-bank 2/2,
candidate selection 5/5, clear lifecycle 2/2, clear walk 3/3, clear-work
creation 5/5, collateral 13/13, coupled authority 2/2, coupled settlement 2/2,
DirectSelectionV2 2/2, **disagreement exhibit 2/2**, entitled clearing 4/4,
general epoch 3/3, joined lifecycle 3/3, native full lifecycle 0/0 (mock-only,
correctly empty under the default feature), native resolution 15/15, native
window preflight 4/4, funded orders 2/2, prefund/source gate 5/5,
ResolutionWork 4/4, batched folds 2/2, **revenue policy 1/1**, source-archive
host 9/9, source ingest 0/0 (mock-only), TerminalClosure 2/2, token leg 6/6.

**CU drift against the `4fded7a6…` seal is at most ±0.034% on every promoted
route.** Of the 119 rows in the promoted families, 96 move and every one of them
moves by −1 to −144 CU except three create_market rows: every ResolutionWork
route and every FoldBatch drops 12 CU per fold (`FoldBatch(12)` 929,573 →
929,429, −0.015%), Direct V2 Select 226,444 → **226,522** (+78, +0.034%, the
largest promoted move in either direction), Direct V2 Freeze 357,876 → 357,868,
the monolithic V4 row 182,859 → 182,857, every occupation-v4 and native-point
resolve/retry/redeem row −2 or −5 CU, `PlaceOrder` −1, `CancelOrder` −1,
`WithdrawCash` −2. **No selected limit moves a quantum on any promoted route and
no admission flips.**

The exception is the blank-bank creation family, which has been the drift-heavy
family since the custom-heap wave and reverses direction between seals:
`create_market` v2 192,048 → **207,044 (+7.8%)**, v3 214,719 → 211,715 (−1.4%),
v4 213,324 → 210,320 (−1.4%). **No projection quote derives from the
create_market rows**, and the byte-exactness and rollback assertions of the same
suite gate its semantics unchanged.

The `direct_v3` family is excluded from the drift window on purpose and not
silently: its rows are *not reproducible between runs* — the suite's fixture
keypairs are freshly random per run and each PDA bump probe costs 1,500 CU — so
run-to-run and seal-to-seal deltas there measure the fixture, not the code. Each
of its 23 rows is sealed as a fresh three-run spread, and the observed
seal-to-seal movement lands on the documented 1,500-CU quantum (largest,
`verify_candidate` index 1/2 ±8.0%, six quanta). **Everything in that family
that is *not* keypair-dependent is byte-identical to the superseded seal**: all
nine close routes with every `pre_close` balance and every recipient delta, all
four rollback observations, the closed-row-to-route map, and all three strand
figures — re-derived independently from three new logs, not carried forward.

**No account width moved.** The offline probe re-run at `04acf61` reproduces
`account-probe-d77d670.txt` **byte for byte**, all 38 probed rows and both rent
metadata lines. The revenue plane's one persistent family,
`revenue.policy_record.v1` (156 bytes / 1,976,640 lamports), is post-probe
pinned from `programs/solana-layout/src/revenue.rs`
(`REVENUE_POLICY_RECORD_BYTES = 2 + 32 + 32 + 32 + 56 + 1 + 1`, account tag
`REVENUE_POLICY_RECORD_TAG = 27`), landed in the inventory by the preceding
rows-first commit; the terminal inventory stands at **49 rows and 16 blocking
ids**, the new id being `REVENUE.REALM_PERMANENCE_HOLDS_RECORD`.

## The fee-bearing boundary, driven and refused

`programs/clutch-sbf/svm-tests/tests/revenue_policy.rs` drives the
`RevenuePolicyV1` boundary on a real bank against this exact ELF and is sealed
as the new UNPROMOTED family `revenue_boundary`. **It prints no CU label and no
headline row**, so this seal derives **no CU row, no quote, and no refusal code
from it** — the eight refusal codes it asserts live in the suite source, and a
number transcribed out of source is not evidence. The family declares
`per_route_cu: NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED` and
`refusal_codes: NOT_PRINTED_BY_SUITE_ASSERTED_IN_SOURCE_ONLY`, exactly the
convention `terminal_closure` established for tags 60–67, and
`policy.py::require_revenue_boundary_evidence` refuses any `_cu`/`_rows` field
in it that is not one of those declarations.

What the family *does* carry, welded to the inventory in both directions: both
fee rates are zero and no fee-bearing epoch admits; the treasury is the
distinguished `REVENUE_TREASURY_UNSET_V1` sentinel, so the refusal is
structural rather than a value someone could set; and
`revenue.policy_record.v1` is an honest **STOP** carrying its own residual id.
That residual is real and worth naming precisely: `CloseRevenuePolicyRecord`
(tag 68) exists, is a full close route, and pays the exact recorded principal to
the exact recorded payer — but it is gated on the Realm account being *gone*,
and the `realm` row is `PERMANENT_INFRA` with no close route at all. The
record's principal is therefore capitalized for the Realm's whole life. The
mandatory `GeneralFundingLedgerV1` sibling means the unowned-refund residual
cannot arise here, which is why this row carries a new, narrower id
(`REVENUE.REALM_PERMANENCE_HOLDS_RECORD`) instead of
`RENT.ACCOUNT_REFUND_UNOWNED`.

The profile's own rule is unchanged and now welded at this boundary too: **a fee
is never liveness funding, at any rate.** The day a fee-bearing epoch can open
is a decision, not a derivation, and the checker refuses first.

## Rung W1: the walk plane's quotes grow to five families and 35 routes

Every W1 row is re-derived from this seal's own tables, as the rung's teeth
require. Seven of the 25 carried-over routes move a selected limit by one
10,000-CU quantum — `advance_clear_slices` 230,000 → 220,000,
`advance_clear_work_pass1_forty_order` 490,000 → **500,000**,
`advance_clear_work_pass1_small_book` 380,000 → 370,000,
`advance_clear_work_pass2_small_book` 370,000 → 360,000, `entitle_slice_single`
270,000 → 260,000, `settle_page_entitled_direct_slice` 70,000 → 60,000, and
`settle_page_entitled_portfolio_full_pair` 300,000 → 290,000. The worst route is
still `FreezeEpoch` at 3 pages / 40 orders, **717,815 CU** (limit 900,000,
reward 1,010,000 lamports), **64.1%** of the 1,120,000 raw-CU admission
boundary. All 35 routes clear the 25%-headroom rule; compute is still not this
plane's problem.

**The rung's quoted-family list grows from four to five, and the reason is the
rung's own honesty rule.** `disagreement_exhibit.rs` (the L2 exhibit, landed
after the `4fded7a6…` seal was cut) drives the *same* general-plane routes
against the *same* ELF under the *same* frozen `GENERAL_CLEARING_POLICY_V1`, at
a third book composition — 13 orders, 7 slices, five entitled single crossings
and one portfolio full pair — and it prints its labels. Several of those
observations are **hotter** than the two-suite books': `AdvanceClearWork` pass 1
at **411,611 CU** against the forty-order book's 393,207, `EntitleSlice (single)`
at 224,645 against 203,097, and `SettlePage (entitled portfolio full pair)` at
250,584 against 224,233. W1 already forbids a measured CU field in a quoted
family from going unquoted — but a family *outside* the quoted list escapes that
check entirely, which is exactly the loophole a hotter unpublished observation
would slip through. So the family is quoted, as ten new routes of its own:

| route | max CU | selected limit | keeper reward |
| --- | ---: | ---: | ---: |
| `advance_clear_work_pass1_exhibit_book` | 411,611 | 520,000 | 630,000 |
| `advance_clear_work_pass2_exhibit_book` | 301,177 | 380,000 | 490,000 |
| `entitle_slice_portfolio_pair_exhibit_book` | 270,660 | 340,000 | 450,000 |
| `settle_page_entitled_portfolio_full_pair_exhibit_book` | 250,584 | 320,000 | 430,000 |
| `entitle_slice_single_exhibit_book` (5 obs.) | 224,645 | 290,000 | 400,000 |
| `advance_clear_slices_exhibit_book` | 163,296 | 210,000 | 320,000 |
| `complete_clear_work_exhibit_book` | 118,706 | 150,000 | 260,000 |
| `freeze_entitlement_exhibit_book` | 98,407 | 130,000 | 240,000 |
| `init_clear_work_exhibit_book` | 70,613 | 90,000 | 200,000 |
| `settle_page_entitled_direct_slice_exhibit_book` (5 obs.) | 42,374 | 60,000 | 170,000 |

The four original families' rows are **unchanged in kind by this addition**,
because each W1 row already bounds its own measured composition and no other —
that is what `BATCH_SHAPE_VARIABLE_OBSERVED_MAXIMUM_ONLY` says out loud. The
exhibit is a new measured composition, so it gets its own quote rather than
silently widening someone else's. One further weld: the exhibit's own walk
sender attaches the identical `request_heap_frame(262144)` instruction to every
transaction it measures but never re-prices it, so its routes are charged the
`clear_walk` suite's measured **150 CU** rider rather than a figure invented for
them, and `require_walk_plane_w1_quotes` refuses if the family stops declaring
that borrowing.

Everything else about the rung is unmoved: live flags stay `UNTOUCHED`, no
keeper program consumes these quotes, the rent side is **not** quoted, tags
60–67 get no row at all, and W2 stays blocked on
`RENT.ACCOUNT_REFUND_UNOWNED`, `GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`, and
`PROFILE.STORAGE_INVENTORY_INCOMPLETE` plus the five section-3 evidence gaps.

## Unpromoted standing is unchanged

The general clearing plane, the disagreement exhibit, the Direct V3 venue, and
the revenue boundary are **SBF-executed bank evidence only, deliberately
unpromoted** — twenty same-ELF measurement families in all, twenty-one bank
logs. No admission, quote, or reward row is derived for any tag-49–68 route
outside rung W1's explicit quotes-without-flags shape, no live flag moves,
`live_v3` stays false, and the reference adapter refuses tags 49–68 with
`UnsupportedIntent`. Admission-policy treatment of these planes is ember's
decision, not this seal's; `decision_owner` is `ember` on every one.

`SETTLEMENT_BLOCKERS` is unchanged at exactly `[PartialFillLedger, VirtualPot]`
and `RETIRED_SETTLEMENT_BLOCKERS` is unchanged at six.

Default Endow still refuses with `0x79`
(`ClutchError::SourceReleaseUnavailable = 0x0079`, asserted by the suite
together with byte-and-lamport-exact rollback) because no production source
release is registered. No CU value is invented for that log, and no
mock-feature ELF evidence is mixed here.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-04acf61-reseal-evidence/artifact-df0aece1e241951b7b70521e20b15e35428c8b464ffa9ce02d9b9aed2141ae1b`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-04acf61-reseal-evidence/authoritative-audit-console.log`
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-04acf61-reseal-evidence/source-04acf61-d78cec1eea9e04cba3eb59588d52d29d12bda2382c2b7376783ef5d5563f56df.tar`
- First-party frame table: `first-party-frame-audit.txt`; account probe:
  `account-probe-04acf61.txt`; section comparisons:
  `comparison-4fde-vs-df0a.txt`, `comparison-canonical-vs-crosspath.txt`, and
  `comparison-canonical-vs-relocated.txt`; relocation controls:
  `sbf-build-relocatedB.log` and `sbf-build-relocatedC.log`; cross-path build
  log: `sbf-build-crosspath.log` (sealed in-root)
- Evidence checksum ledger: `SHA256SUMS` (mirrored here as
  `upstream-SHA256SUMS`)

## Owed, not done here

- `programs/clutch-sbf/audit/audit_artifact.sh` runs its relocation probe under
  `$TMPDIR` and therefore reports `PATH_SENSITIVE` on any macOS host by
  construction. Resolving the probe's work directory (or running it at both the
  symlinked and resolved forms and recording both) is a protocol amendment and
  belongs to a decision, not to a reseal lane.
- Four pre-existing rustdoc warnings remain in-closure and unrepaired:
  `FULL_RELATION_CANDIDATE_PREIMAGE` and `feed_leg` in
  `clutch-batch-policy-identity`, `CANDIDATE_FEED_TAG` and `ClearWorkAccount` in
  `clutch-solana-layout`. Neither crate can take `RUSTDOCFLAGS='-D warnings'`
  until they are, and `clutch-batch-policy-identity` has no `cargo_doc` gate at
  all — which is how six fresh warnings landed there unseen in the fee wave.
