# SBF artifact and supply-chain audit — 2026-08-18

## Result

`programs/clutch-sbf/audit/audit_artifact.sh` passed against the clean source
closure named below.  The result is **local artifact evidence**, not a formal
verification result, deployment record, public-cluster observation, security
audit, or release attestation.

| Fact | Recorded value |
|---|---|
| Git source commit | `0c41870de8283e974ccf9db34313f49a62f8bc64` |
| Declared source-closure SHA-256 | `83b59dca7e402bb64ef5d3d8381f15898f91934896bd9daaa0fc77d4503efe55` |
| Declared source files | 55 |
| Runtime-recipe ELF SHA-256, pass 1 | `0c66e76ddeadf7ec6de81a26b0a816e247fdf8c42ff5e7258f7635acf2ddf71e` |
| Runtime-recipe ELF SHA-256, pass 2 | `0c66e76ddeadf7ec6de81a26b0a816e247fdf8c42ff5e7258f7635acf2ddf71e` |
| ELF bytes | 505,960 |
| Relocated-Cargo-home probe SHA-256 | `f7f805aac7897ec8430eb49f4930a6fa770e05bca4393f8195d1c626660ddbd6` |
| Overall local audit | PASS |

The command was:

```sh
programs/clutch-sbf/audit/audit_artifact.sh
```

It made no network request, signed nothing, started no validator, and submitted
or deployed nothing.

## What was pinned and checked

The audit selected one Anza distribution by canonical path, rejected mixed
per-binary and Rust/Cargo build overrides, and then recorded both version claims
and executable digests:

| Component | Version or source | SHA-256 |
|---|---|---|
| Anza release | commit `549805f3e85f345c9df98d59759691443eef57aa` | distribution identity |
| `solana` | `solana-cli 4.0.2` (`src:549805f3`) | `68f5fd83350ff3e7927a023fa641bfe39e123fc05ec987cb08834538fe6798f7` |
| `cargo-build-sbf` | 4.0.0 | `37c37d1a2ef0aa44065cde8c6ad07f0685bcef24699b4a9dd101372d7d4ef6e7` |
| platform tools | v1.53 | selected by `cargo-build-sbf` |
| platform `rustc` | 1.89.0-dev | `c58ec8ad482a40216152a4e4a6172f485be22a7f4482623e7e3cdf6c7085beb8` |
| platform `cargo` | 1.89.0 | `1eb15e30b4fddb0342872595b63b076d5f8599db716fec43de4d177de8d90fbc` |
| `llvm-objdump` | platform-tools v1.53 | `9e1ee0abcd8a6aecb3b846dafe48b9f5601185d79219722a437b245be3c84a09` |
| `llvm-readobj` | platform-tools v1.53 | `ab7e88ef8c81a2ebce08e8a821579206f1d8ee9d7fcd8720538b0d3488417c5b` |
| `llvm-objcopy` | platform-tools v1.53 | `1d8b8f2e312e12a06a8f508f705dad795b2752d554978755f6508885c40309f2` |
| `lld` | platform-tools v1.53 | `bf59d2b06b83b3bc0d3ff63ea1153cdf214f43f5d2d3bf9006891e800d70fabc` |
| fixed Cargo-home config | local input | `6c975f506bd33f4e8d3d92099425c284661f755679b6304ec3bf5bc10e8446ed` |

The workspace lock resolved 24 packages: six first-party AGPL-3.0-or-later
packages, 17 crates.io packages with locally rechecked archive hashes, and one
checked-in vendored package.  Every registry package had an exact lockfile
checksum and a nonempty SPDX license expression; Git dependencies, alternate
registries, source replacement, and active global patches were refused.

| crates.io package | Version | Declared license |
|---|---:|---|
| `autocfg` | 1.5.1 | Apache-2.0 OR MIT |
| `five8` | 1.0.0 | MIT |
| `five8_const` | 1.0.0 | MIT |
| `five8_core` | 1.0.0 | MIT |
| `num-traits` | 0.2.19 | MIT OR Apache-2.0 |
| `solana-account-info` | 3.1.1 | Apache-2.0 |
| `solana-address` | 2.6.1 | Apache-2.0 |
| `solana-cpi` | 3.1.0 | Apache-2.0 |
| `solana-define-syscall` | 4.0.1 | Apache-2.0 |
| `solana-instruction` | 3.4.1 | Apache-2.0 |
| `solana-instruction-error` | 2.4.0 | Apache-2.0 |
| `solana-program-entrypoint` | 3.1.1 | Apache-2.0 |
| `solana-program-error` | 3.0.1 | Apache-2.0 |
| `solana-program-memory` | 3.1.0 | Apache-2.0 |
| `solana-pubkey` | 4.2.0 | Apache-2.0 |
| `solana-sanitize` | 3.0.1 | Apache-2.0 |
| `solana-stable-layout` | 3.0.1 | Apache-2.0 |

