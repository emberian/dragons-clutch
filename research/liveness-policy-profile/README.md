# Liveness policy evidence profile

Status: **R1 ARTIFACT SEALED / MEASURED RESOLUTIONWORK / TERMINAL INVENTORY
CHECKED / PROTOCOL ADMISSION STOP**.

This directory contains:

- `admission_math.py`: fail-closed CU quotes and staged ResolutionWork/Direct
  path maxima;
- `terminal_admission.py`: strict account/value terminal checker;
- `terminal_profile.py`: complete current-runtime account classification;
- `src/main.rs`: exact account-width and pinned-default-rent probe;
- `policy.py`, `evidence.json`, and the normalized capture: exact artifact,
  bank, source/test identity, rent, reward, and source-drift seal;
- `artifacts/df0aece1e241951b`: the current canonical ELF, build and stack/ELF
  audit evidence, and bank logs measured against that exact ELF;
- `artifacts/4fded7a67a2d8994`, `artifacts/e8ba31d582be3939`,
  `artifacts/d692954949d57db22`, `artifacts/fda59705ac1c1869`,
  `artifacts/187d5ee16f72946a`, `artifacts/af6bb79cc3766bd0`,
  `artifacts/bd20711b01828a74`, and `artifacts/a5725a3d8e149b2b`: the eight
  preceding historical seals, retained in full for audit continuity but
  excluded from the current projection. `policy.py` refuses a seal that
  overwrites a superseded artifact root or drops any of its evidence files.

Every sealed path is checked for repository membership, not merely for
presence on the running disk. The root `.gitignore` excludes `*.so` and
`*.log`, so a plain `git add` of a new artifact root silently commits a
fraction of it while every hash of a working-tree file keeps passing;
`check_tracked_evidence` therefore requires each current and retained
historical evidence path to be tracked and to equal its committed blob at
`HEAD`, refusing an ignored, staged-but-uncommitted, or
modified-after-commit file. If git cannot answer that question the checker
reports `UNAVAILABLE` with the exact git failure and exits nonzero; an
unanswerable question is never reported as tracked.

The profile never treats Hoard principal, **fees**, future volume, a future
subscriber, a token price, or a token-to-SOL conversion as liveness funding.
It publishes no finite work quote when the requested CU headroom fails and no
complete `LivenessPolicy` tuple while any mandatory path remains stopped.

Run the exact seal, strict current-runtime drift gate, and stable arithmetic
and terminal checks:

```sh
cd research/liveness-policy-profile
python3 policy.py
python3 policy.py --check-current
python3 -m unittest -v \
  test_policy.py \
  test_admission_math.py \
  test_terminal_admission.py \
  test_terminal_profile.py

cargo run --offline --locked \
  --manifest-path Cargo.toml
cargo clippy --offline --locked \
  --manifest-path Cargo.toml \
  --all-targets -- -D warnings
```

The current artifact source and test/evidence ancestry is exact commit
`04acf61` (the `56ec1ed` fee-plumbing merge plus exactly one in-closure
housekeeping commit). `runtime_ref`, `evidence_ref`, and
`artifact.source_ref` are that one commit **by construction** at this seal.

The seal covers **fee plumbing to the boundary and nothing past it**
(`docs/decisions/ADOPTED_2026-08-20.md` items 6, 8, and 9): the
`RevenuePolicyV1` frozen const family, the per-Realm `RevenuePolicyRecordV1`
account (tag 27, 156 bytes / 1,976,640 lamports) written inside `InitRealm`
with a **mandatory** funding-ledger sibling, `CloseRevenuePolicyRecord`
(tag 68), the treasury-Position path, the carry fields, and the fee-bearing
sibling policy const. **Both rates stay zero and no fee-bearing epoch admits.**
It also carries the cycle-F housekeeping commit, which is *inside* the SBF
source closure and therefore forks the identity on its own: thirteen rustdoc
warnings repaired across nine in-closure files, four `PROPOSED` status comments
moved to `FROZEN` per item 1, and six unresolved intra-doc links in
`revenue_policy_v1.rs` crate-qualified. Not one executable statement changed in
that commit; the ELF forked anyway, which is exactly why the roadmap held those
debts for a reseal-bearing wave.

