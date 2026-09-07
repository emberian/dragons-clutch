# BUILD-DEALER — the scoring-rule Dealer, built by source inspection

Lane BUILD-DEALER, 2026-09-06, worktree branch `build/dealer` off `main` at
`f7c03e845`. Nothing here was built for SBF, run against a validator, or
merged; the convergence wave compiles and enmeshes it. What WAS run: the Lean
ABI module builds (`lake build DClutchSemantics.ScoringRuleAbiV1`), both
emitters run and the Rust emission is byte-identical to an independent Python
twin of the arithmetic, `cargo check -p dclutch-trading` is green, and the
kernel's 27 unit tests pass against the corpus Lean evaluated.

The design is `docs/design/MECHANISM_SCORING_DEALER_2026_09_04.md` and the
Lean owner is `formal/dclutch-semantics/DClutchSemantics/ScoringRuleV1.lean`;
this document is the delta.

## 1. What was built, file by file

### 1.1 Lean (the layouts, first)

| file | what |
|---|---|
| `formal/dclutch-semantics/DClutchSemantics/ScoringRuleAbiV1.lean` | the fund record (`DCLSFUN1`, 224 B), the quote record (`DCLSQUO1`, 240 B), four request wires (`DCLSFDR1` 160, `DCLSQTR1` 88, `DCLSFLR1` 512, `DCLSWDR1` 96), the one receipt (`DCLSRCP1`, 192), the accelerator's fill witness (`DCLSFLW1`, 272), the four account frames (21 / 6 / 19 / 14 slots, one signer at zero — a theorem), the fill's cash legs (`cashLegs`, with `hoard_receives_the_mint` and `fund_moves_by_the_debit` proven), the PDA domains, the sub-band offset. Every width is `decide`, every layout `tiles`. Field-name functions for both emitters live here so Rust and TS print the same identifier. |
| `formal/dclutch-semantics/EmitScoringRuleV1Rust.lean` | prints `crates/dclutch-trading/src/scoring_rule/generated_scoring_rule.rs`: the 63-entry root-chain table as `u128`, the Q62 constants, magics, widths, offsets, domains, frames (index constants + `_WRITABLE`/`_SIGNER` tables), and under `#[cfg(test)]` a corpus Lean evaluated (`exp2Neg`, `log2Ceil`, `subsidyOf`, `lmsrValue`, `pricesOf`). |
| `formal/dclutch-semantics/EmitScoringRuleV1Ts.lean` | the same object as `packages/dclutch-sdk/lib/generated/scoringRuleV1.ts` (`bigint` for the table and Q62 constants). |
| `formal/dclutch-semantics/DClutchSemantics.lean` | `import DClutchSemantics.ScoringRuleAbiV1` added (the root imports every module). |

`ScoringRuleV1.lean` itself is untouched: its rule record (`ruleSchema`, 112 B), table, potential, prices, subsidy and theorems are consumed, not restated.

### 1.2 The kernel and codecs (`crates/dclutch-trading/src/scoring_rule/`)

