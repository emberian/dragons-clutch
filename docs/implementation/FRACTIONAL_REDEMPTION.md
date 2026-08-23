# Exact fractional redemption for native B-spline Eggs

Status: **RUNTIME CONTRACT PROMOTED / SBF DISABLED** (2026-08-23).
`crates/clutch-fractional-redemption-runtime` now owns the safe `no_std`,
no-allocation, fixed-layout transition and account contract. Intent family
79/v1 and account tags `0xa4..=0xa7` remain `ReservedDisabled`; no Solana route
or release capability is enabled. `research/fractional-redemption` remains the
derivation and exhaustive small-domain model, not a second runtime truth.

## 1. The obligation

For one immutable native resolution vector:

```text
D > 0
w_i in [0,D]
sum_i w_i = D
T_i = remaining quantity of native Egg i
K = sum of every persistent credit numerator
C = collateral atoms retained by the Hoard
```

The exact resolved liability is measured in numerator units:

```text
R = sum_i T_i*w_i
D*C >= R + K
```

`K=0` for exact-lot semantics. Under credit semantics, `K` is not dust, a
rounding pot, revenue, or an inference from user accounts. It is a first-class
market liability with one persisted market aggregate. A transition may pay
only whole collateral atoms. No transition floors and forgets the remainder,
credits it to treasury, makes it an executor bounty, or treats a direct holder
burn as permission to withdraw surplus.

The version-three native Resolution design supplies the immutable Market,
common `D`, exact vector, and repair generation. A runtime integration must use
the Resolution-owned vector (or the terms-owned preset), never a client copy or
a second persisted vector. The model deliberately does not edit that active
integration surface.

## 2. Exact lots

### 2.1 Resolved and universal formulas

For a frozen resolved weight `w`, the least positive integral redemption lot is

```text
L(w) = D / gcd(D,w).
```

This includes the useful edge cases: `L(0)=1` because a losing Egg burns for
zero, and `L(D)=1` at a one-hot endpoint.

For one **fixed resolved vector**, the least common quantity that can redeem
every outcome independently is:

```text
L_resolved = lcm_i L(w_i).
```

This is often strictly smaller than `D`. For `[16,40,8]/64`, the per-outcome
lots are `[4,8,8]` and `L_resolved=8`. Calling `D` the smallest resolved lot
would be wrong.

Before resolution, for reachable weights `W_i` of outcome `i`, the least safe
lot is

```text
L_i = D / gcd(D, {w : w in W_i}).
```

The model checks both integrality and minimality exhaustively for denominators
through 24. If admission deliberately quantifies over **every** integer-simplex
vector, the family contains weight `1`, so its conservative common lot is:

```text
L_i = D for every outcome.
```

That is a safe upper bound, not a proof about the smaller family actually
reachable by one B-spline degree/knot/domain/quantizer instance. The current
evaluator evidence does not establish that every terms instance attains gcd 1
for every outcome. A terms compiler can enumerate or prove each reachable
weight gcd and freeze a smaller `L_i`; accepting that result creates a new
admission/certificate boundary. Until that proof exists, `D` is the simple
conservative pre-resolution lot, not the mathematically smallest claim about
the real reachable family.

For an optional structured wrapper with primitive nonnegative coefficients
`a`, the model also checks:

```text
universal L_a = lcm_i D/gcd(D, |a_i-a_0|)
              = D/gcd(D, |a_i-a_0| for all i)

resolved L_a(w) = D/gcd(D, dot(a,w)).
```

The universal formula and its minimality are exhaustively tested over small
three-outcome integer simplexes. This does not promote direct wrapper
redemption: unwrapping to exact native components remains the conservative
path described by `research/structured-claim-wrapper`.

### 2.2 Closure requirements

Lots solve per-wallet exit liveness only if the entire path that can separate a
complete set preserves a pre-resolution lot. If terms freeze per-outcome
`L_i`, each outcome-moving path uses its own lot; if the runtime chooses one
common lot, it uses `lcm_i L_i`. Under the conservative all-integer-simplex
family, that common lot is `D`. The gate covers:

- Split and Merge quantities;
- internal transfers and funded order reservations;
- every scalar or coefficient-intent fill leg;
- Materialize and Dematerialize conversions; and
- per-outcome and direct-wrapper redemption.

Complete-set redemption is independently exact at every quantity because
`sum_i q*w_i = q*D`. It is an important balanced exit, but it does not repair an
unbalanced sub-lot holding. Allowing arbitrary Split while later allowing one
component to move recreates the dead state.

