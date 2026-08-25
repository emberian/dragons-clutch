# dClutch Product payoff SBF adapter

This is a deliberately narrow physical boundary around the generated,
fixed-memory Product payoff interpreter. It permissionlessly creates one
immutable 176-byte certificate for either:

- exact payoff evaluation at one in-domain coordinate; or
- comparison of one available amount with the conservative sum-of-amplitudes
  liability bound.

Before emitting either result, the adapter authenticates an exact 432-byte
Product record and an exact 216-byte artifact-release record. Both must be
rent-exempt finalized raw-record PDAs owned by the same Registry/Core program
and paired with vacant canonical staging PDAs. The release must name this
program, the fixed Product-payoff semantic release, and Upgradeable Loader V3.
The adapter then parses the current Program and ProgramData accounts, checks
their canonical link, hashes the complete ELF tail, and checks the deployment
slot and upgrade-authority policy against that release.

Certificates are PDAs over the Registry, both exact record digests, operation,
and query. Creation is permissionless and payer-funded. Repeating the exact
request is idempotent; an occupied PDA with any different byte refuses.

This program does **not** compile a user expression, select a Product or
release for a Market, create a Market, mint or burn claims, move collateral,
declare a liability bound minimal, or mutate any economic state. A Market or
other consumer must separately authorize the certificate's Registry, Product,
and artifact-release identities before lending it authority. The generated
Rust interpreter has finite differential agreement with the checked-in Lean
corpus and named Lean payoff-bound theorems; that is not universal source
refinement or a proof of Solana, SHA-256, Loader, compiler, or runtime behavior.

## Optimized SBF and real-ELF evidence

The checked source was built locally with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-product-payoff-sbf/Cargo.toml \
  --lto --optimize-size \
  --sbf-out-dir target/product-payoff-deploy
```

`cargo-build-sbf 4.0.0`, platform-tools v1.53, and SBF rustc 1.89.0
produced a verifier-clean 84,768-byte ELF with SHA-256
`a7dff1f21dafce7523eb30d9e66f71c4e7c14f3032a0d13b5263049e1e69f05a`.

The real-ELF Upgradeable Loader V3 ProgramTest campaign authenticated that
same complete ELF through its artifact-release record. It created an exact
evaluation certificate for coordinate 37 and payout 17 in 65,517 CU, then an
exact liability certificate for available 36 and bound 37 in 66,209 CU. Their
SHA-256 digests were respectively
`1d41582985b0f331fa0cd7a0aff5013924bd04fd32705720bfabaac10edbb7e1` and
`bd409f5ae8b24672cf40b7ea10c0c57dac225e87e9c987287f551011fb72d628`.
The same campaign refused a truncated request, an out-of-domain coordinate, a
canonical-record envelope containing noncanonical Product bytes, a
structurally valid artifact release naming a substituted ELF digest, and a
non-vacant staging PDA. Refused transactions created no certificate, and a
late wire refusal preserved a previously emitted certificate byte-for-byte.

These are local build and ProgramTest observations, not a checked release,
deployment, universal SBF refinement theorem, or mainnet evidence. A rebuild
changes the artifact-release and therefore certificate identities unless the
ELF is byte-identical.
