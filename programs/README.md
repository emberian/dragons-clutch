# Planned Solana adapter

No Solana program exists yet. This directory is reserved for a minimal hostile-
byte/account/CPI adapter over the exact Eggcrate semantics.

It currently contains two offline prototypes, neither of which is an
entrypoint, a deployable program, a CPI adapter, or a verified artifact:

- `solana-layout` — a standalone dependency-free `no_std` byte-codec
  prototype (fixed record layouts, domain-separated identities, canonical
  padding). It has no Solana SDK, `AccountInfo`, PDA derivation, or SBF
  entrypoint.
- `solana-reference` — a `no_std`, safe, allocator-free offline transition
  adapter over the layout and kernel crates, exercised entirely in host tests.
  `Resolve` and `RedeemInternal` refuse unconditionally; it has no Solana SDK,
  `AccountInfo`, PDA derivation, SBF entrypoint, CPI, token program, RPC,
  signing, or deployment behavior.

The eventual adapter is not called verified. It must validate exact accounts, aliases,
owners, signer/writable status, PDAs, generations, token profiles, clocks, source
identity, and CPI construction. Local program-test work grants no devnet or
mainnet deployment authority.
