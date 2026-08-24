# General frequent-batch contract

`dclutch-general-contract` is the pure semantic owner of the optional General
venue. It is not the Market core, a Solana adapter, an orderbook index, or a
claim mint. The crate is `no_std`, `no_alloc`, safe Rust, fixed-capacity, and
uses checked integer arithmetic throughout.

## Authority

One `GeneralConfigV1` binds all venue activity to:

- one exact Market identity commitment and occurrence generation;
- the Market's exact ClaimBasis content identity and finite width;
- one immutable General capability release selected by the capability
  manifest;
- one Realm-selected settlement asset; and
- one liftable capacity-profile identity.

The adapter must authenticate those content identities against the Market and
capability manifest. There is no second feature bitmap, admin switch, or static
client authority.

## Frequent-batch lifecycle

The capability root is `Active -> Quiescing -> Terminal -> Retired`. Quiescing
immediately stops new batches but lets owned batches converge. Terminal requires
zero open batches. Retirement additionally requires the adapter to discharge
all segregated funding and rent ownership.

Each batch is:

1. `Collecting`: adapter-authenticated signed orders can be opened or cancelled.
2. `Selecting`: collection is locked; anyone can submit and page-verify a
   candidate.
3. `Settling`: the selection deadline freezes the deterministic best valid
   submitted candidate, but no economic mutation has begun.
4. `Applying`: exact complete-set collateral conversion has committed and
   receipt pages must converge. This phase cannot expire.
5. `Quiescent -> Retired`: no economic mutation remains, then child/rent state
   is discharged.

An unstarted `Settling` batch may expire without consuming orders. An `Applying`
batch cannot be abandoned after partial work. Prepaid liveness capital funds
permissionless completion. This distinction is deliberate: paginated SVM work
cannot honestly claim whole-batch transaction atomicity.

## Orders and replay

An order is one signed coefficient vector in the canonical ClaimBasis order.
One scalar `fill_lots` applies to every coefficient, so a solver cannot cherry
pick legs of a portfolio. The order binds Market, ClaimBasis, generation, batch,
owner, nonce, expiry, lot cap, and one exact upper quote-debit limit.

The adapter reserves a unique `(owner, nonce, order_id)` replay record and locks
the order's worst-case required quote/outcome custody. `OrderStateV1` tracks
remaining lots and admits only `Open -> Cancelled` before collection close or
`Open -> Open/Consumed` through the winning settlement. Orders must remain valid
through the entire pre-application settlement window. Once collection closes,
cancellation is refused.

Candidates are checked against immutable order-state snapshots without
consuming them. Only the selected candidate's applying pages consume replay
state. Pages and the settlement cursor require strictly increasing order IDs,
which refuses duplicate execution inside a candidate.

## Exact prices, objective, and conservation

Prices are nonnegative `u64` simplex coordinates whose checked sum equals the
configured positive scale exactly. Unused coordinates are canonical zeroes.
There are no floats.

For portfolio coefficients `q[i]`, prices `p[i]`, and scalar lots `L`, the exact
quote-debit numerator is:

```
L * sum(q[i] * p[i])
```

The candidate score is the checked sum of each executed order's exact preference
surplus:

```
L * (order.max_quote_debit_per_lot_numerator - sum(q[i] * p[i]))
```

Higher verified score wins. An equal score selects the lexicographically smaller
candidate content identity. The contract intentionally calls this the **best
valid submitted candidate**. It makes no optimality claim. A future release can
add a checked optimality certificate without changing V1's vocabulary.

The adapter's order-admission/custody policy must price account and computation
use so costless wash-order flooding cannot consume the finite batch profile.
Those charges are not General liveness capitalization and are not sourced from
Hoard. V1 candidate ranking does not pretend to be Sybil-proof without that
economic admission boundary.

At verification completion, aggregate outcome inventory must be exactly
`[k, k, ..., k]`. This is the only inventory the venue may create or destroy:
`k > 0` is a virtual complete-set split and `k < 0` is a complete-set merge.
Because simplex prices sum to the scale, aggregate trader quote debit must equal
`k * scale`. The contract checks both laws independently.

