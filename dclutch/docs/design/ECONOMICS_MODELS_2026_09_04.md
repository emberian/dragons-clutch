# The economics, modelled adversarially

Status: **the C-11 precondition, discharged for five knobs and stated as owed
for the sixth.** Written 2026-09-04 by the ECONOMICS lane under ruling D1 and
its amendment.

`docs/MASTER_COMPLETION_CONTRACT.md:96` is the only row with a precondition on
Ember before code may be written:

> *"Fee rates, beneficiaries, opener shortfall, upkeep vault and donation
> treatment are modeled adversarially and receive Ember's explicit economic
> rulings before implementation."*

This document is the adversarial half. Per knob it names **the adversary**, **the
invariant that bounds them**, and **the worst case on cohort-13, -14 and -15's
real numbers**. Every claim carries a `file:line` or a cohort citation. A number
with neither is a derivation, and it says so.

**What it is not.** It is not a proof that the protocol is well-designed. Four of
the five adversaries below are bounded to a number and one is bounded only by
disclosure, which is a real answer and a weaker one, and the document says so
where it is true.

## 0. The measurement basis, and why three rates

Rent is a cluster parameter and it moved under us. Devnet ran at **6,333
lamports a byte** through cohorts 13, 14 and 15's foundings
(`docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md:2147-2152`,
five widths, all at that rate) and then moved to **5,080** at the epoch-1141
boundary (`:1818-1820`), stranding 491,176 lamports of one funding ledger's
principal as surplus (`:1833-1837`, `:2491-2493`). The kernel's own fixtures
compute at **6,960** — `(128 + bytes) * 3480 * 2`, the mainnet default
(`crates/dclutch-claims-svm/src/claim_check_conservation_v1.rs`,
`rent_exempt_reference_v1`).

So every lamport figure below is given at the rate it was measured at, and where
a figure is derived it is given at all three. **A single quoted rent figure is
wrong within a day and this document does not print one.**

Two derived figures are corroborated against chain reads rather than left as
arithmetic: a four-outcome Position plus an admission record at the founding
rate is **5,877,024**, which is exactly what cohort-14's L7 census read twice
off chain (`COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:1698-1702`, *"the
-11,754,048 is 2 x 5,877,024 PDA rent"*); and the 288-byte Position at
**1,823,904** is `COHORT15:2152`'s own row.

---

## 1. A founder sets the fee to 500 basis points against their own market

**The adversary.** A founder who intends to trade against the strangers they
admit sets the venue rate at the top of the band and takes it from both sides of
every fill.

**The measured worst case.** All three cohorts founded at **50 bps per side**
(`COHORT13:268`, `COHORT14:281`, `COHORT15:799`) and the rate is **irreversible**
(`COHORT13:270`, `feeRateIsIrreversible true`). The rate is per side —
`fee = mul_div_floor(gross, POLICY_FEE_BPS, FEE_DENOMINATOR)`,
`seller_net = gross - fee`, `buyer_debit = gross + fee`
(`crates/dclutch-direct-aot-v3-contract/src/lib.rs:177-185`) — so on the
cohorts' own fill of gross 200:

| rate | fee each side | combined | share of gross | seller nets | buyer debits |
| --- | ---: | ---: | ---: | ---: | ---: |
| 50 bps/side, as founded | 1 | 2 | **1.0%** | 199 | 201 |
| 500 bps/side, at the band | 10 | 20 | **10.0%** | 190 | 210 |

The 50-bps column is the chain's, to the lamport (`COHORT14:532-534`,
`COHORT15:1259-1262`). The 500-bps column is the same arithmetic at the band.

**The invariant that bounds them.** `DIRECT_MAX_FEE_BASIS_POINTS_V1 = 500`,
enforced once at config construction — the path every founding reaches, because
the immutable record is built from that type
(`crates/dclutch-direct-codec/src/successor.rs:423`) — again on the Token-2022
setup wire (`token_setup_v1.rs:288-290`), again as a relation of the authored
transition program (`crates/dclutch-direct-aot-v3-contract/src/lib.rs:171`,
Lean `DirectOrdinaryV3.lean:527`). As of this ruling it is also the
**constitutional ceiling** of the governed record (§6): the effective cap is a
record field, the 500 is a source constant, and governance may narrow and can
never widen (`ProtocolParametersV1.lean`,
`the_fee_band_can_only_narrow`).

