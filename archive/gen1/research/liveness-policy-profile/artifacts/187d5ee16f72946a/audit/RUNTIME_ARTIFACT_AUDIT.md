# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `d8c50347556353939d4f5a91af616ba40d70419a`. This is local build,
static artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary

- Clean detached worktree:
  `/Users/ember/jobs/dragons-clutch-r1-6c25df4-stack-audit-worktree`
  (0 porcelain lines at `d8c5034`)
- Exact Git archive: 17,336,320 bytes, SHA-256
  `cdf351a2304805f4fd79a86ba9bd36e47b2e664d142e4022c394ad4e07f3f6dc`
- Declared SBF closure: 94 files, SHA-256
  `d01a873f53d52e06d087af3771ad4770d13d561b0930d959c18aceb7fe2e41cc`
- `d8c5034` is `6c25df4` (the SHA-256-syscall conversion merge) plus exactly
  one `research/liveness-policy-profile/Cargo.lock` line, added so the sealed
  account probe resolves offline; nothing in the SBF closure differs between
  the two commits, and both were built to the identical ELF during this audit.
- Versus the `2d530d2`/`af6bb79c…` seal, the declared closure keeps the same
  94 paths; within it only `programs/solana-layout/src/lib.rs` (+422 lines:
  canonical digests routed through the safe `sol_sha256` wrapper) and three
  lock-line records change. The larger half of the conversion — dropping the
  software `sha2` compression function and instantiating the twelve
  policy-identity folds against the syscall — lives in
  `research/batch-policy-identity`, which is a first-party path dependency
  **outside** the declared 94-file closure. That closure gap is pre-existing
  (the same crate sat outside the `af6bb79c…` closure); this audit compensates
  by building from a fully clean detached worktree, so every path dependency,
  declared or not, is exactly the committed tree. Exact provenance is in
  `source-change-vs-af6bb.txt`.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Pass 1/pass 2 stripped ELF: byte-identical SHA-256
  `187d5ee16f72946ae81eb928e127347c1e17aaf0a0ad7d837be5c4186dde16bd`,
  1,420,608 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `d438297944f2e5b3d007c362c27ef711c89483a09f591995cff7bec34cfb7794`,
  1,558,832 bytes
- Relocated-Cargo-home stripped ELF: SHA-256
  `187d5ee16f72946ae81eb928e127347c1e17aaf0a0ad7d837be5c4186dde16bd` —
  **byte-identical to the canonical artifact**, unstripped likewise
  (`d4382979…`). The path sensitivity measured in every previous seal
  dissolved in the same change that removed the software `sha2` crate from
  the SBF graph. This is one relocated path on one host; it upgrades the
  local claim from path-sensitive to relocation-independent-as-measured and
  still makes no cross-host equality claim.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was unused.
No network, RPC, signing, deployment, submission, or external-state mutation
occurred.

## Final-LTO and direct-frame gate

- 42 backend diagnostic lines naming 33 unique symbols.
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostics by lines/symbols: `clutch_batch` 12/9,
  `clutch_batch_policy_identity` 18/13, `clutch_solana_layout` 5/4, and
  `clutch_solana_reference` 7/7. Every one is nonresident. The two
  policy-identity additions over the prior seal are the deliberately refused
  witness-scale buffered folds (`full_relation_candidate_digest` and the
  offline `DirectPrefreezePageV3` page digest) that cannot fit a 4 KiB SBF
  frame in buffered form and are off the program's reach graph.
- 806 resident text symbols at 804 addresses; all 804 addresses were
  disassembled.
- 48,272 direct `r10` references; maximum offset 4,096; zero invalid positive,
  zero, or greater-than-4,096 references.
- 421 first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `audit/first-party-frame-audit.txt`
  (retained evidence).

Selected first-party maxima (unchanged from the `af6bb79c…` seal):

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

The backend-survivor check is authoritative alongside these direct offsets;
an offset at or below 4,096 alone is not evidence that a nested-call warning is
safe.

