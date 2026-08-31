# The fee leg as a second transaction

Status: **design, and a recommendation.** This document changes no byte of
program code. It answers the five questions a fee-bearing Direct trade raises
once the fee leg stops sharing a transaction with the fill, and it closes each
one with a mechanism and the refusal that enforces it.

It exists because `docs/evidence/DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md`
executed the fee-bearing route for the first time and found it **over the
1,400,000 ceiling by 115,003 CU on the luckiest possible key** — an
all-searches-first-try floor of 1,515,003 against a fee leg that is itself only
182,386 CU. There is no tail to optimise into and no lever inside the leg that
reaches. (**Numbers corrected**: this document was written against FEEWALL's
pre-rebase figures — 106,527 over a 1,506,527 floor, fee leg 174,119. The lane
landed on `main` at `24b2b7f2`+`3d5dda0e` with the meter-truncation artifact
fixed and the figures reproduced to the compute unit across two ELF sets, which
moved all three **upward**. Nothing in this design turns on the difference: the
gap was already larger than the whole leg either way. One derived figure below
inherits an older basis: *"the 46,592 CU of worst-seed headroom FEEWALL
measured"* (`:449`, restated at `:835`) is a pre-ALLKEYS worst-seed sample.
After ALLKEYS (`308c3dff`..`e7805d62`) tx1 has no band at all — every key costs
1,336,742 CU with a flat **63,258** of headroom — so the tx1 budget those two
passages reason against is larger and no longer a sample.) That evidence recommended (b), the two-transaction lifecycle, and
ember pre-ruled multi-transaction lifecycles acceptable (`WAVE.md`, Rulings,
2026-08-30: *"I don't care about multi-tx lifecycles; if that's what we need to
do"*).

Every claim below is **verified-from-source** (read at HEAD, cited by symbol) or
marked **ruling** (a decision this document makes) or **estimate** (a number
assembled from measured comparables, with the arithmetic shown) or — added by
the amendment in §1.3 — **measured** (executed on the program-test substrate,
with the evidence document named). Paths are
`~/dev/dclutch` unless stated otherwise. Line numbers are hints; the symbol is
the citation.

---

## 0. What this document decides

1. **Nothing binds tx2 to tx1 by transaction, and nothing ever did.** The first
   hour went into the seed derivation and the replay sequencing, and it found
   no atomicity check to remove (§1). Custody would accept the fee request in a
   later transaction today.
2. The obligation between the transactions is **`fee_owed: u64` on the buyer's
   Direct maker replay**, and the buyer's next fill in that market refuses
   while it is nonzero (§2). The elegant version — leave the obligation as the
   residual SPL delegation and store nothing — is **refused**, because the
   debtor owns the collateral and erases it in the ordinary course of trading
   (§2.3).
3. **tx2 is permissionless, unsigned, unrewarded, and undeadlined** (§3). No
   funded crank is needed and `FUNDED_CRANK_V1` is deliberately not applied;
   the reason is that document's own §3.2 rule.
4. tx2 costs an **estimated 165,000–210,000 CU** in an **18–21 account frame**
   with a wire under 128 bytes (§4). It has over a million CU of headroom, so
   ALL KEYS is satisfied by margin rather than by mechanism there; the hint
   suffix is still recommended and is one byte.
5. The zero-fee single-transaction path keeps its semantics, its register
   trace, and its route exactly (§5) — but not, automatically, its CU figures,
   and §5 says which instrument settles that.
6. Two residuals go to ember (§7). Everything else is recommended and closed.

---

## 1. What binds tx2 to tx1

### 1.1 The first hour, and its correction

FEEWALL closed with a NOT VERIFIED that named this as the design's first task:

> Whether the fee leg's Custody request would validate in a later transaction.
> §6(b)'s observation is about the replay revision arithmetic only. **The caller
> authority's seeds carry the parent request digest**, and this lane did not
> check whether Custody would accept the fee route's request outside the
> transaction that produced that parent.

**The premise is false for the Custody routes, and the correction is the whole
answer.** `derive_child_authorities`
(`crates/dclutch-operator/src/direct_inline_route_v3.rs:1626-1684`) derives two
different kinds of caller authority from the same six-seed tuple:

```rust
let claims_seeds = CallerAuthoritySeedsV1::new(
    release_set, context.market, ExecutionRoleV1::Trading,
    context.parent_request_digest,          // <- Claims: the family digest
    claims_request_digest,
)?;
...
for (index, request) in projected.custody.iter().enumerate() {
    let seeds = CallerAuthoritySeedsV1::new(
        release_set, context.market, ExecutionRoleV1::Trading,
        context.buyer_maker_root,           // <- Custody: the MAKER ROOT
        digest,
    )?;
```

The `context` seed of `CallerAuthoritySeedsV1`
(`crates/dclutch-release-set-contract/src/lib.rs`, `as_slices` → `[domain,
release_set, market, caller_role, context, role_request_digest]`) is the
**buyer maker replay root** for every Custody route, not the family request
digest. The fixture states the same fact where it derives the four route
authorities: *"the context seed is the request's own `context` field — the buyer
maker root the Effect projects into every Custody request — and never the family
request digest, which enters only as the fifth seed through the child request's
hash"*
(`programs/dclutch-trading-sbf/program-test/direct-hot/src/fixture.rs`,
`custody_route_authorities`).

So a Custody caller authority is **maker-scoped and request-scoped, never
trade-scoped and never transaction-scoped.** The family digest reaches it only
through `role_request_digest = hash(child_request_bytes)`, and those bytes are a
deterministic projection Trading recomputes from state.

### 1.2 What Custody actually checks

`authenticate_common_frame` (`programs/dclutch-custody-sbf/src/lib.rs`) is the
complete admission for a delegated transfer. It rebuilds the seeds itself and
compares:

```rust
let caller_seeds = CallerAuthoritySeedsV1::new(
    ContentId::new(request.release_set)?, request.market,
    registry_role(request.caller_role), request.context, request_digest,
)?;
let expected_caller = match carried_bump {
    Some(bump) => Pubkey::create_program_address(&[.., &[bump]], caller_program.key)?,
    None       => Pubkey::find_program_address(&caller_seeds.as_slices(), caller_program.key).0,
};
if caller_authority.key != &expected_caller {
    return Err(CustodySbfError::CallerAuthority.into());
}
```

Every seed comes out of the request bytes. **Not one of them names the
transaction, the slot, the blockhash, or the instruction index.** The
instructions sysvar is not read on this path. `CustodyRequestV1::validate`
(`crates/dclutch-custody-contract/src/lib.rs`) requires
`semantic.parent_request_digest` to be *nonzero* and nothing more — the only
route in the tree that cross-checks a parent digest against something is the
dealer reservation family
(`programs/dclutch-custody-sbf/src/dealer_reservation_v1.rs`, which compares it
against `checkpoint.input.request_digest`), and Direct is not that family.

**The answer to Q1 is therefore: Custody accepts the fee request in a later
transaction, unchanged, today.** There is no atomicity check to relax and no new
binding to invent. FEEWALL's estimate of "an hour's work, not a design" was
right, and the hour's result is that the design starts from a smaller problem
than it looked.

### 1.3 The two binders that do exist, and what each one refuses

The sequencing the two-transaction form needs is carried by two mechanisms the
accounts already hold. Both are transaction-agnostic by construction.

**Binder one — the replay revision.** `CustodyReplayV1::advance`
(`crates/dclutch-custody-contract/src/lib.rs`) refuses unless the request's
`expected_revision` equals the replay's live `next_revision`:

```rust
if request.expected_revision != self.next_revision {
    return Err(Error::ReplayRevisionMismatch);
}
```

and refuses `Error::ReplayBindingMismatch` unless `caller_role`, `release_set`,
`market`, `realm`, `context`, `caller_program` and `semantic.generation` all
equal the replay's own recorded values. The replay is a per-`(market,
release_set, caller_role, buyer_maker_root)` PDA
(`CustodyReplaySeedsV1::from_request`), so it is exactly one counter per buyer
per market per release. That gives, for free:

- **ordering** — a fee request built for revision `r+1` refuses while the
  replay sits at `r`, so tx2 cannot precede tx1;
- **at-most-once** — after tx2 the replay is at `r+2`, so a replayed tx2 refuses
  with the same code;
- **binding to this market and this buyer** — six equality checks, each with a
  named refusal.

`CustodyRequestV1::validate` additionally refuses `Error::RevisionOverflow`
unless `resulting_revision == expected_revision + 1`, so a request cannot claim
to advance the replay by two.

**Binder two — the delegated allowance.** This is the one that names the
obligation, and it is the more interesting of the two.
`DelegatedCustodyRequestV2::validate`
(`crates/dclutch-custody-contract/src/delegated.rs`) enforces:

```rust
|| self.allowance_before.checked_sub(self.custody.amount) != Some(self.allowance_after)
|| self.starts_atomic_debit != (self.allowance_before == self.total_debit)
|| self.terminal != (self.allowance_after == 0)
|| (self.terminal && !is_zero(self.delegate_after))
|| (!self.terminal && self.delegate_after != self.delegate_before)
```

Read carefully: **`starts_atomic_debit` and `terminal` are derived facts about
the allowance arithmetic, not assertions about a transaction.** The phrase
"atomic debit" describes the *allowance*, not the atom of execution. Nothing
requires a non-terminal transfer to be continued in the same transaction, and
`programs/dclutch-custody-sbf/src/delegated.rs::execute_token_effect` checks the
delegation against the live token account (`before_allowance.delegate !=
request.delegate_before || before_allowance.amount != request.allowance_before`
→ `CustodySbfError::TokenState`), which is a fact about the chain, not about the
frame.

The shipped `SellerIntermediate` route already leaves the buyer's token account
carrying `delegate = custody_authority` and `delegated_amount = combined_fee`,
with `delegate_after == delegate_before` because it is non-terminal
(`fixture.rs::custody_request_bytes`, route `SellerIntermediate`:
`allowance_before = buyer_debit`, `allowance_after = combined_fee`,
`terminal = false`). **The obligation is already a live, visible, on-chain SPL
delegation of exactly the unpaid amount.** That it is not durable is §2.3.

### 1.4 tx2's fee request, field by field, and where each field comes from

With §1.1–§1.3 established, a fee request is fully determined by chain state.
Trading builds it in tx2 from four authenticated sources and accepts no economic
value on the wire:

| field | source |
|---|---|
| `market`, `release_set`, `realm`, `caller_program`, `context`, `caller_role`, `semantic.generation` | the Custody replay's own recorded fields, which `advance` re-checks for equality |
| `expected_revision`, `resulting_revision` | `replay.next_revision`, `+1` |
| `amount`, `allowance_before` | `buyer_maker_replay.fee_owed` and the live `delegated_amount` (§2.4) |
| `allowance_after`, `terminal`, `delegate_before`, `delegate_after` | forced by `DelegatedCustodyRequestV2::validate` once the two above are fixed |
| `source` | the buyer's collateral account, named by the buyer's Direct maker identity |
| `destination` | a token account whose `owner == DirectExecutionConfigV1::fee_recipient` and whose mint is the Realm mint — the config is immutable and content-addressed (`crates/dclutch-direct-codec/src/successor.rs`, and ADR 0014 §2 on why it can never be updated) |
| `semantic.parent_request_digest`, `semantic.order` | the seller leg's request digest, read from `replay.last_request_digest` |

The last row is worth a sentence. In the single-transaction shape the fee
request's parent is the family request; in the two-transaction shape its parent
is **the seller leg it continues**, and `replay.last_request_digest` is that
digest, written by tx1 and program-owned. The field stays truthful and stays
nonzero, so `CustodyRequestV1::validate`'s `require_nonzero` is satisfied
without a caller supplying anything.

**The refusal story, complete.** Every way tx2 can be wrong already has a named
code and a site:

| a submitter who… | refuses at | code |
|---|---|---|
| submits tx2 before tx1 | the live-allowance read in `delegated::execute_token_effect`; `CustodyReplayV1::advance` never reached | `CustodySbfError::TokenState` (measured) |
| replays tx2 | the same live-allowance read | `CustodySbfError::TokenState` (measured) |
| points tx2 at another market, buyer, realm or generation | `CustodyReplayV1::advance` | `ReplayBindingMismatch` |
| forges the caller authority | `authenticate_common_frame` | `CustodySbfError::CallerAuthority` |
| supplies a wrong caller-authority bump hint | the same equality, after `create_program_address` | `CustodySbfError::CallerAuthority` |
| moves an amount other than the owed one | `DelegatedCustodyRequestV2::validate`, then the live-allowance read | `Allowance`, `CustodySbfError::TokenState` |
| routes the fee to a stranger | Trading's new route (`owner != fee_recipient`) | new, §6 lane C |
| runs tx2 with nothing owed | Trading's new route (`fee_owed == 0`) | new, §6 lane C |

Only the last two are new. Everything above them is shipped.

**Amendment, 2026-08-30, from execution.** The first two rows above originally
sent both hostile orderings to `CustodyReplayV1::advance` and
`ReplayRevisionMismatch`. Executed, both refuse one function *earlier*, at
`CustodySbfError::TokenState` (`0x6006`): `delegated::process` runs
`execute_token_effect` before `commit_delegated`, and the token effect's first
state comparison is `before_allowance.delegate != request.delegate_before ||
before_allowance.amount != request.allowance_before` against the **live** token
account. An out-of-order fee request states `allowance_before = combined_fee`
while the chain still holds `buyer_debit`; a replayed one states `combined_fee`
while the chain holds zero. Both die there, before `invoke_exact_transfer` and
long before any revision is compared, and nothing moves in either case. So
**the delegated allowance is the first line of defence for these routes and the
replay revision is the second** — which §2.3 has to reckon with, because the
obligation it refuses to rest the design on is the same object that currently
enforces ordering, and a `fee_owed` field on the maker replay does not change
that on its own: the request's `allowance_before` still has to match a token
account the buyer controls. The word "hostile-tested" also came out of the
sentence above, and it had to: because the fee-bearing route is over the compute
ceiling and its two Custody legs are co-enabled by one register, the
`FeeContinuation` admission had **never returned successfully anywhere in this
tree** before this measurement — it was reached, never completed. Evidence, with
the CU and account figures and the S0/S1/S2 ledger read back off the bank:
`docs/evidence/FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md`; instrument:
`programs/dclutch-trading-sbf/program-test/tests/direct_hot_fee_second_transaction.rs`.

---

## 2. If tx2 never lands

### 2.1 The three states, and the ledger identity in each

Write `B`, `S`, `F` for the buyer, seller and fee-recipient collateral balances
and `D` for the buyer's delegated amount to the market's Custody authority.
`gross = fill · price / scale` exactly (`mul_div_exact`), `fee = floor(gross ·
bps / 10000)`, `seller_net = gross − fee`, `buyer_debit = gross + fee`,
`combined_fee = 2·fee` — all from
`crates/dclutch-direct-aot-v3-contract/src/lib.rs`.

| | B | S | F | D |
|---|---|---|---|---|
| **S0** admitted, pre-fill | `B₀` | `S₀` | `F₀` | `buyer_debit` |
| **S1** tx1 landed, fee unpaid | `B₀ − seller_net` | `S₀ + seller_net` | `F₀` | `combined_fee` |
| **S2** tx2 landed | `B₀ − buyer_debit` | `S₀ + seller_net` | `F₀ + combined_fee` | `0` |

**Identity 1 — collateral conservation. Holds in all three states.**
`B + S + F` is invariant, because `buyer_debit = seller_net + combined_fee` is
proved by the transition itself (`require(checked_add(seller_net,
combined_fee)? == buyer_debit)`, `lib.rs:180`) and because conservation is a
per-transfer property, not a per-transaction one. This is
`admitted_collateral_conserved` (`DirectProofs.lean:128-133`), and **the
intermediate state does not weaken it.**

**Identity 2 — allowance conservation. Holds in all three states.**
`total_debit − D = Σ amounts moved under this atomic debit`. S1:
`buyer_debit − combined_fee = seller_net`. S2: `buyer_debit − 0 = seller_net +
combined_fee`. This is `DelegatedCustodyRequestV2::validate`'s
`allowance_before − amount == allowance_after` composed along the sequence, and
`terminal == (allowance_after == 0)` is the protocol's own statement that the
sequence has closed.

**Identity 3 — the settlement identity, and it is FALSE in S1.** The makers
signed a fill in which the buyer is debited `buyer_debit` and the fee recipient
credited `combined_fee`. In S1 both are short by exactly `combined_fee`. Naming
this as a temporarily-false identity is the honest move; pretending Identity 1
covers it is not. **S1 is conserved and unsettled, and those are different
words.**

### 2.2 Who is short, and it is not the seller

The arithmetic decides this and it is worth stating loudly, because it is what
makes the fee the right leg to move.

`seller_net = gross − fee` moves **in tx1**. The seller's half of the fee is
realised as a *reduced credit*, not as a later debit, so **the seller is whole
the instant tx1 lands.** The entire unpaid `combined_fee` sits in the buyer's
account, un-debited. If tx2 never lands, the buyer has acquired the claims for
`gross − fee` instead of `gross + fee` — they keep `2·fee`, which at 50 bps is
**1% of the fill's notional** — and the party short is the venue.

That is the reason the fee leg, and not the seller leg or the Claims leg, is the
one to defer. Deferring the seller leg leaves a party who consented to nothing
holding a claim-shaped IOU; deferring the Claims leg does the same to the buyer.
**The fee leg is the only one of the four whose deferral leaves a bounded,
single-beneficiary shortfall owed to the party that wrote the policy.**

### 2.3 The elegant design, and why it is refused

The obvious design stores nothing. The residual delegation *is* the obligation:
it names the amount, it names the creditor (only the Custody authority can spend
it), and it is already there. tx2 reads
`replay.last_poststate_commitment`, is handed the
`DelegatedCustodyPoststateFactsV2` preimage as untrusted caller data, hashes it,
compares, and thereby authenticates `allowance_after`, `source`,
`delegate_after` and the seller leg's digest — untrusted data checked against a
program-owned commitment, which is exactly O-016's shape and exactly ember's
"trust ratchets forward as state mutates" ruling. No new account, no rent, no
new bytes, and **zero added CU in tx1**.

**Refused.** The collateral is an SPL delegation on an account the *debtor*
owns, and the debtor can erase it two ways:

- `Revoke` clears the delegate outright;
- `Approve` **sets** the delegated amount rather than adding to it —
  `source_account.base.delegate = PodCOption::some(*delegate_info.key);
  source_account.base.delegated_amount = amount.into();`, `process_approve`,
  `spl-token-2022-11.0.0/src/processor.rs` (the audited v11 this tree ships
  against) — so the buyer's next trade in any market on this mint wipes the
  residue as a normal side effect of trading. The admission flow issues exactly
  that instruction
  (`approve_checked(.., custody_authority, participant, collateral.quantity_atoms, ..)`,
  `tools/local-validator/bootstrap/successor/src/user_position_admission.rs`),
  and the fill requires it: `collateral.buyer_source.delegated_amount !=
  settlement.effects.buyer_collateral_debit` refuses
  (`crates/dclutch-direct-codec/src/inline_candidate_v2.rs::validate_collateral`).

So the failure is not an attack the venue can price. **It is the default
behaviour of a returning customer**, and under the commitment-only design the
second fill silently overwrites `last_poststate_commitment` too — the first
fill's obligation becomes unreachable, with every conservation identity still
true. A fee that is silently forgiven by the buyer's next trade is not a fee.

The general rule, stated so the next lane does not re-derive it:

> **No amount of program-owned bookkeeping secures a debt whose only collateral
> is an allowance on the debtor's own account.** A record makes the debt visible
> and permanent; it does not make it collectable. Collectability comes from
> making non-payment cost the debtor something the venue controls.

That rule also disposes of the "fee ticket PDA" variant. A ticket records the
amount durably and still cannot compel the allowance to exist when tx2 runs. Its
only added power over a field on an account tx1 already writes is the ability to
hold *several* unsettled fees at once, which is a capability this design
deliberately does not want (§2.4).

### 2.4 The recommendation: `fee_owed` on the buyer's maker replay

**Ruling.** The obligation is a `u64` field on the buyer's Direct maker replay,
and its being nonzero refuses that maker's next fill in that market.

```
DirectMakerReplayLayoutV1        152 bytes today, exactly packed
  (crates/dclutch-direct-codec/src/successor.rs;
   DIRECT_MAKER_REPLAY_BYTES_V1 = 152 in generated_successor.rs)
    0   magic          8      88  next_nonce            8
    8   version        2      96  live_count            8
   10   bump           1     104  minimum_live_nonce    8
   11   reserved       5     112  rent_owner           32
   16   market        32     144  rent_principal        8
   48   generation     8    ---- 152 ----
   56   maker         32     152  fee_owed              8   <- proposed, → 160