**The fee-bearing boundary, driven on a real bank** (`logs/bank/revenue_policy.log`,
new UNPROMOTED family `revenue_boundary`): fee-bearing admission with no record
refuses with its own code, the record rides Realm creation byte-for-byte with
its mandatory ledger, and with the record present the walk reaches the treasury
byte and refuses **there** — on the distinguished `REVENUE_TREASURY_UNSET_V1`
sentinel, which is what makes the B4a deferral structural rather than a value
someone could set. The close refuses while the Realm stands and moves nothing;
a hostile re-creation refuses and the bytes stand; the zero-fee plane is
untouched. **The suite prints no CU label and no headline row, so no CU row, no
quote, and no refusal code is derived from it** — the eight codes it asserts
live in the suite source, and a number transcribed out of source is not
evidence. The family says so in its own declarations
(`per_route_cu: NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED`,
`refusal_codes: NOT_PRINTED_BY_SUITE_ASSERTED_IN_SOURCE_ONLY`) and
`require_revenue_boundary_evidence` refuses any `_cu`/`_rows` field that is not
one of them.

`revenue.policy_record.v1` is an honest **STOP**, and its residual is narrower
than the general plane's. `CloseRevenuePolicyRecord` is a real close route that
pays the exact recorded principal to the exact recorded payer, and the
mandatory ledger means `RENT.ACCOUNT_REFUND_UNOWNED` cannot arise here at all —
but the close is gated on the Realm account being *gone*, and `realm` is
`PERMANENT_INFRA` with no close route. The record's principal is capitalized
for the Realm's whole life. New id: **`REVENUE.REALM_PERMANENCE_HOLDS_RECORD`**.
The terminal inventory stands at **49 rows and 16 blocking ids**.
`SETTLEMENT_BLOCKERS` is unchanged at exactly `[PartialFillLedger, VirtualPot]`.

Per the 2026-08-20 build-path protocol amendment
(`docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md`) the canonical identity is
the in-place double build at the canonical checkout path: pass 1/pass 2 are
byte-identical `df0aece1…` (1,986,104 bytes, growing from 1,979,512). The
cross-path worktree build produced `7cd3beb8…` — same length, **481 `.text`
bytes at 195 sites and 6 `.rel.dyn` bytes at 3 sites**, no other section
touched: the tied-pair signature, wider than the last seal's four sites and
close to the V3 campaign's. `cross_path_builds` stays an observed-digest list
and `policy.py` refuses both the retired scalar field and any entry equal to
the canonical digest.

**The relocated-Cargo-home probe diverged again — and this seal corrects its
attribution.** The protocol probe reproduced the `4fded7a6…` mechanism to the
byte: `dd20707f…` at 1,986,656 bytes, `.rodata` larger by exactly 552 bytes
carrying exactly three absolute registry `panic::Location` paths
(`solana-address`, `solana-program-entrypoint`, `solana-account-info`) that the
canonical build renders relative. But two controls, same recipe and same fresh
extraction, differing only in *where the relocated home sits*, both reproduced
the canonical bytes exactly — including one inside the same temporary
filesystem in resolved `/private/var` form. The protocol probe builds under
`$TMPDIR`, which macOS reaches through the `/var` → `/private/var` symlink. So
the divergence tracks **a `CARGO_HOME` path containing an unresolved symlink
component**, not relocation as such. The seal records the protocol probe's
digest and its `PATH_SENSITIVE` disposition — that is what the declared probe
measured — and `check_artifact_binding` now **requires** a diverged probe to
carry its controls and to name what the divergence tracks, refusing an
attribution with no reproducing control behind it. Owed, and deliberately not
done by this lane: `audit_artifact.sh` will report `PATH_SENSITIVE` on any
macOS host by construction until its probe resolves its own work directory,
which is a protocol amendment, not a reseal decision.

