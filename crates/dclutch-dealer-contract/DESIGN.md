# Covered multi-LP Dealer V1

Status: implemented SDK-free semantic, instruction, exact-frame, derivation,
and Market/capability activation contract. It is not yet a Solana account-memory
or CPI adapter, deployed program, or performance claim.

## Mechanism and authority

Dealer V1 is an optional Market capability for immediate, fully covered
liquidity. It is a discrete two-sided quote-bin ladder, not the former sponsor
RFQ facility, a credit promise, or a compatible successor to either. There is
one state contract and no old decoder or parallel sponsor authority.

The Pool is the sole full attachment owner. Its 264-byte
`LiquidityAttachment` stores:

- the occurrence-specific `MarketIdentity`, including generation and native
  `ClaimBasis`;
- the Dealer capability release content identity;
- the immutable `LiquidityConfigV1` content identity; and
- the sole beneficiary of unused service collateral.

The Pool does not persist its own address. Configuration, LP-position, and
execution children carry only a 40-byte `ParentPool`: the physical Pool address
and Market generation. The adapter supplies the authenticated Pool address to
every transition. No physical child persists its own account address.

`LiquidityConfigV1<N, B>` is the one capability-bound V1 price record. Its
prices, fee, capacities, maximum trade quantity, and positive reset interval are
immutable for the Pool's lifetime. `decode(content_id, bytes)` carries the
adapter-authenticated content identity in memory without self-persisting it;
every Pool transition compares that identity with the Pool attachment. The SBF
adapter must verify the exact encoded bytes against that content identity before
calling the kernel. There is no configuration authority or successor-price
transition in V1.

The config is an immutable content-addressed physical record assembled before
Dealer activation and authenticated against the Market-selected capability
entry. Prices and capacities never travel in an activation instruction. V1
uses the config account's immutable RentCredit beneficiary as both the bootstrap
LP owner and the unused-service refund authority. The activation plan requires
the Pool attachment, Pool rent, initial LP rent, and signer account to name that
same authority. This prevents a permissionless activator from taking ownership
of prepaid liquidity without adding a second owner record.

## Canonical lifecycle authority

Dealer has one hostile-decodable instruction family with eight actions:
Activate/Open, create LP position, add, remove, trade, timed reset, close LP
position, and retire Pool/config. Wires contain only generation/replay guards,
compact LP IDs, requested shares/claim quantity, and maximum/minimum collateral
or reserve limits. They never contain an account key, observed balance,
FundingState compartment amount, price, capacity, fee rate, reset interval,
owner, rent amount, or Clock slot. Unknown actions, nonzero flags/reserved bytes,
wrong exact widths, zero compact IDs, and out-of-profile claim indices refuse.

The activation plan joins all of the following before returning any state:

1. authenticated Market identity, generation, and phase;
2. the manifest content ID committed by that Market;
3. the selected manifest entry from the FundingState's own entry index;
4. that entry's immutable Dealer release/config IDs;
5. the reusable capability-owned `CapabilityFundingDerivationV1` seed root;
6. the immutable bootstrap owner/refund/RentCredit authority;
7. exact observed FundingState capitalization; and
8. Market direct-child replay.

Rent and creation debit comes only from `FundingStateV1::activate`. LP principal
and service capitalization are the entire then-present Liquidity and Service
compartments released by `FundingStateV1`; neither amount can be supplied in the
instruction. The initial complete-set claim quantity and share quantity are
wire quantities, but the adapter must prove the Position owns and atomically
transfers every native claim. Pool, LP, and Pool-owned native Position funded
rent is obtained from authenticated Rent observations, and their checked sum
must equal the activation rent debit. The returned plan must be applied
atomically or discarded.

Retirement similarly joins the quiescent Pool transition to the Market's exact
direct-child decrement. `ExecutionReceipt`, liquidity-change, reset, position,
and retirement receipts are transient return-data/application plans; V1 defines
no receipt PDA and persists no duplicate receipt account. Replay authority lives
only in Pool, LP position, and Market state.

The exact domain-separated PDA seed preimages are:

```text
Pool   = ["dclutch/dealer-pool/v1",   Market key, generation_le, config_id]
Config = ["dclutch/dealer-config/v1", Market key, generation_le, config_id]
LP     = ["dclutch/dealer-lp/v1",     Market key, generation_le, config_id, lp_id]
```