```

Why this account and not another:

- **It is already in tx1's writable set and already in the finalization's
  poststate list** — `DirectInlineAccountPrestatesV3::buyer_maker_replay`, one
  of the ten `DIRECT_INLINE_POSTSTATE_COUNT_V3` roles
  (`crates/dclutch-direct-codec/src/direct_finalization_v3.rs`). Writing a field
  there is a change to a projection that already runs, not a new account, a new
  CPI, or a new frame coordinate.
- **Its scope is exactly the obligation's scope** — one per `(market, maker)`,
  which is also the scope of the Custody replay's `context` (§1.3). At most one
  unsettled fee can exist per buyer per market, which is the invariant that
  makes the gate a single comparison.
- **Its rent already exists and is already accounted.** The record carries
  `rent_owner` and `rent_principal`, and the lifecycle creates and closes it.
  So the funding question that `FUNDED_CRANK_V1` §2 exists to answer never
  arises: nothing new is created and nothing new is closed. That document's own
  §3.2 is the authority — *"before choosing a cap, confirm there is a residual
  at all. A cap on nothing is a conversion that cannot exist."*
- **The 5 reserved bytes are not enough and should not be used.** Forty bits
  bounds `combined_fee` at 1.1×10¹² atoms — about 1,100 tokens on a 9-decimal
  mint. A fee mechanism that refuses large fills is worse than one that costs
  eight bytes. Widen to 160 and leave the reserved range canonical-zero.

The invariants the design must carry, stated so they can be tested rather than
assumed:

1. `fee_owed != 0` ⟹ the Custody replay's last receipt is non-terminal, i.e.
   the buyer's account carries an obligation the market can still spend.
2. `fee_owed == 0` is a precondition of admitting a new fill for that maker.
3. tx1 sets `fee_owed := combined_fee`; tx2 sets `fee_owed := 0` and moves
   exactly `combined_fee`; no other route writes the field.
4. tx2 refuses `delegated_amount < fee_owed`, and transfers exactly `fee_owed`
   — never "whatever is delegated", which is how a buyer who re-approved a
   smaller amount would settle short and clear the flag.

**The cure is the debtor's, which is what makes this a gate and not a
punishment.** A buyer who revoked can re-`Approve` `combined_fee` and then
submit tx2 themselves — tx2 is permissionless (§3). A buyer who does neither
keeps `2·fee` and is locked out of that one market, on that one immutable
config, forever. The venue's exposure is bounded at one fill's `combined_fee`
per defaulting maker per market, and the default is self-limiting because it
costs the defaulter their access to the thing they defaulted on.

### 2.5 The three dispositions the mission named, ruled

| option | ruling |
|---|---|
| **funded permissionless crank** (`WorkRewardV1` / record `Abort`) | **Not applied, and the reason is `FUNDED_CRANK_V1`'s own.** Nothing closes, so there is no residual to carve a reward from (§3.2), and a prepaid bounty would need a new account created in tx1 — 8,000–15,000 CU (estimate) out of the 46,592 CU of worst-seed headroom FEEWALL measured. More decisively: a reward is unnecessary because the two parties who care can both call. The fee recipient will crank to be paid; the buyer will crank to be unblocked. That is the census's GREEN-SELF shape, and `FUNDED_CRANK_V1` §9 already prefers it (*"when the caller IS the refund wallet they are already the beneficiary and need no reward"*). |
| **the maker's next action gated on fee settlement** | **Adopted** (§2.4). It is the only option that makes the fee collectable rather than merely recorded. |
| **owed-and-crankable forever** | **Adopted as the duration**: no deadline, no expiry, no forgiveness. See §7 residual 1 for why a deadline would be worse than none. |

---

## 3. Who may submit tx2

**Ruling: permissionless. No account in tx2's frame is a signer except the
transaction's fee payer, and the fee payer's key is checked against nothing.**

The tree has already made this argument twice, in the two places that matter,
and the second is a live defect the census caught.

`programs/dclutch-claims-sbf/src/fractional_retirement_v3.rs`, module header:

> **Nothing here is privileged.** All three acts are permissionless, because all
> three are fully determined by state that is authenticated before they run …
> A route that additionally demanded a signature would hand whoever holds it the
> power to strand every shard holder's collateral behind a walk nobody else may
> crank — the hostage-taking this family already refuses one phase earlier.

`FUNDED_CRANK_V1` §6, the general rule: *the caller's signature — where one is
present at all — establishes who is owed, never who is permitted* — and the
failure mode it names is the dealer checkpoint cleanup, which **refuses**
`beneficiary.is_signer`, so the one party with an interest in the cleanup is
forbidden to turn its own crank.

Applied here the argument is sharper than usual, because **both** obvious
signers are disqualified for opposite reasons:

- **the buyer** profits from non-settlement, so a buyer signature makes the
  obligation optional and the gate (§2.4) unenforceable;
- **the fee recipient** would then hold a lock on the buyer's access to the
  market — the gate would become a hostage — so a recipient signature converts a
  fee mechanism into a censorship mechanism.

Neither party may be the gate, so nobody signs. And because tx2's every economic
value is derived from authenticated chain state (§1.4), a stranger's submission
is effect-free beyond the outcome the fill already fixed. That is the same
justification `generic_founding_stages_v1.rs` gives for its stage 2 — *"before
`expiry_slot`, anyone may complete stage 2 (submission is permissionless because
it is effect-free beyond the pinned outcome)"*.

**One bounded degree of freedom the submitter does keep**, named rather than
hidden: the fee *destination* is checked by `owner == fee_recipient` and mint,
not by address, so a stranger may route the fee into a different token account
belonging to the same recipient. Nothing is stolen — the recipient owns it
either way — and pinning the exact account would require carrying an address
tx1 did not commit. **Recommendation: accept it, and say so in
`docs/guides/trader.md`.**

**Frame writability, and this will bite whoever implements it.** Use
`FrameRoleV3`'s one-directional pin
(`fractional_retirement_v3.rs`), not an exact `(signer, writable, executable)`
triple. `is_writable` merges across a transaction's instructions, so a
coordinate tx2 only reads arrives `true` whenever the caller's *other*
instruction had to write it — which is precisely the case a builder that batches
tx1 and tx2 into one transaction produces. FRACLIFE shipped that bug
(`PRIVILEGE_PIN_UNEXEMPTED`, fixed at `80b78181`) and Custody shipped it before
that (`16351a13`). A two-transaction lifecycle whose two acts are unbatchable is
a worse lifecycle for no gain.

---

## 4. The wire, the frame, and the CU

### 4.1 tx2 must be a Trading instruction. This is structural, not stylistic.

The Custody caller authority is a PDA of the **caller program**, and only that
program can sign it (`Pubkey::find_program_address(&caller_seeds.as_slices(),
caller_program.key)`, with `caller_program.key.to_bytes() !=
request.caller_program` → `CustodySbfError::Release`). The replay independently
refuses any request whose `caller_program` differs from the one it recorded
(`advance` → `ReplayBindingMismatch`). The replay for a Direct trade records
Trading. **So tx2 is a Trading top-level instruction that makes one Custody CPI,
and no other shape is reachable.**

Note also what tx2 must *not* be: a Registry continuation. The continuation is
a CPI-depth mechanism inside one transaction, not a second transaction — it
costs a measured, seed-independent **+35,127 CU**, and
`continuation_roles(OperationV1::Transfer)` returns `None`
(`programs/dclutch-custody-sbf/src/lib.rs`), so a delegated Transfer is not
reachable under a continuation at all.
`docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md` recommends
demoting that route to harness-only; nothing here depends on the outcome.

### 4.2 The account frame

Custody's `Transfer` frame is exactly 14 accounts
(`TRANSFER_ACCOUNT_COUNT_V1 = 14`,
`crates/dclutch-custody-contract/src/frame_spec_v1.rs`), with roles and
privileges owned by `CustodyFrameSpecV1::account`:

| # | role | privileges |
|---|---|---|
| 0 | `CallerAuthority` | signer |
| 1 | `CoreMarket` | readonly |
| 2 | `ActivationCache` | readonly |
| 3 | `RegistryProgram` | executable |
| 4 | `CallerProgram` (Trading) | executable |
| 5 | `CallerProgramData` | readonly |
| 6 | `RealmRecord` | readonly |
| 7 | `RealmStaging` | readonly |
| 8 | `Replay` | writable |
| 9 | `Mint` | readonly |
| 10 | `TransferSource` (buyer collateral) | writable |
| 11 | `TransferDestination` (fee collateral) | writable |
| 12 | `CustodyAuthority` | readonly |
| 13 | `TokenProgram` | executable |

tx2 adds, on top of those fourteen: the **Custody program** (the callee — and
`require_custody_frame_shape_v3` refuses a frame that *carries* its own callee,
so it goes beside the frame, not in it), the **buyer's Direct maker replay**
(writable, to clear `fee_owed`), the **Trading capability root** (readonly, for
`TradingSbfError::Root`), and the **immutable Direct config record plus its
staging cursor** (both readonly — the vacant-cursor pair is how this tree spells
"immutable", `crates/dclutch-registry-contract/src/immutable_registry.rs`).

**Total: 18–19 accounts, plus the transaction fee payer.** Compare FRACLIFE's
three acts at 16 / 22 / 13. No address lookup table is required; one may still
be used.

### 4.3 The CU floor — an estimate, with its arithmetic

The closest measured comparable in the tree is a **whole transaction that does
exactly tx2's payload**: a wrapper program CPIs Custody once for a delegated
external transfer, with the caller-authority PDA signing.

```
custody-token-2022-delegated-external-transfer   139,746 CU measured
custody-legacy-delegated-external-transfer       148,297 CU measured
    tools/gauntlet/CU_BUDGETS.json; the campaign is
    programs/dclutch-custody-sbf/tests/program_test.rs, step
    "delegated external transfer"; band 0 over two runs at one revision
