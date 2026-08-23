# Exact model and integration boundary

This crate is an offline semantic model. It owns no Solana account, Token-2022
mint or balance, CPI, clock, source, resolution authority, auction page, solver,
or cryptographic identity. No current runtime instruction accepts these structs.

## Facts owned by this model

- immutable depth, capacity, schedule, native claim-domain, and sponsor binding;
- the admitted nonnegative-price inventory domain;
- the canonical integer endpoint potential and rational diagnostic prices;
- exact minimum sponsor capitalization;
- facility-attributed external inventory, retained complement Eggs, free cash,
  and complete sets in Hoard;
- one aggregate transition recipe with exact cash, Egg, split, and merge flow;
- two-sided, buyback-only, resolved, and retired lifecycle arithmetic; and
- exact terminal payout accounting for the conservative universal lot profile.

The model does not own the Terms, payout basis, actual Hoard, actual token
supply, call-auction candidate, or resolved source result. It binds identities
that a live adapter must authenticate against their existing semantic owners.

The `signed_dealer` extension has a distinct custody boundary. Its LPs deposit
existing native Eggs already backed by the global Hoard. Deposits and trades do
not create facility-attributed Hoard backing and must never increment a supply
or Hoard mirror. Resolution redeems only the exact custodied Egg balances
through the ordinary claim path.

## Existing semantic owners to reuse

| Model fact | Existing or planned owner |
| --- | --- |
| native basis, outcome count, payout denominator, Terms digest | canonical `TermsAccount` / future compact Instance |
| Market and claim-domain identity | canonical Market plus native wrapper-domain rules |
| actual Egg supplies and Hoard balance | Token-2022 accounts, SupplyLedger, Hoard mirror, and kernel |
| coefficient intent and reservation | `PortfolioRecord` and reservation accounts |
| best valid submitted candidate | the versioned `clutch-batch` relation and candidate lifecycle |
| selected exact native flow | accepted candidate receipt/entitlement state |
| terminal payout weights | authenticated resolved Kernel/Terms state |
| token movement | minimal SBF adapter with exact post-CPI reloads |
| recurring funding | immutable Series plan and segregated SeriesFunding compartments |

A live implementation should add the smallest facility policy/state codec and
translate into those owners. It must not create a second persisted Terms,
Hoard, Position, Portfolio, wrapper, resolution, or auction truth.

## Required atomic call-auction transition

For each facility touched by one candidate, the verifier must aggregate all of
that facility's fills into one canonical native `sell_to_users` and
`buy_from_users` vector. It then recomputes one receipt from the authenticated
pre-state and binds:

1. policy, facility, Market, Terms, Instance, claim domain, phase, slot, and
   pre-generation;
2. exact aggregate native Egg debits and credits;
3. exact aggregate trader collateral in or out from the endpoint potential;
4. exact complete-set split or merge and facility cash change;
5. every affected reservation and entitlement; and
6. the post-generation and all post-CPI token balances.

Pricing each partial fill separately or in solver-selected order is forbidden.
Although endpoint charges telescope, one aggregate transition is the simplest
way to prevent ambiguous allocation, replay, and inconsistent intermediate
capacity. The existing auction's other cash and fee conservation equations
must include the facility receipt exactly. The facility does not certify the
candidate as best, globally optimal, or even included.

For the signed dealer, one candidate also supplies each user order's exact
dealer-leg cash allocation in strictly increasing immutable order identity.
The pure model checks a frozen dealer-leg envelope excluding fees. The live
verifier must derive and authenticate that envelope from every all-in order
limit after exact fees or rebates, require the net signed sum against the
facility to equal the one aggregate endpoint receipt, and close the existing
total cash/fee equations. The endpoint alone does not determine a unique
per-user split. A net-zero same-outcome flow must be removed before the
facility leg rather than used to manufacture volume.

## Promotion gates

Before a live route exists, all of the following must close:

1. **Canonical bytes and identity.** Freeze policy/state codecs, derive the
   policy and facility IDs cryptographically, authenticate all copied
   bindings, and reject unknown versions or trailing bytes.
2. **Custody separation.** Facility sponsor cash needs a program-owned vault or
   exact Position compartment distinct from claimant Hoard principal, batch
   fee pot, prepaid work/liveness, rent bonds, treasury, and other facilities.
