# Semantic vector spine — data

Status: **PROPOSED**. Every file here is data, and every expectation in it is a
proposal that has not crossed the review gate of
[`docs/implementation/VECTOR_SPINE_PROPOSAL.md`](../../docs/implementation/VECTOR_SPINE_PROPOSAL.md)
§7. Nothing here is IMPLEMENTED, MODEL, or frozen. A green run of the checker is
one Rust implementation agreeing with a hand-written manifest; it is not a proof,
not a refinement, and not cross-runtime agreement.

This directory extends the fixture contract of [`../README.md`](../README.md):
synthetic, canonical, reproducible, with a provenance manifest, and no wallet
material, secrets, credentials, or unlicensed copied inputs.

## Layout

```text
TAXONOMY.json                 the §2 registry, machine-readable
SCHEMA.json                   the §3.3 JSON Schema
kernel/core.json              clutch-kernel
batch/scalar.json             clutch-batch, the scalar clearing lab
batch/relation-v1.json        clutch-batch, the coupled BatchRelationV1
accumulator/window.json       clutch-accumulator, the window state machine
adapter/reference-transition.json   solana-reference
```

## Ownership and direction (§6)

- **Vectors depend on nothing.** There is no `Cargo.toml` here, no build script,
  and no dependency on any crate. Every implementation depends on the vectors.
- The reader is [`tools/vector-check`](../../tools/vector-check), a standalone
  crate that depends on the vectors and on the semantic crates. Nothing depends
  on it, and no semantic crate depends on it. Rocq, Lean, and SBF readers, when
  they exist, are separate programs reading the same JSON.
- **An implementation may never edit a vector to make a test pass.** This is the
  same rule as `AGENTS.md`'s "Do not weaken a refusal to make an integration test
  pass." A disagreement is triaged as either an implementation defect or a vector
  defect; a vector defect mints a **new** vector id and marks the old one
  `superseded` with `superseded_by` (VER-5). Vectors are never silently
  corrected, and a frozen vector's bytes never change.

## How the expectations were derived

Every expected value in every vector was derived by **reading the
implementation** and, where a reference reading was useful, by reading the
implementation's own tests. No generator produced an expectation. A generator
that ran the implementation and wrote down whatever it returned would encode
today's bugs as tomorrow's truth, which is precisely the failure the spine
exists to prevent.

The one thing a program did produce is the digests: `digests.vector`,
`digests.manifest`, and `digests.taxonomy` are SHA-256 over RFC 8785 (JCS)
canonical JSON, per §3.5. The checker recomputes all three on every run and
refuses a placeholder outright (DIG-5), so a stale digest is a failure rather
than a silent pass.

## Determinism

Every file is UTF-8, ASCII-only, `sort_keys=True`, two-space indented, with a
trailing newline and no timestamps, hostnames, paths, or run identifiers. The
bytes are a function of the content alone, so a re-emit is a no-op and a diff is
always a semantic change.

## Deviations from the committed proposal

The proposal was written before three surfaces and one account landed. Nothing
below is hidden inside a file; each is stated in the file that carries it.

1. **`TAXONOMY.json` carries additive codes beyond §2.3.** §2.4 opens with "Six
   Rust error surfaces, 104 variants, all mapped." Those six numbered surfaces
   span nine error enums; the tree holds twelve error enums and 215 mapped rows.
   Three are mapped nowhere in §2.4 — `clutch_batch::relation_v1::ErrorV1`,
   `clutch_accumulator::WindowError`, and
   `clutch_solana_reference::ResolutionRefusal` — and three mapped surfaces have
   grown variants since. Every §2.3 code is transcribed verbatim, with its number,
   name, granularity, coarsening, and `by_design` flag unchanged; every added code
   carries `"origin": "extension"` and a new number in the correct band. Nothing
   is renumbered, repointed, or given a new meaning (TAX-2, VER-2, VER-3).
   `taxonomy_version` stays 1 because the §2.3 registry was never frozen — §7 is
   explicit that until G1–G7 are decided, the document is the artifact.
   `proposal_deltas` in that file is the machine-readable statement of this gap.
2. **Twelve `ModelError` variants are `unmapped-pending-review`.** They carry
   `"code": null`. TAX-8 makes an unmapped fact a taxonomy change under review,
   not a runtime string, and a reviewer rather than this lane should decide their
   codes. No vector may reference them, and the checker refuses a vector whose
   code the taxonomy does not define (VER-8).
3. **`SCHEMA.json` adds two `state.form` values.** `accumulator.window/v1` and
   `batch.relation-v1/v1`; see that file's `x-deviations-from-proposal`.
4. **The reader crate is `tools/vector-check`, not `tools/clutch-vectors`.** §6
   proposes the latter name. The landed directory is the former; the properties
   §6 asks for — host-only, `std`, standalone manifest, no library-target edge
   into any semantic crate — all hold.
5. **§4.3's identity table is not reproducible and was recomputed.**
   `canonical_profile_hash` now refuses any preimage that is not exactly
   `PROFILE_PARENT_BYTES` (64) long, so `canonical_profile_hash(b"fixture-profile")`
   returns `CodecError::Truncated` rather than the profile hash the table names.
   The adapter vectors carry the real derivations for the canonical 64-byte parent
   preimage with realm nonce 7 and market nonce 9, and the transition carries the
   seventh account (`SupplyLedgerAccount`) that §4.3 predates.

## Running the checker

```sh
cd tools/vector-check && cargo run --offline -- --root ../../fixtures/vectors
cd tools/vector-check && cargo test --offline
```

The report always prints a per-executor disposition table. Four of the five
executors do not exist; they are counted, named with their blocker, and never
skipped, so "the gate passed" can never quietly mean "one executor ran".
