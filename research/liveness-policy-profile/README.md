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
- `artifacts/d692954949d57db22`: the current canonical ELF, build and stack/ELF
  audit evidence, and bank logs measured against that exact ELF;
- `artifacts/fda59705ac1c1869`, `artifacts/187d5ee16f72946a`,
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
`853fecb` (the `87fd342` T2-6 general-epoch/streaming-walk merge plus the
benchmarks-only cost-lab re-pin, one GOAL.md log commit, and one probe-lane
change adding the `epoch.window` row). The seal covers the T2-6 wave — the
general epoch lifecycle and on-chain streaming walk (intents 49–53: general
InitEpoch/FreezeEpoch, AdvanceClearWork with the per-order reservation
sweep and owner interner, AdvanceClearSlices, CompleteClearWork), the
default-on custom-heap upward bump allocator, CandidateRecord v3 (337
bytes, score digest), the 2,050-byte ClearWork interner region, the
84-byte EpochWindowAccount, and refusal codes `0x0090`–`0x0092`. The
stripped ELF grows from `1,527,640` to `1,785,904` bytes and no section
except `.dynstr` and `.shstrtab` is byte-identical, so this is a
materially different artifact and no old CU row was reused as
current-artifact evidence; every measured row was rerun against exact
`d6929549…`. CU drift against `fda59705…` on the measured routes is at
most +0.8% per route except blank-bank `create_market`
(+10.1%/+5.0%/+2.1% for v2/v3/v4, noted in the audit); no admission
flips, and two selected limits move one 10,000-CU quantum (FoldBatch(2)
220,000 → 230,000, FoldBatch(12) 1,160,000 → 1,170,000, rewards
following). Three account rows genuinely moved, all re-derived by the
sealed offline probe: `legacy.clear_work` 50,054 bytes / 349,266,720
lamports (the interner region), `legacy.candidate` 337 bytes / 3,236,400
(the v3 score digest), and the new `epoch.window` 84 bytes / 1,475,520
(created by InitEpoch, closed by no handler; the terminal inventory grows
to 45 rows, same 14 blocking ids). The walk's own CU evidence is sealed
as two new UNPROMOTED measurement families (`general_epoch`,
`clear_walk`, twelve same-ELF families in all, three new bank logs): no
admission, quote, or reward row is derived for any walk route, live flags
are untouched, and admission-policy treatment of the walk is ember's
decision, not this seal's. Direct SelectionV2 Select completes at a
measured 226,446 CU and commits (V2 stays unpromoted on its unimplemented
empty-frozen lapse), every occupation-v4 monolithic profile clears the
25%-headroom gate, and Direct V3 remains resident but unmeasured, so no
V3 CU row enters the projection. The relocated-Cargo-home probe is
**path-sensitive at this seal**: three registry-crate panic-location
strings (solana-address, solana-account-info, solana-program-entrypoint)
render relative at the canonical home and absolute at a relocated one,
superseding the two prior seals' relocation byte-identity — the audit
quantifies the divergence and the recorded workspace-path-length bound;
ordinary-path rebuilds (including the independently rebuilt bank fixture)
stay byte-identical. The declared source closure grows 104 → 106 files
(exactly the two T2-6 instruction modules). Native full-lifecycle tests
are intentionally excluded from the default feature: running them
requires the distinct non-production mock-source ELF, so they are not
smuggled into this projection.

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
admits at 929,105 CU at this seal (selected limit 1,170,000). The
`resolution_work_batched` projection prices the fewest-transaction plan for
a 32-record work item — Begin, then
FoldBatch(12)+FoldBatch(12)+FoldBatch(8), then Finalize — next to the
per-transaction worst case; collapsing the per-transaction fixed overhead
cuts the payer cold outlay from 18,711,920 to 14,861,920 lamports. One
honest caveat is sealed with the row: the bank harness transports
transactions in-process, so the cluster wire packet budget (1,232 bytes,
which a 12-fold message exceeds) is not modeled by these measurements —
`cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`.