FundingState deliberately does not get a Dealer derivation. Activation uses the
capability crate's shared six-component funding derivation after the adapter
authenticates the exact manifest content hash. Pool/config domain separation
prevents identical remaining seed components from aliasing.

Every action has one exact ordered frame. It names Market, Realm, native
Position, collateral Vault, Pool/config/LP roots, the applicable Pool custody
vaults and permanent RentCredits, collateral mint, Realm-selected token program,
System Program, and Rent where required. Account privilege is exact, not a
minimum. Ordinary aliases refuse except (a) activator/system-payer with LP owner
on creation and (b) multiple semantic rent destinations that resolve to the same
permanent RentCredit. Pool/config/LP/custody/Position aliases always refuse.
Reset has exactly Market, Pool, and config accounts: Clock is trusted adapter
input and is never an instruction meta or caller field.

Native claims are not tokens and Dealer does not create one token Vault per
claim. All categorized Pool claim inventory lives physically in one canonical
`PositionV1<N>` owned by the Pool PDA, using the Realm contract's shared exact
`["dclutch/position/v1", Market, Pool]` derivation. LP/trader Position and Pool
Position are separate exact roles and may never alias. Dealer's `claim_reserves`
are the categorized LP ledger mirrored by that Position; the adapter requires
exact equality before and after every transition. Three Realm-collateral token
Vaults are instead derived under
`["dclutch/dealer-vault/v1", Pool, compartment_tag]` for principal, realized
fees, and service funding. Their physical separation enforces the kernel's
compartment ontology. Permissionless token or native-claim deposits above the
categorized ledger are unclassified gifts, not fees or LP value. Retirement
routes token excess to the immutable service-refund Vault and claim excess to
that authority's canonical Position before closing custody. The Pool Position's
eventual close lamports go only to the permanent RentCredit derived for the Pool
authority.

The Pool has four disjoint value compartments:

1. LP principal collateral;
2. LP native-claim reserves, exactly one per claim in the exhaustive partition;
3. realized fee collateral owned by LP shares; and
4. prepaid service funding, which is never LP value.

There is no Hoard handle and no future-revenue field. Hoard principal cannot be
represented as liquidity, fees, rent, or service funding in this crate.

## Exact covered trades

The immutable config provides `B` bids and `B` asks for each of the exact `N`
native claims. Prices are integers in `(0, price_scale]`; bids strictly decrease,
asks strictly increase, and the best bid does not cross the best ask. Every bin
has a positive claim-quantity capacity.

Because the native claims form one exhaustive categorical complete set, raw
top-of-book prices also satisfy the checked invariant:

```text
sum_i best_bid[i] <= price_scale <= sum_i best_ask[i]
```

Both sums use checked `u64` addition. Equality is valid; a one-atom crossing is
refused, and fees may not rescue crossed raw prices. Since each bid row strictly
decreases and each ask row strictly increases with depth, the top of book is
extremal: depletion can only reduce the complete-set bid sum and increase the
complete-set ask sum.

Trades are immediate and all-or-nothing. The kernel walks best to worst using
only capacity remaining in the current time window:

```text
buy segment notional  = ceil(quantity * ask / price_scale)
sell segment notional = floor(quantity * bid / price_scale)
trader fee             = ceil(total notional * fee_bps / 10_000)
```

A segment that rounds to zero is refused. Each product is one checked
`u64 * u64 -> u128`, followed by explicit division and checked conversion to
`u64`. Sums, custody changes, fills, slots, shares, and replay sequences use
checked `u64` arithmetic. There are no floats, unchecked casts, saturating
fallbacks, allocation, or unbounded loops.

The solvency and depth invariant before and after every accepted trade is:

```text
principal_collateral >= every accepted gross bid notional
claim_reserves[i] >= every accepted gross ask quantity for claim i
filled[side][i][b] <= immutable_capacity[side][i][b]
```

Specifically:

- A buy succeeds only if the Pool already owns the entire claim quantity. The
  trader's gross present-collateral debit is `notional + fee`; principal receives
  only notional, fee custody receives only fee, and claims deliver the entire
  quantity.
