# A fractional market can retire — Begin, the walk, Finish — 2026-08-30

## Result

**Yes, end to end, on chain, against real ELFs.** `a_fractional_market_retires_end_to_end_from_begin_through_finish`
drives four real transactions with no planted cursor and no planted
intermediate state: admit the zero reserve Position while the Market is Open,
resolve it, `Begin`, retire the sole coordinate against the audited Token-2022
v11, `Finish`. Six of six tests in the campaign pass; zero SBF frame
diagnostics.

Measured at width 1:

| act | CU |
|---|---|
| `Begin` | 38,123 |
| `RetireCoordinate` | 197,489 |
| `Finish` | 28,190 |

Measured after the seam fixes at `6e689907`. The same campaign read
41,678 / 207,931 / 31,558 two bases earlier and 44,678 / 203,431 / 33,058 one
base earlier; the named frame role is cheaper than the three-way tuple
comparison it replaced. Treat these as a few thousand units wide, not as pins.

## Three things were wrong, and only the first was known

### 1. Begin and Finish did not exist

FRACR3's finding, sized in `FRACTIONAL_CLAIM_CHECK_SIZING_2026_08_30.md`:
`fractional_retirement_v3.rs` dispatched only `RetireCoordinate` and refused
the other two actions outright, so nothing on chain could create or close the
cursor the coordinate walk advances. The contract's
`FractionalRetirementCursorV3::{begin, finish}` existed with tests; no route
called them.

### 2. The walk could not have taken a second step

Strictly downstream of the first, and invisible while the first was true.

`authenticate_root` required `root.revision == request.expected_revision`.
`cursor.advance` simultaneously requires
`request.expected_revision == cursor.revision`. The Trading-owned root is
written once by the activation that creates it and mutated by **no program in
the tree** — its revision is a constant. The cursor consumes one revision per
act. So the two comparisons are satisfiable together for exactly one
coordinate, and refuse every step after it.

`protocol-position/tests/lifecycle.rs` had papered over this by planting
`root.revision = 5` while calling `begin` with `pre_revision = 4` — a root one
revision ahead of the one a real `Begin` would have left. That works for
coordinate 0 and nothing else.

The relation that actually holds needs no new persisted byte, because the
296-byte cursor is fully packed and already carries both halves of it:

```text
cursor.revision == root.revision + 1 + cursor.next_coordinate
```

`FractionalRetirementCursorV3::root_revision_anchor` states it;
`RootRevisionBindingV3` names the two cases (`Request` at begin, `CursorAnchor`
after) so a later route cannot pick whichever comparison was written beside it.

### 3. The only on-chain test of the route had stopped walking it

The selection-config weld (`4630ad77`) taught the **fractional-atomic** fixture
to plant the identity a real activation writes — `digest(selection_config)` —
and left **protocol-position** planting the terms record digest. Its retirement
test had been refusing at `0x500B` in `authenticate_root` ever since, satisfying
its own `!accepted` assertion for the wrong reason and never reaching the
Token CPI it exists to observe. That is why (2) went unnoticed.

## What landed

### `Begin`

16 accounts. Permissionless, like everything else in this family, because the
cursor's content is fully determined by state authenticated before it runs: the
terms and the root supply every field, and the rent beneficiary comes from the
root. A route that demanded a signature would hand whoever held it the power to
strand every shard holder's collateral behind a walk nobody else may crank.

| gate | refusal |
|---|---|
| request width, magic, version, reserved bytes, action tag | `0x5000` `Instruction` |
| frame count, exact signer/writable/executable per named index, address distinctness | `0x5001` `Accounts` |
| terms record finalized under the Registry; request bound to terms | `0x5008` `Representation` |
| aggregate re-derived at `[LIABILITY_BASIS_MARKET_SEED_V2, market]` and Claims-owned | `0x5002` `Identity` |
| Core state PDA re-derived from its own identity; market/release/generation join | `0x5002` `Identity` |
| Core phase `TerminalOrRetiring` **and** `terminal_receipt.is_some()` | `0x5002` `Identity` |
| terms project to the Market-selected config | `0x500B` `SelectionConfig` |
| root PDA, owner, bump, terms, market, rent beneficiary, **frozen revision** | `0x5008` `Representation` |
| TokenBehavior record finalized and selecting the request's Token program | `0x5009` `Token` |
| cursor address vacant — system-owned and empty | `0x5008` `Representation` |
| allocation post-check: Claims-owned, 296 bytes, funded | `0x5008` `Representation` |
| receipt construction and self-binding | `0x5007` `Receipt` |

