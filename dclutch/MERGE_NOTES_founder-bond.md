# MERGE NOTES — founder-bond (CONVERGE)

Branch `build/founder-bond`, rebased onto `main` (`dfcf02069`) with one conflict,
resolved. 38 files, +4,471/−239, 16 commits.

## 1. READY

**READY.**

`cargo check --workspace --tests --offline` is **green, zero errors.** That is the whole
workspace, not one crate — including the five downstream crates the close-out warned
about and four more it did not find.

**No warning in the whole workspace names anything this family added.** Five files this
family touches carry warnings, every one of them about something else and present on
`main` (`account_of`, `completion_name`, `beneficiary`, `release_set`, a duplicated
attribute); they are not cleared here. The one exception is three dead imports on a line
this family edited (`tools/gauntlet/journey/src/ledger.rs`), which are deleted.

Green besides:

| gate | result |
| --- | --- |
| `cargo check --workspace --tests --offline` | green |
| `cargo test -p dclutch-claims --lib founder_bond_v1` | 15 pass |
| `cargo test -p dclutch-claims --lib claim_check_conservation_v1::tests` | 39 pass (4 new) |
| `cargo test -p dclutch-operator --lib wallet_terminal_payout_v3::tests` | 7 pass (3 bond) |
| `lake build` (whole 137-module library) | green, 36 s |
| SDK `vitest run` | 911 pass, 0 fail |
| SDK `tsc --noEmit`, web `tsc --noEmit` | clean |
| `tools/gate census` | PASS, 355 codes, 0 unclassified |
| `tools/cohort/check-steps.py --cohort 17` | 39 steps, green |

**Not green, and not this family's to fix.** All pre-existing on `main`, each convicted
rather than assumed:

- `apps/dclutch-web` **`lib/sbomVerify.test.ts`**: `tools/sbom/SBOM.md` is stale by the
  whole Solana 2.1.0 → 3.1.12 bump (I regenerated it to see, then reverted: 480 lines,
  none of them mine). Owner is whoever bumped Solana.
- `apps/dclutch-web` **`abi:sbf-runtime:verify`**: `Cargo.lock pins 2 copies of
  solana-sbpf: 0.13.1, 0.23.0`. Two copies on `main` too; this family changed no lock.

**Red because of this family, and owed to the convergence owner** (see §2).

## 2. SHARED FILES — for the merge lane

### 2.1 The refusal code — DO THIS FIRST

```
programs/dclutch-claims-sbf/src/lib.rs:357    FounderBondFrame = 0x5012,   ->   = 0x5013,
```

**The bond's frame code is `0x5013`.** `build/failure-arm` lands first with
`Overdraw = 0x5012`; decision 0007 permits the renumber because neither has shipped.

It is `0x5012` on this branch and could not have been anything else: `pin_refusal_band!`
asserts the discriminants are the **contiguous run from the band base**
(`crates/dclutch-refusal-registry/src/lib.rs:286-289`), so `0x5013` does not compile until
`0x5012` is occupied, and only the failure arm may author `Overdraw`.

This is fail-closed, not a hope. Both branches insert their variant in the same place —
last in the enum, last in the `pin_refusal_band!` list — so a textual merge puts
`Overdraw` above `FounderBondFrame` and rustc then refuses the merge with
*discriminant value `0x5012` assigned more than once* until the literal moves. There is
no silent wrong outcome.

The enum is the **sole author** of that number: `grep -rn 0x5012` over the tree outside
`target/` and the ledger finds `lib.rs:357` and nothing else. The runbook row that used
to pair the name with the hex now names only `ClaimsSbfError::FounderBondFrame`.

### 2.2 Generators the convergence owner runs