- A sell succeeds only if LP principal already covers the entire notional. The
  trader supplies the entire claim quantity and separately supplies fee
  collateral. Principal pays the gross notional. The fee is not netted from
  proceeds and service funding cannot cover it.

`ExecutionReceipt` records gross trader legs, touched custody values before and
after, and selected-side bin fills before and after. Validation checks every
conservation equation and that fill deltas sum to exact traded quantity. Failed
computations do not mutate state.

## Honest time-window depth resets

Anytime resets would let a trader consume the best bin, reset immediately, and
bypass the depth curve. V1 therefore makes `reset_interval_slots` a positive
immutable configuration fact. At Pool opening:

```text
next_reset_slot = opened_at_slot + reset_interval_slots
```

A permissionless reset must match the current global replay sequence and use an
adapter-authenticated Clock slot `now_slot >= next_reset_slot`. It only zeros
fills for the identical authenticated config, increments `reset_number`, and
sets:

```text
next_reset_slot = now_slot + reset_interval_slots
```

All arithmetic is checked, so neither opening nor reset may wrap. Trades bind
both `reset_number` and sequence; a pre-reset quote cannot execute afterward.
Capacity is therefore an honest per-time-window depth limit. Coverage remains
authoritative after reset, so reopened capacity cannot create cash or claims.
New LP deposits cover current unfilled bins immediately and become available to
the entire identical ladder at the next timed reset.

V1's static prices create genuine adverse-selection risk. If information moves
the economic value away from the immutable ladder, arbitrage can consume its
time-window capacity and covered inventory. The interval limits how quickly the
best depth reopens; it does not make prices adaptive or protect LPs from a stale
model. There is deliberately no admin repricing that could rug incumbent LPs.
A differently priced venue requires retiring this Pool and authenticating a new
capability/config attachment, not mutating capital underneath existing shares.

## Multi-LP shares without dilution

Every provider has a compact independently replayed `LpPosition`; the Pool owns
total shares and the number of non-closed positions. The first LP supplies
positive principal and positive reserves for every native claim. Its chosen
initial share count defines the share unit.

For `m` new shares against `S` existing shares, every LP compartment independently
requires:

```text
deposit = ceil(reserve * m / S)
```

This includes realized fees. Because the deposit is at least `reserve*m/S`, the
new total cannot reduce an incumbent's exact per-share claim. A maximum vector
prevents silent substitution among collateral, fees, and claims.

For `b` burned shares:

```text
withdrawal = floor(reserve * b / S)
```

Flooring cannot extract more than the burned pro-rata claim. If `b == S`, the
division is bypassed and the last LP receives every remaining unit of principal,
every claim, and every realized fee. The Pool proves all LP compartments zero
and enters `Retiring`. A non-final burn whose whole vector rounds to zero is
refused. Service funding is absent from both formulas.

## RentCredit ownership and terminal closure

Every physical account stores only compact `RentCreditTerms`:

```text
beneficiary: 32 bytes
funded_rent_principal: u64
```

This identifies attributable funded principal but does not pretend to classify
the account's eventual excess lamports. On close, the adapter routes **all actual
lamports** to the permanent RentCredit derived for the beneficiary. Any excess
is an unclassified gift to that RentCredit; there is no persisted donation
recipient or donation amount truth.

Empty LP positions may be reused or closed under both replay clocks. Pool
retirement requires zero shares, zero LP value, and zero live positions. It
refunds unused service collateral only to its immutable beneficiary and emits
the Pool and config RentCredit terms. Rent, service, fees, and LP principal never
substitute for one another.

## Why V1 is not an FPMM

An exact complete-set FPMM remains attractive, but V1 does not claim one. A
direct product of sixteen `u64` reserves does not fit `u128`. Safe execution also
needs a conservative integer inverse and a proof that every rounded transition
preserves the intended invariant without creating an uncovered claim or
collateral obligation. Checking individual multiplications, truncating a wider
product, changing the invariant silently, or using floats would not prove that.

The quote-bin ladder has a smaller honest proof surface: each bin exposes finite
covered inventory, each multiplication has two `u64` operands and a `u128`
result, time-window depth is explicit, and custody coverage is checked before
mutation. It is not an AMM and does not claim durable or adaptive liquidity. A
future FPMM must be a distinct capability release after its
accumulator, inverse, rounding, and N-way coverage theorems exist.

