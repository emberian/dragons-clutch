# The lifecycle walk: one market, end to end, as one gate

Status: **PASS**, recorded below. Ten steps, one market, one gate: nine
accepting transactions whose every writable account came back byte-identical
to the offline reference adapter's post-state, one recorded refusal, and a
terminal accounting identity read out of the bytes the bank returned and
closed six ways. Both falsifiability self-checks fired.

This is a bring-up walk under a simulating runtime, not a deployment, not a
proof, and not a mainnet market. Read
[What this walk is not](#what-this-walk-is-not) before quoting it.

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
the walk's ~70 additional program addresses out of process while it builds the
plan.

## What "one walk" can and cannot mean here

`simulateTransaction` never commits. One address therefore carries exactly one
pre-state in one genesis, and a chained walk of ten steps needs ten
pre-states. The walk produces them the way the rest of this lane produces
every non-trivial pre-state: it **runs the offline reference adapter forward**.
Step *k*'s genesis is the adapter's post-state after steps 1..*k*-1, and step
*k*'s SVM post-state is compared byte for byte against the adapter's
post-state after step *k*.

So the honest statement is:

> Ten pre-states, each of which is the previous step's output, each executed by
> a real bank, each compared byte for byte against a second implementation.

and **not**:

> A bank committed ten transactions in order.

What differs between the ten planes is the market *identity* — one nonce per
step, so the addresses differ. Everything else is the previous step's
reference post-state. The harness asserts that chaining at build time
(`assert_walk_chains`): each plane's replay account must sit at exactly the
sequence its position in the walk requires, the opening state must be
`CreateMarket`'s own post-state with exactly one field credited, and the
kernel must be resolved before anything redeems. A walk that quietly stopped
being a chain is a `cargo test` failure, not a green differential over a
fiction.

**Step 6 is the exception, and it is the strongest step.** Its three
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
| 1 | `walk-01-create-market` | 1 | Eight all-zero accounts at their canonical addresses become one active market. The collateral cap, the outcome basis, the payout set, and the feed identity are read out of the frozen terms artifact; the instruction chooses none of them. |
| 2 | `walk-02-split` | 4 | One collateral debit credits one unit of every Egg. No mint CPI and no token account: the complete set lives in the position's internal balances, and the Hoard's collateral rises by exactly the quantity split. |
| 3 | `walk-03-materialize` | 5 | Part of one outcome leaves the internal ledger for the external shadow. `total_i` is preserved exactly and the Hoard does not move. |
| 4 | `walk-04-dematerialize` | 5 | The reverse crossing, for part of what was materialized. The remainder stays outstanding externally for the rest of the walk and is what the terminal Hoard must cover. |
| 5 | `walk-05-merge` | 4 | The promise of PROJECT.md section 1 exercised: a complete set can always be recombined into its collateral **before** resolution. |
| 6 | `walk-06-feed-advance` | 6, 7 | Three contiguous observation pages fold into one feed head inside one transaction. The cursor moves 100 → 102 → 103 → 104, and 104 is exactly the maturity bound the market's window needs before it can seal. |
| 7 | `walk-07-resolve` | 7, 9 | No reporter chooses the cell. The buffer carries observation records; the gate folds them through the accumulator's `Open → Mature → Sealed` machine against the terms' own window domain, reads the matured cursor off the feed head, and refuses unless the payout index the caller named is the one the sealed window selects. |
| 8 | `walk-08-merge-after-resolve` | 4, 10 | **A recorded refusal.** The same complete-set merge step 5 accepted is refused once the market has resolved. This is the boundary the product model draws, and it is why the terminal accounting is a *redemption* identity rather than a merge identity. |
| 9 | `walk-09-redeem-winning` | 9 | The first payoff shape: the unit vector on the realized cell. Collateral leaves the Hoard for cash, one atom per claim. The resolution record is presented read-only and must come back unchanged, so a redemption can never edit its own authority. |
| 10 | `walk-10-redeem-losing` | 9, 10 | The second payoff shape: the zero vector on an unrealized cell. The claims burn and the Hoard does not move by one atom — the half of the solvency promise that is easy to state and easy to get wrong. |

### Measured

Recorded by `programs/clutch-sbf/scripts/run_bringup.sh`, one validator
session shared with the per-family gate, on `aarch64-apple-darwin`,
`solana-cli 4.0.2 (Agave)` / `cargo-build-sbf 4.0.0 platform-tools v1.53`.

- `HEAD` = `cf87451`, working tree **DIRTY** (the gate prints the file list; it
  included in-flight edits to `programs/solana-reference` and
  `programs/clutch-sbf/program`, both of which are ELF inputs). The digest
  below therefore names this working tree and not a commit. Re-run on a clean
  tree and re-record before citing the digest.
- ELF built twice into fresh target directories:
  `sha256=6e737ed64b106d74b48dccef0c07c29f23a9495ffd8ee4b4769ffe85f362ce9b`,
  402 192 bytes, `sbf_reproducibility=PASS`.
- Genesis: **122** program-owned accounts, one validator invocation. Of those,
  the walk contributes eight market planes of eight accounts each plus three
  observation-page buffers.
- The per-family gate in `SBF_BRINGUP.md` passed in the same session, before
  the walk: 9 accepting transactions, 17 refusals, 0 undrivable.

```text
   #  step                                 CU  tx bytes  written  differential
   1  walk-01-create-market            695567       822        8  MATCH
   2  walk-02-split                     76611       650        5  MATCH
   3  walk-03-materialize               76679       683        4  MATCH
   4  walk-04-dematerialize             70680       683        4  MATCH
   5  walk-05-merge                     82731       650        5  MATCH
   6  walk-06-feed-advance         15663 [6207, 4728, 4728]       669        1  MATCH
   7  walk-07-resolve                  531627       681        4  MATCH
   8  walk-08-merge-after-resolve       85784       650        0  REFUSED Custom(0x0016)
   9  walk-09-redeem-winning           427800       689        5  MATCH
  10  walk-10-redeem-losing            426300       689        4  MATCH
```

`CU` is the compute the program under test consumed, filtered by program id so
the `SetComputeUnitLimit` instruction ahead of an expensive step is not
miscounted. Step 6's bracketed list is per instruction: three feed advances in
one transaction, 6 207 for the first (which folds two observations) and 4 728
each for the two single-observation pages. `written` is how many compared
accounts actually changed — a step whose expectation happened to equal its
pre-state could not pass by accident, because the plan marks each role
`changed` or `unchanged` and the gate requires it.

Four steps carry a `SetComputeUnitLimit` of 1 400 000 because they do not fit
the runtime's 200 000-unit default: `create-market` at 695 567 (50% of the
ceiling), `resolve` at 531 627 (38%), and the two redemptions at ~427 000
(31%). The whole walk costs **2 489 442** program units across ten
transactions. That is a resource finding, not a victory lap: one fixture, two
outcomes, three observations, one position.

