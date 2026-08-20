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
- `artifacts/e8ba31d582be3939`: the current canonical ELF, build and stack/ELF
  audit evidence, and bank logs measured against that exact ELF;
- `artifacts/d692954949d57db22`, `artifacts/fda59705ac1c1869`,
  `artifacts/187d5ee16f72946a`, `artifacts/af6bb79cc3766bd0`,
  `artifacts/bd20711b01828a74`, and `artifacts/a5725a3d8e149b2b`: the
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
`2dbc9fc` (the `6e4702a` T2-8 entitlement/settlement merge — itself after
the `8fe5f9e` T2-7 selection merge — plus three closure-neutral commits:
the build-path root-cause note and protocol amendment, one GOAL.md log
commit, and the audit-gate `sol_memmove_` review). The seal covers the
T2-7/T2-8 wave that completes Tier 2 end to end — candidate submission,
retention, and selection (tags 54–57: SubmitCandidate,
WriteCandidateFeed, SealCandidate with top-3 displacement,
FinalizeSelection with the full-width digest tiebreak and the honest
0-verified lapse), EpochWindow v2 (231 bytes: deadline, live cardinality,
retained registry, selection result), the CandidateFeedStage staging
prefix (tag 25, the feed account mid-write, not a new family), and the
entitlement freeze plus generalized consumption (tags 58–59:
FreezeEntitlement creating the epoch's FinalPot at `pot_pda`,
EntitleSlice creating per-(candidate, slice) SettlementReceipt
entitlements at `receipt_pda`, reservations `ACTIVE → ENTITLED →
CONSUMED` archive, and SettlePage widened to the entitled direct-slice
and portfolio full-pair shapes — a portfolio pair settles with exact
conservation in bank evidence). Per the 2026-08-20 build-path protocol
amendment (`docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md`) the
canonical identity is the in-place double build at the canonical checkout
path: pass 1/pass 2 are byte-identical `e8ba31d5…` (1,914,432 bytes,
growing from `1,785,904`), the one cross-path worktree build is recorded
as the relocation probe under disposition `PATH_TIED_SYMBOL_ORDER`
(observed byte-identical at this seal — the path-dependence lives in the
unstripped symbol table and no hash-sorted tie survives stripping here),
and the relocated-Cargo-home probe returned byte-identical, superseding
the previous seal's registry-panic-string sensitivity for this artifact.
The undefined-import surface grows by exactly one reviewed symbol:
`sol_memmove_`, entered by LLVM lowering the portfolio full-pair copies,
shimmed by the pinned platform-tools compiler-builtins — the audit gate
refused it first and the review commit admitted exactly it. CU drift
against `d6929549…` is within ±0.1% on every promoted route (no admission
flips, no selected limit moves a quantum); three families exceed the ±1%
window and are flagged in the audit: blank-bank `create_market`
(−7.1%/+0.005%/+1.4% for v2/v3/v4, reversing most of the prior seal's
+10.1% v2 rise), the unpromoted general-epoch single placement (−1.5%),
and unpromoted clear-walk pass-1 slot observations (up to +3.1%). Account
rows re-derived by the sealed offline probe: `epoch.window` moves 84 →
231 bytes / 2,498,640 lamports (v2), and the two new T2-8 general-plane
families are classified post-probe with layout-crate byte pins —
`epoch.final_pot` 262 bytes / 2,714,400 (one per epoch, created
`POT_PHASE_CLOSED` with provably zero scalars) and `epoch.receipt` 217
bytes / 2,401,200 (at most 416 per selected candidate); neither has any
close path (TerminalClosure stands in the settlement blocker ledger), so
the terminal inventory grows to 47 rows — 13 blocking ids after the Direct
V3 close campaign below retired `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`. The
general-clearing CU evidence now spans four UNPROMOTED measurement
families (`general_epoch`, `clear_walk`, `candidate_selection`,
`entitled_clearing`; sixteen same-ELF families in all, eighteen bank
logs): no admission, quote, or reward row is derived for any tag-49–59
route, live flags are untouched, the reference adapter refuses all of
them, and admission-policy treatment of the plane is ember's decision,
not this seal's. Direct SelectionV2 Select completes at a measured
226,445 CU and commits (V2 stays unpromoted on its unimplemented
empty-frozen lapse), every occupation-v4 monolithic profile clears the
25%-headroom gate, and Direct V3 is measured but unpromoted — its rows are
sealed as two UNPROMOTED families and no V3 admission row enters the
projection (see the rung-V1 section below). The declared source closure grows
106 → 108 files (exactly the two T2-7/T2-8 instruction modules). Native
full-lifecycle tests are intentionally excluded from the default feature:
running them requires the distinct non-production mock-source ELF, so
they are not smuggled into this projection.

