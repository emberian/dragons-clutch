# Decision 0021: whose rent a state carries is a per-plan declaration, not one global rule about lamport flow

Status: **PROVISIONAL — ruled by the orchestrator on 2026-09-03, REFUTED the
same day by the lane it was given to, re-made as the declaration below, and
reversible by ember at the cost §8 states**. Both the first ruling and its
refutation carry the standing formula *"RULING (under the standing goal; ember
may reverse)"* and are on the record at `GOAL.md:3252-3256` and `:3286-3298`.
The declaration landed at `d190297d6`, `648fad0a0`, `5fc108bd5`, `c60f853be`
with frame rows `fa6fa4828`, `7e8f6448d`, all 2026-09-02.

**This record exists mostly for §2.** A ruling made under the standing goal was
wrong, was refuted by its own executing lane rather than by a reviewer, and the
refutation is the load-bearing part of the history.

## 1. The question

The Dealer multi-LP family admitted **exactly one LP owner per market
generation**. The Open transition requires the lifecycle beneficiary output to
equal the position's owner (operation 12, `identity_eq(18, 5)`), and the kernel
made every created state's beneficiary the RentCredit's refund wallet — one
immutable wallet per (domain, market, generation). *"It passed for as long as
the campaign staged that credit with the LP owner's own key, and refused the
first honest second owner"* (`d190297d6`; `GOAL.md:3247-3250`).

The question was found only because decision 0020 cleared the compute budget far
enough for the campaign to reach a stage no run in its history had reached
(`271ce0edb`).

## 2. The first ruling, and its refutation

**First ruling** (`GOAL.md:3252-3256`, 2026-09-03 03:45):

