# Exact fractional redemption for native B-spline Eggs

Status: **RUNTIME CONTRACT PROMOTED / COMPLETE ROUTES STAGED-DISABLED** (2026-08-23).
`crates/clutch-fractional-redemption-runtime` now owns the safe `no_std`,
no-allocation, fixed-layout transition and account contract. Intent family
79/v1 and current accounts `0xa4/v2`, `0xa5/v1`, `0xa6/v2`, and `0xa7/v2`
remain `ReservedDisabled`; the complete exact-internal, exact-bearer, and
claims-exhausted handlers are present, but no release capability is enabled.
`research/fractional-redemption` remains the
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

Resolution V5 supplies the full-width immutable Market and NativeClaimBasis,
common `D`, exact vector, generation, body semantic ID, and PDA-bound data ID.
The runtime persists only the data-ID reference and reconstructs the vector
from the authenticated V5 body on every transition. Its returned payout
projection binds the exact outcome and burn quantity to `q*w = D*whole + r`;
the direct route exact-refuses `r != 0`, while the credited Fractional route
retains `r` atomically rather than flooring it or making it a permanent amount
restriction.

The canonical persisted schemas are policy `0xa4/v2`, aggregate ledger
`0xa5/v1`, owner credit `0xa6/v2`, and credit tombstone `0xa7/v2`. The
never-activated V1 policy, credit, and tombstone coordinates are explicitly
withdrawn because their corresponding identity slots meant payout-vector
digests. Current decoders refuse those versions; V2 uses fresh policy/credit
PDA domains and a fresh policy-state identity domain. The unchanged aggregate
ledger had no reinterpreted field and remains `0xa5/v1`.

ClaimLedger V3 no longer predicts those accounts before Resolution exists. It
is founded with an explicit fractional `OpenUnlatched` state, zero a4/a5
identities, sequence zero, and a zero latch. Resolution activation changes only
the liability lifecycle and Resolution account; it preserves that fractional
state. Fractional Initialize alone performs the one-way `OpenUnlatched →
Latched` transition after the exact Resolution V5 data identity is known,
stores the canonical a4/v2 and a5/v1 accounts, advances sequence zero to one,
and emits the Product five-family admission receipt. Mixed zero/live identity
encodings, fractional activity before the latch, and every relatch refuse.

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

For an internal Position, the promoted runtime uses the owner's credit PDA,
debits the canonical Position V3 Egg balance, credits internal cash by whole
atoms, reclassifies the same amount of Hoard V2 locked principal into cash
liability, and updates `K`. The separate credit PDA lets internal and bearer
paths share one liability owner without adding another Position field.

For an external bearer Egg, fractional redemption is no longer positionless.
The claimant must present or create the credit PDA. In one Solana transaction:

1. authenticate Market, terms, immutable native Resolution, complete mint
   vector, Hoard, claimant source/destination, credit, aggregate credit ledger,
   and replay state;
2. bind canonical ClaimLedger V3 materialized supply, synchronize it downward
   to the full authenticated Token-2022 mint vector for prior direct holder
   burns, and refuse any observed increase or inactive-outcome supply;
3. compute and validate the entire prospective state;
4. burn exactly `q` bearer Eggs with claimant authority and accept the exact
   selected mint/source deltas before exposing a collateral request;
5. externally transfer exactly `paid` collateral atoms from the Hoard under an
   accepted release-selected receipt (zero emits no CPI);
6. commit claim supply, Hoard accounting, credit numerator, aggregate `K`, and
   replay sequence; and
7. re-read exact token deltas.

Solana rollback is necessary but does not replace the prospective arithmetic or
post-CPI delta checks. The credit is owed even when `paid=0`; omitting the
credit account because no collateral moved is silent confiscation.

For a structured wrapper, the safest path remains unwrap then redeem native
Eggs. An optimized aggregate redemption may use the same claimant credit,
adding `q*dot(a,w)` to its numerator, only after the wrapper burn, native vault
debit, canonical ClaimLedger V3 debit, Hoard V2 reclassification/withdrawal,
payout, and credit update form one checked
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