Ordinary Token-2022 transfers cannot enforce a raw-claim lot. Existing one-atom
Eggs may remain freely transferable: after resolution a holder can aggregate
at least `L(w_i)` atoms of outcome `i` in one account and redeem them exactly.
That is a sound exact-or-refuse policy, but not a promise that every wallet's
arbitrary fragment can exit by itself. Two stronger external encodings are
materially different:

1. **One token atom = one raw internal claim.** A holder may fragment `D` raw
   units among wallets. Aggregate liability stays correct, but each sub-lot is
   unredeemable until voluntarily recombined. A hostile dust transfer cannot
   steal value, yet it can create irreparable account/retirement clutter.
2. **One token atom = a frozen pre-resolution lot of raw internal claims.**
   Materialize and Dematerialize perform the exact scaling. Every ordinary
   bearer transfer then moves a whole lot. Per-outcome `L_i` gives different
   economic scales across mints; a common lot avoids that display hazard. `D`
   is one conservative common choice, not necessarily the smallest.

Only the second encoding provides total bearer exit under a zero-state lot
policy. It does not make indivisibility disappear. In the current dimensions,
`Split(q)` deposits `q` collateral atoms and creates `q` raw claims of every
outcome. If one bearer token atom represents `L` raw claims, creating one such
bearer atom requires at least `L` raw claims, normally produced by splitting
`L` collateral atoms. Redefining Split to deposit one collateral atom while
minting one `L`-raw-claim bearer complete set would multiply liabilities by
`L` and break the complete-set identity. Token decimals also do not perform
this conversion; it is an explicit economic quantity mapping frozen in Market
terms, SDKs, receipts, and exact post-CPI checks.

### 2.3 Exact-lot costs and strengths

Strengths:

- zero new persistent liability state;
- no new claimant object, rent payer, replay lane, transfer instrument, or
  terminal residual rule;
- no floor or rounding boundary at redemption;
- direct bearer redemption remains positionless; and
- current exact-or-refuse kernel arithmetic is reusable.

Costs:

- the global quantity unit leaks into Split, the venue relation, every order
  reservation, and every internal/bearer bridge;
- a conservative `L=D=65,536` profile has a minimum pre-resolution separable
  raw holding normally backed by 65,536 collateral atoms (0.065536 units for
  six-decimal collateral, about 0.000065536 units for nine-decimal collateral);
- the existing one-raw-unit external encoding cannot honestly claim arbitrary
  bearer exit; and
- after resolution, a smaller `L(w)` exists but pre-resolution issuance and
  ordinary bearer units remain bound to the universal scale.

The model's `ExactLotMarket` confirms exact refusal atomicity and that direct
burns only create conservative slack.

## 3. Persistent numerator credits

### 3.1 Transition algebra

Let one claimant already hold credit `r`, with `0 <= r < D`. Burning quantity
`q` of outcome `i` computes exactly:

```text
n       = r + q*w_i
paid    = floor(n/D)       whole collateral atoms only
r'      = n mod D
```

The claim supply falls by `q`, the Hoard and claimant collateral accounts move
by exactly `paid`, the claimant credit changes from `r` to `r'`, and the market
aggregate `K` changes by `r'-r`. Therefore:

```text
q*w_i = D*paid + r' - r

(R - q*w_i) + (K + r' - r)
  = R + K - D*paid.
```

Both sides of `D*C >= R+K` fall by exactly `D*paid`; existing slack is
preserved. The bounded exhaustive campaign checks every two-outcome simplex
through `D=16`, complete-set quantities through 12, both outcome orders, and
both internal/bearer labels. A deterministic 2,000-case campaign varies
denominator, weight, quantity, fragmentation, claimant slot, and reaggregation.

One credit can aggregate burns from every outcome because all native weights
share the same `D`. A credit is not outcome-specific. This is the central
advantage over lots: Split, transfers, and venue fills can remain in arbitrary
raw quantities.

### 3.2 Identity and transfer

Each credit binds exactly:

```text
CreditDomain = (Market, denominator D, settlement/credit generation)
CreditKey    = (claimant, CreditDomain)
```

The generation must be frozen from and checked against the immutable resolution
and credit-accounting era. It must not be a client timestamp. An ABI successor,
reopened identity, wrong denominator, or different Market cannot merge old
numerators.

Custom credit transfer is possible without making credits a Token-2022 mint:

1. authenticate the exact source key and source claimant authorization;
2. name the exact destination key;
3. require byte-equal Market/`D`/generation domains;
4. move an explicit numerator amount;
5. if destination aggregation crosses `D`, pay the resulting whole atom to the
   destination claimant in the same atomic transition; and
