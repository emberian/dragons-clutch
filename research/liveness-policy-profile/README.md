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
the terminal inventory grows to 47 rows, same 14 blocking ids. The
general-clearing CU evidence now spans four UNPROMOTED measurement
families (`general_epoch`, `clear_walk`, `candidate_selection`,
`entitled_clearing`; fourteen same-ELF families in all, fifteen bank
logs): no admission, quote, or reward row is derived for any tag-49–59
route, live flags are untouched, the reference adapter refuses all of
them, and admission-policy treatment of the plane is ember's decision,
not this seal's. Direct SelectionV2 Select completes at a measured
226,445 CU and commits (V2 stays unpromoted on its unimplemented
empty-frozen lapse), every occupation-v4 monolithic profile clears the
25%-headroom gate, and Direct V3 remains resident but unmeasured, so no
V3 CU row enters the projection. The declared source closure grows
106 → 108 files (exactly the two T2-7/T2-8 instruction modules). Native
full-lifecycle tests are intentionally excluded from the default feature:
running them requires the distinct non-production mock-source ELF, so
they are not smuggled into this projection.

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
