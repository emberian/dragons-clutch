# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `2d530d218b470e5e2d1cf52480c4a9d1636c08e1`. This is local build,
static artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary

- Clean detached worktree:
  `/Users/ember/jobs/dragons-clutch-r1-2d530d2-stack-audit-worktree`
- Exact Git archive: 15,400,960 bytes, SHA-256
  `e9b5f72f94540a586c09e02c7ef930e3d3dea02b6133963386332383e79b371b`
- Declared SBF closure: 94 files, SHA-256
  `a0b00f394380e79bbe5eb2a57c469020a5aa455ca9f5c9cc4b2bef98e7134730`
- Versus the old `83e124d`/`bd20711b…` seal, the declared closure grows from 88
  to 94 files: the Direct V3 selection lifecycle merge (`fb72b34`) adds five
  `instructions/direct_selection_v3*` files and `solana-layout`'s
  `direct_selection_v3.rs`, and modifies the dispatcher, seeds, artifact,
  genesis, orders-batch, legacy direct-selection, and Solana-reference roots.
  Exact provenance is in `source-change-vs-bd207.txt`.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Pass 1/pass 2 stripped ELF: byte-identical SHA-256
  `af6bb79cc3766bd0d889b46dc1becfebe140c7df2746971943e9edf4efc2014b`,
  1,490,544 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `a82716bfea73eadc0b10d9eccc73527bc0bf10d559767c69ea5a655508590de2`,
  1,629,840 bytes
- Relocated-Cargo-home stripped ELF: SHA-256
  `241a01ef056a135d3d2601d485a28b1f7f0631cb0981d22456911d5a8bde353a`
- Relocated unstripped ELF: SHA-256
  `23d4e51caf0141f4701fa42a7b7931aa95992b84eab3cc2145e967438a06a3ad`
- As in the old seal, Cargo-home relocation is path-sensitive. No
  path-independent or cross-host equality claim is made. The relocated probe
  path is a fresh temporary directory per run, so its digest is not stable
  across audits and is never substituted for the canonical artifact.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was unused.
No network, RPC, signing, deployment, submission, or external-state mutation
occurred.

## Final-LTO and direct-frame gate

- 40 backend diagnostic lines naming 31 unique symbols.
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostics by lines/symbols: `clutch_batch` 12/9,
  `clutch_batch_policy_identity` 16/11, `clutch_solana_layout` 5/4, and
  `clutch_solana_reference` 7/7. Every one is nonresident.
- 810 resident text symbols at 808 addresses; all 808 addresses were
  disassembled.
- 49,521 direct `r10` references; maximum offset 4,096; zero invalid positive,
  zero, or greater-than-4,096 references.
- 421 first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `audit/first-party-frame-audit.txt`.

Selected first-party maxima:

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
1,430,592-byte `.text`, entrypoint `0xCACC0`, and only the reviewed undefined
imports `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address`. Loader-v3 Program/Buffer/ProgramData sizing is
36/1,490,581/1,490,589 bytes, with 8,995,171 bytes of data-length headroom.

## Exact comparison with old `bd20711b…` seal

Unlike the `a5725a3d…` → `bd20711b…` step, this is a **materially different
artifact**, not a line-record perturbation. The stripped ELF grows from
1,228,192 to 1,490,544 bytes (+262,352) and 894,355 byte positions differ.

- `.text`: **different**, 1,179,784 → 1,430,592 bytes
- `.rodata`, `.data.rel.ro`, `.dynamic`, `.dynsym`, `.rel.dyn`: **different**
- `.dynstr` and `.shstrtab`: identical — the undefined dynamic-symbol *names*
  and section-name table are unchanged, so no new syscall entered the surface
- Normalized ELF headers, sections, segments, dynamics, symbols, and
  relocations: **different**
- Instruction disassembly after removing only file-path headers, function-label
  spellings, and symbolic branch annotations: **different**, 144,879 → 175,382
  instruction lines

Exact section digests and both normalized dumps are under `comparison/`. No CU
row, stack row, frame row, or ELF-shape row from the `bd20711b…` seal is
carried forward; every current row below and in the liveness profile was
remeasured against exact `af6bb79c…`.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified crates.io
archives/unpacked trees, and one vendored package. Exact versions, archive
hashes, unpacked paths, source classes, and licenses are retained in
`audit/dependencies.tsv` and `audit/registry-source-verification.tsv`.

The independently retained current-bank fixture was rechecked as exact
`af6bb79c…` before every suite. Current `2d530d2` tests pass: artifact 6/6,
blank-bank 2/2, funded orders 2/2, ResolutionWork 4/4, collateral 13/13,
DirectSelectionV2 2/2, prefund/source gate 5/5, source-archive host tests 9/9,
and native resolution 15/15. Exact external paths and log hashes are recorded
in `same-elf-bank-linkage.txt`.

The measured ResolutionWork route moved by exactly one CU per route against the
old seal — Begin 810,992 → 810,993, Fold(4) 815,573 → 815,574, Finalize
1,094,832 → 1,094,833, Abort 587,197 → 587,198 — which is the cost of the one
added dispatcher arm. Every route still clears its 25%-headroom admission.
Direct SelectionV2 Select still reaches 1,400,000 CU and rolls back: this is a
functional liveness STOP, not a stack STOP. Default Endow still refuses with
`0x79` and rollback because no production source release is registered. No CU
value is invented for that log, and no mock-feature ELF evidence is mixed here.

The merged Direct V3 lifecycle is resident and stack-clean in this artifact but
is deliberately **not** promoted into the liveness projection by this audit: it
has no measured CU row, no rent/refund/close row, and no terminal-admission row
in this evidence package.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-2d530d2-stack-audit-evidence/artifact-af6bb79cc3766bd0d889b46dc1becfebe140c7df2746971943e9edf4efc2014b`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-2d530d2-stack-audit-evidence/authoritative-audit-console.log`
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-2d530d2-stack-audit-evidence/source-2d530d2-e9b5f72f94540a586c09e02c7ef930e3d3dea02b6133963386332383e79b371b.tar`
- Environment and same-ELF linkage: `environment.txt` and
  `same-elf-bank-linkage.txt`
- Evidence checksum ledger: `SHA256SUMS`
