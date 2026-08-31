# The fractional claim-check, the wall the sizing did not see, and what the half really costs — 2026-08-30

Written by FRACCHECK, from FRACR3's sizing
(`docs/evidence/FRACTIONAL_CLAIM_CHECK_SIZING_2026_08_30.md`) and
`docs/design/CLAIM_CHECK_COMPACTION_V1.md` §17.

## Result

**The sleeping shard holder gets paid, and the reason is that the record names
no payee — but not by the route the sizing costed, because that route cannot
exist.** A shard cannot be burned by its holder's signature. The design's
closing sentence — *"the holder redeems by burning, with their own signature,
forever"* — is false against the Token program this family actually deploys
against, and it is false structurally rather than by an oversight in the
implementation.

The claim survives the correction. What changes is that fractional compaction
has to do one more thing while the market is still alive, and that one more
thing is what makes the redemption route claims-only afterwards.

Four commits landed: the record type, the two conservation plans, the refusal
sub-bands with the routing gate that extends FRACR3's weld, and a program-test
campaign that executes the wall against the audited Token program rather than
reading it. Eight did not, and the re-size below says why and by how much.

## The finding: a shard holder cannot burn a shard

Every shard Mint in this family carries Token-2022's `PermissionedBurn`
extension, and `Token2022BehaviorProfileV2::read_mint` requires it — the Mint is
refused outright without it — pinned to the Mint's controller:

```rust
// crates/dclutch-token-svm/src/behavior_profile_v2.rs:232-240
PERMISSIONED_BURN_EXTENSION if !burn_seen => {
    require_extension(entry, PERMISSIONED_BURN_EXTENSION, AUTHORITY_EXTENSION_BYTES)?;
    require_key(entry.value, expected_controller)?;
    burn_seen = true;
}
// ...
if !close_seen || !burn_seen || pointer_seen != metadata_seen {
    return Err(Error::InvalidExtensionLayout);
}
```

In `fractional_atomic_v3.rs::process_terminal` that `expected_controller` is
`root_account.key` — the Fractional capability root, which
`Pubkey::find_program_address(&header.seeds().as_slices(), trading_program.key)`
derives **under the Trading program**.

The audited fixture is `spl-token-2022` v11.0.0
(`programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance`). Its
`process_burn` is unambiguous:

```rust
// spl-token-2022-11.0.0/src/processor.rs:1148-1177
BurnInstructionVariant::Standard => {
    // Standard burns cannot be used when the permissioned burn
    // extension is present.
    if maybe_permissioned_burn_authority.is_some() {
        return Err(TokenError::InvalidInstruction.into());
    }
}
BurnInstructionVariant::Permissioned => {
    // ...
    if !approver_ai.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if *approver_ai.key != expected_burn_authority {
        return Err(ProgramError::InvalidAccountData);
    }
}
```

### It was executed, not inferred

`programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/permissioned_burn_wall.rs`
runs this against the audited v11 ELF (sha256 `447ca3c6…`, 615,704 bytes — the
`macos_arm64_audit` row of the provenance, which the campaign checks for itself).
The holder owns the account, signs for it, and the chain says:

```text
Program log: Instruction: BurnChecked
Program log: Error: Invalid instruction
Program Tokenz…Pxu failed: custom program error: 0xc
```

`0xc` is `TokenError::InvalidInstruction`. Two further refusals pin *which*
signature is missing rather than merely that one is: the permissioned burn with
the authority present but not signing is `MissingRequiredSignature`, and with a
different authority signing is `InvalidAccountData`. The fourth transaction is
the control — both signatures, accepted, supply and holder balance each down by
exactly the burn, 2,090 CU — without which the three refusals would be equally
consistent with a Mint nobody can burn at all.

So a burn of a shard needs **two** signatures — the source account's owner *and*
the Mint's configured burn authority — and the standard single-signature burn is
not merely unused, it is refused. The tree already builds it that way:
`execute_terminal_burn` passes `root.key` as the permissioned authority and
`actor.key` as the owner, and gets the root's signature by propagation from the
Trading frame that CPI'd in.

Three consequences, and each of them contradicts something the sizing assumed:

1. **A claims-only, holder-signed redemption route is impossible as specified.**
   Claims cannot sign a Trading PDA, and no CPI reaches a route that survives the
   market.
2. **A redemption frame containing the root would not survive retirement**, so it
   could not answer `survives_retirement()` and would fail the frame's own test —
   the discipline catches this correctly, which is why the record type below
   declares the root as a refused role rather than omitting it.