The undefined-import surface is **unchanged**: the same ten symbols, with
`.dynstr` byte-identical to the previous seal — the fee wave adds no syscall.
CU drift against `4fded7a6…` is **at most ±0.034% on every promoted route**:
every ResolutionWork and FoldBatch route drops 12 CU per fold
(`FoldBatch(12)` 929,573 → 929,429), Direct V2 Select 226,444 → 226,522
(+78, the largest promoted move either way), Direct V2 Freeze 357,876 →
357,868, the monolithic V4 row 182,859 → 182,857, and every native/occupation
row moves −2 or −5 CU. **No selected limit moves a quantum on any promoted
route and no admission flips.** The one family outside that window is
blank-bank `create_market`: v2 +7.8% (192,048 → 207,044), v3 and v4 −1.4%. It
has been the drift-heavy family since the custom-heap wave, it reverses
direction between seals, its byte-exactness and rollback assertions gate its
semantics unchanged, and **no projection quote derives from it**. The 23
`direct_v3` rows are excluded from the drift window on purpose: they are not
reproducible between runs, so their seal-to-seal movement measures the fixture
rather than the code, and it lands on the documented 1,500-CU bump quantum.
Everything in that family that is not keypair-dependent — all nine close routes
with every balance and delta, all four rollback observations, and all three
strand figures — is byte-identical to the superseded seal, re-derived from
three new logs.

**No account width moved**: the offline probe re-run at `04acf61` reproduces
the sealed probe **byte for byte**, all 38 rows and both rent metadata lines.
The wave's one new persistent family, `revenue.policy_record.v1` (156 bytes /
1,976,640), is post-probe pinned from `revenue.rs`. The declared source
closure grows 109 → 111 files (exactly `programs/solana-layout/src/revenue.rs`
and `research/batch-policy-identity/src/revenue_policy_v1.rs`). Native
full-lifecycle tests are intentionally excluded from the default feature:
running them requires the distinct non-production mock-source ELF, so they are
not smuggled into this projection.

Current `04acf61` tests pass: **26 default-feature targets, 104 tests**, plus
three further independent runs of the Direct V3 suite (9 more). The
general-clearing CU evidence now spans six UNPROMOTED measurement families
(`general_epoch`, `clear_walk`, `candidate_selection`, `entitled_clearing`,
`disagreement_exhibit`, `terminal_closure`); with the two Direct V3 families
and the revenue boundary that is **twenty same-ELF families in all, twenty-one
bank logs**. Five of them are **quoted at rung W1** (below); no live flag moves
for any of them, the reference adapter refuses all of them, and full admission
of the plane remains ember's decision, not this seal's. Direct SelectionV2
Select completes at a measured 226,522 CU and commits (V2 stays unpromoted on
its unimplemented empty-frozen lapse), every occupation-v4 monolithic profile
clears the 25%-headroom gate, and Direct V3 is measured but unpromoted.

## Walk plane, rung W1: quotes without live flags

Adopted by `docs/decisions/ADOPTED_2026-08-20.md` item 10 (rung W1 of
`REPORT_clearing-plane-promotion_2026-08-20.md` §2.1), unblocked by item 1's
freeze of `GENERAL_CLEARING_POLICY_V1` and `CANDIDATE_WINDOW_SLOTS = 1,000` —
a quote against a PROPOSED window pin would have been a quote against an
unfrozen lifecycle schedule. Those two doc comments now *say* FROZEN, as of
this wave.

`derive()` computes, for **thirty-five** general-clearing routes across
**five** measured families, the selected compute limit and keeper reward by
exactly the arithmetic every promoted family uses: `ceil(measured x 5/4)`
rounded up to the 10,000-CU quantum, priced at the 10,000-lamport base-fee cap
plus 1 lamport/CU plus the 100,000-lamport keeper tip. Rows are re-derived from
this seal's own tables on every run. Seven of the 25 routes carried over from
`4fded7a6…` move a selected limit by one quantum here —
`advance_clear_work_pass1_forty_order` up to 500,000, and six routes down.
**All 35 clear the 25%-headroom rule**; the worst is `FreezeEpoch` at 3 pages /
40 orders, **717,815 CU** (limit 900,000, reward 1,010,000 lamports), which is
64% of the 1,120,000 raw-CU admission boundary. Compute is not this plane's
problem.

