# Terminal lifecycle V2 (model-only)

Status: **MODEL-ONLY / HOST-TESTED**. The executable model is
[`research/terminal-lifecycle-v2`](../../research/terminal-lifecycle-v2/). It
does not change a deployed program, SBF/kernel source, account ABI, existing
market, Token-2022 mint, or release claim.

## Scope and STOP boundaries

V2 is a creation-time profile for new markets. A legacy mint without a creation
time Token-2022 mint-close authority returns `LegacyStop`; this work neither
migrates it nor claims it is terminal. The model also rejects fractional claim
remainders. It has no fractional-credit/carry account, so that route remains a
STOP, not a rounding policy.

The model is not a live-token adapter. It cannot authenticate signer, program,
PDA, owner, extension, or post-CPI account state. It is therefore an
**internal-only terminal profile**: the aggregate external bearer fields exist
only to reject corrupt prestate, while `materialize_bearer` and `burn_bearer`
return `ExternalBearerStop`. Allowing a bearer transfer before a per-account
exact-lot/credit model exists would create a terminal dead end. A production
design must prove those adapter facts separately; this model does not authorize
a deployment or migration.

`research/terminal-economics-r4` is a separate later model, not an
implementation or promotion of V2. It permits arbitrary raw bearer quantities
only by retaining a CreditRoot/CreditVault and every nonzero claimant credit;
its `I`/`E`/`A` supply planes make the raw-bearer, indivisible-collateral,
tombstone-only conflict explicit. It likewise supplies no live ABI,
Token-2022 adapter, authority, migration, or SBF terminal walk.

## Versioned account and rent identities

Every V2 account carries the immutable tag
`(market_id, V2, generation)`. A rent principal contains that tag plus a
distinct `Role`, role-derived account identity, fixed `refund_to`, and its own
nonzero lamport principal. The model maintains a per-role payment ledger; a
role is paid exactly once, and its principal cannot alias a different role.

| Role/account | Fact owned | Principal disposition |
|---|---|---|
| Market | lifecycle, terminal authority, neutral sink | fixed market refund, once |
| Hoard | collateral and unowned donations | fixed market refund, once after zero collateral |
| Kernel | aggregate/kernel lifecycle | fixed market refund, once after Resolution |
| Supply | program ledger totals | fixed market refund, once after all mints |
| Resolution | immutable payout and receipt | fixed market refund, once after Supply |
| Position(slot) | internal holder claims | its fixed recipient, once after zero claims |
| Mint(outcome) | authoritative outcome supply | fixed market refund, once after authoritative zero |
| Replay | compact terminal receipt | separately prepaid, permanent, never refunded |

Resolution has its own principal when created; it does not reuse Market rent.
The Replay principal is prepaid at market creation and survives Market close.
It is copied into a replay PDA with only the V2 identity and terminal receipt,
so a recreation or stale final-close replay is rejected without retaining the
closed graph or reusing refunded Market rent.
The model cross-checks that persistent tombstone against the closed Market's
exact Replay `RentIdentity` and confirms its role was never refunded.

## Conservation, authoritative supply, and surplus

For each outcome `i`, the hostile-prestate invariant is:

```text
sum(Position.claims[i]) + external_bearer[i]
  == Supply.total[i]
  == OutcomeMint.authoritative_supply
```

`check()` rejects either aggregate closure failure or a mint/ledger mismatch.
In the active internal-only phase, each `Supply.total[i]` must also equal the
complete-set ingress accumulator; direct donations cannot manufacture claims.
`external_bearer` must be zero in every reachable phase, not merely at
resolution. `terminal_receipt` is zero before Market terminalization and then
must equal Resolution's immutable receipt.
Terminal mint close takes an explicit supplied authority, authenticates it
against both the per-mint `MintCloseAuthority` and Market terminal authority,
and requires all three quantities to be zero. This model does not claim that a
cached supply is adequate.

The same exact-lot check runs separately for every Position before admitting a
resolution and on every resolved-state check. Thus two independently owned
`1`-unit positions cannot sneak the aggregate `[1,1]/2` test even when their
summed numerator happens to be whole, and a hostile post-resolution
redistribution to fractional lots is rejected.

For a resolved vector `w_i / D`, every `S_i * w_i` must divide by `D` exactly
and `sum_i(S_i * w_i / D) <= Hoard.collateral`. A direct prefund increases
Hoard donation/collateral only; it is never a claimant or rent-refund credit.
A voluntary burn can create surplus, but surplus moves only after liability is
zero and only to Market's immutable `surplus_sink`. The sink is required as an
explicit input and must not be the rent recipient. It is a disposal destination,
not a hidden refund path.

The hostile `[1, 1] / 2` fragment is therefore refused: its aggregate happens
to equal one atom, but each individual bearer claim is half an atom. Combining
unrelated bearers would change ownership.

## Ordered, exactly-once close effects

Each close transition is transactional in the model: refusal leaves the state
and rent ledger unchanged. `closed` status and the payment ledger cross-check
each other, so replay of Position, Mint, Supply, Resolution, Kernel, Hoard, or
Market close returns `AlreadyClosed` rather than paying rent again.
The ledger must equal the exact set of actually present closed roles: vacant
Position slots and padded outcome slots have canonical empty state and cannot
contain a phantom paid principal.

```text
zero-claim Positions
  -> zero authoritative-supply V2 outcome Mints (supplied MintCloseAuthority)
  -> Supply
  -> Resolution
  -> Kernel
  -> Hoard after liability = 0 and surplus_sink disposal
  -> Market (supplied terminal authority)
  -> retain independently prepaid Replay tombstone forever
```

The host tests include prefund/donation, guessed refund recipient, wrong close
authority, holder burn surplus, `[1,1]/2`, corrupted aggregate/mint prestate,
bearer-profile STOP, permuted close order, every close replay, stale
replay/recreation, and legacy uncloseable-mint refusal.

## Adapter obligations deliberately absent

A real implementation still needs canonical PDA and account layouts, token
account and mint-owner authentication, signer and close-authority checks,
Token-2022 extension compatibility, authoritative post-burn/post-close reads,
atomic CPI ordering, rent-exemption accounting, replay-PDA seed binding, and
tests against the pinned live Token-2022 program. It must also decide a complete
bearing-claim redemption path or explicitly retain the same STOP boundary; no
model result here supplies that authority.
