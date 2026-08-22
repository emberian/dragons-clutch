# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `846afabf0e87c2069d57e8800874115b71d72521`. This is local build, static
artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

This is the **cycle-G** seal, and it is the batched one the wave deliberately
deferred to: the tree took eleven landings between `df0aece1…` and here without
moving the identity, and the canon recorded that gap as an open `seal_lag`
discrepancy rather than smoothing it. This seal closes it.

## Source boundary — in place, per the build-path protocol amendment

- **Canonical build location:** `/Users/ember/dev/dragons-clutch` itself. Per
  `docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md` the ELF identity is
  same-path-reproducible only, so the canonical identity is defined at the
  canonical checkout path, where every bank log's fixture binds. Before each
  build the declared closure was verified exactly HEAD by the audit script's
  own dirty gate.
- Exact Git archive: 37,611,520 bytes, SHA-256
  `e4883d9735a77c97365566a803e441b06fbd2347b8c05447be4cb1bea24f0770`
- Declared SBF closure: **129 files**, SHA-256
  `1e04aee979e8bba761f9cebfa98abe6212acfbe9cebc1f6bed4e1769010ecf0f`
- The closure grows **111 → 129 files** against the `df0aece1…` seal. The
  declared `source_paths` are unchanged — no path entered or left the
  declaration. The eighteen additions are the wave's own: the v2 source
  generation (`source_v2.rs` and its four modules `auth`/`crossing`/`fixtures`/
  `spec`, `source_identity.rs`, `source_generation.rs`, `source_archive_v2.rs`,
  `instructions/source_ingest_v2.rs`, plus `instructions_sysvar.rs`,
  `loader_state.rs`, and `pyth_receiver.rs`), the composite fee arithmetic
  (`relation_v1_fee_tests.rs`, `fee_composite_rate_identity.rs`, and the two
  fixture generators with their differential table), and the moment cone
  (`relation_v1_moment_cone_tests.rs`, `generate_moment_cone_tables.py`).
- **Three commits carry this identity, and only the first is inside the
  closure.** `c55f471` landed an uncommitted `cargo fmt` result the wave had
  left in the working tree — format-only, but `research/batch-policy-identity`
  is inside the closure and reflow moves line numbers, which reach `.rodata`
  through `core::panic::Location`, so it forks the identity on its own
  (precedent `9c371fe`, and the cycle-F housekeeping commit at `04acf61`).
  `42948f4` and `846afab` touch only `svm-tests`, which is **not** in the
  closure: the closure digest is byte-identical at `c55f471`, `42948f4`, and
  `846afab`, and the audit script was re-run at `42948f4` to confirm the ELF
  digest rather than assume it.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Every toolchain binary digest is unchanged from the `df0aece1…` seal.
- Pass 1/pass 2 stripped ELF, both built in place with fresh
  `CARGO_TARGET_DIR`s: byte-identical SHA-256
  `0d52c561909cedef96f571ddeca3a21e621a629be778f775dd7e0a8023956cc7`,
  **2,149,672 bytes**
- The whole double build plus the relocation probe was executed **twice**, at
  `c55f471` and again at `846afab`'s parent `42948f4`, and produced the same
  digest and the same disposition both times.

### The relocated-Cargo-home probe: `INDEPENDENT` — the amendment's first reading

This is the **first seal since the cycle-F amendment** to
`programs/clutch-sbf/audit/audit_artifact.sh`, and the disposition it now
measures is recorded here whichever way it fell. It fell the way cycle F's
controls predicted.

The amended probe resolves its relocated `CARGO_HOME` symlink before using it
(`relocated_home="$(cd "$relocated_home_raw" && pwd -P)"`). With that one
change the relocated build produced
`0d52c561909cedef96f571ddeca3a21e621a629be778f775dd7e0a8023956cc7` — **byte
for byte the canonical artifact**, at 2,149,672 bytes, with no `.rodata`
growth and no absolute registry path anywhere in it. The disposition is
`INDEPENDENT`.

