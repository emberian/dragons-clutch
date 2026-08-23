# Fee runtime account-neutral schema V1

Status: **PURE INNER CODECS / OWNER TAG-VERSION RESERVED / NO LIVE
CAPABILITY** (2026-08-23).

Every body begins with an eight-byte discriminator and little-endian `u16`
version. The five original bodies use version `1` plus zero `u16` flags. The
owner finalization uses version `2`, followed by its terminal outcome and a
zero byte; candidate terminal bodies use version `1`, followed by their typed
outcome/count header. Decode requires exact length, canonical zero padding,
and semantic validation. Persisted derived words never become an independent
truth.

| Semantic account | Discriminator | Exact bytes | Semantic reconstruction | Mutation owner |
| --- | --- | ---: | --- | --- |
| Selected fee record | `DCFEESEL` | 336 | `SelectedCompositeFeeV1::select` from exact batch and revenue preimages | Immutable after selected-candidate creation |
| Owner fee carry | `DCFEECRY` | 128 | `OwnerFeeCarryV1::restore`, bound to selected fee record and owner | `OwnerFeeTransitionIntentV1`; one-way outer `0x83/1` prestate |
| Payer allocation | `DCFEEPAY` | 2,680 | `allocate_payer_debit` from authenticated assessment and pre-transition signed envelopes | Same owner transition; temporary snapshot |
| Recipient allocation | `DCFEEREC` | 2,640 | `allocate_recipients` from selected policy and candidate-verified standing makers | `RecipientAllocationIntentV1`; candidate-wide temporary snapshot |
| Treasury ledger | `DCFEETRY` | 144 | `TreasuryLedgerV1::restore`, bound to selected treasury owner and ordinary Position | Begin/credit/settle transitions; owner-authorized ordinary withdrawal |
| Owner fee finalization | `DCFEEFIN` / inner v2 | 496 | Settled path from the exact General owner-cash realization plan; abort path from the exact owner transition and signed envelopes | In-place outer `0x83/2` successor; immutable until candidate terminal consumes and closes it |
| Fee closure manifest | `DCFEECLS` | 224 | Canonical outcome/count/totals plus authenticated ordered closure-set data digest | Candidate-wide immutable terminal evidence |
| Fee-record terminal | `DCFEEEND` | 544 | Exact selected book, owner finalizations, recipient allocation, treasury state, value disposition, and closure manifest | Candidate-wide immutable terminal evidence |

The payer and recipient snapshots deliberately bind all 64 fixed-capacity
identity/amount rows. They are not permanent revenue pots and must be retired
after their settlement obligations close. Their measured rent/compute cost is
still a capability-profile gate; these codecs do not claim the representation
is deployable or optimal.

## In-place owner finalization

The owner carry keeps the same `(fee record, owner)` PDA and outer account tag
`0x83`. Its canonical outer forms are:

| State | Outer version | Inner bytes | Bump/flags | Exact outer bytes |
| --- | ---: | ---: | ---: | ---: |
| Mutable carry | 1 | 128 | 2 | 132 |
| Immutable finalization | 2 | 496 | 2 | 500 |

The v2 transition is not a reinterpretation of v1. It atomically authenticates
the complete deleted `0x84` payer-allocation data ID, final 288-byte owner-row
data ID, Position and cash-pot pre/post values, exact owner fee, and the data ID
of the canonical rent-ledger transition. No opaque duplicate finalization ID is
persisted: the one-way `0x83/1 -> 0x83/2` transition and final owner-row data ID
are the replay fact.

`OwnerFeeRentDispositionV2` requires the realloc top-up to be
`max(v2_rent_minimum - observed_balance, 0)` and requires its authenticated
top-up payer to equal the existing carry rent-refund owner. Carry donation is
preserved as donation; it may reduce present top-up need but never becomes
refundable principal. Payer-allocation principal and donation are separately
committed for atomic refund/neutral-sink disposition. Fee value, Hoard
principal, collateral, keeper funds, and liveness budgets cannot enter this
calculation.

The owner finalization is temporary immutable evidence. Candidate terminal
consumes it, closes it, and accounts for that close in `DCFEECLS`. The payer
snapshot was already closed atomically during owner finalization and has no
independent receipt account; its exact disposition is committed through the
persisted rent-transition data digest.

## Candidate-wide terminal receipts

Both settled and abort construction require the complete canonical owner book.
Every owner receipt must match its lexicographic book row and the sum of owner
fees must equal the book's candidate-selected `u128` fee total. Settled closure
also authenticates fee settlement, recipient allocation, and terminal treasury
ledger state. Abort closure requires the same selected recipient-allocation
snapshot but zero treasury credit, withdrawal, and availability; authorization
is released rather than collected.

`DCFEECLS` binds one immutable data digest for the canonical closure ordering:
selected fee record, recipient allocation, treasury ledger, then owner
finalizations in book order. Its account count is exactly `owner_count + 3`.
Rent principal refunds and hostile-prefund neutral-sink credits are conserved
separately. `DCFEEEND` binds that manifest plus the selected economic identities,
settled/released totals, recipient amounts, and external value-disposition and
terminal-authority receipt identities.

`project_pre_row_owner_fee_v2` consumes the owner-settlement-owned
`OwnerSettlementExpectationBasisV2` before action 24 creates a V2 row. It
reconstructs the exact payer allocation from signed Reservation envelopes and
returns only `(owner, fee_atoms)`. `basis.with_selected_fee(row)` then seals the
persisted expectation without a fee/row circular dependency. Buy-side presence
comes from the basis mask and `PresentConsiderationV2`, so a present zero-price
buy remains distinct from an absent buy. The terminal projection repeats this
exact join before the payer account can become Replay evidence or be deleted.
`GeneralOwnerFeeFinalizationProjectionV2` and
`GeneralFeeTerminalProjectionV1` expose the authenticated owner and candidate
facts without creating parallel semantic owners. `DealerFeeTerminalProjectionV1`
exposes only fee policy/candidate/outcome identity and returns zero for fee,
Hoard, or liveness funding availability.

The V3 action-24 path does not reread every Reservation. Snapshot creation is
the unique point that rederives all signed envelopes, proves cumulative
post-debit equals the closed carry's `paid_atoms`, and persists the canonical
`DCFEEPAY` outer. Later `project_pre_row_owner_fee_v3` reauthenticates those
unchanged bytes, exposes their complete-data ID, binds the same fee record,
candidate, policy, owner, denominator, terminal boundary, fresh `0x81/3` row
PDA, and exact owner-order count, then returns `(owner, fee_atoms)`. This is
allocation evidence only. Action 25 accumulates actual buy Reservation cash
handoffs in the V3 owner row; action 38 alone checks that cash covers exact
consideration plus the selected fee.

The delivery-complete V4 successor never lowers through V3.
`project_pre_row_owner_fee_v4` consumes the exact verifier-derived
`OwnerSettlementExpectationBasisV4`, the fresh V4 row PDA, and the same
authenticated persisted payer snapshot. It seals and returns the complete
`OwnerSettlementExpectationV4`; `expected_merge_delivery_count` remains
identity-bound in that expectation but is not a fee input. The projection still
proves allocation only—no Reservation cash, Hoard principal, future revenue, or
liveness funding. General independently authenticates the counted
SettlementRoot and derives the row PDA before consuming the private V4
projection. V4 book assembly uses `Option` rows so every participant is
explicit and inactive padding has no fabricated empty expectation.

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
