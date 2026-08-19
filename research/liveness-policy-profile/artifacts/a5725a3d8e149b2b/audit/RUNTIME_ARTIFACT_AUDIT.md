# Dragon's Clutch R1 frozen runtime artifact audit

Status: **artifact and first-party SBF stack audit PASS** for the exact runtime
source commit below. This is local build and in-process-bank evidence. It is not
a deployment, release, cluster observation, formal verification, or production
source-provider claim.

## Frozen source

- Runtime commit: `7e8f6b1714c3c97a31a4250ecd19f87041433c2d`
- Final repository ancestry: `316c620c8c618142fc3a21964052bcb42c4336b6`.
  The intervening commits change the SVM fixture, research lock, and documents;
  an exact path comparison reports no change in the 88-file declared SBF
  runtime source closure. Accordingly, the artifact's runtime-source identity
  is `7e8f6b1`, while `316c620` is the repository release ancestry. The build
  and source archive deliberately record the former, not the later test-only
  ancestry.
- Clean detached worktree:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-worktree.wacx0l`
- Declared SBF source closure: 88 files,
  SHA-256 `2fcf7d778d09e0c647f265883f6070c8b1de03aba985a70929c910c5d9d097d4`
- Git source archive: 10,496,000 bytes,
  SHA-256 `9d7bcd122ae4791b1b670cea62db68838671d443fab25425e037fe95a3aa890f`
- Source archive path:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-evidence/source-7e8f6b1-9d7bcd122ae4791b1b670cea62db68838671d443fab25425e037fe95a3aa890f.tar`

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`
- `platform-tools v1.53`
- SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Solana CLI `4.0.2`, source `549805f3`
- Runtime pass 1: SHA-256
  `a5725a3d8e149b2b52605e1785f7ad29fdc6b2db1ed32ca83a31b41822d6b6a1`,
  1,228,192 bytes
- Runtime pass 2: byte-identical SHA-256
  `a5725a3d8e149b2b52605e1785f7ad29fdc6b2db1ed32ca83a31b41822d6b6a1`,
  1,228,192 bytes
- The result is byte-identical to the prior out-16 candidate.
- Relocated-Cargo-home probe: SHA-256
  `b1a8de39c4405fed6b6da3b430ea7d6109280779dcc2c67011daedda8c06c2de`.
  The recipe remains Cargo-home-path-sensitive; no path-independent claim is
  made.
- Runtime unstripped pass 1/pass 2: byte-identical SHA-256
  `dd7f7bf972c4a32ab4e258323684242f7890c0c696aff82ff7ccfd998339de6c`,
  1,344,944 bytes.
- Relocated unstripped ELF: SHA-256
  `e48f0b8aa963fcc028d69c7aac5e44932e2db88ceae284cf66914be28e50da4c`,
  1,345,656 bytes.

The build ran locally on an Apple M2 Max under macOS 26.6.1. This lane did not
expose hbox/`swarm-build`; hbox was not used. No GPU was used. No network, RPC,
signing, deployment, submission, or external state mutation occurred.

## Final-LTO diagnostics and direct frames

- 34 unique backend diagnostic lines naming 27 unique symbols.
- Zero diagnostics name `clutch_sbf`.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostic attribution by lines/symbols:
  `clutch_batch` 12/9, `clutch_batch_policy_identity` 10/7,
  `clutch_solana_reference` 7/7, `clutch_solana_layout` 5/4. Every named
  foreign symbol is nonresident.
- 707 resident text symbols at 704 distinct addresses; all 704 addresses were
  disassembled.
- 40,389 direct `r10` references; deepest offset 4,096; zero positive, zero,
  or greater-than-4,096 references.

Selected resident direct-frame maxima (the diagnostic-survivor gate is the
authoritative complementary check, not these offsets alone):

| resident function | maximum direct `r10` offset | references |
| --- | ---: | ---: |
| `dispatch::process` | 0 | 0 |
| `dispatch::decode_only` | 328 | 3 |
| `dispatch::process_split` | 408 | 29 |
| every other routed dispatch helper | 672 | 10 each |
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

The stripped ELF has three load segments, no writable-executable segment, a
1,179,784-byte `.text`, entrypoint `0x9C7A8`, and only these undefined dynamic
symbols: `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memset_`, `sol_panic_`, `sol_sha256`, and
`sol_try_find_program_address`.

Loader-v3 exact-length sizing is 36 bytes for Program, 1,228,229 bytes for
Buffer, and 1,228,237 bytes for ProgramData, leaving 9,257,523 bytes below the
10,485,760-byte maximum data length.

## Dependency and source inventory

The closed graph contains 42 packages: 11 first-party AGPL-3.0-or-later
packages, 30 crates.io packages whose archive hashes and unpacked source trees
were rechecked, and one vendored Apache-2.0 package. The vendored
`solana-define-syscall 5.1.0` tree SHA-256 is
`30db35e18af5a72674a1fffbe38d98065c35e7b09cccb2348077abdbc47d009d`.
Exact names, versions, source classes, archive hashes, licenses, and unpacked
paths are retained in `audit/dependencies.tsv` and
`audit/registry-source-verification.tsv`.

## Same-ELF local-bank capture

Every bank run below rechecked the digest-named fixture as exact
`a5725a3d...d6b6a1` before starting. The default-ELF suites from `7e8f6b1`
passed: artifact transport 6/6, blank-bank lifecycle 2/2, funded orders 2/2,
ResolutionWork 4/4, collateral leg 13/13, DirectSelectionV2 2/2, source/value
prefund gate 5/5, and source/archive host tests 9/9.

The original `7e8f6b1` native-resolution fixture produced three `0x0050`
refusals. This was fixture drift, not a runtime regression: its legacy point
projection encoded source-adapter version 1 while its parser, SourceSpec, Feed,
Terms, and archive used version 7. The typed subreason is
`WindowError::MismatchedFeed`. Commit
`161f530fc33c5784f35a0d6e2d695725054f2180` gave that fixture fact one semantic
owner without changing the SBF artifact. An independent rerun of the corrected
fixture source against the same `a5725a3d...` ELF passed all 15/15 native
resolution tests. Both the original-red and corrected-green logs are retained.

Selected same-ELF CU/account rows:

| route | exact captured values |
| --- | --- |
| blank-bank categorical | resolution 165 bytes; rent 2,039,280; CreateMarket 915,790 CU |
| blank-bank native v3 | resolution 319 bytes; rent 3,111,120; CreateMarket 923,461 CU |
| blank-bank occupation v4 | resolution 383 bytes; rent 3,556,560; CreateMarket 931,066 CU |
| PlaceOrder | buy 598,879; sell 596,909 CU |
| CancelOrder | buy 470,318; sell 474,991 CU |
| ResolutionWork normal | Begin 807,676; Fold1 804,616; Fold2 809,225; Finalize 1,094,832; monolithic 1,253,326 CU |
| ResolutionWork folds | span1 802,253; span2 812,193; span3 813,128; span4 815,573 CU |
| ResolutionWork abort/reopen | BeginAbort 807,676; AbortExpired 587,197; BeginReopen 807,676 CU |
| native v3 d1 | Resolve 1,088,267; retry 934,622; internal redeem 775,978; bearer 785,073 CU |
| native v3 d2 | Resolve 1,096,640; retry 942,995; internal redeem 778,021; bearer 786,873 CU |
| native v3 d3 | Resolve 1,103,534; retry 949,889; internal redeem 776,411; bearer 784,578 CU |
| occupation v4 d1 | Resolve 1,243,529; retry 1,089,893; internal 774,692; bearer 783,711 CU |
| occupation v4 d2 | Resolve 1,254,699; retry 1,101,063; internal 776,735; bearer 785,511 CU |
| occupation v4 d3 | Resolve 1,268,630; retry 1,114,994; internal 779,625; bearer 787,716 CU |
| collateral focused | CreateMarket 948,052; Split 231,409; Merge 231,440; Materialize 112,755; Dematerialize 112,744 CU |
| WithdrawCash | paid 73 unreserved atoms; 229,756 CU |
| direct-selection V2 | Init 532,149; Freeze 935,108; Submit maximum 1,194,085; Select STOP 1,400,000 CU |

The default source/value gate correctly refuses Endow with `0x0079` because no
production source release is registered, while preserving rollback. The full
native lifecycle file is intentionally compiled out under the default profile;
running it requires the distinct `non-production-mock-source` ELF, so it
supplies no same-`a5725a3d` row.

The direct-selection Select transaction still reaches the 1,400,000-CU limit
and is a measured liveness STOP with rollback. This is not a stack STOP. The
production source-release absence is likewise an explicit functional STOP,
not an artifact or stack failure.

## Retained evidence

- Complete selected artifact evidence:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-evidence/artifact-a5725a3d8e149b2b52605e1785f7ad29fdc6b2db1ed32ca83a31b41822d6b6a1`
- Default frozen-source bank logs:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-evidence/bank-logs-a5725a3d8e149b2b52605e1785f7ad29fdc6b2db1ed32ca83a31b41822d6b6a1`
- Corrected-fixture native bank log:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-evidence/bank-logs-fixture-161f530-a5725a3d8e149b2b52605e1785f7ad29fdc6b2db1ed32ca83a31b41822d6b6a1/native_resolution.log`
- Environment record:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-evidence/environment.txt`
- Runtime-source versus repository-ancestry comparison:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-evidence/release-ancestry-diff.txt`
- Evidence SHA-256 manifest:
  `/Users/ember/jobs/dragons-clutch-r1-7e8f6b1-evidence/SHA256SUMS`