3. **§17.3's "shards cannot be compacted, only their backing" is right for the
   wrong reason.** No crank *should* be able to burn a holder's shards; the
   reason it *cannot* is not that shards live in ordinary Token accounts, it is
   that the burn authority is somebody else's program's PDA.

### The resolution, and its price

`SetAuthority` with `AuthorityType::PermissionedBurn` can move the authority, and
`processor.rs:996-1007` shows the shape: the **current** authority must sign, and
the new one may be any address (or none). So:

> **Fractional compaction re-points the shard Mint's burn authority from the
> Fractional root to the Claims escrow PDA, while the root is still alive to
> authorize it.** After that instruction, a redemption is claims-only and
> holder-signed: the escrow PDA signs as the burn approver, which Claims *can*
> do, and the holder signs as their token account's owner, which only they can
> do.

The second test in the campaign executes exactly this. A stranger attempting the
hand-off is refused `OwnerMismatch` (`0x4`); the current authority's hand-off is
accepted; the old authority is then as powerless as the market that held it
(`InvalidAccountData`); and the holder-signed, escrow-approved burn goes through.

The price is real and is not in the sizing:

- **Fractional compaction becomes Trading-composed.** The root's signature comes
  only from the Trading program, exactly as `RetireCoordinate` already gets it
  (`protocol_position_v2.rs:844-846` requires both the Trading caller-authority
  PDA and the root to sign). This costs the "permissionless" property nothing —
  `fractional_retirement_v3.rs` is permissionless *and* Trading-composed today,
  and its module header argues at length for why it must be — but it does cost a
  Trading route, which is a second ELF and a second cohort.
- **A re-pointed Mint no longer satisfies `read_mint`.** That function requires
  one `expected_controller` to be the mint authority *and* the close authority
  *and* the burn authority. After the re-point the burn authority is the escrow
  and the other two are still the root, so a new profile —
  `read_compacted_shard_mint(program, mint, data, root, burn_authority)` — is
  needed in `dclutch-token-svm`, and `RetireCoordinate`'s compacted arm must use
  it rather than `check_mint`.

An alternative was considered and rejected. Shards could be *impounded* rather
than burned — transfer is not permissioned, only burning is — into a
claims-owned sink. It needs no Trading route and no new Mint profile. It was
rejected because it replaces a supply the whole family reads with a balance in a
second account: `RetireCoordinate`'s supply gate, the terminal evaluator's
`expected_base_supply`, and the record's own
`escrowed == floor(supply/denominator) * payout_per_claim` invariant would each
have to learn about the sink, and the sink itself becomes a perpetual account
holding instruments nobody can destroy. Burning keeps one number meaning one
thing.

## What landed

| commit | what |
|---|---|
| `9b65d15c` | `FractionalClaimCheckV1` — 320 B, magic `DCLTFCK1`, kind 3, seeds `[FRACTIONAL_CLAIM_CHECK_SEED, aggregate, shard_mint]`, the redemption frame spec |
| `592ad8d9` | `FractionalClaimCheckCompactionPlanV1` and `FractionalClaimCheckRedemptionPlanV1` |
| `49a83dc6` | refusal sub-bands `0x5640` / `0x5660`, and `claim_check_route_for` |
| `ee7e86ac` | the permissioned-burn campaign: the wall and the hand-off, on real ELFs |

### The record, and why the unsignable owner dissolves

The record's coordinates are the **instrument**, not a person. The native
record's address proves its holder; this one's proves which Mint it answers to.
Nobody is named as the payee, so nobody has to sign as the payee, so the fact
that the reserve Position's owner is a PDA stops mattering: the record was never
going to pay that owner. That is the whole of the fractional correction, and it
is why FRACR3's weld does not have to move — the two gates are complementary,
and `no_owner_kind_is_admitted_by_both_gates` says so over the whole enum.

### The lifetime, argued against the sizing's reasoning

FRACR3 specified "paid down across burns" and asked what replaces absence-based
anti-replay. The answer has two halves:

- **In the middle, the shards are the anti-replay.** A redemption burns
  `whole_claims * denominator` shards and burned shards cannot be burned twice.
  The record is a *budget*, not a ticket: it does not authorize a payment, it
  bounds the total of them. A replayed presentation fails at the Token program.
- **At the end, absence returns exactly.** `pay_down` reports `Settled` at zero
  and the route closes the record there, so a record promising nothing never
  exists — the same rule `ClaimCheckV1` enforces, for the same §15.1 reason.

