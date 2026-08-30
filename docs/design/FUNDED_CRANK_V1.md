# Funded cranks — one shape for every permissionless act in the tree

Status: **ruling, and a template.** This document changes no byte of program
code. It exists so that the fourth, fifth and sixth conversion of a
`permissible` route into a `live` one does not re-decide what the first three
already decided, differently.

Census row **Y1** / **Y2**, pattern **P1**
(`docs/evidence/LIVENESS_CENSUS_2026_08_29.md:95-96,332-348`).

Every claim below carries **verified-from-source** (read at HEAD with
`file:line`) or is marked **ruling** (a decision this document makes). A claim
with no `file:line` is a ruling, not a finding.

Paths are `~/dev/dclutch` unless stated otherwise.

---

## 0. What this document decides

1. There is **one** funded-crank shape, and it is the deployed record `Abort`
   route's, not the crate-only candidate escrow's (§1).
2. The reward floor is **derived from the Rent sysvar, never written as a
   source literal** (§3). This is the ruling with the widest blast radius and
   the tree currently violates it in one place.
3. There are **two** legitimate funding sources — *prepaid-at-creation* and
   *residual-at-close* — and which one a site gets is determined by a test, not
   by taste (§2).
4. The expiry comparison is **inclusive** (`>=`). The tree currently spells it
   both ways (§4).
5. Lamport arithmetic lives in a **contract crate as a plan struct that refuses
   to exist unless it balances**, never inline in the program (§5).
6. The caller **signs only to own the reward**, never to be authorized (§6).
7. What a converted route must prove before it counts as GREEN (§7).

---

## 1. The template is record `Abort`, not the candidate escrow

The census names `WorkRewardV1`
(`crates/dclutch-general-adapter-contract/src/candidate_v1.rs:237-249`) as the
canonical GREEN shape, and as a *description of the property* it is exactly
right — its own doc comment states the doctrine better than this document can:

> the compartment has already been debited by the transition that produced it,
> so a caller that drops it has taken work and withheld the payment its own
> record now says was made. (`candidate_v1.rs:239-242`)

and

> The solver signs, but only to own the escrow and its refund -- not to be
> authorized. Anyone may submit. (`candidate_v1.rs:297-298`)

**But it has no deployed SBF dispatcher.** Its only non-crate consumer is a
harness test
(`programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs`).
This was found and stated first by the claim-check design
(`docs/design/CLAIM_CHECK_COMPACTION_V1.md`, §6.2), and this document adopts
that call rather than re-litigating it.

The tree's one **deployed** funded permissionless post-deadline crank is record
`Abort`:

| part | where |
|---|---|
| transition + conservation | `crates/dclutch-record-contract/src/lib.rs:1699-1730` |
| conservation struct | `crates/dclutch-record-contract/src/lib.rs:1418-1471` |
| the prepay force at Begin | `crates/dclutch-record-contract/src/lib.rs:417-419`, `:539-540`, `:1251-1252` |
| chain-derived floor | `programs/dclutch-registry-sbf/src/record_v1.rs:348,362` |
| dispatcher | `programs/dclutch-registry-sbf/src/record_v1.rs` |

Read that route before writing a new one. Its five properties are §§2-6.

---

## 2. Two funding sources, and the test that picks one

**Ruling.** A crank is funded either by lamports *prepaid at the value's
creation* into a compartment sized for the cranks its lifecycle needs, or by a
capped slice of lamports *already leaving the account at the close*. These are
not interchangeable and the difference is observable:

- **Prepaid-at-creation** *can* refuse for underfunding, and that is safe only
  because creation force-prepays it. Record `Abort` is this shape: `Begin`
  refuses a zero bounty at three sites (`lib.rs:417-419`, `:539-540`,
  `:1251-1252`), so by the time `Abort` runs the money is provably there and its
  `checked_sub`→`InsufficientCleanupBounty` (`lib.rs:1708-1711`) is
  belt-and-braces rather than a live failure mode.
- **Residual-at-close** *can never* refuse, because the reward is a capped
  residual of a sum that is already moving:
  `reward := min(floor, released - obligations)`. The claim-check design states
  the reason this property is mandatory rather than nice, and states it well
  (§6.2): *"A compaction that could refuse for lack of funds would reintroduce
  R3 through the funding door."* A crank that can refuse for money is an
  unturned crank, which is the unfunded-liveness defect wearing a different hat.

**The test.** Use prepaid-at-creation **if and only if there is a creation act
that (a) already signs, (b) already pays rent, and (c) can be made to refuse.**
If any of the three is absent — most often (c), because the creation route is in
a frozen program or its refusal would break a live caller — use
residual-at-close.

