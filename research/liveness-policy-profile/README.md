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
- `artifacts/0d52c561909cedef`: the current canonical ELF, build and stack/ELF
  audit evidence, and bank logs measured against that exact ELF;
- `artifacts/df0aece1e241951b`, `artifacts/4fded7a67a2d8994`,
  `artifacts/e8ba31d582be3939`, `artifacts/d692954949d57db22`,
  `artifacts/fda59705ac1c1869`, `artifacts/187d5ee16f72946a`,
  `artifacts/af6bb79cc3766bd0`, `artifacts/bd20711b01828a74`, and
  `artifacts/a5725a3d8e149b2b`: the nine preceding historical seals, retained
  in full for audit continuity but excluded from the current projection.
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
# Separate, opt-in current-tree overlay; never rewrites sealed evidence.json:
python3 policy.py --current-tree-fold4 \
  inflight/record-dense-fold4-current-tree.json
# Only after changing deterministic policy arithmetic or its declared inputs:
python3 policy.py --write-projection
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
`916731b` (the published, reachable commit; the seal originally recorded a
pre-rewrite twin `9cbc835` reachable from no ref — the cycle-G attestation
found the orphan and this pin was corrected to the twin whose tree carries
the identical closure). `runtime_ref`, `evidence_ref`, and
`artifact.source_ref` are that one commit at this seal.

