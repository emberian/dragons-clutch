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
- `artifacts/fda59705ac1c1869`: the current canonical ELF, build and stack/ELF
  audit evidence, and bank logs measured against that exact ELF;
- `artifacts/187d5ee16f72946a`, `artifacts/af6bb79cc3766bd0`,
  `artifacts/bd20711b01828a74`, and `artifacts/a5725a3d8e149b2b`: the
  preceding historical seals, retained in
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
`e58aef4` (the `b1b4369` T2-3 staged-ClearWork merge plus the audit-closure
declaration and one probe lock line). The seal covers the 2026-08-20 wave —
frame Tier 0 (ten opt-z overflowers restructured with boxed-decode helpers,
all measured at or under the 4,096-byte line in this exact ELF), Tiers 1/3
(host-only), the T2-1 ClearWork checkpoint codec, T2-2 projection, T2-4
live-cardinality binding, T2-5 policy profile, T2-3 staged creation
(intents 47/48), and the Direct V3 terminal classification (44 rows, 14
blocking ids, unchanged here). The stripped ELF grows from `1,420,608` to
`1,527,640` bytes and no section except `.dynstr` and `.shstrtab` is
byte-identical, so this is a materially different artifact and no old CU
row was reused as current-artifact evidence; every measured row was rerun
against exact `fda59705…`. CU drift against `187d5ee1…` is at most ±0.2%
per route except blank-bank `create_market` (-7.3%/-1.4%/-1.4% for
v2/v3/v4, noted in the audit); no admission, limit, or reward quote flips.
The one account row that genuinely moved is `legacy.clear_work`:
T2-1 re-pins its body to the codec's `ENCODED_BYTES`, so the probe
re-derives 48,004 bytes / 334,998,720 lamports. Direct SelectionV2 Select
completes at a measured 225,949 CU and commits (V2 stays unpromoted on its
unimplemented empty-frozen lapse), and every occupation-v4 monolithic
profile clears the 25%-headroom gate. Direct V3 is resident but unmeasured
here, so no V3 CU row enters the projection. The relocated-Cargo-home
probe stays byte-identical to the canonical artifact (single host, no
cross-host claim). This seal also closes the recorded closure gap:
`research/batch-policy-identity` is now inside `audit_artifact.sh`'s
declared source closure (94 files at the prior seal's commit; 98 under the
old declaration at `e58aef4`; 104 declared and digested now), and the
evidence drift gate pins `crates/clutch-batch/src` and
`research/batch-policy-identity/src` alongside the other runtime sources.
Native full-lifecycle tests are intentionally excluded from the
default feature: running them requires the
distinct non-production mock-source ELF, so they are not smuggled into this
projection.

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
admits at 927,017 CU at this seal (selected limit 1,160,000). The
`resolution_work_batched` projection prices the fewest-transaction plan for
a 32-record work item — Begin, then
FoldBatch(12)+FoldBatch(12)+FoldBatch(8), then Finalize — next to the
per-transaction worst case; collapsing the per-transaction fixed overhead
cuts the payer cold outlay from 18,711,920 to 14,841,920 lamports. One
honest caveat is sealed with the row: the bank harness transports
transactions in-process, so the cluster wire packet budget (1,232 bytes,
which a 12-fold message exceeds) is not modeled by these measurements —
`cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`.
