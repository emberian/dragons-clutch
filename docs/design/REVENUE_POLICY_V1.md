# RevenuePolicy V1 — the authenticated fee destination

Status: **DESIGN-ONLY, with its six queued decisions returned 2026-08-20** —
still no runtime code, no ABI change, no gate relaxed, and no fee chargeable by
this document. Ember's adoption record
([../decisions/ADOPTED_2026-08-20.md](../decisions/ADOPTED_2026-08-20.md)
item 8, with item 6 for Plane L, on
[../decisions/REPORT_revenue-policy-v1_2026-08-20.md](../decisions/REPORT_revenue-policy-v1_2026-08-20.md))
closes §11's queue as follows; the design below is **unedited**:

| §11 queue item | section | outcome |
| --- | --- | --- |
| 1. treasury key | §3, §5 | **DECIDED** (B4a): custody requirements adopted; the **pubkey itself is DEFERRED to the first fee-bearing Realm** and stays reserved to ember |
| 2. Plane C shape | §5 | **DECIDED** (B4b): treasury Position (D6), with the mid-epoch-close grief rider joining the hostile walk (§10) |
| 3. Plane L disposition | §4 | **DECIDED** (item 6 / B4c): the five ResolutionWork charges are a **permanent zero as frozen policy**; L1-over-L0 is the disposition of record for any future nonzero lamport charge, and **neither vault nor record is built now** |
| 4. sequencing | §2, §4-§5 | **DECIDED** (B4d): per B4c |
| 5. V1 split vector | §7 | **DECIDED** (B4e): 60/0/40 + `AllRestingMakers`; constrains nothing until a fee-bearing Realm exists |
| 6. terminal classification | §3, §4 | **DECIDED** (B4f): both Realm-lifetime rows accepted, under item 7's R4 ratification |

Still **not decided** here and unchanged by that record: the fee **rate** (the
base *shape* is selected elsewhere — [FEE_GEOMETRY.md](../FEE_GEOMETRY.md) —
with both rates open), promotion, and real-money activation. Every §9 gate
stays closed and the §10 falsifiers stay owed.

**Boundary landing, 2026-08-21** (the fee-plumbing lane; SBF-EXECUTED on a
bank, unpromoted, no gate relaxed): the §3 policy object exists as
`research/batch-policy-identity/src/revenue_policy_v1.rs` (`REVENUE_POLICY_V1`
— 60/0/40 + AllRestingMakers, treasury pinned to a structural UNSET sentinel
per B4a, digest + `validate()` envelope refusals + split arithmetic); the
per-Realm `RevenuePolicyRecordV1` family exists (layout tag 27, the design's
seed, TerminalIdentityV1 header embedded, mandatory funding-ledger sibling),
creatable **only inside `InitRealm`** — the record's absence IS the D4
zero-take state — with `CloseRevenuePolicyRecord` (tag 68) keeping close
admissible behind the Realm's absence; the fee-bearing sibling const
`GENERAL_CLEARING_FEE_SHAPE_V1` exists (composite shape, **both rates
explicit zeros**; any nonzero rate is a new const + digest + ember decision);
and general epoch admission enforces the §5 seam — a fee-bearing epoch
refuses `RevenuePolicyRecordMissing` / `RevenueTreasuryUnset` today, always,
because the treasury byte stays deferred. The B4b grief rider has a host
kernel (`TreasuryServiceLedger` in `crates/clutch-liveness`), while the older
`EnvelopedIntentFeeCarry` is arithmetic evidence, not the selected composite
carry owner. The 2026-08-23 successor contract makes `(fee record, owner)` the
§6 owner and deliberately chooses no byte host yet. The reservation format is
unbumped. The §10 falsifiers run at zero rates only; every nonzero-rate
obligation stays owed.

Written
2026-08-20 for ember's morning review, executing step 3 of
`docs/reviews/FEE_ECONOMICS_FINDINGS_2026-08-19.md` §6: *decide the
destination before the base*. The claim vocabulary of `CURRENT_TRUTH.md` §1
governs; everything below is PROPOSED and nothing below promotes any surface.

