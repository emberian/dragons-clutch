# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `853fecb7210b324f9701415c0b169e69d9ac71d6`. This is local build,
static artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary

- Clean detached worktree:
  `/Users/ember/jobs/dragons-clutch-r1-853fecb-reseal-worktree`
  (0 porcelain lines at `853fecb`)
- Exact Git archive: 21,575,680 bytes, SHA-256
  `60589dd96b0c30fc971299f7f70a7cb4fb8aa9ea59e2700e7f8bf94b170cb48f`
- Declared SBF closure: **106 files**, SHA-256
  `2a0148cc8f0cab891e26159bd3934a474a2ab4aaffc26214d91896e1ed7ed5eb`
- `853fecb` is `87fd342` (the T2-6 general-epoch/streaming-walk merge) plus
  the cost-lab re-pin (`67fd9de`, benchmarks only), one GOAL.md log commit,
  and exactly one evidence-lane change: the sealed account probe
  (`research/liveness-policy-profile/src/main.rs`) now enumerates the
  general-epoch deadline window as `epoch.window` — the same shape of
  probe-lane-only delta the `e58aef4` seal recorded for its lock line.
  Nothing in the SBF closure differs between `87fd342` and `853fecb`.
- The closure grows 104 → 106 files against the `fda59705…` seal's
  declaration: the two additions are exactly
  `programs/clutch-sbf/program/src/instructions/orders_batch/clear_walk.rs`
  and
  `programs/clutch-sbf/program/src/instructions/orders_batch/general_epoch.rs`
  (T2-6). `research/batch-policy-identity/src/general_clearing_v1.rs`, which
  T2-6 extends, was already inside the declaration since the T2-5 pin. The
  declared `source_paths` themselves are unchanged from the `fda59705…`
  seal — no path entered or left the closure declaration.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Pass 1/pass 2 stripped ELF: byte-identical SHA-256
  `d692954949d57db22a22daec4ff62f1eeccbe61618a33833e46310f790dd4733`,
  1,785,904 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `4f80aba767e893fe2272430c59b6cae0495963742cec8b85cdb1a5c27d4581f4`,
  1,948,520 bytes
- The independently rebuilt bank fixture (ordinary dev-tree workspace path,
  same recipe, separate target directory) is byte-identical
  `d692954949d57db2…` — a second ordinary workspace path reproducing the
  identity in this audit alone, alongside the pre-seal verification
  campaign's three-path convergence.

### The relocation and path-length findings (both honest STOP-shaped bounds)