| what | command | why it is owed |
| --- | --- | --- |
| refusals reference + its client mirror | `tools/gate reference --converge`, then `npm run abi:refusal-registry` in `packages/dclutch-sdk` | the three new codes reach `docs/reference/refusals.md` and `refusalRegistryV1.ts` nowhere else. AGENTS.md gives this to the convergence owner and it closes a cycle across every branch that adds a code. **Run it after the `0x5013` edit.** |
| SDK route census | already regenerated at this HEAD (`npm run abi:route-census`) | the SDK's own `abiVerification` gate was RED without it. **Regenerate again after `0x5013`** — the mirror carries `code: 20498`. No other build branch touches this file. |
| web wasm digest pins | in `apps/dclutch-web`, with one shared `DCLUTCH_WASM_TARGET_DIR`: `npm run abi:{wallet-terminal-payout,wallet-terminal-input,source-provider,source-readiness,user-position-admission}` | each pins a sha256 of a wasm artifact built from `dclutch-claims` / `dclutch-operator`. This family changes both, so all five are red here. They need `wasm32-unknown-unknown` + `wasm-bindgen`, and the digests change again after `0x5013` and after every other branch, so regenerating them per-lane is waste. |

### 2.3 Baselines and budgets this family leaves red

- **`tools/gates/frames-baseline.json` — the frame ratchet is RED and this branch does not
  capture it.** Four new `#[inline(never)]` symbols in the Claims link:
  `authenticate_founder_bond` (`founding_v5.rs`), `authenticate_founder_bond_arm` and
  `apply_founder_bond_draw` (`terminal_settlement_v3.rs`),
  `admit_founder_bond_disposition_v1` (`market_closure_v1.rs`). Capture with
  `tools/gate frames --at <commit> --capture` on the convergence commit; the preamble
  forbids SBF builds in a lane, so no lane can have done it.
- **`tools/gauntlet/CU_BUDGETS.json` / `.md`** — no row moved for founding (+ the bond
  conjunct), payout (+ the draw) or closure (+ the disposition). Provisional and
  unmeasurable without a run.
- **`docs/decisions/0007-namespaced-refusal-codes.md:129-139`**, the Claims sub-band
  table, three rows: `ClaimsSbfError` `0x5000–0x500B` → **`0x5013`**,
  `ClaimsFoundingSbfErrorV5` `0x5180–0x5190` → `0x5191`,
  `ClaimsMarketClosureSbfErrorV1` `0x5500–0x5505` → `0x5507`. **Collision:**
  `build/claims-split-merge` adds a row to the same table
  (`ClaimsConservationSbfErrorV1 | — | 0x5300–0x530C`) and `build/failure-arm` extends
  the first row too. Land all of them in one commit; a textual merge resolves cleanly
  and is still wrong if any row is dropped. Left untouched here for that reason.
- **`tools/cohort/cohorts/18.json` does not exist on `main`**, so this family's two rows
  (`founder-bond`, `refund-bond-walk`, both `since 18`) cannot be gated yet.
  `build/general-lifecycle` built that manifest. Run
  `python3 tools/cohort/check-steps.py --cohort 18` once both have landed.

### 2.4 The `steps.tsv` conflict, and how it was resolved

`main` rewrote the `retire` row wholesale (the frozen routing table, the lookup-table
producer, the phase byte). This branch appended one sentence to that row's verifier.
**Resolution: main's row verbatim, with this family's sentence appended to column 8** —
main wins on what it changed deliberately. `build/series` and `build/claims-split-merge`
also insert rows into this file; those are inserts, not edits to the same row.

## 3. What this lane completed, and what it ruled

### Closed since the build wave

1. **The five downstream seams were nine, and one was not a `None`.**
   `wallet_terminal_payout_exterior.rs` needed the bond PAIR: its payout poststate read is
   now 5 or 7 coordinates the way the frame is 36 or 39, and the escrow's and recipient's
   observed lamports go to the kernel's postcondition. Passing `None` there would have
   made a refunding market's exterior verification refuse `Postcondition` forever. The
   close-out also missed three `ledger::ObservationV1` sites (`ledger.rs` ×2,
   successor `main.rs`) and named the wrong struct at `:1532` (`Poststate`, not `Route`).
2. **`test-fixtures` on `dclutch-operator` had no consumer** — a producer-missing pattern
   in a Cargo manifest. `wire::tests::input()`, the fixture two wasm crates' tests name,
   was gated out of every build, so neither crate's tests had compiled. Fixed in the two
   dev-dependencies; the fixture module's assert-only imports and helpers are now
   `#[cfg(test)]` so a consumer taking the fixture does not compile the cases around it.
   Main's red, outside this family, fixed so this family's own gate could run.
