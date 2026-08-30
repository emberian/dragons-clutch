# Reviewed patch: activate the capability-close crank (fenced — trading-sbf)

**For the post-cut trading window / cohort-8 owner.** Prepared by ESCROW.
The enabling half is already on `main` in `bc26e8cd` and is **inert**: nothing
passes a nonzero cap, so every close today behaves byte-for-byte as it did.
This document is the other half.

**Two files, both one-site changes.** Sites are named by *symbol*, not line —
line numbers in this tree drift within the hour.

---

## What already exists (landed, tested, dark)

`crates/dclutch-capability-contract/src/funding.rs`, commit `bc26e8cd`:

| added | meaning |
|---|---|
| `FundingLedgerCloseCustodyV2::native_with_crank(ledger_lamports, exact_rent, credit, cap)` | as `native_only`, plus a chain-derived reward ceiling. `cap == 0` is exactly `native_only` — proven by test, plans *and* ledger bytes identical |
| `FundingLedgerEntryClosePlanV2::crank_reward()` | lamports owed the crank. **Zero unless `ledger_can_close()`** |
| `FundingLedgerEntryClosePlanV2::native_refund_total()` | every native lamport the RentCredit is owed, **net of the crank** |
| `FundingLedgerEntryClosePlanV2::validate_native_conservation(observed)` | refuses unless every ledger lamport is leaving or staying, and the reward is carved *from* the refund rather than added *to* it |

**The reward is carved only from rent the close liberated** — the ledger's own
Rent reserve plus its surplus — and never from `remaining_native_lamports`,
which is a depositor's principal. So it is nonzero only on the final row close,
the one that actually frees the ledger. Nobody is worse off than if the crank
had never run: unturned, that rent stays locked and reaches no one.

---

## Patch 1 — `programs/dclutch-trading-sbf/src/outer.rs`, in `prepare_close`

### 1a. Offer the cap

The `FundingLedgerV2::close_slot_in_place(...)` call currently passes

```rust
FundingLedgerCloseCustodyV2::native_only(
    account.lamports(),
    exact_ledger_rent,
    credit.key,
)
```

Change to `native_with_crank(account.lamports(), exact_ledger_rent, credit.key, crank_cap)`
where `crank_cap` is `0` when no crank account is present and
`rent.minimum_balance(0)` when one is — **derived from the Rent sysvar already
in the frame, never a source literal** (`FUNDED_CRANK_V1.md` §3).

### 1b. Net the refund — **this is the load-bearing line**

`refund_total` currently sums `exact_root_rent + root_surplus + ledger_total`.
`ledger_total` is *gross* and must stay gross: it is compared against the
physical ledger account's own balance, and that comparison is correct.

Subtract the reward from `refund_total` only:

```rust
let refund_total = exact_root_rent
    .checked_add(root_surplus)
    .and_then(|value| value.checked_add(ledger_total))
    .and_then(|value| value.checked_sub(close.crank_reward()))   // <-- add
    .ok_or(TradingSbfError::Content)?;
```

`credit_post_lamports` then follows unchanged. **Do not touch `ledger_total`** —
netting it there would break the runtime/physical balance equality check
immediately above and refuse every close.

### 1c. Credit the crank, and check conservation

After the existing writes, credit the recipient by exactly `close.crank_reward()`
and call `close.validate_native_conservation(account.lamports())` with the
ledger's *observed pre-close* balance. Refuse on `Err`.

### 1d. The recipient's frame shape

Optional, keyed on frame length — the idiom `CloseAccountsV2` in rent-sbf and
now `ExpiryAccounts` in core-sbf both use, and the reason the two landed
conversions changed no existing caller. The recipient must be writable, not
executable, System-owned, data-empty, and must alias nothing.

**Say nothing about `is_signer`, in either direction** (`FUNDED_CRANK_V1.md` §6).
It is usually the fee payer and so usually signs; a signature here would
establish who is *owed*, never who is *permitted*. Requiring one gates a
permissionless verb; refusing one is the live defect in dealer checkpoint
cleanup, where the beneficiary may not pay its own fee and so nobody turns the
crank at all.

---

## Patch 2 — `programs/dclutch-core-sbf/src/capability.rs`, `Action::CloseCapability` arm

