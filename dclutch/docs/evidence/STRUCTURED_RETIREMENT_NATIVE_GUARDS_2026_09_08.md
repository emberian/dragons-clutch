# Structured retirement native guard controls — 2026-09-08

Evidence level: real-ELF Solana ProgramTest. This is not a local-validator
full lifecycle receipt. The fixture stages Core Retiring and the nonzero-supply
counterexample through ProgramTest account replacement; it does not claim a
runtime path from Open to Retiring.

The canonical sparse descriptor has coefficients `[3, 0, 7]`. Three separately
filtered tests exercise omitted, duplicate, and extra support. Each requires
`RationalLifecycleSbfErrorV2::InvalidSupport`, verifies all eight observed
accounts remain byte-for-byte equal, then executes the complete ordered support
control and verifies receipt closure and exact RentCredit credit. Each hostile
packet uses an actual address lookup table and the normal Solana packet limit.

All three tests fail against the preserved Claims ELF from checked source
551ffc1b (coarse `Instruction`) and pass against the bf06c8d75 Claims ELF. The
other program ELFs are held constant. The existing late-failure lifecycle test
also checks an actual nonzero shard Mint against a zero-supply retirement request:
it fails against bf06's coarse `MintProfile` and passes against the new dedicated
`MintSupply` mapping, with the entire eight-account snapshot unchanged. The
accepted closure control and existing late-CPI rollback controls remain green.

## Source and commands

Repository: `/Users/ember/dev/dclutch`. Source snapshot base:
`bf06c8d752eafa572a433b49cb0db38fe413fbbd`, archived to
`hbox:/tank/dregg-build/structured-native-bf06c8d75-20260908/source`, with only the
named lifecycle test and (for supply green) lifecycle adapter overlays. Each
checkout owns its Cargo target. Measurements were reviewed from live HEAD
`0f7d65466da0a3bfd7cb886d0e3e831da2474b14`; this does not make the isolated
source a whole-HEAD build.

Adapter overlay SHA-256:
`47a5cf2f3cf74bec8a6a98a34d06427bba92ae9ae301837ba2ce0145e0e5f486`.
Test overlay SHA-256:
`e770dc0eb5a1198f44cfe6d377ae311ba7336cb82133eef40a1aa10f67a7d271`.

Commands use `SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=6 swarm-build` and
`SBF_OUT_DIR` pointing at the controlled ELF directory:

- `cargo test --locked -p dclutch-rational-lifecycle-program-test --test lifecycle complete_sparse_support_refuses_ -- --test-threads=1`: red 0 passed/3 failed; green 3 passed/0 failed.
- `cargo test --locked -p dclutch-rational-lifecycle-program-test --test lifecycle real_token_2022_lifecycle_refuses_ata_substitution_and_rolls_back_every_late_failure -- --test-threads=1`: red 0 passed/1 failed; green 1 passed/0 failed.
- `cargo check --locked -p dclutch-claims-sbf` passed before the supply SBF build.
- `cargo build-sbf --manifest-path programs/dclutch-claims-sbf/Cargo.toml --sbf-out-dir ../supply-elf -- --locked` passed; no frame diagnostic or silent-swarm no-op marker. The formal frame ratchet remains owed to the exact-source shared capture.

## Preserved artifacts

All paths below are relative to
`hbox:/tank/dregg-build/structured-native-bf06c8d75-20260908/`.

| Artifact | SHA-256 |
| --- | --- |
| `red-elf/dclutch_claims_sbf.so` | `b18309487b59dd39745e7937fe59e6e49eb6a983b1a0f4ea6c2f2ed056039ef2` |
| `green-elf/dclutch_claims_sbf.so` | `4d9a098705f765957b65118243a281c88504c63acaa7193af9434b74d75d06de` |
| `supply-green-elf/dclutch_claims_sbf.so` | `3982915b30a4569ba99f977bc9cd34e790066c00ba9b67f986877480f8a5da25` |
| `support-red-final.log` | `dcd74f50ca7fe1c8a4592d5ba7943316c9f9905257e97dce5b90b3a77cc57af5` |
| `support-green-final.log` | `e285194e228628ef7e4ba6c369c6bfc86162d672ac2a618f26a3dc313a37938d` |
| `supply-red.log` | `49da68c6d951c2c877915480460d3ead9cebf21c3572a27be05239d6e8d32c72` |
| `supply-green.log` | `e785b8bf153988f52aae6c3f08bcc00c42c58fa5fab89fb94bf79af979babaa8` |
| `supply-sbf.log` | `0bbb9a389907298d7ad06cae3a1a24ab91876e54a5f8206e49300b6f3e3a0daa` |

Fixed caller ELF SHA-256:
`f83650a15cc9ab40f68a4e0538f7c811ff58921bd0d968f147fd684898b15391`.
Repository-pinned Token-2022 ELF SHA-256:
`e2acdfb750881462ad613a15cc9c54ae17ce066580e867e1e635fbdfe01f5697`.

The generic Structured Profile 13 bridge and the selected ProgramSet retirement
entries still require actual validator execution through receipt/coordinate
activation, representation, authentic terminal settlement, and retirement. These
native guards establish the Claims owner's support and supply refusal boundary.