```

To that, tx2 adds Trading's own preamble and its two small reads. The nearest
measured Trading route that authenticates a release and drives one Custody
operation is `dcltpca1-unwind` at **159,496 CU** — 19,750 above the bare
wrapper. It is a generous comparable rather than an exact one, and generous in
tx2's favour: it is a 36-account frame
(`PROJECTED_CUSTODY_ABORT_ACCOUNTS_V1 = 19 + 17`,
`tools/local-validator/bootstrap/successor/src/market.rs`), it drives the
*projected* Custody path rather than the delegated one, and its operation
(`AbortSourceAndClose`) closes an account where a Transfer does not.

```
  payload, measured (Token-2022 arm)                        139,746
  Trading release/root/preamble, from the DCLTPCA1 delta     20,000
  immutable config record read + content-address reproduce    3,000
  maker replay decode and the fee_owed clear                  3,000
  caller-authority derivation, hinted (one create_program_address)  ~0
                                                           ---------
  central estimate                                          ~166,000
  stated band, allowing for a heavier Trading preamble
  and an unhinted search                                165,000-210,000
```

**Margin under the ceiling: about 1,190,000 CU, roughly 85% of the budget
unused.** Two consequences follow, and the second is the important one:

1. tx1 returns to the zero-fee cost profile. tx1 keeps the shipped
   `SellerIntermediate` route (§5), which differs from `SellerTerminal` only in
   six patched scalars in a request of identical width, so its Custody span
   should land within noise of the zero-fee control's measured 126,399–135,399.
   The whole fee-bearing overrun disappears with the second CPI, and it takes
   two of the route's ten search instances with it.
2. **ALL KEYS is satisfied in tx2 by margin, not by mechanism.** ember's ruling
   — *"It's completely unacceptable that ANY numerator of keys fails to
   transact"* — is about a key-dependent refusal. At 166,000 CU against
   1,400,000, no combination of `find_program_address` draws can produce one;
   ten unlucky searches at 1,500 CU each is 15,000 CU, one percent of the
   headroom. tx2 is the first route in the Direct family where that is true.

### 4.4 Bump hints

`HotBumpHintsV1` — ALLKEYS' block at envelope offset 120, eight slots, **branch
`lane/allkeys` only; it is not on main** — does not apply to tx2, for three
reasons and one of them is a hard constraint:

- tx2 is not a `DCLTHOT3` submission. It has its own wire, so it inherits the
  envelope's decode of nothing.
- **All eight slots are already spoken for** by the Direct hot route (market,
  root, two lifecycle, two child caller, two child relay). There is no ninth,
  and the block was placed *inside* the envelope precisely so the packet would
  not grow — the Registry continuation has four spare bytes of the v0 ceiling.
- tx2 does not need it to fit (§4.3).

**Recommendation: use the suffix carrier instead, which tx2 can afford and the
hot route could not.** `split_caller_authority_bump_v1`
(`programs/dclutch-custody-sbf/src/lib.rs`) already reads a one-byte bump
appended *after* a Custody request, dispatching on exact length with a
compile-time non-collision assertion over the five known wire widths, and
`authenticate_common_frame` reproduces the address with
`create_program_address` rather than trusting it — *the derivation is the
check*. tx2 carries a fresh 1,232-byte packet and needs nothing else on it, so
one appended byte is free. Do the same for the maker replay's bump, which
Trading must reproduce anyway.

The wire itself: a magic, `release_set`, `market`, the buyer maker identity, and
two bump bytes — **under 128 bytes**, with no economic value on it at all.

---

## 5. What this does not change

- **The zero-fee single-transaction path keeps its route, its register trace and
  its semantics exactly.** A trade whose fee floors to zero sets
  `seller_terminal = 1` and all three other enable registers to `0`
  (`crates/dclutch-direct-aot-v3-contract/src/lib.rs:205-220`), makes one
  Custody CPI, and is untouched by anything here.
- **But its CU figures are not automatically preserved, and the claim must be
  measured rather than asserted.** The transition and its AoT mirror change by
  one register (§6, lane A), so the Trading ELF changes, and this tree does not
  let a CU number travel between builds — `release_set_id` hashes the five role
  ELF digests. FEEWALL built exactly the right instrument for this:
  `direct_hot_fee_bearing_margin_gate.rs`'s `ZERO_FEE` arm is a byte-identical
  reproduction of the historical fixture, and the gate that must stay green is
  its 32-seed sweep at floor 1,318,907 / worst 1,353,408, with the three-arm
  wrong-bump control still partitioning 16,387 / 24,580 / 24,580.
- **The four declared Custody routes keep their shapes and their addresses.**
  tx1 runs `CUSTODY_ROUTES_V3` slot 1 (`SellerIntermediate`) unchanged —
  non-terminal, `delegate_after == delegate_before`, `allowance_after =
  combined_fee`. tx2 runs a request of slot 2's shape at a later revision. All
  four caller-authority coordinates (34, 48, 62, 76) stay where they are.
- **Admission does not change.** The buyer still approves `buyer_debit` to the
  Custody authority, and `validate_collateral` still requires
  `delegated_amount == buyer_collateral_debit`.
- **The rate does not change and no rate is frozen.** ADR 0014's N-15
  precondition (*the composite fee base's characterization is formalized before
  any rate freezes*) is not tripped: this document moves where a fee settles,
  never what it is.
- **ADR 0014 D3 unblocks on this landing.** The demo's rate diversity is
  currently blocked behind exactly this, and market19's zero-fee founding —
  recorded at `227387da` as a judgement and upgraded by FEEWALL to a measurement
  — stands until it lands. The release const
  `DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1 = 50`
  (`crates/dclutch-direct-codec/src/token_setup_v1.rs`) can then be replaced by
  D2's band check.

**One finding for whoever takes ADR 0014 D2.** `CUSTODY_ROUTES_V3` slot 3,
`FeeSole`, is enabled by `seller_net == 0 && combined_fee != 0`
(`crates/dclutch-direct-aot-v3-contract/src/lib.rs:208`). `seller_net == 0`
means `floor(gross · bps / 10000) == gross`, which for `gross > 0` forces
`bps == 10000` exactly. **So `FeeSole` is reachable only at a rate of 100%** —
the take-everything market ADR 0014 D2 option B proposes to forbid at
`MAX_FEE_BPS = 500`. Ruling D2 for the band therefore makes slot 3 unreachable,
and it should be retired in the same lane rather than left as a declared route
no admissible config can enable. It is also, today, still unexecuted (FEEWALL
§7), so nothing is lost by removing it and something is gained: the Effect drops
from four declared Custody routes to three.

---

## 6. Sizes

Five lanes, three to four days of swarm time. FEEWALL priced (b)'s mechanism at
two to three lanes; the difference is entirely §2.4's gate, which FEEWALL's
estimate did not include because §2.3's refusal had not been found yet.

**Lane A — the transition and the candidate. Protocol tier.** One enable
register: the fee-continuation route must be disabled in tx1 while
`SellerIntermediate` stays enabled. Touches
`formal/dclutch-semantics/EmitDirectOrdinaryV3Rust.lean`,
`crates/dclutch-direct-codec/src/generated_ordinary_v3.rs`, the AoT mirror
`crates/dclutch-direct-aot-v3-contract/src/lib.rs`, and the two candidate
postconditions in `crates/dclutch-direct-codec/src/inline_candidate_v2.rs`
(`compile_custody`'s closing check becomes `delegated_after == delegated_amount
− seller_net_collateral_credit`, and `fee_destination_after ==
fee_destination_before` in tx1). Lean regeneration plus a semantics review.
**1 lane, 1 day.** This is the lane that touches proved statements; review it as
protocol tier, not as mechanical.

**Lane B — the maker replay widening.** `DirectMakerReplayLayoutV1` 152 → 160
with `fee_owed` at 152, the generated offsets, encode/decode, the poststate
projection in `direct_finalization_v3.rs`, and the rent principal. Read **both
widths** — 152 means `fee_owed = 0` — following the tree's own
`direct_with_witness` precedent (`FUNDED_CRANK_V1` §9: *prefer an optional
recipient to a mandatory one wherever an existing caller would otherwise have to
change*), so no live replay migrates and no existing zero-fee market is
disturbed. **1 lane, half a day.**

**Lane C — the tx2 route.** A new Trading top-level entry, its wire (§4.4), its
19-account frame built with `FrameRoleV3` (§3), the fee-request projection from
replay + maker replay + immutable config (§1.4), reuse of
`custody_composition_v3::execute_custody_route_v3` for the CPI and receipt
verification, and two new refusal discriminants in `TradingSbfError`'s
registered band (`0x4009` and `0x400A`; ADR 0007 and
`dclutch_refusal_registry::TRADING_REFUSAL_BASE + BAND_SPAN` bound them, and the
compile-time assertions at `programs/dclutch-trading-sbf/src/lib.rs` already
enforce the band). **1 lane, 1 to 1.5 days.**

**Lane D — the gate and the evidence.** The admission refusal on `fee_owed !=
0`; FEEWALL's fixture extended to a two-transaction fee-bearing scenario;
the margin gate re-measured on both arms; the zero-fee control asserted
byte-identical (§5); and hostiles that pin every row of §1.4's refusal table,
each shown to *reach* the check it names. **1 lane, 1 day.** The bar is
`FUNDED_CRANK_V1` §7's six conditions with (1) and (3) inapplicable — nothing is
paid and no floor is derived — and (5), the negative control, mandatory: prove
the new assertions red against the old code, or a change that changed nothing
observable still goes green.

**Lane E — the callers.** The `crates/dclutch-operator` builder for tx2, the
TypeScript twin, and the trade panel showing an unsettled fee where it already
shows the rate (`apps/dclutch-web/components/MarketTradePanel.tsx` already
narrates *"{feeBasisPoints} bps each side / immutable, founded with the
Market"*). **1 lane.**

Lanes A and B are independent of C; D depends on all three; E depends on C.

---

## 7. For ember

Two questions, and only two. Everything else above is recommended and closed.

### 7.1 Is a settled fill with an unpaid fee an acceptable interim state, and for how long?

**Recommended: yes, and forever — no deadline.**

The state is safe in the ways that usually matter: collateral is conserved
(§2.1, Identity 1), the seller is whole the instant tx1 lands (§2.2), the buyer
cannot trade in that market again until it clears (§2.4), and both interested
parties can end it unilaterally (§3). What is *not* true is the settlement
identity the makers signed, and that is stated as a named, temporarily-false
identity rather than hidden inside a conservation claim.

**Why a deadline would be worse than none.** The tree has a deadline idiom right
there — `RecordActionV1::Abort` with `expiry_slot` and a disclosed bounty
(`crates/dclutch-record-contract/src/lib.rs`, `prepare_abort_v1`), the only
route in the tree that pays a bounty to its own caller. But an expiry on a fee
obligation can only do one of two things at the deadline: **forgive** the fee,
or **strand** the maker permanently. Forgiveness makes non-payment a waiting
game. Stranding is what the undeadlined design already does, minus the timer. A
deadline adds a clock and a Clock sysvar to a decision that has no time in it.

The thing to weigh is the shape rather than the duration: **the venue is
accepting that a buyer can decline to pay one fill's fee, once, per market, at
the price of never trading there again.** At 50 bps that is 1% of one fill's
notional. If that is not acceptable, the only stronger designs move the fee to
*admission* — the buyer pre-pays into Custody before any fill — which is a
different and much larger change to how a Direct trade is signed and admitted,
and it is not this document's.

### 7.2 Is locking a maker out of a market until they settle an acceptable consequence?

**Recommended: yes.** But it is a real restriction on a real person, imposed by
the venue, and it is the load-bearing piece of the whole design — §2.3 shows
that without it the fee is silently forgiven by the buyer's own next trade, so
this is the difference between a fee and a suggestion.

Three properties that make it defensible, and one that does not:

- It is **scoped to one market**. The config is immutable and per-market (ADR
  0014 §2), so the lockout follows the fee policy the buyer signed and reaches
  no other market, no other family, and no other mint.
- It is **curable by the debtor alone**, at any time, with no counterparty and
  no permission: re-approve if needed, submit tx2, trade again.
- It is **visible before the fact**. The trade panel already narrates the rate
  and its immutability; it can narrate the obligation the same way.
- What it is *not* is symmetric. A buyer whose crank simply never got turned —
  because the venue never bothered and the buyer did not know to — is locked out
  by a mechanism they did not understand. The mitigation is product, not
  protocol: surface `fee_owed` and a one-click settle. Worth deciding whether
  that is a blocker for the demo or a follow-on.

---

## 8. What this document does NOT establish

- **That the fee request validates in a later transaction on chain.** §1 is a
  reading of the seed derivation, `authenticate_common_frame`,
  `CustodyReplayV1::advance` and `DelegatedCustodyRequestV2::validate` at HEAD.
  It is a source argument, not an executed one. The cheapest possible
  confirmation is two transactions against FEEWALL's existing fixture with the
  fee route's request submitted separately, and it is worth doing **first** in
  lane C, before any of lanes A, B or D is written.
- **tx2's CU cost.** §4.3 is an estimate assembled from three measured
  comparables in `tools/gauntlet/CU_BUDGETS.json`, labelled as such. Nothing
  here has been built or run.
- **tx1's cost after the split.** §4.3's claim that tx1 returns to the zero-fee
  profile is an inference from the two routes' request widths and identical
  frame, not a measurement. FEEWALL's gate is the instrument; run it.
- **The cost of `fee_owed` in tx1.** A `u64` written into an account the
  finalization already projects should be near-zero, but "near-zero" is a
  prediction and tx1's worst-seed headroom is 46,592 CU.
- **Any rate other than 50 bps.** As with FEEWALL: the shape is rate-independent
  by argument (`combined_fee != 0` is the only thing the routes read), which is
  an argument and not a measurement. §5's `FeeSole` finding is arithmetic on the
  enable registers and is exact.
- **Whether existing on-chain maker replays exist that would migrate.** Lane B's
  both-widths reader makes the question moot by design; nobody has counted.
- **Anything about the registered (resting-order) family.** Everything here is
  the inline ordinary route. `registered.rs` charges differences of floors of
  cumulative gross and telescopes across fragmentation
  (`cumulative_fee_telescopes`, `DirectProofs.lean:143-153`); whether its fee
  leg has the same shape is unexamined.
