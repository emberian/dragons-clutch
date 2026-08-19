# The lifecycle walk: one market, end to end, as one gate

Status: **PASS**, recorded below. **Eleven** steps, one market, one gate: ten
accepting transactions whose every writable account came back byte-identical
to the offline reference adapter's post-state, one recorded refusal, and a
terminal accounting identity read out of the bytes the bank returned and
closed six ways. Both falsifiability self-checks fired.

**Regenerated 2026-08-19 for the mandatory token plane.** The walk this
document used to record predated commits `472b7fe` and `50c6e35`; every one of
its steps was refused `AccountCount` (`0x0001`) once those landed, and the
harness had to be rewritten. Two things changed in the walk itself, not only in
its numbers:

1. **The collateral is real now.** Steps 3 and 6 move Token-2022 balances
   between the actor's account and the Hoard's; steps 4 and 5 mint and burn a
   real outcome token; step 11 pays a redemption out of the Hoard's token
   account. Those accounts are compared byte for byte alongside the state
   accounts. The old "not a token movement" caveat at the foot of this
   document is retired.
2. **The walk's opening cash is driven.** It used to be the one field of the
   opening state that no instruction produced — a number this harness wrote
   into the genesis position account. The genesis plane added `Endow`, so it is
   now step 2, with a signer, a replay sequence, and a cap. What it still is
   not is a *backed* deposit, and item 2's entry says exactly that.

