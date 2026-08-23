# Transferable structured claim: implementation-ready adapter plan

Status: **PURE STRUCTURED-CLAIM KERNEL IMPLEMENTED; SBF ADAPTER NOT IMPLEMENTED**
(2026-08-22).

The production-bound, allocation-free `no_std` semantic core now
lives in `crates/clutch-structured-claim`. It promotes the exact rational
compiler boundary, canonical backing algebra, flat associative composition,
deployment-bound identity preimages, supply-sensitive wrap/unwind, direct burn,
beneficiary-free compaction, exact terminal lots/redemption, and retirement
into fixed-capacity safe Rust with transactional refusals and frozen vectors.
It deliberately does not promote any Solana account, Token-2022, hashing, CPI,
replay, reservation, or deployment-authentication concern into the core.
Base Hoard/total-supply mutation delegates to general transitions in the
first-party `clutch-kernel`; the SBF adapter must additionally reconcile its
authenticated internal/external SupplyLedger closure and enforce the immutable
collateral cap.

The remaining implementation work in this plan is therefore the adapter and
bank-evidence boundary, not a redesign of wrapper economics. In particular,
the base atomic Position transfer, donation transitions, and aggregate-vector
redemption described below are still absent from the live SBF dispatcher.

This plan names the smallest trustworthy runtime seam for a genuinely
transferable shaped position without narrowing the protocol's analytic or
clearing ambitions. It does not treat an atomic portfolio order as a bearer
asset, does not add a second liability to the Hoard, and does not store a
wrapper-of-wrapper graph.

## Audit conclusions

The existing repository already owns more of this design than the earlier
wrapper note recognized:

1. `clutch-solana-layout::portfolio_settlement::NativePortfolioClaimV1` is the
   canonical live identity for a GCD-one Egg coefficient vector. It binds the
   Market, the complete Terms digest, degree, denominator, outcome count, and
   canonical padded coefficients. A wrapper must reuse it. The Python
   `ClaimDescriptor.digest` now mirrors these bytes, with a Rust/Python golden
   vector, rather than defining a competing runtime identity.
2. `TermsAccount::binds_market` checks the immutable Realm, Profile, feed,
   width, and Terms digest relationship. Wrapper creation must consume both
   authenticated accounts rather than accepting a caller-supplied digest.
3. `PositionAccount` already stores both free `cash_atoms` and the fixed native
   internal Egg vector. Sell reservation moves Eggs out of `Position.internal`
   into the per-order `ReservationAccount`; buy reservation is represented by
   `reserved_cash_atoms`. A checked Position-to-Position move can therefore
   consume only free assets without scanning all open orders.
4. Portfolio clearing settles exact native Egg vectors. It does not settle a
   shaped wrapper, and a successful coefficient order leaves separable Eggs.
   Joining the wrapper to the same `NativePortfolioClaimV1` makes post-fill
   wrapping deterministic; it does not retroactively make the order a
   transferable shape.
5. The current Split/Merge routes already establish the exact equivalence
   between one cash atom and one complete Egg set while Active. This makes
   complete-set compression an existing algebra, not a new pricing rule.
6. The current per-Egg redeem routes may refuse fractional components. A direct
   wrapper redemption therefore needs an aggregate exact-vector base
   transition or an explicit remainder ledger. Unwrapping to owned backing
   must remain available regardless.
7. Token-2022 supply, not a descriptor shadow, must remain the wrapper-supply
   truth. Ordinary direct holder burns create surplus backing and never a
   caller, keeper, fee, or treasury entitlement.

## Canonical economic representation

Let `p` be the primitive GCD-one coefficient vector named by the live native
portfolio claim. Define:

```text
k   = min_i p_i
r_i = p_i - k

1 W_p <-> k Position cash atoms + r_i internal native Eggs
```

For every admitted weight vector `w`, where `sum(w) = D`:

```text
k + dot(r, w)/D
  = k*sum(w)/D + dot(p-k, w)/D
  = dot(p, w)/D
```

The transformation is exact. At least one `r_i` is zero, so the vault never
stores a redundant complete set. This decreases the base Market's Egg supply
and Hoard collateral together by `k` per wrapper while moving the released
cash into the wrapper Position. The wrapper remains a custody receipt over
already-accounted assets; it is not counted in `SupplyLedgerAccount`.

The wrapper may also accept the full `p` Egg vector. While the Market is Active,
it first merges the `k` complete sets in its vault and reaches the same
canonical backing. After resolution, Split/Merge are unavailable; the canonical
cash-plus-residual representation is therefore the phase-independent unwind.

## Compiler and identity boundary