6. leave both source and destination canonical residues below `D`.

Thus a transfer changes the claimant field only through an explicit authorized
successor operation. A merge is transfer of the entire source residue. Existing
destination accounts must match their full key; an empty destination is created
under the explicitly supplied key. The model refuses wrong Market, denominator,
generation, claimant substitution, slot aliasing, zero transfer, and excess
credit before mutation.

Credits should not be freely minted as a second bearer token. That would merely
move the fractional-redemption problem into a recursive instrument, weaken
claimant identity, and add direct-burn/supply truth. Custom transfer is enough
to support voluntary aggregation and a secondary matching service without
turning it into protocol authority.

### 3.3 Required Solana state

Promoting credits requires at least:

- a fixed-layout credit PDA keyed by `(Market, claimant, generation)` carrying
  `D`, canonical numerator `<D`, bump/version/flags, and a sequence or receipt
  binding;
- one market-owned `credit_numerator_total` wide enough for admitted live
  credit accounts (`u128` in the model), updated by every credit mutation;
- a frozen maximum/account-lifecycle policy proving the aggregate cannot
  overflow and cannot be reconstructed by scanning claimants;
- rent paid explicitly by the claimant or an accepting transferee, never by
  Hoard principal and never by hoped-for future fees;
- replay protection for redemption, transfer, merge, close, and reopen;
- zero-only close with generation advance or a permanent tombstone, so a stale
  instruction cannot resurrect a prior numerator; and
- exact post-CPI checks for every external burn and collateral transfer.

Account creation is part of the atomic external-redemption transaction. If
credit creation or rent funding fails, no bearer Egg may remain burned. A
third-party transfer must not force rent or account state on an unconsenting
recipient: either the recipient signs/accepts, or the sender explicitly funds a
bounded destination under a frozen anti-grief policy.

The market-level aggregate is indispensable. The program cannot scan user
credits when testing solvency, withdrawal, or retirement. The model corrupts
that aggregate deliberately and refuses the state.

### 3.4 Internal and bearer implications

For an internal Position, redemption can use the owner's credit PDA (or a
future Position field), debit the local Egg balance, credit internal cash by
whole atoms, and update `K`. A Position field is smaller in account count but
makes credit transfer, close/reopen, and multi-Position aggregation depend on
Position generation. A separate credit PDA is more explicit and lets internal
and bearer paths share one liability owner.

For an external bearer Egg, fractional redemption is no longer positionless.
The claimant must present or create the credit PDA. In one Solana transaction:

1. authenticate Market, terms, immutable native Resolution, complete mint
   vector, Hoard, claimant source/destination, credit, aggregate credit ledger,
   and replay state;
2. synchronize prior direct bearer burns as forfeitures;
3. compute and validate the entire prospective state;
4. burn exactly `q` bearer Eggs with claimant authority;
5. transfer exactly `paid` collateral atoms from the Hoard (zero is allowed);
6. commit claim supply, Hoard accounting, credit numerator, aggregate `K`, and
   replay sequence; and
7. re-read exact token deltas.

Solana rollback is necessary but does not replace the prospective arithmetic or
post-CPI delta checks. The credit is owed even when `paid=0`; omitting the
credit account because no collateral moved is silent confiscation.

For a structured wrapper, the safest path remains unwrap then redeem native
Eggs. An optimized aggregate redemption may use the same claimant credit,
adding `q*dot(a,w)` to its numerator, only after the wrapper burn, native vault
debit, base SupplyLedger debit, payout, and credit update form one checked
transaction. Direct wrapper burns remain donations and create no credit.

## 4. Direct burns and donation surplus

An ordinary holder who directly burns a bearer Egg without invoking redemption
has forfeited that claim. Both policies do exactly this:

```text
T_i' = T_i - q
C'   = C
K'   = K
```

The invariant slack increases by exactly `q*w_i`. No claimant was paid, so no
credit is created. The retained collateral is conservative donation surplus,
not an operator withdrawal, fee, treasury balance, burn bounty, or final
rounding pot. Donation compaction may destroy corresponding liabilities under a
separately proved lifecycle, but may not transfer the Hoard surplus to a person.

## 5. The terminal sub-atom is a real impossibility boundary

After all native claims are gone, credits may remain. Write:

```text
K = D*A + r,  0 <= r < D.
```

`A` whole atoms are economically aggregatable if their owners voluntarily
transfer/merge credits. The residual `r` cannot be paid exactly in indivisible
collateral atoms:

- paying zero erases a live claim;
- paying one overpays one chosen claimant and consumes value not represented by
  the credit;