## Direct V3, rung V1: the syscall-era campaign (evidence-only)

Commissioned by `docs/decisions/ADOPTED_2026-08-20.md` item 10 (rung V1 of
`REPORT_clearing-plane-promotion_2026-08-20.md` §2.2). The Direct V3
two-order venue (tags 36–46) had **no measurement family at all** and two
unsealed syscall-era headline figures. It now has two families,
`direct_v3` (all 23 CU rows) and `direct_v3_close` (the close/rollback
campaign), both `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`, both bound to this
same `e8ba31d5…` ELF. **No program source moved, no admission/quote/reward
row is derived for any V3 route, `live_v3` stays false**, and the
`evidence_ref` alone advances (`runtime_ref` and the artifact are frozen) —
the same evidence-only shape the batched-fold cycle used.

The campaign drove the **sealed** `clutch_sbf.so` from this artifact root,
staged into `svm-tests/tests/fixtures/` and hash-verified, rather than a
fresh `cargo-build-sbf`. That is not a shortcut, it is a finding worth
recording against the build-path protocol
(`docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md`): building this same
source at two paths other than the canonical checkout produced two further
distinct digests (`7fc8ba9f…` and `47c011d2…`), each exactly 1,914,432
bytes, each differing from the seal in the same 486 `.text` bytes and 6
`.rel.dyn` bytes with `.rodata` byte-identical. The
`PATH_TIED_SYMBOL_ORDER` disposition is real, but the sealed note that the
one cross-path probe came back byte-identical does not generalize, and the
divergence is in `.text`, not only in the unstripped symbol table. This is
an **unsealed campaign observation**, not a new reproducibility row; it is
recorded here so nobody reads a rebuilt worktree ELF as this artifact.

Three bank logs, not one, because **the V3 CU rows are not reproducible**:
the suite's fixture keypairs are freshly random per run and each PDA bump
probe costs 1,500 CU, so a row moves in 1,500-CU steps between runs. Each
CU row is sealed as its three-run spread. The worst observation in the whole
venue is `FreezeDirectEpochV4` at **390,272 CU**, comfortably under the
1,120,000 raw-CU admission boundary — a fact about the rows, not an
admission of them. The `Submit replacement` row that STOPped on headroom in
the pre-syscall generation (1,127,892 CU) measures 203,585–209,440 here.

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
numbers is identical across all three runs. `direct.candidate.v3`,
`direct.window.v3`, `direct.work_budget.v1`, and `direct.reservation.v2` are
therefore `REFUNDABLE_TRANSIENT`; the terminal inventory keeps 47 rows and
drops to 13 blocking ids, and the protocol terminal result is still STOP.
`policy.py::require_v3_close_evidence` welds the classification to the
evidence in both directions, so neither can drift alone.

One finding corrects the promotion report's rent story. It names two
structurally stranded V3 families (Epoch V4 + final policy artifact,
7,127,040 lamports). The sealed run shows a **third**: `InitOrderPageV4`
creates one 4,012-byte OrderPage per V4 epoch, records its principal, and no
V3 route closes it — measured at 28,814,401 lamports still held after both
settle and lapse. The honest per-epoch structural strand is therefore
**35,941,440 lamports (~0.0359 SOL)**, five times the published figure. The
row already STOPs on its own blockers, so nothing was over-admitted, and the
projection publishes the corrected number rather than the quoted one.

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