ELF shape also passes: three load segments, no writable-executable segment,
1,361,184-byte `.text`, entrypoint `0xCAD00`, and only the reviewed undefined
imports `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address` — the identical surface as the `af6bb79c…`
seal; the conversion widened *use* of `sol_sha256`, not the import surface.
Loader-v3 Program/Buffer/ProgramData sizing is 36/1,420,645/1,420,653 bytes,
with 9,065,107 bytes of data-length headroom.

## Exact comparison with old `af6bb79c…` seal

This is a **materially different artifact**, not a line-record perturbation.
The stripped ELF shrinks from 1,490,544 to 1,420,608 bytes (-69,936) and
688,001 byte positions differ.

- `.text`: **different**, 1,430,592 → 1,361,184 bytes
- `.rodata`, `.data.rel.ro`, `.dynamic`, `.dynsym`, `.rel.dyn`: **different**
- `.dynstr` and `.shstrtab`: identical — the undefined dynamic-symbol *names*
  and section-name table are unchanged, so no new syscall entered the surface
- Normalized ELF headers, sections, segments, dynamics, symbols, and
  relocations: **different**
- Instruction disassembly after removing only file-path headers, function-label
  spellings, and symbolic branch annotations: **different**, 175,382 → 166,786
  instruction lines

Exact section digests and both normalized dumps are under `comparison/`
(retained evidence). No CU row, stack row, frame row, or ELF-shape row from
the `af6bb79c…` seal is carried forward; every current row in the liveness
profile was remeasured against exact `187d5ee1…`.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified crates.io
archives/unpacked trees, and one vendored package. `sha2` is no longer in the
SBF graph. Exact versions, archive hashes, unpacked paths, source classes, and
licenses are retained in `audit/dependencies.tsv` and
`audit/registry-source-verification.tsv`.

The independently rebuilt current-bank fixture was rechecked as exact
`187d5ee1…` before every suite. Current `d8c5034` tests pass: artifact 6/6,
blank-bank 2/2, funded orders 2/2, ResolutionWork 4/4, collateral 13/13,
DirectSelectionV2 2/2, prefund/source gate 5/5, source-archive host tests 9/9,
and native resolution 15/15. Exact external paths and log hashes are recorded
in `same-elf-bank-linkage.txt`.

Every measured CU row moved by roughly 3x-8x against the `af6bb79c…` seal —
this is the cost of the software SHA-256 compression function leaving the
program. Two prior measured STOPs dissolve and are re-recorded as
measurements, not as promotions:

- Direct SelectionV2 Select, previously a functional liveness STOP at the
  full 1,400,000-CU cap with rollback, now **completes at 226,071 CU and
  commits its selection**. Whether V2 selection may be called live remains a
  staged-verification and candidate-rent question; V3 remains the shipped
  path and the V2 subsystem stays unpromoted (its empty-frozen lapse is still
  unimplemented).
- Every occupation-v4 monolithic span/degree profile, previously above the
  25%-headroom admission gate at roughly 1.24m-1.27m CU, now measures roughly
  173k-198k CU and clears the gate with the bank test asserting all six
  admissible.

Default Endow still refuses with `0x79` and byte/lamport-exact rollback
because no production source release is registered. No CU value is invented
for that log, and no mock-feature ELF evidence is mixed here.

The merged Direct V3 lifecycle is resident and stack-clean in this artifact
but is deliberately **not** promoted into the liveness projection by this
audit: it has no measured CU row, no rent/refund/close row, and no
terminal-admission row in this evidence package.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-6c25df4-stack-audit-evidence/artifact-187d5ee16f72946ae81eb928e127347c1e17aaf0a0ad7d837be5c4186dde16bd`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-6c25df4-stack-audit-evidence/authoritative-audit-console.log`
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-6c25df4-stack-audit-evidence/source-d8c5034-cdf351a2304805f4fd79a86ba9bd36e47b2e664d142e4022c394ad4e07f3f6dc.tar`
- Environment and same-ELF linkage: `environment.txt` and
  `same-elf-bank-linkage.txt`
- Evidence checksum ledger: `SHA256SUMS`