3. **All four warnings §7 named**, cleared: the unreachable `RecipientRouteV1` alternative
   and two `rent` parameters `funded_rent_persists_v1` made dead.
4. **The vacuous theorem is gone** — see §5.
5. **The exhausting walk is walked** — see §5.
6. **S29**: the wallet-terminal generator resolves a frame coordinate the Rust *derives*,
   so the five bond scalars reach `walletTerminalPayoutV3.ts`. A browser could previously
   build the 36-account frame and no other. **S32**: the three tail coordinates are
   arithmetic off `TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3` rather than hardcoded 36/37/38,
   and the generator reproduces the same numbers, which is the join.
7. **S24**: `capabilitySurfaceV1.ts` regenerated. It also picks up one module `main` added
   and never regenerated for (`shadowDigestV3` on `/general`).
8. **S5**: `tools/gate census` PASSES at 355 codes, 0 unclassified.
9. **The kernel's dead half**: fourteen public items with no caller outside their own file
   are `pub(crate)`. The census stops counting them as surface; the tests still read them.

### Ruled provisionally, here

- **The bond's frame code is `0x5013`** and the branch carries `0x5012` because the band
  macro forbids a gap (§2.1). Reversal is one literal.
- **`docs/reference/*`, `refusalRegistryV1.ts` and the five web wasm digests are not
  regenerated per-lane.** AGENTS.md gives the reference cycle to the convergence owner;
  the wasm digests are invalidated by the very next commit that touches Claims. Handed on
  rather than churned.
- **`tools/sbom/{SBOM,NOTICES}.md` are not regenerated here** even though the test is red:
  the drift is the whole Solana major bump and belongs to whoever made it.

### Still open, named precisely

- **The compaction planner's bond draw (S22).** `crates/dclutch-operator/src/claim_check_v1.rs`
  passes `founder_bond_draw: 0` and builds the 42-account frame;
  `CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1` is measured off the 36-account terminal frame,
  so on a refunding Market the planner builds 45 metas and the route refuses `Binding`.
  Fail-closed and named at the site. The program's 44-account arm
  (`COMPACT_WITH_BOND_ACCOUNT_COUNT_V1`) therefore has **no builder anywhere in the tree**
  — a producer-missing pattern in miniature. The fix is a second constant plus the escrow
  pair and outstanding count at plan time, which is the same read
  `wallet_terminal_payout_v3` already does: one function, not one design. **Both halves of
  the crank's safety are now proved in Lean and in the crate** (§5), so what is missing is
  only the builder.
- **`refundsOnFailure` on the market page (S25).** Still passed `null` on purpose, so the
  page states the bond's AMOUNT and never its EXIT. A producer exists in the SDK
  (`directHotChain.ts:320 categoricalRefundsOnFailureV1`) but the page does not read the
  Product basis record; wiring it is a fifth RPC round trip. **Do not infer it from the
  escrow's seating** — that is the exact substitution main's ledger recorded on
  2026-09-06 as printing the wrong sentence on a refunding market.
- **The manifest a browser reads carries no tail (S23).** `ManifestRouteV3`
  (`crates/dclutch-operator/src/wallet_terminal_payout/wire.rs:631`) lists the thirty-six
  route keys as `String`s and no `founder_bond_escrow_position` /
  `_escrow_admission` / `_recipient`, so a browser that has the manifest still cannot
  address the escrow pair. The generated scalars now exist (S29, done) but the addresses
  do not. Three optional `String` fields beside the existing keys, `skip_serializing_if`
  like the three `PlanInputV1` fields already added — wire-compatible with cohort-16/17
  manifests. Not built here: the manifest's golden vector
  (`manifest_json_has_one_stable_golden_vector`) moves with it and that vector is the
  browser's own contract, so it wants its own commit rather than a tail on this one.
- **Nothing exercises the 39-account terminal frame, `0x5013`, or the SBF compaction bond
  arm on a real ELF.** Both program-tests that would (§5) need `SBF_OUT_DIR`.

## 4. What a reviewer should distrust in `BUILD_founder-bond.md`

