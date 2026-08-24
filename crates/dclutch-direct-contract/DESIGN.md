# Direct signed-intent contract

`dclutch-direct-contract` is a safe-Rust, `no_std`, `no_alloc`, fixed-layout
kernel contract. It has no Solana SDK, token, account, CPI, signature, or hash
dependency.

## Authority and replay

The adapter signs/verifies the exact `DirectIntentV1::signed_preimage` and
provides an `OwnerAuthorization`. The checker requires the authenticated owner,
signed maker, and affected `PositionV1` owner to agree. It binds Market identity
and generation, maker, nonce, inclusive slot interval, side, outcome, capacity,
and limit price. `IntentStateV1` binds the locator `(market, generation, maker,
nonce)`, tracks exact partial fill, and has a terminal maker-authorized cancel.
A matcher selects only compatible fills and prices. Each signed intent also
binds the exact fee-config/release identity and fee rate. The adapter creates
`MarketVenueAuthorization` only after authenticating that the immutable Market
selected that config/release; the kernel requires that attestation, the policy,
and every signed intent to agree on Market, generation, config, and rate.
Therefore a caller or matcher cannot invent a fee policy.

## Settlement

Ordinary settlement moves one selected native claim from sell maker to buy
maker, transfers exact quote collateral to the seller, transfers a fee to the
venue recipient, and returns both replacement Positions and replay states.

Complementary settlement has exactly N Buy intents in canonical outcome order.
Each has a distinct owner/Position, shares one fill, and uses prices summing
exactly to `PRICE_SCALE`. The checker credits each indexed outcome and requires
gross buyer debits to equal the fill; the adapter must atomically put that exact
amount in the Market collateral vault. Fees are separate buyer debits and
transfers. Complementary sells have exactly N Sell intents in the same
canonical order from one owner and atomically debit that owner's complete set;
the Market vault debits exactly `fill`, seller receives `fill - fee`, and the
venue receives the fee. There is no Hoard, reserve, future-fee funding,
candidate account, or General-style workflow.

## Arithmetic and bounds

Quotes use exact scaled integers: `fill * price / PRICE_SCALE` must divide
exactly or refuse. The one named rounding boundary is
`FeeRounding::Floor = floor(gross * bps / 10_000)`. All arithmetic is checked.

The N=2..16 width is a **provisional measured-profile bound** inherited from
`PositionV1`, not a mathematical maximum. Lifting it requires a new reviewed
Position/capacity profile and a direct-contract schema release that rechecks
canonical complementary ordering and fixed layout. `u64` domains are machine
mathematical bounds and overflow always refuses.
