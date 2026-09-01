# Admitting the registered Direct family through generic Hot

**Status:** specification. Nothing here is authorised; two rulings gate all of
it. Written by lane DIRECT-SELLBUY, 2026-09-01, from measurements taken on an
exact-HEAD ELF pack and on two deliberately labelled probe builds that were
never committed.

**Companion evidence:** commit `8f1a5473`, and
`programs/dclutch-trading-sbf/program-test/tests/direct_registered_creation_hot.rs`,
whose header carries the measurements and whose `#[ignore]`d
`registered_sell_then_buy_execute_on_current_elves` is the acceptance gate this
document plans the unblocking of.

---

## 1. What this specifies, and what it deliberately does not

`hot_v3.rs:5372` admits exactly one Direct action through generic Hot. This
document says, per action, what admitting the rest would actually require:
which routes exist, which artifacts exist, what is merely unrouted versus
structurally blocked, and in what order the work is buildable.

It does **not** authorise relaxing that gate, and it does not contain a patch.
The relaxation is a refusal relaxation; §4 explains why one particular action
makes it dangerous to perform bluntly.

## 2. The gate

`programs/dclutch-trading-sbf/src/hot_v3.rs`, opening
`prepare_direct_inline_hot_crosscheck_v3`:

```rust
if selected_kind != DIRECT_SUCCESSOR_KIND_ID_V3 {
    return Ok(None);
}
if selected_action != DirectExecutionActionV3::InlineOrdinary as u32 {
    return Err(TradingSbfError::UnsupportedContent.into());
}
```

A foreign kind passes through with no crosscheck at all; a Direct kind carrying
any action but `InlineOrdinary` is refused outright. Thirteen of the fourteen
`DirectExecutionActionV3` variants are on the wrong side of that comparison.

## 3. Proved, inferred, and never attempted

Kept separate on purpose, because the strength of the plan below depends on
which is which.

**Proved by measurement, on the exact-HEAD six-ELF pack** (digests in the
commit message of `8f1a5473`):

- The gate is reached, and reached late. A registered Sell refuses
  `UnsupportedContent` (0x4000) at **323,523 CU** — 7.8x the ~40,000 CU three
  prologue refusals on the same route cost — and a `hot-cu-profile` build puts
  it **451 CU past the `preflight-children` checkpoint**. The Sell has by then
  passed the manifest, the program set, the validated-artifact seal, the
  descriptor, the config, the lifecycle policy, the account profile, the
  request profile, the transition, the effect projection, the lifecycle
  preplan, the candidate, the replan and child preflight.
- Behind a probe build with that gate returning `Ok(None)`, a registered Sell
  **executes**, at **374,455 CU**, writing the fixture's exact root, maker-replay
  and record poststates and leaving the Claims aggregate and both Positions
  byte-identical.
- The manifest-entry gate (§5) is real and side-symmetric: minting the entry
  from the Sell descriptor makes the Buy refuse; minting it identically from
  the Buy descriptor moves the refusal to the Sell, in the same band, with
  nothing else changed.

**Inferred from source, not executed:**

- That the same gate blocks the other eleven unrouted actions. It is a single
  `!=` against one constant, so this is a strong inference, but only
  `RegisterSell` and `RegisterBuy` have actually been submitted.
- Per-action artifact coverage in §6, read out of `crates/dclutch-direct-codec`
  rather than exercised.
- That `CloseDirectRoot`'s dedicated non-Hot route is complete rather than only
  its `BeginRetiring` half. `direct_begin_retiring_on_chain.rs` is green; the
  terminal close was not read end to end.

**Never attempted:** every action except `InlineOrdinary`, `RegisterSell` and
`RegisterBuy` has never been submitted to a program by anything, in any harness.

## 4. Why the gate cannot simply be deleted

Returning `Ok(None)` for every non-ordinary action is what the probe did, and
it is the right shape for *most* of them — a foreign kind already takes that
path, and for an action with no crosscheck the Transition and the Effect are
the sole authority, which is how every other family works.

