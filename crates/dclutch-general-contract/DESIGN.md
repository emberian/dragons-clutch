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
- one liftable capacity-profile identity.

The adapter must authenticate those content identities against the Market and
capability manifest. The General config deliberately does not repeat a
settlement-asset identity: Market -> Realm -> Mint/token release is the sole
authority for that fact. There is no second feature bitmap, admin switch, or
static client authority.

The recognized V1 release is closed by four domain-separated SHA-256 preimages
and their exported content identities:

- `dclutch/general/capability-kind/v1`;
- `dclutch/general/frequent-batch-release/v1`;
- `dclutch/general/child-schema/v1`; and
- `dclutch/general/child-derivation/v1`.

`validate_general_capability_entry_v1` requires those exact kind, release,
schema, and derivation identities plus the exact config and capacity-profile
identities. The contract also exports distinct, at-most-32-byte PDA domains for
config, funding, root, batch, order replay, order custody, and quote escrow.
Their hashes and domain separation have derivation tests; an adapter verifies
the resulting account addresses without inventing release constants.

## Canonical mutable-state records

The contract is the sole codec and invariant owner for its persisted records.
`GeneralConfigV1` is exactly 200 bytes, `GeneralRootV1` is exactly 72 bytes,
`GeneralFundingV1` is exactly 144 bytes, `BatchRootV1` is exactly 136 bytes, and
`OrderStateV1` is exactly 96 bytes. Exact-N `GeneralOrderCustodyV1<N>` is
`192 + 8N` bytes. Each state record has a distinct eight-byte type magic plus
the V1 schema and artifact-profile tags, and decoders reject short, trailing,
reserved, unknown-tag, zero-identity, arithmetically invalid, and unreachable
state encodings.

The records encode every field read or changed by a transition. Optional
candidate identities use one canonical discriminator and require an all-zero
payload when absent. Batch deadlines are strictly increasing, batch winner
shape agrees with candidate count and phase, root child counts cannot exceed
reserved sequences or survive terminalization, and order replay phase agrees
with remaining lots. Replay authentication additionally binds the persisted
order identity, exact nonzero signing key, nonce, and remaining lots to the
immutable signed order and its original lot ceiling. Custody binds that order,
Market, generation, signing key, rent beneficiary, and quote escrow, and owns
only remaining quote and native-claim principal; replay availability remains
solely in `OrderStateV1`. Funding persists committed, remaining, spent, and
refunded amounts independently for all three compartments; checked conservation
and the one atomic terminal-refund shape are revalidated on every decode and
encode. An SVM adapter must use these codecs directly rather than define a
parallel account DTO.

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
the exact nonzero Ed25519/SVM `OwnerKeyV1`, nonce, expiry, lot cap, and one exact
upper quote-debit limit. `OwnerKeyV1` has the same physical width as a content
identity but is not one: the adapter compares its bytes directly with the
authenticated signer key.

Admission atomically creates a unique `(owner, nonce, order_id)` replay record
and exact-N custody. Worst-case quote reserve is
`ceil(max(0, max_quote_debit_per_lot_numerator) * max_lots / price_scale)`;
native reserve for outcome `i` is
`max(0, -coefficient[i]) * max_lots`. Both use checked exact arithmetic and
there is no maximum-width padding. The adapter must couple the returned quote
escrow and Position debits to persistence in the same transaction.

`OrderStateV1` tracks remaining lots and admits only `Open -> Cancelled` before
collection close, `Open -> Open/Consumed` through winning receipts, or
`Open -> Released` after its batch is quiescent. Receipt consumption binds the
order, owner, nonce, Market generation, batch, exact width, remaining lots, and
per-outcome coefficient products before it atomically advances replay and
custody. Cancellation returns every reserve only with the cancellation state
change. Batch closure returns all residual quote/claims for either a fully
consumed order or an unfilled remainder, making that remainder permanently
unavailable. Orders must remain valid through the entire pre-application
settlement window. Once collection closes, cancellation is refused.

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
compartments: liveness, work, and bounty. Its only capability activation
constructor authenticates the exact manifest funding state and closed General
release, invokes the capability ledger's activation transition, and maps the
immutable quote exactly: service -> General liveness, work -> work, and bounty
-> bounty. Provider and liquidity must both be zero. Rent and creation remain
the capability activation outputs. No caller supplies compartment amounts, and
the returned plan releases all quote principal from the generic ledger so one
principal cannot remain owned twice.

A General debit consumes present principal from exactly one compartment.
Remaining plus spent plus refunded must always equal the founding quote for
that same compartment. Terminal refund cannot borrow across compartments.

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
2. Verify `OwnerKeyV1` signatures and exact config/funding/root/batch/replay/
   custody/escrow PDA derivations.
3. Atomically execute the contract-returned admission reserve against the
   owner's Position and quote escrow, then couple every receipt or release to
   the corresponding token/Position movement.
4. Atomically couple `Applying` entry to exact complete-set custody/Hoard token
   movements, then atomically couple each page to its returned replay states,
   receipts, token movements, cursor, and funded work debit.
5. Make applying pages permissionless and non-expiring, with liveness payments
   drawn only from the segregated compartment.
6. Add canonical contract-owned account encodings for candidate and settlement
   cursors. Config, roots, funding, batch roots, order replay state, exact-N
   custody, signed orders, and settlement receipts already expose hostile
   exact-width codecs here.
7. Measure account rent, transaction account counts, SBF stack, and compute units
   before replacing any provisional bound.

The static operator or index may discover and construct these calls, but it is
an untrusted projection of the onchain records above.

## Exact physical vector geometry

Every persisted outcome-bearing semantic record is const-generic over its
selected ClaimBasis width `N`: `PortfolioOrderV1<N>`,
`GeneralOrderCustodyV1<N>`, candidate state and cursor, settlement cursor,
`SettlementReceiptV1<N>`, and the page result that carries receipts. Their
constructors require the recorded outcome count to equal `N`. Orders, custody,
and receipts expose checked exact `encoded_len`, decode only that length, and
encode only into that length. Consequently an N=2 order is 216 bytes rather
than the former 328-byte max-width record (112 bytes saved); an N=2 receipt is
192 bytes rather than 312 bytes (120 bytes saved); and N=2 custody is 208
bytes. At N=16, orders are 328 bytes, receipts are 304 bytes, and custody is
320 bytes. The receipt geometry also removes the former unused eight-byte
tail.

`VerificationPageV1<N>` and `SettlementPageResultV1<N>` retain only the
separate `MAX_EXECUTIONS_PER_PAGE_V1` fixed execution envelope. That bound is
an ephemeral stack/page-work bound, not an outcome vector and not an account
or rent geometry. The `N`-dependent orders, prices, cursors, and receipts
inside the envelope remain exact-width.
