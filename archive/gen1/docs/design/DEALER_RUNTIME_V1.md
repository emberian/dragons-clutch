# Covered dealer runtime V1

Status: **PURE V1 SEMANTIC BODIES FROZEN / NO SBF ACCOUNT WRAPPERS OR ROUTE /
ALL ACTIONS DISABLED / NO LIVE AUTHORITY** (2026-08-23).

This document defines the intended runtime refinement of the exact signed
dealer model in
[`research/bounded-liquidity-facility/src/signed_dealer.rs`](../../research/bounded-liquidity-facility/src/signed_dealer.rs)
and the canonical owner-blind batch join in
[`crates/clutch-batch/src/dealer_leg_v2.rs`](../../crates/clutch-batch/src/dealer_leg_v2.rs).
The standalone
[`clutch-dealer-runtime-contract`](../../crates/clutch-dealer-runtime-contract)
crate now freezes allocation-free local semantic-body bytes and validators. It
does not allocate central account or instruction tags, define the outer SBF
account wrapper, enable a capability, or claim that any transition executes
under SBF.

The central registry reserves dealer family 76 but allocates no family-local
dealer actions. The current product enables no extension action. Those facts
must remain true until the activation gates at the end of this document close.

## Selected runtime shape

One facility is a fully collateralized, nonlinear counterparty for one native
Market/Terms/Instance claim domain. It is not an independent sequencer. It may
participate only as one aggregate leg of the best valid submitted batch
candidate selected under the frozen candidate policy.

LPs fund exact unit baskets of collateral and existing, already-backed native
Eggs. A sponsor separately deposits present cash `K`. Before activation `K` is
refundable only after valid cancellation. Activation irrevocably donates `K`
to the LP pool. The dealer never mints an Egg, borrows collateral, reaches into
Hoard principal, or treats future fees as present capital.

For immutable shares `S`, unit cash `c_u`, unit Eggs `g_ui`, and signed net
inventory `q`, the model-owned asset identities are

```text
pool_cash(q) = S*c_u + K + C_hat(q)
pool_egg_i(q) = S*g_ui - q_i
```

where `C_hat` is the signed, upward-rounded quadratic endpoint potential.
`q_i > 0` is a net Egg sale and `q_i < 0` is a net Egg purchase. Immutable
bounds, full mixed-corner price checks, lower-corner cash financing, and actual
Egg custody must all pass. These equations are semantic requirements; a
computed value is never evidence that an account holds the asset.

## Frozen pure semantic codecs

These are exact local bodies, not global Solana account discriminators. Every
body begins with an 8-byte magic, little-endian `u16` version `1`, and zero
`u16` flags. Decoders require the exact length and reject unknown versions,
trailing bytes, nonzero reserved bytes, invalid enum values, and noncanonical
fixed-width padding.

| Pure V1 body | Magic | Exact bytes | Frozen principal geometry |
| --- | --- | ---: | --- |
| `DealerPolicyV1` | `DCDPOLV1` | 1,148 | sixteen 32-byte identities, including the one neutral sink; outcome width and policy arrays/rules; no mutable rent tail |
| `DealerFacilityGenesisV1` | `DCDFGNV1` | 116 | policy, sponsor, refund recipient, and content-derived facility nonce |
| `FacilityPositionBindingV1` | `DCFPBND1` | 244 | full facility/policy/Market and Position/Replay/State authority join |
| `DealerFacilityPositionV1` | `DCFPOSV1` | 388 | sole live cash/Egg owner with full Market and authority identities |
| `DealerRootTombstoneV1` | `DCRTMBV1` | 276 | permanent terminal root evidence and exact rent disposition |
| `DealerLivenessScheduleV1` | `DCLSCHV1` | 372 | exact 22-action maximum-call and successful-call lamport vectors |
| `DealerFundedBudgetDependenciesV1` | `DCFDDEP1` | 348 | Dealer/external liveness policy and projection digests, fee/collateral/token bindings, State authority, generation, and exact Dealer work principal |
| `DealerStateV1` | `DCDSTAT1` | 680 | eleven identities, phase/sponsor facts, generation and child sequence, share facts, signed `q[16]`, eleven child counts, 88-byte root-rent tail |
| `LpPageV1` | `DCLPPGV1` | 1,208 | policy/facility, counted generation, chain/flags/revision, sixteen 64-byte entries, 80-byte child-rent tail |
| `DealerLeaseV1` | `DCLSEV01` | 652 | sixteen identities, `g -> g+1`, three slots/deadlines, outcome/row counts, 80-byte child-rent tail |
| `SettlementPotV1` | `DCPOTV01` | 1,084 | twelve identities, one of three persisted phases, `g -> g+1`, row count/two cursors, `U`/`D`/`F` aggregates, two liabilities, collect/deliver progress, 80-byte child-rent tail |
| `FeeBudgetV1` | `DCFEEV01` | 348 | six identities, counted generation, exact principal partition, liability count/phase, 80-byte child-rent tail |
| `LivenessBudgetV1` | `DCLIVV01` | 348 | the same geometry under a disjoint magic and content domain |

