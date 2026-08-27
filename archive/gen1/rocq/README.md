# Rocq shadow model (superseded; historical)

Status 2026-08-22: this directory holds a 428-line partial model untouched
since the initial commit, with zero proved theorems. ADR-0005 adopted
**Lean as the proof substrate of record** (`lean/` — zero-sorry theorems,
zero project axioms, zero dependencies); the Rocq role is retired and this
tree is kept as historical specification only. Nothing here gates
anything; `proof.rocq_check` remains labeled non-proof-content in the
manifest.

The original intent (a Rust-independent model with extraction as a
differential oracle) is realized by the Lean model plus the CHECKED-FINITE
differential corpora instead.