### Where the remainder goes: nowhere, and that is the decision

Compaction escrows `floor(supply / denominator) * payout_per_claim` — every whole
claim the outstanding supply could form. Holders can only redeem
`sum_i floor(shards_i / denominator)`, which is smaller whenever the supply is
spread across accounts carrying sub-denominator dust. Worked example: denominator
1,000, supply 250 held as 90 + 90 + 70; the compaction escrows nothing at all,
and if the same 250 sat in one account it would escrow nothing either — but at
supply 12,345 spread as 12,000 + 345 the escrow holds twelve claims and the dust
holder can form none.

That gap is **not stranded**, and the difference matters. Shard *transfer* is
ordinary — only burning is permissioned — so any two dust holders can consolidate
and redeem what neither could alone. The remainder therefore stays escrowed
against a claim that can still be formed, and the record can stay open forever if
nobody forms it. This is the escrow's own §4.9 ruling one level down: an escrow
holding a live claim-check is holding somebody's collateral, and closing it would
be taking their money. A sweep-to-a-beneficiary rule was written, then deleted:
it pays a third party out of collateral whose claimants had merely not
coordinated yet.

### The one place fractional conservation is stricter than native

A native record promises a **total**, so §15.4's fee tolerance works: a vault
credited less than was sent can promise the smaller number and the one holder is
paid what is there. A fractional record promises a **rate** that every holder
applies independently, arriving one at a time and unknown at compaction.
Reducing the rate underpays the early holders; keeping it leaves the last holder
unpayable. So a short vault credit is `RateNotCovered` — a refusal, not a smaller
promise. The credit is still *observed* rather than assumed; only its tolerance
is zero.

### Conservation tests, by name

| test | what it pins |
|---|---|
| `the_only_division_floors_and_sub_denominator_dust_claims_nothing` | the quotient is the sole division, floors at eight inputs including `u64::MAX`, and never consumes more than the holder holds |
| `a_burn_is_paid_exactly_what_on_time_redemption_would_have_paid` | the payout is a multiplication by the persisted rate, and overflow refuses rather than wraps |
| `the_escrowed_balance_pays_down_to_the_atom_and_settles_exactly_at_zero` | four partial burns sum to the opening balance; everything but the balance is immutable across a pay-down |
| `a_pay_down_that_would_overdraw_another_holder_is_refused` | overdraw is `Arithmetic`, not saturation; exactly the balance settles, one atom less does not |
| `one_burn_pays_exactly_what_on_time_redemption_would_have_paid` | 3,400 shards → 3 claims, 3,000 burned, 400 returned, `3 × payout_per_claim` paid |
| `conservation_holds_to_the_atom_across_the_whole_pay_down` | four arrivals in the wrong order, one sub-denominator; per burn `collateral × denominator == consumed × payout_per_claim` and `presented == consumed + change`; across the pay-down the payouts sum to the opening escrow, the vault reaches zero, and the shards left outstanding are exactly the 345 atoms of dust the compaction never escrowed |
| `a_vault_credited_less_than_the_rate_requires_is_refused_not_reduced` | `RateNotCovered` at one atom short; the shared plan still refuses one atom over |
| `the_settling_burn_closes_the_record_and_no_other_burn_moves_a_lamport` | closing a live record and leaving a settled one funded are both `PostconditionMismatch` |
| `a_post_state_that_burned_the_wrong_number_of_shards_is_refused` | six single-field mutations of the post-state, each refused |
| `a_compaction_escrows_every_whole_claim_the_supply_can_form_and_no_dust` | the 345-atom remainder is escrowed for nobody |
| `the_lamport_split_is_the_native_plans_and_not_a_second_copy_of_it` | the rent ordering has one author |

### The weld, extended

FRACR3's `owner_kind_can_open_a_claim_check` is untouched and is now read by a
test in the fractional module rather than restated:

| test | what it pins |
|---|---|
| `the_native_gate_is_not_relaxed_by_this_lane` | all three arms of the shipped weld, asserted against the shipped function |
| `no_owner_kind_is_admitted_by_both_gates` | over the whole enum: a Position compacted twice would resolve one pot of collateral into two records |
| `the_pda_claimant_design_is_what_admits_the_kind_the_weld_refuses` | `TradingRecord` is refused natively and admitted fractionally, and neither gate had to move |
| `every_owner_kind_is_routed_and_exactly_one_is_still_stranded` | the routing is total, and `ClaimsCapability` is still stranded — counted, so the debt cannot quietly grow or vanish |