The fixed maxima are 16 outcomes, 16 LP entries per page, 4,096 LP pages,
and 64 settlement rows. The allocation-free content-ID stack buffer is exactly
the largest body, 1,208 bytes. Content identity is
`SHA256(exact_domain || exact_body)` under the seven disjoint trailing-NUL
domains `dragons-clutch/dealer-runtime/{policy,state,lp-page,lease,settlement-pot,fee-budget,liveness-budget}/v1\0`.
The exact byte regions and frozen vectors are recorded in
[`SCHEMA.md`](../../crates/clutch-dealer-runtime-contract/SCHEMA.md).

The SDK-free PDA preimages are also frozen. The executing program identity and
bump remain adapter-owned:

| Family | Exact ordered seed preimage |
| --- | --- |
| Policy | `b"dc-dealer-policy-v1"`, `policy_id[32]` |
| Facility genesis | `b"dc-dealer-facility-v1"`, `facility_id[32]` |
| Facility Position binding | `b"dc-dealer-pos-bind-v1"`, `facility_id[32]` |
| Facility Position | `b"dc-dealer-position-v1"`, `facility_id[32]` |
| Facility Replay | `b"dc-dealer-replay-v1"`, `facility_id[32]` |
| Liveness schedule | `b"dc-dealer-live-sched-v1"`, `schedule_id[32]` |
| Funded dependencies | `b"dc-dealer-funded-v1"`, `facility_id[32]` |
| State | `b"dc-dealer-state-v1"`, `facility_id[32]` |
| LP page | `b"dc-dealer-lp-page-v1"`, `facility_id[32]`, `page_ordinal_le[4]` |
| Lease | `b"dc-dealer-lease-v1"`, `facility_id[32]`, `pre_generation_le[8]` |
| Settlement pot | `b"dc-dealer-pot-v1"`, `facility_id[32]`, `pre_generation_le[8]` |
| Fee budget | `b"dc-dealer-fee-v1"`, `facility_id[32]` |
| Liveness budget | `b"dc-dealer-live-v1"`, `facility_id[32]` |

## One semantic owner per persisted fact

The pure bodies below are frozen. Names ending in `AccountV1` still describe
future SBF wrappers and must not be confused with the local body magic above.

| Persisted fact | Sole semantic owner | Required relationship |
| --- | --- | --- |
| collateral selection and admitted token profile | immutable Realm | The dealer policy references it; no DREGG branch exists. |
| native basis, payout denominator, Terms, Instance, claim domain | existing canonical Market/Terms/Instance owners | The dealer copies only authenticated digests and active width. |
| depth, prior, signed box, unit basket, share/page bounds, schedule, queue quorum, price/curve/certificate/fee/liveness/retirement policies, neutral sink | frozen `DealerPolicyV1` inside a future immutable `DealerPolicyAccountV1` | Content-addressed policy bytes; no mutable duplicate in state. |
| phase, dealer generation, child sequence, signed `q`, total/queued shares, sponsor refund state, active epoch binding, child counts | frozen `DealerStateV1` inside a future `DealerStateAccountV1` | References the policy digest and facility semantic identity; stores no asset balance. |
| live pool collateral and existing native Eggs | one existing `PositionAccount` in the facility-authority role | The Position is the sole asset balance owner; state may cache no second pool balance. |
| Facility Position replay/deposit sequence | its existing `ReplayAccount` companion | It is not an asset balance and must retire with the Position owner plane. |
| LP owner, shares, queued shares, terminal claim, claim state | frozen `LpPageV1` bodies inside future canonical page accounts | Each page has strict owner order and canonical padding; `DealerLpFundingFoldV1` streams the complete sealed ordered set and binds its root/totals to State after the adapter authenticates every page PDA/owner. |
| maximum paid calls and reward lamports per frozen Dealer action | immutable `DealerLivenessScheduleV1` | The liveness-policy owner selects and measures values; the Dealer groups exact dot products into six non-Source external compartments. |
| external liveness custody, principals, payers, owners, funding classes, quote/receipt programs, and terminal-path bounds | external seven-account runtime plus ephemeral `DealerRuntimeLivenessBindingV1` projection | The external runtime is sole mutable owner. Dealer persists only the authenticated projection digest and exact grouped work principal; rent remains separate. |
| nonzero fee assessment, owner carry, Position debit, recipient allocation, and treasury credit | separately authenticated owner-netted fee runtime | Dealer binds the immutable fee-policy ID and owns no fee vault or caller-asserted revenue balance. |
| one selected candidate's exclusive generation lock | frozen `DealerLeaseV1` inside a future lease account | Exactly zero or one live lease per facility. |
| one selected leg's transient dealer cash/Egg assets and cursors | frozen `SettlementPotV1` inside a future pot account | The pot is the sole transient asset owner until atomic completion; it does not mirror Facility Position balances. |
| charged fees, rebates, recipients, and distribution state | a separately versioned fee pot/budget owner | Never a field or balance in dealer pool cash. |
| keeper rewards, compute work, rent principal, and refund destinations | immutable SeriesFunding compartments or a separately capitalized dealer work/rent budget | Never `K`, LP assets, Hoard principal, or expected future fees. |
| selected candidate, canonical fill rows, reservations, and entitlements | existing/future versioned batch lifecycle owners | Dealer accounts store only bound digests, counts, and cursors. |
| resolved payout vector and terminal allocation projection | authenticated canonical resolution owner plus future checked page-set projection | The frozen State/Page bodies can represent phases and terminal claims, but do not yet define the authenticated payout-to-claim projection. |
| terminal child counts and close authority | the repository's counted-retirement aggregate | No child closes or disappears outside that graph. |