`TerminalOrRetiring` rather than `Exactly(Terminal)` for the reason the tree
already records twice: `begin_retiring` is permissionless, so refusing at
`Retiring` would let any stranger push a Market one phase forward and strand a
retirement that had not started.

Anti-replay is the cursor account's own existence. A second `Begin` finds a
Claims-owned, non-empty address and refuses at the vacancy check. No counter,
no cursor about the cursor.

### `Finish`

13 accounts, likewise permissionless. `FractionalRetirementCursorV3::finish`
refuses unless `next_coordinate == representation_width`, so a cursor abandoned
mid-walk cannot be closed out from under the coordinates it still owes
(`0x5008`). The RentCredit is authenticated through the same
`authenticate_rent_credit` the Position close uses, now taking a named
`LifecycleRentCreditIdentityV2` instead of a `ProtocolPositionRequestV2` so both
callers ask one author; a RentCredit that is not the Market's own refuses at
`ProtocolPositionSbfErrorV2::Rent`, `0x5146`. The close is the tree's five-step shape:
`fill(0)`, absolute lamport assignment, `resize(0)`, `assign(system)`, re-verify
all four.

### Two griefing surfaces, closed

Both are the same lesson `protocol_position_v2` recorded for admission,
arrived at from the other end — there a donation blocked entry, here it blocked
the exit.

- **The cursor's live lamport check is now a floor.** It was an equality
  against the declared principal. A cursor is a keyless, off-curve address, so
  anyone may send it lamports at any time: under the equality, one stranger
  lamport froze a fractional market's retirement **permanently** — the cursor
  could never advance, never finish, and every shard holder's collateral stayed
  behind a Position nothing could close. For about one lamport, once.
  Underfunding still refuses, and rent exemption is checked against the declared
  principal rather than the live balance so a donation cannot paper over a
  cursor that was never exempt.
- **`Begin` tops up rather than transferring a fixed amount**, and `Finish`
  settles the account's **entire** balance rather than the principal it
  declared. A cursor is about to stop existing; a lamport left in it is burned.

## Tests

### Contract (`crates/dclutch-fractional-claim-contract/tests/fractional_v3.rs`, 14/14)

| test | what it pins |
|---|---|
| `the_root_revision_anchor_is_constant_across_the_whole_ordered_walk` | the anchor over begin + 3 coordinates + finish, asserting at each step that the request revision has already diverged from the root's. This is the test that would have caught (2). |
| `an_anchor_underflow_is_an_arithmetic_refusal_and_never_a_wrapped_revision` | a forged cursor whose coordinate advanced without its revision refuses rather than wrapping |
| `both_ends_of_the_walk_round_trip_and_bind_to_the_request_that_produced_them` | begin names the root's revision and finish the cursor's; both leave `expected + 1`; a substituted digest and a neighbouring revision each refuse |
| `a_lifecycle_receipt_cannot_disagree_with_itself_about_the_lamports` | begin settling anything, finish settling nothing, and finish stranding part of the principal are all unrepresentable; a donation above the principal round-trips |
| `only_a_complete_walk_finishes_and_only_finish_claims_one` | the completeness equivalence, from both sides |
| `a_lifecycle_receipt_may_not_carry_the_coordinate_action` | the two receipt families do not share a magic and neither impersonates the other |
| `both_lifecycle_frames_are_exact_and_below_the_coordinate_frame` | 16 and 13, both under the lock bound |

### On chain (`programs/dclutch-claims-sbf/program-test/protocol-position/tests/lifecycle.rs`, 6/6)

| test | what it pins |
|---|---|
| `a_fractional_market_retires_end_to_end_from_begin_through_finish` | the whole walk; conservation in lamports at every step and once over the walk; `root_revision_anchor == 4` before and after the coordinate, which is the only place (2) is observable on chain; a donated cursor makes begin's bill zero; finish settles principal **plus** the stray |
| `an_incomplete_walk_cannot_be_finished_and_the_cursor_survives_the_attempt` | `0x5008` at the revision begin leaves behind — the only one a caller could try — and the cursor byte-identical afterwards |
| `a_second_begin_refuses_on_the_cursors_own_existence` | `0x5008` on replay, the cursor undisturbed, and the top-up path: an underfunded cursor lands on exactly `minimum_balance(296)` |
| `begin_admits_a_readonly_coordinate_that_the_callers_other_instruction_writes` | the writability exemption, proved red against the pre-fix ELF (`0x5001`) before it was trusted green |
| `real_sbf_late_token_refusal_rolls_back_fractional_position_mint_and_cursor` | repaired, and now reaching the Token CPI it asserts about for the first time since `4630ad77` |

