# The fee-bearing Direct trade, as a pair — 2026-08-31

**The result:** the fee-bearing Direct trade executes as two transactions, both
real, on five real ELFs against one bank. `FEE_SECOND_TRANSACTION_V1`'s lane C
exists: `DCLTDFS1`, a permissionless Trading top-level route that reads the
buyer's `fee_owed`, projects the whole fee request from state, signs one Custody
CPI, clears the obligation, and verifies what it did. Neither half is near the
1,400,000 per-transaction ceiling.

The measurement that motivated the design is retired by it. The single
transaction was over the ceiling by more than the whole fee leg costs; the fill
alone is now within a rounding error of a zero-fee fill, which is the design's
§4.3 prediction — *"tx1 returns to the zero-fee cost profile"* — measured rather
than inferred.

Instrument: `programs/dclutch-trading-sbf/program-test/tests/direct_hot_fee_pair.rs`,
run by `programs/dclutch-trading-sbf/program-test/run-fee-pair.sh`. Seven tests,
all passing, one fixture draw (`PAIR_SEED = 0`).

## 0. The question this closes

`FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md` closed with **"Nothing about
Trading"** as its first open item:

> Whether Trading can authenticate itself, read `replay.last_request_digest`,
> and project a well-formed fee request in tx2 is untested; that is §6 lane C's
> work.

**The answer has no mechanism in it.** Custody's admission for a delegated
transfer rebuilds `CallerAuthoritySeedsV1` out of the request's own bytes and
compares the derived address against frame coordinate 0, and that address is a
PDA **of the caller program**. So the whole of Trading's self-attestation is
that it can produce a signature for that PDA, which only Trading can, and which
it does with `invoke_signed`. The sixth seed is the digest of the request
Trading has just built — not of anything committed earlier — so there is nothing
for the fill to have registered and nothing for the settlement to look up.
Custody binds `request.caller_program` to the Trading role of the activated
release set (`authenticate_calling_release` → `Release`), and the replay
independently refuses any other. Every link of that chain already shipped.

What the route owes is therefore not authentication but **derivation**: every
economic value in the fee request must come from program-owned state, or a
stranger's submission is not effect-free. §1.4's field table is now code, and it
lives in the codec crate
(`dclutch_direct_codec::fee_settlement_v1::project_direct_fee_request_v1`) so
that the caller and the program build the same bytes from the same function.
They have to: a builder that reproduced the table separately would address an
authority nothing signs the moment the two drifted by a byte, and the refusal
would name the authority rather than the drift.

## 1. The numbers

| | transaction CU | frame |
|---|---:|---|
| tx1, the fee-bearing Direct fill | **1,280,996** | the canonical top-level Hot frame |
| tx2, `DCLTDFS1` fee settlement | **169,590** | 19 accounts in the instruction, 20 in the transaction |
| the pair | 1,450,586 | — |

**Against §4's estimate.** The design estimated tx2 at 165,000–210,000 CU in an
18–21 account frame. Measured: **169,590** in a 19-account frame. The estimate is
confirmed, and the term it could not price — the Trading route's own
authentication on top of the fee leg — is the difference between this and
FEEPROOF's 149,210 minimal-caller whole-transaction figure: **20,380 CU**,
inside the 16,000–61,000 the band left for it.

**Margin.** tx2 leaves over 1.2M CU unused, so ALL KEYS is satisfied there by
margin rather than by mechanism, exactly as §4.3 argued: no combination of
`find_program_address` draws can produce a key-dependent refusal when ten
unlucky searches at 1,500 CU each is one percent of the headroom.

One fixture draw for the pair above. The seed moves PDA search depths and
nothing else, and tx2 has no distribution question worth asking at that margin.
tx1's distribution is swept by `direct_hot_fee_bearing_margin_gate.rs`,
rewritten in this lane to the one-Custody-route shape, at thirty-two seeds per
arm on one ELF set:

| arm | executed | best | worst | mean | band | worst margin | key-independent floor |
|---|---|---:|---:|---:|---:|---:|---:|
| fee-bearing | **32/32** | 1,271,994 | 1,295,997 | 1,280,672 | 24,003 | **104,003** | **1,262,994** |
| zero-fee | 32/32 | 1,272,125 | 1,294,627 | 1,280,803 | 22,502 | 105,373 | 1,263,125 |

**The fee-bearing floor is 131 CU BELOW the zero-fee one** — less than a tenth
of one bump attempt, which is to say the two shapes are the same route. The same
131 was measured before this lane merged main, on a different ELF set with every
bump depth redrawn, which is what a key-independent statistic is for. The same statistic before the
split was a LOWER BOUND of 1,435,274 on a route that could not be run to
completion, 35,274 over the ceiling.

That retired the file's decomposition test, which existed only because a route
over the meter reports the meter and cannot report itself. It had written its
own retirement condition into an assertion — *"the fee-bearing arm EXECUTED at
seed N. That is the good news this file was written to be able to report"* — so
it is deleted rather than left passing on a premise that is now false.

