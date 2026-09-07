# MERGE NOTES — family `economics`

Branch `build/economics`, rebased onto `main` at `9b8026334` (clean rebase, no
conflicts; `git rebase main`, not a merge). Decision 0024's row, C-11.

## 1. READY or NOT

**READY** — with one ratchet left red on purpose and named in §2.

### Green, and how it was measured

`CARGO_TARGET_DIR` was a private scratch dir, never shared, `CARGO_INCREMENTAL=0`.

| package | `cargo check --offline --tests` | filtered tests run |
| --- | --- | --- |
| `dclutch-custody` | green | 9 pass (`--lib upkeep_vault`) |
| `dclutch-market` | green | 15 pass (`--lib protocol_parameters`) |
| `dclutch-trading` | green | 11 pass (`--lib close_maker`), 27 pass (`--lib successor::`) |
| `dclutch-custody-sbf` | green | 2 + 1 pass (the two sub-band tests) |
| `dclutch-trading-sbf` | green | no tests in the touched module |
| `dclutch-operator` | green | 14 pass (`--lib direct_close_maker_v1`) |
| `dclutch-claims` | green | — (one constant projected, no behaviour) |
| `dclutch-devnet-scenarios` | green | — |
| `dclutch-journey-campaign` | green | — |
| `dclutch-local-successor-bootstrap` | green | 7 pass (`--bin … close_maker`) |
| `dclutch-trading-program-test` | green | **not run**: needs SBF ELFs (§6) |
| `packages/dclutch-sdk` | — | 16 pass (`vitest run lib/openerTerms.test.ts`) |
| `apps/dclutch-web` | — | `tsc --noEmit` clean |