The facility authority is a program-derived role distinct from the semantic
facility digest. The digest identifies canonical facility semantics; the PDA
controls the Facility Position. A live decoder must authenticate both rather
than treating an account key as content identity.

The Facility Position owns internal native claims backed by the existing
pooled Hoard and supply theorem. Sponsor and LP cash must first enter an
ordinary Position through the real Token-2022 endowment boundary. Contribution
then debits the sponsor/LP Position and credits the Facility Position. LP Eggs
must already be authenticated internal native claims; contribution debits the
LP Position and credits the Facility Position without changing Hoard backing.
An externalized Egg needs an explicitly admitted token-to-internal ingress; V1
does not assume one exists. No per-outcome dealer token vault is introduced.
Every transfer or CPI is followed by exact account reload and postcondition
checks.

### Rent ownership

Rent principal is not pool capital. `DealerPolicyV1` owns one immutable
`neutral_sink`. Every mutable body carries an exact rent tail joined back to
that sink:

```text
DeletableRentOwnerV1 (80 bytes) =
    payer[32] | neutral_sink[32] |
    refundable_principal u64 | donation_floor u64

RootRentOwnerV1 (88 bytes) =
    payer[32] | neutral_sink[32] |
    refundable_live_principal u64 |
    permanent_tombstone_principal u64 | donation_floor u64
```

The payer and sink are live, distinct identities; required principal is
positive and every lamport sum is checked. State uses the root form. LP pages,
leases, pots, and both budgets use the deletable form. Page, State, Lease, and
budget policy joins require the rent sink to equal `DealerPolicyV1.neutral_sink`;
the Pot joins the Lease sink. Each budget additionally requires its economic
principal payer and refund recipient to differ from the same sink, and its own
sink field to equal both its rent sink and the Policy sink.

The `donation_floor` is construction-time hostile prefund and routes only to
the Policy sink. Refundable State rent returns to its construction payer;
LP-page rent to the named page/rent-budget payer; lease and pot rent to the
named candidate/work-budget payer; and budget rent to its own named payer.
Root tombstone principal remains locked. V1 Policy account rent is not in the
immutable Policy body and remains locked with the retained catalog's future
account wrapper. A closer gains no discretion over rent, `K`, LP assets, fees,
or Hoard backing.

## Facility phase machine

```text
Funding --sufficient, permissionless activation--> Trading
Funding --valid cancellation--> Cancelled --exact refunds--> Retiring --> Closed
Trading --sponsor halt / queue quorum / timed close--> UnwindOnly
Trading --authenticated maturity payout--> Resolved
UnwindOnly --authenticated maturity payout--> Resolved --all claims--> Retiring --> Closed
```

`Retiring` and `Closed` are runtime phases not present in the pure research
state. They add no economic transition. They make terminal child disposition
and rent recovery explicit.

Every successful economic/lifecycle transition consumes an exact
`dealer_generation` and produces the next generation. This includes successful
Funding contributions and withdrawals: initialization may start at zero, but
the Funding phase is not constrained to generation zero. A separate monotone
`child_sequence` covers epoch binding, lease/pot/page construction or closure,
and counted close without invalidating a quote merely because its escrow
children were created. Pot-owned cursors are independently monotone. Selection
leaves `dealer_generation` equal to the quote's `pre_generation`; finalization,
canonical empty-epoch lapse, and pre-collection abort consume it. While an
epoch binding or dealer lease exists, every unrelated Facility State transition
refuses. In particular, queue, halt, timed close, and resolution cannot make a
selected quote stale halfway through settlement. The prepaid work budget and
permissionless progress rules make finishing the bounded pot the liveness path.

