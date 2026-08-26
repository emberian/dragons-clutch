# Dealer V2: exact finite-scenario collateral

Status: executable semantic/kernel sketch. This is not an accepted Trading
content profile, deployment, release, or claim that the Solana composition is
complete.

## Product boundary

Dealer V1 quotes individual native outcome claims from covered inventory. V2
generalizes the same solvency discipline to atomic portfolio legs across one
finite Product domain. It does not introduce signed native Positions, margin
debt, credit creation, or another persisted liability vector.

For each terminal scenario `s`, the adapter projects from canonical Claims:

- `inventory[s]`: native claims already held by the Dealer Position;
- `acquired[s]`: nonnegative claims transferred to the Dealer in this atomic
  fill; and
- `delivered[s]`: nonnegative claims transferred from the Dealer.

The exact new complete-set reserve is:

```text
reserve = max_s(delivered[s] - (inventory[s] + acquired[s]), 0)
```

The Dealer must fund `reserve` from present TradingPrincipal custody. Custody
moves it into Market Hoard principal and Claims mints exactly `reserve` equal
complete sets into the Dealer Position. After all nonnegative portfolio
transfers, the Dealer may merge at most:

```text
mergeable = min_s(inventory[s] + acquired[s] + reserve - delivered[s])
```

equal complete sets. Claims burns them and Custody returns the same Hoard
principal to Dealer TradingPrincipal. The executable kernel plans these
amounts with two checked passes and leaves its output untouched on refusal.

This is scenario collateral in the literal sense: one collateral atom covers
one atom in whichever single terminal scenario occurs. It is not statistical
margin and it never counts fees, expected order flow, LP capital not yet in
custody, or future liquidation proceeds.

## Sole owners and physical order

Claims remains the sole owner of aggregate supply, native/materialized supply,
and holder balances. Market Hoard remains the sole collateral backing those
claims. Dealer state must not persist an inventory mirror; every fill projects
the exact Dealer Claims Position and rejoins the Product width, Market,
release set, revision, and holder identity.

The Rust kernel therefore accepts an ephemeral Claims Position observation,
not a stored Dealer vector. It rejects a substituted Market, substituted
holder, stale optimistic revision, or empty width before reserve arithmetic and
before touching its caller-owned output.

An admitted atomic composition is ordered:

1. authenticate the Trading child root, descriptor/config, Core Market, Claims
   aggregate and Positions, Realm, Custody vaults, revisions, and actor consent;
2. calculate the complete plan without writes;
3. move present reserve to Hoard and mint equal complete sets when nonzero;
4. execute nonnegative incoming and outgoing Claims basket transfers;
5. merge only the requested equal residual set and return exactly that Hoard
   principal when nonzero;
6. move the separately priced quote leg, cumulative fee, and funded work
   reward through their distinct Custody compartments; and
7. verify every child receipt and physical delta, then commit Dealer counters
   last.

No partial step is a valid state. A late child refusal relies on SVM
transaction-wide rollback and needs byte-for-byte real-ELF evidence.

## Epoch transition

Quote curves and risk policy are immutable per epoch. Every order binds the
Trading child root, Candidate/epoch identity, and revision. A successor epoch
may activate only when:

- it was precommitted with a higher revision and present liveness funding;
- the old fill window is closed, so stale signed orders fail their exact epoch
  coordinate;
- any durable reservation count is zero;
- the canonical Dealer Claims inventory satisfies the successor bounds; and
- every persisted owner opt-in required for capital migration is present.

Activation resets curve-usage coordinates but never rewrites Claims balances
or moves Hoard principal implicitly. Nonconsenting capital remains in the old
epoch and exits through its existing claims; there is no forced rollover.

## LP loss tranches

LP tranches are not part of the smallest safe kernel. Full claim
collateralization prevents claimant insolvency, but Dealer capital can still
lose value through adverse selection. A future tranche therefore represents a
waterfall over residual Dealer-owned quote and Claims assets after protocol
fees and funded work—not a claim on Market Hoard principal and not a guarantee
of par.

Issuing such shares requires one canonical residual-asset share contract with
explicit senior/junior loss allocation, mint/burn supply, epoch consent, and
terminal redemption. Until that owner exists, the protocol must not simulate
tranches with Dealer counters, offchain bookkeeping, fee promises, or a second
Claims truth.

## Implemented and remaining

Implemented here:

- Lean definitions and general theorems for maximum-shortfall coverage,
  per-scenario conservation, netting versus gross reserve, and safe merge;
- executable funded, underfunded, and over-merge cases; and
- a safe `no_std`, no-allocation, runtime-width Rust planner with hostile width,
  funding, overflow, release, and mutation-atomicity tests.

Remaining before V2 is executable protocol behavior:

- canonical capability descriptor, config/request/root/account/derivation/
  capacity/effect schema identities;
- Claims basket mint/transfer/merge child requests and receipts in the one
  Claims authority;
- Trading register projection from canonical Claims rather than Dealer state;
- cumulative portfolio quote/fee rules and bounded work rewards;
- epoch activation and optional residual-asset share semantics; and
- real-ELF success, substitution, overflow, late-CPI rollback, rent, and CU
  evidence through the one canonical Trading Program.