> **RULING (under the standing goal; ember may reverse): the refund follows the
> debit** — the beneficiary derives from the funding source (the credit's when
> the credit funds, the payer's when a payer is debited), one rule for every
> lifecycle-rent family; Lean first if the rule is emitted.

**Refuted by the lane it was given to** (`GOAL.md:3286-3288`):

> **The rent RULING above is REFUTED and corrected:** "the refund follows the
> debit" unconditionally is a **theft vector** — a maker replay root is a shared
> structure of the market and the same route admits a stranger as payer.

**The incident behind the refutation is dated and measured: 2026-08-31.**
Direct's maker replay root is created by whoever pays for one fill, its
`rent_owner` is `plan.beneficiary` (`hot_v3.rs:6409` at HEAD, `:6378` when the
commit was written), and `direct_close_maker` pays the whole observed balance to
that `rent_owner`.
`maker_root_rent_beneficiary_v1`
(`tools/local-validator/bootstrap/successor/src/direct_trade_producer.rs:2824`,
used at `:2356`) documents the invariant with that measured incident behind it —

> a stranger paying their own fees would walk away owning the rent of something
> the market depends on

— and an unconditional payer rule makes exactly that happen (`d190297d6`). The
invariant now sits in the kernel's own doc comment at
`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs:470-488`, which
names both type cases and cites the incident by date.

**The obvious patch was refuted too.** A `payer_debit > 0` conditional *"adds a
griefing vector (one donated lamport refuses an owner's Open forever)"*
(`GOAL.md:3288-3289`). So the corrected answer is not a cleverer global rule; it
is that there is no global rule to be had.

## 3. The ruling as re-made, verbatim

> **whose rent a state carries is a per-plan DECLARATION** — action-plan byte
> five `REFUND_SOURCE_CREDIT = 0 / PAYER = 1` (zero keeps every prior policy's
> bytes) — the kernel proves the named party is one the plan's funding admits;
> Direct requires `Credit` at both plan readers; Dealer LP declares `Payer`.
> — `GOAL.md:3289-3294`

## 4. What it changed in the trust model

From *one global rule about lamport flow* to *a property of the state itself*,
declared in the record and proved against the plan's own funding
(`d190297d6`):

> A state's refund identity is a property OF THE STATE: market-shared structures
> owe their rent to the market's wallet, and one participant's own position owes
> it to the participant.

The action-plan record's **reserved byte five** becomes that declaration
(`lifecycle_v3.rs:138-139` for the encoding, read at `:2195`):

- `REFUND_SOURCE_CREDIT = 0` — the permanent RentCredit's immutable wallet.
- `REFUND_SOURCE_PAYER = 1` — the payer the plan names and the profile debits.

The kernel then proves the named party is one the plan's own funding admits, and
that **nothing else is nameable**:

- **create** — `beneficiary = credit.beneficiary | *payer.key` per the tag; when
  the caller owns the observation registers the declared identity must equal it
  (the old credit-equality, generalized to the funding).
- **close** — the declared register is the create's recorded answer read back out
  of the state; `Credit` re-derives from the credit exactly as before, `Payer`
  requires only that a create recorded one.
- **protected authenticate** — the same split.

Dealer LP Open and Close declare `Payer`: their PDA is seeded by
`LP_OWNER_IDENTITY_V3` and the Open debits that owner, so operation 12 *"becomes
a real authentication of payer == owner instead of a check that the market's
sponsor happened to be the one admitted owner"* (`d190297d6`). Every other plan
declares `Credit`, which is what it already meant.

**Landing it found two more authors of the old law** (`GOAL.md:3293-3294`):

- `apply_lifecycle_closes_v3` re-ran the credit-equality at the mutation
  boundary — a rule that boundary cannot derive for a payer-funded state,
  because the payer funded the account in an earlier transaction and is not an
  account of this one. The Dealer LP Close had been refusing `Commit 0x4005`
  there, *"one of 237 sites for that code — with every observation already
  agreeing"* (`c60f853be`). Fixed by `closing_refund_identity_admitted_v3`
  reading the plan's own declaration.
- `direct::lifecycle` *"never said whose authority"*.

## 5. What it saved, measured

- **Byte-compatibility by construction.** `Credit = 0`, so *"every policy encoded
  before the distinction existed keeps its exact bytes and its exact meaning;
  all 60 pre-existing contract tests pass unmoved"* (`d190297d6`).
- **The multi-LP family stops admitting one owner.** *"LP Open #1 → hostile Add →
  honest Add → LP Open #2 (671,787 CU, second owner) → second Add all COMMIT"*
  with a real sponsor (`GOAL.md:3294-3296`). Campaign 30/1.
- **Frames:** three frames moved and every one of them is smaller (`7e8f6448d`);
  the declaration itself moved none (`fa6fa4828`).

## 6. The hostiles that guard it

Two new bodies of evidence, **both proven red first by defeating the rule they
test** (`d190297d6`):

- `payer_refunded_states_name_the_funding_and_refuse_every_stranger` holds both
  arms on one fixture two bytes apart — the market arm refuses the payer, the
  owned arm refuses the market's sponsor and an unrelated key alike, the close
  reads back what the create recorded, an `Authenticate` may not declare a
  refund identity at all, and an unknown tag is `UnknownTag` rather than a
  silently accepted zero.
- the protected `AuthenticateOrCreate` test grows the same split, including a
  state that recorded no refund identity at all.

`direct::lifecycle::validate_create` is now the conjunct enforcing Direct's
`Credit`-only requirement, sited next to the maker replay root's
`rent_owner: plan.beneficiary` adoption *"and now says why that is only sound
for a `Credit` plan"* (`c60f853be`).

## 7. Named debt carried

Recorded here rather than left to be found (`GOAL.md:3283-3285`):

- **The rule is not in Lean.** `StateLifecyclePolicyV5Abi` is layout only.
- Six fixtures encoded the old law.
- The builder mirror at `registers.rs:1481` wrote the credit's wallet
  unconditionally.

## 8. The cost of reversal

Three different reversals with three different costs, and they must not be
confused:

- Reverting to **"the refund follows the debit"** reinstates the theft vector the
  2026-08-31 incident measured — a stranger owning the rent of a market-shared
  structure.
- Reverting to the **pre-declaration law** reinstates one LP owner per market
  generation, and the Dealer LP Close returns to refusing `Commit 0x4005` with
  every observation agreeing.
- Reverting **only the byte** is free in bytes, because `Credit` is zero — and
  re-refuses the second LP Open, which is the whole thing the change bought.

## Evidence pointers

`GOAL.md:3247-3256`, `:3283-3298`; commits `d190297d6`, `648fad0a0`,
`5fc108bd5`, `c60f853be`, `fa6fa4828`, `7e8f6448d`;
`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs:138-139`,
`:470-488`, `:2195`, `:2235`, `:3395`;
`tools/local-validator/bootstrap/successor/src/direct_trade_producer.rs:2356`,
`:2824`; `programs/dclutch-trading-sbf/src/hot_v3.rs:6409`;
`programs/dclutch-trading-sbf/src/lib.rs:152` (`Commit 0x4005`);
`docs/decisions/0020-finalization-observes-the-deployment.md` (which made the
defect reachable).