## 2. The ledger, read back off the bank

`FEE_BEARING`: fill 400 at 50 of 100, so gross 200, one basis-point-floored fee
of 1 per side, `seller_net = 199`, `combined_fee = 2`, `buyer_debit = 201`.

| | buyer | seller | fee recipient | delegated | `fee_owed` |
|---|---|---|---|---|---|
| **S0** staged | 1,000 | 30 | 40 | 201 → custody authority | — (no replay yet) |
| **S1** after tx1 | 801 | 229 | 40 | **2** → custody authority | **2** |
| **S2** after tx2 | **799** | 229 | **42** | **0**, delegate cleared | **0** |

Every cell is asserted, and so are three things about them:

* **Collateral is conserved in all three states.** `B + S + F` is invariant,
  which is the design's Identity 1 and the property the intermediate state does
  not weaken.
* **The settlement identity is FALSE in S1 and true in S2**, asserted as its own
  statement rather than hidden inside the conservation one. S1 is conserved and
  unsettled, and those are different words.
* **The seller is whole the instant tx1 lands.** Their half of the fee is a
  reduced credit and not a later debit, so the whole unpaid `combined_fee` sits
  in the buyer's account and the party short is the venue (§2.2).

The settlement's receipt reports what it did, and the replay is read back to
confirm it: `next_revision`, `last_request_digest` and
`last_poststate_commitment` all equal what the receipt claims.

## 3. The refusals, each reaching the check it names

| a submitter who… | refuses at | code |
|---|---|---|
| settles a maker who never owed | Trading's route | `FeeNotOwed` `0x400C` |
| replays a settlement that landed | Trading's route | `FeeNotOwed` `0x400C` |
| routes the fee to a stranger | Trading's route | `FeeDestination` `0x400D` |
| takes the fee out of somebody else's account | Trading's route | `FeeSource` `0x400E` |
| substitutes another maker's replay | Trading's route | `Content` `0x4003` |
| substitutes another account for the Direct root | Trading's route | `Root` `0x4002` |

**The third new code is a hostile the design's §1.4 table does not enumerate,
and it is the sharpest finding of this lane.** Custody checks `source.key ==
request.source` and the source's mint, and **never** `semantic.source_owner`.
Without the route's own pin, maker A could settle A's own `fee_owed` out of
maker B's collateral whenever B's standing delegation happened to equal A's debt
— which is exactly the state B is in between their own two transactions. A is
cleared for free, B's allowance is consumed by somebody else's obligation, B's
`fee_owed` still stands, and B is behind the §2.4 lockout **permanently**,
because the allowance they needed to settle with is gone. The obligation is the
debtor's and it comes out of the debtor's account.

**"Never owed" and "already settled" are one code on purpose.** Zero is both,
and they are the same state: nothing to move and nobody blocked (§2.4 invariant
3). The test reaches the code from both directions rather than assuming one
implies the other — the "never owed" arm uses the *seller's* replay, a real
Trading PDA of the same market at the same generation created by the same fill,
because a vacant pre-fill replay would not test this at all: there is no account
there to refuse about.

**E5's vanished-recipient condition, in both directions.** A recipient token
account closed between the fill and its settlement strands nobody: the
destination is bound by `owner == config.fee_recipient` and the Realm's mint,
never by address, so any account of that owner will do and an idempotent
associated-token-account creation is permissionless. The test admits a
*different* account of the configured recipient and refuses a stranger's,
because "any account will do" must not mean "any account".