It is the wrong shape for exactly one action. `FillRegisteredOrdinary` is the
registered form of the settlement the ordinary crosscheck was written for: two
signed intents matched, collateral moved, Claims moved, a fee split. The
crosscheck exists because that settlement is worth checking twice, from the
typed candidate rather than from the Effect's own projection. Waving the fill
through with `Ok(None)` would silently give the most consequential registered
action *less* checking than its inline twin has today.

So the gate should become a **dispatch**, not a deletion:

```text
InlineOrdinary            -> the existing inline crosscheck
FillRegisteredOrdinary    -> a registered crosscheck (does not exist; §6)
every other Direct action -> Ok(None), the Effect and Transition are authority
```

The middle line is the only genuinely new semantic work the gate itself
requires. The last line is the change that unblocks creation and the terminals.

## 5. The second gate, which is not in `hot_v3` at all

Admitting an action at `5372` is necessary and not sufficient. A capability
root persists **one** manifest-entry index
(`CapabilityExecutionSelectionV1`), and
`CapabilityProgramV4::validate_selection` requires

```rust
self.derivation_policy == entry.child_derivation_id()
```

where `derivation_policy` is the digest of the descriptor's own
`LifecyclePolicyV5` — `hot_v3` separately requires
`descriptor.derivation_policy() == descriptor.lifecycle().program()` before it
reads that record. **Two actions can therefore share a root only if their
lifecycle policies are byte-identical.**

Today they are not. `DIRECT_REGISTER_BUY_LIFECYCLE_BYTES_V5` exceeds
`DIRECT_REGISTER_SELL_LIFECYCLE_BYTES_V5` by two
`LifecycleCurrentRentQuoteInputV5` rows — the Custody replay and vault a Buy
opens and a Sell has no business quoting. Different width, different digest,
different identity.

The scope is wider than Sell-versus-Buy, and this is the part most likely to be
underestimated: **the ordinary action's lifecycle policy is a third distinct
record.** A Direct market founded to trade inline pins the ordinary lifecycle
in its root header, so on today's contract that market cannot admit *any*
registered action even after the `5372` gate opens.

### 5.1 The destination this document recommends

Not a contract change. `StateLifecyclePolicyV5` **already selects plans by
action** — `action_plan_count(action: u32)` and its action-selected plan
accessor are on the public V5 surface, and `hot_v3` already carries
`selected_action` into lifecycle selection (the host engine's
`EngineInputV1::action` is documented as "for lifecycle plan selection").

So the buildable destination is **one Direct `LifecyclePolicyV5` per market,
carrying an action-keyed plan for every Direct action that market admits**,
with the manifest entry pinning that single shared record. Nothing in
`validate_selection` has to move; the descriptors stop disagreeing.

What must be checked before committing to it, and was not checked here:

1. **The rent-quote bank is not obviously action-scoped.**
   `LifecycleCurrentRentQuoteV5` carries `exact_data_len` and a
   `scalar_destination`, with no action field, and quotes are addressed by
   ordinal. A merged policy therefore holds the *union* of quotes. Whether an
   action may safely be projected against a bank containing quotes it does not
   use — or whether the quote bank needs action scoping the way plans have it —
   is the one open question that decides whether this destination works.
   Answering it is the first task in §7.
2. Whether a merged policy stays inside the packet and CU budget. The Sell's
   374,455 CU is a floor, not the merged figure.
3. Whether `RECIPE_COUNT`, `SEED_COUNT` and `BINDING_COUNT` are likewise
   unioned, and whether any two actions' seeds collide.

If (1) answers badly, the fallback is a contract ruling instead: either the
manifest entry stops pinning `child_derivation_id` for multi-action program
sets — the per-action descriptor in the program set already pins its own
lifecycle, and the validated-artifact seal authenticates the bytes
independently, so the conjunct may be redundant rather than load-bearing — or a
root gains the ability to select an entry per action. Both are
capability-contract rulings and neither is made here.

