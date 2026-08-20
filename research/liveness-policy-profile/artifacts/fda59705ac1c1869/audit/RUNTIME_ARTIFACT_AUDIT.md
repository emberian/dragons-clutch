# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `e58aef497a17a77d7ca00fc30db2e289a6be42a2`. This is local build,
static artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary

- Clean detached worktree:
  `/Users/ember/jobs/dragons-clutch-r1-e58aef4-reseal-worktree`
  (0 porcelain lines at `e58aef4`)
- Exact Git archive: 19,456,000 bytes, SHA-256
  `b8babaa49b9d6a2428bd2672297a3a35efd625c3f903d4b63a01182eb76cc108`
- Declared SBF closure: **104 files**, SHA-256
  `b545ed4ac82790643f4f42cd5a5f9893dc8307e21137b3c057e82c56aed606f2`
- `e58aef4` is `b1b4369` (the T2-3 staged-ClearWork merge) plus one GOAL.md
  log commit and exactly two evidence-lane changes: the audit script's
  closure declaration below, and one `research/liveness-policy-profile/
  Cargo.lock` line so the sealed account probe resolves offline (the same
  shape of lock-only delta the `d8c5034` seal recorded). Nothing in the SBF
  closure differs between `b1b4369` and `e58aef4`.
- **The recorded closure gap is closed at this seal.** Every prior audit
  documented that `research/batch-policy-identity` — the first-party path
  dependency where the `sol_sha256` policy-identity folds live — sat outside
  the audit script's declared `source_paths`, compensated only by the clean
  detached worktree. `audit_artifact.sh` now declares it, so the dirty-tree
  gate and the source-closure digest cover all eleven first-party packages
  in the linked graph. `crates/clutch-batch`, which the layout crate newly
  depends on, was checked for the same gap and found already inside the
  closure via the whole-directory `crates` entry. Exact counts: the prior
  seal declared 94 files at its own commit; the same old declaration at
  `e58aef4` selects 98 files (the night's merges added
  `orders_batch/clear_work.rs`, `solana-layout/src/projection.rs`,
  `solana-layout/tests/projection_stream.rs`, and
  `clutch-batch/src/relation_v1_stream_codec_tests.rs`); the closed
  declaration selects 104 (those 98 plus the six tracked
  `research/batch-policy-identity` files).

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Pass 1/pass 2 stripped ELF: byte-identical SHA-256
  `fda59705ac1c18692faed454fb7588a32851028eea5ecc17e00f9d3f6af309ec`,
  1,527,640 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `a321fdf92aacb90f52dc4dc14e2bdc96c4548c803d372c04acfef0f89a9cb951`,
  1,671,784 bytes
- Relocated-Cargo-home stripped ELF: SHA-256
  `fda59705ac1c18692faed454fb7588a32851028eea5ecc17e00f9d3f6af309ec` —
  **byte-identical to the canonical artifact**, unstripped likewise
  (`a321fdf9…`). The relocation byte-identity first measured at the
  `187d5ee1…` seal (when the software `sha2` crate left the SBF graph)
  **persists** through the Tier 0/1/3 and T2 wave. This remains one
  relocated path on one host: it keeps the local claim at
  relocation-independent-as-measured and makes no cross-host equality
  claim.

The build ran locally on an Apple M2 Max under macOS 26.6. GPU was unused.
No network, RPC, signing, deployment, submission, or external-state mutation
occurred.

## Final-LTO and direct-frame gate

- 36 backend diagnostic lines naming 28 unique symbols (prior seal: 42/33).
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostics by lines/symbols: `clutch_batch` 11/8,
  `clutch_batch_policy_identity` 18/13, `clutch_solana_layout` 2/2, and
  `clutch_solana_reference` 5/5. Every one is nonresident.
- 849 resident text symbols at 847 addresses; all 847 addresses were
  disassembled.
- 48,490 direct `r10` references; maximum offset 4,096; zero invalid
  positive, zero, or greater-than-4,096 references. The deepest direct
  reference sits in `claim_truth::observe_outcome_mints`.
- 449 first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `first-party-frame-audit.txt`
  (retained evidence).

