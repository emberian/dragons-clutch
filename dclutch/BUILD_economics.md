# BUILD — family `economics`

Branch `build/economics`, worktree of `/Users/ember/dev/dclutch` at `f7c03e845`.
Written incrementally as the build lands; every seam is `file:line` at this
branch's HEAD unless it says "live tree". The family: decision 0024 (CONFIRMED,
amended) — the upkeep vault built, the governable record USED, the crank-first
cost stated, C-11's row updated.

## 1. What was built, file by file

### 1.1 Lean (the one author for every layout)

| file | what |
| --- | --- |
| `formal/dclutch-semantics/DClutchSemantics/CustodyAbi.lean` | fifth PDA domain `upkeepVaultPdaDomain = "dclutch:custody-upkeep:v1"`, in `pdaDomains` so the ≤32-byte and pairwise-distinct theorems cover it |
| `formal/dclutch-semantics/DClutchSemantics/UpkeepVaultV1.lean` (new) | the vault: `SourceClass` (residue, donation, seatRent, deposit — closed, `every_decoded_source_class_is_one_of_the_four`), `Operation` (found, credit) with `debitTag` refused by name (`there_is_no_spend_instruction`), the `Vault` record, `legible`/`unreceipted`, the one transition `credit` and its laws (`credit_never_moves_the_outflow_total`, `credit_preserves_legibility`, `the_receipted_position_never_falls`, `credit_preserves_consistency`), three decided witnesses, and the three wires: record 128 B (`DCLCUPK1`), request 160 B (`DCLCUPQ1`), receipt 112 B (`DCLCUPC1`); frame widths 4/4/7 |
| `formal/dclutch-semantics/DClutchSemantics/ProtocolParametersV1.lean` | `protocolTakeAdmittedThisRelease := false` (the constitution's sixth constant), `takeAdmissible`, `Refusal.takeBeforeMainnet` checked after the bands in `propose` and `applyChange`; witnesses `a_take_before_mainnet_never_stages`, `a_take_before_mainnet_never_applies`; law `no_applied_record_takes_before_mainnet`; the request wire 112 B (`DCLTPRQ1`, `GovernanceAct` found/propose/withdraw/apply), `receiptPdaDomain = "dclutch:parameters-receipt:v1"`, frame widths 5/2/5; the record's seeds are `[domain]` (ruled, §3) |
| `formal/dclutch-semantics/DClutchSemantics/DirectSuccessor.lean` | `MakerClosePlan.upkeepCredit`; `closeMaker` pays the recorded owner exactly `rentPrincipal`, the closer `min cap donation`, the vault the rest; theorems updated, plus `the_donation_reaches_the_closer_or_the_vault_and_nobody_else` and `a_zero_cap_pays_no_closer_and_houses_the_whole_donation` |
| `formal/dclutch-semantics/EmitUpkeepVaultV1Rust.lean` (new), `EmitUpkeepVaultV1Ts.lean` (new), `EmitProtocolParametersV1Ts.lean` (new), `EmitProtocolParametersV1Rust.lean`, `EmitCustodyAbiRust.lean` | emitters; the Rust ones are guarded (`crates/dclutch-custody/check-generated-upkeep-vault.sh`, the existing parameters guard) |
| `formal/dclutch-semantics/DClutchSemantics.lean` | imports `UpkeepVaultV1` |

**Lean check (scratch copy of the warm `.lake`, `lake build` of the four touched modules):** `CustodyAbi` green; `ProtocolParametersV1` green with the new conjunct and proofs; `UpkeepVaultV1` and `DirectSuccessor` — see §7 for the state at the last check.

### 1.2 Contracts

| file | what |
| --- | --- |
| `crates/dclutch-custody/src/upkeep_vault_v1.rs` (new) | `UpkeepSourceClassV1`, `UpkeepOperationV1` (`decode(2) = Err(NoSpendRoute)`), `UpkeepVaultSeedsV1`, `UpkeepVaultV1` (genesis/decode/to_bytes/`credit`/`receipted`/`unreceipted`/`class_total`), `UpkeepRequestV1` (+`UpkeepCreditV1`, `UpkeepProtocolCallerV1`), `UpkeepCreditReceiptV1`; seven hostile tests at exact discriminants |
| `crates/dclutch-custody/src/generated_upkeep_vault_v1.rs` (new) | emitted constants and offsets |
| `crates/dclutch-custody/src/generated.rs`, `src/lib.rs` | the fifth domain, re-exported and bound-asserted |
| `crates/dclutch-market/src/protocol_parameters/mod.rs` | `Error::TakeBeforeMainnet` (+`UnknownAct`), `take_admissible`, the conjunct in `propose`/`apply_change`/`decode`; `ProtocolParametersRecordSeedsV1`, `ProtocolParametersReceiptSeedsV1`; `authenticate_protocol_parameters_account_v1` (pure: owner == Custody, key == derived, decode) |
| `crates/dclutch-market/src/protocol_parameters/request_v1.rs` (new) | `GovernanceActV1`, `ProtocolParametersRequestV1` (decode/to_bytes; a withdraw carries zeros) |
| `crates/dclutch-market/src/protocol_parameters/generated.rs` | the take constant, request width/magic/offsets, receipt domain, act tags, frame counts |
| `crates/dclutch-market/src/protocol_parameters/tests.rs` | hostile 4 (`a_take_before_mainnet_refuses_by_name`: propose, apply through a smuggled pending digest, and decode), request round trips, consumer authentication |

### 1.3 The two SBF route tables (added by CLOSEOUT-C — the maker never wrote them)

**`programs/dclutch-custody-sbf/src/upkeep_vault_v1.rs`** (new, 418 lines).
Selected by exact width **160** + magic `DCLCUPQ1`; operation byte at request
offset 10 (`0` Found, `1` Credit, `2` reserved debit — refused at decode).
A Credit's sub-discriminant is `credit.caller`: `None` → deposit, `Some(_)` → protocol.

| route | accounts | frame |
| --- | ---: | --- |
| Found | **4** | `[0 vault (w, !signer, PDA), 1 payer (signer, w), 2 system_program, 3 rent_sysvar]` |
| Credit / deposit | **4** | `[0 vault (w), 1 depositor (signer, w), 2 system_program, 3 rent_sysvar]`; class must be voluntary |
| Credit / protocol | **7** | `[0 vault (w), 1 caller_authority (signer), 2 activation_cache, 3 registry_program, 4 caller_program, 5 caller_programdata, 6 rent_sysvar]`; no signer at index ≥ 2; class must be non-voluntary |

`UpkeepVaultSbfErrorV1`, sub-band `0x6200` (ruling R7), eleven codes:
`0x6200 Instruction` · `0x6201 AccountFrame` · `0x6202 Vault` · `0x6203 Create` ·
`0x6204 NoSpendRoute` · `0x6205 SourceClass` · `0x6206 CallerAuthority` ·
`0x6207 Release` · `0x6208 Unreceipted` · `0x6209 Commit` · `0x620A Legibility`.

**`programs/dclutch-custody-sbf/src/protocol_parameters_v1.rs`** (new, 471 lines).
Selected by exact width **112** + magic `DCLTPRQ1`; act byte at offset 10
(`0` Found, `1` Propose, `2` Withdraw, `3` Apply), dispatched at `:150-153`.

| route | accounts | frame |
| --- | ---: | --- |
| Found | **5** | `[0 record (w, !signer, PDA), 1 founder (signer, w), 2 custody_programdata, 3 system_program, 4 rent_sysvar]`; founder must equal ProgramData's current upgrade authority; body must equal `genesis(authority)`; no upgrade authority ⇒ frozen record (ruling R5) |
| Propose | **2** | `[0 record (w), 1 authority (signer)]` |
| Withdraw | **2** | `[0 record (w), 1 authority (signer)]` |
| Apply | **5** | `[0 record (w), 1 receipt (w, !signer, PDA `[receipt-domain, generation LE8]`), 2 payer (signer, w), 3 system_program, 4 rent_sysvar]`; permissionless; `set_return_data(receipt)` |

`ProtocolParametersSbfErrorV1`, sub-band `0x6100`, sixteen codes:
`0x6100 Instruction` · `0x6101 AccountFrame` · `0x6102 Record` · `0x6103 Create` ·
`0x6104 FoundingAuthority` · `0x6105 GovernanceFrozen` · `0x6106 UnauthorizedGovernance` ·
`0x6107 ProposalOutstanding` · `0x6108 ParameterOutOfBand` · `0x6109 NoPendingProposal` ·
`0x610A ProposalNotMatured` · `0x610B ProposalDigestMismatch` · `0x610C TakeBeforeMainnet` ·
`0x610D Receipt` · `0x610E Commit` · `0x610F Arithmetic`.

### 1.4 The Trading close-maker widening (the last commit, undocumented by the maker)

This is where the family reaches out of Custody and into Direct, and it is where
all the hard breakage is.

- `crates/dclutch-trading/src/close_maker_v1.rs` — the receipt widens **248 → 288**
  with `closer: [u8;32]` inserted **mid-record at offset 208** and
  `upkeep_credit: u64` at 264, shifting principal/donation/carve/total/remaining
  to 240/248/256/272/280. The frame widens **22 → 27**: coordinate 22 parameters
  record (RO), 23 upkeep vault (W), 24 closer (W, signer), 25 Custody program (X),
  26 caller authority (RO). `direct_close_maker_account_privileges_v1` becomes
  5 writable / 4 executable. **`DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1` was deleted.**
- `crates/dclutch-trading/src/successor.rs` — `MakerReplayClosePlanV2` gains
  `upkeep_credit`; `close_maker_replay_v2`'s fourth parameter changes from
  `closer_reward_cap: u64` to `parameters: ProtocolParametersV1` (`:2548`). The
  carve becomes `parameters.closer_carve(unclassified_donation)`, `total_credit`
  becomes exactly `maker_root.rent_principal`, and the remainder becomes
  `upkeep_credit` — ruling R3, in code.
- `programs/dclutch-trading-sbf/src/lib.rs` — three refusals
  `CloseMakerParameters = 0x402C`, `CloseMakerUpkeepVault = 0x402D`,
  `CloseMakerCloser = 0x402E`; `TradingSbfError::ALL` 44 → 47. **No adapter code
  raises any of them yet** (§2.2, S8).

## 2. THE SEAMS

Three things the convergence lane should know before reading further.
**First: the Custody half is complete and self-wired** — every `mod` is declared,
both dispatch arms are in `process_instruction`, and `cargo check -p dclutch-custody`
is green. **Second: the Trading half is hard-broken** in four crates that no check
on this branch touched. **Third: nothing here is annotated** — there is not one
`todo!()` on the branch (§4).

### 2.1 Already wired — no seam (recorded so nobody re-derives it)

- All five new `.rs` files are declared: `crates/dclutch-custody/src/lib.rs:28,29`;
  `crates/dclutch-market/src/protocol_parameters/mod.rs:68`;
  `programs/dclutch-custody-sbf/src/lib.rs:26,29`.
- **The dispatcher is wired**: `programs/dclutch-custody-sbf/src/lib.rs:239-247`
  routes both families *before* `split_caller_authority_bump_v1` (`:248`), each on
  exact width + magic. Neither width (160, 112) collides with the four existing
  Custody request widths.
- No Cargo manifest change is owed anywhere on the branch — every dep the new
  code uses is already listed in all five touched crates.
- `crates/dclutch-vm/check-generated-account-profile.sh` and its request-profile
  sibling guard the VM's *agreement* profile ABI, not per-route account widths.
  Neither new account is VM-profiled. **Not a seam.**

One assertion is owed at the dispatcher, though:
**S0 — `programs/dclutch-custody-sbf/src/lib.rs:247`** — add a
`const _: () = assert!(...)` binding the two new widths disjoint from
`CUSTODY_REQUEST_BYTES_V1` / `PROJECTED_CUSTODY_REQUEST_BYTES_V1` /
`DELEGATED_CUSTODY_REQUEST_BYTES_V2` / `RETIREMENT_REPLAY_HANDOFF_REQUEST_BYTES_V1`.
The selection is correct today by inspection; nothing holds it correct.

### 2.2 HARD BREAKS — the Trading close-maker, four crates, none of them checked

The deleted constant and the 22 → 27 frame are the whole of it.

| # | file:line | the one-line change |
| --- | --- | --- |
| S1 | `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs:567` | Passes `DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1` (deleted) as the 4th arg to `close_maker_replay_v2`. Both the name **and** the type are wrong: it now takes a `ProtocolParametersV1` read from frame coordinate 22. |
| S2 | `crates/dclutch-operator/src/direct_close_maker_v1.rs:77` | `use` of the deleted constant. |
| S3 | `crates/dclutch-operator/src/direct_close_maker_v1.rs:1197` | The same 4th-arg call site. |
| S4 | `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs:41` | Stale doc reference to the deleted constant. |
| S5 | `crates/dclutch-operator/src/direct_close_maker_v1.rs:218-241` | `DIRECT_CLOSE_MAKER_META_CLASSES_V1` is a 22-element literal typed `[…; DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1]`, now 27 — array-length mismatch. Five entries owed: 22 `LookupStable` (one record per deployment), 23 `LookupStable` (one vault), 24 `InlineSigner` (the closer), 25 `InlineProgram` (Custody), 26 `InlineRequestBound` (caller authority, from the request digest). |
| S6 | `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs:105-127` | The adapter `Accounts` struct is still the 22-field frame; `:136` now demands 27 accounts while only 22 are bound. Five fields owed. |
| S7 | `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs:135` | `accounts.iter().any(|account| account.is_signer)` **unconditionally refuses any signer** — which makes the new closer at coordinate 24 unreachable by construction. The rule needs a named exemption, not a relaxation. |
| S8 | `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs:432-442`, `:481-489` | The receipt is built with no `closer` and no `upkeep_credit` (struct-literal missing-field errors), and `total_credit` still goes to the rent owner with nothing debiting the vault leg. **The vault CPI does not exist**, which is why the three new `0x402C/D/E` refusals have zero call sites. |
| S9 | `crates/dclutch-operator/src/direct_close_maker_v1.rs:1227-1237`, `:1305-1313`, `:1290`, `:534-536`, `:697`, `:1534`, `:1664`, `:1909` | Receipts built without the two new fields; `total_credit` summed under the old conservation identity; the report struct needs `upkeep_credit`; `[&ObservedAccount; 27]` returned from a 22-account gather; `capability_manifest: observed(22)` now collides with the record's coordinate; two stale count assertions. |
| S10 | `tools/local-validator/bootstrap/successor/src/direct_close_maker.rs:766`, `:850-851`, `:921-922` | The key-length check demands 27 from a 22-key builder; the printer emits carve + total and neither `upkeepCredit` nor `closer`. |
| S11 | `programs/dclutch-trading-sbf/program-test/tests/direct_close_maker_on_chain.rs:740`, `:1028`, `:1225`, `:1383-1386` | Stale counts, and `assert_eq!(receipt.total_credit, case.clean.lamports)` at `:1028` is now **false by design** — `total_credit` is the principal alone. |
| S12 | `programs/dclutch-trading-sbf/Cargo.toml:47` | `dclutch-custody` is `optional = true`, enabled only by `series-family`/`dealer-family`/`outer-only`. Close-maker is a **default** Direct route; a vault CPI naming `dclutch_custody::upkeep_vault_v1` compiles only when a family feature happens to be on. Drop `optional`, or add `dep:dclutch-custody` to an unconditional feature in `default`. |
| S13 | `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs` (new code) | Two authentications ruling R1/R5 imply and nobody wrote: the Custody program at coordinate 25 must be the one the release set names (the frame already carries `cache` at 13 and `registry` at 18, so it belongs beside the existing release check), and the caller authority at 26 must be `CallerAuthoritySeedsV1` over *this* request's digest — exactly what `programs/dclutch-custody-sbf/src/upkeep_vault_v1.rs:318-333` re-derives. |
| S14 | `docs/evidence/witnesses/cohort-17-discovered.json:232,255,604,627` | Four `DCLTDMC1` witnesses pin the 248-byte receipt. The new field is inserted **mid-record**, so every one of those digests is stale. |

### 2.3 Emission guards

| # | file:line | the one-line change |
| --- | --- | --- |
| S15 | `tools/gates/emission-coverage.md:23`, `:99`, `:4` | The new `check-generated-upkeep-vault.sh` **is** auto-discovered (`tools/gates/emission.py:107-118` scans tracked `*check*.sh`), so the guard runs — but the census file is byte-gated and stale. One guard row before `:23`, one generated-file row after the `generated.rs` row at `:99`, and the `:4` summary 91/85 → 92/86. Run `tools/gate emission --write`; do not hand-edit. |
| S16 | `packages/dclutch-sdk/package.json:73` | **`EmitUpkeepVaultV1Ts.lean` has no guard and no target.** It prints an `abi:upkeep-vault` module and neither `packages/dclutch-sdk/lib/generated/upkeepVaultV1.ts` nor a package script exists. Add the `abi:upkeep-vault` / `abi:upkeep-vault:verify` pair (`node scripts/lean-emit.mjs DClutchSemantics.UpkeepVaultV1 EmitUpkeepVaultV1Ts.lean lib/generated/upkeepVaultV1.ts [--check] ts`). Only the `--check` form registers as a guard (`tools/gates/emission.py:126-129`). |
| S17 | `packages/dclutch-sdk/package.json:73` | The same for `EmitProtocolParametersV1Ts.lean` → `lib/generated/protocolParametersV1.ts`. Each new `lib/generated/*.ts` also produces a `docs/reference/abi/<name>.md` page and a README row via `tools/genref/generate.mjs:1720-1746` — regenerate with `tools/gate reference --converge`. |
| S18 | `crates/dclutch-market/check-generated-protocol-parameters.sh` | Already listed at `tools/gates/emission-coverage.md:30`, so no census row is owed — but its `grep -q` pins should widen to the new constants (the take flag, the request width/magic). |

`EmitCustodyAbiRust.lean`'s new domain is already covered by
`programs/dclutch-custody-sbf/check-generated.sh:37`. No seam.

### 2.4 Registries

| # | file:line | the one-line change |
| --- | --- | --- |
| S19 | `docs/reference/routes.md:220` | Two **entry** rows in the `## custody` block (sorted after `custody/projected::process`, before `custody/retirement_replay_handoff_v1::process`) for `custody/protocol_parameters_v1::process` and `custody/upkeep_vault_v1::process`, plus **action** rows for the six sub-routes. `@generated` — regenerate with `tools/gate reference --converge`, do not type. |
| S20 | `docs/reference/refusals.md:349` | **27 rows.** Sixteen `0x6100`–`0x610F` and eleven `0x6200`–`0x620A` after the custody table's current end at `0x6011`, plus `0x402C`/`0x402D`/`0x402E` in the Trading table. Same regeneration. |
| S21 | `tools/gauntlet/blocked.json` (after `:76`) | No tier drives either new family, so both entry routes and all six actions land in the census's "NO stated reason at all" class. Add entries with `route`, `class` (`status-report` is the honest class — nothing structural blocks them), `reason`, `owner`. **`tools/genref/generate.mjs` refuses the file if any entry lacks a known class.** |
| S22 | `docs/reference/programs.md` | The `dclutch-custody-sbf` entry-route and action counts move by +2 and +6. |
| S23 | `docs/reference/route-witnesses.md:165-166` | The tally lines are cross-checked against `routes.md` (`tools/genref/generate.mjs:1194-1204` asserts witnessed/blocked/unrecorded agree). Moves with S19. |
| S24 | `packages/dclutch-sdk/lib/generated/routeCensus.ts` + `docs/reference/abi/routeCensus.md` | `DCLTPRQ1` and `DCLCUPQ1` are absent from both machine-generated inventories. Regenerate. |
| S25 | `tools/cohort/steps.tsv` | **Ruling R2 says "the runbook founds it right after deploy (row `upkeep-found`)" and that row does not exist.** Two `once`-shape rows are owed (`upkeep-found`, `parameters-found`), and every lifecycle row that now touches the vault must list them in `blocks`. Gated by `check-steps.py --cohort N`. This is the one place the doc's own prose promises something the branch did not build. |
| S26 | `tools/gauntlet/CU_BUDGETS.json` | No `custody/upkeep_vault_v1` or `custody/protocol_parameters_v1` stage ids. A pinned budget row is owed per driven route — but `tools/gate budgets` refuses a campaign no bindings file names, so **campaign first (S21/S25), budget second.** |
| S27 | `tools/gauntlet/magic-collisions.json` | `DCLTPRQ1`, `DCLCUPQ1`, `DCLCUPK1`, `DCLCUPC1` are new; no other holder was found in the tree, so `--check-unique` should pass. Verify rather than assume; a genuine collision needs an argued `verdict`, not a mute. |
| S28 | `tools/gates/wire-vector-pins.tsv` | No pin for either wire. Only owed once S16/S17 land a two-sided Rust↔TS fixture. |
| S29 | `tools/gates/root-targets.tsv` | Nothing owed — the branch adds no root-workspace integration target. |

### 2.5 Orphans — defined here, called by nothing

The pattern is uniform and it names the family's real gap: **there is no client.**
Nothing in `crates/dclutch-operator/`, `tools/dclutch-cli/`,
`tools/local-validator/bootstrap/` or `packages/dclutch-sdk/` constructs either
request, which is why S25's runbook rows have nothing to call.

- `ProtocolParametersRequestV1::to_bytes` and `UpkeepRequestV1::to_bytes` — no
  off-chain builder anywhere.
- `authenticate_protocol_parameters_account_v1`
  (`crates/dclutch-market/src/protocol_parameters/mod.rs:769`) — only its own test
  calls it. Its intended consumer is the Trading close-maker read at coordinate 22
  (S8/S13). A textbook producer-missing reader.
- `ProtocolParametersV1::take_admissible` (`…/mod.rs:232`) — called only inside its
  own file.
- `UpkeepSourceClassV1::label` and `::ALL`
  (`crates/dclutch-custody/src/upkeep_vault_v1.rs:111,140`) — no census or CLI reader.
- `UpkeepRequestV1::digest` (`…/upkeep_vault_v1.rs:611`) — unused: the adapter
  hashes `instruction_data` directly at
  `programs/dclutch-custody-sbf/src/upkeep_vault_v1.rs:121`. **Two authors for one
  digest — pick one** before a client is written against the wrong one.

## 3. Ruled provisionally (ember rules by reversal)

R1. **The vault's address is `[dclutch:custody-upkeep:v1]` under the Custody program: one vault per Custody deployment, no market in the seeds.** 0024 says "one protocol-owned lamport PDA"; the protocol's identity on a chain is its release set's Custody program.
R2. **The vault is FOUNDED, once, by anyone, funded to exactly its own rent minimum; a credit against an unfounded vault refuses by name.** The runbook founds it right after deploy (row `upkeep-found`), so no lifecycle route ever meets an unfounded vault on a cohort that ran the runbook. The rent minimum is the account's own and is NOT an inflow (I3 subtracts it).
R3. **The donation remainder after the closer's carve goes to the vault, and the recorded `rent_owner` receives exactly their principal.** 0024 item 5 says the carve touches only the donation slice and never the owner's entitlement; the owner's entitlement is `rent_principal` (COHORT9 rulings); UPKEEP_VAULT_V0 §3 routes the slice to the vault. On every cohort to date the slice was zero, so the change moves no measured money.
R4. **The parameters record's seeds are `[dclutch:protocol-parameters:v1]` (one record); receipts are `[dclutch:parameters-receipt:v1, generation LE8]`.** The Lean doc's "`[domain, generation]`" is read as describing the receipt stream, not the record: a consumer must find the record without knowing its generation.
R5. **The parameters record is founded only by the Custody program's current upgrade authority (read from its ProgramData); an immutable program founds a FROZEN record (authority zero).** Decision 0024 §3: "today its authority is the deployer key, named as a placeholder" — the upgrade authority IS that key, and reading it off ProgramData makes the founding race-free.
R6. **"No protocol take before mainnet" is a source constant of the release (`protocolTakeAdmittedThisRelease = false`) refused by its own name, not a record field.** "Before mainnet" is a fact about which ELF is deployed; the mainnet ruling's release (decision 0026) flips it.
R7. **Both the upkeep vault and the parameters record live in the Custody program, in sub-bands `0x6200` and `0x6100` of Custody's band 6.** One economics program; no new band (0007 bands are append-only and a program is a band).
R8. **No outflow route is built.** 0024 charters "no spend instruction"; UPKEEP_VAULT_V0 §4's candidates are unruled prices. `outflow_total` is a running number the census holds at zero; the first ruled outflow route carries its own Rent-derived price and its own decision.
R9. **Unreceipted lamports (a stray System transfer into the PDA) are a census class held to a declared delta, not a refusal and not silently receipted.** Anyone may credit any account; the vault cannot refuse them; only a signer's `deposit` credit or a protocol credit receipts them.

R10. **(CLOSEOUT, provisional.)** Three `rustfmt` reorderings will move code the
moment anyone runs `tools/lane.sh fmt`, and a convergence lane should do it
deliberately rather than discover it in a diff:
`crates/dclutch-custody/src/lib.rs:27-28` (the two new `mod`s are declared after
`retirement_replay_handoff_v1`; `reorder_modules` sorts them after
`generated_projected_state_v2` at `:22`),
`crates/dclutch-trading/src/successor.rs:16` (the `dclutch_market` import sits
above three `use crate::` lines; `reorder_imports` puts `crate` first), and
`crates/dclutch-market/src/protocol_parameters/mod.rs:74-81` (the new names were
appended inside a brace group rustfmt re-sorts whole).
Also `formal/dclutch-semantics/DClutchSemantics.lean:98` — the new import sits
between `ProtocolParametersV1` and `RationalCrossDomainV3` where every other line
in that file is alphabetical.

## 4. Stubbed, and why

**Nothing is marked.** The only grep hit across the branch is the `XXXXXX` of a
`mktemp` template at `crates/dclutch-custody/check-generated-upkeep-vault.sh:5`.

The honest statement is the one ruling R8 already makes and the code keeps:
**no outflow route was built, on purpose.** Decision 0024 charters "no spend
instruction", and `UpkeepOperationV1::decode(2)` returns `Err(NoSpendRoute)` —
the absence is enforced by name (`there_is_no_spend_instruction`), not left as a
hole. `outflow_total` is a running number the census holds at zero. That is a
stub in the good sense: a refusal where the unwritten thing would go.

Everything else missing on this branch is missing by **absence** — §2.2's four
crates of hard breakage and §2.5's client that was never started. No marker
points at any of it.

## 5. Tests written and what each proves

Fifteen tests, all on the Custody side. **Zero on the Trading side of §1.4, and
no SBF program-test or gauntlet campaign for either new family** — the two entry
routes and six action routes are untested on a bank and unwitnessed by any
binding (which is exactly what S21 must declare).

**`crates/dclutch-custody/src/upkeep_vault_v1.rs`** (8, from `:791`)

| test | proves |
| --- | --- |
| `genesis_round_trips_and_is_legible_at_its_rent` (`:828`) | ruling R2: a founded vault is legible at exactly its own rent minimum, and the rent is not an inflow. |
| `there_is_no_spend_instruction_and_it_says_so` (`:845`) | ruling R8 at the wire: discriminant 2 refuses by name, not by falling through. |
| `the_source_classes_are_closed_and_shaped` (`:859`) | the four classes are exhaustive — the Lean's `every_decoded_source_class_is_one_of_the_four` in Rust. |
| `requests_round_trip_and_refuse_reserved_bytes` (`:889`) | the 160-byte wire is exact and hostile bytes are refused. |
| `a_credit_moves_exactly_the_amount_and_nothing_else` (`:914`) | the Lean's `credit_adds_exactly_the_amount_to_the_total` + `credit_never_moves_the_outflow_total`. |
| `a_record_whose_totals_do_not_close_refuses_on_decode` (`:948`) | consistency is checked at decode, so an inconsistent vault cannot be read at all. |
| `receipts_round_trip_and_refuse_a_credit_larger_than_the_total` (`:964`) | the 112-byte receipt, and that a receipt cannot claim more than the vault holds. |
| `the_seeds_are_the_domain_alone` (`:987`) | **ruling R1** — one vault per Custody deployment, no market in the seeds. |

**`programs/dclutch-custody-sbf/src/upkeep_vault_v1.rs`** (2): `the_sub_band_starts_where_custody_reserved_it` (`:398`) proves ruling R7's `0x6200`;
`a_spend_is_refused_by_name_on_the_wire` (`:409`) proves R8 survives the adapter.

**`programs/dclutch-custody-sbf/src/protocol_parameters_v1.rs`** (1):
`the_sub_band_starts_where_custody_reserved_it_and_names_the_take` (`:458`) — `0x6100` and `0x610C`.

**`crates/dclutch-market/src/protocol_parameters/tests.rs`** (3 new of 15):
`a_take_before_mainnet_refuses_by_name` (`:426`) proves **ruling R6** three ways —
on `propose`, on `apply_change` through a smuggled pending digest, and on
`decode`, so no path reaches a taking record;
`governance_requests_round_trip_and_a_withdraw_carries_nothing` (`:488`);
`a_consumer_reads_a_parameter_only_out_of_the_custody_owned_record_at_its_address`
(`:534`) proves **ruling R4** — a consumer finds the record without knowing its
generation.

**`crates/dclutch-trading/src/close_maker_v1.rs`** (1 rewritten, 1 extended):
`the_frame_carries_the_record_the_vault_and_a_closer` (`:717`, replacing
`the_carve_ceiling_is_zero_because_the_frame_admits_no_closer` — the old test
asserted the very absence this branch removes);
`a_receipt_carving_more_than_the_donation_refuses` (`:655`) gains the vault-era bound.

**`crates/dclutch-trading/src/successor.rs`**: no new `#[test]`;
`the_closer_carve_comes_out_of_the_donation_and_never_the_principal` (`:3662`) was
rewritten around the new `carve_cap` helper (`:3646`) with a fifth "halved" case —
that is **ruling R3**, the one claim on this branch that moves real money.

## 6. Frames and codes expected to move, and which cohort carries them

- **Custody gains two entry routes and six actions**, sub-bands `0x6100` and
  `0x6200`, 27 new refusal codes. New program, new ELF: the cohort that redeploys
  Custody carries them.
- **Trading gains three codes** (`0x402C`–`0x402E`) and `ALL` 44 → 47.
- **The Direct close-maker frame moves 22 → 27 and its receipt 248 → 288**, with
  a field inserted mid-record. Every `DCLTDMC1` digest moves; the four cohort-17
  witnesses at `docs/evidence/witnesses/cohort-17-discovered.json` are stale
  (S14). This is a Trading ELF redeploy plus a witness re-capture, and it is the
  expensive half of the family.
- **Nothing moves for a market already open.** The vault and the record are new
  accounts founded once per deployment (rulings R1, R2, R4); no existing account
  changes shape. On every cohort to date the donation slice was zero, so ruling
  R3 moves no measured money — it changes where a future nonzero slice goes.

## 7. Build/check state

`cargo check -p dclutch-custody --offline` (CLOSEOUT-C, shared target dir):
**GREEN, exit 0, zero errors.** The Lean-emitted constants, the 993-line vault
kernel and the fifth PDA domain all compile.

**That green covers one crate of six the family touches, and it is the crate
least likely to be wrong.** Not checked, and per §2.2 several are certainly red:
`dclutch-market` (the parameters record), `dclutch-custody-sbf` (889 lines of new
route code), `dclutch-trading`, `dclutch-operator`, `dclutch-trading-sbf`, and
`tools/local-validator`. The last four contain S1–S11 — a deleted constant with
three live call sites, a 22-element array typed at 27, a 22-field struct behind a
27-account check, and a signer rule that forbids the signer the frame just added.
**Do not read the green as "economics compiles."** It means the vault kernel does.

The maker's Lean note (§1.1) says `CustodyAbi` and `ProtocolParametersV1` were
green and that `UpkeepVaultV1` and `DirectSuccessor` were still being checked when
the quota ran out; no `lake` run happened in this worktree under CLOSEOUT, so
those two remain unconfirmed here.

## 8. STOPPED HERE — the next three steps

The maker built the Custody half to completion — kernel, two adapters, 27 codes,
15 tests, dispatcher wired, contract crate green — and then began the reach into
Trading and stopped mid-reach. The branch's shape is therefore unusual and worth
saying out loud: **the new program is finished and the old program is broken.**

1. **Repair the Direct close-maker (S1–S11), in dependency order.** Start at
   `crates/dclutch-trading/src/close_maker_v1.rs` (already correct) and work
   outward: the operator's meta-class table (S5) and call sites (S2, S3, S9), then
   the SBF adapter's struct, count check and signer rule (S6, S7), then the
   local-validator builder (S10) and the on-chain test's assertions (S11). Then
   `cargo check --workspace`, which is the first check this family will ever have
   had. Do S12 (the optional-dep question) before writing the CPI, not after —
   it decides whether the CPI can exist at all.
2. **Write the vault CPI leg and its two authentications (S8, S13).** This is the
   only step that makes the family *do* anything: today the donation remainder is
   computed by `successor.rs` and delivered nowhere, `authenticate_protocol_parameters_account_v1`
   has no caller, and the three `0x402C/D/E` refusals cannot be raised. Ruling R3
   is proved in Lean and in a unit test and is not yet true on a chain.
3. **Give the family a client, then a runbook row (S25).** Nothing constructs
   either request; that is why R2's promised `upkeep-found` row does not exist.
   Pick the digest author first (§2.5's last bullet — `UpkeepRequestV1::digest`
   or the adapter's direct hash, not both), then the two `to_bytes` builders in
   the operator, then the two `steps.tsv` rows, then S21's `blocked.json` entries
   so the census stops calling the routes unrecorded.

Deferred deliberately: S15–S18 (the emission census and the two TS emitters with
no target) and S19–S24 (the generated reference pages). All of them are
`tools/gate … --write` regenerations that should run **once**, after steps 1–3
have settled the routes and codes — regenerating now would just pin a census of
a family that is about to change shape. S26 in particular must wait: the budget
gate refuses a campaign no bindings file names, so the campaign has to exist first.
