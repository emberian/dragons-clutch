# Dependency closure

§4.1 of `docs/design/MAINNET_STATE_RELAY.md` states the one rule the
`RelayedMainnetStateV1` family exists to enforce:

> The relayer signs **observations**. It never signs interpretations.
>
> This is enforceable rather than aspirational, and the release check is a
> one-liner: **the relayer daemon's dependency closure must contain no venue
> IDL, SDK, or layout crate.** A relayer that cannot parse a `PoolState` cannot
> interpret one.

§6.3 item 8 makes that a release-evidence obligation: *"Relayer independence,
checked mechanically. The daemon crate's resolved dependency closure contains no
venue IDL, SDK, or layout crate."*

This file is that check, and the full `cargo tree` below is the evidence.

## The check

Run from `tools/relayer/`:

```sh
cargo tree -e normal --prefix none \
  | grep -iE 'anchor|meteora|pump|raydium|orca|whirlpool|jupiter|serum|openbook|dlmm|lifinity|phoenix|borsh|idl'
```

**Result at the state committed here: no matches.**

What that rules out, stated so the check can be argued with rather than trusted:

| Class | Present? | Note |
| --- | --- | --- |
| Anchor (`anchor-lang`, `anchor-client`, `anchor-spl`) | no | Anchor is the usual carrier of a venue IDL |
| Venue crates (`meteora-*`, `pump*`, `raydium*`, `orca*`, `whirlpool*`, `jupiter*`, `dlmm*`, `lifinity*`, `phoenix*`) | no | |
| `serum`/`openbook` DEX layout crates | no | |
| `borsh` / `borsh-derive` | no | the usual derive path for a venue account layout; `solana-instruction` is pulled with `default-features = false` precisely so its optional `borsh` feature stays off |
| `spl-token`, `spl-token-2022`, `spl-associated-token-account` | no | a mint layout is still a layout |

What *is* present, and why it is not an interpretation surface:

- `solana-address`, `solana-hash`, `solana-instruction`, `solana-message`,
  `solana-signature`, `solana-transaction` — transaction *construction*
  primitives for the execution-gated submit path. They describe how bytes are
  addressed and framed, never what any account's bytes mean.
- `sha2` — the daemon computes the digests the wire crate refuses to compute.
- `ed25519-dalek`, `curve25519-dalek` — signing, and PDA derivation's
  on-curve check.
- `reqwest`/`hyper`/`rustls`/`tokio` — the HTTP transport.
- `serde_json` — JSON-RPC. Note that the daemon decodes `getMultipleAccounts`
  *envelopes*: `lamports`, `owner`, `executable`, `space`, and a base64 blob it
  never looks inside.
- `dclutch-relay-contract` and its `dclutch-*` dependencies — the wire codec,
  which is `no_std` and hashes nothing.

The only account layouts named anywhere in this crate are the Loader V3
`ProgramData` 45-byte metadata prefix and the `Clock` sysvar, both first-party
runtime structures rather than venue state, and both used for exactly the
purposes `src/chain.rs` documents.

## A caveat, stated rather than glossed

`cargo tree` is a check on the *resolved closure*, not a proof of behaviour. It
would not catch a venue layout hand-transcribed into this crate's own source,
which is why `src/chain.rs` carries the second half of the argument in prose and
why the observation path builds every signed byte through
`dclutch_relay_contract::wire`.

## Full `cargo tree -e normal`

Generated from `tools/relayer/` with the toolchain pinned by the repository's
`rust-toolchain.toml`.

