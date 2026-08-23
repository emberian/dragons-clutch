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

## Economic exclusions preserved at integration

The adapter must refuse dynamic depth, negative inventory, margin, borrowing,
loss mutualization, sponsor substitution, facility cross-netting, a second
resolution owner, or any attempt to count Hoard principal or anticipated fees
as sponsor capital. Multiple facilities compose only as independently solvent
sponsors. Realized fee or spread revenue may be paid under a separately frozen
policy after it exists; it is never part of this model's solvency or liveness
proof.