### Funding and activation

Logical transitions, with SBF instruction tags and account wrappers still
unallocated but the pure bodies above frozen:

1. `InitializeFacility` authenticates the Realm, native domain, and sponsor
   Position, creates the policy/state/Facility Position, transfers and reloads
   present sponsor cash `K`, and records its still-refundable ownership.
2. `ContributeLpBasket` debits an authenticated LP Position and credits the
   Facility Position with an integer number of exact unit baskets, reloads both,
   updates one canonical LP page, increments total shares, and advances the
   funding generation.
3. `WithdrawFundingBasket` reverses an exact basket only before the funding
   deadline or in `Cancelled`, and advances the generation on success.
4. `CancelFunding` is permissionless after an underfunded deadline or when a
   sufficiently funded facility was never activated by trading close.
5. `RefundCancelledSponsor` returns exactly `K` only in `Cancelled` and only
   once. LP refunds are independent and order-invariant.
6. `ActivateFacility` is permissionless after the funding deadline. It proves
   the exhaustive page total, minimum and maximum shares, zero child lease/pot,
   `Position.cash_atoms = K + S*c_u`, and
   `Position.internal[i] = S*g_ui`. It then freezes
   the LP roster and makes `K` irrevocably LP-owned.

An individual `LpPageV1` validates a page ordinal below 4,096, a next ordinal
that is exactly consecutive and in range, a full non-tail page, strictly sorted
owners, exact empty padding, and an optional Policy-specific page cap. The
Policy quorum check is the exact division-free comparison
`queued*denominator >= total*numerator`; zero total never reaches quorum, and a
Trading State whose queued shares already meet the threshold refuses. These
local checks do not yet authenticate an exhaustive cross-page root or fold.

Token profiles with transfer fees, opaque balance changes, transfer hooks,
frozen/default-frozen state, delegation, wrong mints, or wrong claim domains
refuse unless a later separately proved adapter explicitly admits them.

## Quote authentication and the one-generation lease

The quote service is untrusted. Runtime authority comes from recomputation over
authenticated accounts and proofs, not from a service signature.

Before a candidate may acquire a lease, the runtime must:

1. authenticate the RelationV2 domain, frozen book, candidate, exact price
   policy decision, fee-policy projection, Facility State, and policy account;
2. recompute the upstream economic candidate digest;
3. recompute the dealer quote semantic digest, which binds that upstream
   digest and therefore the exact price context;
4. run `verify_economic_candidate_with_dealer_v2`, making
   `MinimumGrossHamiltonV1` the sole per-order allocation owner; and
5. call the signed facility reconciliation, which binds facility, policy,
   generation, aggregate trade, and quote digest and independently recomputes
   aggregate curve cash.

A pure `DealerLegVerdictV2` is a public projection, not an authentication
token. Verification may be staged only through a canonical sealed checkpoint
owned by the candidate lifecycle. Lease acquisition must reauthenticate that
checkpoint and the exact accounts/digests above, then rerun aggregate facility
reconciliation in the atomic selection transition.

Before a facility quote or dealer candidate is admitted, `BindDealerEpoch` must
bind the Facility State to exactly one canonical Market/Instance/epoch and its
bounded candidate window. Only the next epoch admitted by the immutable
Series/auction policy may bind; an arbitrary caller-chosen epoch cannot
monopolize the facility. The binding fixes the generation consumed by every
candidate quote. If the epoch selects no dealer candidate, a permissionless
bounded lapse clears the binding and advances generation. No second epoch may
admit this facility while the binding exists.

`SelectWithDealerLease` must atomically create the lease and its initial
`Collecting` pot, deposit exact `D_out/F_sell`, and leave both cursors and all
user-progress totals zero. There is no selected dealer candidate without a pot
capable of progress. The lease binds at least:

```text
facility_semantics_digest
policy_semantics_digest
dealer_state_account
facility_position_pre_semantics_digest
lease_account
pre_generation / post_generation = pre_generation + 1
market / instance / epoch
selected_candidate_economic_digest
upstream_economic_candidate_digest
dealer_quote_semantics_digest
checked_dealer_leg_verdict_digest
curve_price_certificate_digest
canonical_settlement_rows_root
row_count / outcome_count
settlement_pot_account
fee_budget_account
liveness_budget_account
created / collect-deadline / deliver-deadline slots
```

