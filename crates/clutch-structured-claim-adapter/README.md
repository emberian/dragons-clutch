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

The descriptor contains no mutable supply shadow. Actual wrapper supply must
always come from the authenticated extension-free Token-2022 mint. Direct
burns create beneficiary-free surplus backing, never a fee or treasury claim.

The crate currently proposes descriptor coordinate `0x7f/1`. It is not a live
allocation until the central collision registry adopts that exact coordinate
alongside the SBF capability that consumes it; allocation alone will not make
the structured-claim family executable.
