# The fee leg, admitted in a later transaction — 2026-08-30

**The result:** Custody accepts the Direct fee request in a transaction of its
own, in a later block, and leaves exactly the poststate
`FEE_SECOND_TRANSACTION_V1` §2.1 predicts. The design's foundation is executed,
not argued. The fee leg costs **136,943 CU inside Custody** and **149,210 CU as
a whole transaction** carrying a minimal caller, in a **fourteen-account Custody
frame**, a **fifteen-account instruction** and a **sixteen-account transaction**
— under the protocol's 200,000 CU default, with no compute-budget instruction of
any kind.

**The correction:** §1.3's refusal table sends both hostile orderings — tx2
before tx1, and tx2 replayed — to `CustodyReplayV1::advance` and
`ReplayRevisionMismatch`. Both of them measured refuse **earlier than that**, at
`CustodySbfError::TokenState` (`0x6006`), because
`delegated::execute_token_effect` reads the live SPL delegation *before*
`commit_delegated` ever compares a revision. Nothing moves in either case. The
protection is real; the site named for it is not the one that fires.

The instrument is
`programs/dclutch-trading-sbf/program-test/tests/direct_hot_fee_second_transaction.rs`,
run by `programs/dclutch-trading-sbf/program-test/run-fee-second-transaction.sh`.
Three tests, all passing.

## 0. Why this could not be run by the shipped route

Two walls, and they compose.

**The transition co-enables the two legs.** `fixture::custody_registers`
reproduces the Effect's own `select_zero` arithmetic:
`intermediate = fee_nonzero && seller_net != 0` gates `SellerIntermediate` and
`FeeContinuation` together, off one register. There is no admissible scenario at
this market's 50 bps that projects the fee leg alone: `FeeSole` needs
`seller_net == 0`, which needs `gross == fee`, which needs the rate to be the
whole 10,000. So the shipped Direct route emits both Custody CPIs or neither.

**And both together do not fit.** `DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md`
measured a key-independent floor over the 1,400,000 ceiling. On this substrate
the fee-bearing route has never *completed*, which means the `FeeContinuation`
admission — a non-terminal continuation of another leg's atomic debit — had
never returned successfully anywhere in this tree. It has now.

## 1. The instrument, and the one substitution it makes

Custody admits a delegated transfer only from the program the activated release
set binds to the role the request names — `authenticate_calling_release`
refuses `Release` unless `receipt.program() == request.caller_program` — and the
caller authority is a PDA *of that program*, so only it can sign. No third
program can present a Direct fee request beside Trading. It has to stand in
Trading's place.

So the probe stages a release set whose **Trading role is
`test-programs/custody-leg-caller`**: a 32 KB program that decodes the projected
`DelegatedCustodyRequestV2`, derives the caller authority from that request's own
seeds, and forwards the exact bytes to Custody. Everything else is real — the
real Custody, Core, Claims and Registry ELFs, the fixture's Core state, the
Registry activation cache, the Realm record, the legacy token program, and the
fixture's byte-exact projected request for each of the four declared routes.

The test asserts the two legs' caller authorities and request digests equal the
ones `direct_case_v5` installed, so the probe is addressing the accounts the
fixture planted and not four addresses of its own.

**What this buys and what it does not.** It executes Custody's admission for the
fee leg exactly as Custody executes it, in a transaction containing nothing
else. It says nothing about whether *Trading* can build such a request in tx2 —
that route does not exist and this program is not a sketch of one. Its CU
figures are Custody's leg plus a thin caller, not the design's tx2, which would
additionally carry a Trading route's own authentication. And because the release
set hashes the Trading ELF digest, every PDA depth here is redrawn: no number in
this document is comparable to a number measured on the shipped release set.

## 2. The ledger, read back off the bank

`FEE_BEARING`: fill 400 at 50 of 100, so gross 200, one basis-point-floored fee
of 1 per side, `seller_net = 199`, `combined_fee = 2`, `buyer_debit = 201`. The
fixture plants the Custody replay at revision 7.

| | buyer | seller | fee recipient | delegated | replay |
|---|---|---|---|---|---|
| **S0** staged | 1,000 | 30 | 40 | 201 → custody authority | 7 |
| **S1** after tx1 | 801 | 229 | 40 | **2** → custody authority | 8 |
| **S2** after tx2 | **799** | 229 | **42** | **0**, delegate cleared | **9** |