`research/bspline-shape-compiler/src/wrapper.rs` implements the host boundary:

- exact `BigRational` coefficients are integerized through the least common
  denominator and divided by the integer-vector GCD;
- the minimal pair `(wrapper atoms, target-shape units)` is retained so neither
  analytic amplitude nor token quantity is silently rounded;
- the primitive vector is passed through the live
  `NativePortfolioClaimV1::compile` owner;
- the common complete-set floor and residual vector are derived;
- proportional analytic artifacts converge on one native claim and one wrapper
  product; and
- certificate digest, analytic family, label, and display scaling remain
  content-addressed provenance rather than fungibility inputs.

The wrapper-specific product id is:

```text
SHA256(
  "dragons-clutch/transferable-wrapper/v1" ||
  wrapper program || wrapper ProgramData || wrapper deployment slot ||
  base program || base ProgramData || base deployment slot ||
  Token-2022 program || Token-2022 ProgramData || Token-2022 deployment slot ||
  backing policy version || native claim id
)
```

This is deliberately stricter than the native claim id. The native claim says
which payoff vector is being traded. The wrapper product additionally says
which executable and token trust boundaries custody it.

ProgramData slots make upgrades visible; they do not make an upgrade authority
trustless. A public bearer wrapper whose exit depends on base, wrapper, and
Token-2022 code should be promoted only under a release policy that explicitly
accepts all three upgrade-governance boundaries; immutability is the strongest
profile. If an authenticated ProgramData slot changes, an old descriptor
refuses every mint, burn-for-backing, compaction, and direct redemption route
until an explicit migration design exists. That fail-closed rule also means an
upgradeable profile cannot promise unconditional future exit. A checked release
manifest is still required before describing any deployment as official.

## Persisted accounts

### StructuredClaimDescriptorV1

The descriptor stores one semantic representation and derives redundant facts:

| field | bytes |
| --- | ---: |
| tag, version, flags | 4 |
| base program | 32 |
| base ProgramData | 32 |
| base deployment slot | 8 |
| wrapper ProgramData | 32 |
| wrapper deployment slot | 8 |
| Token-2022 program | 32 |
| Token-2022 ProgramData | 32 |
| Token-2022 deployment slot | 8 |
| Market | 32 |
| Terms digest | 32 |
| primitive `[u64; 16]` | 128 |
| state and descriptor/mint/vault bumps | 4 |
| total | 384 |

The wrapper program id is the account owner and PDA derivation program, so it
is not persisted twice. Outcome count comes from authenticated Terms. Native
claim id, wrapper product id, complete-set floor, residual vector, and mint PDA
are recomputed and checked. No supply, price, payout, analytic bytecode,
certificate, label, or mutable metadata is stored.

At the default Rent parameters this 384-byte descriptor, an 82-byte extension-
free mint, one current 220-byte base Position, and one 84-byte base Replay cost
about `0.008922720 SOL` in persistent infrastructure. One ordinary 170-byte
`ImmutableOwner` holder account costs `0.002074080 SOL`. These are arithmetic
estimates, not compiled-program or live-bank measurements.

### Wrapper mint

- exact pinned Token-2022 program;
- decimals zero;
- mint authority is the canonical wrapper-authority PDA;
- no freeze authority;
- no mint extension;
- no transfer hook, transfer fee, permanent delegate, close authority,
  interest/scaled-UI semantics, pausing, confidential state, metadata pointer,
  group pointer, or permissioned burn; and
- actual mint supply is authenticated on every supply-sensitive instruction.

### Wrapper vault Position and Replay

The base Position is canonically derived from `(Market, wrapper-vault owner)`.
It holds only free `cash_atoms`, zero `reserved_cash_atoms`, and internal native
Eggs. The wrapper descriptor is never permitted to place an order from this
Position. The Replay account is the base program's existing mutation ordering
anchor. A unique vault owner PDA has no external signer or alternate authority.

## One required base transition

Add an exact, supply-neutral base instruction:

```text
AtomicPositionAssetTransferV1 {
    market,
    source_owner,
    destination_owner,
    source_generation,
    destination_generation,
    cash_atoms,
    internal: [u64; 16],
    phase_policy: ActiveOrResolved,
}
```

The base adapter must authenticate:

- exact program id, Market, Terms, both Position PDAs, owners, generations,
  Replay PDAs/sequences, and signer/PDA authority;
- both Positions open and belonging to the same Market;
- `cash_atoms <= source.cash_atoms - source.reserved_cash_atoms`;
- each internal debit against `source.internal` (already free because sell
  reservation removed reserved Eggs from that field);