| probe | relocated `CARGO_HOME` | stripped SHA-256 |
| --- | --- | --- |
| raw form (what the unamended probe used) | `/var/folders/…/T//clutch-sbf-artifact-audit.sKHkNY/cargo-home-relocated` | not built — the amendment resolves before use |
| resolved form (the amended probe) | `/private/var/folders/…/T/clutch-sbf-artifact-audit.sKHkNY/cargo-home-relocated` | `0d52c561…` **canonical** |

**What this settles.** The `d6929549…`, `4fded7a6…`, and `df0aece1…` seals all
reported `PATH_SENSITIVE` here. Cycle F showed by two hand-run controls that
the divergence tracked *a `CARGO_HOME` whose path contains an unresolved
symlink component* — which defeats the relative-path computation Cargo
otherwise performs and hands rustc absolute crate-root paths that land in
`core::panic::Location` strings — and **not** relocation as such. It recorded
amending the probe as owed, explicitly declining to make a protocol change
inside a reseal lane. The amendment has since landed, and this seal is the
first to run it: the divergence is gone, reproduced twice. Cycle F's narrower
attribution is now confirmed by the protocol probe itself rather than by
controls beside it.

The claim this supports is precise and small: **the recipe is independent of
where the Cargo home sits**, on this host, when the path is given in resolved
form. It says nothing about cross-host reproducibility, and the `.rodata`
mechanism cycle F identified is unchanged — an unresolved symlink component in
`CARGO_HOME` will still fork the bytes, which is why the probe resolves it.

### The cross-path probe: `PATH_TIED_SYMBOL_ORDER`, unchanged disposition

One build of the same commit in a detached worktree at
`/Users/ember/jobs/dragons-clutch-r1-c55f471-xpath-worktree` (fresh target
directory, same recipe) produced stripped SHA-256
`468b286a9941a495bab05e2e04f88e2e26ce9454648b330aff69fd76d9000400` — same
2,149,672 bytes, **different bytes**. The divergence is exactly the tied-pair
signature and nothing else: **483 `.text` bytes at 195 contiguous sites** and
**6 `.rel.dyn` bytes at 3 sites**, with `.rodata`, `.data.rel.ro`, `.dynstr`,
`.dynsym`, `.dynamic`, and `.shstrtab` all byte-identical.

That is the cycle-F shape almost exactly — 481 `.text` bytes at 195 sites and 6
`.rel.dyn` bytes at 3 sites there — reproduced at a program 8% larger. The
evidence convention remains the **observed-digest list**
`artifact_reproducibility.cross_path_builds`, and
`policy.py::check_artifact_binding` still refuses both the retired scalar field
and any list entry equal to the canonical digest.

**The two published build recipes agree at the canonical path.** The suite
runner's recipe (`run_svm_tests.sh`, no `--arch v0`, no `--offline --locked`)
staged a fixture whose digest is exactly `0d52c561…`, verified before the first
bank started and re-verified unchanged after the last one.

## Final-LTO and direct-frame gate

- **37** backend diagnostic lines naming **28** unique symbols — the *same
  twenty-eight functions* as the `df0aece1…`, `4fded7a6…`, and `e8ba31d5…`
  seals, modulo their hash suffixes.
- **Zero of the 28 is a `clutch_sbf` symbol.** They are eight
  `clutch_batch::relation_v1` functions, two `clutch_solana_layout`
  `OrderPageAccount` decoders, five `clutch_solana_reference` functions, and
  thirteen `clutch_batch_policy_identity` direct-lifecycle functions. The
  program crate itself draws no diagnostic.
- **Zero diagnosed symbols survive final LTO.**
- The line count moves 36 → 37, and the one added line is attributable:
  `clutch_batch::relation_v1::settle_cash` now draws **both** diagnostics — the
  "function call overwrites values in the frame" line as well as the frame
  overflow it already had — and its estimated frame grows **6,080 → 6,208
  bytes** with its excess offset **1,984 → 2,112**. That is the composite fee
  arithmetic landing in the cash settlement, and it is the only diagnostic that
  moved.
- **1,081** resident text symbols at **1,078** addresses (from 1,011 at 1,009);
  all 1,078 addresses were disassembled.
- **66,106** direct `r10` references (from 60,441); maximum offset **4,096**;
  zero invalid positive, zero, or greater-than-4,096 references. The deepest
  direct reference still sits in `claim_truth::observe_outcome_mints`,
  unchanged across four seals.