## The re-size

FRACR3 estimated **one lane, eight commits**. That was wrong by roughly
**+5 commits and one extra program**, and the correction is upstream of the
estimate rather than a scaling error inside it.

| # | commit | status | note |
|---|---|---|---|
| 1 | record type and seeds | **landed** | `9b65d15c` |
| 2 | two conservation plans | **landed** | `592ad8d9` |
| 3 | refusal sub-band | **landed** | `49a83dc6`; grew from one sub-band to two (`0x5640`, `0x5660`) plus the routing gate |
| 3b | **new** — the permissioned-burn campaign | **landed** | `ee7e86ac`; not in the sizing because the sizing did not know the wall was there |
| 4 | **new** — `read_compacted_shard_mint` in `dclutch-token-svm` | not written | a re-pointed Mint fails `read_mint`; the profile has to admit a split controller |
| 5 | **new** — a Trading route that composes fractional compaction | not written | a second ELF and a second cohort; the root's signature exists nowhere else |
| 6 | the compaction route | not written | the sized one, plus the `SetAuthority` leg and its own postcondition |
| 7 | the burn-and-pay redemption route | not written | the sized one, with the escrow signing as burn approver |
| 8 | `RetireCoordinate`'s compacted arm | not written | must skip `execute_mint_close`, admit nonzero supply, and use the profile from #4 |
| 9 | **new** — the fractional escrow close | not written | the native `CloseClaimCheckEscrow` reads `ClaimCheckEscrowV1`'s counter; fractional records admit and retire against the same counter, so the close needs to be shown correct for a mixed escrow rather than assumed |
| 10 | the campaign | not written | larger than sized: it must drive a re-point, at least two holders, a partial burn and a settling burn |
| 11 | the operator surface | not written | as sized |
| 12 | **new** — the design amendment | not written | §17.3's closing paragraph is wrong on the record and should say so where it is read |

**Fourteen commits, two programs, two cohorts.** The expensive half FRACR3
identified as already paid — the terminal fixture — is genuinely already paid.
What is not paid is the Trading half, which the sizing did not model at all
because it assumed the burn was the holder's to make.

## Not verified

- **No dClutch route was built, so no route CU was measured.** §17.3's ~928k
  projection is an addition of two measured numbers; it is now also missing the
  `SetAuthority` CPI and the re-point postcondition, so it is a lower bound on a
  route that does not exist. The only measurements here are Token-2022's own:
  a permissioned `BurnChecked` at **2,090 CU**, `SetAuthority` at **1,165 CU**,
  and the refused standard burn at **1,353 CU** before it failed. Those are the
  two CPIs a fractional redemption and a fractional compaction each add; nothing
  else on this page is a measurement.
- **The burn wall is measured on a Mint this campaign built, not on a Mint the
  Fractional route produced.** The Mint carries the base state plus one
  `PermissionedBurn` entry, which is the half of the shard profile the wall turns
  on, and `read_mint` requires that entry — but the campaign does not drive
  `fractional_atomic_v3` to mint a real shard Mint first. What is proved is
  Token-2022's rule; that the Fractional family's Mints carry the extension is
  read from `behavior_profile_v2.rs`, not executed here.
- **The escrow in the hand-off test is an ordinary keypair, not a PDA.** What is
  under test is who Token-2022 will accept as an approver, not the derivation of
  the approver. An `invoke_signed` PDA signature is the same signature to the
  Token program, but no Claims route was built to produce one.
- **No devnet write, no gauntlet witness set, no claims-sbf ELF rebuild.** The
  three code commits are contract-crate and program-crate code with host tests,
  and the campaign loads only Token-2022. No dClutch SBF ELF was rebuilt, so
  there are no frame diagnostics to report either way — zero SBF frame
  diagnostics is not a claim this lane earned.
- **Nothing was proved about a mixed escrow.** Native and fractional records
  would share one `ClaimCheckEscrowV1` and its outstanding count. The types admit
  it and nothing forbids it; no test exercises it, which is why the re-size gives
  it its own commit.
- **`ClaimsCapability` is still stranded**, exactly as before this lane. A market
  carrying one still cannot retire past a sleeping holder, and
  `every_owner_kind_is_routed_and_exactly_one_is_still_stranded` is the assertion
  that keeps that from being forgotten rather than a fix for it.