### 5.2 Do not solve it by padding

Making a Sell's lifecycle match a Buy's by giving it the Buy's two rent quotes
would have a Sell quoting a Custody vault it never opens. That is the same
defect class as a profile demanding unauthenticated dummy accounts for a
disabled side, which the creation artifacts were deliberately built to avoid
(Sell 13 coordinates, Buy 56). An action-keyed plan bank is the honest version
of the same idea; a padded quote bank is not.

## 6. Per action

`R` = routed and green on real ELFs. `A` = artifacts exist. `B` = a bundle
builder producing a `CapabilityProgramV4` descriptor exists. Gate column: `#2`
= the `5372` action gate, `#3` = the shared-lifecycle gate of §5, `—` = neither.

| # | Action | sigs | Codec state | Gates | Real work remaining |
|---|---|---|---|---|---|
| 1 | `InlineOrdinary` | 2 | R, A, B | — | none; 1,310,719 CU measured today |
| 2 | `RegisterSell` | 1 | A, B | #2, #3 | none in the codec — it executes behind the probe |
| 3 | `RegisterBuy` | 1 | A, B | #2, #3 | wall #4 (§6.1) |
| 4 | `FillRegisteredOrdinary` | 0 | A | #2, #3 | a bundle **and** a registered crosscheck (§4) |
| 5 | `SplitRegistered` | 0 | request only | #2, #3 | whole artifact family |
| 6 | `MergeRegistered` | 0 | request only | #2, #3 | whole artifact family |
| 7 | `CancelRegistered` | 1 | draft (§6.2) | #2, #3 | resolve the draft, then a bundle |
| 8 | `ExpireRegistered` | 0 | draft (§6.2) | #2, #3 | resolve the draft, then a bundle |
| 9 | `CloseInvalidated` | 0 | request only | #2, #3 | whole artifact family |
| 10 | `CancelThrough` | 1 | request only | #2, #3 | whole artifact family |
| 11 | `CloseMakerReplay` | 0 | request only, **but see §6.3** | — today | nothing urgent |
| 12 | `CloseDirectRoot` | 0 | request only, **but see §6.3** | — today | nothing urgent |
| 13 | `SplitInline` | tail | request only | #2 | whole artifact family |
| 14 | `MergeInline` | tail | request only | #2 | whole artifact family |

Signature counts are `native_signature_count_v3`, which is authoritative.
"Request only" means `registered_requests_v4.rs` encodes and decodes the
action and nothing produces its profile, transition, effect or lifecycle.

### 6.1 `RegisterBuy` carries a fourth wall of its own

Behind probes for both gates the Buy reaches the `p5r-account-projection`
checkpoint and refuses `Content` at 311,068 CU on a diagnostic build: its own
`AccountProfileV2` projection rejects its own fixture frame. The projection's
typed error is discarded by `.map_err(|_| TradingSbfError::Content)`, so the
chain cannot name it.

**This is not gated on any ruling and is the one piece of unblocked work in
this document.** The tool for it is
`programs/dclutch-trading-sbf/program-test/bundle-builder/src/registers.rs::run_engine`,
which reproduces the same projection chain on the host and fails with
`BuilderError::Projection(&'static str)` naming the stage. Wiring an
`EngineInputV1` for the registered Buy is the missing step; the ordinary
equivalent already exists in `direct-hot/src/fixture.rs::via_builder`.

It is unknown whether this is a fixture defect or a profile defect. Both are
plausible: 56 coordinates across three Custody frames have never been checked
against a live account set.

#### 6.1.1 ANSWERED, 2026-09-01 (lane BUY-PROJECTION, commit `f1da7cbe`)

**It is a profile defect. The fixture is right, and it is exactly one defect.**

`run_engine` names the stage `Projection("account-projection")`, and the kernel
under it names `Error::IdentityMismatch`. Not a width: every one of the Buy's
56 declared widths is satisfied by the frame, and that is now asserted.