| file | what |
|---|---|
| `mod.rs` | the kernel: `exp2_neg` (Ê), `log2_ceil` (L̂), `potential` → `Potential { minimum, cost }` (Ŵ = minimum − cost, both `Nat`), `prices_of` (p̂), `subsidy_of`, `require_simplex`, `rounded_debit` (`roundedQuoteFor`: receipt up, delivery down, once), `admit_fill` (R0–R3, the debit derived here — one author, never on the wire), `withdraw_admissible` (`withdraw_floor`), `admit_founding`, `RuleParameters::admit` (`parametersAdmissible` + `τ < scale`), `ScoringRefusal` (14 named conjuncts). `no_std`, `no_alloc`, every intermediate checked. 27 tests: the table is the root chain, every corpus value agrees with Lean, prices are a simplex at 64 sampled states, the null fill clears, `NotNormalized`/`OffSchedule`/`Uncovered` each refuse by name, Φ never falls along an 8-leg round trip, founding holds the deposit to the subsidy, the withdraw floor is exactly Φ, six hostile parameter sets refuse by name. |
| `generated_scoring_rule.rs` | emitted; guarded by `tests/scoring_rule__generator_fresh.rs`. |
| `records_v1.rs` | `ScoringRuleRecordV1`, `DealerFundV1` (with `potential()`, `cash_claims()`, `debit_atoms()` — the ONE atoms↔claims conversion), `DealerQuoteV1` (`is_fresh(fund)`), PDA seeds, `read_vector`/`write_vector`. |
| `requests_v1.rs` | `DealerFoundRequestV1`, `DealerQuoteRequestV1`, `DealerFillRequestV1` (with `taker_delta`/`dealer_delta`), `DealerWithdrawRequestV1`, `DealerReceiptV1` (`from_fund`, `authenticate_for_request`), `DealerFillWitnessV1`, the four `is_dealer_*_v1` predicates, the four `*_privileges_v1` tables read off the emitted frames. |
| `solver.rs` (host-only) | `invert_prices` (`inv′_i = b·log₂(p_max/p_i)` through the kernel's own `L̂` — integers only), `solve_fill` (target price → (mint, receive, deliver, p̂)), `solve_buy` (the RFQ taker: buy `claims` of `outcome`, delivering from inventory before minting). |
| `crates/dclutch-trading/src/lib.rs` | `pub mod scoring_rule;` |
| `crates/dclutch-trading/tests/scoring_rule__generator_fresh.rs` | the emission guard (build the ABI module, run the emitter, rustfmt-normalise, byte-compare). |

### 1.3 The routes (Trading, band 4, sub-band `0x4100`)

`programs/dclutch-trading-sbf/src/scoring_dealer_v1/` — **2,037 lines, five files,
committed by CLOSEOUT exactly as the maker left them.** Every route is selected by
its magic alone, parses a fixed PREFIX of accounts the Lean states, and hands the
child WINDOWS that follow it — exact Custody and Claims frames, in a stated order —
to the child programs, which refuse their own frames by their own names. This
program authenticates what is its own (the Market, the fund, the rule, the Dealer's
Position, the release, the arithmetic); Custody and Claims authenticate theirs. The
kernel is LINKED, not called across a CPI.

| file | lines | what |
|---|---|---|
| `mod.rs` | 669 | `ScoringDealerErrorV1` (25 variants, `0x4100..0x4118`, pinned through `dclutch_refusal_registry::pin_refusal_band!` at `TRADING_REFUSAL_BASE + generated::REFUSAL_SUB_BAND_OFFSET`); `parse_prefix` (frame width + privileges + alias); `get`; `MarketFactsV1` / `authenticate_market_v1`; `authenticate_release_v1` (activation cache + Trading role); `authenticate_fund_v1` (PDA, body, owner, market, dealer, revision); `authenticate_rule_v1` (rule PDA + `rule_digest` seal); `read_inventory_v1` (the Dealer's Claims Position); `authenticate_vault_v1`; `CustodyLegV1` + `invoke_custody_transfer_v1` + `read_replay_v1`; `commit_fund_v1`; `write_quote_v1`; `emit_receipt_v1`. `extern crate alloc;` lives here. |
| `found.rs` | 543 | `DealerFound` (`DCLSFDR1`): seal a rule, open a fund, admit the Dealer's Position, write the first quote. Anyone may found; the founder is the sponsor of record. `foundFrame` (21) then **four windows in the only order Custody admits**: `InitializeReplay` (13, revision 0→1), `OpenVault` (16, 1→2), `Transfer` (14, 2→3, sponsor → vault), Claims `Admit` (26, the Position owned by the fund PDA). |
| `quote.rs` | 123 | `DealerQuote` (`DCLSQTR1`): write `p̂(inv)` into the quote account, permissionlessly, so the price series is a CHAIN fact (0031 §5 defect 6 — it lived only in a candidate account). `quoteFrame` (6), no child window; derived from the Position the chain holds, never the fund's cache, and bound to the fund revision. |
| `fill.rs` | 553 | `DealerFill` (`DCLSFLR1`): the batch of two, Direct as an RFQ. `fillFrame` (19) then four windows: Claims signed delta (20 + 2) then three Custody `Transfer`s (14 each) — fund→Hoard, taker→Hoard, and fund↔taker whichever way the net goes. Legs are `ScoringRuleAbiV1.cashLegs`; a zero leg is not invoked. **The rule is checked FIRST** (R0–R3 over the inventory the Position holds) and nothing moves unless it admits. `cash_legs_v1` / `CashLegsV1` are the Rust twin of the two Lean theorems. |
| `withdraw.rs` | 149 | `DealerWithdraw` (`DCLSWDR1`): the sponsor takes cash down to `Φ = cash + Ŵ(inv)` and not one atom more (`withdraw_floor`). `withdrawFrame` (14) then one Custody `Transfer` (14): vault → the sponsor's token account. Admitted while the Market is Open (profit, or subsidy the inventory no longer needs) and after it is terminal (the residue — §4.2). |

- `programs/dclutch-trading-sbf/src/lib.rs` — `#[cfg(feature = "dealer-family")] pub
  mod scoring_dealer_v1;` and four dispatch arms in `process_instruction`, each
  guarded by the same cfg and selected by `is_dealer_*_v1(instruction_data)`.
  `dealer-family` was ALREADY declared in `programs/dclutch-trading-sbf/Cargo.toml:26`
  and already pulled in by `families` — main declared a feature that gated nothing,
  and this branch is what it gates.

## 2. Route table

| route | magic | request width | frame | codes | phase |
|---|---|---|---|---|---|
| `trading/scoring_dealer_v1::process_dealer_found_v1` | `DCLSFDR1` | 160 | 21 accounts, signer 0 = sponsor | `0x4100..` (see §2.1) | `market: Open` |
| `trading/scoring_dealer_v1::process_dealer_quote_v1` | `DCLSQTR1` | 88 | 6, signer 0 = payer (permissionless) | | `market: Open`, `fund: Open` |
| `trading/scoring_dealer_v1::process_dealer_fill_v1` | `DCLSFLR1` | 512 | 19, signer 0 = taker | | `market: Open`, `fund: Open` |
| `trading/scoring_dealer_v1::process_dealer_withdraw_v1` | `DCLSWDR1` | 96 | 14, signer 0 = sponsor | | `fund: Open`; `market: Open` or `Terminal`/`Retiring` (the residue) |

### 2.1 The sub-band (`ScoringDealerErrorV1`, `TRADING_REFUSAL_BASE + 0x100`)

| code | variant | conjunct |
|---|---|---|
| `0x4100` | `Frame` | account count, a privilege, or an alias |
| `0x4101` | `Request` | the wire did not decode |
| `0x4102` | `Market` | not the canonical Core state, or its phase refuses the route |
| `0x4103` | `Release` | the activation cache / Trading role |
| `0x4104` | `RuleSeal` | the rule PDA, its body, or the fund's `rule_digest` |
| `0x4105` | `Parameters` | `parametersAdmissible` false |
| `0x4106` | `Subsidy` | recorded ≠ `subsidyOf`, or deposit < subsidy |
| `0x4107` | `Fund` | fund PDA / body / owner / market / dealer |
| `0x4108` | `FundStale` | `expected_fund_revision ≠ fund.revision` |
| `0x4109` | `Position` | the Dealer's Claims Position PDA or body |
| `0x410A` | `Width` | R0 |
| `0x410B` | `Deliverable` | R0 |
| `0x410C` | `NonCanonical` | R0 |
| `0x410D` | `NotNormalized` | R1 |
| `0x410E` | `OffSchedule` | R2 |
| `0x410F` | `Uncovered` | R3 |
| `0x4110` | `PricesNotSimplex` | Σp̂ ≠ scale or a zero coordinate |
| `0x4111` | `WithdrawBelowFloor` | amount > Φ |
| `0x4112` | `Sponsor` | the signer is not the fund's sponsor |
| `0x4113` | `Custody` | a vault, token account or Custody receipt disagrees |
| `0x4114` | `Claims` | the aggregate, a Position or a Claims receipt disagrees |
| `0x4115` | `Overflow` | a checked intermediate (unreachable under `Parameters`) |
| `0x4116` | `Commit` | write-back digest mismatch |
| `0x4117` | `Phase` | the fund is retired, or the Market's phase refuses the route |
| `0x4118` | `Quote` | the quote PDA or body |

## 3. THE SEAMS

_(filled in as each piece lands; every row is `file:line` + the one-line change)_

| # | file:line | change |
|---|---|---|
| S1 | `formal/dclutch-semantics/DClutchSemantics.lean:113` | `import DClutchSemantics.ScoringRuleAbiV1` — DONE on this branch |
| S2 | `crates/dclutch-trading/src/lib.rs:71` | `pub mod scoring_rule;` — DONE on this branch |
| S3 | `tools/gates/emission-coverage.md` | `tools/gate emission --write` regenerates the census: two new generated files (`generated_scoring_rule.rs`, `scoringRuleV1.ts`), two new emitters, guard `crates/dclutch-trading/tests/scoring_rule__generator_fresh.rs` (cargo-test, normalises) and the SDK `abi:scoring-rule:verify` script (lean-emit) |
| S4 | `packages/dclutch-sdk/package.json` scripts | add `"abi:scoring-rule": "node scripts/lean-emit.mjs DClutchSemantics.ScoringRuleAbiV1 EmitScoringRuleV1Ts.lean lib/generated/scoringRuleV1.ts ts"` and the `:verify` twin with `--check ts` |
| S5 | `programs/dclutch-trading-sbf/src/lib.rs:127-130` | **A DOC COMMENT WAS REBOUND.** The new `pub mod scoring_dealer_v1;` was inserted BETWEEN `/// Wallet-authorized caller for one canonical Claims User Position.` and the `pub mod user_position_admission_v1;` it documented. That sentence now documents the Dealer module (whose own `///` line follows it), and `user_position_admission_v1` has none under `#![deny(missing_docs)]` (`lib.rs:3`). Move the Dealer's declaration below `user_position_admission_v1`, or move the stolen line back. |
| S6 | `programs/dclutch-trading-sbf/src/scoring_dealer_v1/fill.rs:20` | `SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3` is imported from `dclutch_claims::signed_delta_v3`, where it does not exist. It is `dclutch_claims::frame_spec_v1::SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3` (= 20; `composition_v3.rs:44` carries an identical second spelling — pre-existing debt, not this branch's). One-line import move. |
| S7 | `scoring_dealer_v1/{found,fill,withdraw}.rs` (10 sites) | `alloc::vec![…]` / `alloc::vec::Vec` are written in the CHILD modules, but `extern crate alloc;` is in `mod.rs`, which binds `alloc` in that module's namespace only. Either add `extern crate alloc;` to `programs/dclutch-trading-sbf/src/lib.rs` (crate root) or `use alloc::vec::Vec;` per child module. |
| S8 | `scoring_dealer_v1/found.rs:464` | `ProtocolPositionRequestV2 { … }` is missing the field `capability_outcome: u32` (`crates/dclutch-claims/src/protocol_position_v2.rs:354`). The Dealer's Position is not a capability position; the value the founding means is almost certainly the sentinel the other non-capability callers pass — read one before choosing. |
| S9 | `scoring_dealer_v1/{found,fill,withdraw}.rs` (3 sites) | `pub fn process_dealer_*_v1(program_id: &Pubkey, accounts: &[AccountInfo<'_>], …)` fails borrowck through `parse_prefix<'accounts, 'info>`: `__AccountInfo<'a>` is invariant, so the elided lifetimes must be tied. rustc's own suggestion is the fix: `pub fn process_dealer_*_v1<'a>(program_id: &Pubkey, accounts: &'a [AccountInfo<'a>], …)`. `quote.rs` is unaffected (it takes no windows). |
| S10 | `docs/reference/refusals.md` + the census band pin | 25 new Trading codes in sub-band `0x4100`. Generated by `tools/gate reference --converge`; `--check-unique` must see them. |
| S11 | `tools/gates/frames-baseline.json` | the Trading link moves (four routes, 2k lines). Recapture with `tools/gate frames --at <commit> --capture` after the SBF build. |
| S12 | **No runbook row, no successor driver, no SDK decoder for the ROUTES.** §4.1 names `devnet-dealer-found-v1` and a `found-dealer` row with `--dealer-fund ATOMS --dealer-b B [--dealer-scale --dealer-tolerance]`; neither the command nor the row exists. `tools/cohort/steps.tsv` and `tools/local-validator/bootstrap/successor/src/main.rs` are untouched by this branch. |
| S13 | `programs/dclutch-accelerator-sbf` | §4.7 rules that the accelerator's Dealer arm evaluates the same kernel over `DealerFillWitnessV1` (`DCLSFLW1`, 272 B, emitted and in the Lean). **The arm was never written**; the witness layout has no producer and no consumer. |

## 4. Provisional rulings (ember rules by reversal)

1. **The scoring Dealer is founded after the Market is Open, by its own route, by anyone.** `DealerFound` (`DCLSFDR1`) seals the rule, opens the fund's `TradingPrincipal` vault, deposits, and admits the Dealer's Claims Position. It does not ride the generic Market founding (`DCLTGMF3`), so no founding frame moves and the failure-arm/general lanes' founding work is untouched. The founding input is the `found-dealer` runbook row's `--dealer-fund ATOMS --dealer-b B [--dealer-scale --dealer-tolerance]` to `devnet-dealer-found-v1`.
2. **The founder's fund refund source is the sponsor recorded at founding.** `DealerFundV1.sponsor` is the only signer `DealerWithdraw` admits and the address the residue returns to; under 0025's failure selector the Dealer's ordinary claims are refunded pro rata into its Position and `DealerWithdraw` (Market terminal, fund Open) takes the cash out to that sponsor down to Φ — which, with the inventory redeemed, is the whole balance. No second refund-source field, no per-plan source (0021 governs plans, not this participant's own capital).
3. **A quote is fresh iff its `fund_revision` equals the fund's current revision; `DealerFill` never reads the quote.** R2 against the post-fill state is the only price authority, so a stale quote can mislead a reader and nothing else; the SDK decoder reports `fresh`. One quote account per Dealer, rewritten in place, permissionless to crank.
4. **The Direct-as-RFQ fill mints.** A fill carries `mint` complete sets minted at par into the fill; the Dealer's row is `(receive, deliver)` exactly as `ScoringRuleV1.Fill`; the taker's Position moves by `mint + deliver − receive`, the Dealer's by `receive − deliver`, the aggregate by `mint`. The cash legs are `cashLegs` (Lean): the Hoard receives exactly the mint's par, the fund moves by exactly the Dealer's debit, the taker pays the rest — at most two Custody transfers. This is what lets a fresh Dealer (empty inventory) sell.
5. **The debit is derived on chain**, never carried on the wire: `roundedQuoteFor` over `(y, z, p̂)` at the rule's scale, receipt rounded up, delivery rounded down, once per fill (against the Dealer, which is what the theorem tolerates and `τ` lets a solver shade).
6. **Claim units ↔ atoms at one place.** The fund records `claim_unit_atoms` at founding (the founding input, to be authenticated against `ProductBasisV3::payout_scale` — seam S-x); every route converts through `DealerFundV1::debit_atoms`/`atoms`.
7. **The kernel links into Trading and the accelerator's Dealer arm evaluates the same kernel over the fill witness.** The route's own check is authoritative (≈39k–93k CU per §6 of the note); the accelerator arm exists so a General candidate verifier that already runs through the accelerator can evaluate a Dealer row by the same code, and so the family-magic dispatch names the scoring family.

## 5. Stubs, and why

- No `todo!()` and no `unimplemented!()` anywhere in the branch.
- What is STUBBED is structural, and it is the whole outside of the program:
  - **the accelerator's Dealer arm** (S13) — `DealerFillWitnessV1` is emitted from
    Lean, decoded in `requests_v1.rs`, and read by nobody;
  - **the successor driver and the runbook rows** (S12) — §4.1 states the founding
    input as if `devnet-dealer-found-v1` existed; it does not;
  - **the SDK's route decoders** — `packages/dclutch-sdk/lib/generated/scoringRuleV1.ts`
    carries the constants, but nothing decodes a `DealerReceiptV1` or renders a
    quote's freshness (§4.3 says the SDK "reports `fresh`"; that reader is unwritten);
  - **the claim-unit authentication** — §4.6 says `claim_unit_atoms` is "to be
    authenticated against `ProductBasisV3::payout_scale` — seam S-x". That seam was
    never numbered and the check is not in `found.rs`: the founding takes the
    sponsor's word for the conversion factor every later route divides by.

## 6. Tests written and what each would prove

- `crates/dclutch-trading/src/scoring_rule/mod.rs` tests (27, RUN, green): the emitted table is the root chain; Ê/L̂/subsidy/Ŵ/p̂ agree with Lean's corpus; simplex at every sampled state; the null fill clears every batch; R1/R2/R3 refuse by name; Φ never falls on an admitted path; the withdraw floor is exactly Φ; six hostile parameter sets.
- `records_v1.rs` / `requests_v1.rs` / `solver.rs` tests (RUN, green): codecs round-trip and refuse hostiles by name; every frame has one signer at zero; the inversion is the identity on the lattice; a buy from an empty Dealer mints and is admitted; a buy from inventory delivers before minting.
- `crates/dclutch-trading/tests/scoring_rule__generator_fresh.rs` (needs lake): the committed emission is the emitter's.

## 7. Frames and codes expected to move; the cohort

- The Trading ELF gains four routes and a 25-code sub-band; the accelerator ELF gains the scoring family arm. Both links move: **cohort-18** carries them (cohort-17 is deployed and closed at `157bdd9d3`; decision 0031's order — joint clearing, then the scoring Dealer — puts this behind JOINT-CLEARING's Trading change, so the two ride one cohort).
- Frame baseline rows owed by the convergence's SBF build (`tools/gate frames --capture`); this branch touches crates compiled into the Trading and accelerator links and states here that it leaves the ratchet to the convergence.

---

# STOPPED HERE (lane CLOSEOUT, 2026-09-06)

`cargo check -p dclutch-trading-sbf --offline` (run in a target dir private to this
family): **RED, 15 errors + 4 warnings.** `dclutch-trading` — the kernel, the
codecs, the solver, the emission — compiled CLEAN; every error is in the 2,037 new
program lines, and every one is mechanical. Verbatim, the first three:

```
error[E0432]: unresolved import `dclutch_claims::signed_delta_v3::SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3`
  --> programs/dclutch-trading-sbf/src/scoring_dealer_v1/fill.rs:20:62
   |
20 |     DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
   |                                                              ^^^ no `SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3` in `signed_delta_v3`

error[E0433]: cannot find module or crate `alloc` in this scope
   --> programs/dclutch-trading-sbf/src/scoring_dealer_v1/fill.rs:461:22
    |
461 |     let mut packet = alloc::vec![0_u8; packet_bytes];
    |                      ^^^^^ use of unresolved module or unlinked crate `alloc`

error[E0433]: cannot find module or crate `alloc` in this scope
   --> programs/dclutch-trading-sbf/src/scoring_dealer_v1/found.rs:401:20
    |
401 |     let mut data = alloc::vec::Vec::with_capacity(request_bytes.len() + CUSTODY_BUMP_RELAY_BYTES_V1);
    |                    ^^^^^ use of unresolved module or unlinked crate `alloc`
```

The remaining twelve are: nine more `alloc` sites (S7), one `E0063` missing
`capability_outcome` (S8), and three `error: lifetime may not live long enough`
on `process_dealer_{found,fill,withdraw}_v1` (S9). **Nothing in the list is a
design fault** — S6/S7/S9 are import-and-signature edits and S8 is one field.

The next three steps, concretely:

1. **Apply S6, S7 and S9** (one import move, one `extern crate alloc;` at the crate
   root, three `<'a>` signatures) and re-run the check. That should leave only S8.
2. **Answer S8 honestly**: read what a non-capability caller passes for
   `ProtocolPositionRequestV2::capability_outcome` (the Series family's founding is
   the nearest neighbour) rather than picking a number, then close the claim-unit
   hole §5 names — `found.rs` should authenticate `claim_unit_atoms` against
   `ProductBasisV3::payout_scale` before it seals the fund.
3. **Fix S5 before anything else touches `lib.rs`** — the rebound doc comment is
   invisible in a green build and silently mislabels `user_position_admission_v1`
   forever.

Not run: any Lean build, the emission guard (needs `lake`), or
`cargo check -p dclutch-trading` on its own (it compiled as a dependency and was
clean). No SBF build, no suite, no chain.

---

# ADDENDUM (lane CONVERGE-DEALER, 2026-09-06)

This document is the build wave's handoff and is left as it was written. Nine
of its claims are false or stale, and every one of them is listed with what is
actually true in **`MERGE_NOTES_DEALER.md` §7**. The four that would mislead a
reader fastest:

- the sub-band is **`0x4200 .. 0x421B`**, not `0x4100 .. 0x4118`: `0x4100` was
  already `SeriesAccountErrorV3`'s and `tools/gate census` convicted all five
  overlaps at once;
- `quoteFrame` is **eight** accounts, not six: it grew the activation cache and
  the registry program, because the price it writes is a chain fact and a
  Position had to be provably a Position;
- the emission guard is **not** unrun. `lake` is on this machine, both guards
  pass, and the checked-in Rust and TypeScript are the emitters' own bytes;
- §4.2's residue sentence is false as arithmetic (`Ŵ(0) = −Ŝ`, so `Φ = cash −
  Ŝ`). The withdraw route now lifts the floor once the Market is terminal,
  which is what makes the sentence true.

STOPPED HERE's three next steps were all taken, and its diagnosis of S9 was
wrong: rustc's suggested `<'a>` propagates into the family-neutral
`process_instruction`. The defect was `CustodyLegV1<'a>` holding one lifetime
for a slice and its account data.