Selection must match and transition the active epoch binding to its selected
lease substate. The State records exact epoch/lease identities and one live
Epoch, Lease, and Pot count. A second selection against the facility refuses
even if it names the same generation.
The pure join requires the exact Policy content ID, MarketInstanceV2, Facility
Position pre-ID, active Epoch/Lease IDs, State generation and width, one live
Lease/Pot, both budgets, and the Policy neutral sink. It intentionally does not
bind the whole mutable State content ID, so a child-sequence-only update cannot
stale the quote. Finalization consumes `pre_generation`, produces
`pre_generation + 1`, clears the epoch and lease root bindings/counts, and
atomically closes the lease and pot.

V1 intentionally admits at most one active auction epoch/selected lease per
nonlinear facility. Independent facilities may operate concurrently. Lifting
this restriction requires either independently capitalized facility slices or
one proved inventory/cash reservation ledger with a canonical ordering across
epochs. Optimistic parallel selection against the same `q` is forbidden.

## Settlement pot phase machine

```text
Absent
  |
  | SelectWithDealerLease: create pot and deposit facility inputs
  v
Collecting --all canonical rows collected--> Delivering
    |                                           |
    | abort permitted only at cursor 0          | all canonical rows delivered
    | and before any external input             v
    v                                       Finalizing
  Absent                                        |
                                                |
                                                | sweep exact dealer residue,
                                                | apply one aggregate receipt,
                                                | update root, close Pot/Lease
                                                v
                                              Absent
```

The only persisted pot phases are `Collecting`, `Delivering`, and `Finalizing`.
There is no serializable `Complete` phase and no persisted swept-cash or
swept-Egg fields. Finalize is one atomic sweep/receipt/root-update/close
transition; failure leaves the pre-Finalize Pot, Lease, State, Position, and
counts unchanged.

The pot's twelve immutable identities are Policy, facility, Lease content,
Epoch, final SettlementCandidate, aggregate verdict, curve-price certificate,
Facility Position pre-state, expected Facility Position post-state, settlement
row root, FeeBudget account, and LivenessBudget account. It also stores
`pre_generation` and its exact successor, immutable row count,
`collect_cursor`, `deliver_cursor`, exact `U_in/U_out/D_in/D_out`,
`F_buy/F_sell[16]`, fee/liveness liabilities, and only the monotone
collected/delivered totals required to derive current custody. A separate fee
pot owns `fee_cursor` if nonzero fees are enabled. The dealer pot does not
persist a second balance, a bitmap, a sweep total, or derived per-order
allocations. Each progress call reruns or authenticates the selected candidate
projection and derives the bounded contiguous row slice from `dealer_leg_v2`.

### Escrow refinement while state is leased

With no lease, the Facility Position alone must equal the exact pool assets
derived from policy, shares, `q`, and phase. Begin deliberately moves facility
inputs into the pot before `q` advances, so the Position alone does not satisfy
that idle identity while a lease exists. This is not an allowed unexplained
cache mismatch. The authenticated State/lease/pot aggregate owns one temporary
refinement:

```text
leased_position_cash  = pre_pool_cash - D_out
leased_position_egg_i = pre_pool_egg_i - F_sell_i.
```

Those Position balances remain frozen through Collecting and Delivering. The
pot must simultaneously satisfy the phase equations below. No other State or
Position transition is admitted. Finalize sweeps the exact residue, applies
the one pure aggregate receipt, and restores the ordinary one-Position
identity at post-generation. The atomic root update publishes `q'`, the bound
Facility Position post-state ID and `post_generation`, clears the active
Epoch/Lease identities and their Epoch/Lease/Pot counts, advances the child
sequence for closure, and deletes the Lease/Pot under their named rent rules.
Abort instead restores the pre-generation Position identity before advancing
`dealer_generation` with unchanged `q`.

An adapter validating a leased State must authenticate the exact lease and pot
PDAs and verify this aggregate refinement. It may not accept an arbitrary
Position mismatch merely because `active_lease_count` is one.

The frozen pure transition join additionally computes

```text
q'_i = q_i + F_sell_i - F_buy_i
delta_cash = ceil(C(q')) - ceil(C(q))

if delta_cash >= 0: D_in = delta_cash, D_out = 0
if delta_cash <  0: D_in = 0,          D_out = -delta_cash.
```

It validates the Policy, pre-State, and Lease; enforces the signed box and lot;
and in `UnwindOnly` requires every component to move toward zero without
crossing it. This independently recomputed join, not the verdict digest, is the
cash authority. The Pot does not impose the scalar `MAX_ATOMS` cap on gross
`U`, `D`, or `F` aggregates: checked arithmetic and the exact endpoint/box join
admit a valid full-box crossing larger than one inventory-component cap.

### Idempotent cursor contract

Rows are in strict immutable-order-ID order. Each progress request names one
half-open slice `[requested_start, requested_end)`:

- `requested_start == cursor < requested_end` may process the bounded
  contiguous slice and set `cursor = requested_end` only after every transfer
  and postcondition passes;
