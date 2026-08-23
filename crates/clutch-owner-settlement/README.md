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

The fixed-capacity builder recomputes those rows from the complete authenticated
filled-order set plus one explicit fee row per participating owner. It refuses
duplicate order indices, missing/duplicate/foreign fee rows, seller cash
reservations, or any mismatch with the candidate's owner count, buy/sell
price-unit totals, fee atoms, rounding pot, and receipt-end count. Output rows
are lexicographically owner-sorted for canonical account creation and paging.

The account-neutral adapter contract binds each row to the ordered
`owner-settlement:v1`, Epoch, final-candidate, owner PDA preimage; creates it
with pre-fund-safe rent ownership; and stages each receipt-end accounting latch
for an atomic join with the complete Egg/reservation transition. Terminal cash
realization is buyer-first:
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

The successor direct-Egg contract closes the value-plane half of receipt
accounting without reintroducing per-slice cash rounding. It authenticates both
frozen order memberships, both Position generations and replay accounts, both
ENTITLED Reservations, the paired selected receipt, and both owner rows. One
pure plan transfers the exact Egg quantity, advances quantity and exact
price-unit ledgers, returns a completing portfolio seller's entire unfilled
Egg vector, hands a completing buyer Reservation's cash-envelope ownership to
the frozen owner row, and sets the independently named buy/sell receipt-end
latches. Position cash and replay sequences are preserved; their eventual cash
poststates remain owned by terminal owner-level realization.

No General action number is assigned to this contract. A future SBF adapter
must authenticate the opaque complete-transition identity, bind it by equality
to its payload, and write both Positions, both Reservations, both owner rows,
and the receipt latch atomically.

Virtual split and merge use separate typed contracts and cannot pass through
the paired-direct API. Their default-deny authority records bind a checked
relation witness, selected candidate, exact amount or receipt, direction, and
complete transition identity. Inventory split consumes FinalPot cash principal,
adds the same collateral principal to the Hoard, and adds one internal claim of
every active outcome to both the FinalPot and aggregate supply. Inventory merge
is the exact inverse. Separately, a virtual-split receipt moves one selected Egg
from FinalPot inventory to its sole real buyer; a virtual-merge receipt moves
one selected Egg from its sole real seller into FinalPot inventory. Only the
real end advances a Reservation, owner row, and independent receipt latch.

The virtual cash field is principal attributed inside pooled Realm collateral.
It is never classified as a fee, donation, revenue, rent, or liveness funding.
Terminal FinalPot disposition and its exact join to owner cash realization
remain separately owned integration work.

This crate contains no Solana SDK, account memory, hashing implementation,
dynamic allocation, fee policy selection, or persisted DTO. It does not make
General V1 accept shapes that its current per-order realization cannot settle;
it is the successor General V2 semantic owner.
