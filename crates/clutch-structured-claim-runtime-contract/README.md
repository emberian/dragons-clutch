# `clutch-structured-claim-runtime-contract`

This crate is the allocation-free runtime contract between the pure
`clutch-structured-claim` economics and the small SBF/Token-2022 adapters. It
keeps the historical 384-byte descriptor v1 decode-only, freezes the sole
future 449-byte descriptor v2, and owns the exact 624-byte Series-scoped
Structured root plus fixed-depth wrapper-recipe membership. The descriptor
reconstructs native-claim and deployment/root/recipe-bound product identity
from authenticated Product, basis, and deployment facts. Custody stages the
required atomic Position cash/native-Egg transfer without touching global
supply or Hoard collateral.

The first inline recipe-set profile carries at most sixteen ordered leaves so
its fixed proof fits the existing instruction packet. That is a wire-profile
capacity, not an economic limit: a future paged set owner can take a fresh
version while retaining the same recipe identities.

The root's terminal receipt is derived from its complete final body with only
the recursive receipt field omitted. It therefore commits the exact Product
link lineage, immutable Series/root binding, admission and terminal
transcripts, exhaustive counts, rent principal, donation residue, and bump;
an arbitrary caller receipt cannot become Product terminal evidence.

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

Descriptor coordinate `0x88/1` is permanently historical and decode-only;
`0x88/2` is the sole future descriptor. The mutable Structured market root is
`0xaf/1`; `0xad/1` belongs to Product SeriesMarketLink and `0xae/1` belongs to
Dealer CoveredDealerSelection. Central allocation alone does not make any
Structured route executable.