**Tier 0 frame verification.** The `a04176f` merge restructured the ten
functions that exceeded the 4,096-byte SBF frame at opt-level "z", using
boxed single-decode `#[inline(never)]` helpers. Their measured maxima in
this exact artifact (pre-fix disassembly maximum → current):

| function | pre-fix max direct `r10` | current max | references |
| --- | ---: | ---: | ---: |
| `orders_batch::place_order` | 4,936 | 4,096 | 422 |
| `observe_resolve::recorded_redeem` | 4,704 | 4,096 | 364 |
| `orders_batch::settle_page` | 4,592 | 4,096 | 172 |
| `orders_batch::prepare_direct_v4_economics` | 4,528 | 4,096 | 236 |
| `direct_selection::authenticate_settlement` | 4,424 | 1,432 | 423 |
| `direct_selection::prepare_selection_commit` | 4,344 | 4,096 | 200 |
| `observe_resolve::resolve_global` | 4,328 | 4,096 | 620 |
| `observe_resolve::apply_native_market_resolution` | 4,256 | 4,096 | 270 |
| `claim_truth::commit_observed_supplies` | 4,168 | 1,080 | 67 |
| `observe_resolve::apply_legacy_market_resolution` | 4,128 | 4,096 | 261 |

Every one is now at or under the 4,096-byte line, and the backend emits no
diagnostic for any of them.

Selected first-party maxima (unchanged from the `187d5ee1…` seal):

| function | maximum direct `r10` | references |
| --- | ---: | ---: |
| `dispatch::process` | 0 | 0 |
| `dispatch::decode_only` | 328 | 3 |
| `dispatch::process_direct_selection_v3` | 320 | 10 |
| `dispatch::process_split` | 408 | 29 |
| every other dispatch family helper | 672 | 10 |
| `split::kernel_step` | 2,720 | 53 |
| `observe_resolve::pure_market` | 4,080 | 30 |
| `observe_resolve::apply_occupation_market_resolution` | 4,096 | 401 |
| `resolution_work::begin` | 4,096 | 162 |
| `resolution_work::fold` | 4,096 | 126 |
| `resolution_work::abort` | 4,096 | 92 |
| `resolution_work::finalize` | 4,096 | 62 |
| `resolution_work::process` | 0 | 0 |
| `direct_selection::submit_candidate` | 4,096 | 160 |
| `direct_selection::select_window` | 4,096 | 134 |
| `direct_selection::settle` | 4,096 | 245 |
| `direct_selection_v3::staged::submit` | 4,096 | 526 |
| `direct_selection_v3::staged::verify` | 4,096 | 146 |
| `direct_selection_v3::terminal::finalize` | 4,096 | 577 |
| `direct_selection_v3::terminal::settle` | 4,096 | 557 |
| `native_window::preflight_verified_archive` | 4,096 | 39 |
| `native_window::occupation_window` | 4,096 | 17 |

The new T2-3 staged-creation handlers measure
`clear_work::create_first_stage` 4,096/215, `init_clear_work` 4,096/53,
`finish_creation` 4,096/8, `write_grow_stage` 4,096/5, and
`grow_clear_work` 328/58.

The backend-survivor check is authoritative alongside these direct offsets;
an offset at or below 4,096 alone is not evidence that a nested-call warning
is safe.

ELF shape also passes: three load segments, no writable-executable segment,
1,414,848-byte `.text`, entrypoint `0xCEE38`, and only the reviewed undefined
imports `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address` — the identical surface as the `187d5ee1…`
seal. Loader-v3 Program/Buffer/ProgramData sizing is 36/1,527,677/1,527,685
bytes, with 8,958,075 bytes of data-length headroom.

## Exact comparison with the superseded `187d5ee1…` seal

This is a **materially different artifact**, not a line-record perturbation.
The stripped ELF grows from 1,420,608 to 1,527,640 bytes (+107,032) and
833,082 byte positions differ.

- `.text`: **different**, 1,361,184 → 1,414,848 bytes
- `.rodata`: **different**, 9,969 → 58,369 bytes (the T2 codec/projection
  tables dominate the growth)
