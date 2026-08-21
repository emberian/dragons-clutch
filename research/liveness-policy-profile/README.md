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
- `artifacts/4fded7a67a2d8994`: the current canonical ELF, build and stack/ELF
  audit evidence, and bank logs measured against that exact ELF;
- `artifacts/e8ba31d582be3939`, `artifacts/d692954949d57db22`,
  `artifacts/fda59705ac1c1869`, `artifacts/187d5ee16f72946a`,
  `artifacts/af6bb79cc3766bd0`, `artifacts/bd20711b01828a74`, and
  `artifacts/a5725a3d8e149b2b`: the preceding historical seals, retained in
  full for audit continuity but excluded from the current projection.
  `policy.py` refuses a seal that overwrites a superseded artifact root or
  drops any of its evidence files.

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

The profile never treats Hoard principal, fees, future volume, a future
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
`d77d670` (the `966ee2c` TerminalClosure merge for the general clearing
plane, plus exactly one closure-neutral `GOAL.md` log commit).
`runtime_ref`, `evidence_ref`, and `artifact.source_ref` are that one
commit **by construction** at this seal; the previous seal carried a later
`evidence_ref` than its `runtime_ref`, which is how a harness lockfile blob
was able to drift between the two.

The seal covers **TerminalClosure (tags 60–67)** — the general clearing
plane's lifecycle end, and the first close path it has ever had. The
owner-signed post-terminal release (tag 60) plus seven permissionless
closes run the dependency DAG: exhausted receipt (61), pages (63, each live
record's reservation proven RELEASED or CONSUMED), reservation archives
(62, page-absent first), the provably empty pot (64, pages-absent first),
candidate pairs (65, the SELECTED pair only after pot and pages are gone),
checkpoints (66), and the epoch+window root (67). Every close refuses
before any byte moves, pays exactly the recorded principal to the exact
recorded payer, burns every surplus at the frozen incinerator, and
zeroes/resizes/reassigns the account. Who paid is recorded at creation: the
six creating instructions gain one **optional** trailing
`GeneralFundingLedgerV1` sibling (account tag 26, 85 bytes / 1,482,480
lamports), written in the same transition that debits the payer.

**The sealed walk, measured on a real bank against this exact ELF**
(`logs/bank/terminal_closure.log`, new UNPROMOTED family
`terminal_closure`): a CLEARED epoch's machinery held **531,652,377**
lamports across 27 accounts, **531,639,600 were reclaimed** to the exact
recorded payers, **12,777 burned** at the frozen sink — exactly the two
injected donations — and the residual is exactly **1,336,320 lamports**,
the declared-permanent 64-byte batch-policy artifact and nothing else. The
LAPSED twin reclaimed all 47,167,920 it held, burned nothing, and left the
deliberately unledgered candidate pair standing at 47,738,640 lamports by
design. The suite prints no per-route CU label, so **no CU row is invented**
for any close route.

**No terminal row is reclassified `REFUNDABLE_TRANSIENT`, and the reason is
structural rather than evidentiary.** The close routes exist, are driven,
and conserve exactly; what does not hold *unconditionally* is either
property `terminal_admission` demands of a refundable row. The funding
ledger is **optional** at every creating instruction
(`accounts.len() == N || N + 1`), every close runs through
`close_ledgered_group`, which requires it, and the lapsed walk *proves* the
unledgered state is reachable — so `rent_principal_recorded` is a property
of the call, not the family (`RENT.ACCOUNT_REFUND_UNOWNED`). And tag 60 is
the only signer-gated edge in the DAG, so an abandoned zero-fill
reservation holds its page, the pot, and the epoch root open at recorded
rent cost, with the design explicitly declining to invent a sweep right
(new id `GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`). What the seal *does*
retire is the reason those rows carried
`PROFILE.STORAGE_INVENTORY_INCOMPLETE` — no close path existed — replacing
it on `epoch.window`, `epoch.final_pot`, and `epoch.receipt` with the two
precise residuals, and keeping it on the four `legacy.*` rows for a
different and still-true reason: their cardinality is UNADMITTED.
`policy.py::require_terminal_closure_evidence` welds both halves so neither
can drift alone. The settlement blocker ledger moves with it:
`SETTLEMENT_BLOCKERS` is now exactly `[PartialFillLedger, VirtualPot]`.

Per the 2026-08-20 build-path protocol amendment
(`docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md`) the canonical identity is
the in-place double build at the canonical checkout path: pass 1/pass 2 are
byte-identical `4fded7a6…` (1,979,512 bytes, growing from 1,914,432).
**Both relocation probes diverged this cycle, and one prior claim is
retracted.** The cross-path worktree build produced
`d33bab44…` — same length, **5 `.text` bytes different at 4 sites** and no
other section touched, the exact tied-pair signature the root-cause note
predicts. The `e8ba31d5…` seal's cross-path build happened to come back
byte-identical and that was read as a property of the artifact; it was a
one-sample coincidence (the V3 campaign then observed `7fc8ba9f…` and
`47c011d2…` at two other paths). The evidence convention is now the
**observed-digest list** `cross_path_builds`, and `policy.py` refuses both
the old scalar field and any entry equal to the canonical digest. The
relocated-Cargo-home probe is **`PATH_SENSITIVE`** again: `6302d3ee…` at
1,980,064 bytes, `.rodata` larger by exactly 552 bytes carrying exactly
three absolute registry `panic::Location` paths (`solana-address`,
`solana-program-entrypoint`, `solana-account-info`) that the canonical build
renders relative — restoring the `d6929549…` finding the `e8ba31d5…` seal
believed superseded.

The undefined-import surface is **unchanged**: the same ten symbols, with
`.dynstr` byte-identical to the previous seal. CU drift against
`e8ba31d5…` is **at most ±0.005% on every promoted route** — every
ResolutionWork and FoldBatch route moves +1 to +12 CU, `FoldBatch(12)`
929,561 → 929,573, Direct V2 Select 226,445 → 226,444, the monolithic V4
row unchanged — no admission flips and no selected limit moves a quantum.
Seven of the 104 compared rows exceed ±1% and every one is in an UNPROMOTED
family or in the family from which no projection quote derives:
`entitled_clearing` (SettlePage entitled portfolio full pair +4.0%,
EntitleSlice single +3.0%, SettlePage entitled direct slice −2.7%,
EntitleSlice portfolio pair −1.1%), `clear_walk` (hottest pass-1 slot
−2.25%), `general_epoch` (portfolio placement +1.6%), and blank-bank
`create_market` v2 (−1.5%). The 23 `direct_v3` rows are excluded from that
window on purpose: they are not reproducible between runs, so their
seal-to-seal movement measures the fixture rather than the code, and it lands
exactly on the documented 1,500-CU bump quantum (largest,
`LapseUnselectedDirectV3` +8.2% = nine quanta). Everything in that family that
is not keypair-dependent — all nine close routes with every balance and
delta, all four rollback observations, and all three strand figures — is
byte-identical to the superseded seal, re-derived from three new logs.

**No account width moved**: the offline probe re-run at `d77d670`
reproduces all 38 probed rows byte- and rent-identically. The wave's one new
persistent family, `general.funding_ledger` (85 bytes / 1,482,480), is
post-probe pinned from `clearing.rs`, so the terminal inventory grows to
**48 rows and 15 blocking ids** — the two new ids being
`GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT` and
`DIRECT.ORDER_PAGE_RENT_PERSISTS`. The latter names the V3 campaign's third
stranded family out loud: `init_direct_v4_order_page` creates the 4,012-byte
V4 page with no ledger and no close route, re-measured at **28,814,401
lamports** still held after both settle and lapse in all three bank runs.
The general plane's instance of that same row *does* close (tag 63), which
is exactly why the row stays an honest STOP rather than moving either way;
the corrected per-epoch V3 structural strand is unchanged at
**35,941,440 lamports**.

The general-clearing CU evidence now spans five UNPROMOTED measurement
families (`general_epoch`, `clear_walk`, `candidate_selection`,
`entitled_clearing`, `terminal_closure`; eighteen same-ELF families in all,
nineteen bank logs). Four of them are **quoted at rung W1** (below); no live
flag moves for any of them, the reference adapter refuses all of them, and
full admission of the plane remains ember's decision, not this seal's.
Direct SelectionV2 Select completes at a measured 226,444
CU and commits (V2 stays unpromoted on its unimplemented empty-frozen
lapse), every occupation-v4 monolithic profile clears the 25%-headroom
gate, and Direct V3 is measured but unpromoted (see the rung-V1 section
below). The declared source closure grows 108 → 109 files (exactly
`orders_batch/terminal_closure.rs`). Native full-lifecycle tests are
intentionally excluded from the default feature: running them requires the
distinct non-production mock-source ELF, so they are not smuggled into this
projection.

## Walk plane, rung W1: quotes without live flags

Adopted by `docs/decisions/ADOPTED_2026-08-20.md` item 10 (rung W1 of
`REPORT_clearing-plane-promotion_2026-08-20.md` §2.1), unblocked by item 1's
freeze of `GENERAL_CLEARING_POLICY_V1` and `CANDIDATE_WINDOW_SLOTS = 1,000` —
a quote against a PROPOSED window pin would have been a quote against an
unfrozen lifecycle schedule.

`derive()` now computes, for **twenty-five** general-clearing routes across
the four measured families, the selected compute limit and keeper reward by
exactly the arithmetic every promoted family uses: `ceil(measured x 5/4)`
rounded up to the 10,000-CU quantum, priced at the 10,000-lamport base-fee cap
plus 1 lamport/CU plus the 100,000-lamport keeper tip. Rows are re-derived
from this seal's own tables on every run — the promotion report's table was
compiled against the superseded `e8ba31d5…` root, where 23 of the 25 measured
maxima and **5 of the 25 selected limits** differ from this seal's
(`PlaceOrder` single and portfolio, the forty-order pass-1 walk,
`EntitleSlice` single, and the full-pair `SettlePage`).
**All 25 clear the 25%-headroom rule** at the 10,000-CU quantum; the worst is
`FreezeEpoch` at 3 pages / 40 orders, **717,825 CU** (limit 900,000, reward
1,010,000 lamports), which is 64% of the 1,120,000 raw-CU admission boundary.
Compute is not this plane's problem.

W1 is *quotes and nothing else*, and each half of that is welded in
`require_walk_plane_w1_quotes` rather than merely written down:

- **live flags stay false.** The four families keep
  `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`, `general_clearing_walk.status`
  stays `SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP`, and `live_flags` stays
  `UNTOUCHED`. A walk family that acquires any `live*` field refuses, naming
  the W2 ids and evidence gaps that are still outstanding.
- **no keeper program consumes these quotes.** There is no runtime reward
  schedule for the plane to cover, so a W1 row is a policy row, not an
  operational promise; the block says so (`runtime_reward_schedule:
  NONE_NO_KEEPER_PROGRAM_READS_THESE_QUOTES`), and it publishes **no** path or
  lifecycle total (`path_quote: NOT_DESIGNED_NO_BOUNDED_TRANSACTION_PLAN` —
  W2 item 5).
- **the rent side is NOT quoted.** TerminalClosure gave the plane real close
  routes; the cycle-E reclassification still leaves all eight general-plane
  rows honest STOPs on the optional funding ledger and the owner-signed
  release edge. W1 names those rows and prices none of them, and refuses if
  one stops being a STOP.
- **tags 60–67 get no row at all.** The `terminal_closure` family declares
  `per_route_cu: NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED` and the suite prints no
  per-route CU label; the block records that string as its own exclusion
  reason, so an invented close quote and a drifted declaration cannot part.
- **W2 stays blocked** on `RENT.ACCOUNT_REFUND_UNOWNED`,
  `GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`, and
  `PROFILE.STORAGE_INVENTORY_INCOMPLETE`, plus the five section-3 evidence
  gaps (wider grids, full-width tie/displacement campaigns, a second
  independent bank profile, rent/close rows under a ratified R4 carve-out, and
  a freeze-to-settle path-quote model). Every named id retiring refuses, so
  the rung is re-decided rather than silently upgraded.

Two honesty rules the block enforces on its own rows. **Variability is
declared**: five routes are `BATCH_SHAPE_VARIABLE_OBSERVED_MAXIMUM_ONLY` —
`AdvanceClearWork` in both passes on both books and `AdvanceClearSlices` — because
the driver chooses how many orders, reservations, or slices ride in one
transaction, and the sealed suite drove eleven distinct pass-1 slot shapes on
the forty-order book alone (1–16 records, 0–11 reservations). Those quotes
bound the measured compositions and no others. **Nothing measured goes
unpublished**: every `_cu`/`_rows` field of a quoted family must be consumed by
a W1 route or be the one declared non-route — the walk's
`request_heap_frame(262144)` rider, measured at 150 CU, which every
`clear_walk` limit must still cover with the route. A new field, a dropped
field, a new `FreezeEpoch` or `FinalizeSelection` shape, or a duplicated shape
label each refuse.

An over-boundary route is never clamped into a price: it publishes
`W1_STOP_HEADROOM_NO_QUOTE` with null limit, null fee cap, and null reward,
and drops the whole block to `STOP_HEADROOM`. The profile already had this
exact shape once — V2's Select is quoted PASS inside a family-level STOP — and
W1 is that shape applied to twenty-five routes.

## Direct V3, rung V1: the syscall-era campaign (evidence-only)

Commissioned by `docs/decisions/ADOPTED_2026-08-20.md` item 10 (rung V1 of
`REPORT_clearing-plane-promotion_2026-08-20.md` §2.2). The Direct V3
two-order venue (tags 36–46) had **no measurement family at all** and two
unsealed syscall-era headline figures. It has two families, `direct_v3`
(all 23 CU rows) and `direct_v3_close` (the close/rollback campaign), both
`UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`. Both are **re-measured from
scratch at this seal** against the exact `4fded7a6…` ELF — three fresh bank
runs, every row re-derived from the new logs, nothing carried forward.
**No admission/quote/reward row is derived for any V3 route and `live_v3`
stays false.**

The campaign that first sealed these families ran the previous artifact
staged from its own root rather than a fresh `cargo-build-sbf`, and that
produced the finding the build-path protocol now encodes
(`docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md`): building that same
source at two paths other than the canonical checkout produced two further
distinct digests (`7fc8ba9f…` and `47c011d2…`), each differing from the
seal in the same 486 `.text` bytes and 6 `.rel.dyn` bytes. This seal
observes a third such path-digest at its own source (`d33bab44…`, 5 `.text`
bytes at 4 sites). The `PATH_TIED_SYMBOL_ORDER` disposition is real; the
`e8ba31d5…` note that its one cross-path probe came back byte-identical did
**not** generalize and is retracted above. Cross-path builds are recorded
as an observed-digest list, never as an equality claim, and `policy.py`
now refuses the shape that made the wrong reading possible.

Three bank logs, not one, because **the V3 CU rows are not reproducible**:
the suite's fixture keypairs are freshly random per run and each PDA bump
probe costs 1,500 CU, so a row moves in 1,500-CU steps between runs. Each
CU row is sealed as its three-run spread. The worst observation in the whole
venue is `FreezeDirectEpochV4` at **382,784 CU**, comfortably under the
1,120,000 raw-CU admission boundary — a fact about the rows, not an
admission of them. The `Submit replacement` row that STOPped on headroom in
the pre-syscall generation (1,127,892 CU) measures 198,960–202,097 here.

**`DIRECT.V3_CLOSE_EVIDENCE_UNSEALED` is retired**, by exactly the
measurement its own text named. Every close route the four blocked families
have is driven and measured: the displacing `Submit`, `Finalize`'s two
unselected closes, `Settle`'s seven, all three `Lapse` phases, and the
zero/one/two `AbortUnfrozen` prefixes. Each route logs what every account
held before it closed, the exact lamport delta on every recorded payer and
on the frozen neutral sink, and an **asserted** equality between the two —
`Settle` closes 27,706,854 lamports and every one of them lands on a
recorded recipient; the reservation owners recover 5,192,160 each, the exact
rent-exempt minimum of a 618-byte account. Rollback is measured on the close
routes themselves: substituting a close recipient at `Finalize` or at the
two-order `Abort` refuses and leaves the accounts byte-and-lamport
identical, as does an underfunded `Freeze` or `Submit`. Every one of those
numbers is identical across all three runs, and identical again to the
three runs the previous seal recorded. `direct.candidate.v3`,
`direct.window.v3`, `direct.work_budget.v1`, and `direct.reservation.v2`
remain `REFUNDABLE_TRANSIENT` — still the only four refundable rows outside
ResolutionWork, and the general plane's close DAG adds none, for the
reasons above. `policy.py::require_v3_close_evidence` welds the
classification to the evidence in both directions, and now also refuses a
stranding row that fails to carry its own blocking id.

One finding corrects the promotion report's rent story. It names two
structurally stranded V3 families (Epoch V4 + final policy artifact,
7,127,040 lamports). The sealed run shows a **third**: `InitOrderPageV4`
creates one 4,012-byte OrderPage per V4 epoch, records its principal, and no
V3 route closes it — re-measured at 28,814,401 lamports still held after
both settle and lapse. The honest per-epoch structural strand is therefore
**35,941,440 lamports (~0.0359 SOL)**, five times the published figure. It
now has its own blocking id, `DIRECT.ORDER_PAGE_RENT_PERSISTS`, instead of
hiding inside the generic unowned-refund one. The row already STOPs, so
nothing was over-admitted, and the projection publishes the corrected
number rather than the quoted one.

Two blessed policy-plane changes landed earlier on 2026-08-20 as one
evidence-only cycle (at the `187d5ee1…` seal) and are re-derived at this
seal. First, the CU rounding quantum is 10,000, not 50,000: every selected
limit, fee cap, and keeper reward is re-derived from `admission_math.py`
under the finer quantum, and the 5/4-headroom admission bound (measured CU
at most 1,120,000 raw under the 1,400,000-CU ceiling) is unchanged. Second,
batched folds are measured and admitted: `tests/resolution_work_batch.rs`
composes N singleton Fold instructions into one transaction for N in
{2, 4, 8, 12} (`logs/bank/resolution_work_batch.log`), proves the batched
final account state byte-identical to the same folds driven one per
transaction, and proves one invalid Fold mid-batch reverts the entire
transaction to its prestate. Twelve is the largest measured batch and it
admits at 929,561 CU at this seal (selected limit 1,170,000). The
`resolution_work_batched` projection prices the fewest-transaction plan for
a 32-record work item — Begin, then
FoldBatch(12)+FoldBatch(12)+FoldBatch(8), then Finalize — next to the
per-transaction worst case; collapsing the per-transaction fixed overhead
cuts the payer cold outlay from 18,711,920 to 14,861,920 lamports. One
honest caveat is sealed with the row: the bank harness transports
transactions in-process, so the cluster wire packet budget (1,232 bytes,
which a 12-fold message exceeds) is not modeled by these measurements —
`cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`.
