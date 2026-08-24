# Withdrawn model-only SBF stack evidence

The `MarketLedger`/`StructuredClaimMachine` transition model measured below was
deleted when current HoardV2/ClaimLedgerV3/PositionV3/ReplayV3 execution became
the sole implementation boundary. These numbers are retained only as historical
provenance and are not evidence for any current Structured route or release.

Measured 2026-08-22 with `cargo-build-sbf 4.0.0`, Solana platform-tools
`v1.53` (`rustc 1.89.0`, LLVM 20.1.7-rust-dev), using the release profile.

The reproducible build command was:

```sh
RUSTFLAGS='-Z emit-stack-sizes' cargo build-sbf --manifest-path Cargo.toml
```

The generated `rlib` members were extracted with the matching `llvm-ar`, then
read with:

```sh
llvm-readelf --stack-sizes clutch_structured_claim-*.o
```

Selected frame sizes:

| function | SBF frame bytes |
| --- | ---: |
| `realize_rational_shape` | 1,280 |
| `CompositionAccumulator::finish` | 576 |
| `wrap_canonical` | 768 |
| `wrap_full` | 2,688 |
| `unwind_canonical` | 768 |
| `unwind_full` | 2,688 |
| `direct_burn` | 704 |
| `compact_donation` | 2,496 |
| `terminal_lot` | 576 |
| `redeem_terminal` | 3,008 |
| `transfer_wrappers` | 320 |
| base `donate_collateral` | 64 |
| base `donate_internal_vector` | 576 |
| base `redeem_internal_vector_exact` | 512 |

These deleted-model measurements are not evidence for the current adapter's
full call path, account decoding, CPI depth, compute units, heap use, or bank
behavior. A checked current release still requires new measurements over the
exact linked wrapper and base ELFs.