- **927** first-party resident function regions (from 924) are enumerated with
  their exact direct-reference count and maximum in `first-party-frame-audit.txt`
  (retained evidence). Zero regions of any provenance exceed 4,096.

The backend-survivor check is authoritative alongside these direct offsets; an
offset at or below 4,096 alone is not evidence that a nested-call warning is
safe.

## ELF shape and the unchanged import surface

ELF shape passes: three load segments, no writable-executable segment,
**1,960,936-byte `.text`**, entrypoint `0x11B6C0`, and exactly ten undefined
imports: `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memmove_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address`.

**The wave adds no syscall.** `.dynstr` is byte-identical to the `df0aece1…`
seal (163 bytes, the same ten names); `.dynsym` is the same 312 bytes with
different symbol values, since the entrypoint and every defined address moved
with the wave's growth (`0xFB080` → `0x11B6C0`). The audit gate's
exact-surface predicate — including its hostile self-check that the predicate
still rejects a second hash syscall — passed unmodified on the first run.

Loader-v3 Program/Buffer/ProgramData sizing is 36/2,149,709/2,149,717 bytes,
with **8,336,043** bytes of data-length headroom.

## The dependency count moved, and the linked graph did not

`dependency_packages` goes **42 → 101** at this seal. Read without care that
looks like the ELF's supply chain more than doubling. It did not, and the
distinction is worth stating precisely because the audit script's own output
does not draw it.

`audit_artifact.sh` runs `cargo metadata --manifest-path program/Cargo.toml`,
which resolves the whole **workspace**. Until this wave the workspace was the
program plus `clutch-sbf-harness`, whose dependencies were a subset of the
program's, so the metadata graph and the linked graph coincided at 42 and the
seal could report one number. The wave added a third workspace member —
`clutch-keeper`, a host-side binary — and it brings 58 host crates
(`ed25519-dalek`, `curve25519-dalek`, `rand`, `serde`/`serde_json`,
`solana-keypair`, `wincode`, and their transitive closure) that the program
never links.

Re-deriving the reachable set from the resolve graph in `metadata.json`,
excluding dev-only edges, from the program root: **the program's own graph is
still exactly 42 packages.** The 59 remaining packages in the metadata are
`clutch-keeper`, `clutch-sbf-harness`, and the 57 crates reachable only from
them. The ten-symbol import surface, byte-identical `.dynstr`, and 8% ELF
growth all agree with that.

The audit's verification is not weakened by this — every one of the 101 gets
its license checked, its locked checksum verified, and its unpacked registry
tree re-derived from the archive, so the gate now covers *more* than the ELF's
closure. What is owed is the audit script emitting both numbers instead of one
ambiguous one; see "Owed, not done here".

`dependencies.tsv`, `registry-source-verification.tsv`, and `vendor.diff` are
retained in full; `vendor.diff` is empty and the vendored tree digest is
unchanged.

## Exact comparison with the superseded `df0aece1…` seal

This is a **materially different artifact**. The stripped ELF grows from
1,986,104 to 2,149,672 bytes (+163,568) and 988,788 byte positions differ over
the common prefix.

| section | df0aece1… | 0d52c561… | verdict |
| --- | ---: | ---: | --- |
| `.text` | 1,805,656 | 1,960,936 | different |
| `.rodata` | 107,929 | 108,825 | different |
| `.rel.dyn` | 49,696 | 54,400 | different |
| `.data.rel.ro` | 21,224 | 23,912 | different |
| `.dynstr` | 163 | 163 | **identical** |
| `.dynsym` | 312 | 312 | different (values only) |
| `.dynamic` | 176 | 176 | different |
| `.shstrtab` | 72 | 72 | **identical** |

Exact section digests are retained evidence (`comparison-df0a-vs-0d52.txt`).
No CU row, stack row, frame row, or ELF-shape row from the `df0aece1…` seal is
carried forward; **every current row in the liveness profile was remeasured
against exact `0d52c561…`**, including the families no one expected to move.

## Dependency and same-ELF execution linkage

The staged bank fixture was verified as exact `0d52c561…` before every suite
and re-verified unchanged after the last one, in a single run under the
`/tmp/claude-501/suite.lock` spinlock.

