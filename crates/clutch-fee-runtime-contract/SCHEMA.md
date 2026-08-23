# Fee runtime account-neutral schema V1

Status: **PURE INNER CODECS / NO SBF TAGS, PDA SEEDS, ACTIONS, OR LIVE
CAPABILITY** (2026-08-23).

Every body begins with an eight-byte discriminator, little-endian `u16`
version `1`, and zero `u16` flags. Decode requires the exact length, zero
padding, semantic reconstruction through the authoritative constructor, and
byte equality with the canonical re-encoding. Persisted derived words never
become an independent truth.

| Semantic account | Discriminator | Exact bytes | Semantic reconstruction | Mutation owner |
| --- | --- | ---: | --- | --- |
| Selected fee record | `DCFEESEL` | 336 | `SelectedCompositeFeeV1::select` from exact batch and revenue preimages | Immutable after selected-candidate creation |
| Owner fee carry | `DCFEECRY` | 128 | `OwnerFeeCarryV1::restore`, bound to selected fee record and owner | `OwnerFeeTransitionIntentV1`; terminal once |
| Payer allocation | `DCFEEPAY` | 2,680 | `allocate_payer_debit` from authenticated assessment and pre-transition signed envelopes | Same owner transition; temporary snapshot |
| Recipient allocation | `DCFEEREC` | 2,640 | `allocate_recipients` from selected policy and candidate-verified standing makers | `RecipientAllocationIntentV1`; candidate-wide temporary snapshot |
| Treasury ledger | `DCFEETRY` | 144 | `TreasuryLedgerV1::restore`, bound to selected treasury owner and ordinary Position | Begin/credit/settle transitions; owner-authorized ordinary withdrawal |

The payer and recipient snapshots deliberately bind all 64 fixed-capacity
identity/amount rows. They are not permanent revenue pots and must be retired
after their settlement obligations close. Their measured rent/compute cost is
still a capability-profile gate; these codecs do not claim the representation
is deployable or optimal.

## Typed identity joins

The generic `AccountIdV1<Kind>` has distinct marker types for fee record,
owner carry, payer allocation, recipient allocation, treasury ledger, and
owner settlement. Safe Rust cannot swap two kinds. Constructors require every
account identity to exist and independent mutation accounts to be distinct.

Ordered semantic joins:

1. `SelectFeeRecordIntentV1`: fee record, Realm, Market, counted Epoch, final
   `settlement_candidate_id`, batch-policy digest, revenue-policy digest,
   treasury Position. The candidate is never a settlement-witness
   representation.
2. `OwnerFeeTransitionIntentV1`: fee record, owner carry, payer allocation,
   owner settlement, final settlement candidate, revenue policy, owner.
   Carry is keyed only by `(fee record, owner)`, never by order or reservation.
3. `RecipientAllocationIntentV1`: fee record, recipient allocation, treasury
   ledger, final settlement candidate, revenue policy, treasury Position.
4. `TreasuryCreditIntentV1`: fee record, candidate-wide recipient allocation,
   treasury ledger, complete owner settlement book, final settlement candidate,
   revenue policy, treasury Position. Individual payer snapshots are joined by
   their owner transitions and must not substitute for the complete book.

The future SBF adapter must preserve these identities in its account ordering
or reconstruct an equivalent typed join before calling the pure transition.

## Owner-settlement projection

`project_terminal_owner_fee_v1` requires:

- a terminal `OwnerFeeCarryV1` and terminal-ceil assessment under the same fee
  record, owner, and relation-derived denominator;
- byte/account identities from `OwnerFeeTransitionIntentV1`;
- exact recomputation of the terminal `PayerAllocationV1` from pre-transition
  signed envelopes;
- cumulative post-transition envelope debits equal to the closed carry's
  `paid_atoms`; and
- `owner_debit_atoms(buy_price_units, price_scale, fee_atoms)` no greater than
  the authenticated aggregate buy cash reservation.

An intent without a buy cash reservation must bind a zero maximum and zero
debit. A seller-only participant therefore projects an explicit zero fee row.
`assemble_selected_owner_fee_book_v1` then requires one lexicographically
owner-ordered row per participant, canonical zero padding, and exact equality
with `CandidateSettlementTotalsV1.owner_count` and its candidate-selected
`u128` fee total. Recipient allocation refuses if that total cannot enter the
collateral account's `u64` amount domain.

Plane-C fee movement remains internal Position-ledger accounting: payer cash
debit to maker Position rebates and ordinary treasury Position credit. Hoard
collateral custody is unchanged. Eventual treasury withdrawal uses the normal
authenticated holder-withdrawal path; there is no fee-specific token CPI.