Three commits carry the identity and only the first is inside the closure.
`c55f471` landed an uncommitted `cargo fmt` result the wave had left in the
working tree — format-only, but `research/batch-policy-identity` is inside the
closure and reflow moves line numbers, which reach `.rodata` through
`core::panic::Location`, so it forks the identity on its own (precedent
`9c371fe`, and cycle F's own housekeeping commit). `42948f4` and `846afab`
touch only `svm-tests`, which is not in the closure: the closure digest is
byte-identical at all three, and the audit was re-run at `42948f4` to confirm
the ELF rather than assume it.

The seal covers **fee plumbing to the boundary and nothing past it**
(`docs/decisions/ADOPTED_2026-08-20.md` items 6, 8, and 9): the
`RevenuePolicyV1` frozen const family, the per-Realm `RevenuePolicyRecordV1`
account (tag 27, 156 bytes / 1,976,640 lamports) written inside `InitRealm`
with a **mandatory** funding-ledger sibling, `CloseRevenuePolicyRecord`
(tag 68), the treasury-Position path, the carry fields, and the fee-bearing
sibling policy const. **Both rates stay zero and no fee-bearing epoch admits.**

This seal covers far more than that boundary, because it is the **batched
cycle-G reseal** the wave deferred to. Eleven landings sit between `df0aece1…`
and here — partial fills and reservation v3, the realized rounding pot, the
virtual split and merge, the moment cone bound on chain, the composite fee
arithmetic, the v2 source generation at tags 70–73, the keeper, and the
operator bench — and the tree deliberately took no per-wave reseal for any of
them. The `.spw` canon recorded that gap as an open `seal_lag` discrepancy
rather than smoothing it, marked *open until cycle G*. This is cycle G, and it
closes it.

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
**`SETTLEMENT_BLOCKERS` is empty**, and `RETIRED_SETTLEMENT_BLOCKERS` stands at
ten. The wave retired the last four — `PartialFillLedger`,
`RoundingPotRealization`, `VirtualPot`, `VirtualMergeCredit` — and two of those
four were *born* in it, filed as new rows when a retiring row's residual turned
out to be a missing settlement fact rather than a narrowing.

**Empty is not "nothing refuses", and the difference is the ledger's grammar.**
A missing settlement join is a row. An implemented join whose *admission* is
narrower than the relation's is a **recorded residual** of the row that
implemented it, written down where it was created, with the coincidence that
would close it checked rather than assumed. What is *authority-gated* rather
than unimplemented stays off the list too: the pinned
`GENERAL_CLEARING_POLICY_V1` is fee-free, so every seam requires
`max_fee_atoms == 0` and answers `AuthorizationUnavailable` otherwise — a fee
plane needs a frozen fee base and a named recipient, which is a policy fact and
not a settlement one. A new row belongs here only when a *settlement* fact is
found missing again. Three residuals stand: the per-owner conversion
coincidence (checked as `distinct_owners == filled_order_count`), the merge's
ordering rule, and the two terminal rent residuals this profile already
carries as `RENT.ACCOUNT_REFUND_UNOWNED` and
`GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`.

Per the 2026-08-20 build-path protocol amendment
(`docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md`) the canonical identity is
the in-place double build at the canonical checkout path: pass 1/pass 2 are
byte-identical `0d52c561…` (2,149,672 bytes, growing from 1,986,104), and the
whole double build plus probe was executed twice, at `c55f471` and again at
`42948f4`, with the same digest and the same dispositions both times. The
cross-path worktree build produced `468b286a…` — same length, **483 `.text`
bytes at 195 sites and 6 `.rel.dyn` bytes at 3 sites**, no other section
touched: the tied-pair signature again, at almost exactly the width the last
seal saw (481 bytes, the same 195 sites) on a program 8% larger. `cross_path_builds` stays an observed-digest list
and `policy.py` refuses both the retired scalar field and any entry equal to
the canonical digest.

**The relocated-Cargo-home probe reports `INDEPENDENT`, and that is the
amendment landing.** Cycle F reproduced the divergence three seals had called
`PATH_SENSITIVE` and then narrowed its attribution with two hand-run controls:
same recipe, same fresh extraction, differing only in *where the relocated home
sits*, both reproducing the canonical bytes — including one inside the same
temporary filesystem in resolved `/private/var` form. The divergence tracked **a
`CARGO_HOME` path containing an unresolved symlink component**, not relocation
as such, and cycle F recorded amending the probe as owed rather than making a
protocol change inside a reseal lane.

The amendment has since landed, and this is the first seal to run it. The probe
resolves its relocated home before using it, and the build comes back
`0d52c561…` — **byte for byte the canonical artifact**, no `.rodata` growth, no
absolute registry path anywhere in it, reproduced at both of this seal's audit
runs. Cycle F's narrower attribution is now confirmed by the protocol probe
itself rather than by controls beside it.

The claim this supports stays small: the recipe is independent of *where the
Cargo home sits*, on this host, when the path is given in resolved form. The
`.rodata` mechanism is unchanged — an unresolved symlink component will still
fork the bytes, which is exactly why the probe resolves it. The controls
apparatus in `check_artifact_binding` is kept and is now exercised by the tests
against a synthetic diverged probe, because a gate that stops running on live
evidence is a gate that rots.

The undefined-import surface is **unchanged**: the same ten symbols, with
`.dynstr` byte-identical to the previous seal — this wave adds no syscall
either. CU drift against `df0aece1…` is **at most +0.34% on every promoted
route**, and it is one-directional: every ResolutionWork and FoldBatch route
gains 0.2–0.34% (`FoldBatch(12)` 929,429 → 932,057, `Fold(4)` 95,710 → 96,031,
`Begin` 91,345 → 91,542, `Finalize` 164,718 → 164,892, `Abort` +3 CU). **Exactly
one promoted selected limit moves a quantum**: `Fold(4)` 120,000 → 130,000, on a
+321-CU measurement that crossed a rounding boundary. No admission flips.

The one family outside that window is blank-bank `create_market`, which has
been the drift-heavy family since the custom-heap wave and reverses direction
between seals: v2 **207,044 → 195,232 (−5.7%)**, v4 210,320 → 204,508 (−2.8%),
v3 211,715 → 211,903 (+0.1%). It reverses the sign of the move the last seal
recorded (v2 was +7.8% then). **No projection quote derives from the
create_market rows**, and the byte-exactness and rollback assertions of the same
suite gate its semantics unchanged.

**Three account widths moved, and no new persistent family entered the tree.**
The offline probe re-run at this commit differs from the sealed one in exactly
three rows: `order.reservation.v1` **570 → 618 bytes** (schema generation v3,
**version byte 4** — the PartialFillLedger wave made the account the cumulative
consumption ledger and the VirtualMergeCredit wave split quantity from cash),
and both epoch accounts gain exactly one byte for `basis_degree`, the moment
cone's on-chain binding: `legacy.epoch.v2` 328 → 329 and `direct.epoch.v3`
344 → 345. Rent follows each width. The probe emits one new line,
`artifact.maximum.stage`, which is a **derived maximum** over the artifact
stage rows and equal to `artifact.terms.stage` exactly — `check_rent_and_accounts`
verifies it as that alias rather than admitting it as a row. The terminal
inventory therefore takes **no new row**.

The declared source closure grows **111 → 129 files**: the v2 source generation
(`source_v2.rs` and its four modules, `source_identity.rs`,
`source_generation.rs`, `source_archive_v2.rs`, `instructions/source_ingest_v2.rs`,
`instructions_sysvar.rs`, `loader_state.rs`, `pyth_receiver.rs`), the composite
fee arithmetic, and the moment-cone tables. Native full-lifecycle tests are
intentionally excluded from the default feature: running them requires the
distinct non-production mock-source ELF, so they are not smuggled into this
projection.

Current `846afab` tests pass: **41 default-feature targets, 156 tests**, zero
failures, in one locked pass under the suite spinlock, plus three further
independent runs of the Direct V3 suite (9 more). The general-clearing CU
evidence now spans seven UNPROMOTED measurement families (`general_epoch`,
`clear_walk`, `candidate_selection`, `entitled_clearing`,
`disagreement_exhibit`, **`scale_clearing`**, `terminal_closure`); with the two
Direct V3 families and the revenue boundary that is **twenty-one same-ELF
families in all**, and every suite's log — quoted or not — is sealed in the
artifact root. **Six** of them are quoted at rung W1 (below); no live flag moves
for any of them, the reference adapter refuses all of them, and full admission
of the plane remains ember's decision, not this seal's. Direct SelectionV2
Select completes at a measured 227,464 CU and commits (V2 stays unpromoted on
its unimplemented empty-frozen lapse), every occupation-v4 monolithic profile
clears the 25%-headroom gate, and Direct V3 is measured but unpromoted.

**Sealed does not mean quoted.** Sixteen of the sealed logs are recorded
evidence only — `cone_gate`, `r2_v2_wire`, `vpot_split`, `vpot_merge`,
`pot_position_close`, `clear_work_creation`, `degree_terms_admission`,
`joined_lifecycle`, the two `r2_pull_*` suites, `token_leg`, the two
`coupled_*` suites, `source_ingest`, and both `native_*` preflight suites. They
ran green against this exact ELF in the same pass, their logs are tracked so
the run is reproducible from the tree, and **no CU row, quote, or reward is
derived from any of them**. A log in the evidence set is a record that the
suite ran, not a promotion.

## Walk plane, rung W1: quotes without live flags

Adopted by `docs/decisions/ADOPTED_2026-08-20.md` item 10 (rung W1 of
`REPORT_clearing-plane-promotion_2026-08-20.md` §2.1), unblocked by item 1's
freeze of `GENERAL_CLEARING_POLICY_V1` and `CANDIDATE_WINDOW_SLOTS = 1,000` —
a quote against a PROPOSED window pin would have been a quote against an
unfrozen lifecycle schedule. Those two doc comments now *say* FROZEN, as of
this wave.

`derive()` computes, for **one hundred and seven** general-clearing routes
across **six** measured families, the selected compute limit and keeper reward
by exactly the arithmetic every promoted family uses: `ceil(measured x 5/4)`
rounded up to the 10,000-CU quantum, priced at the 10,000-lamport base-fee cap
plus 1 lamport/CU plus the 100,000-lamport keeper tip. Rows are re-derived from
this seal's own tables on every run.

**The worst route is no longer `FreezeEpoch` at three pages.** It is
`scale_freeze_epoch_4pages_64orders` — the maximum 64-order book across four
dense pages — at **988,469 CU** (limit 1,240,000, reward 1,350,000 lamports),
which is 88% of the 1,120,000 raw-CU admission boundary. All 107 clear the
25%-headroom rule and the block is `PASS`, but the margin at the maximum book
is 11% of the 1,400,000 ceiling rather than the 36% the three-page book left.
Compute is no longer *comfortably* not this plane's problem; it is not this
plane's problem *yet*.

**The rung's family list grew from five to six, and the rung's own honesty
rule is why — for the second seal running.** At `df0aece1…` the joiner was
`disagreement_exhibit.rs`, whose third book composition measured several routes
hotter than the two sealed books. Here it is the six **scale campaigns**, and
the gap they exposed is much larger than a few percent.

The campaigns drive the same general-plane routes against the same ELF under
the same frozen policy, at shapes the sealed books never reached: the maximum
64-order book across four dense pages, thirty partial fills across two, a
twelve-completion rounding pot, three concurrent epochs, the complete 64-tick
table, and a sixteen-deep tied candidate field against three retained. They
print 399 labelled CU rows.

**What they found is that a page count is not a nuisance parameter.** The
sealed `entitle_slice_single` row is **207,315 CU** and its suite's epoch is
*one page*. The same instruction measures **416,385** at two pages and
**759,892** at four, because `EntitleSlice` is the page-set-wide route: it must
be presented with the whole bound page set and re-derives the live orders by
walking every page in it. A flat quote for that route was not a slightly stale
number — it was a quote for a different transaction, understating the real one
**3.7-fold** at the maximum book.

So the shape coordinate now lives **in the route key**. The 399 rows collapse
into 64 (route, shape) groups, each quoted as `scale_<route>_<coordinate>` with
variability `SHAPE_LABELLED_BY_THE_ROUTE_KEY`; the group's maximum bounds every
observation at that shape and no other. There is deliberately no combined
`entitle_slice` row. These routes are *generated from the tables* rather than
hand-listed — unlike the four original families, whose keys are prose and few —
so a shape the campaigns start driving becomes a published quote automatically
instead of waiting to be noticed, while an undeclared table, a duplicated
shape, or a row missing its coordinate each refuse.

**Every scale row is ledgered, and that is why they are the quotable ones.**
Each created account carries its optional `GeneralFundingLedgerV1` sibling. An
account created *without* one records no payer, so no close route will ever
guess it — the keeper found that at the `init_epoch` row's own 60,000-CU limit
the ledgered InitEpoch exhausted its meter at 59,850 on a real validator and
died. The unledgered rows the four original families carry are **kept and
labelled** as the non-closeable variant rather than dropped: they are what
those suites measured, and the unledgered shape is real — it is simply one
whose rent no close route can return, which is the standing
`RENT.ACCOUNT_REFUND_UNOWNED` residual.

`entitled_clearing` also gained eight routes, and not from the campaigns: the
partial-fill wave added eight measured CU fields to that suite (inexact pot
funding, mixed legs, a fragmented buy, the four strands), and the coverage rule
refused the seal until every one of them was quoted. The original families'
rows are unchanged in kind, because each already bounds its own measured
composition and no other; a new composition gets a new quote rather than
silently widening someone else's.

W1 is *quotes and nothing else*, and each half of that is welded in
`require_walk_plane_w1_quotes` rather than merely written down:

- **live flags stay false.** The six families keep
  `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`, `general_clearing_walk.status`
  stays `SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP`, and `live_flags` stays
  `UNTOUCHED`. A walk family that acquires any `live*` field refuses, naming
  the W2 ids and evidence gaps that are still outstanding.
- **no keeper program consumes these quotes — and a keeper now exists, so
  read that precisely.** `programs/clutch-sbf/keeper` was built during this
  wave and it *does* log the W1 route it spends against, its limit, and the CU
  the bank actually charged. What it does not do is take a reward from a
  runtime schedule: there is no on-chain reward schedule for this plane to
  cover, so a W1 row remains a policy row and not an operational promise. The
  block still says `runtime_reward_schedule:
  NONE_NO_KEEPER_PROGRAM_READS_THESE_QUOTES` — no *program* reads them — and it
  still publishes **no** path or lifecycle total (`path_quote:
  NOT_DESIGNED_NO_BOUNDED_TRANSACTION_PLAN`). The keeper is a client that
  quotes itself against this table and refuses to claim a row it cannot cover;
  every shape it sends carrying an unmeasured ledger allowance is forced to
  `UNQUOTED`.
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

**What the scale evidence now covers, stated but not taken.** Two of the five
evidence gaps are substantially answered by this seal's campaigns, and saying
so is input to a promotion decision this lane does not make. No live flag moves
and the gap list is unchanged.

`WIDER_PAGE_ORDER_AND_CANDIDATE_GRIDS` is substantially covered: 4 pages /
64 orders is the layout maximum (`MAX_ORDER_PAGES`, `MAX_EPOCH_ORDERS`), and
2/30, 2/24, the complete 64-tick table, and three concurrent epochs are all
measured. **What is not covered is the portfolio form** — no campaign places a
portfolio slot, so `entitle_slice_portfolio_pair` and
`settle_page_entitled_portfolio_full_pair` still have no wide-book counterpart,
and they are among the hotter routes. `FULL_WIDTH_TIE_AND_DISPLACEMENT_CAMPAIGNS`
is substantially covered: a sixteen-deep tied field against
`MAX_RETAINED_CANDIDATES = 3`, thirteen refused tied-field positions, a
displacement against a full component-tied registry, and a 3-retained /
3-verified digest tie. The other three gaps are untouched — one needs another
host, one needs a ratified decision, and the path quote is still
`NOT_DESIGNED_NO_BOUNDED_TRANSACTION_PLAN`.

**All three blocking ids stay live.** The campaigns passing a funding ledger
everywhere shows the closeable shape exists; it does not show the unledgered
one stopped being constructible, and the ledger is still optional at every
general-plane creating instruction.

Two honesty rules the block enforces on its own rows. **Variability is
declared**: ten routes are `BATCH_SHAPE_VARIABLE_OBSERVED_MAXIMUM_ONLY` —
`AdvanceClearWork` in both passes on all three books, `AdvanceClearSlices` on
two, and the partial-fill wave's two strand routes — because the driver chooses how many orders, reservations, or slices
ride in one transaction. Those quotes bound the measured compositions and no
others. **Nothing measured goes unpublished**: every `_cu`/`_rows` field of a
quoted family must be consumed by a W1 route or be the one declared non-route
(the heap-frame rider). A new field, a dropped field, a new `FreezeEpoch` or
`FinalizeSelection` shape, or a duplicated shape label each refuse.

An over-boundary route is never clamped into a price: it publishes
`W1_STOP_HEADROOM_NO_QUOTE` with null limit, null fee cap, and null reward,
and drops the whole block to `STOP_HEADROOM`. The profile already had this
exact shape once — V2's Select is quoted PASS inside a family-level STOP — and
W1 is that shape applied to one hundred and seven routes.

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
**re-measured from scratch at this seal** against the exact `0d52c561…` ELF —
three fresh bank runs, every row re-derived from the new logs, nothing carried
forward. **No admission/quote/reward row is derived for any V3 route and
`live_v3` stays false.**

Four bank runs, not one, because **the V3 CU rows are not reproducible**: the
suite's fixture keypairs are freshly random per run and each PDA bump probe
costs 1,500 CU, so a row moves in 1,500-CU steps between runs. Each CU row is
sealed as its spread.

**That quantum is now carried in the quote model rather than confined to this
family's excuse.** `find_program_address` counts a bump down from 255 and pays
one `create_program_address` per failed attempt at 1,500 CU, so any route
deriving *m* addresses carries `sum(255 - bump_i) * 1500` CU of fixture noise —
V3 is simply where it was loudest. Every W1 row now publishes whether it rests
on a **single observation** and, when it does, that its measured maximum is
known only to within `k * 1500` CU. This widens no quote: the selected limit is
still `ceil(measured x 5/4)` rounded up. It states what the maximum is known
to, which is what a reader needs in order to decide whether one send was
enough. The disagreement exhibit's five `EntitleSlice` sends are the evidence
the term is real and show its exact shape — their gaps sit on the 1,500-CU
lattice with a 16-CU residual of genuine per-slice work on top. The worst observation in the whole venue is
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
Fold instructions into one transaction for N in {2, 4, **6**, 8, 12}, proves the
batched final account state byte-identical to the same folds driven one per
transaction, and proves one invalid Fold mid-batch reverts the entire
transaction to its prestate.

**The plan those rows compose is re-derived at this seal, because the sealed
one could not be sent.** `[12, 12, 8]` was chosen on *compute* alone — twelve is
the largest batch the 1,120,000-CU raw bound admits, and it does admit, at
932,057 CU here. Compute is not what binds a fold batch on the wire. The
keeper's `fold-wire-probe` measured the serialized message at every width and
had a real validator's `sendTransaction` agree with the serializer: **six** Fold
instructions frame at **1,216 bytes** and seven do not, at **1,347** against the
1,232-byte legacy packet budget. A twelve-fold message is **2,002 bytes**. The
sealed plan priced three transactions no keeper can submit.

Width **6** is therefore measured rather than interpolated between the 4 and 8
that bracket it — `FoldBatch(6) = 486,413 CU`, selected limit 610,000 — and the
measured Fold(1)-instruction plan is composed only of sendable widths:
`[6, 6, 6, 6, 6, 2]`, six transactions. Its external success keeper budget is
**4,490,000 lamports**, including the 230,000-lamport Begin quote. This number
is transaction-fee caps plus keeper tips; it is not an onchain reserve and does
not include rent. The formerly published 15,291,920-lamport figure merely added
10,801,920 of rent principal to that external budget and is now mechanically
labelled `INVALID_RENT_PLUS_EXTERNAL_KEEPER_BUDGET_NOT_RUNTIME_PREFUND`. The
measured rows at 8 and 12 are **kept** — they are real bank measurements of real
transactions — and labelled
`MEASURED_ON_A_BANK_BUT_OVER_THE_1232_BYTE_PACKET_BUDGET_EXCLUDED_FROM_THE_PLAN`.

The old caveat `cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY` is
**discharged and removed**: the budget is measured now, by serialization and by
transport, and the projection publishes `cluster_packet_budget_bytes: 1232`,
`maximum_sendable_batch: 6`, the superseded plan, and its reason.

The sealed `0d52c561…` projection deliberately makes no further claim. Its
record-dense plan packs six `Fold(4)` instructions — 24 records — into one
packet and needs two transactions for a 32-record item, but that sealed ELF has
no composed Fold(4) measurement. Its ingredient is measured (`Fold(4) =
96,031 CU`), yet composing per-instruction CU into a transaction total is
exactly what the batch rows exist to measure. The sealed row therefore remains
`STOP_UNMEASURED_COMPOSED_FOLD4_TRANSACTION_CU` and carries no quote.

A separate **`UNSEALED_CURRENT_TREE`** campaign closes the measurement question
without changing that sealed identity. Against the production-inert ELF then
labeled `default-empty-registry`, `a6381fbe…`, with source-closure digest
`2012201b…`, the 32-record `[6,2]` plan
measured 514,332 CU / 1,228 bytes and 171,765 CU / 704 bytes. Both packets fit
the 1,232-byte budget; the first is only four bytes below it. The same campaign
measured Begin at 76,064 CU and Finalize at 152,730 CU, proved Work/Reserve/
Resolution byte equality against eight separate Fold(4) transactions, and
proved an invalid fourth call rolls the entire six-call transaction back.

The opt-in command above verifies the current fixture, test-source hashes, and
the same conservative 129-file source closure used by the artifact audit before
deriving any row. It quotes the two measured Fold transactions at **1,090,000
lamports** total external keeper budget; the measured Begin + two Fold sends +
Finalize lifecycle totals **1,610,000 lamports**. Every row repeats
`UNSEALED_CURRENT_TREE`, the full ELF digest, and the full source-closure digest.
It neither reads the current-tree row into `evidence.json` nor promotes it:
resealing remains a separate STOP.

That unmeasured external budget is independent of the runtime economics. The
onchain minimum-deposit rule must cover the legal worst case in which every one
of 32 records succeeds in its own Fold call: 32 × 1,160,000, plus the larger
terminal reward (1,510,000), plus 10,801,920 of Work/Reserve rent principal =
**49,431,920 lamports**. No external transaction budget is included. Under the
named current-ABI execution plan — eight successful Fold(4) calls grouped as
`6 + 2` transactions — runtime pays 9,280,000 for Fold calls and 1,510,000 for
Finalize, then returns **38,641,920** to the payer (27,840,000 unused prepaid
budget plus 10,801,920 released rent). Those runtime amounts are cross-checks,
not ingredients of the external 1,090,000-lamport Fold-transaction budget.
Hoard principal and future fee revenue remain excluded from every calculation.
