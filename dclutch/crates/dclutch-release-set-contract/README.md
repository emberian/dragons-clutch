# dClutch execution release set contract

This allocation-free contract owns the one canonical semantic map from five
execution roles—Core, Claims, Trading, Resolution, and Custody—to exact program
identities and checked artifact-release content identities.

The profile is deliberately not “five distinct programs.” Roles may share one
identical `(program, artifact release)` pair. The codec refuses either half of
that pair being aliased independently, because one live program identity cannot
simultaneously denote two artifact releases and one artifact release cannot
authorize two program identities.

The 336-byte value is a content-addressed preimage. This crate does not hash it,
inspect Loader accounts, admit a release, read a Market, or execute a CPI. A
composing adapter must:

1. authenticate a finalized record under
   `EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1`;
2. hash the exact record bytes and match the one release-set identity selected
   by the immutable Market capability profile;
3. resolve every referenced artifact release through checked Program,
   ProgramData, deployment-slot, ELF, and upgrade-policy evidence; and
4. persist or authenticate one activation binding before lending any
   controller PDA authority.

`CapabilityExecutionSelectionV1` is the Lean-owned, generated-layout projection
from one exact manifest entry to the fixed Trading role. It carries the
manifest, entry index, kind, semantic capability release, and config—never a
Program, artifact release, or family tag. Core reconstructs this 144-byte value
for activation and closure, then selects the executable only through the
Market's Trading binding. Hot actions authenticate the selection persisted in
the Trading-owned child root and do not repeat the prefix.

The capability entry's `release_id` remains semantic capability authority. It
is not an artifact-release ID and cannot be used as a dynamic Program registry
key. General, Dealer, Direct, Series, and later data-defined families share the
one Registry-selected Trading interpreter.

Run `./check-generated.sh` from this directory to prove the checked-in Rust
layout is the exact output of the Lean ABI emitter.