I checked every load-bearing claim. Five are false or overstated.

| # | the doc says | what is true |
| --- | --- | --- |
| S17 | `wallet_terminal_payout_exterior.rs:1532` needs `founder_bond: None` | the struct is `WalletTerminalPayoutPoststateV3`, not the route, and `None` is the WRONG value — it makes every refunding payout refuse `Postcondition`. The exterior now observes the pair. |
| S17–S21 | five downstream sites | **nine.** Three `ledger::ObservationV1` constructions were missed entirely. |
| S26 | `refund-bond-walk`'s `-` invocation column needs a `blocked.json` row; "an empty column is neither" | there is **no `blocked.json` in `tools/cohort`** and **sixteen** existing rows carry `-` there. The row is the same shape as its peers and the runbook's own gate passes. Owed nothing. |
| S27 | the census row and the chain disagree about the ladder term, so "a laddered refunding market is green on chain and red at this row" | the host **does** fund Λ: `tools/local-validator/bootstrap/successor/src/market.rs:2362 ladder_funding_v1` sums each rung's Bounty quote off the manifest entry the policy's attempt names, and `:10575` passes it to `founding_bond_size_v1`. The chain's Λ = 0 is a floor **below** what the host funds (ruling R2), so the row reads back what the founding paid. Owed nothing unless some other driver founds a refunding market at the chain's floor — and the successor bootstrap is the only one that founds one. |
| S33 | `lib.rs:349`'s doc comment cites the tail SIZE where it means the frame WIDTH | read in context it is right: the sentence names three accounts and calls them a trailing tail, and `TERMINAL_SETTLEMENT_FOUNDER_BOND_ACCOUNT_COUNT_V3` is that tail's size. Owed nothing. |
| §2 "dead half" | `FounderBondClosureV1::surrendered()` has zero callers **including tests** | it has two, `founder_bond_v1.rs:822` and `:842`. It stays public. The other fourteen are now `pub(crate)`. |

Confirmed as written: the Lean is a **+57-line append** to a 536-line file already on
`main`, not a new module; five theorems, one of them vacuous (now four real ones plus
three new); `cargo check -p dclutch-operator` green proving less than it looks; the
`0x5012` collision.

## 5. Tests added, and what each proves

### Lean — `formal/dclutch-semantics/DClutchSemantics/FounderBondV1.lean`

`a_compacted_redemption_draws_what_the_holder_would_have` was
`draw r o q = draw r o q := rfl`. **Deleted.** The property it stood for is now stated
over terms that can differ:

| theorem | proves |
| --- | --- |
| `a_compacted_redemption_draws_what_the_holder_would_have` | `submittedRedemption` carries the submitter, and the submitter is an argument of the redemption and **not of its draw**. |
| `the_crank_pays_the_claim_check_address_instead` | the coordinate the submitter DOES reach is the destination — so the theorem above is a claim about two different redemptions rather than two names for one term. This is what makes the pair non-vacuous. |
| `a_compacted_record_carries_the_whole_draw` | a record minted over a draw holds its own rent AND the draw, whatever dust was already there. |
| `counting_the_draw_as_dust_absorbs_it` | **the hostile**: under the replaced policy the record ends at exactly its rent and the sleeper's share has gone to the cranker, the opener and the RentCredit. This is the lamport-losing policy `founder_bond_draw` exists to refuse. |

**Nobody had ever run `lake` on this file.** It elaborates; the whole 137-module library
builds; zero `sorry`; the six theorems above and the close's four depend on nothing but
`propext` and `Quot.sound`.

### Rust — `crates/dclutch-claims/src/claim_check_conservation_v1.rs` (4 new)

| test | proves |
| --- | --- |
| `a_founder_bond_draw_rides_into_the_record_and_the_dust_does_not` | the Lean pair's twin, with **both policies computed**, so the test states the loss rather than only the number the current code produces: correct → record at `rent + draw`; as-dust → record at exactly `rent`, and the difference lands on the other three sinks. |
| `the_sleeper_is_swept_the_whole_draw_the_crank_made_for_them` | the redemption side: a record minted over a draw pays the holder the draw as well as the rent. |
| `a_bond_draw_with_no_record_to_carry_it_refuses_by_name` | `Conservation`, exact name — a losing outcome mints no record, so a draw would strand on a vacant address. |
| `a_draw_larger_than_the_address_holds_refuses_by_name` | `Conservation`, exact name — the caller's number checked against the chain's. |