**And here is the honest part: no ledger law bounds the rate.** L1–L8 are
conservation laws. L7 says the fee payer's lamports moved by exactly the fees
its own transactions paid plus what landed in a watched account
(`tools/gauntlet/journey/src/ledger.rs:48-59`) — it checks that the fee moved
what the signed intents said, not that the rate was reasonable. **A 10% round
trip is conserved.** What bounds this adversary is the band, and inside the
band, disclosure: the rate is immutable at founding, shown on the market page
(`apps/dclutch-web/components/MarketActivity.tsx:234`), shown on every ticket
(`components/trade/TicketCard.tsx:109`, `:126`) and copied into the signed terms
where the compose form cannot change it (`trade/MakerOfferComposer.tsx:309`).

**Ruling D1 item 1 keeps it there**: no protocol take, no protocol beneficiary,
and the fee is a per-market fact the founder sets inside the band, swept by the
market's own `fee_recipient` by ordinary transfer. There is no protocol treasury
and no protocol sweep instruction — the SPL owner is set once at setup
(`programs/dclutch-trading-sbf/src/direct_token_setup_v1.rs:721-726`) and all
eight `DirectTokenAccountRoleV1::Fee` sites are setup or derivation, none a
withdrawal.

**Residual exposure, named:** a founder may found at 500 and a stranger may
trade there. Nothing refuses it and nothing should — the alternative is a
protocol that prices markets. The exposure is bounded to *disclosed*, and the
one thing that would make it undisclosed is a surface that renders a rate it did
not read.

---

## 2. A stranger cranks to farm the crank reward

**The adversary.** Someone with no interest in the market turns permissionless
cranks purely to collect their rewards, either by cranking repeatedly or by
manufacturing work to crank.

**The invariant that bounds them, and there are two, because there are two
shapes of crank.** `docs/design/FUNDED_CRANK_V1.md` §3.1 rules the distinction
and the tree implements both:

- **Closing routes** — the account is going away regardless — take
  `reward := min(floor, residual)`. The compaction crank is this shape
  (`claim_check_conservation_v1.rs:176`). Farming is unprofitable because the
  work cannot be manufactured cheaply: an escrow exists only if someone advanced
  its rent.
- **Surplus routes** — the account survives and the caller chooses the amount —
  take a **share**: `min(rent_floor, swept / 16)`
  (`crates/dclutch-rent-contract/src/lifecycle_v2.rs:519`,
  `LIFECYCLE_SWEEP_CRANK_SHARE_DIVISOR_V2`). Its own docstring states the attack
  a plain cap would admit: *"a cranker would sweep exactly the floor and take
  100% of it, repeatedly, and the wallet would receive nothing forever from a
  route that reads as funded"* (`:513-518`). A share makes it unprofitable **by
  construction rather than by threshold**, and the refund wallet keeps at least
  15/16 of every sweep.

**The measured worst case, closing route.** One full farm cycle at the cohorts'
own rate: the farmer opens an escrow (advancing 4,287,441 lamports for the
escrow record and its token vault), then cranks it and collects the reward plus
their own repayment.

| | lamports |
| --- | ---: |
| advanced to open | 4,287,441 |
| crank reward collected | 200,000 |
| repaid as opener | 3,042,496 |
| **net per cycle** | **−1,044,945** |

Strictly negative, and it stays negative at every rate, because the reward cap
is a fixed 200,000 while the outlay scales with rent. Farming this route is
paying 4.29M to harvest 0.2M.

**The measured worst case, surplus route.** Bounded by the share rather than by
a cycle: a cranker can take at most 1/16 of any sweep, and the sweep's
beneficiary is a creation-fixed wallet, so repeated sweeping converges to the
cranker having taken 1/16 of the credit rather than all of it.

**The one thing that is NOT bounded here, named as debt.**
`COMPACTION_CRANK_REWARD_LAMPORTS_V1 = 200_000`
(`crates/dclutch-claims-svm/src/claim_check_v1.rs:129`) is still a source
literal, which `FUNDED_CRANK_V1.md` §3 rules against: *"a literal needs a human
to notice; a `minimum_balance` call does not."* At the deployed rate it is 14.8x
below the closing-route floor the record `Abort` route derives, so it under-pays
rather than over-pays and the failure mode is a crank nobody turns rather than a
drain. §6's record is where it moves to; the runtime read is owed.

---

## 3. A donor routes principal into a fee vault