- `requested_end <= cursor` is a checked idempotent retry and succeeds with no
  state or asset mutation when all bound digests match;
- a partial overlap, gap, reversed range, wrong row identity, or end beyond
  `row_count` refuses without mutation; and
- a Solana transaction failure rolls back both assets and the cursor.

No bitmap or candidate-carried row state can create a second allocation truth.
Strict cursors also make the number of remaining permissionless calls explicit
for the liveness budget.

### Exact cash conservation

Let

```text
U_in   = sum canonical buyer cash paid to the dealer
U_out  = sum canonical seller cash paid by the dealer
D_in   = dealer_net_cash_in_atoms
D_out  = dealer_net_cash_out_atoms
```

The dealer relation proves the one-directional receipt and exact equation

```text
U_in + D_out = U_out + D_in
D_in == 0 or D_out == 0.
```

Pot cash must refine that equation at every phase:

```text
after Begin:    pot_cash = D_out
after Collect:  pot_cash = D_out + U_in
after Deliver:  pot_cash = D_out + U_in - U_out = D_in
atomic Finalize consumes D_in and closes the pot; conceptual residue = 0
```

All user cash is debited from authenticated reservations. `D_out` is debited
from the Facility Position at Begin. `D_in` is the exact cash residue swept
back to that Position at Finalize. Seller output is impossible before every
buyer and seller input row is collected. When nonzero fees are enabled, entry
to `Delivering` also requires the separately authenticated fee cursor to have
collected every active row.

### Exact Egg conservation

For each native outcome `i`, let

```text
F_sell_i = aggregate Eggs sold by the facility to dealer-filled buys
F_buy_i  = aggregate Eggs bought by the facility from dealer-filled sells.
```

`dealer_leg_v2` derives these uniquely from RelationV2 imbalance and proves
the sorted dealer rows reproduce them. Pot custody evolves as

```text
after Begin:    pot_egg_i = F_sell_i
after Collect:  pot_egg_i = F_sell_i + F_buy_i
after Deliver:  pot_egg_i = F_buy_i
atomic Finalize consumes F_buy_i and closes the pot; conceptual residue = 0.
```

Begin debits `F_sell_i` from the Facility Position. Collect debits exact
coefficient-expanded Eggs from seller reservations. Deliver credits exact Eggs
to buyer Positions. Finalize sweeps `F_buy_i` to the Facility Position. Its
post-state must satisfy

```text
new_position_cash  = old_position_cash - D_out + D_in
new_position_egg_i = old_position_egg_i - F_sell_i + F_buy_i
                    = S*g_ui - q'_i.
```

The adapter reloads every affected account and then applies the facility's one
aggregate pure receipt exactly once. Per-row mutation of `q`, curve cash, or
generation is forbidden.

### Abort and stuck work

Abort is admitted only when the dealer cursors are zero and the authoritative
candidate lifecycle proves that no other leg of the selected candidate has
started settlement. It atomically lapses the whole selected candidate, returns
the exact Begin deposits to the Facility Position, proves the pot is zero,
clears the epoch binding, closes the pot/lease, and advances generation without
changing `q`. The generation bump prevents replay of the abandoned quote. Once
any candidate input or output has moved, abort is forbidden. Any caller may
advance collection, delivery, and finalization from the immutable selected
projection, funded by the prepaid work budget.

A successor recovery transition for corrupted or unavailable selected inputs
must define a complete reverse ledger before it is enabled. V1 does not assume
that partial assets can be swept, socialized, or donated.

## Fee plane is separate

`DealerQuoteRowV2.external_fee_atoms` and the verdict's total are
upstream-quoted, digest-bound amounts. They do not establish fee direction,
funding, recipient authority, custody, or transfer conservation.

A nonzero-fee runtime must independently:

1. freeze the fee derivation and recipient policy in semantic bytes;
2. derive each row's charge from authenticated order/reservation content and
   derive any rebate as a separately directed output;
3. reserve charges in addition to dealer cash and pre-fund every rebate or
   reward liability;
4. move fee assets through the separately owned fee pot using its own
   idempotent cursor;
5. prove exact input, output, carry, and recipient conservation; and
6. close or retire the fee pot under its named rent and residual rules.

Dealer cash, `K`, LP assets, Hoard principal, rent principal, and expected
future fees are unavailable to this plane. Until this contract and its tests
exist, an initial runtime profile may admit only exact zero external fees; it
may not describe them as implemented economics.

## Resolution, claims, and counted retirement

Close, resolution, claim, and retirement do not require the sponsor. The
facility may enter `UnwindOnly` through sponsor halt, share-weighted queue
quorum, or timed close. A live generation lease does not block this safety
phase change: it must finish under exposure-reducing rules and cannot open a
new Trading lease afterward. At maturity an authenticated
payout may resolve `Trading` or `UnwindOnly` directly after any active lease
finishes.