3. **Physical retained Eggs.** Bind one canonical token account or exact
   internal balance per native Egg. A Split must debit facility cash and credit
   Hoard exactly; a Merge must burn/consume the complete set and credit facility
   cash exactly. Reload every account after CPI.
4. **Supply and mirror refinement.** Prove the receipt refines Token-2022 mint
   supply, SupplyLedger, kernel totals, Hoard locked backing, actual Hoard token
   balance, and any Position conservation identity, including unsolicited
   surplus handling.
5. **Auction integration.** Version the nonlinear facility leg in the candidate
   relation, define its exact ordering and duplicate rules, aggregate it once
   per facility, and prove cash allocation and rollback with portfolios,
   partial fills, fees, and virtual legs. Until then this facility has no live
   executable quote.
6. **Replay and authority.** Consume the exact pre-generation. Authenticate the
   sponsor only for early halt and withdrawal; close and resolution progress
   must not depend on sponsor availability.
7. **Wrapper translation.** Admit only canonical wrappers whose complete native
   coefficient vector and claim-domain digest are authenticated. The facility
   trades native Eggs; it neither mints wrapper liabilities nor accepts a UI
   label as a position.
8. **Resolution.** Bind the exact authenticated payout vector and atomically
   redeem/burn retained Eggs. Preserve the recorded external payout as Hoard
   backing until external users redeem their own Eggs. That backing is a
   conservation check and claimant principal, not a facility-owned asset.
9. **Fractional policy.** V1 uses full-denominator lots. A smaller-lot successor
   must reuse or extend the protocol's one semantic fractional-credit owner and
   prove carry conservation, rather than flooring terminal payout.
10. **Funded progress.** Prepay close, source, resolution, redemption, account
    retirement, and rent work from named compartments. Sponsor capital and
    Hoard principal are not keeper funds. Expected fee revenue is not a present
    liveness balance.
11. **Resource evidence.** Measure serialized widths, rent, CU, stack, and
    transaction account limits; test blank-bank construction, signed
    transactions, hostile account substitution, post-CPI rollback, and every
    frozen maximum.
12. **Independent proof.** If promoted into Eggcrate, reimplement under its
    kernel policy, add independent vectors, and prove/refine the loss,
    conservation, transition-totality, and lifecycle theorems. Do not call this
    model formally verified.

The signed dealer adds these gates:

13. **Existing-Egg deposits.** Authenticate every LP Egg mint, owner, amount,
    Terms/Instance binding, transfer profile, and post-transfer vault balance.
    Reject fee-bearing, opaque, frozen, delegated, or wrong-domain assets.
14. **Share roster.** Freeze the exhaustive unique-owner set at activation,
    prevent share mutation thereafter, bind queue votes to those balances, and
    implement the exact terminal allocation once. Wallet identity is not a
    proved beneficial-owner anti-Sybil rule.
15. **Expense separation.** Charge all rent, keeper, transfer, fee, and
    resolution costs outside the guaranteed LP assets, or capitalize a separate
    immutable worst-case expense compartment. Neither `K` nor future revenue
    silently pays them.
16. **Signed receipt integration.** Aggregate native flow once per dealer and
    generation; check physical vault conservation, derive dealer-leg envelopes
    from authenticated all-in limits and fees, and atomically apply the receipt
    with all user legs.
17. **Terminal retirement.** Join every LP claim, Egg vault, cash vault,
    generation, and immutable terminal allocation into the counted-retirement
    authority before reclaiming dealer state or rent.

The pure API deliberately distinguishes the mathematical first-loss minimum
from the actual sponsor cash minimum. The latter also finances the all-buy
lower corner after minimum LP cash. A live initializer must transfer and reload
that actual amount atomically; a computed requirement is not evidence of funds.

## Economic exclusions preserved at integration

The issuance adapter must refuse dynamic depth, margin, borrowing,
loss mutualization, sponsor substitution, facility cross-netting, a second
resolution owner, or any attempt to count Hoard principal or anticipated fees
as sponsor capital. Multiple facilities compose only as independently solvent
sponsors. Realized fee or spread revenue may be paid under a separately frozen
policy after it exists; it is never part of this model's solvency or liveness
proof.

The signed dealer admits negative *net-sold flow* only against present cash and
authenticated long Egg custody. That is not authority for a negative token
balance, short borrow, cross-facility netting, or margin.