Core computes an *expected* post-state and CPIs to Trading, so **core and
trading must ship together or the close refuses**: core's expectation would not
match trading's writes.

Mirror 1a in the `FundingLedgerCloseCustodyV2::native_only(...,
state.rent_beneficiary.to_bytes())` call, and let `expected_post_lamports` keep
following `close.expected_post_ledger_lamports()` (unchanged — the reward comes
out of what leaves, not out of what stays).

The recipient rides the **child tail**, which Core forwards verbatim and does
not authenticate; Trading pins it. Note `require_authenticated_suffix_aliases`
runs an all-pairs distinctness census with exactly seven excused pairs, so a new
account must alias nothing.

---

## Tests that will go red, and which are *supposed* to

These are good negative controls, not obstacles. Each asserts the property being
deliberately changed; each should gain a funded twin rather than be weakened.

| test | file | asserts |
|---|---|---|
| `native_close_refunds_principal_rent_and_surplus_then_replay_refuses` | `programs/dclutch-trading-sbf/program-test/tests/activation.rs` | frame length `== 25`; the five-term refund sum; a beneficiary substitution refuses commit-last. **The strongest lamport-destination test in the tree** |
| `canonical_high_selector_closes_through_real_core_and_trading` | `programs/dclutch-core-sbf/tests/capability_close_alias_program_test.rs` | `credit.lamports == before + root_lamports + funding_lamports` — exactly the property a crank fee changes |
| `shifted_substituted_and_extra_aliases_refuse_with_rollback` | same | hard-codes `accounts[33]`/`accounts[36]` and `accounts.len() == 38` |
| `dependency_writable_reordered_substituted_and_mutated_refuse_with_rollback` | same | hard-codes `accounts[5]`/`accounts[6]` |
| `the_frame_this_route_requires_is_the_frame_a_blanket_census_refuses` | `programs/dclutch-core-sbf/src/capability.rs` | frame shape |
| `close_alias_policy_admits_only_the_seven_authenticated_suffix_pairs` | same | frame shape |
| `direct_v2_ledgers_activate_and_close_only_the_trading_selection` | `programs/dclutch-trading-sbf/src/outer.rs` | close buckets |

With a 25-account (unfunded) frame **all of these should still pass unchanged**.
If any fails without a crank account present, the optional-account wiring is
wrong — that is the first thing to check.

## Cost, measured

- **No wire format moves.** `FundingLedgerCloseCustodyV2` and
  `FundingLedgerEntryClosePlanV2` have no `encode`/`decode`/`to_bytes` — they are
  in-memory values. The manifest, ledger, and execution selection are untouched,
  so **nothing Lean-generated regenerates and no emission guard runs.** The
  census's "this needs a `CapabilityManifestV1` ABI change" (Q5/Y1) holds only
  if the fee is *authored* in the manifest, which §3 forbids.
- **No new refusal code**, so `docs/reference/refusals.md` stays put — reuse
  `TradingSbfError::Content` and `CoreSbfError::AccountFrame`. Worth protecting:
  regenerating that file rewrites all of `docs/reference/`, which in a
  multi-lane tree sweeps other lanes' changes into your commit.
- **CU:** no budget is pinned to the capability-close route (it is blocked in
  `tools/gauntlet/blocked.json`). Six *other* core-sbf rows recompile; the
  binding constraint in the file is `dcltgmf1-whole` at 1,348,747 against the
  1,400,000 ceiling.
- **`CapabilityFundingLedgerV2.lean`** — spec-only, no emitter, so **nothing
  goes red if you desynchronize it**, which is worse rather than better. Its
  `theorem final_native_close_classifies_every_observed_lamport` is the one a
  native split touches. Update it in the same change and add a named test
  mirroring it, exactly as `1cdddf27` did for series permit expiry.

## One end-to-end driver, and it is not the gauntlet

The route is blocked in `tools/gauntlet/blocked.json`, so **no
gauntlet campaign proves the crank is reachable.** The only driver is
`tools/local-validator/bootstrap/successor/src/terminal_sequence.rs`, stage
`TerminalStageV1::DirectCloseCapability`, whose `protocol_lamport_deltas` map
hard-codes who receives what and will need the crank leg added.