**The adversary.** A caller — or a future route written by someone who did not
read C-10 — moves Hoard principal into a fee vault, so that the collateral every
outstanding claim is redeemed against pays somebody's revenue.

**The measured worst case, before 2026-09-04.** Unbounded in principle and zero
in fact. The Custody Transfer wire admitted **64 of 81 ordered compartment
pairs** and `HoardPrincipal -> FeeVault` was among them (`WAVE.md:2900-2913`);
the contract *"does not enforce that and was never the place it lived"* — every
compartment rule lived in a calling program. The caller census found the
invariant true by enumeration: *"Nothing pins `HoardPrincipal -> FeeVault` —
both FeeVault-funding sites take `TradingPrincipal`"* (`WAVE.md:2927-2929`),
re-run at HEAD and still true, with `External` as the only other source and
every `HoardPrincipal` source paired with `External`, which is redemption.

Had a route been written, the size of the movement is the Hoard: **500,000,000
atoms** on each of cohorts 13, 14 and 15 (`COHORT13:1080`, `COHORT14:489`,
`COHORT15:351-353`), which is the whole collateral backing every outstanding
claim.

**And L1 through L7 would all have passed.** L1 is a collateral-closure sum, L5
a stage delta, L7 a lamport account: a transfer between two tracked accounts
balances every one of them. **Only L8 catches it** — *"a transfer between any
other pair of compartments passes every one of L1..L7, which is the
cross-subsidy C-10 exists to forbid. The class is DERIVED from the vault's own
PDA seeds, so this law cannot be satisfied by relabelling an account"*
(`tools/gauntlet/journey/src/ledger.rs:1004-1012`). L8 runs in the journey
harness. It does not run on chain.

**The invariant that bounds them, as of this ruling.** The pair is refused on
the wire, by name:
`dclutch_custody_contract::Error::ForbiddenCompartmentPair`
(`crates/dclutch-custody-contract/src/lib.rs`, the `Transfer` arm of
`CustodyRequestV1::validate`), surfaced on chain as
`CustodySbfError::ForbiddenCompartmentPair = 0x6011` rather than folded into
`Instruction`, and proved in Lean —
`hoard_principal_never_funds_the_fee_vault`, `only_the_named_pair_is_refused`,
`admissible_ordered_pairs_are_sixty_three`
(`formal/dclutch-semantics/DClutchSemantics/CustodyAbi.lean`). The census that
recorded 64 now records **63** and is red on 64, both in Rust and under
`native_decide`.

**Exactly one pair moved.** `FeeVault -> HoardPrincipal` — fees capitalizing a
Hoard — is a different movement with a different argument and is deliberately
NOT ruled here.

---

## 4. A closer inflates the donation slice

**The adversary.** The permissionless closer of a maker replay makes the
donation slice bigger so their carve is bigger, or donates into the account
themselves to harvest the carve.

**The invariant that bounds them.** The slice is not a caller input. It is
`observed_lamports - maker_root.rent_principal`, both read out of program-owned
state — the account's own balance and the principal the replay recorded at first
use. The carve is then `min(cap, donation)`, and:

- `the_closer_carve_never_touches_principal` proves `rentPrincipal <=
  totalCredit` for **every cap a caller can pass**, so the recorded `rent_owner`
  always receives at least everything the maker put in
  (`formal/dclutch-semantics/DClutchSemantics/DirectSuccessor.lean`);
- `the_closer_carve_is_capped_and_bounded_by_the_donation` proves the carve is
  under both bounds;
- the receipt refuses a carve larger than the donation it names, by exact
  discriminant (`crates/dclutch-direct-codec/src/close_maker_v1.rs`,
  `a_receipt_carving_more_than_the_donation_refuses`).

**Self-donation is never profitable.** A closer who donates `d` receives
`min(cap, d) <= d` back and pays a transaction fee. At `cap >= d` it is
break-even minus fees; at `cap < d` it is a strict loss. **The cap is therefore
the entire exposure**, and it is bounded by the funded-crank floor, which is
what ruling D1 item 4 says.