**The rung's family list grew from four to five, and the rung's own honesty
rule is why.** `disagreement_exhibit.rs` — the L2 two-model exhibit, landed
after the last seal was cut — drives the *same* general-plane routes against
the *same* ELF under the *same* frozen policy, at a third book composition (13
orders, 7 slices, five entitled single crossings and one portfolio full pair),
and it prints its labels. Several of its observations are **hotter** than the
two-suite books': `AdvanceClearWork` pass 1 at **411,611 CU** against 393,207,
`EntitleSlice (single)` at 224,645 against 203,097, `SettlePage (entitled
portfolio full pair)` at 250,584 against 224,233. W1 already forbids a measured
CU field in a quoted family from going unquoted — but a family *outside* the
quoted list escapes that check entirely, which is precisely the loophole a
hotter unpublished observation slips through. So the exhibit is quoted, as ten
routes of its own. The four original families' rows are unchanged in kind,
because each already bounds its own measured composition and no other; a new
composition gets a new quote rather than silently widening someone else's.

W1 is *quotes and nothing else*, and each half of that is welded in
`require_walk_plane_w1_quotes` rather than merely written down:

- **live flags stay false.** The five families keep
  `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`, `general_clearing_walk.status`
  stays `SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP`, and `live_flags` stays
  `UNTOUCHED`. A walk family that acquires any `live*` field refuses, naming
  the W2 ids and evidence gaps that are still outstanding.
- **no keeper program consumes these quotes.** There is no runtime reward
  schedule for the plane to cover, so a W1 row is a policy row, not an
  operational promise; the block says so (`runtime_reward_schedule:
  NONE_NO_KEEPER_PROGRAM_READS_THESE_QUOTES`), and it publishes **no** path or
  lifecycle total (`path_quote: NOT_DESIGNED_NO_BOUNDED_TRANSACTION_PLAN`).
- **the rent side is NOT quoted.** All eight general-plane rows stay honest
  STOPs on the optional funding ledger and the owner-signed release edge. W1
  names those rows and prices none of them, and refuses if one stops being a
  STOP.
- **tags 60–67 get no row at all.** The `terminal_closure` family declares
  `per_route_cu: NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED` and the suite prints no
  per-route CU label; the block records that string as its own exclusion
  reason, so an invented close quote and a drifted declaration cannot part.
- **a borrowed measurement may not become a silent one.** Every walk
  transaction the exhibit measures carries the same
  `request_heap_frame(262144)` rider the `clear_walk` suite prices at 150 CU,
  and the exhibit never re-prices it — so its routes are charged the
  `clear_walk` figure, its family must declare the borrowing, and every one of
  its selected limits must still cover route plus rider.
- **W2 stays blocked** on `RENT.ACCOUNT_REFUND_UNOWNED`,
  `GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`, and
  `PROFILE.STORAGE_INVENTORY_INCOMPLETE`, plus the five section-3 evidence
  gaps. Every named id retiring refuses, so the rung is re-decided rather than
  silently upgraded.

Two honesty rules the block enforces on its own rows. **Variability is
declared**: eight routes are `BATCH_SHAPE_VARIABLE_OBSERVED_MAXIMUM_ONLY` —
`AdvanceClearWork` in both passes on all three books, and `AdvanceClearSlices`
on two — because the driver chooses how many orders, reservations, or slices
ride in one transaction. Those quotes bound the measured compositions and no
others. **Nothing measured goes unpublished**: every `_cu`/`_rows` field of a
quoted family must be consumed by a W1 route or be the one declared non-route
(the heap-frame rider). A new field, a dropped field, a new `FreezeEpoch` or
`FinalizeSelection` shape, or a duplicated shape label each refuse.

An over-boundary route is never clamped into a price: it publishes
`W1_STOP_HEADROOM_NO_QUOTE` with null limit, null fee cap, and null reward,
and drops the whole block to `STOP_HEADROOM`. The profile already had this
exact shape once — V2's Select is quoted PASS inside a family-level STOP — and
W1 is that shape applied to thirty-five routes.

## TerminalClosure (tags 60–67), carried forward