When every claim, credit, and locked claim-principal atom is exactly zero, the
runtime prepares one terminal family close for immutable policy `0xa4` and
aggregate ledger `0xa5`. ClaimLedger first commits the exact transient `0xa5`
retirement-state identity. A private Product five-family aggregator authorization must
bind the Market, generation, both physical accounts, both terminal state IDs,
and the ClaimLedger retirement transition before either account is deleted.
Each account refunds only its own stored rent payer; hostile or unsolicited
lamports go to the frozen neutral sink. Fractional projects the terminal
receipt, committing both physical accounts and terminal state IDs, the
ClaimLedger post/transition IDs, both exact payer/neutral rent splits, and a
separately adapter-authenticated Fractional runtime/capability release ID. Its
crate-private SBF postwrite capability hostile-decodes the exact writable
a4/a5/ClaimLedger bodies, authenticates all three PDAs, rechecks both observed
rent balances, and derives the release only from a loader-authenticated
registry capability narrowed to action 10. Product consumes that private value;
it cannot construct a substitute receipt, invent a release, or reuse the Realm
collateral release retained by the policy. The executable route remains
disabled only until Product lands its stable atomic aggregator/root consumer.

## 6. Selected runtime contract and activation boundary

The promoted runtime deliberately supports both useful paths:

1. an exact redemption path that creates no credit and reports whether the
   resolved common-lot fast path was used; and
2. an arbitrary-quantity path that carries the exact residue in one
   owner-scoped credit and updates the sole aggregate `K` owner atomically.

This selection preserves arbitrary raw-claim trading and does not force a lot
quantum into Split, transfer, reservation, fill, Materialize, or
Dematerialize. Credits are custom same-domain accounting objects, not a second
Token-2022 instrument. Their transfer and merge routes require explicit source
authority and destination acceptance and pay every newly aggregated whole atom
in the same transition.

A zero-credit deployment profile remains possible: it can expose only the
exact actions and use either resolved lots or a separately proved
pre-resolution lot profile. That is a capability/product choice, not a second
arithmetic truth. The runtime does not claim that a particular reachable
B-spline family has a smaller universal lot without the corresponding gcd
evidence.

The exact-internal action-2, exact-bearer action-3, and claims-exhausted
action-9 SBF handlers are present but capability-disabled until Product lands
the stable five-family admission producer needed by action 1. The Fractional
side already exposes a crate-private postwrite capability that proves exact
a4/a5/ClaimLedger founding bodies and PDAs and carries the full admission
receipt, including the canonical claim-issuance binding. Product Foundation
remains the sole owner of the slot-11/12 debit and preallocation evidence: the
prestates are prefunded zero-data System-owned writable PDAs, and Fractional
Initialize must allocate/assign/write them without a second debit or refund.
Action 3 already composes the real Token-2022
burn adapter and Realm-selected collateral CPI, orders burn acceptance before
collateral request exposure, and writes `0xa5`, ClaimLedger V3, and Hoard V2
atomically. Action 9 advances only `0xa5` and ClaimLedger after exact canonical
supply reaches zero; it requires neither a signer nor bearer-release authority.
The remaining live-route blocker is Product's stable private per-slot
preallocation authority plus its atomic admission/terminal consumers and final
account order. Release-profile admission and local-bank adversarial execution
remain open. Family 79/v1 stays `ReservedDisabled` until those boundaries are
integrated and reviewed.

## 7. Evidence and intentionally deferred validation

The standalone research model's prior promotion evidence includes 13 host
tests:

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

The promoted runtime contract also carries authored adversarial tests.
This tranche adds exact two-phase bearer-burn binding, prior direct-burn supply
synchronization, hostile observed-supply refusal, full post-burn vector
refusal, dynamic account-geometry checks, release-bound terminal/rent receipt
checks, exact founding-postwrite verification, and substituted physical/latch
refusal. Per the implementation-cycle instruction, none of those tests, the
build, a formatter, or a validator was run for this tranche; validation is
deliberately deferred rather than implied by their presence.