At `Applying` entry, the adapter reauthenticates locked order custody and moves
exactly `k` collateral atoms between General settlement custody and Hoard while
the kernel changes Hoard principal and equal per-outcome liabilities by the same
amount. Hoard principal must equal those liabilities before and after. Hoard is
never work funding, fees, bounty, rent, reserve, or treasury capital.

### The one rounding boundary

Individual token transfers must be integral even when an individual portfolio's
quote numerator is not divisible by the price scale. V1 has exactly one named
rounding boundary: **canonical-prefix carry at settlement receipt emission**.

Starting with carry zero, for each execution in strict order-ID order:

```
combined = prior_carry + exact_signed_quote_delta_numerator
receipt.quote_delta_atoms = floor_euclid(combined / price_scale)
next_carry = rem_euclid(combined, price_scale)
```

The final carry must be zero. Aggregate quote flow is therefore exact; rounding
only assigns indivisible atoms deterministically among ordered receipts. There
is no dust account or hidden protocol remainder.

## Permissionless paginated verification

Anyone may create a candidate and advance its bounded cursor. Each page commits:

- its zero-based page index;
- the exact predecessor transcript identity;
- a nonzero successor transcript identity; and
- one canonical leading run of executions, with unused envelope entries absent.

The kernel copies the cursor, validates the whole page, and commits only after
every execution succeeds. A bad later execution cannot leave partial verifier
progress. The cursor commits page/execution counts, last order, transcript,
aggregate coefficients, quote debit, and score.

The kernel deliberately implements no hash. The SVM adapter must hash the
canonical candidate/page preimages under a pinned transcript release and prove
the supplied successor. This is a named runtime trust boundary, not a second
authority.

## Funding

`GeneralFundingV1` contains three immutable, prepaid, independently conserved
compartments: liveness, work, and bounty. A debit consumes present principal
from exactly one compartment. Remaining plus spent plus refunded must always
equal the founding quote for that same compartment. Terminal refund cannot
borrow across compartments.

No API accepts Hoard principal or prospective fee revenue as funding.

## Bounds and lifting

- `MAX_OUTCOMES_V1 = 16` is a **provisional program-profile bound**. It is not a
  mathematical claim-family limit. Lift it with a new capability release and
  capacity-profile identity, preserving existing Market and receipt meanings.
- `MAX_EXECUTIONS_PER_PAGE_V1 = 4` is a **provisional program-profile bound**.
  It is intended to be measured against adapter account frames and compute
  units. Lift it in a new release without changing transcript order or the
  objective.
- `max_orders_per_candidate` and `max_pages_per_candidate` are **immutable
  Market profile bounds**. Construction proves orders fit the page envelope.
- `u64` raw token quantities and `i64` signed receipt deltas are **adapter word
  bounds**. Larger domains require a new token/capability release, not unchecked
  casts.
- The simplex sum and complete-set vector laws are **mathematical bounds**, not
  liftable capacity restrictions.

## Runtime seams still owned by integration

This crate does not claim an end-to-end venue until a small SVM adapter and
operator implement and test all of the following:

1. Authenticate config, Market identity, ClaimBasis, capability release, and
   transcript hashes from canonical bytes.
2. Verify owner signatures and unique replay PDA derivations.
3. Lock worst-case order custody before an order is admitted; keep it immutable
   after collection close.
4. Atomically couple `Applying` entry to exact complete-set custody/Hoard token
   movements, then atomically couple each page to its returned replay states,
   receipts, token movements, cursor, and funded work debit.
5. Make applying pages permissionless and non-expiring, with liveness payments
   drawn only from the segregated compartment.
6. Persist canonical account encodings for roots, funding, candidate cursors,
   and settlement cursors. Config, signed orders, and settlement receipts already
   expose hostile exact-width codecs here.
7. Measure account rent, transaction account counts, SBF stack, and compute units
   before replacing any provisional bound.

The static operator or index may discover and construct these calls, but it is
an untrusted projection of the onchain records above.