Current `846afab` tests pass: **41 default-feature targets, 156 tests**, zero
failures (from 26 targets and 104 tests at `df0aece1…`), plus three further
independent runs of the Direct V3 suite (9 more) for its fresh-keypair spread.
Every suite's log is sealed in this root.

## The quote-model corrections

Two campaign findings and two keeper findings land here as changes to the
model, not as prose beside it.

### 1. `EntitleSlice` gains a page coordinate, because it is a different route at every page count

The sealed `entitle_slice_single` row was **207,315 CU** and its suite's epoch
is **one page**. The six scale campaigns drove the same instruction at wider
books:

| shape | measured CU | observations |
| --- | ---: | ---: |
| 1 page | 217,235 | 8 |
| 2 pages | 416,385 | 24 |
| **4 pages** | **759,892** | 32 |

`EntitleSlice` is the page-set-wide route: it must be presented with the whole
bound page set and re-derives the live orders by walking every page in it. A
flat quote is therefore not a slightly stale number — it is a quote for a
different transaction, understating the real one by **3.7x** at the maximum
book. `settle_page_direct` carries the same coordinate, and `freeze_epoch`
already did.

So the coordinate goes **into the route key**. The campaigns' 399 labelled rows
collapse into **64 (route, shape) groups**, each published as its own W1 route
named `scale_<route>_<coordinate>` with variability
`SHAPE_LABELLED_BY_THE_ROUTE_KEY`. There is deliberately no combined
`entitle_slice` row. The routes are *derived from the tables* rather than
hand-listed — a shape the campaigns start driving becomes a published quote
automatically instead of waiting to be noticed, and an undeclared table, a
duplicated shape, or a row with no coordinate refuses.

### 2. The 1,500-CU PDA-attempt quantum is carried in the model

`find_program_address` counts a bump down from 255 and pays one
`create_program_address` per failed attempt, measured at **1,500 CU**. A route
deriving *m* addresses carries `sum(255 - bump_i) * 1500` CU of fixture noise,
and the fixture's genesis keys are freshly random per run.

Every W1 row now publishes `single_observation` and, when true,
`measured_cu_known_to_within:
PLUS_OR_MINUS_K_TIMES_1500_CU_PDA_ATTEMPT_QUANTUM`. A row with several sends
does not carry the caveat, because the spread is the evidence — the exhibit's
five `EntitleSlice` sends differ by exactly 3,000 and 4,500 CU, two and three
quanta. **This does not widen any quote**: the selected limit still derives
from the observed maximum by the ordinary 5/4 rule. It states what the maximum
is known to, which is what a reader needs in order to decide whether one send
was enough. The two passages in `scale_tick_table.rs` that still said "roughly
1,200 CU" — contradicting the corrected figure four lines away — now say 1,500.

### 3. The ledgered shapes are the quotable ones

The keeper found that every W1 creation row was measured on machinery created
**without** its optional `GeneralFundingLedgerV1` sibling, and an account
created without one records no payer, so no close route will ever guess it. At
the `init_epoch` row's own selected limit of 60,000 the *ledgered* InitEpoch
exhausted its meter at 59,850 on a real validator and died
`ProgramFailedToComplete`.

Every `scale_clearing` row is ledgered, and the family declares it
(`funding_ledger: EVERY_CREATED_ACCOUNT_CARRIES_ITS_GENERAL_FUNDING_LEDGER_V1_
SIBLING_THE_ONLY_SHAPE_A_KEEPER_CAN_CLOSE`), welded by `policy.py`. The
unledgered rows the four original families carry are **kept and labelled** as
the non-closeable variant rather than dropped: they are what those suites
measured, and the unledgered shape is a real shape — it is simply one whose
rent no close route can ever return, which is the standing
`RENT.ACCOUNT_REFUND_UNOWNED` residual.

### 4. The fold-batch plan is re-derived under the real wire bound

The sealed fewest-transaction plan `[12, 12, 8]` was chosen on **compute
alone**. The keeper's `fold-wire-probe` measured the serialized message at
every width and had a real validator's `sendTransaction` agree with the
serializer: **six** Fold instructions frame at **1,216 bytes** and seven do not,
at **1,347** against the 1,232-byte legacy packet budget. A twelve-fold message
is **2,002 bytes**. The sealed plan priced three transactions that cannot be
submitted.