This is a bring-up walk under a simulating runtime, not a deployment, not a
proof, and not a mainnet market. Read
[What this walk is not](#what-this-walk-is-not) and the skip list in
[Section 10 items this walk does not drive](#section-10-items-this-walk-does-not-drive)
(items 3 and 8 are not driven; items 1 and 2 are part-driven; and item 6's
observations are accepted but never source-authenticated) before quoting it.

PROJECT.md section 10 does not define success as ten green instruction
families. It defines it as **one reproducible local walk** proving that the
same frozen terms can carry one market from creation to a closed accounting
identity. The per-family evidence in
[`SBF_BRINGUP.md`](SBF_BRINGUP.md) is the prerequisite for that claim and is
not that claim: ten families that each pass in isolation say nothing about
whether the eleventh state is reachable from the first.

This document is the walk. It is one ordered narrative over one market, run
inside a real Agave bank, and it fails as a unit.

Run it:

```sh
# The whole gate: reproducible ELF, per-family differential, its falsifiability
# self-check, then the walk and the walk's two self-checks -- one validator
# session for all of it.
programs/clutch-sbf/scripts/run_bringup.sh

# The walk alone, against an already-running gate validator:
python3 programs/clutch-sbf/scripts/simulate.py --url http://127.0.0.1:18899 \
    --plan "$WORK/plan" --lifecycle

# The walk's build-time assertions, with no validator at all:
cargo test --manifest-path programs/clutch-sbf/Cargo.toml -p clutch-sbf-harness
```

`cargo test` needs the pinned `solana` CLI on `PATH` or in `SOLANA_BIN`:
address derivation is not compiled into these crates, so the harness derives
the walk's ~130 additional program addresses out of process while it builds the
plan. That derivation is why `cargo test` on this crate takes about eighty
seconds: the plan now derives roughly 300 addresses, one CLI invocation each.

## What "one walk" can and cannot mean here

`simulateTransaction` never commits. One address therefore carries exactly one
pre-state in one genesis, and a chained walk of eleven steps needs eleven
pre-states. The walk produces them the way the rest of this lane produces
every non-trivial pre-state: it **runs the offline reference adapter forward**.
Step *k*'s genesis is the adapter's post-state after steps 1..*k*-1, and step
*k*'s SVM post-state is compared byte for byte against the adapter's
post-state after step *k*.

So the honest statement is:

> Eleven pre-states, each of which is the previous step's output, each executed
> by a real bank, each compared byte for byte against a second implementation.

and **not**:

> A bank committed eleven transactions in order.

What differs between the planes is the market *identity* — one nonce per
step, so the addresses differ. Everything else is the previous step's
reference post-state. The harness asserts that chaining at build time
(`assert_walk_chains`): each plane's replay account must sit at exactly the
sequence its position in the walk requires, the opening state must be
`CreateMarket`'s own post-state with **nothing** added, the endowed state must
be that plus exactly the credited cash and one consumed sequence, and the
kernel must be resolved before anything redeems. A walk that quietly stopped
being a chain is a `cargo test` failure, not a green differential over a
fiction.

**One step is not the reference adapter's.** Step 2's `Endow` has no reference
transition at all — `apply` refuses `Intent::Endow` with `UnsupportedIntent` —
so its expectation is this harness moving the two fields the transition moves,
through the frozen `PositionAccount` and `ReplayAccount` codecs. That is the
weakest oracle in the walk and the plan labels it as one. What it still
establishes is that a real bank executed it and that every *other* byte of both
accounts came back untouched.

**Step 7 is the exception in the other direction, and it is the strongest
step.** Its three
`FeedAdvance` instructions ride in *one* transaction against *one* writable
feed head, so the bank itself sequences that chain: page three is folded
against page two's writes, which were folded against page one's. That is the
same mechanism the per-family `roundtrip` case uses, generalized to three
links.

## The walk

Every step records compute units consumed, transaction bytes, how many
accounts it actually wrote, and its differential verdict. **The walk fails as
a unit.** A step that diverges, refuses where it should accept, or accepts
where it should refuse fails the whole walk, because a lifecycle that closes
nine tenths of the way closes nothing.

| # | step | §10 item | what it establishes |
| --- | --- | --- | --- |
| 1 | `walk-01-create-market` | 1 | Eight all-zero accounts at their canonical addresses become one active market — **and three accounts that did not exist come into existence**: the Hoard's Token-2022 collateral account and one outcome mint per outcome, created by System-program CPI and initialized by the token program, then re-admitted through the very policies every later instruction applies to them. The collateral cap, the outcome basis, the payout set, and the feed identity are read out of the frozen terms artifact; the instruction chooses none of them. |
| 2 | `walk-02-endow` | 2 (in part) | The walk's opening cash, credited by an instruction rather than written into a fixture: the position owner signs, the market's immutable cap bounds the credit, and one replay sequence is consumed. It is still **unbacked** — no collateral moves, and the Hoard's untouched bytes in this step are that statement driven. |
| 3 | `walk-03-split` | 4 | One collateral debit credits one unit of every Egg. The complete set lives in the position's internal balances, and **both** collateral truths rise by the quantity split: `HoardAccount::collateral_atoms` and the Hoard's real Token-2022 balance, which the program refuses to let disagree. |
| 4 | `walk-04-materialize` | 5 | Part of one outcome leaves the internal ledger for the external shadow, and a real `MintTo` signed by the market PDA is what makes it external. `total_i` is preserved exactly, the mint's supply equals the market-wide external term, and the Hoard does not move. |
| 5 | `walk-05-dematerialize` | 5 | The reverse crossing, for part of what was materialized: the holder's tokens are burned under the owner's own signature. The remainder stays outstanding externally for the rest of the walk and is what the terminal Hoard must cover. |
| 6 | `walk-06-merge` | 4 | The promise of PROJECT.md section 1 exercised: a complete set can always be recombined into its collateral **before** resolution. This is the one direction that is impossible without the program signing for the Hoard authority itself. |
| 7 | `walk-07-feed-advance` | 6, 7 | Three contiguous observation pages fold into one feed head inside one transaction. The cursor moves 100 → 102 → 103 → 104, and 104 is exactly the maturity bound the market's window needs before it can seal. |
| 8 | `walk-08-resolve` | 7, 9 | No reporter chooses the cell. The buffer carries observation records; the gate folds them through the accumulator's `Open → Mature → Sealed` machine against the terms' own window domain, reads the matured cursor off the feed head, and refuses unless the payout index the caller named is the one the sealed window selects. |
| 9 | `walk-09-merge-after-resolve` | 4, 10 | **A recorded refusal.** The same complete-set merge step 6 accepted is refused once the market has resolved. This is the boundary the product model draws, and it is why the terminal accounting is a *redemption* identity rather than a merge identity. |
| 10 | `walk-10-redeem-winning` | 9 | The first payoff shape: the unit vector on the realized cell. Collateral leaves the Hoard's token account for the redeemer's, one atom per claim. The resolution record is presented read-only and must come back unchanged, so a redemption can never edit its own authority. |
| 11 | `walk-11-redeem-losing` | 9, 10 | The second payoff shape: the zero vector on an unrealized cell. The claims burn, the transfer runs for **zero**, and neither the Hoard's accounting nor its token balance moves by one atom — the half of the solvency promise that is easy to state and easy to get wrong. |

### Measured

Recorded by `programs/clutch-sbf/scripts/run_bringup.sh`, one validator
session shared with the per-family gate, on `aarch64-apple-darwin`,
`solana-cli 4.0.2 (Agave)` / `cargo-build-sbf 4.0.0 platform-tools v1.53`.

- `HEAD` = `7ce4c09`. The gate reports the working-tree status of the ELF's
  own input paths separately from everything else; this run reported
  `elf_inputs=DIRTY` for exactly one file, `programs/clutch-sbf/Cargo.lock`,
  whose only change is the harness package's own dependency list. That is an
  argument rather than evidence, so the same ELF was rebuilt from a clean
  extraction of `HEAD` and produced the identical digest.
- ELF built twice into fresh target directories:
  `sha256=59c48c482831626ae9d7cb908f4de0e3f93b1572cdd82105c61f2f87bdaad25f`,
  505 960 bytes, `sbf_reproducibility=PASS`.
- Genesis: **195** accounts in one validator invocation — 131 program-owned,
  63 Token-2022-owned, and one System-owned account holding the creator's
  lamports, because a creator with no lamports cannot pay rent for the
  accounts `CreateMarket` founds. The walk contributes nine market planes (72
  program-owned accounts), the Token-2022 plane of the eight founded ones (40
  accounts), and three observation-page buffers. `walk-found` installs no
  token accounts at all: step 1 creates them.
- The per-family gate in `SBF_BRINGUP.md` passed in the same session, before
  the walk: 10 accepting transactions, 22 refusals, 0 undrivable.

```text
   #  step                                 CU  tx bytes  written  differential
   1  walk-01-create-market            852843      1119        8  MATCH
   2  walk-02-endow                     81675       492        2  MATCH
   3  walk-03-split                    198810       888        7  MATCH
   4  walk-04-materialize               86647       822        6  MATCH
   5  walk-05-dematerialize             94125       822        6  MATCH
   6  walk-06-merge                    209346       888        7  MATCH
   7  walk-07-feed-advance         15672 [6210, 4731, 4731]       669        1  MATCH
   8  walk-08-resolve                  551929       681        4  MATCH
   9  walk-09-merge-after-resolve       76308       888        0  REFUSED Custom(0x0016)
  10  walk-10-redeem-winning           551239       920        7  MATCH
  11  walk-11-redeem-losing            557239       920        4  MATCH
```

The pre-token walk, kept for the comparison: `create-market` 695 567, `split`
76 611, `materialize` 76 679, `dematerialize` 70 680, `merge` 82 731,
`resolve` 531 627, the redemptions ~427 000, and a total of **2 489 442**
units over ten transactions. The regenerated walk costs **3 265 434** units
over eleven, and the whole 776 000-unit increase is the token plane and the
endowment: about 135 000 per collateral transition (two software SHA-256
digests inside `collateral::verify_profile_identity`, not the CPI), about
15 000 per outcome transition, 157 000 more for a `CreateMarket` that performs
seven creation CPIs, and 81 675 for a step that did not exist.

`CU` is the compute the program under test consumed, filtered by program id so
the `SetComputeUnitLimit` instruction ahead of an expensive step is not
miscounted. Step 7's bracketed list is per instruction: three feed advances in
one transaction, 6 210 for the first (which folds two observations) and 4 731
each for the two single-observation pages. `written` is how many compared
accounts actually changed — a step whose expectation happened to equal its
pre-state could not pass by accident, because the plan marks each role
`changed` or `unchanged` and the gate requires it. The `written` counts grew
with the token plane for the reason the plane exists: step 3 writes seven
accounts because two of them are Token-2022 accounts, and step 11 writes four
rather than seven because a losing redemption moves no collateral and the
gate requires those three to come back *unchanged*.

**Every step but the feed advance carries a `SetComputeUnitLimit` of
1 400 000** — the walk's most expensive step, `create-market` at 852 843, is
61% of that ceiling, and its cheapest raised one, `endow` at 81 675, would
have fitted the default. Uniformity is deliberate: a per-step decision about
which transaction gets a raise is a decision that goes stale, and the raised
number is itself the measurement. The whole walk costs **3 265 434** program
units across eleven transactions. That is a resource finding, not a victory
lap: one fixture, two outcomes, three observations, one position.

**Step 9's refusal code is a deliberate vocabulary difference, not a
disagreement.** The offline reference adapter refuses a post-resolution merge
as the generic `MismatchedState` (`0x300e`); this program *refines* it to
`ClutchError::NotActive` (`0x0016`), one of the two refinements
`split.rs`'s projection table documents. The harness pins both halves: it
asserts the reference's code and expects the program's, so a drift on either
side is loud rather than absorbed.

## The terminal accounting identity

Step 11 leaves the market drained except for one thing: the claims that were
materialized in step 4 and never brought back in step 5. There is no
`RedeemExternal` instruction, so those stay outstanding, and PROJECT.md
section 1's central promise says exactly what the Hoard must then hold:

> For every reachable protocol state, the market-local Hoard covers the maximum
> payout allowed by the market's immutable terms.

At the end of the walk that inequality is an **equality**, which is a stronger
and much more falsifiable statement, and it is what section 10 item 10 asks
for.

Every number is derived twice and the two derivations must agree, or the
harness panics before a validator is ever started:

- from the **walk's own arithmetic** over its quantities (`WALK_CASH`,
  `WALK_SPLIT`, `WALK_MATERIALIZE`, `WALK_DEMATERIALIZE`, `WALK_MERGE`), and
- by **decoding the offline reference adapter's terminal post-state**.

Nothing is transcribed from an observed run. The gate then reads each number a
third time — out of the bytes the bank returned for step 11, at an offset the
harness located by *probing the frozen codec* (write the field as `1`, write it
as `256`, see where the encoding moved) rather than by hard-coding it — and
evaluates the identities over those on-chain values.

The identities, in the form the gate prints them:

| identity | equation |
| --- | --- |
| collateral conservation | `opening_cash == position_cash + hoard_collateral` |
| the Hoard covers exactly the unredeemed obligations | `hoard_collateral == Σᵢ payout_weight[resolved][i] · kernel_total_supplyᵢ` |
| the internal ledger drains to zero | `0 == Σᵢ position_internalᵢ` |
| the kernel supply is exactly the outstanding external claims | `Σᵢ kernel_total_supplyᵢ == Σᵢ external_balanceᵢ` |
| the supply ledger closes, per outcome | `kernel_total_supplyᵢ == ledger_internalᵢ + ledger_externalᵢ` |

The payout weights are read out of the terminal kernel's own resolved payout
vector, not retyped. The walk asserts that vector's denominator is 1; a
fractional payout policy would need a divisor here, and the assertion is what
makes that a build failure instead of silent rounding.

### Recorded

```text
  terminal state, read out of the on-chain bytes:
    hoard_collateral         3            (ok, walk-redeemed.hoard +98)
    position_cash            61           (ok, walk-redeemed.position +202)
    position_internal_0      0            (ok, walk-redeemed.position +74)
    kernel_total_supply_0    0            (ok, walk-redeemed.kernel +38)
    external_balance_0       0            (ok, walk-redeemed.external +74)
    ledger_internal_0        0            (ok, walk-redeemed.supply +75)
    ledger_external_0        0            (ok, walk-redeemed.supply +203)
    position_internal_1      0            (ok, walk-redeemed.position +82)
    kernel_total_supply_1    3            (ok, walk-redeemed.kernel +46)
    external_balance_1       3            (ok, walk-redeemed.external +82)
    ledger_internal_1        0            (ok, walk-redeemed.supply +83)
    ledger_external_1        3            (ok, walk-redeemed.supply +211)

  accounting identities:
    CLOSED  opening_cash (64) == position_cash + hoard_collateral
              opening_cash=64 = 64 = position_cash(61) + hoard_collateral(3)
    CLOSED  hoard_collateral == sum_i payout_weight[1][i] * kernel_total_supply_i (denominator 1)
              hoard_collateral(3) = 3 = 0*kernel_total_supply_0(0) + kernel_total_supply_1(3)
    CLOSED  0 == sum_i position_internal_i
              zero=0 = 0 = position_internal_0(0) + position_internal_1(0)
    CLOSED  sum_i kernel_total_supply_i == sum_i external_balance_i
              kernel_total_supply_0(0) + kernel_total_supply_1(3) = 3 = external_balance_0(0) + external_balance_1(3)
    CLOSED  kernel_total_supply_0 == ledger_internal_0 + ledger_external_0
              kernel_total_supply_0(0) = 0 = ledger_internal_0(0) + ledger_external_0(0)
    CLOSED  kernel_total_supply_1 == ledger_internal_1 + ledger_external_1
              kernel_total_supply_1(3) = 3 = ledger_internal_1(0) + ledger_external_1(3)
```

In words. The walk was endowed with 64 atoms of cash and split 20 of them into
complete sets. It materialized 8 claims of the winning outcome, brought 5
back, and merged 4 complete sets before resolution. After the window sealed on
the winning cell it redeemed its 13 remaining winning internal claims for 13
atoms and burned its 16 losing internal claims for nothing. What is left is 3
materialized winning claims that no instruction can redeem, and the Hoard
holds exactly 3 atoms against them: **not more, which would be stranded
collateral, and not less, which would be an insolvent market.** The 61 atoms
of cash and the 3 in the Hoard are the 64 it started with.

And the 3 atoms are not only a field. The Hoard's real Token-2022 account holds
3 atoms of the collateral mint too, because `token::require_hoard_mirror`
refuses any transition after which the two disagree — so the identity above is
closed over the accounting *and* the asset.

### It can go red

Two mutations run inside the same validator session, after the walk passes:

1. **The readout.** One terminal expectation is moved by one atom. The gate
   must report a mismatch — which is only possible if it really read the
   bank's bytes.
2. **The arithmetic.** One payout weight in one identity is doubled. The gate
   must report that the identity `does not close`, with both sides printed —
   which is only possible if it really evaluated the equation.

A gate whose self-checks *pass* fails the run. Recorded, in the same validator
session, immediately after the walk passed:

```text
== lifecycle falsifiability self-check (same validator session) ==
the terminal Hoard expectation was moved by one atom; the walk went red:
    terminal identity: hoard_collateral is 3 on chain, expected 4
one payout weight in one identity was doubled; the walk went red:
    terminal identity `hoard_collateral == sum_i payout_weight[1][i] *
    kernel_total_supply_i (denominator 1)` does not close:
    hoard_collateral(3) = 3 != 6 = 0*kernel_total_supply_0(0) +
    2*kernel_total_supply_1(3)
```

## Section 10 items this walk does not drive

Silent absence is how a definition of success quietly gets easier. Every
section-10 item the walk cannot drive is carried in the plan as an explicit
skip with a reason, printed by the gate on every run.

### Item 1 (in part) — initialize a Realm

**NOT DRIVEN THIS ROUND, and no longer for want of an instruction.** This
entry used to say "there is no Realm, Profile, price-grid, or terms
initialization instruction in this program". That is now false:
`instructions/genesis.rs` implements `InitRealm`, `InitProfile`,
`InitPriceGrid`, `InitTerms` and `InitOrderPage`, each creating its account
through a real System-program CPI, with the two large artifacts riding a
verbatim-copied evidence buffer.

Neither this walk nor the per-family plan drives them, and the reason is
scope rather than impossibility. Each needs its own fresh identity plane whose
target address is absent from the genesis dump — a Realm nonce, a collateral
policy, a grid body and a terms body whose digests are not the ones already
installed — and none of the five has a reference oracle: `reference::apply`
refuses all of them with `UnsupportedIntent`, so a differential would compare
this program against a re-encode of its own intent. **Their creation CPI has
therefore never executed on a bank.** The only account creation this walk
drives is `CreateMarket`'s, which is a different code path in a different
module.

The Realm-wide plane is still loaded at genesis as frozen bytes the frozen
codecs accept. "Initialize a realm and market" is therefore still part-driven,
and the part that is driven is the market.

### Item 2 (in part) — prepay all mandatory work

**PARTLY DRIVEN, at step 2, and the residue is exact.** This entry used to
read "no endowment, prepayment, or deposit instruction exists" and to name the
fixture-written opening cash as the sharpest gap in the walk. The genesis
plane added `Endow`, and the walk drives it: the position owner signs, the
market's immutable `collateral_cap` bounds the credit, one replay sequence is
consumed, and the opening state the endowment lands on is now byte-for-byte
exactly what `CreateMarket` wrote, with nothing added. The harness asserts
that too.

What no instruction does is **back** the credit. The value leg is a Token-2022
`TransferChecked` into the market's Hoard token account; `token.rs` constructs
exactly that CPI and no instruction wires it. So the endowed cash is an
internal-ledger entry with no deposit behind it, and `genesis.rs` says so in
its own module documentation rather than in a footnote here. The cap is also
**necessary and not sufficient**: it bounds all collateral the market may ever
hold, so a fortiori any one position's claim on it, but the sufficient check
needs a market-wide cash aggregate that no account in the frozen layout
carries.

The gap is therefore narrower and sharper than it was. Protocol
sustainability (PROJECT.md section 7) is a claim about *prepaid* resources,
and this walk contains a signed, sequenced, capped credit that nobody paid
for.

### Item 3 — compile and prove one exhaustive state partition

**NOTED, not driven.** The immutable terms artifact **is** the compiled
partition: outcome count, payout vectors, payout map, knots, and the
statistic, edge, and ambiguity policy identities, frozen into one digest the
market binds and the resolve gate re-reads on every evidence-gated
transaction. There is no on-chain compiler instruction and none is claimed.
Step 1 *consumes* the artifact; it does not produce it. "Prove" in item 3
remains entirely undischarged here — this walk contains no proof about the
partition, only the frozen artifact and the digest binding it.

### Item 8 — clear one coupled simplex batch with portfolio intents

**SKIPPED, for two independent reasons.**

1. `PlaceOrder` has **no SVM oracle**. The offline reference adapter models no
   order family, so there is no second implementation for a differential to
   disagree with; a green result would be the program agreeing with itself.
2. Settlement **awaits the streaming verifier on-chain**. Cancellation and
   batch settlement are honest stubs.

The batch-auction plane (epoch, order page, candidate, final pot, settlement
receipt) is loaded at genesis and no implemented instruction transacts against
it. Portfolio intents in particular are unexercised end to end.

### Item 11 — reproduce in the Rocq model, the Verus-verified kernel, and the SBF harness

**OUT OF SCOPE** for this walk, per the standing deprioritization. This walk is
the SBF-harness leg alone. It makes no claim about the Rocq model or the
Verus-verified host kernel, and it is not a triple reproduction.

## What this walk is not

- **Not a commit.** Every transaction is simulated with `sigVerify: false`.
  The `is_signer` bits the program reads do come from the transaction message
  header, and that is the whole of what the authorization steps establish. No
  Ed25519 signature is verified anywhere in this walk.
- **Not one committed sequence.** See the first section: it is eleven
  pre-states chained by the reference adapter (ten of them; step 2's is this
  harness's own re-encode), plus one three-instruction chain the bank
  sequenced itself.
- **A token movement now, and the old caveat here is retired.** This bullet
  used to read "the walk's transactions present no token accounts and exercise
  no CPI". Since the token plane became mandatory the walk moves real
  Token-2022 balances: a `TransferChecked` in each direction on the collateral
  legs, a `MintTo` and a `Burn` on the outcome legs, and seven creation CPIs
  in step 1. What is still true is the *provenance* of the genesis token
  accounts: they are bytes this harness wrote, not bytes the token program
  wrote, because a validator loaded from a genesis dump cannot run an
  instruction before its first slot. They are not taken on trust — the real
  Token-2022 program executes against them and refuses anything it did not
  consider a mint or an account, and the harness re-runs this program's own
  admission over every image before emitting it.
- **Not evidence about any deployment.** Loopback only, one local
  `solana-test-validator`, nothing committed to any ledger.
- **Not an envelope.** One fixture, two outcomes, one three-observation
  window, one position. Worst-case outcome counts, multi-position closure, and
  larger evidence buffers are unmeasured, and every compute number would grow.
- **Not verified.** Nothing here is a proof. The offline reference adapter
  agreeing with the SBF program on this trajectory is two implementations
  agreeing on one trajectory.

### The two feed heads

Step 7 advances one feed head; step 8 resolves against another. They are two
accounts of two feed identities, because one address cannot hold both cursor
100 and cursor 104 in one genesis. The harness asserts that.
the cursor is the only fact the resolve gate reads off a feed head
(`require(feed.feed == market.feed)` then `feed.cursor`), and step 7's three
advances must land it on exactly the value step 8's head carries. The two heads'
page counts differ, because they are two identities with two page histories;
nothing in the gate reads that field.

This is a named artifact of simulate-only execution, not a modelling choice.
Under a committing runtime it would be one head.


> **Token-plane regeneration record (2026-08-19).** The staleness notice that
> stood here is discharged. Every walk step was refused `AccountCount`
> (`0x0001`) after commits `472b7fe` and `50c6e35` until the harness was
> regenerated for the mandatory token planes — 16 accounts for `Split`/`Merge`,
> 13 for `Materialize`/`Dematerialize`, 19 for `RedeemInternal`, 21 for
> `CreateMarket` at two outcomes — and for the Profile identity becoming
> `ParentProfile::from_policy(policy).identity()`, which moved every PDA in
> every fixture. Full gate at `HEAD = 7ce4c09`: bring-up **PASS**, lifecycle
> **PASS** over eleven steps, all three falsifiability self-checks fired, exit
> 0. ELF sha256
> `59c48c482831626ae9d7cb908f4de0e3f93b1572cdd82105c61f2f87bdaad25f`,
> 505 960 bytes, reproduced from a clean `HEAD` extraction. The pre-token
> numbers and the pre-token digest `d8a9267c…` are retired rather than
> superseded: the transactions that produced them no longer have valid account
> planes.