**The measured worst case: zero, and this is the finding.** No cohort has ever
carried a donation. Cohorts 13, 14 and 15 record no observed
`unclassified_donation` figure at all; cohort-13's census declares `unclassified
+0` and it HOLDS (`COHORT13:1086`, `:1089`), cohort-14's fill boundary reads the
same (`COHORT14:495`), and cohort-15 states only that widening a check *"admits
a real donation as custody; the recorded figure does not"* (`COHORT15:1866-1869`)
with no amount. **So the donation slice funds nothing today even if the cap
moved**, and a closer reward carved from it would be zero on every market this
protocol has ever founded.

That is not an argument against the carve — the shape has to exist before a
donation can find a home, and refusing donations was rejected for good reason
(anyone can transfer one lamport into a Trading-owned PDA, so a refusal lets a
griefer strand any replay and the market behind it, permanently, for nothing —
`docs/design/COHORT9_CLOSEMAKER_RULINGS_2026_08_31.md:43-62`). It is an argument
that **the carve is not a liveness mechanism**, because a mechanism whose
funding source has never carried a lamport cannot be relied on to make a crank
turn. Whatever pays the closer in practice has to come from somewhere the
protocol has actually seen money.

**And the frame cannot pay one today, which is a separate fact from the
ruling.** `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs` refuses ANY
signer, so there is no closer account in its twenty-two-account frame. A
twenty-third account with a signer conjunct is a released AccountProfile change
— descriptor digest, derived identities, a re-found. Named as owed, not left as
a zero a reader would take for a policy.

---

## 5. An opener who is the only cranker

**The adversary.** Nobody is adversarial here. This is the case where the
protocol takes money from a cooperative party, which is why it needed a ruling.

**The mechanism.** The opener advances rent for the escrow record and its token
vault. The first compaction sweeps the Position and the admission record, pays
the new claim check's own rent, pays **the cranker**, and repays the opener out
of what is left. The order departs from the design's, deliberately, and the
kernel argues for it in its own words: *"the first crank would pay itself
exactly nothing. An unfunded crank is an unturned crank"*
(`claim_check_conservation_v1.rs:124-138`).

**The measured worst case, at all three rates, four outcomes:**

| rate | opener advances | first crank repays | **still owed** | as SOL | of their own advance | of a whole market lane |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 6,960 (kernel reference) | 4,711,920 | 3,363,520 | **1,348,400** | 0.001348400 | 28.6% | 0.59% |
| 6,333 (the cohorts' founding rate) | 4,287,441 | 3,042,496 | **1,244,945** | 0.001244945 | 29.0% | 0.54% |
| 5,080 (devnet after epoch 1141) | 3,439,160 | 2,400,960 | **1,038,200** | 0.001038200 | 30.2% | 0.45% |

The lane denominator is **0.230228002 SOL**, cohort-14 market C's whole cost for
a founded, activated, filled and armed market (`COHORT14:1741-1750`).

**Cohort-9's recorded figure reproduces.** That ruling recorded 1,348,376
lamports (`WAVE.md:1499-1504`); re-derived at HEAD's widths and the kernel's
reference rate it is 1,348,400 — **within 24 lamports**, so the number the
ruling was made on is the number the code still computes.

**Note what the fraction is NOT.** It is not a fraction of the market's
collateral. The collateral is 500,000,000 atoms of a six-decimal mint and the
shortfall is in lamports; L8 exists precisely to forbid netting one against the
other (`ledger.rs:1004-1012`), and a document that divided them would be
performing the cross-subsidy the protocol refuses. The two honest denominators
are the opener's own advance and the market's own lamport cost, and both are
above.

**The invariant that bounds them.** The conservation conjunct: everything the
two closing accounts held is accounted for by exactly four credits
(`claim_check_conservation_v1.rs:190-197`), so the close can neither strand a
lamport nor pay out more than it held. And the debt does not vanish: it carries
in the escrow record as `opener_debt` (`:99-100`) and discharges progressively
(`:181-183`, `:199-203`), with a two-crank discharge test at `:653-661`.

**Ruling D1 item 2 accepts it and requires it stated**, and it now is: the
founding wizard's funding step and the market page's retirement drawer both
carry the sentence and the number, computed from the cluster's own rent
minimums rather than quoted (`apps/dclutch-web/lib/openerTerms.ts`,
`components/OpenerFirstCrankTerms.tsx`), with a source gate pinning the widths,
the cap and the ORDER against the Rust (`lib/openerTerms.test.ts`).

**What is still owed on this knob.** The residue half — escrow-close residue
after `opener_outlay` is serviced — is unimplemented and routed to the vault
that does not exist (`docs/design/UPKEEP_VAULT_V0.md:42`, `:113-115`). Until it
lands, a single-crank market's opener eats the whole figure above.

---

## 6. The governed parameter surface

Ruled 2026-09-04, amended: *"a GOVERNABLE parameter surface so we are not stuck
like that."*

**The problem it solves.** Every value in §1 through §5 lives as a literal in a
crate. A literal is a fine way to hold a value nobody intends to move, and a bad
way to hold a value everybody knows will move once, at mainnet — because the
only procedure it admits is *ship a new program*, which is also the procedure
for changing what the program MEANS. The two kinds of change become
indistinguishable to anyone reading a release.

**The shape: constitution and statute.** A band is a source constant emitted
from Lean; moving it needs an ELF and a release. A parameter is a record field;
moving it needs the authority, a proposal and a wait.

| field | genesis | band | what a release owns |
| --- | --- | --- | --- |
| `governance_authority` | the deployer key, a named placeholder for cohort-16 | zero is legal and means FROZEN FOREVER | — |
| `protocol_beneficiary` | zero — there is none | zero exactly when the take is zero | — |
| `max_fee_basis_points` | 500 | `<= 500` | the 500 |
| `protocol_take_basis_points` | 0 | `<= max_fee_basis_points`, and nonzero iff a beneficiary is named | — |
| `closer_carve_basis_points` | 10,000 (the whole donation slice) | `<= 10,000` | — |
| `closer_reward_cap_lamports` | 0 | — | — |
| `crank_reward_cap_lamports` | 200,000 | — | — |
| `change_delay_slots` | 1,512,000 (seven nominal days) | `>= 1,512,000` | the floor |
| `generation`, `activation_slot` | 0 | bookkeeping the apply writes | — |

Three properties are worth stating separately because they are what make this a
constitution rather than a knob rack:

1. **The fee band only narrows.** Decision 0014 D2's 500 stops being *the* value
   and becomes the *bound on* the value, so a holder who read the ELF knows the
   worst case without reading the record.
2. **A take and a payee move together, in both directions.** Ruling D1 item 1
   becomes one fact instead of two that could drift apart, and a take with no
   payee is unrepresentable rather than merely absent.
3. **The freeze is one-way.** A zero authority refuses every proposal and nothing
   in the module writes an authority. A deployment that wants immutable
   economics sets it to zero and the record is finished. A reversible freeze is
   not a freeze.

**The change procedure.**

1. **`propose(signer_is_authority, proposed, current_slot)`** — the authority's
   act. Refuses `GovernanceFrozen` on a zero authority, `UnauthorizedGovernance`
   on any other signer, `ProposalOutstanding` if one already stands, and
   `ParameterOutOfBand` if the proposed value violates any band. Writes the
   proposed body's SHA-256 and `earliest_apply_slot = now + change_delay_slots`.
2. **`withdraw(signer_is_authority)`** — the authority's act, and nobody else's.
3. **`apply_change(proposed, current_slot)`** — **permissionless**. A governed
   change is still a crank: an authority that had to show up twice could propose
   and then decline to finish, leaving the record in a state only it can leave.
   Refuses `NoPendingProposal`, `ProposalNotMatured` inside the delay,
   `ProposalDigestMismatch` on substituted bytes, and `ParameterOutOfBand`
   **again** — because a release landing between the two acts may have narrowed
   the constitution under a proposal that was legal when it was made. A proposal
   is a commitment to a value, never a grant of permission to install it.

**The event a census reads.** One 112-byte receipt per applied change:
`previous_digest`, `new_digest`, `generation`, `proposed_at_slot`,
`activation_slot`, `delay_slots`. The three slot numbers close on their own, so
the notice period is checkable from the receipt alone without reconstructing the
record's history — and a receipt whose numbers do not close refuses on decode.
The generation advances by exactly one per change, so the stream is a total
order with no gaps and no repeats.

**Why seven days, derived rather than chosen.**
`COMPACTION_DEADLINE_SLOTS_V1 = 38_880_000` and the comment calling it a
hundred-and-eighty-day wait is only true at 216,000 slots a day — so that rate
was already load-bearing somewhere nobody had written it down. Written down and
checked (`compaction_deadline_is_one_hundred_eighty_nominal_days`), a second
constant derives from it: seven nominal days, which governance may lengthen and
can never shorten, not even by governing its own delay first.

**Where it is.** Lean first for the layout and the procedure —
`formal/dclutch-semantics/DClutchSemantics/ProtocolParametersV1.lean`, with the
three ruled hostiles plus `applied_parameters_are_in_band` (the property the
hostiles serve), `the_fee_band_can_only_narrow`,
`a_take_and_a_payee_move_together`,
`every_applied_change_advances_the_generation`,
`a_proposal_carries_at_least_the_minimum_notice`,
`frozen_governance_refuses_every_proposal`, and four decided witnesses so that
none of the implications is a tautology over an empty domain. Rust twin
`crates/dclutch-protocol-parameters-contract`, generated constants emitted by
`EmitProtocolParametersV1Rust.lean` behind the same `check-generated.sh` gate
Custody uses, twelve hostiles each at its exact discriminant with a control one
value away.

**What is owed.** The runtime read. `DIRECT_MAX_FEE_BASIS_POINTS_V1` and
`DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1` now project the record's genesis instead
of restating it — one author, byte-identical values — but no adapter yet reads
the ACCOUNT. That is a program lane's work: a PDA at
`dclutch:protocol-parameters:v1`, a dispatcher owning the refusal sub-band, and
each consumer taking the value from the frame instead of the constant.

---

## 7. The upkeep vault: charter, as the record a later lane builds

Ruled 2026-09-04: the vault is **wanted** and chartered as
`docs/design/UPKEEP_VAULT_V0.md` sketches it. That document is the argument;
this section is the record shape, so the lane that builds it is not re-deciding.

### 7.1 The three invariants, as testable predicates

- **I1 no involuntary inflow.** Admissible inflows are exactly: residues a
  ruling would otherwise send nowhere, explicitly-ruled donation slices, and
  voluntary deposits. *Testable as:* every credit instruction names its source
  class from a closed enum, and the enum has no member reachable from a trade
  fee, a rent principal or a recorded receivable.
- **I2 no discretionary outflow.** *Testable as:* the record carries **no
  authority field at all** — its absence is the charter, not a zero — and the
  only debit route pays a published price on a receipted state change.
- **I3 full legibility.** *Testable as:* `inflow_total - outflow_total` equals
  the account's lamports above its own rent minimum, at every boundary, as an
  L8-shaped conservation row.

### 7.2 The record

`UpkeepVaultV1`, one PDA, no authority.

| field | why |
| --- | --- |
| `bump` | the record authenticates its own address |
| `release_set` | which release's price table is in force; a price change is release content, never a parameter someone turns |
| `price_table_digest` | SHA-256 of the released price rows, so a payout names the table it was paid under |
| `inflow_total`, `outflow_total` | I3, and the only two running numbers |
| `paid_receipt_digests` cursor | idempotency: one payout per receipted state change, never two |

There is deliberately **no** `authority`, **no** `pending_change`, **no** spend
instruction. The parameter record of §6 is governable because its values are
policy; this record is not, because its values are *prices for verifiable work*
and a governed price is a discretionary payout wearing a schedule.

### 7.3 Inflows, measured

| source | today | measured on cohorts 13/14/15 |
| --- | --- | --- |
| **stranded rent from a cluster rate change** | goes nowhere | **491,176 lamports**, cohort-15, one funding ledger, when devnet moved 6,333 → 5,080 (`COHORT15:1833-1837`, `:2491-2493`) — a residue with no home, exactly I1's class |
| **`CloseMakerReplay` donation slice** | credits the recorded `rent_owner` in full; carve capped at 0 | **zero observed, on every cohort** (§4). The slice has never carried a lamport |
| **escrow-close residue after `opener_outlay`** | unimplemented | **1,244,945 lamports per single-crank market** at the cohorts' rate is what the opener currently eats instead (§5) |
| **seat-prepay reimbursement** | no route reimburses the prepayer | **2,786,520 per seat**, `rent.minimum_balance(312)` to the lamport (`COHORT13:1393-1394`, `COHORT14:382`, `COHORT15:366`); at least four seats across three cohorts = **11,146,080 lamports**, and cohort-14's still sits funded and unconsumed (`COHORT14:763`) |
| **voluntary deposits** | n/a | none |
| **compaction dust residues** | goes nowhere | none observed |

**The finding this table produces.** The two inflows the vault was designed
around — the donation slice and compaction dust — have carried **nothing** in
three cohorts. The two that have carried real money are the **seat prepays**
(11.1M lamports of rent nobody can reclaim) and the **rate-change residue**
(491,176 lamports reclassified as surplus with nowhere to go). A vault sized on
donations would be empty; a vault fed by the two measured classes would have
about **0.0116 SOL** after three cohorts, which finances roughly nine compaction
cranks at the current cap.

That is a small number and it is the right number to know before building: the
vault's honest empty-state story (*acts wait, degradation is safe, top-up is
always open, and the site shows the balance* — `UPKEEP_VAULT_V0.md`'s Serum
lesson) is not a caveat, it is the expected operating condition.

### 7.4 Outflows

Each a fixed price published on chain, paid only on the receipted completion of
a named act, with the price **derived from Rent** and never a source literal
(`FUNDED_CRANK_V1.md` §3): crank rewards where swept rent cannot cover them;
ceremony and upkeep reimbursement at cohort cuts — including the seat prepays
above, which is the class with actual money in it; oracle observation posting;
ZeroBump-class recovery bounties.

### 7.5 The adversary the review asked for

- **Can an involuntary flow be laundered in?** Only by a credit instruction, and
  the closed source-class enum is what to attack. The test is that no reachable
  caller can present a trade fee or a rent principal as a residue.
- **Can a route be made to pay twice for one state change?** The
  `paid_receipt_digests` cursor is the answer and the thing to attack: a payout
  keyed by the receipt digest of the state transition it completed, refused if
  that digest has been paid.
- **The empty-vault story.** Covered above: it is the expected state, not the
  failure state.

---

## 8. What C-11 can now show, and what it still waits on

**Discharged, with evidence:**

| the row's word | where |
| --- | --- |
| fee rates — modelled adversarially | §1, with the worst case at the band on the cohorts' own fill, and the honest statement that no ledger law bounds a rate |
| beneficiaries — modelled | §1: no protocol treasury, no sweep instruction, per-market `fee_recipient`, and the take/payee pair rule making D1 structural |
| opener shortfall — modelled AND ruled AND stated in the terms | §5, three rates, cohort-9's figure reproduced to 24 lamports, and the sentence on two surfaces computed from the cluster |
| donation treatment — modelled AND the carve ruled | §4, with the measured finding that the slice has never carried a lamport |
| upkeep vault — chartered as a record shape with measured inflows | §7 |
| *(new)* the governed parameter surface | §6, Lean-first, twelve hostiles |
| *(new)* the lamport half of the economic-flow sweep | §3: `HoardPrincipal -> FeeVault` refused on the wire, 64 → 63 |

**Waiting, and on what:**

1. **The upkeep vault's build.** A separate lane, per the ruling. §7 is its
   charter.
2. **The runtime parameter read.** The record exists and its consumers project
   its genesis; no adapter reads the account yet (§6, *what is owed*).
3. **The closer-reward route.** The carve and cap are kernel law; the frame
   admits no closer to pay (§4). A twenty-third account and a signer conjunct,
   on a cohort cut.
4. **`COMPACTION_CRANK_REWARD_LAMPORTS_V1` as a literal.** §2, named as debt.
5. **The escrow-close residue.** §5, unimplemented, and until it lands the
   single-crank opener eats the whole shortfall.

## 9. The frame cost, measured, and the ratchet left red

`tools/frameguard/run.sh --at 2812fc00` against baseline `a062dc65`, twelve
links, 1,863 rows. **Exactly one row is this lane's:**

| link | function | before | after | delta |
| --- | --- | ---: | ---: | ---: |
| `dclutch-trading-sbf` | `direct_close_maker_v1::process_direct_close_maker_v1` | 3,008 | **3,072** | **+64** |

That is the closer-carve argument plus the receipt's eight extra bytes, and it
leaves 1,024 bytes of headroom under SBPF v0's 4,096-byte wall. The custody
sweep cost nothing: `8ed7f242` predates the baseline's own commit, so its rows
are already captured.

**The ratchet is left RED, deliberately and by the rule
`tools/frameguard/README.md` states.** The same capture carries eleven rows this
lane did not write — `process_relay_transport_v1` +64 and four new
resolution-proof functions from RECOVERY, five zero-frame Series functions from
SERIES — and `frameguard.py owed` names all three lanes. An exact ratchet cannot
be recaptured by a bystander: admitting eleven rows to land one is the mistake
that document records being made three times on 2026-09-02. The recapture
belongs to whichever lane can hold a quiet tree long enough to take two captures
at one commit.
