# Decision 0024: the five economic knobs, and a parameter surface that can be governed rather than recompiled

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible
on request) — the five knobs ruled by the orchestrator on 2026-09-04 under
ember's standing goal, AMENDED by ember at 10:15 EDT with a sixth item the
orchestrator had not asked for, and reversible at the cost §7 states**. It was
PROVISIONAL from the ruling until 15:50 EDT, when ember read the docket and
accepted it in conversation without amending it; the confirmation line below is
the whole of what was said. The rulings are docket item D1; ember's amendment
is recorded at `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4652-4653`. This is C-11's row
(`docs/MASTER_COMPLETION_CONTRACT.md:96`), the only row in the contract that
names a precondition on ember before code may be written. Item 5 landed the
same morning at `8ed7f242f` (lane ECONOMICS); the rest is the ECONOMICS lane's
charter.

**Confirmed, 2026-09-04 15:50 EDT.** Ember, after reading the docket and the
mechanism cohort page:

> you aren't waiting on me for rulings are you? i was reading the docket and
> contemplating it, but overall find your takes reasonable

The orchestrator's reply: nothing was waiting on ember — the rulings were
provisional and already in force, and the lanes had been working under them
since they were made; *"overall find your takes reasonable"* is taken as
confirmation rather than as an invitation to re-argue them; and the one thing
still genuinely ember's is the flagship conditional market's feature gate, its
slot and its metric (decision 0029's tenth item). So the status above is
CONFIRMED and no longer PROVISIONAL: accepted in conversation, unamended, and
reversible on request at the cost §7 states.

## 1. The question

C-11 requires that *"Fee rates, beneficiaries, opener shortfall, upkeep vault
and donation treatment are modeled adversarially and receive Ember's explicit
economic rulings before implementation"*
(`docs/MASTER_COMPLETION_CONTRACT.md:96`; register row `:186`, *"open; Ember
owns each economic choice"*).

The tree had already built past that gate in four of the five, deliberately: it
built a shape it can defend and left the value knob at a provable default —
`DIRECT_MAX_FEE_BASIS_POINTS_V1 = 500`, `DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1 =
0`, the opener repaid second — so that a ruling is a one-constant change riding
a later ELF rather than a redesign. The precondition holds *literally* for the
upkeep vault only, which has zero code.

## 2. The ruling

**Five items, as put to ember and taken.**

1. **No protocol fee take before mainnet, and it is revisited with the mainnet
   ruling.** Decision 0014's D1 stands as built rather than as ratified: the
   band `DIRECT_MAX_FEE_BASIS_POINTS_V1 = 500` with no lower bound
   (`crates/dclutch-direct-codec/src/successor.rs:68`, enforced once at `:400-402`
   and independently on the Token-2022 setup wire at
   `token_setup_v1.rs:286-290`), 50 bps as the bootstrap's chosen rate and no
   longer a refusal (`token_setup_v1.rs:26-32`), charged **per side**, so 100
   bps of gross on a single fill
   (`crates/dclutch-direct-aot-v3-contract/src/lib.rs:177-185`), rounding toward
   the makers and never toward the venue
   (`tools/gauntlet/direct/expectations.json:6`).
2. **No protocol beneficiary.** `fee_recipient` stays a required per-market key
   in the immutable config (`successor.rs:78-82`, `:336-350` refusing the zero
   key), swept by an ordinary SPL transfer from a Trading-owned PDA whose SPL
   owner it is; there is no treasury, no protocol sweep instruction, and gen-1's
   `REVENUE-TREASURY-UNSET-SENTINEL1` is not carried into this generation
   (`docs/decisions/0014-the-fee-rate.md:134-137`;
   `docs/design/FEE_SECOND_TRANSACTION_V1.md:226`).
3. **The crank-first order stands, and the terms state it.** The lamport order
   in `crates/dclutch-claims-svm/src/claim_check_conservation_v1.rs:124-138`
   departs from the design's stated order on purpose, because paying the opener
   first *"does not close arithmetically: … the first crank would pay itself
   exactly nothing. An unfunded crank is an unturned crank."* The consequence is
   accepted as the cost of opening and **disclosed** rather than discovered:
   multi-crank markets repay the opener progressively through `opener_debt`
   (`:99-100`, `:181-183`, carry-forward `:199-203`), single-crank markets never
   repay them at all, and the market's terms say so. This answers the ruling put
   at `docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:1499-1504`.
4. **The upkeep vault is chartered**, exactly as
   `docs/design/UPKEEP_VAULT_V0.md` sketches it and no wider: one protocol-owned
   lamport PDA with **no authority** (`:10-11`), no involuntary inflow (`:19-23`),
   **no discretionary outflow** — *"There is no spend instruction, no authority,
   no vote. The vault cannot be spent; it can only be earned from"* (`:25-30`) —
   full legibility (`:32-34`), and the four named sources only (`:38-43`). Trade
   fees, rent principals and recorded receivables are explicitly **not** inflows
   (`:45-48`). It is the only home the tree has for compaction dust that today
   *"goes NOWHERE"*, for escrow-close residue after `opener_outlay` is serviced,
   and for the certificate-seat prepay nothing reimburses
   (`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:763`).
5. **The closer's reward is carved from the donation slice alone, capped at the
   funded-crank floor.** `DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1` moves off zero,
   and the carve may touch only the `unclassified_donation` half of the observed
   balance — never principal, never the immutably recorded `rent_owner`'s
   entitlement (`docs/ledger/COHORT9_CLOSEMAKER_RULINGS_2026_08_31.md:43-62`).
   The cap is the funded-crank floor, which is **derived from the Rent sysvar
   and never written as a source literal**
   (`docs/design/FUNDED_CRANK_V1.md` §3 — *"the ruling with the widest blast
   radius"*). Refusing a nonzero donation was rejected and stays rejected:
   *"anyone can transfer 1 lamport into a Trading-owned PDA, so the refusal
   would let a griefer strand any replay (and the market behind it) permanently
   for ~nothing."*
6. **The lamport sweep closes `HoardPrincipal → FeeVault`.** The atom census had
   recorded honestly that the pair was one of sixty-four shape-admissible
   ordered pairs on the Custody Transfer wire and that *"nothing pins"* it
   (`docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:2900-2936`) — an invariant holding only because nobody had written
   the violating caller.

## 3. Ember's amendment

Ember, on the docket at 10:15 EDT 2026-09-04, recorded at `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4652-4653`:

> D1 — the upkeep vault is wanted; crank-first fine but measure the first
> crank; **a governable parameter surface so we are not stuck (prototype the
> policy we intend to deploy)**

Three things move because of it, none of which the orchestrator's five items
contained:

- **The vault is wanted, not merely permitted.** Item 4 was framed as *"charter
  it, or rule that residues go nowhere"*; ember took the charter. It is a
  build, not an option left open.
- **Measure the first crank's cost.** Crank-first is accepted *conditionally on
  a measurement that does not exist yet*: what the first crank actually costs
  the opener on a real route, not the cohort-9 figure of 1,348,376 lamports
  short quoted in the docket. The disclosure in the terms should carry the
  measured number.
- **A governable parameter surface.** Fees, the closer's carve, the crank reward
  and the protocol take are **constants in the ELF today**, so every one of the
  five rulings above is a redeploy to revisit. The amendment asks for one
  protocol-parameters record with **a named authority, a change delay, and a
  census-readable event**, with every consumer reading the record rather than a
  constant — *"prototype the policy we intend to someday deploy"*, so that a
  value chosen at devnet does not become a value we are stuck with. Today its
  authority is the deployer key, named as a placeholder rather than pretended to
  be governance.

The amendment does not reverse any of the five. It changes what "ruled" costs:
a ruled value becomes a record field with a delay and an event, not a literal.

## 4. The lanes implementing it

**ECONOMICS**, amended with the vault charter, the crank-cost measurement and
the governable record (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4657-4659`). Item 6 is already landed by that
lane at `8ed7f242f`. The adversarial model comes first for each ruled value, then
the one-constant changes; the parameter record makes "one-constant" mean one
record field. RECOVERY (decision 0027) shares the funded-crank floor, because the
ladder's permissionless advance is paid by the same mechanism.

## 5. The hostiles and laws that guard it

**The one already landed.** `8ed7f242f` refuses the pair by name and states the
count: `CustodySbfError::ForbiddenCompartmentPair = 0x6011`, the next free code
in the registered band, **Lean first** — `transferPairAdmissible` in
`CustodyAbi.lean` with `hoard_principal_never_funds_the_fee_vault`,
`only_the_named_pair_is_refused` (every other live pair still crosses, including
the opposite direction, which is a different movement with a different argument
and is *not* ruled here) and `admissible_ordered_pairs_are_sixty_three` over the
same nine-compartment enumeration the Rust census walks. The count was
**red-proved**: stated as 64 it fails `native_decide` by name. The hostile flips
one byte — the source compartment tag — on the wire every FeeVault site actually
uses, and names the discriminant; *"a bare `is_err()` here would have accepted
the length refusal, the magic refusal and every shape conjunct."* Two refusals,
two words: a `None` side is `InvalidOperationShape`, and a wire that decodes
perfectly and is refused on economic grounds is neither that nor `Instruction`.

**The standing laws these rulings live inside.** L8, per-class conservation
(`tools/gauntlet/journey/src/ledger.rs:1004-1012`): *"every compartment class
moved by exactly the amount its stage declared … The class is DERIVED from the
vault's own PDA seeds, so this law cannot be satisfied by relabelling an
account."* The capitalization-class refusals
(`tools/activity-properties/activity_properties.py:309-310`, `:661-662`), which
refuse a settlement that does not declare
`debtor-collateral-obligation-not-future-revenue-or-hoard`. The fee band's two
independent enforcements. Physical tag distinctness
(`crates/dclutch-custody-contract/src/lib.rs:1911-1916`). And the Direct fee leg
that moves `External → External` and never crosses a `FeeVault` compartment at
all (`crates/dclutch-direct-codec/src/fee_settlement_v1.rs:412-413`).

**What still needs one.** The upkeep vault's I2 — *no spend instruction* — is a
property of a program that does not exist yet, so it needs a hostile that tries
to spend and is refused by name, not a docstring. The closer's carve needs a
hostile proving the cap binds against the Rent-derived floor rather than a
literal. The parameter record needs one proving the change delay cannot be
skipped and that a consumer reading a stale record refuses rather than
transacting.

## 6. What was given up, named

**No revenue this generation.** Items 1 and 2 together mean the protocol earns
nothing from any market on devnet and nothing on the first mainnet market
either, and the upkeep vault is deliberately *"Not a fee switch, and never
fundable by one"* (`UPKEEP_VAULT_V0.md:95-103`). Whoever does permissionless
work is paid from residues, donations and voluntary deposits, or is not paid.

**The single-crank opener is not made whole.** Item 3 accepts that, and pays for
it with disclosure rather than with lamports. The alternative was named and
refused because it makes the first crank unpaid and therefore unturned.

**The parameter surface is a new authority.** A record with a named authority is
a thing that can be captured, and the tree has spent this generation removing
authorities. The amendment's own containment is that the authority is *named as
a placeholder*, the delay is on-chain, and the event is census-readable — the
prototype is of the governance we intend, deliberately visible rather than
convenient.

## 7. The cost of reversal

**Items 1 and 2 (a protocol take, a protocol beneficiary).** `fee_recipient`
lives in the per-market **immutable** config, refused at zero
(`successor.rs:78-82`, `:336-350`). Introducing a protocol beneficiary is not a
constant change; markets carrying the old config cannot adopt it, so it is a
**re-found**, at the disposability regime's price (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:1443-1450`).

**Item 3.** Reversing to opener-first re-creates the arithmetic the contract's
own docstring refutes: the first crank pays itself nothing, so it is not turned,
so the escrow does not advance. The reversal is not a policy change; it is a
liveness defect with a measured proof already in the tree.

**Item 4.** Un-chartering the vault sends compaction dust, escrow-close residue
and the certificate-seat prepay back to *nowhere* — the state
`UPKEEP_VAULT_V0.md:38-43` names — and leaves the tree with no answer for the
2,786,520-lamport seat that sits funded and unreimbursed after every cohort.

**Item 5.** Returning the carve to zero re-strands every replay a griefer
donates one lamport into, which is the vector the closer's reward exists to
close. Raising it *above* the funded-crank floor spends donations as if they
were a fee, which item 1 refused.

**Item 6.** Un-refusing the pair returns C-10's forbidden movement to
shape-admissible on the wire, guarded only by the accident that no caller has
written it — the exact state the census said an invariant quietly stops being
true in. Three Lean theorems and the sixty-three count would have to be
withdrawn, not just a `match` arm.

**The amendment.** Once consumers read the parameter record, going back to
constants is an ELF change *plus* a record retirement, and every value that was
governable becomes a redeploy again — which is precisely the "stuck" the
amendment was made to avoid.

## Evidence pointers

`docs/MASTER_COMPLETION_CONTRACT.md:96`, `:186`; `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4652-4659`;
`docs/decisions/0014-the-fee-rate.md` (whole, esp. `:7-13`, `:134-137`);
`docs/design/UPKEEP_VAULT_V0.md:3`, `:10-11`, `:19-34`, `:38-48`, `:52-62`,
`:95-103`, `:112-119`;
`docs/ledger/COHORT9_CLOSEMAKER_RULINGS_2026_08_31.md:43-62`;
`docs/design/FUNDED_CRANK_V1.md` §0 items 2-3, §3;
`docs/design/FEE_SECOND_TRANSACTION_V1.md:40`, `:44`, `:50`, `:226`;
`crates/dclutch-claims-svm/src/claim_check_conservation_v1.rs:99-100`,
`:124-138`, `:181-203`, `:653-661`;
`crates/dclutch-direct-codec/src/successor.rs:68`, `:78-82`, `:336-350`,
`:400-402`; `crates/dclutch-direct-codec/src/token_setup_v1.rs:26-32`,
`:286-290`; `crates/dclutch-direct-aot-v3-contract/src/lib.rs:177-185`;
`crates/dclutch-custody-contract/src/lib.rs`,
`formal/dclutch-semantics/DClutchSemantics/CustodyAbi.lean`,
`programs/dclutch-custody-sbf/src/lib.rs` at `8ed7f242f`;
`tools/gauntlet/journey/src/ledger.rs:1004-1012`;
`docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:1499-1504`, `:2900-2936`;
`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:763`.
