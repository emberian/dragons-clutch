# Series V1 contract

Status: SDK-free semantic contract. This crate does not claim Market
deployment, a keeper, or an operator. It owns one safe `no_std` SHA-256
derivation release set; the SBF adapter owns account authentication and
execution.

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

Every content identity is nonzero. V1 admits exactly one pinned Product compiler
and four pinned derivation release IDs: a fixed occurrence-artifact/Product
derivation, a shared immutable source policy, a shared immutable capability
manifest, and canonical Market-identity derivation. Any mixed or unknown
release set refuses. The contract recomputes the complete
`DerivedOccurrenceV1` and its content identity; caller-authored derived fields
are never authority.

The occurrence release hashes a fixed 104-byte artifact containing recipe,
occurrence-schedule, index, time, and generation. It then hashes canonical
Product `OccurrenceV1` and `InstanceV1` preimages. The Market release hashes
the canonical 168-byte Market identity. The exact occurrence-capitalization
record is separately hashed, so changing funding changes only the bound
capitalization identity, not Product or Market identity. This V1 source release
uses one immutable `source_schedule_id` as the shared source specification,
window, statistic, and resolution-policy identity; capability similarly uses
the immutable `capability_template_id` directly. A future per-occurrence source
compiler is a new pinned release rather than caller discretion.

Before accepting Create, and again before instantiation, the SBF release
authenticates the recipe-selected CapacityProfile and proves that the fixed
104-byte, one-page occurrence artifact and categorical outcome width fit it.
Create therefore cannot strand prepaid principal behind an intrinsically
inadmissible recipe. The fixed Product compiler release ID is checked by the
pure recipe decoder rather than carried as an unused advisory field.

The root persists the immutable refund authority as the one semantic owner of
the beneficiary choice. The Rent contract's one-credit-per-authority PDA rule
then determines the RentCredit address; creation, ticket consumption, and close
must authenticate that derivation and the credit record. Series does not persist
a second caller-selectable RentCredit address.

## Present capitalization

`CapitalizationAggregateV1` commits the exact sum of a finite schedule and the
content identity of its first item. Every `OccurrenceCapitalizationV1` commits
the exact next item identity, or canonical `None` only on the final item. The
root owns the one exact next identity while traversing this content-addressed
forward chain. This makes varied per-occurrence allocations possible without a
fixed maximum, dynamic state, Merkle-proof wire, or caller choice. The final
item must equal all remaining principal, so occurrence-count exhaustion can
never strand an unreleaseable remainder.

Each item separates Market-founding principal from ticket-account principal.
At release, that allocation must meet the authenticated current rent minimum;
its complete committed amount moves to the ticket, so any overprovisioning is
later RentCredit rather than a hidden fee. The adapter proves each item is the
canonical next item selected transitively by the aggregate head.

At creation, a separate Series escrow must hold the aggregate's entire
principal now, and a 48-byte permanent replay guard must be funded at its
authenticated current rent minimum. Root state conserves:

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
4. allocates one `OccurrenceTicketV1` at the canonical Series/index PDA.

Like Series creation, ticket vacancy is System ownership, empty data, and a
nonexecutable flag rather than a zero balance. Harmless prefunding reduces the
exact escrow top-up and is preserved in the ticket, so dusting a deterministic
ticket PDA cannot veto liveness. Any surplus eventually follows the ticket's
normal RentCredit close path.

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

`FoundCompositionObligationsV1` is the exact transient output of successful
ticket validation. It carries Realm, Terms, ClaimBasis, CapacityProfile,
compiler, occurrence, Product, source, statistic, resolution, manifest, Market,
generation, time, and exact Market-principal facts. It is not caller wire. The
adapter must obtain an accepted Found transition for that complete bundle, then
commit Found, the ticket's complete deletion, the root ticket-count decrement,
and RentCredit delta in one rollback domain. No individual sub-transition may
land when any other sub-transition refuses.

PDA derivations are exact and domain-separated. Root uses ordered seeds
`[root-domain, recipe-id, aggregate-id, refund-authority, bump]`; escrow uses
`[escrow-domain, root-address, bump]`; ticket uses
`[ticket-domain, root-address, index-le-u64, bump]`; permanent replay guard uses
`[guard-domain, root-address, bump]`. Fixed-width concatenated preimages are
exposed only for stable fixtures; the adapter must preserve these seed
boundaries when calling the chain PDA primitive.

## Cross-lifecycle replay guard

Deleting every record that remembers a Series would make recreation at the same
root PDA possible; rejecting that by proving arbitrary historical Market
nonexistence is neither bounded nor sound. Create therefore requires a vacant
canonical `SeriesReplayGuardV1` and creates it atomically with Root and escrow.
The guard binds the root address and bump; the canonical root PDA already
commits recipe, aggregate, and beneficiary, so the guard does not duplicate
those facts. It is permanent and has no close instruction, so a later Create at
the same root necessarily refuses even after Root and escrow close.

Vacancy means authenticated System ownership, empty data, and nonexecutable
state—not zero lamports. Pre-funded deterministic PDAs are allocated/assigned
and only their exact target deficits are charged to the payer. Consequently an
outsider cannot block Create by dusting Root, escrow, or guard; any excess stays
an unsolicited donation outside root-owned principal.

This is intentional permanent replay state, not recoverable Series rent. During
exhausted close, the adapter authenticates current guard rent. The complete Root
and escrow balances plus any guard balance above that rent floor are credited to
RentCredit; exactly the current guard rent minimum remains. A guard below that
floor refuses close rather than deleting or silently subsidizing replay safety.

## Exhaustion and close

Series V1 has no cancellation, skip, mutable cadence, mutable recipe, or
unfunded extension. It closes only after every finite allocation was released,
every ticket was consumed, and root-owned escrow principal is zero. Closing
credits the complete observed Root and escrow account balances and replay-guard
surplus to the immutable RentCredit. Those balances are account rent plus
possible unsolicited donations, never Hoard or unspent Series capitalization.

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
