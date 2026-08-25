# Physical Direct multiprogram composition — 2026-08-25

## Scope

This evidence concerns source commit
`803286409a4aeaf1ce052ffab98705088bff208b`. It composes four real SBF
artifacts in one transaction:

1. the Direct semantic-controller experiment;
2. the 1,872-byte Lean-owned claim executor;
3. a custody micro-adapter for the Lean-owned 40-byte transfer plan; and
4. the official legacy SPL Token program built from published crate 9.0.0.

The successful route updates replay/claim state and performs two
`TransferChecked` CPIs. The hostile route allows the first token CPI to finish,
then discovers a frozen fee destination and demonstrates transaction-wide
rollback across the controller, claim, and token programs.

This is not a complete Direct successor. The controller still accepts
pre-derived plans instead of authenticating signed intents and deriving the
unique admissible plans itself. It also pins the legacy Token program directly
instead of consuming an immutable Realm, and it does not authenticate exact
controller ProgramData or a checked release manifest.

## Reproducible artifacts

The three first-party programs were built from the exact source commit with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-claims-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
cargo build-sbf \
  --manifest-path programs/dclutch-controller-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
cargo build-sbf \
  --manifest-path programs/dclutch-custody-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
```

The token artifact was built without a native test processor from the
`spl-token` 9.0.0 crate source:

```sh
cargo build-sbf \
  --manifest-path <cargo-registry>/spl-token-9.0.0/Cargo.toml \
  --sbf-out-dir target/deploy
```

The published crate archive SHA-256 is
`878b0183d51fcd8a53e1604f4c13321894cf53227e6773c529b0d03d499a8dfd`.
Its `.cargo_vcs_info.json` binds `program/` to the official
[solana-program/token](https://github.com/solana-program/token) commit
`dfb260231c761be7d9c8b63728e770a102b86495`. The build used
cargo-build-sbf 4.0.0, platform-tools v1.53, and SBF rustc 1.89.0.

| Program | ELF bytes | SHA-256 | Equivalent Loader V3 capitalization |
|---|---:|---|---:|
| claim executor | 1,872 | `229f399d457d494bf5629545794edeee984a6c0437bad0293c4ff12fc4ad9569` | 0.015374640 SOL |
| controller experiment | 17,760 | `ae79fc70a4d827b5ba8a4359ef97ab9b9b772e3cdaa6ec2bd6cd51377b21b975` | 0.125955120 SOL |
| custody adapter | 24,800 | `c4f9a6ac223639158fb3f40d40b1e59ac1c1e369ff0c3c9c0667c1658f787796` | 0.174953520 SOL |
| first-party total | 44,432 | — | 0.316283280 SOL |
| SPL Token 9.0.0 | 93,056 | `c85ce043abbfcb0363b5c724245caa9d9201d2a9b669c02a5c2770512b65d78f` | 0.650015280 SOL |

Capitalization uses `Rent::default()`, one 36-byte Loader V3 Program account,
and 45 bytes of ProgramData metadata per program. SPL Token's number is an
equivalent local measurement, not dClutch deployment capital: the canonical
legacy token program is already deployed. Transient buffers, state accounts,
and transaction fees are excluded.

## Real-SVM evidence

`physical_direct_composition.rs` used `solana-program-test` 4.2.1, preferred
SBF, loaded all four exact ELFs, and registered no native processor or token
mock. Reproduce it with:

```sh
SBF_OUT_DIR="$PWD/target/deploy" \
  cargo test \
  --manifest-path crates/dclutch-svm-harness/Cargo.toml \
  --test physical_direct_composition -- --nocapture
```

Observed routes:

- direct controller-PDA impersonation refused in 7 CU;
- the wrong maker replay-root bump refused in 4,530 CU;
- the complete physical fill committed in 24,901 CU; and
- a frozen second destination refused in 19,420 CU after the first real Token
  `TransferChecked` logged success.

The committed example increments the journal and both replay nonces, changes
the claim balances to 3,000 and 2,200, transfers 1,000 base units to the seller
and 2 to the venue, and consumes the source delegate's exact 1,002-unit
allowance. The hostile example compares the complete controller journal, claim
projection, source, seller, and venue accounts before and after refusal; all
five are restored byte-for-byte.

## Correctness boundary

Lean owns the admitted example's claim and custody plans, exact quote and fee
arithmetic, conservation statement, canonical bytes, and abstract atomic
transition. The claim executor has a separately pinned qedsvm successful-path
theorem. The custody adapter and controller are safe Rust adapter experiments,
and this campaign is runtime evidence rather than a proof of Solana, SPL Token,
their CPI composition, or hostile-path completeness.

The experiment removes the prior false model in which executor-owned integers
stood in for collateral. Real token accounts now move under the existing
Direct V2 maker replay-root delegate. The next gate is to stop accepting effect
plans from instruction data: the controller must authenticate the signed
intent, Product, Realm, release, and chain frame, then derive the only plans it
is willing to authorize.
