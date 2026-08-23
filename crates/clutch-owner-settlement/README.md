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

The presence-explicit V2 semantic body remains exactly 288 bytes. Central
coordinates `0x81/2` select it; `0x81/1` remains withdrawn and no decoder
aliases the versions. V2 reuses one of V1's three trailing padding bytes as a
canonical bitmap: expected buy, expected sell, consumed buy, consumed sell.
The remaining two bytes must be zero. A set bit makes an exact zero value real;
an unset bit requires the corresponding integer field to be zero. Unknown bits,
consumed-without-expected sides, and nonzero padding are refused.

The fixed-capacity builder recomputes those rows from the complete authenticated
filled-order set plus one explicit fee row per participating owner. It refuses
duplicate order indices, missing/duplicate/foreign fee rows, seller cash
reservations, or any mismatch with the candidate's owner count, buy/sell
price-unit totals, fee atoms, rounding pot, and receipt-end count. Output rows
are lexicographically owner-sorted for canonical account creation and paging.

The account-neutral successor binds each future row to the ordered
`owner-settlement:v2`, Epoch, final-candidate, owner PDA preimage; creates it
with pre-fund-safe rent ownership; and stages each receipt-end accounting latch
without moving Eggs. Accounting uses a receipt-scoped `receipt_accounting_id`;
delivery uses a distinct `delivery_transition_id`, so neither replay latch can
stand in for the other. Terminal cash realization is buyer-first:
consideration enters a candidate-wide liability pot before seller credit can
leave it, while selected fees and exact rounding price units remain segregated.
The same 256-byte body carries a typed virtual-cash direction in formerly
reserved space. A split names terminal principal left after owner realization;
a merge names actual opening proceeds that must exist before seller
realization. Its exact closure is `buyer debit + merge proceeds = seller credit
+ split principal + whole-atom rounding`, so the amount cannot be silently
reinterpreted in the opposite direction.
The pot may become allocation-complete, but no API retires it or the rows: the
distinct General V2 FinalPot terminal/disposition authority is not yet owned,
and rounding or virtual-claim principal cannot be sent to the neutral donation
sink.

The candidate projection emits one presence-explicit receipt shape for all
three routes: direct, virtual split to a real buyer, and a real seller to a
virtual merge. Price and consideration are present even when their exact value
is zero, and consideration must still equal `quantity * price`. Receipt,
accounting, and delivery identities remain nonzero and distinct. The pure V2
projection derives one receipt-prestate data ID over its exact canonical
344-byte transcript, including both latch masks; Replay therefore binds the
semantic receipt prestate rather than a detached authorization flag. The pure V2
projection does not activate action 25: its eventual handler must atomically
advance the canonical Reservation accounting state, reserved-cash handoff,
receipt latch, and V2 owner row.
V2 cash realization is an explicitly non-authorizing structural projection. It
consumes the complete V2 row, canonical Position V3, and candidate cash pot;
the row's immutable selected-fee amount is its only fee input. It derives the
finalized row data ID from the exact terminal 288-byte body, stages the exact
Position and pot successors, and preserves buyer-first liquidity refusal. The
live action 38 composer must additionally rederive the fee runtime's private
typed terminal projection and bind the deleted payer-allocation prestate data
ID as GEN1 evidence before any atomic write. A caller-supplied authorization
boolean is not authority.

The successor direct-Egg contract closes the value-plane half only after
accounting and owner cash finalization have completed. Action 25 advances exact
price units, owner rows, Reservation accounting totals, and the accounting
latches without Egg or cash movement. Action 38 converts each complete owner
row once and atomically joins its Position and the candidate cash pot. Its
request ID is the adapter-authenticated data ID of the canonical finalized
288-byte row; the row does not copy another 32 bytes merely for replay.
Action 26 then authenticates both frozen order memberships, Position
generations and replay accounts, Reservations, the paired selected receipt,
and both terminal owner rows. Its one plan transfers the exact Egg quantity,
advances delivery totals, returns a completing portfolio seller's entire
unfilled vector, and sets independent buy/sell delivery latches. It cannot
repeat price accounting or rounding.

All owner cash and Egg transitions consume the canonical 480-byte
`PositionAccountV3`, not a settlement-local balance projection. The input
therefore retains the full MarketInstanceV2, Realm, collateral-policy,
collateral-release, purpose binding, controller, Replay, rent, generation, and
outstanding-Reservation identities while binding the separate General runtime
Market PDA. Every plan returns the exact canonical successor plus the
authenticated prestate semantic ID; the adapter must compare-and-write that
prestate and derive the successor semantic ID from the final body.
The row-data-ID binder re-runs the complete owner realization from its
authenticated row, Position, fee, and cash-pot prestates before it can return a
bound plan, so a caller-authored public plan cannot smuggle an alternate
Position or pot successor through the second-stage hashing seam.

Virtual split and merge use separate typed contracts and cannot pass through
the paired-direct API. Their default-deny authority records bind a checked
relation witness, selected candidate, exact amount or receipt, direction, and
both replay identities. There is no public inventory-only plan. Action 36
atomically moves already-finalized split principal from the owner cash pot into
FinalPot, creates only the complete-set inventory needed by the selected real
buyer end, updates Hoard and aggregate supply, and delivers that Egg. Action 37
atomically accepts an already-accounted real seller Egg, burns the canonical
available complete-set floor, and, only when the exact merge budget completes,
turns FinalPot merge principal into the opening owner cash pot. The seller row
is AccountingComplete/state 0 before action 37 because its later action-38
credit depends on those proceeds; requiring state 1 there would be circular.

FinalPot has one canonical 328-byte inner codec containing its one-to-one
selected virtual budget, cash principal, native claims, and mutable inventory
cursors. There is no separately addressed budget account, rent owner,
lifetime, or close authority. Account address, writability, PDA ownership, and
selected-verifier authentication remain outer adapter facts rather than a
second persisted truth. Direct candidates canonically encode the budget as
`None` with zero witness, amount, and cursors.

The virtual cash field is principal attributed inside pooled Realm collateral.
It is never classified as a fee, donation, revenue, rent, or liveness funding.
Terminal rounding and FinalPot disposition remain separately owned integration
work.

This crate contains no Solana SDK, account memory, hashing implementation,
dynamic allocation, fee policy selection, or persisted DTO. It does not make
General V1 accept shapes that its current per-order realization cannot settle;
it is the successor General V2 semantic owner.
