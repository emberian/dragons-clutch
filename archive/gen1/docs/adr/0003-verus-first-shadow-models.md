# ADR-0003: Verus-first kernel with independent shadow models

Status: superseded by [ADR-0005](0005-lean-proof-substrate-of-record.md) (2026-08-20)

## Context

The executable economic kernel should be proved close to its Rust semantics, but
Verus does not prove Anza's SBF compiler or provide the best language for every
reachable-state theorem. Maintaining multiple production implementations would
create semantic drift.

## Decision

Use Verus as the V1 executable-kernel gate. Maintain a Rust-independent Rocq model
for abstract reachable-state theorems and extraction as a differential oracle.
Keep a language-neutral seam for Lean to reproduce small finite relations or
vector checks without making Lean a duplicate production implementation. Compare
all layers through canonical vectors and named property IDs.

## Consequences

- The exact dual-toolchain compatibility spike is an early stop gate.
- Manual correspondence among models, Eggcrate, adapter, and SBF is disclosed.
- A successful proof never implies the adapter/runtime/ELF is proved.
- Lean can add independent confidence without becoming mandatory by inertia.

## Evidence required

Pinned toolchains, prohibited-shortcut audit, theorem/assumption inventories,
host/SBF differential vectors, mutation tests, source/proof/ELF digests, and an
explicit proceed/redesign/reject record.

## Authority impact

None. Verification does not close Gate L0 or authorize a deployment.