**Step 8's refusal code is a deliberate vocabulary difference, not a
disagreement.** The offline reference adapter refuses a post-resolution merge
as the generic `MismatchedState` (`0x300e`); this program *refines* it to
`ClutchError::NotActive` (`0x0016`), one of the two refinements
`split.rs`'s projection table documents. The harness pins both halves: it
asserts the reference's code and expects the program's, so a drift on either
side is loud rather than absorbed.

## The terminal accounting identity

Step 10 leaves the market drained except for one thing: the claims that were
materialized in step 3 and never brought back in step 4. There is no
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
third time — out of the bytes the bank returned for step 10, at an offset the
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

In words. The walk opened with 64 atoms of cash and split 20 of them into
complete sets. It materialized 8 claims of the winning outcome, brought 5
back, and merged 4 complete sets before resolution. After the window sealed on
the winning cell it redeemed its 13 remaining winning internal claims for 13
atoms and burned its 16 losing internal claims for nothing. What is left is 3
materialized winning claims that no instruction can redeem, and the Hoard
holds exactly 3 atoms against them: **not more, which would be stranded
collateral, and not less, which would be an insolvent market.** The 61 atoms
of cash and the 3 in the Hoard are the 64 it started with.

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

**SKIPPED.** There is no Realm, Profile, price-grid, or terms initialization
instruction in this program. The walk drives `CreateMarket` and nothing else of
item 1; the Realm-wide plane is loaded at genesis as frozen bytes the frozen
codecs accept. "Initialize a realm and market" is therefore half-driven, and
the half that is driven is the market.

### Item 2 — prepay all mandatory work

**SKIPPED.** No endowment, prepayment, or deposit instruction exists, and
nothing in this instruction set moves collateral into a Position. The walk
credits its opening cash in the fixture. That credit is the **one** field of
the walk's opening state that `CreateMarket` did not write, and the harness
asserts exactly that: every other byte of the opening state is `CreateMarket`'s
own post-state.

This is the sharpest gap in the walk. Protocol sustainability (PROJECT.md
section 7) is a claim about prepaid resources, and this walk contains no
prepayment.

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
- **Not one committed sequence.** See the first section: it is ten pre-states
  chained by the reference adapter, plus one three-instruction chain the bank
  sequenced itself.
- **Not a token movement.** There is no CPI and no token program in the ELF.
  Every "collateral" quantity in this walk is a field in a program-owned
  account, not an SPL balance.
- **Not evidence about any deployment.** Loopback only, one local
  `solana-test-validator`, nothing committed to any ledger.
- **Not an envelope.** One fixture, two outcomes, one three-observation
  window, one position. Worst-case outcome counts, multi-position closure, and
  larger evidence buffers are unmeasured, and every compute number would grow.
- **Not verified.** Nothing here is a proof. The offline reference adapter
  agreeing with the SBF program on this trajectory is two implementations
  agreeing on one trajectory.

### The two feed heads

Step 6 advances one feed head; step 7 resolves against another. They are two
accounts of two feed identities, because one address cannot hold both cursor
100 and cursor 104 in one genesis. The harness asserts that step 6's three
advances land the cursor on **exactly** the value step 7's head carries, and
the cursor is the only fact the resolve gate reads off a feed head
(`require(feed.feed == market.feed)` then `feed.cursor`). The two heads'
page counts differ, because they are two identities with two page histories;
nothing in the gate reads that field.

This is a named artifact of simulate-only execution, not a modelling choice.
Under a committing runtime it would be one head.