This document deliberately does **not** select a fee base, a rate, or a
promotion path. Those remain queued (§11). It designs the object that must
exist before any of them can matter: `RevenuePolicy`, named as an
architectural boundary in `docs/ECONOMICS.md:206-208`,
`docs/DEPLOYMENT_REVENUE_BOUNDARY.md:58`, `docs/ENGINEERING_PLAN.md:145`, and
scored at zero lines of code by the findings review (§4.7).

## 1. Governing constraints, inherited not invented

From `docs/DEPLOYMENT_REVENUE_BOUNDARY.md` §3 (`:52-71`):

- a Realm selects an **immutable audited `RevenuePolicy` from a closed set**;
- every sink is **outside Hoard principal and prepaid liveness**;
- a policy must name its recipients, caps, and withdrawal conditions;
- a Realm **cannot silently redirect the fees of an already active Market**.

From `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md` §1(10) and §3(15): the
incinerator is the one frozen neutral sink, and **surplus burns**. That rule
is about *surplus*. Revenue is the opposite classification:

- **(D1) Revenue is an owed compartment with a named owner, never a surplus
  compartment.** The terminal design's own falsifier (§1(10): a burn that
  destroys an *owed* balance proves misclassification) is exactly why fee
  atoms may never ride the donation/forfeiture compartments or the
  `terminal_split` burn. A fee with no owed ledger row is a donation, and the
  program already has a place for donations — the incinerator. What does not
  exist, and what this document designs, is the owed row.

From `programs/solana-layout/src/lib.rs:1017`: Hoard `collateral_atoms` "is
not a fee or liveness balance" — the destination must not be a Hoard field.

From `docs/DEPLOYMENT_REVENUE_BOUNDARY.md` §2/§5: charging real-money fees is
a Track question with a legal perimeter. This design makes a fee *possible*
in the byte plane; it authorizes no deployment, operation, or real-money
activation.

## 2. Two denominations, two planes

The findings review (§6.3) is explicit that the closest live seam —
ResolutionWork's five charge fields — "is a rent/SOL denomination, distinct
from the collateral-atom fee the geometry describes." This design covers
both, as separate planes with separate destinations, and sequences them:

- **Plane L (lamports).** Protocol charges on SOL-denominated service flows:
  today exactly the five `ResolutionWorkCostScheduleV1` charge fields, all
  hardcoded zero with the stated reason *"Every protocol charge is zero
  because V1 has no authenticated fee sink"*
  (`programs/clutch-sbf/program/src/instructions/resolution_work.rs:357`,
  fields at `:370-377`).
- **Plane C (collateral atoms).** The trading fee riding the owner-signed
  `max_fee_atoms` envelope (intent v3 binds it exactly:
  `programs/clutch-sbf/program/src/instructions/orders_batch.rs:302-307`;
  buyers already reserve consideration **plus** fee and sellers withhold from
  proceeds: `programs/solana-layout/src/reservation.rs:85,111`), forced to
  zero at the five gates of §9.

**(D2) Sequence Plane L first.** Plane L needs no candidate ABI change, no
relation change, no carry, and no fee base — only a versioned cost schedule
and a destination account. Plane C needs everything in §5-§8. Landing L first
converts "no authenticated fee sink" from a universal blocker into a solved
precedent the C plane reuses, without waiting on the fee-base fork. The
planes never mix: no conversion rate between lamports and collateral atoms is
assumed anywhere (consistent with `docs/ECONOMICS.md:160-161`).

## 3. The policy object: `RevenuePolicyV1`