Every cell is asserted. So is `replay.last_request_digest`: after tx1 it is the
seller leg's request digest — which is precisely the value §1.4 says tx2's
`semantic.parent_request_digest` would be read from — and after tx2 it is the
fee leg's.

A block boundary sits between tx1 and tx2 (`warp_to_slot`), so "a later
transaction" is here also a later slot, and no same-block artefact can be
carrying the result.

## 3. The numbers

| | transaction CU | Custody CU | caller CU |
|---|---|---|---|
| tx1, `SellerIntermediate` | 146,008 | 135,302 | 10,706 |
| tx2, `FeeContinuation` | 149,210 | 136,943 | 12,267 |

Frame widths for tx2: **14** accounts in the Custody `Transfer` frame
(`TRANSFER_ACCOUNT_COUNT_V1`, checked against the contract rather than written
down), **15** in the instruction that carries it plus the callee, **16** in the
whole transaction once the fee payer is resolved.

No bump relay is sent, so Custody searches all three of the addresses it can
derive — the caller authority, its own replay, and its transfer authority. These
are un-hinted figures at one fixture draw (`PROBE_SEED = 0`); the seed moves PDA
search depths and nothing else, and this file does not sweep them.
`direct_hot_fee_bearing_margin_gate.rs` owns the distribution question.

**Against §4's estimate.** The design estimates tx2 at 165,000–210,000 CU in an
18–21 account frame. The measured Custody leg is 136,943 and the measured whole
transaction with a minimal caller is 149,210, so the estimate's band sits
**above** what the fee leg itself costs, leaving 16,000–61,000 CU for the
Trading route the design would add and 3–6 accounts for what that route needs
beyond the Custody frame. The estimate is not refuted and it is not confirmed:
the term this probe cannot measure is exactly the term the band was built to
cover. What is now known is its floor, and that the whole thing fits the 200,000
default with room, which is the claim §4 actually needed.

## 4. The two hostile orderings

| arm | outcome | code |
|---|---|---|
| tx2 with no tx1 before it | refuses, nothing moves | `0x6006` `TokenState` |
| tx2 replayed after it landed | refuses, nothing moves | `0x6006` `TokenState` |

Both are asserted against `CustodySbfError` itself, not against a literal. In
both arms the balances and the replay revision are read back and are unchanged.

The refusal site is worth stating plainly, because §1.3 gets it wrong in a way
that would mislead the lane that builds the gate. `delegated::process` runs
`execute_token_effect` **before** `commit_delegated`, and the token effect's
first state comparison is

```rust
if before_allowance.delegate != request.delegate_before
    || before_allowance.amount != request.allowance_before
{
    return Err(CustodySbfError::TokenState.into());
}
```

against the live token account. An out-of-order fee request states
`allowance_before = 2` while the chain still holds 201; a replayed one states 2
while the chain holds 0. Both die there, before `invoke_exact_transfer` and long
before any revision is compared. **The replay revision is the second line of
defence for these routes, not the first — the SPL delegation is the first.**

That matters for §2.3, which refuses the "leave the obligation as the residual
delegation" design precisely because the debtor can erase the delegation in the
ordinary course of trading. This measurement says the delegation is not merely
the *record* of the obligation the design rejected; it is also the live check
that currently enforces ordering. A `fee_owed` field on the maker replay does
not by itself change that: the request's `allowance_before` still has to match a
token account the buyer controls.

## 5. What this does not establish

* **Nothing about Trading.** The caller here is a stand-in. Whether Trading can
  authenticate itself, read `replay.last_request_digest`, and project a
  well-formed fee request in tx2 is untested; that is §6 lane C's work.
* **Nothing about the shipped release set's costs.** The Trading ELF digest
  seeds the release set, which seeds the Market, which redraws every bump.
* **Nothing about a swept distribution.** One seed.
* **Nothing about §2's obligation.** `fee_owed` does not exist and this probe
  neither needs nor exercises it. What it does show is that the pre-`fee_owed`
  world already refuses both hostile orderings — at a site the design did not
  name.
* **Nothing about the zero-fee path's semantics** (§5), which was not touched.