For each of those 17 packages, the script rehashed the `.crate` archive against
`Cargo.lock` and compared every file in Cargo's already-unpacked build source
against the archive contents.  That closes the ordinary gap where Cargo trusts
an unpacked cache after its first checksum verification.

The vendored `solana-define-syscall 5.1.0` tree was byte-identical to its one
locally unpacked cache tree (excluding Cargo's `.cargo-ok` marker), declared
Apache-2.0, and had tree digest
`30db35e18af5a72674a1fffbe38d98065c35e7b09cccb2348077abdbc47d009d`.
Its upstream `.crate` archive is absent on this host.  Therefore the upstream
archive digest recorded in `vendor/PROVENANCE.md` was present and reviewed, but
could not be independently recomputed in this run.  Git commit plus the
declared source-closure digest pins the bytes that were actually compiled.

## Final-LTO stack audit

The SBF backend emitted nine distinct diagnostic lines naming eight functions
while it compiled dependency rlibs before fat LTO.  They were in the offline
reference/layout surface:

- `OrderPageAccount::decode`
- `OrderPageAccount::decode_on_grid`
- `DecodedState::decode`
- `apply_inner`
- `redeem_from_evidence`
- `resolve_from_evidence` (both a frame-overflow and call-frame diagnostic)
- `validate_market_init`
- `validate_position_init`

The audit parsed every diagnostic rather than treating the builder's zero exit
as sufficient.  It then inspected the unstripped linked ELF: zero of the eight
diagnosed symbols survived final LTO.  A surviving diagnosed symbol is a hard
failure.

As an independent artifact check, `llvm-objdump` disassembled all 350 distinct
resident `.text` function addresses (352 function symbols; two aliases).  It
parsed 16,305 direct `r10` frame references.  Every direct reference was a
negative offset in the inclusive range 1–4096 bytes.  The deepest was exactly
4096 bytes in `clutch_sbf::instructions::genesis::create_pda_account`.

This is strong compiler/artifact evidence, not a proof of arbitrary register
dataflow.  In particular, the direct-reference scan cannot prove that no future
handwritten assembly copies `r10` into another register and then escapes the
frame.  The backend diagnostic survivor check and final disassembly check are
complementary, and neither is described as formal verification.

## ELF and loader sizing

The stripped result was `elf64-sbf`, machine `EM_SBF`, a shared object with
three load segments and no writable-executable segment.  Its ELF header entry
equaled the defined `entrypoint` symbol at `0x36af0`; `.text` was 483,480 bytes.
The only undefined dynamic symbols were the reviewed runtime surface:

```text
abort
sol_invoke_signed_rust
sol_log_
sol_memcmp_
sol_memcpy_
sol_memset_
sol_panic_
sol_try_find_program_address
```

Using the loader-v3 wire sizes (Program 36 bytes, Buffer metadata 37 bytes,
ProgramData metadata 45 bytes), an exact-length deployment would require:

| Account/data item | Bytes |
|---|---:|
| Program | 36 |
| Buffer including ELF | 505,997 |
| ProgramData including ELF | 506,005 |
| Maximum permitted account data | 10,485,760 |
| ProgramData headroom | 9,979,755 |

This arithmetic does not quote rent, select an upgrade authority, or prove that
any public cluster accepts the program.

## Exact reproducibility boundary

Two builds with fresh target directories, locked resolution, offline mode, the
same source checkout, the same verified Cargo cache, and the same selected
toolchain produced byte-identical ELFs.

The third build changed only `CARGO_HOME` to a fresh isolated path and produced
`f7f805aa…`, not `0c66e76d…`.  Rust embedded absolute dependency source paths
in panic/location material, so the current runtime recipe is
**Cargo-home-path-sensitive**.  The justified claim is therefore:

> Reproduced twice on the same host with the same checkout path, fixed Cargo
> home path and verified cache contents, pinned builder/platform binaries,
> release profile, and locked dependency graph.

It is not yet a path-independent or cross-machine reproducible build.  A future
release recipe should remap the Cargo-home source prefix and then rerun both the
runtime differential gate and this audit on that exact remapped ELF before
strengthening the claim.