```text
dclutch-relayer v0.1.0 (/Users/ember/dev/dclutch/tools/relayer)
├── base64 v0.22.1
├── bincode v1.3.3
│   └── serde v1.0.229
│       ├── serde_core v1.0.229
│       └── serde_derive v1.0.229 (proc-macro)
│           ├── proc-macro2 v1.0.107
│           │   └── unicode-ident v1.0.24
│           ├── quote v1.0.47
│           │   └── proc-macro2 v1.0.107 (*)
│           └── syn v3.0.4
│               ├── proc-macro2 v1.0.107 (*)
│               ├── quote v1.0.47 (*)
│               └── unicode-ident v1.0.24
├── bs58 v0.5.1
├── clap v4.6.6
│   ├── clap_builder v4.6.6
│   │   ├── anstream v1.0.0
│   │   │   ├── anstyle v1.0.14
│   │   │   ├── anstyle-parse v1.0.0
│   │   │   │   └── utf8parse v0.2.2
│   │   │   ├── anstyle-query v1.1.5
│   │   │   ├── colorchoice v1.0.5
│   │   │   ├── is_terminal_polyfill v1.70.2
│   │   │   └── utf8parse v0.2.2
│   │   ├── anstyle v1.0.14
│   │   ├── clap_lex v1.1.0
│   │   └── strsim v0.11.1
│   └── clap_derive v4.6.4 (proc-macro)
│       ├── heck v0.5.0
│       ├── proc-macro2 v1.0.107 (*)
│       ├── quote v1.0.47 (*)
│       └── syn v3.0.4 (*)
├── dclutch-relay-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-relay-contract)
│   ├── dclutch-core-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-core-contract)
│   ├── dclutch-registry-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-registry-contract)
│   │   ├── dclutch-core-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-core-contract)
│   │   └── dclutch-release-set-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-release-set-contract)
│   │       └── dclutch-core-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-core-contract)
│   ├── dclutch-registry-svm v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-registry-svm)
│   │   ├── dclutch-core-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-core-contract)
│   │   └── dclutch-release-set-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-release-set-contract) (*)
│   ├── dclutch-release-set-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-release-set-contract) (*)
│   └── dclutch-source-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-source-contract)
│       ├── dclutch-product-contract v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-product-contract)
│       └── dclutch-product-runtime-v2 v0.1.0 (/Users/ember/dev/dclutch/crates/dclutch-product-runtime-v2)
├── ed25519-dalek v2.2.0
│   ├── curve25519-dalek v4.1.3
│   │   ├── cfg-if v1.0.4
│   │   ├── digest v0.10.7
│   │   │   ├── block-buffer v0.10.4
│   │   │   │   └── generic-array v0.14.7
│   │   │   │       └── typenum v1.20.1
│   │   │   └── crypto-common v0.1.7
│   │   │       ├── generic-array v0.14.7 (*)
│   │   │       └── typenum v1.20.1
│   │   ├── rand_core v0.6.4
│   │   │   └── getrandom v0.2.17
│   │   │       ├── cfg-if v1.0.4
│   │   │       └── libc v0.2.189
│   │   ├── subtle v2.6.1
│   │   └── zeroize v1.9.0
│   ├── ed25519 v2.2.3
│   │   └── signature v2.2.0
│   ├── rand_core v0.6.4 (*)
│   ├── sha2 v0.10.9
│   │   ├── cfg-if v1.0.4
│   │   ├── cpufeatures v0.2.17
│   │   │   └── libc v0.2.189
│   │   └── digest v0.10.7 (*)
│   ├── subtle v2.6.1
│   └── zeroize v1.9.0
├── hex v0.4.3
├── rand_core v0.6.4 (*)
├── reqwest v0.12.28
│   ├── base64 v0.22.1
│   ├── bytes v1.12.1
│   ├── futures-core v0.3.34
│   ├── http v1.5.0
│   │   ├── bytes v1.12.1
│   │   └── itoa v1.0.18
│   ├── http-body v1.1.0
│   │   ├── bytes v1.12.1
│   │   └── http v1.5.0 (*)
│   ├── http-body-util v0.1.5
│   │   ├── bytes v1.12.1
│   │   ├── futures-core v0.3.34
│   │   ├── http v1.5.0 (*)
│   │   ├── http-body v1.1.0 (*)
│   │   └── pin-project-lite v0.2.17
│   ├── hyper v1.11.0
│   │   ├── atomic-waker v1.1.2
│   │   ├── bytes v1.12.1
│   │   ├── futures-channel v0.3.34
│   │   │   └── futures-core v0.3.34
│   │   ├── futures-core v0.3.34
│   │   ├── http v1.5.0 (*)
│   │   ├── http-body v1.1.0 (*)
│   │   ├── httparse v1.10.1
│   │   ├── itoa v1.0.18
│   │   ├── pin-project-lite v0.2.17
│   │   ├── smallvec v1.15.2
│   │   ├── tokio v1.53.1
│   │   │   ├── bytes v1.12.1
│   │   │   ├── libc v0.2.189
│   │   │   ├── mio v1.2.2
│   │   │   │   └── libc v0.2.189
│   │   │   ├── pin-project-lite v0.2.17
│   │   │   ├── socket2 v0.6.5
│   │   │   │   └── libc v0.2.189
│   │   │   └── tokio-macros v2.7.2 (proc-macro)
│   │   │       ├── proc-macro2 v1.0.107 (*)
│   │   │       ├── quote v1.0.47 (*)
│   │   │       └── syn v3.0.4 (*)
│   │   └── want v0.3.1
│   │       └── try-lock v0.2.5
│   ├── hyper-rustls v0.27.9
│   │   ├── http v1.5.0 (*)
│   │   ├── hyper v1.11.0 (*)
│   │   ├── hyper-util v0.1.20
│   │   │   ├── base64 v0.22.1
│   │   │   ├── bytes v1.12.1
│   │   │   ├── futures-channel v0.3.34 (*)
│   │   │   ├── futures-util v0.3.34
│   │   │   │   ├── futures-core v0.3.34
│   │   │   │   ├── futures-task v0.3.34
│   │   │   │   ├── pin-project-lite v0.2.17
│   │   │   │   └── slab v0.4.12
│   │   │   ├── http v1.5.0 (*)
│   │   │   ├── http-body v1.1.0 (*)
│   │   │   ├── hyper v1.11.0 (*)
│   │   │   ├── ipnet v2.12.1
│   │   │   ├── libc v0.2.189
│   │   │   ├── percent-encoding v2.3.2
│   │   │   ├── pin-project-lite v0.2.17
│   │   │   ├── socket2 v0.6.5 (*)
│   │   │   ├── tokio v1.53.1 (*)
│   │   │   ├── tower-service v0.3.3
│   │   │   └── tracing v0.1.44
│   │   │       ├── pin-project-lite v0.2.17
│   │   │       └── tracing-core v0.1.36
│   │   │           └── once_cell v1.21.4
│   │   ├── rustls v0.23.43
│   │   │   ├── once_cell v1.21.4
│   │   │   ├── ring v0.17.14
│   │   │   │   ├── cfg-if v1.0.4
│   │   │   │   ├── getrandom v0.2.17 (*)
│   │   │   │   ├── libc v0.2.189
│   │   │   │   └── untrusted v0.9.0
│   │   │   ├── rustls-pki-types v1.15.1
│   │   │   │   └── zeroize v1.9.0
│   │   │   ├── rustls-webpki v0.103.15
│   │   │   │   ├── ring v0.17.14 (*)
│   │   │   │   ├── rustls-pki-types v1.15.1 (*)
│   │   │   │   └── untrusted v0.9.0
│   │   │   ├── subtle v2.6.1
│   │   │   └── zeroize v1.9.0
│   │   ├── tokio v1.53.1 (*)
│   │   ├── tokio-rustls v0.26.4
│   │   │   ├── rustls v0.23.43 (*)
│   │   │   └── tokio v1.53.1 (*)
│   │   ├── tower-service v0.3.3
│   │   └── webpki-roots v1.0.9
│   │       └── rustls-pki-types v1.15.1 (*)
│   ├── hyper-util v0.1.20 (*)
│   ├── log v0.4.34
│   ├── percent-encoding v2.3.2
│   ├── pin-project-lite v0.2.17
│   ├── rustls v0.23.43 (*)
│   ├── rustls-pki-types v1.15.1 (*)
│   ├── serde v1.0.229 (*)
│   ├── serde_json v1.0.151
│   │   ├── indexmap v2.14.0
│   │   │   ├── equivalent v1.0.2
│   │   │   └── hashbrown v0.17.1
│   │   ├── itoa v1.0.18
│   │   ├── memchr v2.8.3
│   │   ├── serde_core v1.0.229
│   │   └── zmij v1.0.23
│   ├── serde_urlencoded v0.7.1
│   │   ├── form_urlencoded v1.2.2
│   │   │   └── percent-encoding v2.3.2
│   │   ├── itoa v1.0.18
│   │   ├── ryu v1.0.23
│   │   └── serde v1.0.229 (*)
│   ├── sync_wrapper v1.0.2
│   │   └── futures-core v0.3.34
│   ├── tokio v1.53.1 (*)
│   ├── tokio-rustls v0.26.4 (*)
│   ├── tower v0.5.3
│   │   ├── futures-core v0.3.34
│   │   ├── futures-util v0.3.34 (*)
│   │   ├── pin-project-lite v0.2.17
│   │   ├── sync_wrapper v1.0.2 (*)
│   │   ├── tokio v1.53.1 (*)
│   │   ├── tower-layer v0.3.3
│   │   └── tower-service v0.3.3
│   ├── tower-http v0.6.11
│   │   ├── bitflags v2.13.1
│   │   ├── bytes v1.12.1
│   │   ├── futures-util v0.3.34 (*)
│   │   ├── http v1.5.0 (*)
│   │   ├── http-body v1.1.0 (*)
│   │   ├── pin-project-lite v0.2.17
│   │   ├── tower v0.5.3 (*)
│   │   ├── tower-layer v0.3.3
│   │   ├── tower-service v0.3.3
│   │   └── url v2.5.8
│   │       ├── form_urlencoded v1.2.2 (*)
│   │       ├── idna v1.1.0
│   │       │   ├── idna_adapter v1.2.2
│   │       │   │   ├── icu_normalizer v2.3.0
│   │       │   │   │   ├── icu_collections v2.3.0
│   │       │   │   │   │   ├── displaydoc v0.2.7 (proc-macro)
│   │       │   │   │   │   │   ├── proc-macro2 v1.0.107 (*)
│   │       │   │   │   │   │   ├── quote v1.0.47 (*)
│   │       │   │   │   │   │   └── syn v3.0.4 (*)
│   │       │   │   │   │   ├── potential_utf v0.1.6
│   │       │   │   │   │   │   └── zerovec v0.11.8
│   │       │   │   │   │   │       ├── yoke v0.8.3
│   │       │   │   │   │   │       │   ├── stable_deref_trait v1.2.1
│   │       │   │   │   │   │       │   ├── yoke-derive v0.8.2 (proc-macro)
│   │       │   │   │   │   │       │   │   ├── proc-macro2 v1.0.107 (*)
│   │       │   │   │   │   │       │   │   ├── quote v1.0.47 (*)
│   │       │   │   │   │   │       │   │   ├── syn v2.0.119
│   │       │   │   │   │   │       │   │   │   ├── proc-macro2 v1.0.107 (*)
│   │       │   │   │   │   │       │   │   │   ├── quote v1.0.47 (*)
│   │       │   │   │   │   │       │   │   │   └── unicode-ident v1.0.24
│   │       │   │   │   │   │       │   │   └── synstructure v0.13.2
│   │       │   │   │   │   │       │   │       ├── proc-macro2 v1.0.107 (*)
│   │       │   │   │   │   │       │   │       ├── quote v1.0.47 (*)
│   │       │   │   │   │   │       │   │       └── syn v2.0.119 (*)
│   │       │   │   │   │   │       │   └── zerofrom v0.1.8
│   │       │   │   │   │   │       │       └── zerofrom-derive v0.1.7 (proc-macro)
│   │       │   │   │   │   │       │           ├── proc-macro2 v1.0.107 (*)
│   │       │   │   │   │   │       │           ├── quote v1.0.47 (*)
│   │       │   │   │   │   │       │           ├── syn v2.0.119 (*)
│   │       │   │   │   │   │       │           └── synstructure v0.13.2 (*)
│   │       │   │   │   │   │       ├── zerofrom v0.1.8 (*)
│   │       │   │   │   │   │       └── zerovec-derive v0.11.6 (proc-macro)
│   │       │   │   │   │   │           ├── proc-macro2 v1.0.107 (*)
│   │       │   │   │   │   │           ├── quote v1.0.47 (*)
│   │       │   │   │   │   │           └── syn v3.0.4 (*)
│   │       │   │   │   │   ├── utf8_iter v1.0.4
│   │       │   │   │   │   ├── yoke v0.8.3 (*)
│   │       │   │   │   │   ├── zerofrom v0.1.8 (*)
│   │       │   │   │   │   └── zerovec v0.11.8 (*)
│   │       │   │   │   ├── icu_normalizer_data v2.3.0
│   │       │   │   │   ├── icu_provider v2.3.1
│   │       │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
│   │       │   │   │   │   ├── icu_locale_core v2.3.0
│   │       │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
│   │       │   │   │   │   │   ├── litemap v0.8.3
│   │       │   │   │   │   │   ├── tinystr v0.8.4
│   │       │   │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
│   │       │   │   │   │   │   │   └── zerovec v0.11.8 (*)
│   │       │   │   │   │   │   ├── writeable v0.6.4
│   │       │   │   │   │   │   └── zerovec v0.11.8 (*)
│   │       │   │   │   │   ├── writeable v0.6.4
│   │       │   │   │   │   ├── yoke v0.8.3 (*)
│   │       │   │   │   │   ├── zerofrom v0.1.8 (*)
│   │       │   │   │   │   ├── zerotrie v0.2.5
│   │       │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
│   │       │   │   │   │   │   ├── yoke v0.8.3 (*)
│   │       │   │   │   │   │   └── zerofrom v0.1.8 (*)
│   │       │   │   │   │   └── zerovec v0.11.8 (*)
│   │       │   │   │   ├── smallvec v1.15.2
│   │       │   │   │   └── zerovec v0.11.8 (*)
│   │       │   │   └── icu_properties v2.3.0
│   │       │   │       ├── displaydoc v0.2.7 (proc-macro) (*)
│   │       │   │       ├── icu_collections v2.3.0 (*)
│   │       │   │       ├── icu_locale_core v2.3.0 (*)
│   │       │   │       ├── icu_properties_data v2.3.0
│   │       │   │       ├── icu_provider v2.3.1 (*)
│   │       │   │       ├── zerotrie v0.2.5 (*)
│   │       │   │       └── zerovec v0.11.8 (*)
│   │       │   ├── smallvec v1.15.2
│   │       │   └── utf8_iter v1.0.4
│   │       └── percent-encoding v2.3.2
│   ├── tower-service v0.3.3
│   ├── url v2.5.8 (*)
│   └── webpki-roots v1.0.9 (*)
├── serde v1.0.229 (*)
├── serde_json v1.0.151 (*)
├── sha2 v0.10.9 (*)
├── solana-address v2.6.1
│   ├── curve25519-dalek v4.1.3 (*)
│   ├── five8 v1.0.0
│   │   └── five8_core v1.0.0
│   ├── five8_const v1.0.0
│   │   └── five8_core v1.0.0
│   ├── serde v1.0.229 (*)
│   ├── serde_derive v1.0.229 (proc-macro) (*)
│   ├── sha2-const-stable v0.1.0
│   ├── solana-program-error v3.0.1
│   ├── solana-sanitize v3.0.1
│   ├── solana-sha256-hasher v3.1.0
│   │   ├── sha2 v0.10.9 (*)
│   │   └── solana-hash v4.5.0
│   │       ├── five8 v1.0.0 (*)
│   │       ├── serde v1.0.229 (*)
│   │       ├── serde_derive v1.0.229 (proc-macro) (*)
│   │       ├── solana-sanitize v3.0.1
│   │       └── wincode v0.5.5
│   │           ├── pastey v0.2.3 (proc-macro)
│   │           ├── thiserror v2.0.20
│   │           │   └── thiserror-impl v2.0.20 (proc-macro)
│   │           │       ├── proc-macro2 v1.0.107 (*)
│   │           │       ├── quote v1.0.47 (*)
│   │           │       └── syn v3.0.4 (*)
│   │           └── wincode-derive v0.4.6 (proc-macro)
│   │               ├── darling v0.23.0
│   │               │   ├── darling_core v0.23.0
│   │               │   │   ├── ident_case v1.0.1
│   │               │   │   ├── proc-macro2 v1.0.107 (*)
│   │               │   │   ├── quote v1.0.47 (*)
│   │               │   │   ├── strsim v0.11.1
│   │               │   │   └── syn v2.0.119 (*)
│   │               │   └── darling_macro v0.23.0 (proc-macro)
│   │               │       ├── darling_core v0.23.0 (*)
│   │               │       ├── quote v1.0.47 (*)
│   │               │       └── syn v2.0.119 (*)
│   │               ├── proc-macro2 v1.0.107 (*)
│   │               ├── quote v1.0.47 (*)
│   │               └── syn v2.0.119 (*)
│   └── wincode v0.5.5 (*)
├── solana-hash v4.5.0 (*)
├── solana-instruction v3.4.1
│   ├── solana-instruction-error v2.4.0
│   │   ├── num-traits v0.2.19
│   │   ├── serde v1.0.229 (*)
│   │   ├── serde_derive v1.0.229 (proc-macro) (*)
│   │   └── solana-program-error v3.0.1
│   └── solana-pubkey v4.2.1
│       └── solana-address v2.6.1 (*)
├── solana-message v4.4.1
│   ├── serde v1.0.229 (*)
│   ├── serde_derive v1.0.229 (proc-macro) (*)
│   ├── solana-address v2.6.1 (*)
│   ├── solana-hash v4.5.0 (*)
│   ├── solana-instruction v3.4.1 (*)
│   ├── solana-sanitize v3.0.1
│   ├── solana-sdk-ids v3.1.0
│   │   └── solana-address v2.6.1 (*)
│   ├── solana-short-vec v3.2.2
│   │   ├── serde_core v1.0.229
│   │   └── wincode v0.5.5 (*)
│   ├── solana-transaction-error v3.3.2
│   │   ├── serde v1.0.229 (*)
│   │   ├── serde_derive v1.0.229 (proc-macro) (*)
│   │   ├── solana-instruction-error v2.4.0 (*)
│   │   └── solana-sanitize v3.0.1
│   └── wincode v0.5.5 (*)
├── solana-signature v3.4.1
│   ├── five8 v1.0.0 (*)
│   ├── serde v1.0.229 (*)
│   ├── serde-big-array v0.5.1
│   │   └── serde v1.0.229 (*)
│   ├── serde_derive v1.0.229 (proc-macro) (*)
│   └── solana-sanitize v3.0.1
├── solana-transaction v4.1.6
│   ├── serde v1.0.229 (*)
│   ├── serde_derive v1.0.229 (proc-macro) (*)
│   ├── solana-address v2.6.1 (*)
│   ├── solana-hash v4.5.0 (*)
│   ├── solana-instruction v3.4.1 (*)
│   ├── solana-instruction-error v2.4.0 (*)
│   ├── solana-message v4.4.1 (*)
│   ├── solana-sanitize v3.0.1
│   ├── solana-sdk-ids v3.1.0 (*)
│   ├── solana-short-vec v3.2.2 (*)
│   ├── solana-signature v3.4.1 (*)
│   └── solana-transaction-error v3.3.2 (*)
├── thiserror v2.0.20 (*)
├── tokio v1.53.1 (*)
└── toml v0.8.23
    ├── serde v1.0.229 (*)
    ├── serde_spanned v0.6.9
    │   └── serde v1.0.229 (*)
    ├── toml_datetime v0.6.11
    │   └── serde v1.0.229 (*)
    └── toml_edit v0.22.27
        ├── indexmap v2.14.0 (*)
        ├── serde v1.0.229 (*)
        ├── serde_spanned v0.6.9 (*)
        ├── toml_datetime v0.6.11 (*)
        ├── toml_write v0.1.2
        └── winnow v0.7.15
```