**`cargo check --offline --workspace --tests` was run, once, over the whole
tree** — the check this family had never had. It is green except for ONE package
that this branch does not touch by a single line:
`dclutch-wallet-terminal-input-wasm`, six times
`error[E0433]: cannot find 'tests' in 'wire'`. The cause is a manifest, not
code: `crates/dclutch-wallet-terminal-input-wasm/Cargo.toml`'s dev-dependency
reads `dclutch-operator = { path = "../dclutch-operator", default-features =
false }` under a comment that says "The shared fixture, not a second copy of
one", but the fixture is `#[cfg(any(test, feature = "test-fixtures"))] pub mod
tests` at `crates/dclutch-operator/src/wallet_terminal_payout/wire.rs:1371`, and
nothing enables `test-fixtures` (declared at
`crates/dclutch-operator/Cargo.toml:27`). **The fix is one line** — add
`features = ["test-fixtures"]` to that dev-dependency — and it is left for
whoever owns that row: no build branch touches the file, `git diff main...HEAD`
touches neither it, the operator's manifest, nor `wallet_terminal_payout/`, and
`cargo check -p dclutch-wallet-terminal-input-wasm --tests` is red standalone,
so it is red on `main` and not this family's.

`tools/gate census` (`dclutch-route-census inventory --check-unique`) passes:
**154 routes, 449 refusal codes across 84 packages against 22 registered bands,
372 magics / 358 distinct, 5 collisions adjudicated and no sixth.** The four new
magics (`DCLTPRQ1`, `DCLCUPQ1`, `DCLCUPK1`, `DCLCUPC1`) plus the two record/
receipt magics collide with nothing, so **BUILD `S27` is discharged: no
`magic-collisions.json` entry is owed.**

### Not green, deliberately

`tools/gate emission` FAILS with `emission-coverage.md no longer describes this
tree`. That file is SHARED (§2). The gate's own measurement is
`generated=92 guarded=92 unguarded=0 guards=72` against the committed file's
stated `91 generated from 85 emitters`. **Nothing is unguarded** — the new
`crates/dclutch-custody/check-generated-upkeep-vault.sh` is auto-discovered by
`tools/gates/emission.py:107-118` — only the census file is stale.

**No Lean was built in this lane, and no emission guard was RUN.** The three
generated Rust files this family adds or changes
(`crates/dclutch-custody/src/generated_upkeep_vault_v1.rs`,
`crates/dclutch-market/src/protocol_parameters/generated.rs`,
`crates/dclutch-custody/src/generated.rs`) carry what the build wave's `lake`
run printed; this lane re-ran neither `crates/dclutch-custody/check-generated-upkeep-vault.sh`
nor `crates/dclutch-market/check-generated-protocol-parameters.sh`. There was no
warm `.lake` on this machine (a peer lane reclaimed the one that existed for
disk) and a cold Lean build was not affordable against the disk budget. **The
tree's law is that the guard must actually pass, so treat these three files as
unproved-in-this-lane and run `tools/gate guards` — filtered to those two
scripts — before the merge.** Everything downstream of them is proved: the Rust
constants they define are read by 78 passing tests in six crates.

## 2. SHARED FILES — for the merge lane

Each line is the whole change. The first three are shared with another build
branch (`git diff main...build/<other>` shows them touched), so this lane did not
fight over them.

1. `tools/gates/emission-coverage.md` (shared with `build/general-lifecycle`) —
   run `tools/gate emission --write` once after the merge; do not hand-edit.
   Expect the `:4` summary to become **92 generated / 92 guarded, 0 unguarded**,
   a guard row for `crates/dclutch-custody/check-generated-upkeep-vault.sh`, and
   a generated-file row for
   `crates/dclutch-custody/src/generated_upkeep_vault_v1.rs`.
2. `tools/gauntlet/blocked.json` (shared with `build/series`) — after `:76`, one
   entry per new route with `class: "status-report"`, an argued `reason` and an
   `owner`: entry routes `custody/protocol_parameters_v1::process` and
   `custody/upkeep_vault_v1::process`, plus the six actions (found / propose /
   withdraw / apply; found / credit). Nothing structural blocks them — no tier
   drives them yet. `tools/genref/generate.mjs` refuses an entry with no known
   class.
3. `tools/cohort/steps.tsv` (shared with `build/claims-split-merge`,
   `build/failure-arm`, `build/founder-bond`, `build/series`) — two `once`-shape
   rows, `upkeep-found` and `parameters-found`, immediately after the deploy
   rows, and every lifecycle row that reaches the Direct close-maker lists both
   in `blocks`. Gated by `check-steps.py --cohort N`. Both rows now have a
   callee (`crates/dclutch-operator/src/{upkeep_vault_v1,protocol_parameters_v1}.rs`),
   which they did not when the BUILD doc's ruling R2 promised them. The existing
   `close-maker` row also gains `--closer ADDRESS` on its dry-run invocation
   (§6).
4. `docs/reference/routes.md`, `refusals.md`, `programs.md`,
   `route-witnesses.md`, `abi/*` and
   `packages/dclutch-sdk/lib/generated/routeCensus.ts` and
   `packages/dclutch-sdk/lib/generated/refusalRegistryV1.ts` — all `@generated`.
   (Those two are the ONLY browser mirrors of this family's wires; no
   hand-written TypeScript decodes a `DCLTDMC1` receipt, so nothing in the
   browser silently becomes the receipt's last authority when it widens.) Run
   `tools/gate reference --converge` ONCE from a detached worktree at HEAD
   (it refuses a dirty tree, exit 2). Two entry routes, six actions, and
   **thirty** new refusal codes: sixteen `0x6100`–`0x610F`, eleven
   `0x6200`–`0x620A`, three `0x402C`–`0x402E`.
5. `tools/gates/frames-baseline.json` — **THE RATCHET IS LEFT RED.** Every
   `dclutch_trading_sbf::direct_close_maker_v1::*` row (from `:6372`) is stale:
   the frame went 22 → 27 accounts and the route gained a CPI. Capture with
   `tools/gate frames --at <commit> --capture <file>` in the merge commit. This
   lane is not permitted to build SBF, so it could not.
6. `packages/dclutch-sdk/package.json` — add two script pairs, and generate the
   files they check, in the same pass:
   `"abi:upkeep-vault": "node scripts/lean-emit.mjs DClutchSemantics.UpkeepVaultV1 EmitUpkeepVaultV1Ts.lean lib/generated/upkeepVaultV1.ts ts"` plus its `:verify` with `--check`, and the same for
   `DClutchSemantics.ProtocolParametersV1 EmitProtocolParametersV1Ts.lean lib/generated/protocolParametersV1.ts`.
   Only the `--check` form registers as a guard
   (`tools/gates/emission.py:126-129`). **Deferred because it needs `lake`**, and
   because landing the script without the file would put a red guard in the tree.
   Both `.lean` emitters exist and are unreferenced today; that is a real
   producer-missing gap, not a formality.
7. `tools/gauntlet/CU_BUDGETS.json` — a pinned budget row per driven route, but
   **campaign first**: `tools/gate budgets` refuses a campaign no bindings file
   names, so this follows item 2.
8. `docs/evidence/witnesses/cohort-17-discovered.json` — **do not edit.** Four
   `DCLTDMC1` witnesses pin the 248-byte receipt and they are TRUE about what ran
   on devnet on 2026-09-05. The receipt is now 288 bytes with a field inserted
   mid-record, so they are stale relative to the next ELF and the re-capture
   belongs to the cohort that redeploys Trading. An evidence document is dated
   and never edited.
9. `apps/dclutch-web/lib/generated/capabilitySurfaceV1.ts` — **already stale on
   `main`, and not this family's doing.** `node
   scripts/generate-capability-surface.mjs --check` fails at `main` too: the
   missing row is `@dclutch/sdk/shadowDigestV3` on `/general`, added by
   `7f9d391a4` (on `main`) without regenerating the surface. This lane checked,
   confirmed the drift is unrelated to anything it touched, regenerated to see
   the diff, and then REVERTED so the merge lane is not handed someone else's
   fix inside this family's diff. Run `npm run abi:capability-surface` once.
10. `tools/gauntlet/journey/src/ledger.rs` (shared with `build/founder-bond`) —
   nothing owed. The vault is watched from `stages.rs` (this lane's file) through
   the existing `ledger.watch(label, address)`; no ledger change is needed, and
   the vault is deliberately NOT a tenth `CompartmentV1` (see §3, R12).

## 3. What this lane completed, and what it ruled

The build wave left the Custody half finished and the Trading half hard-broken in
four crates nothing had checked. Both halves are now real.

### The four Custody route tables compile

`programs/dclutch-custody-sbf/src/{upkeep_vault_v1,protocol_parameters_v1}.rs`
had **ten `lifetime may not live long enough` errors** — six functions took
`accounts: &[AccountInfo<'_>]` beside a `&AccountInfo<'_>` drawn from that same
slice, and `AccountInfo<'a>` is invariant in `'a`. Named `'info` on `found`,
`propose`, `withdraw`, `apply`, `found` and `credit_route`. Nothing else was
wrong with 889 lines of new route code.

### The Direct close-maker reaches Custody, end to end

- `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs` — five new frame
  bindings; the signer rule became a per-index equality that exempts coordinate
  24 by name (`CloseMakerCloser`) instead of refusing every signer;
  `authenticate_economics` holds the vault, the closer and the governed record to
  their own addresses before a lamport moves; `commit` splits the replay three
  ways with the sum checked against the observed balance first;
  `receipt_upkeep_credit` builds the `UpkeepRequestV1`, derives the
  caller-authority PDA **from those exact request bytes**, and `invoke_signed`s
  into Custody. The close's own answer is `set_return_data` AFTER the CPI,
  because the CPI overwrites return data.
- `reauthenticate_roles` gained the conjunct BUILD `S13` said was owed: the
  Custody program at coordinate 25 must be the one this release set's activation
  cache names for `ExecutionRoleV1::Custody`.
- `programs/dclutch-trading-sbf/Cargo.toml` — `dclutch-custody` is no longer
  `optional`. A default Direct route now names the vault ABI (BUILD `S12`).
- `crates/dclutch-operator/src/direct_close_maker_v1.rs` — the whole planner
  widened: meta classes, coordinate input, snapshot, `frame_accounts`, the
  submission report, and five new refusals that each name one conjunct.
- `crates/dclutch-trading/src/successor.rs` — one stale assertion
  (`total_credit == 111`) was **red before it was fixed**; it now states the
  three-way split.

### The record is READ, and the constants have one author

The governed record has exactly one runtime consumer — the closer's carve, read
out of the frame at coordinate 22 by both the chain and the planner. The other
economic constants could not become runtime reads without widening frames in two
more programs, so what changed is AUTHORSHIP, and that is said at each site:

- `crates/dclutch-claims/src/claim_check_v1.rs` —
  `COMPACTION_CRANK_REWARD_LAMPORTS_V1` projects
  `PROTOCOL_GENESIS_CRANK_REWARD_CAP_LAMPORTS_V1` instead of writing `200_000`.
- `tools/devnet-scenarios/src/model.rs`,
  `tools/local-validator/.../direct_trade_producer.rs` — both `50`s project
  `DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1`, the module that chooses the rate.
- `packages/dclutch-sdk/lib/openerTerms.test.ts` — the source gate stopped
  pinning the literal (which would now pass by finding nothing) and pins the
  CHAIN: the record's generated genesis carries the number and the claims
  constant projects it by name.

**Still a source literal, and named as such**: the Direct fee band
(`successor.rs:424-426`, `token_setup_v1.rs:288-290`) and the crank cap's four
injection sites in `dclutch-claims`/`dclutch-claims-sbf`. Their frames do not
carry the record. Widening them is a cohort's work and belongs to its own row.

### The terms surface prices from a rate, and states it

`packages/dclutch-sdk/lib/openerTerms.ts` gained `fundedRentMinimumV1`,
`fundedRentRateFromMinimumV1` and `openerFirstCrankAtFundedRateV1` — restatements
of `funded_rent_minimum_v2` / `funded_rent_rate_from_minimum_v1`, pinned against
the Rust by the source gate. `OpenerFirstCrankTerms.tsx` takes an optional
`fundedRentRate`: with one, the figure describes THAT market at the rate its own
founding recorded; without one, the rate is derived from the cluster in two
cross-checked readings (`derive_funded_rent_rate_v2`'s discipline) and a cluster
whose rent is not affine in the length is refused by name rather than averaged.
Either way the rate is printed beside the number it produced. The old surface
made four independent reads and stated no rate, so a cohort-15 market priced at
today's devnet rate was a fifth wrong and nothing on the page said so.

### Both records get a client, so the runbook rows have a callee

The build wave's §2.5 named the family's real gap plainly: **there is no
client.** Nothing off chain constructed either request, which is why ruling R2's
promised `upkeep-found` row did not exist and why `ProtocolParametersRequestV1`
had no non-test consumer at all — the record that makes a ruled value governable
could not be founded by anybody. Two new operator modules close it:

- `crates/dclutch-operator/src/upkeep_vault_v1.rs` — the founding and the one
  voluntary deposit. The protocol-credit frame is DELIBERATELY absent: only a
  program can sign a caller-authority PDA, so a host builder for it would be a
  frame nobody can submit, which is worse than no builder.
- `crates/dclutch-operator/src/protocol_parameters_v1.rs` — found, propose,
  withdraw, apply, with the record, the receipt and the Custody ProgramData all
  DERIVED rather than carried. The bands and the take conjunct are applied at
  the host by the record's own predicates **in the record's own order**, so the
  host and the chain accuse the same bytes of the same thing.

What is still owed is CLI wiring: a `tools/dclutch-cli` or bootstrap subcommand
that calls these two and submits. That is a named follow-up, not a hidden one —
the builders are public, tested, and take only keys.

### The vault says which class has ever carried money

`crates/dclutch-custody/src/upkeep_vault_v1.rs` gained a producer census in its
module doc and `UpkeepVaultV1::class_report`, the first reader of
`UpkeepSourceClassV1::ALL` and `::label`. The honest measurement is stated where
it belongs: `Donation` is the class the one wired producer credits and it has
measured **zero on every cohort**; `SeatRent` is the class that has carried
2,786,520 lamports on every cohort and **has no producer**, because the seat is
prepaid by an off-chain driver's System transfer into an account the Resolution
program ends up owning, and Resolution has no seat-close route.

### Provisional rulings added to the BUILD doc's R1–R10

- **R11. A zero credit calls nothing — and that is the donation being zero, not
  the carve eating it.** `receipt_upkeep_credit` returns `Ok(())` when
  `upkeep_credit == 0`. Under the genesis record the carve is
  `min(whole donation, 0)`, so ANY nonzero donation is housed in full and takes
  the CPI; the path is skipped only when the replay carried no donation at all,
  which is what every cohort to date measured. The vault contract refuses a zero amount by name, so
  a CPI on the common path — every cohort to date — would refuse the whole close
  for having nothing to house. The planner mirrors it: with a zero credit there
  is no upkeep request, so coordinate 26 is required to be in the frame and
  nothing more.
- **R12. The vault is an L7 account, not a tenth compartment.** It holds
  lamports, never collateral atoms, so it belongs to lamport accounting and not
  to L8's per-class conservation. Making it a tenth `CompartmentV1` would move
  the Lean pair counts (81 and 63, both `native_decide`) and widen
  `COMPARTMENTS: [_; 9]`, for a class that can never hold an atom.
  `tools/gauntlet/journey/src/stages.rs` watches it from the first census
  instead, so a close that credits it declares that growth.
- **R13. `hash(instruction_data)` is the digest's one author on chain;
  `UpkeepRequestV1::digest` is its off-chain projection.** BUILD §2.5 said "two
  authors for one digest — pick one". They cannot disagree: `decode` refuses
  every non-canonical encoding, so anything that decodes hashes to what
  `to_bytes` would have produced. The adapter keeps `hash(instruction_data)`
  because it must never trust a caller's claim about its own bytes; the builder
  keeps `to_bytes` because it has no instruction data yet.
- **R14. `InvalidCallerAuthority` carries the address it wanted.** Coordinate
  26's last seed is the digest of a request carrying this close's own receipt
  digest, so it is derivable only from a completed plan — unlike `maker_replay`
  and `rent_owner`, which are discoverable from chain state. A refusal saying
  only "wrong" would leave a builder no way to become right except to
  reimplement the planner, which is the second author this crate exists to
  prevent.

## 4. Tests added, and what each proves

Beyond the build wave's fifteen:

| test | file | proves |
| --- | --- | --- |
| `the_class_report_names_every_class_and_closes_on_the_total` | `crates/dclutch-custody/src/upkeep_vault_v1.rs` | I3 as something enumerable: four labelled rows whose totals close on `inflow_total`, with the seat-rent number the cohorts actually measured. |
| `a_plan_pays_the_owner_the_principal_and_houses_the_rest` | `crates/dclutch-operator/src/direct_close_maker_v1.rs` | decision 0024's split end to end under a genesis record: `total_credit == rent_principal`, `upkeep_credit == unclassified_donation`, the vault's predicted balance moved by exactly the donation, the closer's not at all, and the receipt names the closer. |
| `a_frame_whose_record_is_not_the_custody_owned_one_refuses_by_name` | same | a parameters account owned by the Trading program refuses as `ProtocolParameters(Error::InvalidHeader)` — the exact variant, not `is_err()`. The carve cannot be priced off a stranger's record. |
| four `describe`d cases in `the founded-rate pricing is the Rust's` | `packages/dclutch-sdk/lib/openerTerms.test.ts` | the storage overhead and the affine formula are pinned against `funded_rent_minimum_v2`; the 312-byte seat prices to 2,786,520 at 6,333 and inverts back; a minimum no single rate can price is refused rather than rounded; and the same market costs a different number at a different rate, which is why the rate is now printed. |
| `the_founding_frame_is_the_adapters_frame`, `a_deposit_carries_its_class_and_its_amount_and_no_caller`, `a_zero_deposit_refuses_with_the_contracts_own_name`, `the_zero_key_refuses_as_an_identity` | `crates/dclutch-operator/src/upkeep_vault_v1.rs` | the built frame is the adapter's coordinate by coordinate, at the adapter's own count constants, and the bytes are decoded back through the contract rather than compared to a literal; a zero deposit refuses with `UpkeepError::ZeroAmount` before a fee. |
| `the_founding_frame_carries_the_genesis_and_the_programdata`, `a_take_before_mainnet_cannot_even_be_proposed_from_here`, `a_body_outside_the_bands_refuses_by_the_bands_name`, `a_lawful_proposal_builds_a_two_account_authority_frame`, `a_withdraw_carries_nothing_and_an_apply_addresses_its_own_generation` | `crates/dclutch-operator/src/protocol_parameters_v1.rs` | ruling R6 at the host: a taking body that is IN BAND (asserted, so it reaches the take conjunct rather than being caught by the coarser one) refuses as `TakeBeforeMainnet` on both propose and apply; an out-of-band body refuses by the band's name; and an apply addresses the receipt of the generation it will write, not the record's current one. |
| `pins the crank reward cap TO ITS AUTHOR` | same | the authorship chain, not the literal: the record's generated genesis carries the number and the claims constant projects it by name. Breaking either link goes red. |

Rewritten rather than added: `the_frame_carries_the_record_the_vault_and_a_closer`
(replacing the test that asserted the very absence this family removes),
`the_closer_carve_comes_out_of_the_donation_and_never_the_principal` (a fifth
"halved" case where the record's SHARE binds before its cap),
`maker_and_root_closure_conserve_count_and_refund` (was red on
`total_credit == 111`), `clean_replay_emits_exact_unsigned_outer_and_authenticated_receipt`
and `coordinate_closure_owns_placement_classes_and_refuses_hostile_identities`
(the "no account asks for a signature" assertion became "exactly one, at 24").

## 5. What to distrust in `BUILD_economics.md`, because it was checked

- **§7's green is narrower than it sounds, and the doc says so — believe the
  caveat, not the headline.** `cargo check -p dclutch-custody` was green while
  `dclutch-custody-sbf` had ten errors in the very files the doc calls "complete
  and self-wired". "Self-wired" was true (`mod`s declared, dispatcher at
  `lib.rs:242-247`, both widths disjoint by inspection); "compiles" was not.
- **§2.1's "No Cargo manifest change is owed anywhere on the branch" is false.**
  It was true of the branch as written only because the vault CPI did not exist
  yet. `programs/dclutch-trading-sbf/Cargo.toml` had to drop
  `optional = true` from `dclutch-custody` — which `S12` says, three sections
  later, contradicting §2.1. `cargo metadata --locked` still succeeds and
  `Cargo.lock` is unchanged: making an existing dependency unconditional adds no
  version.
- **`S18` is false.** It says
  `crates/dclutch-market/check-generated-protocol-parameters.sh`'s "`grep -q`
  pins should widen to the new constants". That script has no `grep -q` pins at
  all — it re-runs the emitter, rustfmts, and `diff -u`s the whole file, which is
  strictly stronger than any pin. Nothing is owed.
- **`S15`'s arithmetic is wrong.** It predicts the summary moving `91/85 → 92/86`.
  The gate measures `92 generated, 92 guarded, 72 guards`. Do not hand-edit
  toward the doc's numbers; run `--write` and read the diff.
- **`S1`/`S4` were already partly stale.** They point at
  `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs:567` and `:41`; the
  live call site and doc reference had the same defect but the operator copy at
  `crates/dclutch-operator/src/direct_close_maker_v1.rs:77`/`:1197` was the one
  nothing had touched.
- **§2.5's "two authors for one digest" is not a defect.** See R13.
- **`S9`'s `capability_manifest: observed(22)` collision does not exist.**
  `observed(22)` names a key `[22; 32]`, not a coordinate index, and every record
  key is overwritten from the closure afterwards.
- **The premise that "main moved under this family hardest" is false.** Main
  moved 21 commits and **9 files** since the branch point, none of them the
  close-maker's source. COHORT-17B's deleted zero-key conjunct and the code that
  closed both maker roots landed at or before `f7c03e845`, the branch point
  (`git merge-base --is-ancestor` says so for `e5ebdae2d`, `b7e069828`,
  `40f1cb703`) — they are already in this branch's history. What main added
  after the branch point is the EVIDENCE of that run, and it settles the one
  question worth asking of this family: the two devnet closes each moved
  **1,463,040 lamports with donation 0 and closer carve 0**
  (`docs/evidence/COHORT17_SEATED_FILLED_RETIRING_2026_09_06.md:519-527`), so
  under this branch the same two closes pay the owner the same 1,463,040, the
  closer nothing and the vault nothing. **Ruling R3 moves no measured money.**
  What it does move is the receipt: 248 → 288 bytes with `closer` inserted
  mid-record, so those runs' digests are the stale witnesses of §2 item 8. The
  rebase was clean and touched nothing this family owns.
  `DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1` is **not** a fact main renamed: main
  still defines it at `crates/dclutch-trading/src/close_maker_v1.rs:78` as a
  projection of the record's genesis cap, and this branch deletes it because the
  route now reads the record itself.
- **Two files on `main` are not rustfmt fixpoints**, so a lane that formats them
  hands the merge lane unrelated churn:
  `tools/gauntlet/journey/src/stages.rs` (3 lines) and
  `tools/local-validator/bootstrap/successor/src/direct_trade_producer.rs`
  (25 lines), both import-order drift. This lane formatted only its own crates'
  files and deliberately left those two alone.

## 6. The two driver files, and the one thing nobody ran

`tools/local-validator/bootstrap/successor/src/direct_close_maker.rs` gathers
all twenty-seven coordinates and takes the two-pass route ruling R14 forces:
plan once with a placeholder at coordinate 26, read the address out of
`InvalidCallerAuthority(expected)`, re-gather, plan again. **A close with no
donation needs only the first pass**, which is every cohort to date. The closer
is `--closer ADDRESS` on a dry run (which opens no key and so cannot know who
would sign) and the fee payer under `--execute` — the one frame coordinate a
payer may occupy, because it is the one that signs anyway. The printer and the
JSON state `upkeepCredit` and `closer`, and the tool reads the vault's balance
back from the chain and refuses if it is not the projected one.

`programs/dclutch-trading-sbf/program-test/tests/direct_close_maker_on_chain.rs`
installs the vault and the governed record directly on the bank at their derived
addresses, from their own genesis constructors, owned by the release's Custody
program; funds a closer whose keypair the test holds; and replaces the
assertion that is now false by design (`total_credit == case.clean.lamports`)
with the three-way split, including `vault_after - vault_before ==
report.upkeep_credit` **read back from the bank rather than predicted**.

**The fixture exercises the CPI, and that matters more than it sounds.** Under
the genesis record `closer_carve_basis_points` is 10,000 and
`closer_reward_cap_lamports` is 0, so the carve is `min(whole donation, 0) = 0`
and the ENTIRE donation becomes `upkeep_credit`. The clean case's 11-lamport
donation therefore takes the credit path, not the early return — the test's
coordinate 26 cannot be filled statically either, and it plans twice for the
same reason the builder does.

**IT WAS NOT RUN.** A program-test needs SBF ELFs and this lane is not permitted
to build them, so `cargo check --tests` is the whole of its evidence: the test
compiles and its assertions state the right thing, and whether the CPI into
Custody actually lands on a bank is unmeasured. **This is the single largest
untested claim this branch makes** — the vault credit is the only step that
makes the family *do* anything, and no execution of any kind has exercised it.
Run `programs/dclutch-trading-sbf/program-test/run-close-maker.sh` FIRST, before
anything else in §2.

**One caller-visible change the runbook must carry:** `--closer ADDRESS` is now
REQUIRED for a dry run of the close-maker driver. There is no defensible
default — the frame pays a carve to that account — and under `--execute` the
closer is the fee payer, so only the dry run needs it. Any `steps.tsv` row or
runbook line that dry-runs `local-private-validator-direct-close-maker-v1` or
`devnet-direct-close-maker-v1` gains the flag.

Both driver files also carried pre-existing rustfmt drift on `main` (10 and 28
lines). They ARE formatted here, unlike the two files in §5's last bullet, and
the asymmetry is deliberate: this family rewrites eight hundred lines of these
two, so a fixpoint costs nothing, while there it would have been twenty-eight
lines of unrelated churn beside a three-line change.