- canonical zero padding at the Market width;
- checked destination additions;
- distinct semantic owners and nonaliased writable accounts; and
- byte-exact pre/post conservation of Position cash and every internal Egg.

It changes no Hoard, supply ledger, collateral token account, or external Egg
mint. Eggcrate should own the fixed-vector arithmetic, extending its existing
single-outcome `transfer_internal` theorem; the Solana adapter owns the account
and replay facts. A refused transition writes nothing.

This one transition enables canonical-backing wrap/unwrap with one base CPI.
The full-Egg convenience route may compose this transfer with existing
Merge/Split in the same Solana transaction. Only if measured CU/account limits
justify it should the base add fused `CompressAndTransfer` and
`ExpandAndTransfer` instructions; their post-state must be byte-identical to
the two existing semantic steps.

## Wrapper transitions

Every route computes and authenticates the complete post-state before its first
CPI, checks every CPI result and exact post-delta, and relies on Solana rollback
only after all local validation has succeeded.

### CreateDescriptor

1. Authenticate active Market and self-certifying Terms; run
   `TermsAccount::binds_market`.
2. Authenticate the exact base/wrapper/Token-2022 loaders, ProgramData accounts,
   and deployment slots.
3. Compile/check the primitive vector with live `NativePortfolioClaimV1` rules;
   refuse zero, single-Egg, and constant complete-set products.
4. Derive descriptor, mint, authority, vault owner, vault Position, and Replay
   PDAs from the wrapper product identity.
5. Initialize the extension-free Token-2022 mint and empty base Position.
6. The creator prepays all rent. Hoard principal and future fees fund nothing.

### WrapCanonicalBacking

For `q > 0`, compute `q*k` cash and `q*r_i` Eggs with checked arithmetic. This
custody move changes neither base supply nor Hoard and is valid while Active or
Resolved:

1. authenticate actual mint supply `S`, holder destination, source Position,
   descriptor, and empty-reservation wrapper vault;
2. invoke `AtomicPositionAssetTransferV1` from source to vault;
3. invoke Token-2022 `MintToChecked(q)`; and
4. require supply `S + q`, destination `+q`, vault cash `+q*k`, and each vault
   Egg `+q*r_i` exactly.

### WrapFullEggVector

When `k > 0`, while Active only:

1. move `q*p_i` Eggs into the wrapper vault;
2. invoke existing base `Merge(q*k)` as the vault PDA, leaving `q*r_i` Eggs and
   crediting `q*k` vault cash;
3. mint `q` wrappers; and
4. authenticate the same canonical post-state as `WrapCanonicalBacking`.

The initial implementation uses two base CPIs plus the mint. A fused base
transition is an optimization only after the composed route has bank evidence.
When `k = 0`, the full vector is already canonical, no Merge CPI exists, and
the exact route is valid after resolution too.

### UnwrapCanonicalBacking

In either Active or Resolved phase:

1. burn exactly `q` wrappers from the authenticated holder;
2. move `q*k` free cash and `q*r_i` Eggs from vault to the owner's base
   Position; and
3. authenticate supply `S - q`, holder `-q`, and exact vault/owner deltas.

This is the unconditional ownership exit. It returns an exact payoff-equivalent
representation, even after the base Market can no longer Split cash into a
complete set.

### UnwrapFullEggVector

When `k > 0`, while Active only, burn the wrapper, invoke base `Split(q*k)` in
the vault, and move `q*p_i` Eggs to the owner. When `k = 0`, full and canonical
backing are byte-identical and the route remains available after resolution.
This is a convenience representation change; it is not more solvent than
canonical backing.

### Transfer

An ordinary Token-2022 transfer changes only bearer ownership. It neither calls
the wrapper nor mutates backing.

### Direct holder burn and CompactDonation

A direct Token-2022 burn decreases actual supply without releasing backing. For
authenticated supply `S`, define:

```text
cash surplus = vault_cash - S*k
Egg surplus_i = vault_internal_i - S*r_i
```

Both are nonnegative or the descriptor is insolvent and every route refuses.
`CompactDonation` moves cash surplus into the base Hoard without minting claims
and destroys residual-Egg surplus through a base donation transition that
decreases internal supply without releasing collateral. It pays nobody. Until
both donation transitions have live evidence, surplus remains locked and safe.

### Direct terminal redemption

The first runtime version should keep `UnwrapCanonicalBacking` available and
make direct collateral redemption opt-in. A direct route burns `q` wrappers,
consumes `q*k` vault cash and `q*r_i` residual Eggs, and pays:

```text
q*k + q*dot(r,w)/D = q*dot(p,w)/D
```

