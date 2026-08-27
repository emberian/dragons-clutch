# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `83e124dda22adc15cb5ebf18ff9e0ab971c551dc`. This is local build,
static artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary

- Clean detached worktree:
  `/Users/ember/jobs/dragons-clutch-r1-83e124d-stack-audit-worktree`
- Exact Git archive: 12,339,200 bytes, SHA-256
  `433684914c4e4b02fdbb7d1a121aa2ebd8d88dda7f67179d10ba847a96a04e1d`
- Declared SBF closure: 88 files, SHA-256
  `af3fce36f1a4eca17c3f13c1ab27fa2645a51e9249d0d7876b9b117d64d20b2e`
- Versus the old `7e8f6b1`/`a572...` seal, the declared closure changes only
  `programs/solana-reference/Cargo.lock` and the required rustdoc link repair
  in `programs/solana-reference/src/resolution.rs`. Exact provenance is in
  `source-change-vs-a572.txt`.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Pass 1/pass 2 stripped ELF: byte-identical SHA-256
  `bd20711b01828a745ce89de3aacb4b908cbcde32307b61be2c7d612bb8516b60`,
  1,228,192 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `afb56c0f21431fb1768c100c4d73f2ecd06b7bad72ebbda1cb5dbdd4c7448e2e`,
  1,344,944 bytes
- Relocated-Cargo-home stripped ELF: SHA-256
  `c2c549aafd42c243e56902c9096472e09ad29200688f5f8c8275c2367edf4866`
- Relocated unstripped ELF: SHA-256
  `a0aa916cde3da41df8225d5dae1b375af7d684690fae9783c186bd6513e0e84f`
- As in the old seal, Cargo-home relocation is path-sensitive. No
  path-independent or cross-host equality claim is made.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was unused.
No network, RPC, signing, deployment, submission, or external-state mutation
occurred.

## Final-LTO and direct-frame gate

- 34 backend diagnostic lines naming 27 unique symbols.
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostics by lines/symbols: `clutch_batch` 12/9,
  `clutch_batch_policy_identity` 10/7, `clutch_solana_layout` 5/4, and
  `clutch_solana_reference` 7/7. Every one is nonresident.
- 707 resident text symbols at 704 addresses; all 704 addresses were
  disassembled.
- 40,389 direct `r10` references; maximum offset 4,096; zero invalid positive,
  zero, or greater-than-4,096 references.
- 351 first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `audit/first-party-frame-audit.txt`.

Selected first-party maxima:

| function | maximum direct `r10` | references |
| --- | ---: | ---: |
| `dispatch::process` | 0 | 0 |
| `dispatch::decode_only` | 328 | 3 |
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
| `native_window::preflight_verified_archive` | 4,096 | 39 |
| `native_window::occupation_window` | 4,096 | 17 |

The backend-survivor check is authoritative alongside these direct offsets;
an offset at or below 4,096 alone is not evidence that a nested-call warning is
safe.

ELF shape also passes: three load segments, no writable-executable segment,
1,179,784-byte `.text`, entrypoint `0x9C7A8`, and only the reviewed undefined
imports `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address`. Loader-v3 Program/Buffer/ProgramData sizing is
36/1,228,229/1,228,237 bytes, with 9,257,523 bytes of data-length headroom.

## Exact comparison with old `a572...` seal

Old and current stripped ELFs are both 1,228,192 bytes. Exactly seven bytes
differ, all in `.data.rel.ro`; every other stripped section is byte-identical.

- `.text`: identical, SHA-256
  `b86ecac8d29ff18111efed9eaa21daf3ed2d908971adda155f8f48eb94190486`
- `.rodata`: identical, SHA-256
  `9373313db75444896f47e01e8f9a70ab06d88b0bd39fed51e9c3cf28115aac89`
- `.data.rel.ro`: old `1cdd3b20...`, current `7b3fec2d...`
- `.dynamic`, `.dynsym`, `.dynstr`, `.rel.dyn`, and `.shstrtab`: identical
- Normalized ELF headers, sections, segments, dynamics, symbols, and
  relocations: identical
- Instruction disassembly after removing only file-path headers, function-label
  spellings, and symbolic branch annotations: identical, SHA-256
  `54d85b4ff5b5459769568712af3beb848083500c69bb568f0fd9b06c351805d1`

The seven changed fields are the line-number words for the same
`src/resolution.rs` location records: 547→548, 542→543, 571→572, 563→564,
643→644 twice, and 774→775. Each is exactly the one-line displacement caused
by the required rustdoc repair. The exact offsets and section hashes are under
`comparison/`. The unstripped ELFs have the same size but different symbol and
metadata bytes; their instruction sections remain identical, so no unstripped
byte-identity claim is made.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified crates.io
archives/unpacked trees, and one vendored package. Exact versions, archive
hashes, unpacked paths, source classes, and licenses are retained in
`audit/dependencies.tsv` and `audit/registry-source-verification.tsv`.

The independently retained current-bank fixture was rechecked as exact
`bd20711b...` before every suite. Current `83e124d` tests pass: artifact 6/6,
blank-bank 2/2, funded orders 2/2, ResolutionWork 4/4, collateral 13/13,
DirectSelectionV2 2/2, prefund/source gate 5/5, source-archive host tests 9/9,
and native resolution 15/15. Exact external paths and log hashes are recorded
in `same-elf-bank-linkage.txt`.

Direct Select still reaches 1,400,000 CU and rolls back: this is a functional
liveness STOP, not a stack STOP. Default Endow still refuses with `0x79` and
rollback because no production source release is registered. No CU value is
invented for that log, and no mock-feature ELF evidence is mixed here.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-83e124d-stack-audit-evidence/artifact-bd20711b01828a745ce89de3aacb4b908cbcde32307b61be2c7d612bb8516b60`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-83e124d-stack-audit-evidence/authoritative-audit-console.log`
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-83e124d-stack-audit-evidence/source-83e124d-433684914c4e4b02fdbb7d1a121aa2ebd8d88dda7f67179d10ba847a96a04e1d.tar`
- Environment and same-ELF linkage: `environment.txt` and
  `same-elf-bank-linkage.txt`
- Evidence checksum ledger: `SHA256SUMS`
