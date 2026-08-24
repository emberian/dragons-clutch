# Series V1 contract

Status: SDK-free semantic contract. This crate does not claim an SBF account
frame, Market deployment, hashing implementation, keeper, or operator.

## Purpose and authority

Series is a standalone finite factory for recurring Markets. It is not a
Market capability, Market child, source oracle, Product compiler, or mutable
Market registry. `SeriesRecipeV1` is immutable content and owns only recurrence
and derivation selection. `SeriesRootV1` owns only gap-free recurrence progress
and conservation of a finite, presently funded capitalization schedule.

The recipe commits:

- Realm, Terms, categorical ClaimBasis, and CapacityProfile identities;
- Product compiler release;
- occurrence and source schedule identities;
- the capability-template identity;
- occurrence, source, capability, Market, and capitalization derivation
  releases;
- first occurrence time, positive cadence, finite occurrence count, first
  Market generation, and exact categorical width.

Every content identity is nonzero. The composing adapter authenticates content
hashes and executes the selected derivation releases. This crate deliberately
does not hash or accept caller-authored derived identities as authority.

The root persists the immutable refund authority as the one semantic owner of
the beneficiary choice. The Rent contract's one-credit-per-authority PDA rule
then determines the RentCredit address; creation, ticket consumption, and close
must authenticate that derivation and the credit record. Series does not persist
a second caller-selectable RentCredit address.

## Present capitalization

`CapitalizationAggregateV1` commits the exact sum of a finite schedule.
`OccurrenceCapitalizationV1` is one schedule item and separates Market-founding
principal from ticket-account principal. At release, that allocation must meet
the authenticated current rent minimum; its complete committed amount moves to
the ticket, so any overprovisioning is later RentCredit rather than a hidden
fee. The adapter proves each item is the canonical item selected by the
aggregate and capitalization derivation release.

At creation, a separate Series escrow must hold the aggregate's entire
principal now. Root state conserves:

```text
remaining allocations + released allocations = total allocations
remaining principal   + released principal   = initial principal
```

Instantiation requires the observed escrow balance to cover the authenticated
current escrow rent minimum plus root `remaining_principal`. It transfers only
the exact selected allocation. Unsolicited donations remain outside Series
principal and therefore cannot stall or capitalize an occurrence; they are
credited to RentCredit at close. Hoard principal and future fees are not
representable. Ticket-account principal is separate from Market-founding
principal.

## Gap-free instantiation and tickets

Anyone may instantiate the exact next occurrence. The request must repeat the
root's next index and scheduled time, and the authenticated chain clock must
have reached that time. The authenticated derivation record must
bind the same recipe, index, time, generation, occurrence, source, resolution,
capability-manifest, Market-identity, and capitalization identities. The
capitalization item must bind the same recipe, schedule, and index.

Success atomically:

1. subtracts one exact capitalization item from escrow/root remaining state;
2. advances index, time, and generation without a gap;
3. increments the exact outstanding-ticket count; and
4. creates one `OccurrenceTicketV1` at the canonical Series/index PDA.

The last item moves the root to `Exhausted`. A stale request, skipped index,
wrong time, wrong derivation, wrong capitalization, underfunded escrow, or
arithmetic failure leaves root state unchanged.

The ticket is intentionally a compact one-use semantic bridge. A future Found
adapter must authenticate the same immutable recipe, derivation, and
capitalization records; derive and Found the exact committed Market; distribute
exactly the ticket's Market principal; credit the ticket's rent plus unsolicited
lamport donations to the immutable beneficiary's permanent RentCredit; and
decrement the Series ticket count atomically. Until the complete physical frame
is measured, this crate makes no claim that those actions fit one Solana
transaction.

PDA derivations are exact and domain-separated. Root uses ordered seeds
`[root-domain, recipe-id, aggregate-id, refund-authority, bump]`; escrow uses
`[escrow-domain, root-address, bump]`; ticket uses
`[ticket-domain, root-address, index-le-u64, bump]`. Fixed-width concatenated
preimages are exposed only for stable fixtures; the adapter must preserve these
seed boundaries when calling the chain PDA primitive.

## Exhaustion and close

Series V1 has no cancellation, skip, mutable cadence, mutable recipe, or
unfunded extension. It closes only after every finite allocation was released,
every ticket was consumed, and root-owned escrow principal is zero. Closing
credits the complete observed Root and escrow account balances to the immutable
RentCredit. Those balances are account rent plus possible unsolicited
donations, never Hoard or unspent Series capitalization.

## Bounds and lifting

- `2..=16` outcomes is the current **provisional categorical profile**. Lift it
  with a new Series schema selecting a wider reviewed Market/Product profile.
- The 32-byte PDA seed-component ceiling is **chain-derived**. All V1 domains
  are tested below it.
- `u64` counts, generations, lamports, and cadence are **chain-derived
  representation bounds**. Every operation is checked. A wider representation
  requires a new schema.
- `i64` occurrence time is a **chain-derived clock representation**. Recipe
  construction proves the final scheduled time fits.
- Series has no loop over occurrence count, so V1 adds no arbitrary product
  maximum. Finiteness and checked last-time/last-generation arithmetic are the
  only recurrence bounds.
- Whether ticket consumption and Market Found fit atomically is a **measurement
  boundary**. The SBF adapter must measure packet bytes, account locks, stack,
  compute units, rollback, and rent before selecting its physical composition.