The two convicted coordinates are the Buy's only two identity conjuncts:

```text
require_key(MINT_ACCOUNT = 34,          REGISTERED_IDENTITY_MINT_V4 = 24)
require_key(TOKEN_PROGRAM_ACCOUNT = 37, REGISTERED_IDENTITY_TOKEN_PROGRAM_V4 = 25)
```

Both registers are written by `project_identity(REALM_ACCOUNT = 18, ...)` **in
the same operation table**, and `v2.rs::project_atomic` hands `apply_operations`
`registers.input_identities` — the bank as it stood *before* this pass — while
every `project_*` writes into `scratch_identities`. So each conjunct compares a
real 32-byte key against 32 zero bytes. **Unsatisfiable by any transaction.**

Measured, not inferred. Seeding register 24 alone still refuses; seeding 25
alone still refuses; seeding both returns `Ok(())`; nothing else in the 56
coordinates refuses. `both_registered_creation_profiles_project_a_real_frame`
in `direct-hot/src/fixture.rs` pins all four.

**The Sell is the control that names the axis.** Its three `require_*`
operations — `require_owner(0, TRADING_PROGRAM)`, `require_key(11,
SYSTEM_PROGRAM)`, `require_owner(6, SYSTEM_PROGRAM)` — all target
trusted-environment registers, which *are* seeded before the pass. The Sell
projects its own frame green. That is why a registered Sell executes behind the
§4 probe at 374,455 CU and a Buy dies at 311,068.

This is `WAVE.md`'s THE CLASS in its identity form rather than its width form:
a declaration authored in one place, never executed against a real account,
therefore unsatisfiable by anything. The remedy is the same — execute the
declaration — and that is what landed.

**Nothing is behind this wall.** Measured on an *uncommitted* probe that
neutralizes only those two operations in a local copy of the profile bytes (a
refusal relaxation; §9 keeps it out of the tree): the whole Buy pipeline
completes host-side — account projection, rent quotes, native signatures,
request projection, the lifecycle preplan (2 states: maker replay and record),
the transition fold, and the effect projection yielding **3 child invocations**,
the three Custody routes. Routes 1 (`OpenVault`) and 2 (`Transfer`) carry the
Realm's collateral mint and token program in their projected child requests.

