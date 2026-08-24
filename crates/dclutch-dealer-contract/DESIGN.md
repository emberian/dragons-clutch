# Dealer capital contract

Status: SDK-free semantic contract. It is not yet a Solana adapter, executable
token transfer path, or deployment claim.

## Role and authority

Dealer is an optional covered-liquidity capability child. A `DealerBinding`
contains the complete occurrence-specific `MarketIdentity`, including its
generation, plus the selected Dealer capability release, adapter-authenticated
Dealer account identity, and sponsor. A state, epoch, quote, or receipt from a
different binding cannot be composed with it.

The Market collateral Hoard is not representable in this crate. Dealer capital
has exactly five kinds of value:

- quote-settlement cash;
- native claim inventory;
- sponsor loss capital;
- realized fees; and
- prepaid service funding.

These are separate semantic compartments. A customer sale is covered only by
unreserved quote cash. A customer purchase is covered only by unreserved native
claim inventory. Sponsor loss capital, realized fees, and service funding can
never silently satisfy either check. Expected future fees are not capital.

## Immutable capital epochs

Each epoch fixes bid and ask prices, one exact price denominator, fee basis
points, maximum quote lifetime, maximum quantity per quote, per-claim inventory
caps, and per-claim risk weights. Bid must not exceed ask. An epoch number is
immutable and reconfiguration admits only its exact successor.

Risk capital uses one named rounding boundary:

```text
required_loss_capital =
    ceil(sum(inventory[i] * risk_weight[i]) / price_scale)
```

The accumulation uses checked `u128`; the canonical result must fit `u64`.
Incoming quote reservations count against the worst-case projected inventory
without netting outgoing reservations. Thus one quote cannot depend on another
quote executing first to remain inside its cap or loss-capital coverage.

## Covered quotes and execution

Quote admission derives price from the authenticated current epoch rather than
accepting a caller-authored price. For a customer purchase, notional rounds up;
for a customer sale, notional rounds down. Fees round up at the one basis-point
boundary. Zero notional is refused.

Every admitted quote creates an exact `QuoteReservation` child. The Dealer root
holds only aggregate reservations and an exact outstanding-child count:

- customer purchase: reserve claim inventory;
- customer sale: reserve gross quote cash and incoming inventory capacity.

The quote child owns `Active -> Executed | Cancelled` replay state. Execution
atomically releases its aggregate reservation, moves the one inventory leg,
moves only Dealer cash, realizes its fee, and emits an `ExecutionReceipt` with
the prior and next value of every touched root field. The composing adapter must
authenticate the quote child, owners, signatures, token accounts, account
derivations, and atomic physical transfers before accepting the semantic
receipt. Cancellation authorization is also adapter policy.

One global `next_sequence` orders quote admission and capital transitions.
Quotes additionally have an immutable expiry slot. Epoch, generation, binding,
sequence, expiry, and child status are all checked independently.

## Capital entry, reconfiguration, and exit

Entry is an exact external deposit from zero. Reconfiguration and exit require
quiescence: zero outstanding quote children and zero aggregate reservations.
Reconfiguration provides explicit per-compartment deposits and withdrawals,
derives the new snapshot with checked integer operations, validates it against
the successor epoch, and emits the exact old/deposit/withdrawal/new receipt.
Exit returns every compartment and leaves an empty `Exited` replay state.

For each component, conservation is:

```text
old + external_deposit = new + external_withdrawal
```

Deposit and withdrawal in the same component are rejected, giving every net
transition one canonical representation. Reconfiguration cannot withdraw loss
capital below the successor epoch requirement or leave inventory above its new
cap.

## Fixed bounds and lifting

Every vector-bearing contract is parameterized by the Market's exact selected
native-claim width `N`. `N` is not persisted a second time: the authenticated
ClaimBasis/capability configuration selects the decoder profile, and that
profile accepts one exact byte length. Encoders take exact caller-provided
buffers, so the kernel remains `no_alloc`; checked `encoded_len()` functions
reject unsupported profiles and arithmetic overflow.

Current geometry is:

| Contract | Formula | `N = 2` | `N = 16` |
| --- | ---: | ---: | ---: |
| Capital snapshot | `32 + 8N` | 48 | 160 |
| Dealer state | `352 + 24N` | 400 | 736 |
| Capital epoch | `320 + 32N` | 384 | 832 |
| Capital transition receipt | `440 + 32N` | 504 | 952 |

Thus a binary Market does not allocate fourteen unused inventory, cap, risk,
price, or reservation entries. Binding, quote, and execution records contain no
vectors and retain their fixed 264, 344, and 456 byte widths.

`2 <= N <= MAX_NATIVE_CLAIMS`, where `MAX_NATIVE_CLAIMS = 16`, is the current
**provisional artifact-profile guard**, not a mathematical limit,
chain-derived maximum, permanent protocol ontology, or instruction to allocate
at the maximum. It permits bounded total loops after paying only for the
selected width. Its lifting path is a paginated inventory child contract whose
pages commit to one aggregate reservation and risk accumulator; quote claim
indices and exact receipts remain unchanged. No business rule depends on the
number 16.

The `u64` quantities, slots, sequence numbers, and compartment atoms are a
**chain-derived representation choice** aligned with Solana token amounts and
slots. Products that need wider coefficients must compile through an adapter
which proves the resulting native quantities fit this release. All products
and sums use checked arithmetic; no truncating cast or saturating fallback is
admitted.

The 10,000 basis-point denominator is a **mathematical unit definition**. It is
not a cap on richer pricing because an epoch's bid, ask, risk weights, and
`price_scale` are independent exact integers.

The exact selected widths in this release are schema facts, not optimization
evidence. The binary-width reduction removes avoidable account bytes, but its
lamport savings and compute effects still require Solana-adapter measurement
before selecting the final physical account split.

## Deliberate adapter seams

This crate does not hash identities, verify signatures, parse accounts, invoke
token programs, decide whether Dealer is a separate program, or authorize a
sponsor. A later adapter must:

1. authenticate the Market root and immutable capability manifest entry;
2. derive and own the Dealer and quote child addresses;
3. prove the capability release/configuration content identities and select
   the exact `N` decoder matching the authenticated native ClaimBasis;
4. compose every receipt with exact token/cash transfers atomically;
5. maintain the Dealer as one Market direct child and quotes as Dealer
   descendants; and
6. close quote rent to its recorded payer without treating rent as fees or
   liquidity capital.

Until those seams execute in a real local SVM route, this crate is kernel
evidence only.