**Relocated-Cargo-home probe: PATH_SENSITIVE at this seal.** The relocated
build produced SHA-256
`05a7cdec98b025828b818eef0bd1dd41807329882e9ef8ec8a0aa65b9f9cdd3c` at
1,786,448 bytes (+544), with 27,543 differing byte positions across 971
clusters against the canonical artifact. The mechanism is exact and small:
three registry-crate panic `Location` path strings —
`solana-address-2.6.1/src/syscalls.rs`,
`solana-account-info-3.1.1/src/lib.rs`, and
`solana-program-entrypoint-3.1.1/src/lib.rs` — render as short relative
strings when the Cargo home is the canonical `~/.cargo` and as absolute
paths under the relocated home, growing `.rodata` and shifting downstream
offsets and relocations. The canonical artifact embeds **zero** registry
path strings (its only absolute paths are the three platform-tools rust
stdlib paths baked by the toolchain build). This is a path-string effect,
not a semantic change, but it supersedes the relocation byte-identity
first measured at the `187d5ee1…` seal and repeated at `fda59705…`: those
artifacts reached no panic site in these three crates; the T2-6 wave (the
default-on custom-heap entry path and the walk's account plumbing) does.
The reproducibility claim narrows accordingly: byte-identical across
fresh target directories and ordinary workspace paths on this host, **not**
across Cargo-home locations.

**Workspace-path-length sensitivity, recorded rather than discovered late:**
the pre-seal verification campaign measured this identity at three distinct
ordinary paths (dev tree, /tmp, ~/jobs) plus fresh-target rebuilds, all
byte-identical, and found **one** pathological ~110-character workspace
path that produced a 492-byte divergence (7 clusters, one ~1,030-byte
codegen cluster early in `.text`, same total length). Builds at ordinary
path lengths are byte-identical; the job worktree used here is an ordinary
path. The prior seals' unqualified relocation byte-identity claim is
therefore replaced by this bounded statement, not silently carried
forward.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was
unused. No network, RPC, signing, deployment, submission, or
external-state mutation occurred.

## Final-LTO and direct-frame gate

- 36 backend diagnostic lines naming 28 unique symbols (same counts as the
  `fda59705…` seal; different symbol set).
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostics by lines/symbols: `clutch_batch` 11/8,
  `clutch_batch_policy_identity` 18/13, `clutch_solana_layout` 2/2, and
  `clutch_solana_reference` 5/5. Every one is nonresident.
- 915 resident text symbols at 913 addresses; all 913 addresses were
  disassembled.
- 54,001 direct `r10` references; maximum offset 4,096; zero invalid
  positive, zero, or greater-than-4,096 references. The deepest direct
  reference sits in `claim_truth::observe_outcome_mints`.
- 464 first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `first-party-frame-audit.txt`
  (retained evidence).

**T2-6 walk-handler frame verification.** The five new intents (tags
49–53) land with boxed single-decode `#[inline(never)]` helpers by
design; their measured maxima in this exact artifact:

| function | max direct `r10` | references |
| --- | ---: | ---: |
| `orders_batch::general_epoch::init_epoch` (tag 49) | 4,096 | 390 |
| `orders_batch::general_epoch::freeze_epoch` (tag 50) | 968 | 320 |
| `orders_batch::clear_walk::advance_clear_work` (tag 51) | 4,096 | 247 |
| `orders_batch::clear_walk::advance_clear_slices` (tag 52) | 4,096 | 64 |
| `orders_batch::clear_walk::complete_clear_work` (tag 53) | 4,096 | 171 |
| `clear_walk::load_clearing_plane` | 1,328 | 210 |
| `clear_walk::validate_walk_reservation` | 4,096 | 155 |
| `clear_walk::walk_batch` | 904 | 103 |
| `general_epoch::read_general_batch_policy` | 176 | 41 |
| `clear_walk::decode_body_boxed` | 16 | 6 |
| `general_epoch::decode_epoch_boxed` | 336 | 2 |
| boxed interner/checkpoint constructors | 0 | 0 |

Every walk handler is at or under the 4,096-byte line and the backend
emits no diagnostic for any of them. The Tier 0 restructuring of the ten
former opt-z overflowers persists: re-measured here, every one of the ten
(`place_order` 4,096/422, `recorded_redeem` 4,096/364, `settle_page`
4,096/172, `prepare_direct_v4_economics` 4,096/236,
`authenticate_settlement` 1,432/423, `prepare_selection_commit` 4,096/200,
`resolve_global` 4,096/620, `apply_native_market_resolution` 4,096/270,
`commit_observed_supplies` 1,080/67, `apply_legacy_market_resolution`
4,096/261) stays at or under the line, as do the T2-3 staged-creation
handlers (`create_first_stage` 4,096/206, `init_clear_work` 4,096/53,
`finish_creation` 4,096/8, `write_grow_stage` 4,096/5, `grow_clear_work`
328/58).

The backend-survivor check is authoritative alongside these direct
offsets; an offset at or below 4,096 alone is not evidence that a
nested-call warning is safe.

## ELF shape and the custom-heap entry region

ELF shape passes: three load segments, no writable-executable segment,
1,614,088-byte `.text`, entrypoint `0xD9670`, and only the reviewed
undefined imports `abort`, `sol_invoke_signed_rust`, `sol_log_`,
`sol_memcmp_`, `sol_memcpy_`, `sol_memset_`, `sol_panic_`, `sol_sha256`,
and `sol_try_find_program_address` — the identical nine-symbol surface as
the `fda59705…` and `187d5ee1…` seals; the T2-6 wave adds **no** syscall.

The entry region changed with the default-on `custom-heap` feature: the
feature suppresses the Anza `entrypoint!` macro's 32-KiB downward bump
allocator so the program's `bpf` module installs the upward bump allocator
that reaches a transaction-requested heap frame (up to 256 KiB). The
entrypoint symbol moved (`0xCEE38` → `0xD9670` across the seals) and no
new undefined dynamic symbol entered with it. The walk suite measures the
`request_heap_frame(262144)` surcharge at exactly 150 CU on an otherwise
identical transaction (450 vs 300).

Loader-v3 Program/Buffer/ProgramData sizing is 36/1,785,941/1,785,949
bytes, with 8,699,811 bytes of data-length headroom.

## Exact comparison with the superseded `fda59705…` seal

This is a **materially different artifact**, not a line-record
perturbation. The stripped ELF grows from 1,527,640 to 1,785,904 bytes
(+258,264) and 773,774 byte positions differ over the common prefix.

- `.text`: **different**, 1,414,848 → 1,614,088 bytes
- `.rodata`: **different**, 58,369 → 107,121 bytes (the walk/interner and
  general-epoch tables dominate the growth)
- `.data.rel.ro`: **different**, 14,960 → 18,848; `.rel.dyn`:
  **different**, 37,904 → 44,288; `.dynamic`, `.dynsym`: **different**
- `.dynstr` and `.shstrtab`: identical — the undefined dynamic-symbol
  *names* and section-name table are unchanged, so no new syscall entered
  the surface
- Stripped-ELF instruction disassembly: **different**, 173,097 → 197,551
  instruction lines (both measured on the stripped artifacts here)

Exact section digests and both disassemblies are retained evidence
(`comparison-fda5-vs-d692.txt`). No CU row, stack row, frame row, or
ELF-shape row from the `fda59705…` seal is carried forward; every current
row in the liveness profile was remeasured against exact `d6929549…`.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified
crates.io archives/unpacked trees, and one vendored package.
`dependencies.tsv` and `registry-source-verification.tsv` are
byte-identical to the `fda59705…` seal's — the T2-6 wave changed no
external pin.

The independently rebuilt current-bank fixture was rechecked as exact
`d6929549…` before every suite. Current `853fecb` tests pass — 13 suites,
68 tests: artifact 6/6, blank-bank 2/2, funded orders 2/2, ResolutionWork
4/4, batched folds 2/2, collateral 13/13, DirectSelectionV2 2/2,
prefund/source gate 5/5, source-archive host tests 9/9, native resolution
15/15, and the three new T2-6 suites general-epoch 3/3, clear-walk 3/3,
and clear-lifecycle 2/2. Exact fixture hashes per suite are recorded in
`same-elf-bank-linkage.txt` (retained evidence).

CU drift against the `fda59705…` seal is small and one-sided on the
measured routes, consistent with the wave adding code without touching
those routes' paths: every ResolutionWork route moved +174 to +395 CU (at
most +0.45%, on Begin), batched folds +348 to +2,088 CU (at most +0.23%,
at FoldBatch(12) = 929,105 CU), native/occupation resolve/retry/redeem
rows +281 to +445 CU (at most +0.33%), Direct V2 init +282 (+0.80%),
freeze +156, submit +361 to +401, select +497 CU (226,446, +0.22%,
completes and commits), order placement +256/+256 and cancel +121/+121
CU, withdraw +292 CU (+0.56%). Two selected limits move one 10,000-CU
quantum under the finer measurements: FoldBatch(2) 220,000 → 230,000 and
FoldBatch(12) 1,160,000 → 1,170,000 (rewards follow); no admission flips.
**One family exceeds the ±1% window and is noted honestly:** blank-bank
`create_market` rose to 210,047/214,718/210,323 CU
(+10.1%/+5.0%/+2.1% for v2/v3/v4). The rise coincides with the wave's
dispatch/genesis growth and the custom-heap entry path shared by every
transaction in the creation flow; the byte-exactness and rollback
assertions of the same suite gate its semantics unchanged, and no
projection quote derives from the create_market rows.