Width **6** is now measured rather than interpolated between the 4 and 8 that
bracket it — `FoldBatch(6) = 486,413 CU` — and the plan is composed only of
sendable widths. The measured rows at 8 and 12 are **kept**, because they are
real measurements of real transactions, and labelled
`MEASURED_ON_A_BANK_BUT_OVER_THE_1232_BYTE_PACKET_BUDGET_EXCLUDED_FROM_THE_PLAN`.
The projection publishes `cluster_packet_budget_bytes: 1232`,
`maximum_sendable_batch: 6`, the superseded plan, and its reason. The standing
`cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY` caveat is **discharged**:
it is measured now, by serialization and by transport.

One thing is deliberately **not** claimed. The keeper's record-dense plan packs
six `Fold(4)` instructions for 24 records in one packet and would need two
transactions for a 32-record item. Its ingredient is measured (`Fold(4) =
96,031 CU`) but the composed transaction is not, and composing per-instruction
CU into a transaction total is exactly what the batch rows exist to measure —
they show batching runs slightly *cheaper* than the sum. So the record-dense
packet is named as the shape the wire permits and **carries no quote**.

## Rung W1: 35 routes become 107, and the worst route changes

Every W1 row is re-derived from this seal's own tables. The rung's quoted
families grow from five to six: `scale_clearing` joins for the same reason
`disagreement_exhibit` joined at `df0aece1…` — it drives the same general-plane
routes against the same ELF under the same frozen policy, and a family outside
the quoted list escapes the "nothing measured goes unpublished" rule entirely.
That is exactly the loophole a 759,892-CU `EntitleSlice` would have slipped
through while the profile published 207,315 for the same instruction.

`entitled_clearing` also gains eight routes, and not from the campaigns: the
partial-fill wave added eight measured CU fields to that suite (inexact pot
funding, mixed legs, a fragmented buy, the four strands) and the coverage rule
refused the seal until every one of them was quoted.

**The worst route is no longer `FreezeEpoch` at 3 pages / 40 orders.** It is
`scale_freeze_epoch_4pages_64orders` at **988,469 CU** — the maximum 64-order
book across four dense pages — which selects a 1,240,000-CU limit against the
1,400,000 ceiling. Every one of the 107 routes clears the 25%-headroom rule and
the block is `PASS`; compute is still not this plane's problem, but the margin
at the maximum book is now 11% of the ceiling rather than 36%.

Everything else about the rung is unmoved: live flags stay `UNTOUCHED`, the
rent side is **not** quoted, tags 60–67 get no row at all, and W2 stays blocked.

## W2: what the scale evidence now covers, stated but not taken

W1 declares W2 blocked on three ids and five evidence gaps. The scale campaigns
bear directly on two of the five, and this seal states that as **input to a
promotion decision it does not make**. `WALK_PLANE_W2_EVIDENCE_GAPS` is
unchanged here and no live flag moves.

- **`WIDER_PAGE_ORDER_AND_CANDIDATE_GRIDS`** — substantially covered. The
  campaigns drove 4 pages / 64 orders (the layout maximum, `MAX_ORDER_PAGES` and
  `MAX_EPOCH_ORDERS`), 2 pages / 30 orders, 2 pages / 24 orders, the complete
  64-tick table at `MAX_GRID_TICKS`, and three concurrent epochs. What remains
  uncovered is the **portfolio** form: no campaign places a portfolio slot, so
  `entitle_slice_portfolio_pair` and
  `settle_page_entitled_portfolio_full_pair` still have no wide-book
  counterpart, and those are among the hotter routes.
- **`FULL_WIDTH_TIE_AND_DISPLACEMENT_CAMPAIGNS`** — substantially covered. A
  sixteen-deep tied field against `MAX_RETAINED_CANDIDATES = 3`, thirteen
  refused tied-field positions, a displacement against a full component-tied
  registry, and a 3-retained/3-verified digest tie all measured.