- `.data.rel.ro`, `.dynamic`, `.dynsym`, `.rel.dyn`: **different**
- `.dynstr` and `.shstrtab`: identical — the undefined dynamic-symbol
  *names* and section-name table are unchanged, so no new syscall entered
  the surface
- Instruction disassembly: **different**, 166,786 → 173,087 instruction
  lines

Exact section digests and both disassemblies are retained evidence
(`comparison-187d-vs-fda5.txt`). No CU row, stack row, frame row, or
ELF-shape row from the `187d5ee1…` seal is carried forward; every current
row in the liveness profile was remeasured against exact `fda59705…`.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified crates.io
archives/unpacked trees, and one vendored package. `dependencies.tsv` and
`registry-source-verification.tsv` are byte-identical to the `187d5ee1…`
seal's — the wave changed no external pin.

The independently rebuilt current-bank fixture was rechecked as exact
`fda59705…` before every suite. Current `e58aef4` tests pass: artifact 6/6,
blank-bank 2/2, funded orders 2/2, ResolutionWork 4/4, batched folds 2/2,
collateral 13/13, DirectSelectionV2 2/2, prefund/source gate 5/5,
source-archive host tests 9/9, and native resolution 15/15. Exact external
paths and log hashes are recorded in `same-elf-bank-linkage.txt`.

CU drift against the `187d5ee1…` seal is small and two-sided, consistent
with the Tier 0 boxed-decode restructuring: every ResolutionWork route moved
by exactly +4 CU (batched folds +4 per constituent fold, up to +48 at
FoldBatch(12)); native/occupation resolve rows moved +111 to +176 CU and
internal redemptions +274 CU (at most +0.18%, the exact worst case the
Tier 0 lane measured); Direct V2 select moved -122 CU; order placement -72
and cancel +7 CU. One family exceeded the ±1% window and is noted honestly:
blank-bank `create_market` dropped to 190,757/204,428/206,033 CU
(-7.3%/-1.4%/-1.4% for v2/v3/v4). The drop coincides with Tier 0's boxing
of the market-construction reach graph (`claim_truth::observe_outcome_mints`
/ `commit_observed_supplies`, the very functions whose frames were
restructured); the byte-exactness and rollback assertions of the same suite
gate its semantics unchanged. No quote, gate, or admission flips on any of
these deltas: every previously admitted route stays admitted at the same
selected limit except where re-derivation says otherwise in `evidence.json`.

`legacy.clear_work` is the one account row that genuinely moved: T2-1
re-pinned `CLEAR_WORK_BODY_BYTES` to the checkpoint codec's exact
`ENCODED_BYTES`, so `account_len::CLEAR_WORK` is 48,004 bytes (rent
334,998,720 lamports), re-derived by the sealed offline probe at
`e58aef4`. The terminal projection keeps its 44-row inventory and 14
blocking ids; the T2-3 staged creation writes the same canonical PDA and
introduces no new account family.

Default Endow still refuses with `0x79` and byte/lamport-exact rollback
because no production source release is registered. No CU value is invented
for that log, and no mock-feature ELF evidence is mixed here.

The Direct V3 lifecycle is resident and stack-clean in this artifact and
remains classified-but-unpromoted in the terminal inventory: it still has no
measured CU row and no sealed close/rollback bank capture, so the V3 rows
keep their `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED` and rent-persistence STOPs
exactly as the V3 terminal-classification merge recorded them.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-e58aef4-reseal-evidence/artifact-fda59705ac1c18692faed454fb7588a32851028eea5ecc17e00f9d3f6af309ec`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-e58aef4-reseal-evidence/authoritative-audit-console.log`
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-e58aef4-reseal-evidence/source-e58aef4-b8babaa49b9d6a2428bd2672297a3a35efd625c3f903d4b63a01182eb76cc108.tar`
- First-party frame table: `first-party-frame-audit.txt`; environment and
  same-ELF linkage: `environment.txt` and `same-elf-bank-linkage.txt`
- Evidence checksum ledger: `SHA256SUMS` (mirrored here as
  `upstream-SHA256SUMS`)
