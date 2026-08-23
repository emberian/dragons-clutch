# `clutch-structured-claim-adapter`

This crate is the allocation-free runtime contract between the pure
`clutch-structured-claim` economics and a future small SBF/Token-2022 adapter.
It freezes the exact 384-byte descriptor, reconstructs native-claim and
deployment-bound product identity from authenticated basis/deployment facts,
and stages the one required atomic Position cash/native-Egg transfer without
touching global supply or Hoard collateral.

It does not parse Solana accounts, derive PDAs, hash, invoke CPI, or claim that
structured claims are live. The SBF adapter remains responsible for exact
owner/PDA/ProgramData/slot/Token-2022 authentication, Replay account binding,
transaction execution, post-delta checks, and rollback.

The canonical and full-vector wrap/unwind planners join actual mint supply and
holder balance to the pure economic machine, stage exact Market and
Position/Replay poststates, and return the precise `MintToChecked` or
`BurnChecked` quantity. Full routes include the authoritative base complete-set
Merge/Split poststate rather than simulating it as a transfer. The runtime also
stages beneficiary-free surplus compaction and exact resolved terminal-lot
redemption. These routes never spend reserved Position cash.

Retirement now requires zero actual mint supply, empty canonical backing, and
an authenticated successor base-Position close receipt. The descriptor and
extension-free mint remain permanent identity tombstones; retirement revokes
mint authority instead of pretending Token-2022 can close an extension-free
mint or redirect its locked rent.

The family-local wire allocates eight strict actions: descriptor creation,
canonical/full wrap, canonical/full unwind, beneficiary-free donation
compaction, exact terminal redemption, and retirement. Every quantity route
binds the wrapper product, user/vault generations, and both Replay sequences;
trailing, truncated, zero-quantity, and unknown-action payloads have no
interpretation.

Descriptor and mint creation is pre-fund safe: system-owned zero-data targets
may already carry lamports, the creator funds only each exact rent shortfall,
and every pre-existing lamport stays locked in the permanent identity
tombstone. A hostile pre-funder gains no refund, fee, treasury, or protocol
authority. Vault Position/Replay rent remains a separately owned base-program
contract rather than a shadow field in the descriptor.

The descriptor contains no mutable supply shadow. Actual wrapper supply must
always come from the authenticated extension-free Token-2022 mint. Direct
burns create beneficiary-free surplus backing, never a fee or treasury claim.

The crate currently proposes descriptor coordinate `0x88/1`; the earlier
`0x7f/1` proposal was withdrawn after the global Dealer/Series/General block
was allocated through `0x87`. This is not a live
allocation until the central collision registry adopts the exact coordinate
alongside the SBF capability that consumes it; allocation alone will not make
the structured-claim family executable.
