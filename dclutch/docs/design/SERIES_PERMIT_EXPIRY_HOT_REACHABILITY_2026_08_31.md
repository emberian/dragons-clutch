# Series permit-expiry Hot reachability

Status: two unselected designs. This note records the state-transition order;
it does not recommend or implement either design.

## The current contradiction

The permissionless Core route cleans up a permit that was prefunded but never
allocated. Its honest account geometry is:

- future occurrence Market: System-owned, data length `0`;
- permit PDA: System-owned, data length `0`, funded to the rent floor for a
  `SeriesFoundingPermitV1`;
- lifecycle RentCredit: Rent-owned, `128` canonical bytes;
- Series root and Ticket replay: Trading-owned terminal replay;
- Template, occurrence, and Ticket: immutable finalized Registry records.

The nested permit binds `release_set`, the future Market, Ticket identity,
Trading program, generation, parent root, and RentCredit. Core derives the
permit PDA from `(release_set, future Market, Ticket)` and rejoins those fields
to the immutable records and Trading replay.

The common Hot outer authenticates its fixed Market before root authentication,
program-set selection, descriptor reads, or child execution. That Market must
instead be Core-owned, exactly `STATE_BYTES`, and decode as the canonical
`CoreState` for the envelope. The receiptless composer additionally requires
the permit's future Market to equal that fixed Hot Market. No honest account can
satisfy both owner/width predicates at once.

The real-ELF campaign in
`programs/dclutch-trading-sbf/program-test/tests/series_permit_expiry_hot_wall.rs`
pins this ordering: a canonical Series Expire request and exact nested permit
geometry reach the current Trading ELF, refuse as `TradingSbfError::Content`,
invoke no child program, and preserve every material account byte-for-byte.

## Design 1: authenticated pre-Market Hot mode

The account already present at expiry is the Trading-owned Series root, not a
Core Market. Its header, terminal Series tail, Ticket replay, selected release,
and immutable Registry records must be the authority while the future Market is
still vacant.

Required order:

1. Authenticate the top-level instruction and exact root-prestate digest.
2. Authenticate the activated Trading/Core roles without reading a Core Market.
3. Authenticate the Trading-owned Series root/header and selected release.
4. Select and authenticate exactly the Series Expire descriptor and request.
5. Admit Template, occurrence, Ticket, and ordered occurrence proof; join them
   to the root and Ticket terminal replay.
6. Require the fixed future Market to equal the admitted occurrence Market and
   to be System-owned with zero data. Derive any product/config fact needed by
   the child walk from the admitted immutable Series facts, because no
   `CoreState` exists yet.
7. Run the four Custody cleanup routes.
8. Invoke the final exact receiptless Core permit-expiry route. Require empty
   return data and no receipt dependents.
9. Commit the child transcript and the Series replay facts that this action
   owns.

Occurrence identity remains owned by the finalized occurrence/Ticket records
plus Trading root/Ticket replay. Permit identity remains owned by the canonical
nested permit and its Core-derived PDA, rejoined to those facts. The mode must
not accept an arbitrary vacant Market merely because it is vacant.

## Design 2: bind Hot to an authenticated ancestor

The account presented as Hot's fixed Market would be a Core-owned controller or
ancestor Market that is live before every occurrence. The future occurrence
Market remains a separate identity inside the permit and occurrence record.

Required order:

1. Authenticate the ancestor `CoreState` through the ordinary Hot path.
2. Authenticate the ancestor-bound Trading Series root, release, and artifacts.
3. Admit the exact Series Expire request, occurrence, Ticket, and proof under
   that root.
4. Bind the Hot parent coordinate to the ancestor/root and separately bind
   `permit.intent.market` to the admitted occurrence's future Market. Replacing
   the current false equality requires this two-coordinate proof; deleting the
   equality alone is not a design.
5. Run the Custody cleanup routes and then the receiptless Core expiry child.
6. Core repeats the immutable-record, root/Ticket replay, RentCredit, deadline,
   and permit-PDA checks for the future Market.
7. Require empty return data, then commit transcript and replay.

The ancestor `CoreState` and its Trading root own controller authority. The
finalized occurrence/Ticket records and Series replay own the future Market and
permit identity.

Current Series persisted state does not name a Core-owned ancestor that exists
for occurrence zero. A prior occurrence is therefore not an exhaustive anchor.
This design requires an explicit persisted/authenticated ancestor coordinate
and migration/ruling; it is not a composer-only change.
