# dClutch effect SBF experiment

This program is an isolated measurement adapter for the Lean-owned Effect IR.
It authenticates one signer named by a program-owned fixed-layout projection,
hostile-decodes one canonical plan, applies it transactionally through
`dclutch-effect-kernel`, and writes only the complete post-state.

It is **not** a deployable Direct market implementation. In particular, the
named signer is a deliberately explicit semantic-admission trust boundary; this
program does not authenticate intents, derive a Product, prove admissibility,
or move SPL collateral. A production multiprogram design would need a pinned
semantic-controller identity, authenticated controller PDA, real custody
accounts and CPI postconditions, and exact cross-program rollback evidence.

The purpose of this artifact is narrower: measure the code-size and verifier
shape of a fixed-data executor without conflating that result with the current
monolithic adapter or claiming architectural succession.
