# Controller-PDA authority membrane — 2026-08-25

## Scope

This evidence concerns the experimental `dclutch-controller-proof-sbf` relay
and `dclutch-effect-proof-sbf` child. It establishes that a controller program
can derive and authenticate its own PDA, lend that PDA signer only through CPI,
and rely on transaction rollback when the child refuses after caller mutation.

It is not a Direct admission controller, a release-artifact authentication
mechanism, a custody adapter, or a proof of the Solana runtime. In particular,
the controller program used here is upgradeable in the test environment. A PDA
authenticates the program ID, not the presently deployed executable bytes. A
production successor must either bind the exact ProgramData generation on every
call or accept the controller only after its Loader V3 upgrade authority is
removed and the checked release is recorded.

## Exact programs

The exact-account Effect child is unchanged from commit `19692e0`:

- stripped ELF: 2,232 bytes;
- SHA-256:
  `552552310655e3339adace67847c4e8762d36ed861160187e2fffabfe173275b`;
- successful child execution: 155 CU.

The controller experiment was built with cargo-build-sbf 4.0.0, platform-tools
v1.53, and SBF rustc 1.89.0:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-controller-proof-sbf/Cargo.toml
```

- stripped ELF: 17,448 bytes;
- SHA-256:
  `b95d365fe552c0aee43c15b97be07bd814556fecde7fd8cb1ce6bfcc6acd9e2d`;
- Loader V3 `Rent::default()` permanent capitalization: 0.123783600 SOL;
- combined relay plus child capitalization: 0.141663840 SOL.

The relay intentionally uses the ordinary `solana-program` CPI and instruction
machinery. Its size and compute are a functional composition baseline, not the
target for generated code. The next controller generator should emit a fixed
account-frame adapter and borrowed CPI instruction instead of carrying generic
SDK account deserialization and allocation.

## Real-SVM adversarial campaign

`controller_authority_membrane.rs` loaded both exact ELFs through
`solana-program-test` 4.2.1. No native processor or mock child was registered.
The controller's account frame was exactly:

1. read-only controller PDA;
2. writable 16-byte controller-owned journal;
3. writable 104-byte child-owned Effect projection;
4. read-only executable Effect program.

Measured transaction totals were:

- direct nonsigning-PDA impersonation refused: 7 CU;
- authenticated controller relay and successful child: 4,048 CU;
- controller journal mutation followed by late child overflow: refused at
  3,958 CU;
- wrong controller bump: refused at 2,310 CU.

The successful relay incremented the journal once and produced the exact seven
expected child fields. The late child overflow occurred after the controller
incremented its journal; the failed transaction restored the complete journal
and child accounts byte-for-byte. The wrong-bump path also left both unchanged.

## Architectural result

The membrane does not require the claim executor to understand signed intents,
products, matching, or SPL Token. Its only semantic authority is a controller
PDA stored in child state. Conversely, the controller need not own or rewrite
the child's claim projection. This validates the three-part successor shape:

1. a release-authenticated admission controller derives a canonical plan;
2. a generated claim executor owns replay and claim-state effects;
3. a custody adapter performs the separately derived Realm-selected transfers.

The next physical gate is to replace the legacy seven-effect projection with
the Lean-owned four-effect claim plan, then compose its theorem with a real
two-transfer custody CPI campaign. Until that exists, the current experiment is
authority and rollback evidence only.
