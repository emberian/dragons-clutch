# `clutch-owner-settlement`

This allocation-free contract removes the accidental equality between “one
filled order” and “one participating owner.” General V2 freezes one row per
owner with exact aggregate buy/sell price units, order masks, slice count,
reserved cash, and any already-selected fee atoms. Receipt consumption adds
exact price units without rounding. Only terminal owner finalization converts
the aggregate payer side with `ceil` and the aggregate payee side with `floor`,
at the one named `TerminalOwnerFloor` boundary.

The result supports owners with several filled single or portfolio orders and
inexact per-order values without silently introducing extra rounding events.
Egg movements remain receipt-exact. An SBF adapter must authenticate every
receipt/order membership, reconcile reservation funding and Position bytes,
and consume every receipt/root count before closing the epoch.

The semantic body is exactly 288 bytes. Its outer General V2 account
tag/version remains centrally owned and unallocated in this isolated lane, so
the codec cannot accidentally make the runtime capability live by itself.
The shared pure disposition projector applies the same terminal arithmetic as
accumulator finalization for client construction and indexing, but cannot stand
in for runtime receipt authentication or the accumulator's completion state.

The fixed-capacity builder recomputes those rows from the complete authenticated
filled-order set plus one explicit fee row per participating owner. It refuses
duplicate order indices, missing/duplicate/foreign fee rows, seller cash
reservations, or any mismatch with the candidate's owner count, buy/sell
price-unit totals, fee atoms, rounding pot, and receipt-end count. Output rows
are lexicographically owner-sorted for canonical account creation and paging.

This crate contains no Solana SDK, account memory, hashing implementation,
dynamic allocation, fee policy selection, or persisted DTO. It does not make
General V1 accept shapes that its current per-order realization cannot settle;
it is the successor General V2 semantic owner.