- **`SECOND_INDEPENDENT_BANK_PROFILE`**, **`RENT_AND_CLOSE_ROWS_UNDER_A_
  RATIFIED_R4_CARVE_OUT`**, and **`FREEZE_TO_SETTLE_PATH_QUOTE_MODEL`** — not
  touched. The first needs another host, the second needs a ratified decision,
  and the third is still `NOT_DESIGNED_NO_BOUNDED_TRANSACTION_PLAN`.

The three blocking **ids** are all still live and none retires here:
`RENT.ACCOUNT_REFUND_UNOWNED` (the funding ledger is still optional at every
general-plane creating instruction — the campaigns pass one everywhere, which
shows the closeable shape exists, not that the unledgered one stopped being
constructible), `GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`, and
`PROFILE.STORAGE_INVENTORY_INCOMPLETE`.

## Accounts: three widths moved, no new persistent family

The offline probe re-run at this commit does **not** reproduce the sealed rows,
and the three moves are all the wave's:

| row | df0aece1… | 0d52c561… | rent lamports |
| --- | ---: | ---: | --- |
| `order.reservation.v1` | 570 | **618** | 4,858,080 → 5,192,160 |
| `legacy.epoch.v2` | 328 | **329** | 3,173,760 → 3,180,720 |
| `direct.epoch.v3` | 344 | **345** | 3,285,120 → 3,292,080 |

The reservation is the partial-fill and virtual-merge work: schema generation
v3 at **618 bytes**, **version byte 4** (`RESERVATION_ACCOUNT_VERSION = 4`,
`RESERVATION_ACCOUNT_BYTES = 618`). Both epoch accounts grew exactly one byte
for `basis_degree`, the moment cone's on-chain binding.

The probe emits one new row, `artifact.maximum.stage` (1,792 bytes /
13,363,200 lamports). It is a **derived maximum** over the artifact stage rows,
not a new account family — it equals `artifact.terms.stage` exactly. **No new
persistent shape entered the tree at this wave**, and the terminal inventory
takes no new row.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-846afab-reseal-evidence/artifact-0d52c561909cedef96f571ddeca3a21e621a629be778f775dd7e0a8023956cc7`
- Audit consoles: `authoritative-audit-console.log` (at `c55f471`) and
  `audit-console-confirm-42948f4.log` (the confirming re-run)
- Source archive:
  `source-846afab-e4883d9735a77c97365566a803e441b06fbd2347b8c05447be4cb1bea24f0770.tar`
- First-party frame table: `first-party-frame-audit.txt`; account probe:
  `account-probe-c55f471.txt`; section comparisons:
  `comparison-df0a-vs-0d52.txt` and `comparison-canonical-vs-crosspath.txt`;
  cross-path build log: `sbf-build-crosspath.log` (sealed in-root)
- Evidence checksum ledger: `SHA256SUMS` (mirrored here as
  `upstream-SHA256SUMS`)

## Owed, not done here

- **`audit_artifact.sh` reports one dependency number for two different
  graphs.** `dependency_packages=101` is the workspace metadata graph; the
  program's linked graph is 42. They coincided until `clutch-keeper` joined the
  workspace. The script should emit both, and the linked one should be the one
  a reader sees first. Deriving it needs the resolve graph the script already
  fetches, so this is a small change — but it is a protocol change, and it
  belongs to a decision rather than to a reseal lane.
- **The record-dense fold packet is unmeasured.** Six `Fold(4)` instructions in
  one transaction is the shape the wire permits and the plan a keeper would
  actually want; `resolution_work_batch.rs` measures only single-record folds
  per instruction. Measuring it would let the 32-record plan drop from six
  transactions to two on evidence rather than on arithmetic.
- **No scale campaign places a portfolio order.** The portfolio-pair
  entitlement and full-pair settlement are quoted only at the one-page books,
  and they are among the hotter routes. That is the largest remaining hole in
  `WIDER_PAGE_ORDER_AND_CANDIDATE_GRIDS`.
- Four pre-existing rustdoc warnings remain in-closure and unrepaired:
  `FULL_RELATION_CANDIDATE_PREIMAGE` and `feed_leg` in
  `clutch-batch-policy-identity`, `CANDIDATE_FEED_TAG` and `ClearWorkAccount` in
  `clutch-solana-layout`.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was unused. No
network, RPC, signing, deployment, submission, or external-state mutation
occurred. Single-host observation, not a cross-host claim.