Conservation, stated exactly, over the whole walk:

```text
rent_credit_after == rent_credit_before
                   + position_lamports + admission_lamports + mint_lamports
                   + cursor_lamports
```

with `cursor_lamports == minimum_balance(296) + 4,242` — the stray the fixture
plants on the address before anyone begins.

## Not verified

- **No devnet write**, and no gauntlet witness-set run. The campaign was driven
  directly with a scratch `SBF_OUT_DIR`.
- **The walk is width 1.** `compile_narrow_fixture_v2` refuses below outcome
  count 3, and this campaign's terms carry a single shard mint, so the *ordered*
  part of ordered retirement — coordinate 0 then coordinate 1 — is pinned by
  the contract test over three coordinates, not on chain. The anchor is
  asserted on chain at both ends of the one coordinate that runs, which is what
  distinguishes the fixed root from the moving cursor; a second on-chain
  coordinate would add no new arithmetic.
- **The Market is resolved between transactions with `set_account`**, not by
  the Resolution program. `Admit` requires Core to be exactly `Open` and
  `Begin` requires Terminal-or-Retiring, so a campaign driving both with real
  routes has to move the Market between them. Only `phase` and
  `terminal_receipt` are written; everything else is left as the shared
  Product/LBV2 fixture compiled it.
- **The reserve Position's owner kind is planted, not driven** through a real
  `protocol_position_v2::Admit` on the Fractional path — the same fixture gap
  FRACR3 named still stands.
- **The Trading program account is not authenticated by release** on any of the
  three acts; the coordinate path never did either, and this lane matched it
  rather than widening. A forged Trading program can only produce a cursor at a
  different address, which no real reserve Position joins — but it is a gap,
  named as one.
- **CU is measured at width 1 only.** Nothing here bounds a wide walk; each
  coordinate is its own transaction, so width scales transactions rather than
  compute, but that is an argument and not a measurement.

## Two seam-audit findings, and what they were actually about

`tools/seam-audit` reported both of this lane's frames.

**`LIABILITY_BASIS_MARKET_SEED_V2` spelled raw** at the aggregate derivation.
The tree has two dozen sites that spell this two-seed tuple by hand and the
owning crate exported no constructor for it, so the honest fix was to add one
rather than to copy the twenty-fifth spelling:
`LiabilityBasisMarketSeedsV2::new(market).as_slices()` now lives beside the
domain in `crates/dclutch-claims-svm/src/liability_basis_state_v2.rs`, refuses
the zero identity where the seeds are made, and is pinned against the
hand-spelled tuple by
`the_seeds_constructor_reproduces_the_hand_spelled_tuple_exactly`. The other
sites are pre-existing debt and were not touched by this lane.

**`PRIVILEGE_PIN_UNEXEMPTED`** on `authenticate_lifecycle_frame`, and this one
was a real defect rather than a style finding. The frame pinned exact
writability on every coordinate. `is_writable` is a **transaction-level**
property — the runtime merges an account's privileges across every instruction
of the transaction that names it — so a readonly pin does not constrain this
instruction, it forbids the caller's *other* ones. Concretely: `Finish` must
take the RentCredit writable and `Begin` only reads it, so the readonly pin at
`BEGIN_RENT_CREDIT` made the two acts of one walk **unbatchable**. The same
shape `16351a13` fixed on Custody, where a checkpoint pinned readonly could
never compose with the Trading ingest that is its documented atomic partner.

`FrameRoleV3` replaces the privilege triple and pins writability in one
direction only: `Written` and `SigningPayer` must be writable, because that is
a requirement of this instruction; `Read` and `Program` accept either, because
it is not. Nothing is lost — a read-only coordinate arriving writable changes
no byte this route reads, and every account reachable through one of its CPIs
is `Written` or `SigningPayer`.

## Left queued (both FRACR3's, neither in a file this lane edited)

- `ClaimsSbfError`'s upper-band assertion names `ReleaseSuperseded` (`0x500A`)
  while the tail variant is `SelectionConfig` (`0x500B`). Stale by one, not yet
  wrong. The fix worth making is an exhaustive list a new variant cannot join
  silently, not a bumped name.
- `FRACTIONAL_ROOT_PDA_SEED_V1` in `atomic_v3.rs` is dead; nothing derives with
  it.