### Rust — `crates/dclutch-operator/src/wallet_terminal_payout_v3.rs` (1 new)

`an_exhausting_walk_drains_the_escrow_to_exactly_its_recorded_rent`.
`an_exhausting_walk_pays_the_bond_exactly` was proved in Lean, computed in the kernel and
**walked nowhere**. It is walked here through the builder that submits it: two redemptions,
each one's prestate the previous one's *projected* poststate — aggregate, Position, replay,
tokens and the escrow's own lamports — so nothing about the sequence is retyped between
steps. The first draws half the bond and leaves the escrow above its rent (a walk whose
first step already emptied the escrow would prove nothing); the second draws everything
left; **the escrow ends holding exactly the rent its admission recorded and the two draws
sum to the bond.** Both frames are 39 accounts. It also pins that a third redemption is
refused `Route` — neither the Position nor the aggregate holds a claim there, and that
refusal arrives BEFORE the bond arithmetic — and that the escrow's own redemption at the
failure coordinate draws zero.

`fixture_with_scale` gains the ordinary supply as a parameter: a walk that exhausts the
bond has to exhaust the ordinary claims, and the fixture's owner held two of five.

### The two tests that compile and cannot run here

`programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_founding.rs` and
`crates/dclutch-svm-harness/tests/market_retirement_v1_lifecycle.rs` both need
`SBF_OUT_DIR`; the preamble forbids SBF builds, and a run against a **stale** ELF would
prove nothing anyway (the Claims ELF moves on this branch — `0x5191` does not exist in
cohort-17's). They were **audited against the program code instead**. No assertion would
fail. Three defects found and fixed:

- **`CheckpointMarketRetirementReportV1::founder_bond_lamports` and `founder_bond_exit`
  were asserted nowhere in the tree.** The harness could have seeded any principals and
  every other assertion would still have passed — which means the sub-agent's
  `1/1/1/1` → real-principals correction was unobservable. Both are asserted now
  (`market_retirement_v1_lifecycle.rs`, beside `failure_escrow_rent_lamports`), and that
  is what makes the fixture's `founding_bond_size_v1` call load-bearing.
- **The honest arm's positive control claimed more than it gives.** `Liability` (`0x5503`)
  has four raise sites and two sit UPSTREAM of the bond arm, so reaching it proves the
  fixture reached the closure and no more. What proves the escrow identity, the residue
  equality and the bond arm all passed is the exhausted arm's `0x5507`, raised at exactly
  one site and only after all three. Comment corrected.
- **`claims_founding.rs` said the bond is "an exact quantity, not a floor".** `founded_v1`
  is `recorded_rent + bond <= lamports` — a floor, and `founding_v5.rs` says so. What
  MANDATORY means is that the floor is not zero. Prose corrected. One decode whose result
  was discarded (`let _ = ….is_err();`) now asserts.

Two more the audit found, left as they are and named here: `assert_ne!` on two enum
constants (`claims_founding.rs`, twice; `market_retirement_v1_lifecycle.rs`, once) is
constant-folded and documents rather than checks — the load-bearing content is the
`assert_eq!`s above it; and `an_escrow_holding_its_rent_but_no_bond_refuses_the_bond_not_the_rent`'s
control zeroes BOTH escrow accounts where a one-variable control would zero only the
Position.

### Arithmetic checked across the boundary

The fixture, the program and the harness all derive the rent rate by
`derive_funded_rent_rate_v2(rent.minimum_balance(0), position_width, position_rent)`, take
the width from `liability_basis_vector_width_v2`, and pass `ladder_funding = 0`. At rate
6,960 and four outcomes the size rule gives `seat_prepay 3,062,400 + first_crank_shortfall
1,348,400 = 4,410,800`, which is the figure `founder_bond_v1.rs:629` pins as a Lean
witness. No disagreement found on either side.