The required Resolution transition redeems the Facility Position's actual Eggs
through the ordinary native claim path, verifies the resulting cash, and runs
the exact terminal Hamilton allocation once over the exhaustive frozen LP
roster. Claims may then execute in any order without changing allocations.
This transition and authenticated projection are not implemented by the pure
crate: `DealerStateV1` can represent `Resolved`, page entries can hold an
explicitly allocated terminal claim (including zero), and the counted graph can
track unclaimed positions/work, but no current checker joins an authenticated
payout vector to the cross-page allocation and claim mutations.

The counted graph is

```text
DealerStateV1
  |-- exactly one Facility Position plus Replay companion
  |-- lp_page_count
  |    `-- live_lp_position_count / unclaimed_lp_position_count
  |-- active_epoch_binding_count in {0,1}
  |-- active_lease_count in {0,1}
  |    `-- exact selected lease identity
  |-- active_settlement_pot_count in {0,1}
  |    `-- pot-owned collect/deliver cursors
  |-- fee_budget_count in {0,1}
  |-- liveness_budget_count in {0,1}
  `-- resolution_or_claim_work_count in {0,1}
```

The eleven counts are exact `u32` fields in `DealerStateV1`; they are not
reconstructed from an indexer. `DealerChildGraphFoldV1` is constructed with the
exact facility and current generation. Every observed child must bind the same
facility, no child may name a future generation, and Epoch/Lease/Pot children
must name the exact current generation. An observation is transactional: a
rejected duplicate or cap overflow leaves the fold unchanged. Completion
requires exact equality with the State generation and all eleven counts.
Adapter account uniqueness, ownership, PDA authentication, and nested LP-entry
edges remain separate checks. The pure `DealerLpFundingFoldV1` now owns the
ordinal chain, cross-page owner order, page-content transcript, and exact
entry/share/queue totals, but the adapter must authenticate the accounts fed to
it and persist every page/root mutation atomically.

`Cancelled -> Retiring` requires sponsor refund complete, every LP basket
refunded, Facility Position zero, and no lease/pot/fee work. `Resolved ->
Retiring` requires every LP terminal claim paid, Facility Position zero, and no
lease/pot/fee/resolution work. `Retiring -> Closed` additionally requires all
LP pages closed, work/rent budgets disposed under immutable rules, every child
count zero, and the counted-retirement aggregate's exact terminal admission.

V1 retains the content-addressed policy as an immutable catalog artifact; it
therefore needs no mutable reference count that would contradict immutable
policy ownership. A successor that closes policy accounts must introduce a
separate counted reference owner and exact rent disposition. A state account
never closes merely because a static client reports no children.

## Explicit unresolved curve-price quantization gate

The quadratic facility has an exact rational average execution price for the
straight endpoint transition. For pre/post inventory `q,q'`, prior
`pi_i = a_i/A`, depth `b`, outcome count `n`, and totals `Q,Q'`, it is the
midpoint gradient

```text
p*_i = a_i/A + [n*(q_i+q'_i) - (Q+Q')] / (2*b*n).
```

It is an exact simplex inside the admitted box and satisfies the quadratic
endpoint identity

```text
C(q') - C(q) = sum_i p*_i * (q'_i - q_i)
```

before the one named endpoint ceiling boundary. RelationV2, however, consumes
a `u64` integer simplex with one exact `price_scale`. The reduced denominator
of every possible `p*` need not divide any practical frozen scale and may
exceed `u64` when `A`, `b`, and `n` compose. Converting `p*` to an integer
simplex therefore cannot be left to a solver or adapter.

Exactly one of these policies must be selected and proved before activation:

1. **Exact divisibility profile.** Restrict prior, depth, lot, box, and frozen
   price scale so every admitted midpoint price is exactly representable,
   with checked denominator and `u64` bounds.
2. **Canonical quantized profile.** Freeze one rational-to-integer simplex rule
   such as Hamilton largest remainder with exact tie order, then define how
   quantized order-limit admission relates to the separately exact endpoint
   dealer receipt. Prove padding, error bounds, monotonicity needed by limits,
   and adversarial boundary behavior.

The first is exact but may impose severe capacity restrictions. The second is
more general but changes the relationship between displayed clearing price and
exact nonlinear cash. No current decision selects either. Until this gate is
closed, there is no truthful SBF price precondition for the dealer. The frozen
Policy's `curve_price_certificate_policy_id` and the Lease/Pot
`curve_price_certificate_id` are opaque binding slots only; their presence is
not a quantization theorem or a certificate checker.

## Prepaid liveness

