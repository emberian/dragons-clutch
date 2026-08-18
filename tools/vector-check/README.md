# `vector-check` — the `rust-reference` executor

Standalone host-only reader for [`fixtures/vectors`](../../fixtures/vectors). It
loads every manifest, runs every vector against the landed semantic crates, maps
each refusal through the taxonomy tables of
[`docs/implementation/VECTOR_SPINE_PROPOSAL.md`](../../docs/implementation/VECTOR_SPINE_PROPOSAL.md)
§2.4, and prints one disposition line per executor.

```sh
cargo run --offline -- --root ../../fixtures/vectors
cargo test  --offline
cargo clippy --offline --all-targets -- -D warnings
```

## What it is and is not

- It is the **first** executor of five. `verus-host`, `rocq-extracted`,
  `lean-checker`, and `sbf-program-test` do not exist; every vector declares
  their disposition and their named blocker, and the report counts them. A green
  run is one Rust implementation agreeing with a hand-written manifest.
- It is **not** a proof, a refinement, or a translation validation. Agreement on
  a finite vector set is agreement on that finite set.
- A failing vector is a **finding to triage**, never a reason to edit the vector
  (§6, and `AGENTS.md`'s "Do not weaken a refusal to make an integration test
  pass").

## Dependency direction

Path dependencies on `clutch-kernel`, `clutch-accumulator`, `clutch-batch`,
`clutch-solana-layout`, and `clutch-solana-reference`, all read-only. Nothing
depends on this crate, and no semantic crate gains any edge from it. There are no
registry dependencies at all: the JSON reader, the RFC 8785 canonicalizer, and
SHA-256 are written out here, so the crate builds offline exactly as the semantic
crates do.

## Where the taxonomy mapping lives

In `src/exec/*.rs`, not in the semantic crates. TAX-3 forbids serializing an enum
discriminant as a taxonomy code, and §6 forbids a vector depending on a crate, so
the translation belongs to a third party that depends on both. `CodecError`
already carries its own `code()` and this crate defers to it; every other surface
is mapped here.

## Checked rules

Beyond running the operations, the loader enforces the manifest discipline the
proposal states: INT-1/INT-2/INT-3 encoding, ARR-1/ARR-4 active prefixes, BYTE-1
hex, ENUM-1 closed enums, DIG-1/DIG-2/DIG-3 recomputed digests with DIG-5
placeholder refusal, TAX-4 result kinds, TAX-6/D5 declared and directional
coarsening, VER-1/VER-8 taxonomy pinning, COMP-1/COMP-3/COMP-4/COMP-5/COMP-6/
COMP-7, and D2/D7 reason and blocker tokens.