**(D3) The policy is a frozen const plus digest, pinned per Realm at
creation, immutable forever after.** The precedent is exact:
`FrozenPolicyV1` consts with `batch_policy_digest` identity —
`DIRECT_POLICY_V1` pinned at epoch open
(`programs/clutch-sbf/program/src/instructions/direct_selection_v3.rs:320`)
and the sibling-const-plus-new-digest pattern of
`research/batch-policy-identity/src/general_clearing_v1.rs:70`. A future
revenue profile is a sibling const; an existing Realm's pin never moves.

Proposed const shape (authored beside `FrozenPolicyV1`'s family, digested by
the same machinery):

```text
RevenuePolicyV1 {
    version:            u32,
    treasury:           [u8; 32],   sole authenticated revenue recipient
    maker_rebate_num:   u32,        V1: 60
    executor_num:       u32,        V1: 0  (deferred, see section 7)
    treasury_num:       u32,        V1: 40
    split_den:          u32,        V1: 100
    residual:           enum        V1: Treasury (frozen residual-atom rule)
    standing_maker:     enum        V1: AllRestingMakers (section 7)
    lamport_sink:       enum        V1: RevenueVault (section 4)
}
```

`validate()` refuses: share numerators not summing to `split_den`;
`executor_num * 100 > 15 * split_den`; `treasury_num * 100 < 25 * split_den`
(the published envelope, `docs/ECONOMICS.md:146-148`, becomes a structural
refusal, not prose); `treasury` equal to the incinerator or to zero bytes
(an unowned owed compartment is the §1(D1) misclassification). A distinct
`ZERO_REVENUE_POLICY_V1` (zero take, no recipient) is the closed set's
trivial member and is byte-for-byte what every existing Realm already
behaves as.

**Per-Realm record.** New account family `RevenuePolicyRecordV1`:

```text
seeds:  [b"dragons-clutch:revenue-policy:v1", realm]    (seeds.rs convention, :48-127)
realm:            [u8; 32]
policy_digest:    [u8; 32]   digest of the frozen RevenuePolicyV1 const
treasury:         [u8; 32]   copied out for account-list identity checks
terminal:         TerminalIdentityV1 header (payer, payer_principal,
                             donation_floor, generation)
stored_bump:      u8
```

- **Created and funded by the Realm creator, in the Realm-creation flow.**
  New Realms name their policy at birth; **(D4) existing Realms are
  zero-take forever** — there is no retrofit instruction, because any
  retrofit authority is exactly the "silently redirect" surface the boundary
  doc forbids.