**Corollary, and it is the reason most Y1 sites are residual:** an existing
route being converted after the fact almost never has (c). Conversions
default to residual-at-close; only new values default to prepaid.

---

## 3. The reward floor is chain-derived, never a literal

**Ruling, and the tree violates it in one place today.**

The deployed route derives its floor from the Rent sysvar:

```rust
let cursor_rent = rent.minimum_balance(STAGING_CURSOR_BYTES_V1);   // record_v1.rs:348
...
.staging_liveness_policy(cursor_rent)                              // record_v1.rs:362
```

with `STAGING_CURSOR_BYTES_V1 = 296` (`record-contract lib.rs:21`), and `Begin`
prefunding the cursor with `cursor_rent + bounty` where the bounty is pinned
equal to `cursor_rent` (`record_v1.rs:1373`). At the default rent schedule that
is `(128 + 296) * 6960 = 2_951_040` lamports — 0.00295 SOL, about 590x a
5000-lamport base fee.

The one literal in the tree is
`COMPACTION_CRANK_REWARD_LAMPORTS_V1 = 200_000`
(`crates/dclutch-claims-svm/src/claim_check_v1.rs:108`) — 40x a base fee, and
14.8x below the deployed floor.

**The two numbers are not the same kind of thing**, and this document does not
score one as wrong: 200_000 is a *cap on a residual* (§2, cannot refuse);
2_951_040 is a *prepaid floor* (§2, can refuse). Both are defensible in role.

What is not defensible is the tree answering **"what is one crank worth"** in
two places, one of which a fee-market change silently invalidates while the
other re-derives itself every block. A literal needs a human to notice; a
`minimum_balance` call does not.

**The ruling:** the floor is a function of the Rent sysvar. A family may cap
below it or prepay above it, but the *number* comes from one place. Concretely:
`min(floor_from_rent(...), residual)` for residual sites, and
`prepay >= floor_from_rent(...)` for prepaid ones — never a fresh constant.

**Why rent is the right basis** and not a fee estimate: it is the only
chain-visible quantity that scales with the same economics that make the crank
worth turning, it is already in every one of these frames (the routes are
closing accounts), and it is generously above transaction cost at every account
width the protocol uses — so the floor cannot silently fall under the cost of
turning the crank.

### 3.1 Closing routes and surplus routes take the cap differently

A rent-derived floor is large — 1.78M lamports at a 128-byte account, 2.95M at
296. That is correct for a route where the account is *going away*, and wrong
for one where it survives, and the difference has to be ruled or the second kind
becomes a fee farm.

- **Closing route** (the value is leaving regardless — record `Abort`, permit
  expiry, ledger cleanup): `reward := min(floor, residual)`. If the residual is
  below the floor the cranker simply takes it all. The beneficiary is not harmed
  in any sense that matters, because the counterfactual is not "they get 100%"
  but "nobody calls it and they get it years later or never".