- sending it to treasury or a neutral sink confiscates it;
- distributing one atom pro rata recreates the same fractional problem; and
- crossing Markets or generations violates identity and lets one market depend
  on another's future use.

The honest no-subsidy terminal rule is therefore:

> Credit accounts and enough Hoard collateral remain live until same-domain
> aggregation makes whole atoms payable. If the global remainder stays below
> `D`, the final collateral atom and credits do not retire.

This is safe and exact, but it is not total economic closure. A total terminal
policy needs a separately capitalized rounding reserve with a predeclared fair
allocation rule, finer collateral units, or consent to explicit forfeiture.
None exists today, and Hoard principal cannot fund one. The model's
`terminal_facts` reports whole aggregatable atoms and the irreducible numerator;
it has no sweep operation by construction.

## 6. Smallest V1 runtime path

The smallest immediate runtime policy is:

1. keep the existing one-raw-claim Token-2022 atom and arbitrary transfers;
2. after resolution, derive and expose each exact `L(w_i)` from the immutable
   vector;
3. accept only quantities divisible by that outcome's lot; and
4. let holders transfer and aggregate fragments until one account presents an
   exact lot.

This requires no new persistent state and matches the kernel's current
exact-or-refuse arithmetic. It is terminally honest because it never creates a
credit it later erases. It is **not per-wallet exit-total**: an owner of a
sub-lot fragment needs voluntary aggregation, and an abandoned fragment can
keep the Market from retiring. The public terms and client must say that
plainly.

If V1 requires every ordinary bearer token atom to be independently
redeemable, the next-smallest zero-credit path is a **proved pre-resolution lot
profile**:

```text
L_i = D / gcd(D, every terms-reachable weight of outcome i)
L_common = lcm_i L_i                 (optional uniform bearer scale)
```

The compiler must establish the reachable family. `D` may be used as a
conservative common lot when the policy quantifies over the full integer
simplex, but it is not called minimal for a particular B-spline instance
without a gcd-1 reachability proof. Enforce `L_i` (or `L_common`) at every
claim-separating kernel and venue boundary listed in §2.2. A lot-scaled bearer
atom represents that many raw claims, so creating it normally requires the
same number of collateral-backed raw complete sets; the scaling is not a free
change of units.

Complete-set redemption remains exact at arbitrary quantity, but allowing an
arbitrary Split is safe only while its components cannot later be separated
below their frozen lots. The simple closure rule gates Split itself; a more
permissive balanced-bundle state would be a new object and is not the smallest
path.

This recommendation is narrower than saying credits are a bad design. Credits
are the correct experiment if arbitrary sub-lot raw fills are a product
requirement: they
preserve exact conservation and avoid leaking lot alignment into the batch
relation. But they also add a second transferable accounting object and have an
irreducible terminal state under indivisible collateral. That cost should not
enter the first native runtime accidentally.

Promotion gates for the lot path are still substantial:

1. freeze either recombinable post-resolution lots or a proved
   pre-resolution raw-to-bearer quantity mapping in terms and SDK fixtures;
2. prove every Split/Merge/transfer/reservation/fill/materialize/dematerialize
   path preserves the quantum or a balanced complete set;
3. add internal and real Token-2022 SBF tests for minimum lot, multiple lots,
   sub-lot refusal, transfer, direct burn donation, and terminal withdrawal;
4. bind every redemption to the immutable native Resolution vector and `D`;
5. differential-test kernel, reference, and SBF refusal atomicity; and
6. measure transaction/account/compute impact before changing current truth.

Until those gates close, exact fractional native redemption remains **STOP**.

## 7. Executed evidence

The standalone crate currently has 13 host tests:

- exhaustive resolved and reachable-family gcd lot minimality through `D=24`;
- exhaustive structured universal-lot minimality over small integer simplexes;
- exact-lot refusal atomicity and donation-only direct burns;
- mixed-outcome internal/bearer credit aggregation;
- exact-domain transfer/merge and hostile identity refusal;
- one-shot versus fragmented redemption equivalence;
- exhaustive credit conservation for all two-outcome simplexes through `D=16`,
  quantities through 12, and both redemption orders;
- explicit terminal sub-atom retention;
- market aggregate corruption refusal;
- refusal of two persisted slots claiming the same full credit key;
- exact slack increase under direct burns;
- hostile arithmetic boundary checks; and
- a deterministic 2,000-case fragmentation/reaggregation conservation
  campaign.

This is **HOST-TESTED model evidence**, not a proof, SBF execution, audit,
deployment, or source-to-runtime refinement result.
