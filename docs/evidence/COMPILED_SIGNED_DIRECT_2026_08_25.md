# Compiled signed Direct with canonical state — 2026-08-25

## Result

Source commit `8e8e26631877cc2d63a083f7cfb05058d5f43e77` executes one
inline ordinary Direct fill from two independently signed compact intents. The
caller supplies no claim or custody plan. A controller authenticates the native
Ed25519 instruction and exact current instruction, builds a fixed register
frame from runtime-owned facts, runs Lean-generated transition bytecode, and
derives both child plans from the successful output registers.

Replay and claims no longer share a pair-specific projection. The physical
claim child mutates exactly four canonical owners:

- seller execution-profile/generation/maker replay root;
- buyer execution-profile/generation/maker replay root;
- seller execution-profile/maker/outcome Position; and
- buyer execution-profile/maker/outcome Position.

This is local real-SVM evidence for exact artifacts, not mainnet evidence and
not a claim that the Solana program is formally verified.

## Semantic and generated material

Lean 4.30.0 owns:

- the Direct admission relation, exact quote equation, cumulative floor-fee
  boundary, replay progression, limits, lifecycle, and conservation theorems;
- the 35-instruction, 568-byte `DCTV` admission/derivation program;
- the exact 136-byte signed intent, 304-byte controller instruction, and
  136-byte experimental execution-profile encodings and length theorems; and
- the loader-v1 offsets, roles, state tags, and ordered effects for the
  five-account claim child.

The transition-program bytes have SHA-256
`72cc0faa6a9768b766a3003c8ff6f38889f564f49005ce68b2187c98349bff5c`.
`lake exe emit-direct-program-rust` exactly reproduces the embedded Rust array,
and `lake exe emit-claim-sbf-profile` exactly reproduces the claim child's Rust
profile constants.

The immutable execution-profile identity is the sole selector for its fee
policy. Both makers still sign the exact accepted fee rate. Removing the
duplicated fee-policy identifier reduced the program from 37 instructions / 600
bytes to 35 instructions / 568 bytes without weakening fee-rate admission.

## Build and artifacts

The first-party artifacts were rebuilt from the source commit with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-claims-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
cargo build-sbf \
  --manifest-path programs/dclutch-controller-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
```

The build used cargo-build-sbf 4.0.0, platform-tools v1.53, SBF rustc 1.89.0,
and emitted no verifier diagnostic. Host tests used Rust 1.97.1 and
solana-program-test 4.2.1.

| Program | ELF bytes | SHA-256 | Equivalent Loader V3 capitalization |
|---|---:|---|---:|
| canonical claim executor | 3,432 | `5878343447df3e4c703b1047f0fd4f9df890c74a28c410c738bd10d1c5358468` | 0.026232240 SOL |
| signed compiled controller | 56,048 | `b960725cb5d151e30046b66fad1627bfe44c479f199a41fbcbb4b62b6b5cc1f8` | 0.392439600 SOL |
| real custody adapter | 24,800 | `c4f9a6ac223639158fb3f40d40b1e59ac1c1e369ff0c3c9c0667c1658f787796` | 0.174953520 SOL |
| first-party total | 84,280 | — | 0.593625360 SOL |
| official SPL Token 9.0.0 | 93,056 | `c85ce043abbfcb0363b5c724245caa9d9201d2a9b669c02a5c2770512b65d78f` | 0.650015280 SOL |

Capitalization uses `Rent::default()`, one 36-byte Loader V3 Program account
and 45 bytes of ProgramData metadata per program. The canonical legacy token
program is already deployed; its number is an equivalent local measurement,
not dClutch deployment capital. Transient buffers and transaction fees are
excluded.

The experimental mutable-state rent minima are:

| State | Count | Bytes each | Rent each | Total |
|---|---:|---:|---:|---:|
| maker replay root | 2 | 48 | 0.001224960 SOL | 0.002449920 SOL |
| maker/outcome Position | 2 | 56 | 0.001280640 SOL | 0.002561280 SOL |
| execution profile | 1 | 136 | 0.001837440 SOL | 0.001837440 SOL |
| controller journal | 1 | 16 | 0.001002240 SOL | 0.001002240 SOL |
| total | 6 | 360 | — | 0.007850880 SOL |

Replay roots are reusable across fills for one maker and profile generation;
Positions are reusable across counterparties. The table must not be interpreted
as a per-fill cost.

## Real-SVM campaign

The exact controller, claim, custody, and official SPL Token ELFs ran under
solana-program-test with SBF preferred. No native protocol processor or mock
token program was registered. The native Ed25519 precompile and runtime sysvars
provided signature and instruction evidence.

```sh
SBF_OUT_DIR=$PWD/target/deploy cargo test \
  --manifest-path crates/dclutch-svm-harness/Cargo.toml \
  --test physical_direct_composition -- --nocapture
```

| Case | Result | CU |
|---|---|---:|
| direct controller-PDA impersonation | refused | 7 |
| valid signatures, wrong replay bump | refused without mutation | 11,286 |
| valid signatures, wrong Position bump | refused without mutation | 14,457 |
| matcher price below signed seller limit | refused without mutation | 17,001 |
| signed fee-rate byte tampered after signing | native Ed25519 refusal before controller | 0 |
| admitted compiled fill | committed | 39,496 |
| frozen fee destination after first Token CPI | full rollback | 34,035 |

The committed fill advances both replay roots from 0 to 1, moves 2,000 selected
claims from seller to buyer, transfers 1,000 collateral units from buyer to
seller, transfers a floor fee of 2 to the venue, and clears the exact delegate
allowance. The late-refusal log contains a successful first official Token CPI;
the journal, both replay roots, both Positions, source, seller destination, and
venue destination nevertheless equal their pre-transaction bytes afterward.

## Boundary and next gates

The controller is now larger than the specialized children. The 568-byte
interpreter program is not the size problem: current Solana account, sysvar,
Ed25519-evidence, token-state, and CPI adapter code dominates the 56,048-byte
ELF. This measurement motivates generated codecs and descriptors, shared thin
adapters, and possibly a deliberately split authentication/execution program;
it does not justify reintroducing caller-authored plans or combined state.

Still open:

- immutable checked-release admission for the execution profile and controller
  artifact;
- Realm-derived collateral selection rather than the experimental profile's
  direct token bindings;
- generated safe-Rust and TypeScript codecs plus parser/refinement evidence;
- canonical prepaid account-creation and closure workflows;
- a new qedsvm lift for the canonical-owner claim artifact and broader
  machine-code coverage;
- Direct cancel, expiry, partial replay, retirement, and closure routes; and
- specialization of the same IR across the other capability families.

The earlier combined-projection claim artifact and its single-path qedsvm
theorem remain historical evidence only. They do not cover these bytes.