- **Surplus route** (the account survives and the movement is discretionary —
  the RentCredit sweep is the tree's only instance): `min(floor, residual)` would
  hand the *entire* surplus to the cranker whenever the surplus is small, which
  is a route that delivers the beneficiary nothing while looking funded. Such a
  route needs a **minimum-residual gate** — refuse the crank below some multiple
  of the floor — so that every admitted call leaves the beneficiary strictly
  better off. A refusal here does *not* reintroduce the §2 hazard, because
  refusing to sweep a trivial surplus strands nothing: the surplus stays put and
  is recovered at close.

**The tell for which kind you have:** ask what happens to the money if the crank
is never turned. Lost or indefinitely stranded ⇒ closing route. Still sitting
there, recoverable by a later route ⇒ surplus route.

---

## 4. The expiry comparison is inclusive

**Ruling.** `current_slot >= expiry_slot` opens the permissionless window.

The tree currently spells this two ways:

- inclusive — `let expired = observation.current_slot >= cursor.expiry_slot;`
  (`record-contract lib.rs:1700`)
- exclusive — `if current_slot <= self.input.expires_at { return Err(Expiry) }`
  (`crates/dclutch-dealer-codec/src/scenario_checkpoint_v1.rs:710-712`)

One slot, and no live defect today. But "when does a deadline fire" is one fact,
and a protocol that answers it in two directions across two programs is one
cross-program interaction away from an off-by-one nobody can attribute. Inclusive
wins because it is the deployed route's spelling and because a deadline that is
*exactly met* should fire rather than wait a slot for no reason.

---

## 5. Arithmetic lives in a plan struct, not in the program

**Ruling**, already house doctrine in two documents and one deployed route.

The shape: a struct in a contract/`-svm` crate holding every observed and every
computed quantity, whose constructor **refuses to exist unless the movement
balances**, plus a conservation recheck that touches no `AccountInfo`. Canonical:

```rust
pub fn validate_conservation(self) -> Result<()> {
    let total = self.cleanup_bounty_lamports
        .checked_add(self.sponsor_refund_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    if total != self.observed_lamports {
        return Err(Error::LamportConservationMismatch);
    }
    Ok(())
}
```
(`record-contract lib.rs:1460-1470`)

The program's only job is to supply observations and apply the plan's outputs.

**What this rules out, named because the tree does it:** the controller-ledger
cleanups do their arithmetic inline on `AccountInfo` with `checked_add` /
`checked_sub` and exit through a single undifferentiated `TradingSbfError::Commit`
for every distinct failure (`programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs:1270-1288`,
`:1300-1320`). Converting such a site is therefore **not** "add a payout line" —
it is first extracting arithmetic that has no home in a contract crate. Size
conversions accordingly.

**Conservation must account the reward path *and* the unspent-refund path.**
A plan that balances only when the reward is paid has not been checked on the
branch where it is capped to zero.

**And it must not be vacuous.** A conservation check over a two-way split whose
constructor already computes `a_after = a_before - x` and `b_after = b_before + x`
proves nothing — the sum reconciles by construction, and the check is a
tautology that will pass forever including on a route that is wrong. This is
live in the tree: `LifecycleSweepPlanV2`
(`crates/dclutch-rent-contract/src/lifecycle_v2.rs:537-575`) has no
`validate_conservation` while all three of its siblings in the same crate do
(`crates/dclutch-rent-contract/src/lib.rs:136-141`, `:203-208`, `:246-251`), and
**adding one today would be exactly such a tautology.** Conservation earns its
keep only from the third recipient onward. So the check and the funded split are
one change, never two — do not land the check first as "groundwork".

---

## 6. The caller signs only to own the reward

**Ruling.** A funded crank must not become an authorization gate. The caller's
signature — where one is present at all — establishes *who is owed*, never *who
is permitted*. Record `Abort` derives its signature requirement as data rather
than demanding it:

```rust
sponsor_signature_required: !expired    // record-contract lib.rs:1722
```

Before expiry only the sponsor may abort and the bounty is zero; after expiry
anyone may, and is paid. The permission and the payment are separate facts
computed by the same transition.

**The failure mode this forbids** is live in the tree and the census caught it:
dealer checkpoint cleanup *refuses* `beneficiary.is_signer`
(`programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs:1747`), so
the one party with an interest in the cleanup is forbidden to turn its own
crank, while the codec's own comment confirms nobody else can be paid: *"The
returned beneficiary is immutable; a cleanup caller cannot redirect rent or any
other lamports."* (`dealer-codec scenario_checkpoint_v1.rs:707-708`). A route can
be permissionless, deadlined, and conserved, and still have nobody who will ever
call it.

---

## 7. The bar — what makes a conversion GREEN

A converted route counts only when all six hold:

1. **The crank is paid**, and the payment is asserted **from chain/bank state**
   — a post-balance read, not an assumption that the plan was applied.
2. **Conservation is asserted at every exit**, including the branch where the
   reward caps to zero and the branch where everything refunds.
3. **The reward floor is chain-derived** (§3) and no new literal was added.
4. **Hostiles pin refusal codes** and are shown to reach the check they name —
   a hostile that refuses for the wrong reason is a passing test of nothing.
5. **A negative control** proves the new assertion fails against the old code.
   Without it, a conversion that changed nothing observable still goes green.
6. **The census row is updated in place with the commit** (GEO's pattern), so
   the queue inherits work rather than estimates.

---

## 8. Applying this to a site — the checklist

1. Locate the account(s) that close and the lamports already leaving them.
2. Identify the creation act; run §2's three-part test to pick prepaid vs
   residual.
3. Derive the floor from Rent at the width actually being closed (§3).
4. Write the plan struct in the contract crate first, with the conservation
   recheck, before touching the program (§5).
5. Make the reward the residual's cap so it can never refuse (§2).
6. Gate on `>=` (§4); take the slot from the Clock sysvar already in the frame
   where one is present, and say so if one must be added.
7. Prove all six of §7.

---

## 9. Sites, and who owns them

Y1's five named sites resolve as **3 trading-sbf / 2 core-sbf / 1 rent-sbf**
(capability close and series permit expiry share the core ELF). The census row
reads as a tree-wide smear; the debt is in fact concentrated in the one frozen
program, which is why it has survived — no lane could take three-fifths of it.

| site | program | kind (§3.1) | the actual gate |
|---|---|---|---|
| series permit expiry (`programs/dclutch-core-sbf/src/series_permit_expiry.rs`) | core-sbf | closing | **a Lean re-proof.** See below |
| rent sweep (`programs/dclutch-rent-sbf/src/lib.rs:369-427`) | rent-sbf | **surplus** — the tree's only one | gauntlet + operator + §3.1 gate |
| capability close (`programs/dclutch-core-sbf/src/capability.rs:582`) | core-sbf | closing | a `CapabilityManifestV1` ABI change (census Q5) |
| controller-ledger cleanup (`programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs:1270-1320`) | trading-sbf | closing | frozen; **and** the §5 extraction — no plan struct exists |
| dealer checkpoint cleanup (`programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs:1747`) | trading-sbf | closing | frozen; **§6 is the blocker, not funding** — it refuses `beneficiary.is_signer` |
| Direct expiry/invalidation (census Q10) | trading-sbf | closing | frozen — a **fourth** trading-sbf instance the row does not count |

**None of these is mechanical, and that is the finding.** The pattern survived
not because nobody noticed it but because every instance sits behind a
*different* gate — three behind the frozen program, one behind an ABI change,
one behind a Lean re-proof, one behind a live test harness. A queue entry that
says "apply P1" to any of them is an estimate, not work.

**Series permit expiry — why it is a Lean lane.** The route has a formal model
that defines the refund as the *whole* balance:

```lean
def seriesPermitExpiryRefund (observation : SeriesPermitExpiryObservation) : Nat :=
  observation.account.lamports
```
(`formal/dclutch-semantics/DClutchSemantics/MarketCore.lean:368-369`, with
`theorem unexpired_series_permit_refund_refuses` at `:371-375`)

A funded split contradicts that definition. The model is **spec-only** — the
emitters generate the physical ABI (`MarketCorePhysicalAbi.lean:383-410`), not
this — so nothing would go red, which makes it *worse* rather than better: the
conversion would silently desynchronize model from program, with no gate to
catch it. Add to that a 25-account frame in which **all 25 accounts are refused
as signers** (`series_permit_expiry.rs:107-161`), so a funded variant needs a
26th that is signer *and* writable; zero SBF test coverage; a route no campaign
executes — `docs/reference/routes.md:82` classes it *"blocked by rule … needs an
open Series Market"*, which is the weaker of that document's two unexecuted
categories and **not** its `NEVER-EXECUTED, no stated reason` (`routes.md:18`);
and residence in the tree's second-largest ELF whose CU budgets are pinned to
its hash (`tools/gauntlet/CU_BUDGETS.json`).

**Rent sweep — why the surplus/closing distinction was worth ruling.** It is the
only site in the tree where the account survives the crank, which is what forced
§3.1. Its other costs are honest and bounded: `SWEEP_ACCOUNT_COUNT_V2 = 3`
(`programs/dclutch-rent-sbf/src/lib.rs:213`) bumps to 4; `LifecycleSweepPlanV2`
takes a third recipient and gains the conservation check §5 says it cannot get
today without vacuity; and — the part a sizing estimate misses — the **gauntlet
asserts the property being removed**. `tools/gauntlet/journey/src/stages.rs:762-771`
fails the journey unless the fee payer's balance moves by exactly `-fee`, which
is precisely what a funded crank stops being true. The operator builder
(`crates/dclutch-product-runtime-v2-operator/src/lifecycle_rent_v2.rs:211-258`)
is the second writer of the same frame.

**A correction to the census's evidence, not its finding.** Rows Y1 and the
`RentCredit sweep` register row both attribute to the gauntlet the phrase *"a
pure donation of a transaction fee"* (`journey/stages.rs:748-758`). **The
gauntlet does not contain that phrase**, and `grep -n donation` over that file
returns nothing; the only two occurrences in the repository are the census
quoting itself. What is actually at `stages.rs:746-771` is the opposite kind of
statement — an assertion rather than a lament. The *finding* is correct and
verified independently here (zero signers on the sweep path; `wallet.key !=
state.refund_wallet()` at `lifecycle_v2.rs:837` pins a creation-fixed
beneficiary); only the evidence was invented. Both rows are corrected to cite
the real assertion.