Three account rows genuinely moved, all re-derived by the sealed offline
probe at `853fecb`: `legacy.clear_work` is 50,054 bytes (rent 349,266,720
lamports; T2-6 inserts the 2,050-byte owner-interner region between the
checkpoint header and the codec body), `legacy.candidate` is 337 bytes
(rent 3,236,400; CandidateRecord v3 appends the 32-byte `score_digest`
that `CompleteClearWork` stamps at verification), and the probe now
enumerates the one new persistent family, `epoch.window` (84 bytes, rent
1,475,520; the general epoch's deadline-window companion created by
InitEpoch at `seeds::epoch_window_pda` — no handler closes it). The
terminal projection grows to a 45-row inventory, same 14 blocking ids.

**The walk is SBF-executed evidence only, deliberately unpromoted.** The
two new measurement families (`general_epoch`, `clear_walk`) seal the
walk's own CU evidence — InitEpoch 42,376; FreezeEpoch 232,670 (1 page, 4
orders), 476,973 (2 pages, 17 orders), 716,586 (3 pages, 40 orders);
pass-1/pass-2 advances 260,482–388,267 CU; slice passes 177,739/177,756;
CompleteClearWork 122,888/127,076 — with their bank logs
(`general_epoch.log`, `clear_walk.log`, `clear_lifecycle.log`). No
admission, quote, or reward row is derived for any walk route, no live
flag moves, and the reference adapter still refuses tags 49–53
(`UnsupportedIntent`): admission-policy treatment of the walk is a
decision for ember, not this seal. ClearWork and the candidate feed still
have no close path (TerminalClosure remains a recorded ranked blocker in
the Tier 2 plan), so their rent rows keep their standing STOPs.

Default Endow still refuses with `0x79` and byte/lamport-exact rollback
because no production source release is registered. No CU value is
invented for that log, and no mock-feature ELF evidence is mixed here.

The Direct V3 lifecycle is resident and stack-clean in this artifact and
remains classified-but-unpromoted in the terminal inventory: it still has
no measured CU row and no sealed close/rollback bank capture, so the V3
rows keep their `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED` and rent-persistence
STOPs exactly as the V3 terminal-classification merge recorded them.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-853fecb-reseal-evidence/artifact-d692954949d57db22a22daec4ff62f1eeccbe61618a33833e46310f790dd4733`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-853fecb-reseal-evidence/authoritative-audit-console.log`
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-853fecb-reseal-evidence/source-853fecb-60589dd96b0c30fc971299f7f70a7cb4fb8aa9ea59e2700e7f8bf94b170cb48f.tar`
- First-party frame table: `first-party-frame-audit.txt`; environment and
  same-ELF linkage: `environment.txt` and `same-elf-bank-linkage.txt`;
  relocated ELF: `relocated-clutch_sbf-05a7cdec.so`
- Evidence checksum ledger: `SHA256SUMS` (mirrored here as
  `upstream-SHA256SUMS`)