The immutable work policy must bound and presently fund:

- lease/pot construction;
- every maximum-width collect, deliver, fee, and finalize slice;
- abort before collection;
- queue/timed close;
- source publication and authenticated resolution;
- Egg redemption and every LP claim page;
- counted account retirement and rent recovery; and
- hostile retry overhead allowed by the frozen policy.

The bound derives from canonical row/page counts and measured CU/account/rent
rows. It cannot rely on expected volume, future fees, `K`, LP assets, or Hoard
principal. Failure to attract an external keeper may delay progress, but must
not make any caller signature other than the authenticated input owner a
consensus prerequisite. Legacy `LivenessBudgetV1` remains decodable but is not
an activation authority. `DealerLivenessScheduleV1` fixes one exact
maximum-call and successful-call lamport vector over all 22 frozen actions,
then groups exact dot products into Candidate, Clearing, Settlement,
Resolution, Retirement, and Recovery. The external liveness runtime solely
owns those physical accounts plus a separately quoted Source account, work,
rent, donations, receipts, refunds, and terminal transitions.
`DealerFundedBudgetDependenciesV1` binds the authenticated seven-account
projection digest and exact six-compartment Dealer work principal. It selects
no vector values; the liveness-policy owner must derive and measure them from
maximum row/page/CU/account/rent work before any adapter can activate.

## Remaining activation blockers

The pure crate closes the local fixed-body, hostile-codec, content-ID, PDA
preimage, rent-tail, local conservation, and counted-fold layer only. Dealer
family 76 remains disabled until these open blockers close together:

1. **SBF-authenticated LP page streaming and mutation.** The pure canonical
   root and activation fold now cover consecutive sealed pages, global owner
   order, exact entries/shares/queue totals, revisions, and the Policy cap. A
   future adapter must authenticate every page PDA/program owner, stream within
   measured account/compute limits, and atomically update the State root for
   contributions, withdrawals, queue changes, terminal allocations, claims,
   and page retirement.
2. **Live Facility Position asset codec/reload join.** Authenticate the real
   Position and Replay codecs, account ownership and facility authority, claim
   basis, mint/token profile and Hoard semantics; prove the idle and leased
   Position/Pot asset equations; and reload exact pre/post balances after every
   funding, Begin, Collect, Deliver, Finalize, abort, redemption, refund, and
   close operation. The pure semantic Position IDs are not asset evidence.
   Fresh Lease/Pot successors must also replace V1's legacy FeeBudget and
   LivenessBudget account bindings with exact selected fee-runtime artifacts
   and typed external liveness receipt/program/quote joins; V1 fields cannot be
   reinterpreted.
3. **Resolved payout and claim projection.** Join an authenticated canonical
   payout vector to native Egg redemption, exact resulting cash, one terminal
   Hamilton allocation over the authenticated page set, immutable page claims,
   order-independent delivery, resolution-work counts, and terminal retirement.
4. **Price-quantization certificate.** Select one policy above, freeze its
   canonical bytes, implement the checked certificate, and bind the Policy
   checker ID and generation-specific Lease/Pot certificate ID to the exact
   RelationV2 price scale, limits, dealer quote, and endpoint receipt.
5. **Measure and instantiate the presently funded liveness schedule.** Derive
   and measure exact Dealer action quotes plus the independent Source quote for
   maximum-width construction, collect/deliver/finalize/abort, retries,
   queue/close, resolution, redemption, every claim page, and counted
   retirement; admit the exact grouped work and separate rent into the external
   seven-account runtime without future fees, `K`, LP assets, or Hoard principal.
6. **SBF, rollback, and frozen-maxima evidence.** Allocate reviewed outer account
   and instruction tags, exact metas and PDA ownership checks, and a
   disabled-by-default capability/profile admission; authenticate/recompute
   Realm, MarketInstanceV2, RelationV2, candidate, price, zero-fee projection,
   quote, allocation, curve receipt, clocks, and every authority; implement
   atomic selection and non-persisted Finalize; prove rollback for every failed
   transfer, reload, cursor, count, and close; and run signed blank-validator
   scenarios at every frozen CU, stack, account-count, serialized-width, rent,
   retry, portfolio, virtual-leg, and page/row maximum with current-source
   reproducibility. A fresh State/root successor must also own the exact
   funded-dependency identity and count/retire its per-facility account; V1 has
   no such child class, and a permanent rent leak is not an acceptable default.

An initial adapter must remain explicitly zero-external-fee-only. Enabling
nonzero fees still requires the separate funding/custody/cursor/recipient/rebate
conservation contract described above. Passing pure tests or a local fixture
does not promote the runtime. Global capability promotion, release claims, and
public-cluster deployment remain separate decisions even after all six blockers
have evidence.
