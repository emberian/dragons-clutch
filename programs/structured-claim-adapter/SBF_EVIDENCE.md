# Isolated SBF and stack evidence

Measured 2026-08-23 with `cargo-build-sbf 4.0.0`, Solana platform-tools
`v1.53` (`rustc 1.89.0`, LLVM 20.1.7-rust-dev), release profile:

```sh
RUSTFLAGS='-Z emit-stack-sizes' cargo build-sbf --offline
```

The adapter rlib compiled successfully. The command also reports pre-existing
oversized frames in dormant, unrelated full-profile `clutch-batch`,
`clutch-batch-policy-identity`, and broad `clutch-solana-layout` functions;
those functions are not adapter frames and the rlib build still exits zero.
This evidence is for the isolated adapter object, not a linked dispatcher ELF.

The current adapter rlib was inspected with:

```sh
llvm-readobj --stack-sizes \
  target/sbpf-solana-solana/release/deps/libclutch_structured_claim_adapter-*.rlib
```

Selected adapter frames:

| function | SBF frame bytes |
| --- | ---: |
| `plan_route_solana` | 64 |
| `plan_route_into` | 64 |
| preflight coordinator | 64 |
| context/access validation | 576 |
| descriptor/claim/PDA binding coordinator | 640 |
| `bind_descriptor` | 960 |
| market reconstruction | 0 |
| machine reconstruction | 1,216 |
| holder reconstruction | 192 |
| core-transition dispatch | 320 |
| CPI plan + complete post-state staging | 2,752 |
| descriptor hostile decode | 896 |
| product hashing | 640 |
| receipt reconciliation | 64 |
| post-state reconciliation | 64 |

Every adapter-owned frame is below the 4,096-byte SBF frame ceiling. The
largest leaves 1,344 bytes of per-frame margin. The caller-owned
`RouteScratch` is 5,256 bytes on the measured host ABI and therefore must live
on the requested heap, not the SBF stack. The structured-claim core's separately
measured largest frame is `redeem_terminal` at 3,008 bytes.

This is not linked-program compute-unit, CPI-depth, account-count, heap, rent,
or live-bank evidence. Those measurements remain promotion gates after the
small dispatcher seam and missing base CPI instructions exist.
