# Architecture decision records

ADRs record decisions that change implementation direction or trust boundaries.
They do not turn a hypothesis into evidence. A proposed ADR may be superseded only
by another record that explains the migration and preserves historical context.

Statuses: `proposed`, `accepted`, `rejected`, `superseded`, `experimental`.

Every ADR should contain context, decision, consequences, rejected alternatives,
verification impact, legal/authority impact, and links to evidence.

Initial records:

- [0001](0001-specialized-transparent-batch-relation.md): specialized transparent
  relation instead of a generic matching VM.
- [0002](0002-hybrid-claim-representation.md): internal Positions plus optional
  one-outcome Token-2022 materialization.
- [0003](0003-verus-first-shadow-models.md): Verus-first executable kernel with
  independent Rocq and optional Lean shadow seam.
- [0004](0004-offline-first-mainnet-gate.md): offline-first engineering and the
  separate regulatory/deployment gate.
- [ADR-0005](0005-lean-proof-substrate-of-record.md) — Lean is the proof substrate of record (supersedes ADR-0003)