**The pair is batchable.** The route's writability pin is one-directional
(design §3, and `fractional_retirement_v3`'s `FrameRoleV3`), so a coordinate the
settlement only reads is still admitted when the caller marks it writable —
which is what a builder batching a fill in front of it produces. Asserted
structurally, by submitting a settlement whose read-only Direct root arrives
writable, rather than by batching the two acts: they cannot share a transaction
for a compute reason, and a compute refusal would prove nothing about the pin.

**Three refusal codes moved during this lane and the reason is worth recording.**
They were written as `0x4009`–`0x400B`; main's `CloseSeal` route landed on those
three first, so the fee codes became `0x400C`–`0x400E`. A landed code must never
be renumbered — a code seen in a validator log is greppable — so the collision
resolves in main's favour by rule and not by seniority. What caught it was a
contiguity assertion this lane added because `TradingSbfError`'s band bound named
its last variant by hand: that bound stopped bounding the enum **twice in one
week**, once for `CloseSeal` and once for the fee codes, and neither would have
gone red.

## 3a. The zero-fee gate's constant moved, and here is who moved it

`TOP_LEVEL_KEY_INDEPENDENT_CU_V1` is the public Direct route's regression
detector and its own comment says raising it *"IS the act of spending margin,
and it should cost a decision and a sentence saying what got cheaper in
exchange."* It went red. Rather than raise it against the fee lane's word, the
same gate was run by the same method on three trees:

```text
  the constant as recorded on main                                1,254,251
  the fee-core lane's base, a0b1f4cb                              1,259,047   +4,796
  this lane merged with main at 59ecec5f                          1,263,125   +4,078
  --------------------------------------------------------------------------------
                                                                              +8,874
```

**+4,796 is the fee-core protocol tier**: the maker replay widening 152 → 160
for `fee_owed`, and the fee-band `require` in the transition. Neither had ever
been run against an on-chain gate, and that branch was already red here before
this lane touched it.

**+4,078 is this lane plus main's own drift**: one `write_u64` per maker replay
in the Effect — without which `fee_owed` is permanently zero on chain — the
eight extra record bytes hashed on both sides of the poststate comparison, and
the poststate projection's delegation branch. (Measured on the pre-merge tree
this lane's own share was +3,940 of it.)

Nothing got cheaper in exchange and the doc comment's bargain is not met. What
is offered instead is that the route this margin was spent on now *exists*: a
fee-bearing market was unreachable at any price before, over the ceiling by more
than the whole fee leg cost. 8,874 CU buys it, against 105,373 CU of remaining
worst-seed margin on the same sweep. That is a judgement, and it is recorded as
one.

## 4. Three landed defects this lane had to fix to get here

None of them was visible to a unit test, and all three are the same shape: a
protocol tier changed and something downstream that consumed it did not.

**1. The Effect wrote eleven of the record's twelve fields.**
`ordinary_effect_artifacts_v3::push_maker_state` is the only thing that writes a
maker replay, and the account is allocated zeroed, so a field absent from its
list is permanently zero on chain no matter what the contract-side settlement
computed. `fee_owed` arrived on `MakerReplayRootV1` with its gate, its
settlement arithmetic and its both-widths reader; it did not arrive on
`DirectMakerReplayLayoutV1`, which is the write-coordinate surface an emitter
goes through. So the finalization committed to an obligation the chain never
recorded, and the fee-bearing fill refused `TradingSbfError::Commit` after
1,155,154 CU of completed work — which is what made it look like a compute
problem instead of a missing write. Fixed with no new register: the transition
already publishes `SCALAR_COMBINED_FEE_V3` and `SCALAR_ZERO_V3`.

**2. The poststate projection revoked a delegation the chain left standing.**
`buyer_token_poststate_v3` read `custody_count == 0` for "the delegation
survives", which was true while every fill shape spent the whole allowance:
`SellerTerminal` moves the gross and closes the delegation in one leg. A
fee-bearing fill now runs `SellerIntermediate`, non-terminal by construction
*precisely so the second transaction has an allowance to spend*. The condition
is now the one `DelegatedCustodyRequestV2::validate` itself enforces: terminal
exactly when the allowance reaches zero, and only terminal revokes.

**3. FEEPROOF's own instrument is unrunnable, and nobody did anything wrong.**
`run-fee-second-transaction.sh` submits the fixture's *projected*
`FeeContinuation` request. With the continuation register pinned to zero,
`after_fee == after_seller`, so that projection states `expected_revision ==
resulting_revision` and `DelegatedCustodyRequestV2::decode` refuses it as
`RevisionOverflow`. The fixture is right — it mirrors the transition, and a
disabled route's projected bytes are a well-formed byte string that need not be
a valid request, which is what the caller-authority derivation needs them for.
The probe was submitting a disabled route's template. Its replacement is this
file, which projects the fee request from post-fill state instead. **That runner
should be retired or repointed; this lane did neither.**

## 5. What this does NOT establish

* **Any distribution for tx2.** One fixture draw. The fee-bearing fill's
  distribution is `direct_hot_fee_bearing_margin_gate.rs`'s, updated in this
  lane to the one-Custody-route shape; the settlement's is unswept and, at over
  a million CU of headroom, uninteresting.
* **Anything on a real validator.** Program-test, `Immutable` substrate, no
  devnet write.
* **Any rate other than 50 bps.** The shape is rate-independent by argument —
  `combined_fee != 0` is the only thing the routes read — which is an argument
  and not a measurement.
* **The operator and TypeScript builders**, or the trade panel's unsettled-fee
  surface. That is the design's lane E and it is not written. What exists for it
  is `project_direct_fee_request_v1`, which is the function a builder must call
  rather than reimplement.
* **The refusal-registry mirrors** in `apps/dclutch-web` and
  `packages/dclutch-sdk` for the three new codes. They were already stale for a
  larger set of codes before this lane, and regenerating them tree-wide is not a
  thing to do from inside one lane while others hold the same files.
* **A swept `fee_owed` interaction with the admission gate on chain.** The gate
  is unit-tested by FEE-CORE and the field is now written by the Effect, but no
  test in this lane submits a *second* fill from a maker who owes.
* **Anything about the registered (resting-order) family.** Everything here is
  the inline ordinary route.