## Exact widths and liftable bounds

`2 <= N <= 16` and `1 <= B <= 8` are **provisional measured-profile guards**,
not mathematical, business, or ontology limits. The lifting path is
authenticated paginated claim/bin children with one replay cursor and aggregate
custody commitment. Current fixed arrays keep loops statically bounded and the
kernel `no_alloc`.

| Contract | Formula | N=2, B=1 | N=2, B=8 | N=16, B=1 | N=16, B=8 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Immutable config | `128 + 32NB` | 192 | 640 | 640 | 4,224 |
| Mutable Pool | `392 + 8N + 16NB` | 440 | 664 | 776 | 2,568 |
| Execution receipt | `184 + 16B` | 200 | 312 | 200 | 312 |
| LP position | `152` | 152 | 152 | 152 | 152 |

The full Pool-only attachment is 264 bytes, compact parent references and rent
terms are each 40 bytes, and children never duplicate the attachment. Tests also
round-trip concrete N=2/B=2 widths (config 256, Pool 472, execution 216) and the
maximum N=16/B=8 profile. These are schema facts; rent and compute effects still
require SBF/local-validator measurement.

`u64` quantities, slots, and sequences are a **Solana-native representation
profile**. Wider native atoms require a later release with proved conversion and
accumulator bounds. The 10,000 basis-point denominator is a mathematical fee
unit.

Instruction and exact-frame widths are:

| Action | Wire bytes | N=2 accounts | N=16 accounts |
| --- | ---: | ---: | ---: |
| Activate/Open | 80 | 23 | 23 |
| Create LP position | 56 | 9 | 9 |
| Add liquidity | `88 + 8N` | 13 | 13 |
| Remove liquidity | `88 + 8N` | 13 | 13 |
| Trade | 56 | 12 | 12 |
| Timed reset | 24 | 3 | 3 |
| Close LP position | 64 | 7 | 7 |
| Retire Pool/config | 32 | 16 | 16 |

These counts exclude the executing Dealer program ID from the instruction's
account-index vector. All remain below the pinned 128 account-lock ceiling. A
one-signature, one-instruction legacy message under the canonical short-vector
serialization model is 976 bytes for N=16 Activate, 783 bytes for N=16
Add/Remove (including their 216-byte limit wires), and 729 bytes for Retire with
a separate fee payer. They fit the 1,232-byte packet individually without a
lookup table. Multi-instruction composition may still require v0/LUT transport.
Actual transaction compilation, LUT contents, compute, CPI depth, and rollback
remain SBF/local-validator measurements; these byte calculations are not
throughput claims.

## Required SBF and operator seams

This crate contains no Solana SDK, account memory, CPI, token-account parser,
signature parser, PDA hash function, Clock parser, or content hashing code. A
callable adapter must still:

1. authenticate the Market root/capability manifest, apply the exact Dealer and
   shared FundingState seed preimages, and derive/authenticate Pool, config,
   LP-position, custody, and permanent RentCredit addresses;
2. hash/authenticate the exact config content record, pass its content ID to
   `decode`, and select exact `N`/`B` only from the native `ClaimBasis` and
   authenticated config;
3. supply the actual Pool address and Market generation for every compact-child
   join, and authenticate LP owners for withdrawals/closure;
4. read `Clock::slot` through the trusted adapter Clock boundary for opening and
   reset, never from an instruction meta, operator, or client field;
5. maintain physically and semantically distinct collateral custody for LP
   principal, realized fees, and service funding, plus exact native-claim
   custody;
6. atomically apply every gross receipt leg. A sell transfers fee collateral
   independently of the principal payment; a buy splits gross collateral into
   principal and fee custody;
7. route all close lamports to the beneficiary's derived permanent RentCredit,
   treating only recorded funded principal as attributed and all excess as an
   unclassified gift;
8. maintain Market-child/position descendant counts and atomic rollback across
   every token, state, and close operation; and
9. derive unsigned operator material from authenticated accounts and exact
   receipts. Static clients and indexes remain untrusted projections.

Until those seams execute under adversarial local-validator tests, this is a
pure contract implementation, not an end-to-end liquidity claim.