It requires a base aggregate-vector redemption transition so residual legs are
judged together. It must either require the exact resolved lot

```text
L_w = D / gcd(D, dot(p,w))
```

or persist a holder-owned exact numerator remainder. Silent floor rounding,
dust seizure, and dust-to-treasury are forbidden. Unwrapping is never disabled
because a holder owns fewer than `L_w` wrappers.

### Retirement

Retirement requires authenticated Token-2022 supply zero, vault cash zero,
every vault Egg zero, no reservations, and no pending replay/migration work.
The extension-free mint cannot close and remains a canonical tombstone. The
descriptor also remains an immutable tombstone unless a permanent identity
registry makes close/recreate impossible.

## Flattened composition instead of nesting

Persisted wrapper-under-wrapper edges remain forbidden. Composition is more
general and cheaper when flattened to the one native basis.

For input wrapper quantities `q_j` and primitive vectors `p^j`:

```text
A_i = sum_j q_j * p^j_i
g   = gcd_i A_i
p'  = A / g
```

Burning all inputs releases exactly `A`. When `p'` is nonconstant, minting `g`
output wrappers of `p'` reuses it exactly. When `p'` is constant, the compiler
routes the output to complete-set cash and refuses to create a cash wrapper.
Complete-set compression composes too. Input wrappers carry
`C_in = sum_j q_j min(p^j)` cash. The combined vector's canonical cash is
`C_out = min_i A_i`, and

```text
C_out >= C_in
```

because the minimum of a sum is at least the sum of minima. The difference
`C_out - C_in` is a complete-set floor newly exposed by combination; merge that
many sets from the released residual Eggs before minting the output. The Rust
host model computes all four values and tests regrouping/associativity.

A future `FuseWrappers` may execute this bounded operation atomically for
same-wrapper-program, same-Market descriptors. It still leaves the output vault
backed only by native cash/Eggs. Single-Egg/complete-set input wrappers,
cross-Market composition, negative legs, cycles, arbitrary token inputs, and
overflow refuse. A complete-set *output* exits as cash. Negative composition is
a funded order, not a bearer asset.

## Clearing integration

The wrapper does not alter the batch relation:

- portfolio orders continue to bind exact nonnegative native Egg coefficients;
- reservations and clearing use the same `NativePortfolioClaimV1` as the
  wrapper compiler;
- settlement continues to credit separable Eggs; and
- a holder explicitly invokes a wrapper transition afterward to obtain bearer
  product identity.

This preserves the distinction between atomic execution and persistent
atomicity. A wrapper token can trade on a Token-2022 venue as one asset. Adding
wrapper-token legs directly to the base batch would create a second asset and
reservation algebra and is unnecessary: split or wrap at the boundary, then
clear native Eggs under the relation that already verifies them.

An advanced maker may place a native portfolio order from a wrapper-controlled
Position only through an explicit strategy account whose beneficiary, funding,
and mint-after-settlement state machine are frozen. The generic descriptor
vault itself never trades; backing cannot be reserved, scored, or lent while it
supports bearer supply.

## Required adversarial campaign before promotion

1. Rust/Python/cross-client claim and product digest equality, including
   proportional rational compiler outputs.
2. SVM Token-2022 create/mint/ordinary transfer/direct burn/wrapper burn paths
   with exact mint and holder deltas.
3. Base atomic Position transfer at widths 2, 4, 8, and 16, including cash
   reservations, seller reservations, generations, replay, alias, close state,
   foreign Terms/Market, padding, and every overflow edge.
4. Full-Egg and canonical-backing routes reaching byte-identical canonical
   vault state.
5. Resolution races: Active full unwrap succeeds before resolution; after
   resolution it refuses without mutation while canonical unwind succeeds.
6. Direct holder burn followed by no compaction, partial compaction refusal,
   and exact permissionless compaction with no recipient credit.
7. Aggregate redemption exact and inexact lots, weight substitutions, stale
   Resolution, Hoard underfunding, and transaction rollback after each CPI.
8. Flatten/fuse associativity, scalar normalization, newly exposed complete-set
   floors, cross-Market refusal, and account/CPI-depth ceilings.
9. Upgradeable-loader substitution and deployment-slot change refusals.
10. Retirement with nonzero supply, cash, any Egg, reservation, replay, or
    donation surplus.
11. Bank measurements for descriptor creation, both wrap/unwind forms,
    compaction, redemption, and 2/4/8/16-outcome fusion.

Until these gates pass, this directory demonstrates exact host algebra and an
implementation boundary. It is not deployed-wrapper, Token-2022 runtime, or
mainnet evidence.