The general clearing plane's close DAG is unchanged by this wave and its
evidence is re-derived from a new log. A CLEARED epoch's machinery held
**531,652,377** lamports across 27 accounts, **531,639,600 were reclaimed** to
the exact recorded payers, **12,777 burned** at the frozen sink — exactly the
two injected donations — and the residual is exactly **1,336,320 lamports**,
the declared-permanent 64-byte batch-policy artifact and nothing else. The
LAPSED twin reclaimed all 47,167,920 it held, burned nothing, and left the
deliberately unledgered candidate pair standing at 47,738,640 lamports by
design. The suite prints no per-route CU label, so **no CU row is invented**
for any close route, and every TerminalClosure handler's frame row is
byte-for-byte what the last seal measured.

**No terminal row is reclassified `REFUNDABLE_TRANSIENT`, and the reason is
structural rather than evidentiary.** The funding ledger is **optional** at
every general-plane creating instruction, so `rent_principal_recorded` is a
property of the call and not the family (`RENT.ACCOUNT_REFUND_UNOWNED`); and
tag 60 is the only signer-gated edge in the DAG, so an abandoned zero-fill
reservation holds its page, the pot, and the epoch root open at recorded rent
cost (`GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`). The revenue record is the
one family in the tree whose ledger is *mandatory*, which is why it escapes
the first residual and carries only its own.

## Direct V3, rung V1: the syscall-era campaign (evidence-only)

Two families, `direct_v3` (all 23 CU rows) and `direct_v3_close` (the
close/rollback campaign), both `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`, both
**re-measured from scratch at this seal** against the exact `df0aece1…` ELF —
three fresh bank runs, every row re-derived from the new logs, nothing carried
forward. **No admission/quote/reward row is derived for any V3 route and
`live_v3` stays false.**

Three bank logs, not one, because **the V3 CU rows are not reproducible**: the
suite's fixture keypairs are freshly random per run and each PDA bump probe
costs 1,500 CU, so a row moves in 1,500-CU steps between runs. Each CU row is
sealed as its three-run spread. The worst observation in the whole venue is
`FreezeDirectEpochV4` at **382,795 CU**, comfortably under the 1,120,000
raw-CU admission boundary — a fact about the rows, not an admission of them.

Every close route the four blocked families have is driven and measured: the
displacing `Submit`, `Finalize`'s two unselected closes, `Settle`'s seven, all
three `Lapse` phases, and the zero/one/two `AbortUnfrozen` prefixes, each with
what every account held before it closed, the exact lamport delta on every
recorded payer and on the frozen neutral sink, and an **asserted** equality
between the two. Rollback is measured on the close routes themselves.
`direct.candidate.v3`, `direct.window.v3`, `direct.work_budget.v1`, and
`direct.reservation.v2` remain `REFUNDABLE_TRANSIENT` — still the only four
refundable rows outside ResolutionWork. `init_direct_v4_order_page` creates the
4,012-byte V4 page with no ledger and no close route, re-measured at
**28,814,401 lamports** still held after both settle and lapse in all three
bank runs (`DIRECT.ORDER_PAGE_RENT_PERSISTS`); the corrected per-epoch V3
structural strand is unchanged at **35,941,440 lamports**.

## Two blessed policy-plane changes, still re-derived

First, the CU rounding quantum is 10,000, not 50,000: every selected limit, fee
cap, and keeper reward is re-derived from `admission_math.py` under the finer
quantum, and the 5/4-headroom admission bound (measured CU at most 1,120,000
raw under the 1,400,000-CU ceiling) is unchanged. Second, batched folds are
measured and admitted: `tests/resolution_work_batch.rs` composes N singleton
Fold instructions into one transaction for N in {2, 4, 8, 12}, proves the
batched final account state byte-identical to the same folds driven one per
transaction, and proves one invalid Fold mid-batch reverts the entire
transaction to its prestate. Twelve is the largest measured batch and it admits
at 929,429 CU at this seal (selected limit 1,170,000). The
`resolution_work_batched` projection prices the fewest-transaction plan for a
32-record work item — Begin, then FoldBatch(12)+FoldBatch(12)+FoldBatch(8),
then Finalize — next to the per-transaction worst case; collapsing the
per-transaction fixed overhead cuts the payer cold outlay from 18,711,920 to
14,861,920 lamports. One honest caveat is sealed with the row: the bank harness
transports transactions in-process, so the cluster wire packet budget (1,232
bytes, which a 12-fold message exceeds) is not modeled by these measurements —
`cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`.
