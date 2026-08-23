# Series funding state V1

`funding_state` is the allocation-free mutable successor to the immutable V5
Series and V1 quote artifacts. It is deliberately not an SBF account adapter.

The 324-byte state has one lifecycle cursor and five accounting compartments:

1. market core;
2. evidence-recovery reserve;
3. source/window/evaluator work;
4. liquidity facility; and
5. canonical wrapper set.

`SeriesFundingQuoteV1` is the sole owner of each per-occurrence amount. The
mutable state stores only each compartment's remaining refundable payer
principal, separately owned donation residue, and number of absent-component
allocations consumed by creation. It records consumption even when a quoted
amount is exactly zero, so presence is never inferred from an amount. It does
not persist another copy of the quote, spent principal, created count, or
active/closed phase. Those values are derived and checked against the quote.

Activation recomputes the entire immutable Product/Series/Attachment/Quote/
FundingTerms/Genesis/Realm projection join and requires exact
`instance_count * quote_component` principal independently for all five
components. Excess value must be named as donation residue; it cannot cure a
principal shortfall. FundingTerms V2 immutably binds the refund identities,
neutral sink, Realm collateral mint, and token program.

Only the exact next ordinal can advance. During its creation interval, the
adapter supplies an authenticated Clock bucket and an authenticated V2
component-presence projection for the canonical SourcePlane V3 occurrence.
The core derives the exact debit itself. Exact-existing components spend zero;
market core and recovery remain paired. At or after start, lapse advances the
same cursor without spending. Terminal projection keeps refundable principal
and donation residue distinct.

`AuthenticatedSeriesFundingAuthorityV1` defaults every method to refusal. A
trait implementation is still an unverified adapter boundary, not proof: a live
SBF implementation must check exact account/PDA identity, owner, canonical
body, rent, Clock mapping, authenticated central-registry provenance, immutable
Realm/Profile collateral, token program and mint, segregated custody balances,
and atomic transfer/rollback behavior.

No instruction tag, PDA seed, vault layout, or CPI route is allocated here.
Those details are not frozen by the current design, and inventing them inside
the pure semantic core would create a second protocol contract. The existing
non-production SBF profile remains an immutable artifact catalog only.