- **Terminal disposition from day one:** the record carries the
  `TerminalIdentityV1` header (`TERMINAL_LIFECYCLE_RUNTIME_V1.md` §1), bound
  one per Realm, classified beside the Realm row it serves. It is the one
  proposed row whose lifetime equals the Realm's; the header keeps close
  admissible (principal to the stored payer, surplus burned) rather than
  unrepresentable, so the classification can tighten later without an ABI
  change. `research/liveness-policy-profile/terminal_profile.py` gains this
  row (and §4's vault row) before any implementation lane starts — unplanned
  permanent rent is exactly what the DIRECT.EPOCH_RECEIPT_RENT_PERSISTS /
  POLICY_ARTIFACT_RENT_PERSISTS blockers (`terminal_profile.py:100-116`)
  look like after the fact.

## 4. Plane L: the lamport destination

**Decided 2026-08-20 (B4c,
[../decisions/ADOPTED_2026-08-20.md](../decisions/ADOPTED_2026-08-20.md)
item 6): all five ResolutionWork charges are permanently zero as frozen
policy — zero as policy, not as placeholder. No vault is built.** L1 below is
ratified as the disposition *of record* for any future nonzero lamport charge
on an optional service flow, and the `lamport_sink` member of the §3 const
stays, documented as reserved. The rationale of record is the protected-pools
row plus the anti-liveness argument — not the source comment's "V1 has no
authenticated fee sink", which stops being true the moment any sink exists.
This is the **weak form** of permanence: a `RESOLUTION_WORK_COST_VERSION_V2`
sibling const (§4.1 below) may introduce a charge for **new** Works without
breaking any in-flight promise, because Begin freezes the schedule digest per
Work. The two dispositions below therefore read as the design record behind
that choice, not as an open fork.

Two admissible dispositions for a nonzero protocol charge, one recommended:

- **L0 — burn.** The charge compartment already reconciles into the terminal
  neutral total (`resolution_work.rs:1288-1296` folds
  `donation + charges_paid + charge` into the terminal arithmetic), and the
  incinerator is frozen program-wide
  (`direct_selection_v3.rs:62`). Burning charges needs almost nothing — but
  it is not revenue: it funds no maintainer, and `docs/ECONOMICS.md` §6's
  break-even inequality stays `unbounded` exactly as the findings review
  scored it (§4.3). L0 is a deterrent policy, not a RevenuePolicy.
- **L1 — per-Realm `RevenueVaultV1` (recommended, (D5)).**

```text
seeds:  [b"dragons-clutch:revenue-vault:v1", realm]
realm:            [u8; 32]
swept_lamports:   u64        monotone counter of lamports paid out
terminal:         TerminalIdentityV1 header
stored_bump:      u8
```

- **Created and funded by the Realm creator** in the same flow as the §3
  record; exists only for Realms whose policy has a nonzero lamport sink.
- **Flow:** charges move prepaid-budget → vault at transition time. The
  funding ledger already tracks `charges_paid`
  (`resolution_work.rs:827,1292`); the terminal reconciliation must be
  re-derived for lamports that leave the account mid-life — that derivation
  is an implementation obligation with its own conservation test (§10), not
  something this document hand-waves.
- **Withdrawal:** permissionless sweep of everything above the rent floor to
  the `treasury` key read from the Realm's `RevenuePolicyRecordV1` — the
  recipient is authenticated by identity against the record, never by who
  cranks. Nobody's signature moves revenue anywhere else. No admin
  instruction exists; recipient rotation is representable only as a program
  upgrade, i.e. not representable in this immutable deployment.
- **Terminal disposition:** sweep-empty, then the header close (principal to
  payer, residue burned). Bound: one per Realm.

**What relaxing the SOL-plane gates requires** (these are gates in their own
right, kept out of §9's count because they are not `max_fee_atoms` gates):

1. `validate_release_cost_shape` pins all five charges to zero
   (`resolution_work.rs:796-812`) — a nonzero schedule is a
   `RESOLUTION_WORK_COST_VERSION_V2` sibling const with its own digest;
   Begin freezes the schedule digest per Work (`:246-250`), so in-flight
   Works keep the schedule they were sold — the no-silent-redirect rule,
   enforced by existing bytes.
2. The three transition refusals `require(charge == 0, …)`
   (`resolution_work.rs:997`, `:1282`, `:1489`) become exact vault credits,
   and the vault PDA joins the Fold/Finalize/Abort account lists.
3. The vault and record rows land in the terminal inventory first (§3).

## 5. Plane C: the collateral-atom destination is a Position

**(D6) Fee atoms are credited to an ordinary `PositionAccount` owned by the
policy's `treasury` key — the "treasury Position" — one per Market, on the
existing seeds (`seeds.rs:56`, `[SEED_POSITION, market, owner]`).** No new
atom-holding account family exists.

Why a Position and not a new pot family:

- **Conservation is inherited, not re-proved.** A fee is a transfer inside
  the Hoard's liability ledger: buyer's Position debits consideration plus
  fee, seller's credits consideration, treasury's credits the fee. Hoard
  `collateral_atoms` (`lib.rs:1010-1023`) never moves; the fee cannot spend
  Hoard principal *by construction*, which is the boundary doc's hardest
  requirement (`DEPLOYMENT_REVENUE_BOUNDARY.md:67`).
- **Terminal lifecycle is inherited.** Positions already have owner-paid
  rent, close/reopen generations, the terminal walk, and a classification
  row in the 37-row inventory. A fee pot family would re-derive all of it
  and add a new permanent-rent risk; the treasury Position adds **zero new
  account families and zero new terminal rows** to Plane C.
- **Withdrawal is inherited.** Treasury revenue leaves through the same
  audited cash-withdrawal path as any owner's Position cash. No bespoke
  sweep instruction exists on Plane C at all.

Rejected alternatives, named:

- *Hoard counter field* — violates `lib.rs:1017` and conflates owed revenue
  with the burn-at-terminal compartments of
  `TERMINAL_LIFECYCLE_RUNTIME_V1.md` §3(13-15).
- *Standalone `RevenuePotV1` family* — duplicate Position semantics, one new
  rent row per market, a new close handler, a new inventory row; nothing
  bought.
- *Direct transfer to an external treasury token account at settlement* —
  puts an externally-owned account inside the settlement hot path and moves
  real tokens per fill; the Position credit is a ledger write with the
  tokens staying pooled in the Hoard vault, which is the whole point of the
  internal cash plane.

**Creation and funding:** the treasury Position is created through the live
owner-signed entry path — `Intent::Endow` creates the missing
generation-zero Position/Replay pair and the signer must equal the Position
owner (`programs/clutch-sbf/program/src/instructions/genesis.rs:16,53-54`) —
by the treasury authority, rent paid by it, once per Market it elects to
collect on. Refusal-first
consequence: **admission of a fee-bearing epoch refuses while the Market's
treasury Position is absent** — the named recipient must exist before the
first chargeable intent is admitted, which is precisely the "named
recipient" the settlement seam already demands in its own comment
(`orders_batch/settlement.rs:596-597`).

## 6. The carry is owner-scoped, then allocated across signed intents

The earlier per-intent proposal was wrong for the selected composite base.
`G(a,p)` is computed over an owner's aggregate filled payoff vector and is
subadditive across intents. Independent intent carries would first change the
base by preventing owner netting and then introduce independent rounding
boundaries. That can overcharge the same economic owner. A reservation may own
an intent's authorization envelope, but it cannot own the composite rational.

`IntentFeeCarry` (`crates/clutch-liveness/src/lib.rs`) still supplies useful
full-width arithmetic evidence: its exact denominator, remainder, and fragment
numerator are `u128`, required because the admitted
`10_000 * price_scale^2 * 10_000` denominator can exceed `u64`. It is not the
runtime semantic owner of the composite carry.

The account-neutral successor in `crates/clutch-fee-runtime-contract` corrects
the ownership boundary:

- `SelectedCompositeFeeV1` binds one canonical fee-record identity, selected
  candidate, exact rated policy and revenue-policy digests, treasury owner and
  ordinary Position, scale, width, rates, and relation-derived denominator.
- `OwnerFeeCarryV1` is keyed by `(fee record, owner)`, validates restored
  `u128` remainder state, and is the only constructor of the owner-level
  floor or terminal-ceil assessment.
- Only after that quote does `allocate_payer_debit` partition the resulting
  `u64` atom debit across the same owner's strictly intent-ID-ordered signed
  envelopes. The returned rows bind both intent identities and debit amounts.
- Recipient allocation rebinds the selected revenue-policy preimage, applies
  60/0/40, and uses Hamilton largest remainder over candidate-verified
  standing-maker Position weights. A nonzero executor share refuses because
  V1 authenticates no executor identity.

The successor now joins the canonical General V2 owner-settlement builder:
each lexicographically ordered participating owner supplies one authenticated
terminal projection, including explicit zero rows for seller-only owners. The
projection recomputes the terminal payer allocation from signed envelopes,
requires cumulative post-transition envelope debits to equal the closed
carry's cumulative paid atoms, and proves the whole buy consideration plus fee
fits that owner's authenticated buy reservation. The candidate-selected fee
total is the exact sum of all owner rows and is split once, candidate-wide.
Per-owner recipient splitting is not an alternate route because its rounding
would be different.

Account-neutral inner codecs and typed action joins are frozen in
`crates/clutch-fee-runtime-contract/SCHEMA.md`. They allocate no outer SBF tag,
PDA seed, rent payer, action, or capability. Their temporary payer/recipient
snapshot widths remain subject to rent and compute review before promotion.

**(D7 revised): no per-intent carry PDA and no carry words in a reservation.**
The reservation successor keeps the signed `max_fee_atoms` and cumulative
intent debit. The owner carry must live once per owner in a versioned selected-
candidate/epoch fee ledger, or in an equivalently exhaustive fixed owner table
whose identity is covered by the selected fee record. The adapter choice is
still open because the rent, account-lock, maximum-owner, and terminal-close
tradeoffs have not been measured. What is no longer open is the semantic key:
one carry per `(fee record, owner)`, never one per intent.

Required persisted owner row:

```text
owner:              [u8; 32]
carry_denominator:  u128
carry_remainder:    u128
fee_paid_atoms:     u64
terminal:           bool
```

**Envelope rule (D8):** admission under a fee-bearing policy requires the
canonical aggregate remaining capacity of that owner's signed envelopes to
cover the quoted worst-case fee, terminal-ceil atom included. Every debit also
stays within each contributing intent's bound. `max_fee_atoms == 0` remains
admissible only under the zero-fee policy and means "no fee, ever" bit-exactly.

Terminal-ceil fires once when the owner row closes, not when an arbitrary
reservation releases. Its assessed atom is allocated through the same signed
envelope rule and the exact recipient split before the owner row becomes
terminal.

## 7. The split: 60 / ≤15 / ≥25 becomes structure, executor deferred

The prose split (`docs/ECONOMICS.md:146-148`) has no Rust and its only
executable form is the Python lab's `allocate_fee` (`:151-154`). V1
proposal, frozen in the §3 const:

- **Maker rebate 60%, netted at settlement.** Makers' Positions are already
  in the settlement account list; the rebate is a smaller net debit/larger
  net credit computed by the relation, not a later distribution. The
  *standing-maker* predicate is an undecided input
  (`docs/OPEN_QUESTIONS.md` P2: "at least one full frozen Epoch is the
  leading candidate"); V1 pins the trivially-true predicate
  (`AllRestingMakers`) so the structure lands, and a stricter predicate is a
  sibling policy const, not a mutation.
- **Executor 0%, explicitly deferred (D9).** No authenticated executor
  identity exists in the atom plane (direct keeper rewards are lamports from
  a WorkBudget — a Plane L object). The executor share folds into treasury:
  effective V1 vector **60 / 0 / 40**, which satisfies both published
  envelope constraints (≤15 executor, ≥25 treasury) and *tightens* the
  Sybil bound of `docs/ECONOMICS.md:156-158` from ≤75% recovery to ≤60%.
  An executor share returns only with an authenticated executor account
  plane, as a sibling const.
- **Rounding is the frozen rule of `docs/ECONOMICS.md:165-166`:** fee rounds
  up (terminal ceil), rebates round down, residual atoms follow the policy's
  `residual` member — V1: treasury.
- `validate()` (§3) keeps every future sibling inside the published
  envelope; a const violating it does not digest.

## 8. The admission path: fee-bearing profile, domain validator, candidate ABI

What must exist before the domain validator admits a fee-bearing epoch:

1. **A fee-bearing `FrozenPolicyV1` sibling const with a new digest.**
   `fee_base` is already a policy member folded into the digest
   (`crates/clutch-batch/src/relation_v1.rs:255`, `:285-288`), and
   `FeeBaseV1::FlatNotional` is a complete arm with carry, conservation, and
   exactness checks (`:217-225`; findings §5). If the fee-base fork selects
   any other arm, that is a new `FeeBaseV1` variant — a relation change with
   its own tests — which this document does not preempt. Epoch open today
   hardcodes the zero-fee digest
   (`direct_selection_v3.rs:320`); admitting the sibling means the open path
   accepts exactly the enumerated consts, nothing dynamic.
2. **The candidate ABI change the findings doc flags.** The compact
   candidate body carries ids, digests, prices, fills, volume terms, and a
   slot — **no fee field of any kind**
   (`programs/solana-layout/src/direct_selection_v3.rs:759-779`). A
   fee-bearing candidate is a **new candidate version** whose body commits the
   exhaustive owner-level fee/carry rows, intent-envelope debit rows,
   standing-maker rebate rows, and treasury total, so that
   `verify_submitted_candidate`'s verdict (and
   the streaming `ClearWorkV1` verdict on the general route) covers fee
   arithmetic exactly as it covers fills. The fee columns enter the relation
   domain digest; an old-version candidate under a fee-bearing policy is a
   deterministic refusal, not a zero-fee fallback.
3. **What the destination requires of whichever base wins** — stated here so
   the base fork cannot pick an arm the destination cannot host. All four
   lab arms (`research/economics-admission/model.py:529-533`, quotes at
   `:631-671`) must supply:
   - an exact rational quote `(numerator, denominator)` whose denominator is
     computable and frozen at admission (the §6 carry denominator);
   - a worst-case fee bound computable at admission for the (D8) envelope
     check;
   - checked arithmetic within `u128` for every admitted shape;
   - a documented zero-price disposition: three of the four arms charge
     **zero** on transfers supported entirely on zero-priced outcomes,
     however large the model-free range; only `QUOTIENT_RANGE` charges it
     (`research/economics-admission/run_lab.py:59-77`,
     `test_model.py:500-534`). The destination is base-agnostic, but §10
     makes the chosen arm's disposition of that channel a required fixture,
     not a footnote.

## 9. The repeated `max_fee_atoms == 0` boundaries

The historical five-gate count is stale. The current program re-authenticates
the signed zero envelope at every value-moving trust boundary:

- Direct V4 placement (`orders_batch.rs:982`);
- General walk reservation (`orders_batch/clear_walk.rs:377`);
- General entitlement and portfolio-pair settlement
  (`orders_batch/entitlement.rs:841,2084`);
- General direct submission and settlement
  (`orders_batch/settlement.rs:439,695`);
- General virtual split-pay, merge-deliver, and merge-pay
  (`orders_batch/settlement.rs:1290,1468,1581`); and
- legacy/direct selection settlement and reservation validation
  (`direct_selection.rs:909-910,1778`).

These repetitions are trust-boundary checks, not duplicate scar tissue to
delete. A fee-bearing successor must replace each zero comparison with the
same selected fee-record identity, exact policy preimages, owner carry row,
exhaustive intent debit rows, verified recipient rows, treasury Position, and
atomic conservation transition. A zero-fee route keeps its refusal forever.

The layout and pure mirrors relax in lockstep with a versioned runtime route,
never ahead of it. Changing only the host mirror or only SBF would destroy the
differential oracle.

## 10. Falsifiers required before any charge is nonzero

No gate of §9 (or §4) relaxes until each of these exists and runs green
against the real tree; each names the property whose failure kills the
design rather than the test:

1. **Conservation.** Per settlement: sum of Position debits equals sum of
   Position credits including every fee, rebate, and residual atom; Hoard
   `collateral_atoms` unchanged by any fee. Plane L: prepaid-budget debits
   equal vault credits plus rewards plus refunds, and the re-derived
   terminal reconciliation of §4 balances with mid-life charge departures.
2. **No-theft.** Charged atoms never exceed any contributing intent's signed
   `max_fee_atoms`, nor the canonical aggregate remaining capacity of the
   owner's exhaustive envelope set, terminal-ceil atom included. A zero
   envelope pays no atom. The Sybil recovery bound is measured at ≤60% under
   the V1 vector.
3. **No-stranding.** The hostile terminal walk
   (`TERMINAL_LIFECYCLE_RUNTIME_V1.md` §9) extended: treasury Position
   swept and closed, each owner carry closed exactly once per selected fee
   record with reopen refused, vault swept and closed, policy record closed,
   ending in the exact declared account set. The two new rows are in
   `terminal_profile.py` with bounds before implementation.
4. **Carry exactness.** Owner-level fragmentation invariance (partitioning one
   owner's filled payoff vector across settlement fragments cannot change
   total atoms paid) at the byte plane, inheriting
   `clutch-fee-runtime-contract` semantics; noncanonical carry refused at
   decode.
5. **Zero-price channel disposition.** The `run_lab.py` laundering row
   becomes a frozen regression fixture for the selected base: if the base
   is price-weighted, the accepted feeless channel is documented in
   `FEE_GEOMETRY.md` §5's threat list (findings §1) *before* the profile
   const is authored; if the base charges it, the fixture proves that.
6. **Split exactness.** The relation's fee vector equals the Python
   `allocate_fee` on the shared corpus; fee rounds up, rebates round down,
   residuals land per the frozen rule; the envelope refusals of §3
   `validate()` go red on out-of-envelope consts.
7. **No-silent-redirect.** The policy digest pinned at Realm creation is
   immutable: no instruction mutates the record, in-flight ResolutionWork
   keeps its Begin-frozen schedule digest, and an epoch admitted under one
   policy digest settles under no other.
8. **Wash cost.** Strict negativity of round-trip wash cycling under
   terminal-ceil (the property `docs/ECONOMICS.md:158-163` claims and the
   findings §4.1 corrected) measured on the byte plane, not the lab.

## 11. What this design does not decide, and the queue for ember

**The six-item queue below returned decided on 2026-08-20** — outcomes in the
status table at the head of this document
([../decisions/ADOPTED_2026-08-20.md](../decisions/ADOPTED_2026-08-20.md)
items 6 and 8). The queue text is kept as written so the decisions stay
readable against the question they answered. The "not decided here" list
immediately below stands, with one amendment: the fee base *shape* has since
been selected ([FEE_GEOMETRY.md](../FEE_GEOMETRY.md)) while the rate has not.

Not decided here, deliberately:

- **The fee base.** Four lab arms exist
  (`model.py:529-533`); the comparison the promotion gate demands is now
  startable (arms 3 and 6 landed) but un-run, and the market-quality axes
  stay declared out of V1 scope per findings §6. This design is
  base-agnostic within §8.3's requirements.
- **The rate.** No numerator is proposed anywhere above.
- **Promotion.** Every gate stays closed; this document adds falsifiers, it
  does not pass them.
- **Real-money activation.** Track separation of
  `DEPLOYMENT_REVENUE_BOUNDARY.md` §5 stands untouched.

Decisions queued for ember, smallest sufficient set:

1. **The treasury key** — custody, and acceptance that rotation is
   representable only as a new Realm (D3/D4). Everything in §3-§5 hangs off
   this key's existence.
2. **Plane C destination shape** — treasury Position (D6, recommended)
   versus a standalone pot family; if D6, confirm the treasury authority is
   willing to run the owner-signed creation step per Market (§5).
3. **Plane L disposition** — L1 vault (D5, recommended) versus L0 burn, and
   whether ResolutionWork charges should exist at all versus staying a
   permanent zero (charging resolution may be anti-liveness; that is an
   economics call, not a plumbing one).
4. **Sequencing** — Plane L before Plane C (D2, recommended).
5. **The V1 split vector** — 60/0/40 with executor deferred (D9,
   recommended) and the trivially-true standing-maker predicate (§7),
   versus holding Plane C until the standing-maker definition
   (`OPEN_QUESTIONS.md` P2) is decided.
6. **Terminal classification** of the two new rows (§3, §4) — accept the
   Realm-lifetime record/vault rows with headers, or demand a stricter
   bound before any lane starts.

Nothing in this file is runtime truth until each of those returns decided,
the §10 falsifiers exist, and the change crosses its own promotion gate
under `CURRENT_TRUTH.md` vocabulary.