**Proposed repair (not landed — it is a release event and ember's): delete the
two operations, `FIXED_OPERATIONS` 34 → 32.** They are a redundant outer
restatement of the child's own check, which is precisely the reason this same
file already deleted the outer *width* restatements on these very coordinates
("Custody — the semantic owner — already authenticates all three against the
authenticated Realm, so the outer restatement was strictly weaker than the
child's own check"). The binding survives deletion by a stronger route:
`custody-sbf/src/lib.rs:1002-1010` refuses `Realm` for `OpenVault | Transfer |
CloseVault` unless `request.mint == *realm.collateral_mint()` and
`request.token_program == *realm.token_program()`, and
`validate_token_program_and_mint` (`:1757-1768`) then requires the frame's mint
and token-program *accounts* to equal the request's. The child request's fields
are projected from registers 24 and 25, so the Realm remains the sole authority
and the loop closes inside the semantic owner.

The alternative — keep the checks and move them into the shared Transition,
where the projected registers *are* visible — costs two more identity registers
and a fork of the creation transition, and buys a second copy of a check Custody
already makes. It is named here so the ruling is a choice rather than a default.

**Separable / not separable.** Landed: the executable localizer, which relaxes
nothing and asserts the wall by name (the assertion is written to be deleted by
whoever lands the repair). Not separable: the repair itself. Removing two
operations changes the emitted profile bytes, hence the account-profile record
digest, hence the descriptor, hence the descriptor digest, hence the capability
seal, the program set and the manifest entry — release identity. That is
ember's, and it should land together with §7.5 rather than on its own.

### 6.2 The terminal draft, and why it stays unregistered

`crates/dclutch-direct-codec/src/registered_terminal_artifacts_v4.rs` is 2,529
untracked lines covering actions 7 and 8. It is correctly absent from `lib.rs`
and must stay absent until three decisions land. Two are answered here; one is
not.

- **Does registered Sell creation provision the record-owned Claims Position
  the draft's sparse transfer assumes? No.** The draft's own header says it
  "sends a Sell's residual claims from the record Position to its maker". The
  Sell `EffectProgramV4` has **zero routes** and the Sell profile's 13
  coordinates include no Claims Position and no aggregate. Creation and the
  terminal draft disagree; one must move. This is the same finding as the
  `reserved_claims` seam in §8, seen from the other end.
- **Does the draft avoid demanding disabled-side accounts? No.**
  `DIRECT_REGISTERED_TERMINAL_FIXED_ACCOUNTS_V4 = 71` is a single profile
  carrying both the Claims refund frame (10..32) and all three Custody frames
  (32..70). A Sell terminal has no vault; a Buy terminal has no Claims
  transfer. One profile at 71 coordinates necessarily demands dummies for
  whichever side is disabled. The creation artifacts already show the right
  shape: two profiles, two counts, with the one coordinate a Sell may **not**
  drop documented by name
  (`DIRECT_REGISTER_SELL_COLLATERAL_ACCOUNT_V4 = 12`, because the shared
  Transition compares the maker's signed collateral account unconditionally).
- **Coordinate 9 has no semantic owner.** Named constants cover 0..8
  (root, config, product, portfolio, basis, maker, record, rent credit, rent
  program) and resume at 10 (`DIRECT_REGISTERED_TERMINAL_CLAIMS_ACCOUNT_START_V4`).
  No `rule_mut` call touches 9, so it keeps a default-initialised rule. In the
  creation profile the analogous slot is
  `DIRECT_REGISTERED_RECORD_PAYER_ACCOUNT_V4` — a payer alias "named by no
  plan". Either 9 is the terminal's refund-beneficiary alias, or the Claims
  frame should start at 9 and the whole map shifts down one. **Unresolved.**

The terminal semantics themselves are settled and should be preserved
verbatim: maker-signed `Cancel`, unsigned strict-deadline `Expire` with
`valid_through < trusted_slot` where **equality refuses**. That boundary is
load-bearing — a prior mutation campaign found a real hole in exactly this
strictly-later-versus-not-earlier relaxation — and it must not be softened
when the draft is finished.

### 6.3 Two actions already have a working home outside Hot

`CloseMakerReplay` and the Direct root's retirement are served today by
**dedicated top-level Trading instructions**, not by generic Hot:
`DCLTDMC1` (`close_maker_v1.rs`, `close_maker_bundle_v1.rs`) and `DCLTDBR1`
(`retirement_v1.rs`, `begin_retiring_bundle_v1.rs`). Those bundles produce
`CapabilityProgramV1` descriptors with AccountProfileV1 and EffectProgramV2 —
an older artifact generation than the Hot V4 family — and both routes are green
on real ELFs in `direct_close_maker_on_chain.rs` and
`direct_begin_retiring_on_chain.rs`.

So actions 11 and 12 are **action codes with a parallel implementation**, not
capabilities that are missing. They are behind gate #2 only for anyone who
chooses to move them onto Hot, and there is no reason to do that first.
Consolidating the two generations is a separate, later question, and this
document recommends explicitly **not** bundling it into the admission work.

## 7. Buildable order

Each step is independently landable and independently falsifiable.

1. **Answer §5.1(1): is the rent-quote bank safely unionable?** Pure reading
   plus host tests in `dclutch-account-profile-contract`. It decides whether
   the rest of this plan is a codec change or a contract ruling, so nothing
   else should start first.
2. ~~**Localize wall #4** (§6.1) via `run_engine`.~~ **DONE, `f1da7cbe`** —
   §6.1.1. The Buy fixture is right; two profile operations are unsatisfiable,
   and nothing is behind them. What remains is the repair, which is a release
   event and belongs with step 5, not on its own.
3. **Ruling on gate #2**, as the dispatch of §4 rather than a deletion.
4. **Ruling on gate #3**, taking §5.1's shared action-keyed policy if step 1
   permits it, and the contract fallback otherwise.
5. **Land creation.** Delete the two unsatisfiable Buy conjuncts (§6.1.1),
   merge the Sell and Buy lifecycle policies, rebuild both
   bundles, remove `#[ignore]` from
   `registered_sell_then_buy_execute_on_current_elves`. Acceptance is already
   written: exact root, maker-replay and record poststates for both sides,
   Custody replay at revision 3, vault and source balances, the lifecycle
   RentCredit unmoved, and Claims aggregate and both Positions byte-identical
   across both creations.
6. **The registered crosscheck, then the fill.** In that order — §4's middle
   line is the reason. The fill's artifact family already exists
   (`registered_fill_artifacts_v4.rs` and the state/account/effect modules that
   name the action); it needs a bundle builder and the crosscheck.
7. **Finish the terminal draft** against §6.2's three answers — two profiles,
   coordinate 9 given an owner or removed, and the Claims Position question
   settled in whichever direction §8 is ruled — then register the module and
   build Cancel and Expire.
8. **Everything else** (5, 6, 9, 10, 13, 14) is a whole artifact family each
   and should not be scheduled until the fill and the terminals are physical.

## 8. The ruling this plan is waiting on that is not about routing

A registered Sell escrows **nothing**. `register_intent_v2` writes
`reserved_claims = maximum_fill` into the record; the Sell Effect invokes no
child and the Sell profile carries no Claims account. A registered Buy really
does escrow — three Custody routes move collateral into a record-keyed vault
and drain the delegate allowance to zero.

So `reserved_claims` is a **cap, not a reservation**: it bounds what a record
may ever fill and does not make that fill possible. One maker may register any
number of Sell records each reserving full supply, because the Position is not
in the frame to check. `sum(reserved_claims)` over live records is therefore
not bounded by supply, is not a conservation quantity, and no surface may
render it as solvency. Conservation is enforced at the fill, where
`claim_custody_debit: fill` moves real claims and the whole transaction rolls
back if they are gone.

Whether that asymmetry is the intended design decides §6.2's first bullet, and
therefore decides whether the terminal artifacts or the creation artifacts move.
Both are consistent with themselves; they are not consistent with each other.

## 9. What must not be done

- Do not commit the wall-#2 probe. It is a refusal relaxation and it stays out
  of the tree until §7.3 is ruled.
- Do not commit the wall-#4 probe either. Neutralizing the two Buy conjuncts to
  see past them (§6.1.1) is a refusal relaxation like any other; the measurement
  it produced is recorded there and the probe is not in the tree. The repair
  when it comes is a deletion of the two operations, not a rewrite of them into
  something that passes.
- Do not "fix" §6.1.1 by seeding registers 24 and 25 from anywhere but the
  Realm. The seeding in the landed test is a *localizer*, supplying the value
  the pass is about to project so the refusal can be attributed; a profile or
  executor that actually pre-seeded them would move the Realm's authority
  outside the record that carries it.
- Do not pad a Sell's lifecycle to match a Buy's (§5.2).
- Do not relax `ExpireRegistered`'s strict `<` to `<=` (§6.2).
- Do not move `CloseMakerReplay` or the retirement route onto Hot as part of
  this work (§6.3).
- Do not treat an action that refuses at gate #2 as evidence of anything else.
  Record substitution and authority substitution were deliberately **not**
  shipped as hostiles in `direct_registered_creation_hot.rs` for exactly this
  reason: on the current release they refuse at the wall regardless of the
  mutation, which is a test of nothing. They become meaningful the moment
  §7.3 lands, and they are the first hostiles to add then.
