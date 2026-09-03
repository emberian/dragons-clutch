# GOAL — the completion campaign, swarmed (standing, from 2026-09-01 04:5x)

Drive `docs/MASTER_COMPLETION_CONTRACT.md` C-00..C-16 to physical evidence.
Queue is `docs/LETTER_TO_CLAUDE_2026_09_01.md` (five walls, lanes S0–S11,
continuation order). Opus lanes only. Heavy Linux via hbox `swarm-build`.
Dirty tree deliberate — named-path commits through `tools/lane.sh`, never
stash/reset/clean/`add -A`, never delete under `~/dev`.

Measure, refute the obvious suspect, only then move a constant or convict
code. A named wall is a fine deliverable; a green fixture proving the wrong
thing is not.

## Current thrust

Convict the two walls that are already located, in parallel with the
letter's own first crossing (Dealer `Content`).

## Next 3 moves

1. POSTJOIN (orchestrator): `real_registry_executes_profile14_direct_hot_under_protocol_limit`
   is the HONEST CONTROL for `registry_hot_continuation` — its 26 hostiles are
   measured against it, so while it is red every one of them may be passing
   for the wrong reason. Establish execute-failure vs compute-ceiling by
   measurement before naming a cause.
2. STRUCT-DEC: `crates/dclutch-rational-representation-v2-operator/src/lib.rs:1185`
   pins `mint.decimals != 0` with a STATED reason ("this family's claim Mints
   are whole units"). Refute or uphold that reason by reading whether any
   arithmetic consumes `decimals`; only then remove and add hostiles.
3. DEALER-CONTENT: convict the immutable-artifact tranche between Hot
   checkpoints `root-product` and `artifacts-strategy-effect` on the pinned
   `686bf2e5` Trading ELF. Do not weaken `Content`.

## POSTJOIN — convicted and repaired (`ff8ca269`)

`686bf2e5` pinned the lifecycle rent credit's owner to `frame.registry` on the
stated premise "that owner is the already authenticated fixed Registry
coordinate". Instrumented replay measured `owner_program=0x91` (Registry) vs
`account.owner=0x97` (Rent) — the premise is FALSE; `programs/dclutch-rent-sbf`
is the canonical lifecycle RentCredit contract and creates these accounts
itself. The conjunct was unsatisfiable on every honest Direct Hot transaction.

Conviction path (one line out of 2,132 `Content` raise sites): phase
checkpoints bracketed it to `p5-sealed-ownership-arena` -> `request-lifecycle-preplan`,
a single call; instrumenting all 708 raise sites in the file named
`hot_v3.rs:8791`; splitting the ten-way conjunct named
`owner_program.key != account.owner`; a `sol_log_64` byte probe named the two
programs. A clean-HEAD worktree reproduced it identically, so no lane's dirty
WIP is implicated.

The repair REMOVES rather than weakens: `create_program_address` at
`hot_v3.rs:8809` already re-derives the credit's key from its own seeds under
`account.owner`, binding the owner to THIS credit — strictly stronger than
pinning a coordinate. It predates `686bf2e5` and was untouched by it.

Blast radius was 10 of 27, not the 1 CI reported (the runner stops at its
first named case). After the repair: **24 passed, 2 failed**; no hostile
flipped red->green.

**The refusal was MASKING a compute regression** — that held, but the numbers
I first attached to it were WRONG; see the correction section below. The real
failure is Trading as a CHILD of the Registry continuation,
`consumed 1303334 of 1303334 … exceeded CUs meter`, and the moved gates are
`TOP_LEVEL_KEY_INDEPENDENT_CU_V1` / `FEE_BEARING_MEASURED_FLOOR_CU_V1`, not the
continuation floor I named.

Correction to my own earlier hypothesis: Direct's and Dealer's `Content` are
NOT one class. Dealer refuses at `validate_selection` conjunct 5
(`derivation_policy != child_derivation_id`) between `root-product` and
`artifacts-strategy-effect`; Direct passes that bracket and dies later. Both
lanes measured independently and the shared-class guess was refuted.

## Wave 2 (goal: until 1pm) — seven lanes

STRUCT-DEC (resumed, ATA derivation wall) - DEALER-CONTENT (resumed, manifest
vs per-descriptor derivation policy) - SERIES (resumed, activation funding
seam) - COMPUTE-MARGIN (the wall postjoin was hiding) - GENERAL-OPENBATCH -
DIRECT-SELLBUY - CLAIMS-CONSERVE (new files only; `claims.conserve` is
unimplemented and must not be truth-flipped). Orchestrator: tree-wide
consistency and cuts.

**Four of five workspace lockfiles could not resolve under `--locked` at
committed HEAD** (`90e45b29`, plus `371409f4`/`7b8cafd9` for the journey).
That fails any `--locked` CI job on its own and was invisible locally because
nothing runs `--locked` by hand. Repaired as a union — 43 additions, zero
deletions — so no lane lost a line, including the one SERIES asked to have
carried. I caused part of it: the journey relink committed a `Cargo.toml`
dependency without its lock.

## THE NIGHT'S BIGGEST RESULT — one class, three families

Three lanes independently convicted the same shape, none of them looking for
it: **a declaration or guard that no account can satisfy, because it was
authored in one place and never executed against a real account.** Direct Hot's
lifecycle-credit owner (repaired), Dealer's doubly-pinned `derivation_policy`
(ember's ruling), and Structured's account profile declaring `Exact{0}` for the
Rent sysvar and the System program (route-owner proposal). Full write-up in
`WAVE.md`. The lesson is the canonical-generation mandate's sibling: an
expectation must be DERIVED or GENERATED rather than hand-carried — and a
declaration must be EXECUTED before it can be believed, because a component
test over encodings and digests passes forever above an unreachable route.

## CORRECTION: I MISREAD THE COMPUTE FAILURE, AND PUBLISHED THE WRONG NUMBER

I reported repeatedly — here, in `WAVE.md`, and in a published cut message —
that Direct Hot "runs to 1,330,239 of 1,399,700 CU and dies of exhaustion".
**That attribution was wrong.** The COMPUTE-MARGIN lane established that the
1,330,239 line belongs to `the_family_trades_every_geometry_it_is_given`, which
PASSED; parallel test output had interleaved and I read one test's number as
another's. The real failure is Trading as a CHILD of the Registry continuation:
`consumed 1303334 of 1303334 … exceeded CUs meter at BPF instruction`.

Second thing I got wrong: I pointed the lane at
`CONTINUATION_ROUTE_DELTA_FLOOR_V1`. That test PASSES at HEAD and prints that
its route was demoted to harness-only by a decision packet. The gates that
actually moved are `TOP_LEVEL_KEY_INDEPENDENT_CU_V1` (1,268,059) and
`FEE_BEARING_MEASURED_FLOOR_CU_V1` (1,266,429).

What survived: my phase-table ROW ALIGNMENT was right (deltas belong to the
later label) — it was just truncated at the exhaustion point and taken on the
wrong route. The lesson is the cheap one: never read a CU figure out of an
interleaved parallel test log without `--test-threads=1`.

**The conviction that replaced it** is `6e91863c` "trading: execute
authenticated funding lifecycle" — +10,367 CU in ONE commit, 1,176 inserted
lines into `hot_v3.rs` under a one-line message and no measurement. Bisected at
CI parity, 32 seeds, five role ELFs per revision, with the method validated by
reproducing a known-good revision's recorded floor TO THE UNIT. Total +26,466
on the public route; per-phase attribution sums to +26,452, fourteen CU apart.
The lane REFUSED to re-pin either constant, both commits comment-only: "raising
it IS the act of spending margin", and margin has gone 100,437 -> 75,472.

**And the finding behind the finding:** of nine revisions sampled across the
overnight wave, EVERY ONE failed — seven refuse the public trade on 32 of 32
seeds, two do not compile. The route only executes again at `371409f4`. So most
of that wave was never measurable, and the remaining +14,276 cannot be bisected
until the refuse-all band is understood. CI never reported any of it because
the published programs tier dies earlier at
`error[E0433]: could not find 'series' in the crate root` — so the margin gate
has not been running at all.

## LIMIT OF THE UMBRELLA CHECK — feature combinations are not covered

`cargo check --workspace --all-targets` builds DEFAULT features. A lane found a
break it structurally cannot see: `hot_v3/series_expiry_v1.rs:28` gained an
ungated `use crate::series::...`, which is fine by default but E0433 under the
`outer-only` feature that `trading-outer` builds with — and that runner is the
only way to execute the activation suite at all. Unrunnable since `0afa24a5`
landed the same day; repaired in `fd88f013` by moving the four byte-string
capability labels into the kernel that already owns Series schema preimages, so
no lane's file gained a mirror. When a crate has meaningful feature gates, the
umbrella must be run per gate, or a whole suite can go dark while every default
build stays green.

## CORRECTION: there were more than five lockfiles

I repaired four stale workspace locks (`90e45b29`) and reported "all five
workspaces". A lane then found a FIFTH stale lock I had missed —
`crates/dclutch-svm-harness/Cargo.lock`, never regenerated after
`dclutch-provider-transport-v3-operator` took `solana-system-interface 2.0.0`,
which made every harness campaign refuse `--locked` (`9514ca71`). So "five
workspaces" was my count of the locks I happened to look at, not a fact about
the tree. Anyone auditing this should enumerate `Cargo.lock` files rather than
trust the number.

## TREE IS GREEN — the five workspaces I check, verified

`cargo check --workspace --all-targets` clean at the root (4m08s), and the four
SEPARATE workspaces the root check does not cover — trading program-test,
dealer program-test, local-validator successor, gauntlet journey — each clean
too. That gap was mine: five lockfiles should have told me there were five
workspaces, and my first "tree-wide" check covered one of them.

## Public CI scoreboard at cut 3 (`5a09fc0f5`)

GREEN, verified in public and not just locally:
`repository hygiene` - `release tooling refusals` - `web+SDK (sdk)` -
**`the journey campaign compiles`** - **`seam register and emission census`**.

RED, and each is a wall we already own by name:
- `SBF programs and the Direct compute margin` — COMPUTE-MARGIN's.
- `SBF program-test suites`, 2 of 7 rows: `dealer` (26 passed, 1 failed — the
  jointly-unsatisfiable `derivation_policy`, ember's ruling) and `postjoin`
  (0 passed, 1 failed, 26 filtered — the control, which now dies on the COMPUTE
  wall rather than the `Content` refusal, exactly as measured locally, so the
  repair moved it forward). `claims` DID NOT RUN — a host fact by design.
- `web+SDK (web)`, 160 passed / 1 failed — the explorer-coverage drift.

Three jobs turned green tonight. Every remaining red is a named wall with an
owner, which is the state the letter asks for.

## Orchestrator done-log (wave 2)

- **Public CI: `repository hygiene` is GREEN** at cut `5a09fc0f5` — the seam
  audit's 46 findings verified retired in public, not just locally.
- `f3a6a6e0` postjoin runner now reports EVERY row and says outright when a
  failed control voids the hostiles below it. It was four bare invocations
  under `set -e`, so the first failure aborted and three rows never ran while
  CI printed one number. That reporting defect is why a ten-case break looked
  like a one-case break. Class checked: this was the only runner of that shape.
- `65b20c24` **red umbrella caught**: `dealer_v3_multi_lp` did not compile at
  committed HEAD — `v3_equity_operator` moved to borrowing signatures and the
  test never followed. Invisible to every per-file check; found only by a
  tree-wide `cargo check --workspace --all-targets`. Worth repeating after
  each wave — and here is WHY these accumulate: the public job that would
  catch them, `every Cargo workspace checks`, is gated
  `if: github.event_name == 'schedule' || workflow_dispatch` with a 120-minute
  timeout (`rust.yml:340-343`). Nightly-only, so a cross-lane compile break
  can sit in the tree for a day while every per-crate gate stays green. During
  a swarm the orchestrator must run it by hand. It found two real breaks
  tonight, and the second was invisible until the first was fixed, because
  cargo aborts the run at the first failing crate.
- Host repo `02bab0b84` + `4dc8bafbf`: the web suite's rustup conflict is gone
  (fix confirmed working — it now fails differently), and the parity binary is
  prebuilt instead of cold-compiled inside a 30s test timeout.
- Cut 3 published (`5a09fc0f5`), tree-hash gated, swept, pushed.
- **Web suite is down to ONE legitimate finding**: `160 passed, 1 failed`.
  The rustup conflict and then the 30s cold-compile timeout are both gone;
  what remains is `explorerCoverage.test.ts` catching a refusal code
  (`DCGVFY02`) that reached the protocol without the browser's explorer
  learning it — a hand-maintained TS list falling behind its wire, which is
  the exact architecture CONSOLE-TRUTH was sent to retire. Assigned there,
  with the instruction to make the set DERIVE from the refusal registry
  rather than to add one string.
- **Second umbrella find, handed to SERIES**: `tests/series_v2.rs` does not
  compile at HEAD (`unresolved import crate::core_composition_v3`).
  Pre-existing from `2d871068`, invisible because a per-crate check builds the
  library and never this test. NOT a two-line fix: the seam deliberately binds
  real library modules to keep one owner per authenticated type, but
  `core_composition_v3` has zero `pub fn`, so binding cannot reach the
  function; and `#[path]`-including it reaches `crate::child_authority_v4`,
  so the closure needs measuring first. I reverted my partial fix rather than
  leave a misleading half-repair in a shared tree.
  NOTE: the first umbrella run never reached this error — cargo aborted on the
  dealer break first. One tree-wide check is not proof; run it again after
  each repair until it is clean.
- **Operator/guide command audit: CLEAN. A finding I nearly filed was WRONG,
  recorded so nobody repeats it.** `docs/operators/` teaches `ticket
  author|post|verify`; all three exist. I then flagged `dclutch found`,
  `portfolio`, `redeem`, `walk`, `join`, `markets ls`, `intent buy` in
  `docs/guides/` as the "guides teaching impossible commands" P0, because none
  are subcommands of the Rust CLI (`tools/dclutch-cli` dispatches exactly
  five: market, capability, ticket, general, fractional-retirement-next).
  **That premise was false.** There are TWO binaries named `dclutch`: the Rust
  one, and a TypeScript one at `packages/dclutch-cli` whose commands are
  found, join, markets, portfolio, product, redeem, refusal, route, spine,
  trade, walk — plus intent/buy/sell in its dispatch. Every flagged command
  exists. The guides are correct.
  The one real (mild) observation left: two different binaries answer to
  `dclutch` and the guides do not say which they mean. That is legibility, not
  a defect, and it is NOT fixed by rewriting guides.
- **Declared-control class swept: clean.** postjoin's failure mattered because
  a dead control silently voids every hostile measured against it (ledger
  M-38). Four tests in the tree declare a named control; one was postjoin's.
  The other three all pass — `capability_seal_close` is 9/9 including both
  `a_stranger_closes_a_stranded_seal_and_keeps_the_rent` and
  `a_defunct_seal_closes_under_a_mined_bump_candidate`, and
  `the_seal_outer_writes_exactly_the_bytes_the_hot_path_expects` passes in
  `registry_hot_continuation`. So the class is retired, not just the instance.

## Done-log

- Published two cuts (`7509c998b`, `546288b8b`), tree-hash gated + swept.
- `673fcb3e` journey relinked (9 errors -> 0); public CI red repaired.
- `7dc20ad0` ember ruling recorded: devnet is disposable.
- Source readiness native/WASM parity MEASURED INTACT in the live tree
  (4/4, ~2s). The public web-suite red was rustup provisioning, never a
  parity break; toolchain now installed explicitly (host repo `02bab0b84`).
  The test stays UNEXCLUDED — an unproven parity is not a parity.
- Lanes live: STRUCT-DEC (decimals gate), SEAM-VERDICT (hygiene red),
  DEALER-CONTENT (immutable-artifact tranche, hbox), SERIES-ACTIVATION.
  Orchestrator holds POSTJOIN.
- **POSTJOIN reproduced, and the compute hypothesis is REFUTED.** The
  control fails on a REFUSAL, not a ceiling: Registry invoke[1] -> Trading
  invoke[2], Trading consumed 446,017 of 1,303,327 CU and raised
  `custom program error: 0x4003` = `TradingError::Content`
  (`programs/dclutch-trading-sbf/src/lib.rs:186`), surfacing as
  `InstructionError(2, Custom(16387))`. Nowhere near a compute wall, so the
  red `Direct compute margin` job is a SEPARATE story and must not be
  narrated as this one.
- **`Content` is one class in two families.** Dealer's honest LP Add refuses
  the same 0x4003 at 148,093 CU. Direct Hot refuses it at 446,017. A
  Dealer-local fix would retire an instance, not the defect. DEALER-CONTENT
  told; both lanes now bracket with `hot_cu_checkpoint!` (gated behind the
  `hot-cu-profile` feature, `hot_v3.rs:616-626`). Grep cannot convict this:
  `Content` has 2,132 raise sites, 782 in `hot_v3.rs` alone. Instrument.

---

# Historical ledger (pre-2026-09-01)

# GOAL — work until 11am: the protocol as good as it can be, all debt burned down

## CONVERGENCE (~17:3x): docs/HANDOFF_CODEX_2026_08_31.md is the
## queue — 17 items, 3 tracks, house law attached. Four lanes still
## landing (PUBLISH-8, OPERATOR-BOOK, OPERATOR-FORMS, CLOSE-DRIVER's
## verdicts); their reports append here. NOT handed off: the cut
## (steward+ember), ember's four rulings, the vault review, cohort-10.

## THE AFTERNOON SWARM (ember: "what oughta we be swarming over") —
## six lanes live, all opus:
- FLOW-1 LANDED (6ba98617): @theme binds BY REFERENCE (verified in the
  emitted stylesheet — :root stays the single source), units module
  incl. the self-caught parseQuantityV1 ("relabelling the box 'claims'
  without converting input would have been ember's bug pointed the
  other way"), the machine extracted 2-lines-different-of-439 with the
  moving type error as proof of faithfulness. → FLOW-2 SPAWNED: the
  7-step stepper + step bodies + board-first step 3 + refusal mapping.
- TICKET-BOARD LANDED (9d26ae80): relay + clients + `dclutch ticket
  post`; two rulings confirmed (WAVE 0db4b643: no clock, no eviction;
  chip says WELL-FORMED never verified).
- (was) FLOW-1: FLOWFUL phase-1 foundations (@theme token aliasing, the
  units module — 500000000 becomes 500 with atoms one hover away, the
  verbatim machine extraction). Stepper+bodies are the next lane.
- CANON-IMPL DELIVERED THE MANDATE (5 commits): six pins now DERIVE
  (account-profile substitution flipped refusal→acceptance = the
  fluidity proof; schema substitution stays red BY NAME), dealer key
  V3→V5 with hand-transcribed vectors, route-binding gates on all 11
  route constants (red-proof #2 = the historical bug re-staged; the
  gate's own walker had 2 gaps, self-caught), vintage refusals, sweep:
  NO second conviction. Queued from its flags: the private-const PDA
  seed domain (authenticated by nothing on chain — look first), the
  general-successor V2-emit-vs-V3-dispatch, the dealer decoy preimage,
  4 NO-ROUTE deletion questions. Census regen committed by orch
  (5154bd65); 3 workspace-lock debts → LOCKS spawned (real conflict:
  solana-account 4.3.2 vs 4.6.0 across the new path dep).
- (was) CANON-IMPL: S3 derive (six program-id pins drop), S4 dealer→V5,
  S1 route-binding gates on the generators (the structural fix for
  the wrong-file pointer class).
- TICKET-BOARD: the dregg relay (tools/ticket-board, axum, lifted
  validation), lib clients web+sdk, `dclutch ticket post` — §4.5's
  missing maker flow gets its first primitive.
- PROFILE-3: §8.1 brick repro, §8.5 declarations, the bootstrap
  ceremony rehearsal (unblocked by 6a9a2ba0).
★ PUBLISH-8 SHIPPED THE FLOW LIVE (9fe6ec208, pages 33442687321,
  69 commits): the 7-step rail renders on clutch.dregg.pro; the
  DCLTCOR2 fix PROVEN end-to-end (market22 decodes DCLTCOR3/v3/368B —
  the conjunct no account could satisfy now reads one); zero console
  errors; SBOM debt zero; 2 rust rows red→green. Remaining reds
  attributed: seam's 10 self-heal at the next cut (the verdicts landed
  after the pin); the heap-inertness wire pin is stale vs cohort-9's
  five-entry set → orch measuring now. DIRECT SMALLS TAKEN (66edf88f):
  claims runner exits 2 pre-build; the tautology deleted; accepted.rs
  dead items gone.
- OPERATOR-BOOK OPENED (93552415): the contract + two walkthroughs
  written BY RUNNING THEM — a real market founded mid-writing
  (3UugcUQt…, founding-dcltgmf3 at 1,069,561 CU = 76% of the ceiling
  in one tx). FOUR WALLS: two site-published guides teach commands
  that cannot run (→ handoff 18); the gauntlet's full mode is dead
  code after its own 15-min build (→ 19); founding unreachable cold
  (--predecessor-profile has no fixture, → 20); stale release binary
  (→ 21). Book left unwired from the site ON PURPOSE ("unlinked is
  not unpublished" — listing IS publishing; rides the next cut).
  Walkthrough 3 chartered in the book: "Keep the chain" — the tickets
  of walkthrough one crossing on the market of walkthrough two.
- OPERATOR-FORMS LANDED (5 commits, web 1440 / SDK 634, honesty
  guards 246→289): 106 fields audited — 1/106 validates pre-submit,
  46% answer typos in web3.js's voice, 42% fully derivable; seven
  typed fields shipped (validation = a total function of the text);
  /product-v2's six required inputs became DERIVED values proven by
  round-trip through the real builder; /found's refusals land at
  their fields. Found bugs → handoff item 5 (/liquidity 38-vs-39;
  /direct's two dead fields). Its best move: reconnaissance caught
  FLOW-2's landing and REUSED its grammar instead of building a twin.
- CLOSE-DRIVER's verdicts restored seam to PASS (f19e10e5): 4 seed
  restatements fixed via accessors (+2 unflagged bump-bearing siblings
  — retiring the finding isn't retiring the defect), 3 verdicted with
  attribution to PROFILE-3's file, and the class-6 pair EXPOSED A REAL
  CUT-DAY BUG: rent_owner-as-fee-payer (the first thing an operator
  would try) would refuse on chain unexplained — now refused at plan
  time by name, held by a test. Addresses proven unchanged: the real-
  ELF close lands at the identical 111,505 CU. New tag confirmed in
  WAVE.
★ TIERS CLOSED THE MARGIN (e74b5dd8): +6,876 attributed to the BASIS
lane — unconditional admit_selection_v3 (+446) + rewritten decode with
price-gate probe (+4,567) running 4× per trade; all three brief
suspects AND its own leading hypothesis refuted by measurement (the
suspected 50-account frame: 2 CU). Falsified BASIS_ABI_UNIFICATION's
"zero CU" claim → doc corrected by orch (33d89959; cheap recovery
named: hoist to the founding caller, ~4,500 back, unchartered).
Floors re-pinned 1,271,552 / 1,269,919 with the honest comment that
the bargain was NOT met. Clippy fixed at source (the 8-arg group
became a binding struct, killing a transposition hazard). Board: 7/9
green; seam's 10 new findings routed to CLOSE-DRIVER (fix-or-verdict);
suites/claims = the arm64 host limit + an exit-code conflation queued.
★ FLOW-2 SHIPPED THE FLOW (b7a53b6c + 5e184264, web 1363/1363): all
7 steps + the pre-stepper walls; 44 refusals routed to their owning
steps behind a DRIFT GUARD (every fragment asserted verbatim in the
module that raises it — a reworded refusal fails at the rewording);
self-caught the shared-tail bug that would have told a step-7 reader
to re-sign; FOK renders as a fixed value not an input-that-always-
refuses; the effect-loop fix means connecting a wallet hides your own
offers with no relay round-trip; machine BYTE-IDENTICAL to FLOW-1's.
Flags queued: maker flow (§4.5, the board's empty state says so
honestly), TicketBoard fetch-surface test, board slotBasis vs own
finalized slot, devnet e2e needs a live wallet. Doc-comment handback
fixed by orch (a8439ed6). → PUBLISH-8 SPAWNED (the flow + the
DCLTCOR2 guard fix to the live site).
- OPERATOR-FORMS SPAWNED (ember: "extremely raw forms... oughta be
  more semantic"): phase A = the full-input audit across 8 consoles +
  OPERATOR_FORMS_V1 spec (typed fields; DERIVE rule — chain-derivable
  fields pre-fill with provenance; the KEYPAIR rule — no key path ever
  typed into a browser; the ACT shape — simulate primary, execute
  gated); phase B = shared typed fields + the two worst consoles
  converted. Grouping not simplification; precision preserved.
- HBOX-CONTROL CONFIRMED the bump (23/23: campaign 13 + compaction 6 +
  2 walls, canonical Linux Token-2022 fixture e2acdfb7…, digest ring
  closed; proved the run COMPILED the bumped pins — not stale
  artifacts). Peak 6.01 GiB under swarm-build; co-tenants untouched.
  LOCKS' debt paid. Standing fact: hbox:~/dev/dclutch is now a warm
  Linux runner for the campaign (~15 min saved next time; reclaim
  command in its report).
- LOCKS LANDED (3a4565bd): SBOM STOP → PASS (58 manifests, 0/0). The
  conflict decided by STRUCTURE not headcount (downward impossible:
  dclutch-operator hard-pins ALT 3.2.0) + the documented bump-older-
  upward rule; root cause = today's dev→dependencies promotion.
  Honest debt: the compaction campaign not re-confirmed post-bump
  (Token-2022 fixture builder refuses Darwin) → HBOX-CONTROL spawned
  (swarm-build, filtered, co-tenant discipline).
- CLOSE-DRIVER LANDED (7 commits): both cut-day invocations (gate-9
  close + ZeroBump one-shot), dry-run as a TRANSPORT PROPERTY (no
  --execute = ReadsOnly connection), refusals mirrored by calling
  close_maker_replay_v2 itself (cannot drift; ordering pinned so
  LiveIntents reports before FeeOutstanding — never misdirect an
  operator to settle a fee that wasn't the blocker). CUT CHECKLIST +=
  one loopback close run (the RPC derive_coordinates path is the named
  residual risk; ride the bootstrap-stage world). PROFILE-3's parked
  ceremony-CLI wiring cherry-picked onto main by orch (f4bb48b7,
  compiles clean). Queued small: the direct_market capacity tautology.
- (was) CLOSE-DRIVER: the devnet close plan-builder + ZeroBump one-shot
  (cut-day prerequisites; refuse-at-plan-time discipline).
- TIERS: margin-gate attribution mid-measurement (+6,928 both arms,
  three suspects refuted by reading; Claims-frame hypothesis live).
Also landed this hour: UPKEEP_VAULT_V0 design sketch (e9aea603 —
neither taken nor burned, but housed; cohort-10 pair with the
opener-receivable completion; ember likes it).

## COHORT-9 OPENS (~11:0x, ember's steer: "fix up more of them bugs";
## all bumps/breaks authorized; Helius ruled scheduled-rotation)

★ PUBLISH-7 SHIPPED (4e154b5de, pages green): EMBER CAN RETRY — the
served chunk BINDS V4 in its acceptance predicate (verified in the
minified code; both markets tradable=true from the published site).
Cleared 5 web reds via regeneration; refused to baseline the one real
security question (unguarded zero-pubkey at fractional_claim_check
:612 → routed to FRACCHECK-7 with the guard-or-adjudicate charter).
CANON adjudication landed (f035e26a): five surfaces ruled — S1
generate-with-ROUTE-BINDING-gate (a generator must prove its scraped
file is what the live route binds), S3 DERIVE (drop the six equality
pins; Rust binds by content only), S4 the dealer key → V5 from the
route author (V3 has zero binders), S5 refusal-quality half first;
six emitter briefs for the twin-less SUSPECT class; process defect
flagged (twins diverged without twinIdentity redding). Implementation
order: LITERALS (running) → S3/S4 → S1 → S5 → emitters.
LITERAL SWEEP convicted a table (export in scratchpad): hard-drift
incl. directHotChain pinning DCLTCOR2/v2 vs generated DCLTCOR3/v3 (web
AND sdk, which also DIFFER from each other), the stride-16, dealer-256;
shadow constants incl. one sharing the EXACT NAME of its emitted twin;
whole literal decoders beside their imported offset sets. SUSPECT class
(no twin exists yet: V1 resolution cert emitter, the seal contract has
NO generated file — P-007 again) → CANON's doc. → LITERALS spawned
(opus, mechanical fixes; DCLTCOR2 liveness verdict first).
BUDGET STEER (ember, ~2:30pm): Fable at 87% for the week — Opus lanes
resume freely; subFables wind down (land the coherent core + write
zero-research handoff briefs for Opus/codex successors). NO NEW FABLE
SPAWNS. 429 wave resumed: PUBLISH-7 + child (ember waits on the cut),
FRACCHECK-7, PROFILE-BUILD (wind-down: Lean core + handoff; its
scout's campaign map exported to scratchpad/profile-campaign-map),
CANON + child (wind-down: adjudication doc + dispositions).
★ PROFILE-2 landed (12 commits, 5/5 mutants killed on real ELFs): the
succession is REAL end to end. Consumers V2-only with no fallback
(2951b226) and red-proofed BOTH ways -- the structural window before the
ceremony refuses founding, and so does a frame aimed at the sealed
predecessor in a world where nothing is missing. Shipped operator builder
(b8ecb8a4, 23 named refusals, no authority arg and no moved knob: it
derives moved-ness from content and consent from the predecessor record).
Real-ELF ceremony campaign (86f2b87e + 8622ac55) driving that builder, one
hostile per conjunct, 6 targets 33 tests. THE FLOOR FOUND A HOLE: relaxing
conjunct 4 from strictly-later to not-earlier passed the entire campaign,
because every not-forward hostile bound a LATER slot; the equal-slot case
now exists and kills it, 4 of 5 mutants redding EXACTLY the assertion that
owns them. Measured, not assumed: the ceremony costs 174,624 CU (the
ruling's section 7 only argued it would fit), and the unmoved binding
really does ride the predecessor's admission -- 42,445 CU when nothing
hashes. THREE CONSTRAINTS THE RULING DID NOT HAVE: a consent key cannot
also be the payer (privilege union; the cut needs a second wallet), an
unsigned Core authority refuses AccountFrame not Infrastructure (the
frame's parse shadows conjunct 1's own clause), and a world whose Registry
never moved cannot reach V2 AT ALL -- so the local-validator bootstrap must
rehearse the cut ordering, and is left coherently on V1 rather than
half-flipped. Found three tiers ALREADY RED on main and fixed the two that
blocked me: seam (unbaselined since the ceremony route landed) and journey
(since fcd6aecc left FoundStateV2.price_gate unset); programs' Direct
margin gate and emission's market-core-codec clippy are routed to their
owners, not mine. REMAINS: section 8.1's brick (needs two banks), 8.5's
declarations, and the bootstrap ceremony stage (blocked -- that crate does
not compile at HEAD).
★★ THE COMPACTION CAMPAIGN IS DRIVEN (FRACCHECK-7, 7 commits after
six honest refusals): a stranger compacts a sleeping holder end to
end on real ELFs — conservation off the ACTUAL transactions (579,240
CU vs ~928k projected; zero burned; zero residue = correct), the
holder-pays-nothing witness ("the line between a permissionless crank
and a fee levied on the absent"), the worthless-coordinate witness
(Mint bytes identical), its own campaign's hardcoded-scenario defect
self-caught. Rulings answered in WAVE b0e81f7c (seam tag confirmed;
Economic-collapse queued; opener-shortfall → ember).
(PROFILE-2's own fuller entry stands above; bootstrap stage unblocked
by the 16-file adoption 6a9a2ba0.)
★ FLOW-IA SPEC LANDED (2458f320, 1215 lines): three journeys, 7-step
trade flow with ~25 mapped refusals remedy-first, THE UNITS SMOKING GUN
(formatAtomsV1 existed and was tested — ember's 500000000 was 500 all
along; the panel never received the decimals), ticket-board-first step
3 with the transport ladder priced (U-002's on-chain records ALREADY
EXIST in codec+explorer; the missing piece is the MAKER flow §4.5),
shadcn via @theme aliasing (Tailwind already installed, zero utilities
in use; keep <details> or 224 honesty guards go vacuous), phase 1 ≈8
lane-days. Drive-bys documented: /direct is a byte-identical dup of
/trade; /resolution header lies; /live scoreboard tiles hard-coded.
TIERS lane spawned (margin gate +5,500 attribution + re-pin; clippy;
final board). Canvas "Flowful Clutch" updated with all 5 review fixes.
★ PROFILE-RULE ruled + BLESSED (f985bede + WAVE cd666f42): ProfileV2
SUCCESSION rides the cut — slot-tolerance refused as UNSOUND (deployer
alone could rebind everything behind a lying digest); the ceremony =
DeclareSuccessor's conjunct geometry on the infrastructure pair; V1
never mutated; consumers V2-only. Discoveries: the deployed Registry
is DEPLOY-1's ORIGINAL BYTES (cohort-9 ships DeclareSuccessor to chain
for the first time); resolution-proof joins the redeploy set (else no
cohort-9 market can resolve); Custody = the unmoved role; Rent's debt
ruled OUT (deferral is now a decision); P-008 documents the constraint.
→ PROFILE-BUILD spawned (fable, ~3-4 lane-days critical path, defines
the cut's deploy ORDER). For ember non-blocking: the dual-signer
estates question; Rent's deferral list.
★ PANEL-FIX convicted BOTH pins (6 commits): effect.schema pinned to
effect-kernel V3 while chain+Rust bind V4 — the generator was POINTED
AT THE WRONG FILE so the byte gate was green while mirroring a
superseded author (the naming trap: v3.rs's preimage reads "effect-
program-v4-…"); plus itemScalarStride hand-literal 0 vs emitted 2;
plus the allowance ceiling → exact-debit equality with the remedy
named. Both markets reach tradable=true/walls=0 headlessly against
live devnet, real-chain vectors + red proofs now gate offline. Dealer
LIFECYCLE_SCHEMA suspect (ZERO Rust binders) flagged-not-guessed →
CANON. → PUBLISH-7 spawned (regen-debt cleanup + the cut so ember can
retry from Talisman); CANON spawned (the mandate, surfaces named:
generator-source binding gates, literal sweep, the six TS-only pins,
the dealer suspect, release-aware selection).
FRACCHECK-6 landed the ROUTE (5 commits: 8cb9e6c5 the 1,000 security-
critical lines over the 49-frame, 6b's verifier, mutation-proven
guards; w8 refuses 0x5641 EARLIER and better than §17.8 predicted) and
refused the campaign a FIFTH time for the right reason — fixture gaps
now precisely pre-paid (home = fractional-atomic crate; the one missing
admission artifact; the RENT_CREDIT fixture was unfaithful). 50th
account RULED (WAVE b4546291): the Rent program joins the frame while
it's cheap → FRACCHECK-7 spawned (the ruled account + THE CAMPAIGN +
the conservation table from real transactions).
★ LINEAGE-FIX landed (d6e43b11, 5/5 mutants killed on real ELFs): the
one clause — system_program::ID exempt from conjunct 1's executable
refusal, nothing else; the unmoved-role hop frames, lands, records.
AND THE DISCOVERY: the Registry was never casually upgradable — the
write-once ProtocolInfrastructureProfileV1 pins its release BY CONTENT
incl. deployment slot; an upgrade bricks Found/retire/provider-resolve
with no in-tree repair (both escapes dead). The every-cohort carry-
forward was an UNDOCUMENTED LOAD-BEARING CONSTRAINT → PROFILE-RULE
spawned (fable): the invariant, the consumer map, succession vs
slot-tolerance vs defer, the ruling that gates the cut.
★ CURVATURE LANDED ON MAIN (821f5dc6): tag 3 accepted end to end —
a degree-2 market founds on a real Core ELF with its DCLTPGT1
certificate, refuses 0x3012 by name without; 276 refusal codes held
through FOUR merge rounds; seam true; 14/14 found_program_test, zero
frame diagnostics. The landing's own lesson: regeneration found the
AUTHOR'S five codes had never reached any surface on any branch —
a text-merge ships that hole silently. Doctrine added: TS registry
regenerates BEFORE genref (the abi doc derives from TS — wrong order
gives a stale doc with a green check). Frame deviation (optional
37/39) recorded. genref-stale handback = PANEL-FIX's live WIP, left
to its owner.
★ EMBER HIT A LIVE PANEL BUG (Talisman, market21/22): "selected
CapabilityProgramV4 is not the schema-bound signed Direct InlineOrdinary
bundle" — the RUST route accepted these exact records this morning (the
trade landed through them), so the TS authenticator drifted behind
cohort-8's publication (mirror disease, panel edition) → PANEL-FIX
spawned (fix at the author + chain-record vector + the known
allowance-equality bug in the same visit + headless verify against live
market22); publication cut follows its green.
LINEAGE-WRITER done (5 commits on main): the pen exists — builder
(reads authorities from the successor's cache, deployer only via
keypair-env, checked against what the CHAIN named; simulation needs no
key), operator subcommand with --i-mean-devnet, real-SVM campaign 4/4
(which caught two vacuous hostiles of its own, one of which LANDED the
record it meant to refuse), loopback end to end at 81,942 CU. BLOCKER
MEASURED: the deployed Registry refuses the exact hop gate 6 needs —
conjuncts 1+6 mutually unsatisfiable for an unmoved role (the System
Program IS executable; the fixture that hid it presented what no
runtime presents) → LINEAGE-FIX spawned (one clause, red-proofs both
ways + the skipped mutation-testing); REGISTRY JOINS THE COHORT-9
REDEPLOY SET — its first upgrade since carry-forward began.
★ CLOSEMAKER LANDED ON MAIN (502f5a06, self-landed via regenerate-on-
merged-tree — the regen UNIONED refusal registries 267→271 where a text
merge had left stale outputs). The decrement is in the tree; wall-22's
sequence green on real ELFs. DIST CUT by orch: v0.1.0-devnet.3 tagged +
pushed (ebd14cf8b, subtree from live 6c9d46f6, sweep CLEAN) — dclutch
ticket author/verify ships. RELEASE GREEN + VERIFIED: the installed
binary reports 0.1.0-devnet.3 and `dclutch ticket --help` speaks (3
platforms + installer + checksums live).
FRACCHECK-5 done (5 commits on main): both §17.8 rulings implemented —
frame 49=36+13, gate arm with writability inverted, w1-w6 each killed
by a targeted mutation (the predicate-drop mutation redding ONLY w3 is
the witness doing its job); action_geometry exhaustive; hazard 2
CORRECTED (write_claim_check cannot share — 288 vs 320; close_and_split
can, via plan.shared()). Route again honestly refused (unblocked but
undriveable without commit 10) → FRACCHECK-6 spawned: the route, the
verifier, THE CAMPAIGN, w7-w8.
★ CLOSEMAKER BUILT (4 commits on its branch): THE MISSING DECREMENT
EXISTS — Lean-first (feeOwed added, E5 lockout in consumeNonce, fee
receivable conserved: "a close is never the event that ends a nonzero
obligation"), FOUR gates relaxed (found the fourth the review missed —
the transition bytecode in release content), the decrement authored AS
release content cross-derived by the ELF, ZeroBump riding, wall-22's
stop drained on a real bank (~98k CU close). Retiring amendment
BLESSED (WAVE 5c091953); donation slice provisionally 0 pending
ember's ruling. My merge aborted on generated-file conflicts → lane
re-landing via regenerate-on-merged-tree. Queued: devnet close
plan-builder (~1 lane-day); 2 pre-existing reds routed (fractional-
claim-kernel emission drift → FRACCHECK-5's territory; market-core
clippy-1.97 → small).
FRAC-RULE done (§17.8, 564d2d31): ruling 1 — the root's signature is
load-bearing EXACTLY ONCE (the SetAuthority burn hand-off; extend the
gate, writability INVERTS); ruling 2 — TradingCallerAuthority dropped
as ceremony (the root signature proves more than the PDA did; native-
sibling parity; O-016). Veto window EXERCISED: signed off (WAVE
794b2eda), w8 pins the no-Trading door. "Trading-composed" = composed
for signature, not authority. → FRACCHECK-5 spawned (both rulings +
5c + the route + the campaign, witnesses w1-w8 binding).
LINEAGE-DEVNET done (96615596, ZERO spend/writes): the "missing ids"
were TYPE CONFUSION — d202e1f4/97d49888 are checked-upgrade PLAN
digests, not set ids; the real ids were on chain all along (cohort-7
91dcbefd…, cohort-8 559f26e6…, both routes agree, address-derivation
proven). market22 walks to the current world in 0 HOPS today — it owes
a declaration only when cohort-9 activates. Declarations blocked on:
conjunct 6 wants the roles' shared upgrade authority's signature (the
deployer — a legitimate cut-day act), and NO DeclareSuccessor builder
exists anywhere → LINEAGE-WRITER spawned (cut prerequisite: builder +
subcommand + dry-run, 8 hostiles by name, walk-follows-the-hop test).
Passing finds queued to it: the 352-vs-360 help string; the
alreadyCurrent-on-stranded trap.
FRACCHECK-4 refused honestly + LANDED (3 commits, merged): the
"assembly" premise died on contact — frame corrected 48→50 (a finalized
record isn't authenticated without its raw/STAGING pair; only raw
halves can't prove the denominator's terms are settled), and TWO
unnamed gaps verified: fractional_root_signer never admits the
compaction kind, and TradingCallerAuthority has nothing in-frame to
derive against (while the crank is permissionless BY DESIGN — so what
does "Trading-composed" mean here?) → FRAC-RULE spawned (fable
adjudication, §17.8). It reverted its own near-complete commit 6
rather than land a stub. Brief drift caught: 5c lives in the operator
crate, not trading-sbf.
SPLINE-WIRE stopped at the right seam (4 commits on its branch:
overflow envelope, cumulative-floor blessed + other deleted, admission
cascade's FIRST production caller, DCLTPGT1 ported into founding's
reach). MEASURED: degree 3 is unfoundable under u128 (span^6 —
SignedU256 is the unlock, not tidying). Its frame question RULED: take
its own +2 (KAPPA's inversion mooted the ride-along; one cut = one
restrand). Proceeding to the atomic stack, seam LAST.
MIGRATE done (4 commits): the lineage READER half real — the walk
authority + SDK mirror (a market's history followable, `path` = the
traversed sets), the AlreadyCurrent 5-site dedup (two had DRIFTED:
the weaker admitted what the stronger refused, nothing compared them;
and GOAL's "red-proofed both ways" claim was wrong — SetAlreadyCurrentV1
had never been constructed by any test until its five), retroactive
authoring proven sound (the record deliberately carries no clock).
BLOCKER FOUND: cohort-7/8 full 32-byte release-set ids exist NOWHERE
in the repo (8-hex prose only — a truncation can't seed a PDA) →
LINEAGE-DEVNET spawned (sole devnet writer: recover by address-proof,
declare the hops, walk market22's history from the current world).
Commit-4+ (weeks, wire) correctly out of the cut; its CoreState field
must join one batched widening when it comes.
KAPPA done (4 commits, no wire break — premise inverted with evidence):
the chartered widening was ALREADY AT HEAD (principalCapSets at 288
since ff008fea; it corrected C9-REVIEW's stale batching row in place).
What was missing: the capacity refusals were FLATTENED into neighbors
at all four sites (now named 0x500D/0x5168/0x518A/0x5208) and the check
was green BY VACUITY (every fixture founded at u64::MAX) — first real-
ELF red-proof shipped (unbind the cap → the excess commits). Cut debt
named: 3 fixtures still found at MAX; κ=1/4 Provisional; THE FLAGSHIP
REFOUND AT THE CUT SHOULD BE BOUNDED (vol-derived floor → real cap) —
added to the cut charter.
FRACCHECK-3 done + LANDED by orch (merge 5ab11648): both gating Trading
layers closed (composition decode + execution arms + the named 48=36+12
frame); the route deliberately NOT written ("1000 lines I couldn't
demonstrate would look like progress"). CLASS FINDING: a 744-byte
request grew a shared frame 3,072→3,712 with ZERO diagnostics — CI's
grep cannot see below-4096 growth → QUEUED: FRAMEGUARD (delta-ratchet
gate on sbf-frame-sizes.py output). Also queued: 7 pre-existing
slicing-may-panic clippy reds in two codec crates. → FRACCHECK-4
spawned (the 8 assembly commits + the campaign; hazards binding).
C9-REVIEW landed (1ce14755): ALL FIVE BUILD, NONE AS SIZED. Headline
finds: CloseMakerReplay was UNREACHABLE as sized (begin-retiring demands
count==0 before Retiring while the close is gated ON Retiring) + must
refuse fee_owed!=0 (close erases the receivable, launders E5; Lean
MakerRoot lacks feeOwed); spline must carry the founding price-gate
conjunct (removing the decode refusal is today's ONLY gate) + overflow
arm; KAPPA batches with floor_content_id + Found-frame +6 (routed to the
live lane); migration premise FALSE — design exists, lineage
retroactively authorable (dissolves the traded-market worry). ONE CUT,
9 gates. → SPAWNED per verdicts: CLOSEMAKER (fable, amendments binding,
ZeroBump rides its Trading upgrade), SPLINE-WIRE, MIGRATE. Rulings for
ember: donation-slice payee; RECORDS-MIGRATE split; Retiring amendment
(orch veto window).

Earlier in flight: C9-REVIEW (Fable teardown of the plan's creative/challenging
aspects — CloseMakerReplay×fee-debt, selector blast radius, ZeroBump
write-once tension, spline sequencing, what's missing, one-cut-or-two);
FRACCHECK-3 (the 9 remaining compaction commits, the 48-account frame);
KAPPA (manipulation bound onto CoreState via the emitter, version-gated).
HELD for the review's verdicts: CLOSEMAKER, SPLINE-WIRE, ZEROBUMP.

## ☀ GOAL CLOSED 10:32 EDT — every lane landed, every board green

Final state: **all public CI green simultaneously for the first time**
(checks 4/4, rust 4/4, pages; three rows flipped red→green in the last
cut, nothing regressed; sbomVerify's last red cleared at a3d28f2d).
The site at clutch.dregg.pro serves the first public trade, the console
copy, and the full type system (final content-sync 405709b64). The
first market life stands at **82 acts, redeemed in full, conserved to
the atom**, stopped only by cohort-9's chartered CloseMakerReplay.
Twenty-one walls fell tonight; each left a red-proofed gate behind it.

## ☀ MORNING REPORT (written ~10:4x; two lanes still closing)

**THE FIRST PUBLIC TRADE IS ON DEVNET.** `4YQLY9ts…`, slot 490,907,340,
1,309,797 CU, conservation cell-by-cell — and its fee was settled BY A
STRANGER (the permissionless two-tx design, live on a public chain). The
final wall was one byte: a bump hint correct for every zero-fee fill ever
assembled, wrong for the first real fee.

**Cohort-8 is live** (five roles, the seal fix this cut existed for —
THE FIRST CAPABILITY SEAL EVER now on chain — plus CloseSeal, the fee
protocol, the CU floor cut). Two markets with buckets derived from
measured volatility, per your steer. New evidence kind: AlreadyCurrent
(a chain dump outranks a receipt).

**The first market LIFE: 82 acts CONSERVED** — found → filled →
fee-settled → resolved → REDEEMED IN FULL (a real outcome: claim 2 won,
the collateral round-tripped 550,250,000 atoms to the atom) → the first
CoreBeginRetiring ever. TWENTY-ONE walls convicted one honest lane at a
time. The stop is the night's biggest protocol finding — **wall 22:
CloseMakerReplay is encoder-only** (the Lean spec proves the decrement;
chain never implemented it; five-way-enforced gate, no override) — so
every filled market is unretirable until COHORT-9 ships it. Charter
recorded in WAVE.md: CloseMakerReplay, ZeroBump seal recovery, General's
re-publication.

**The site shipped twice** (strike-five copy + the full type rebuild +
AGPL footer; the trade cut is in flight) and **public CI is green for the
first time** — after four decorated gates learned to tell the truth.

**Protocol completion:** General 12/14 actions authored (order book at
artifact level, 15 execution theorems); basis option-D executed wire-free
(de Boor ported, rounding ruled); 0017 tripwires incl. Core's first
continuation coverage; 50 unmerged branches → 3 with tombstones; ledgers
evidence-true; every bound classified (CLIFF_DOCTRINE; K=3 was already
unissuable — the packet, not the record bound).

**Spend:** ~1.34 SOL total across the cut + trade; deployer never signed.
**Yours only:** Helius rotation; the vm_compressor=2 call; cohort-9
riders (ZeroBump seal recovery, General's 68→131 re-publication).
The planned-but-not-launched list you asked for is the next section.

Refreshed 2026-08-31 ~05:1x (ember re-issued /goal; was "excellent and
complete until 10am"). Standing steer: all public drivable, load simulator
on live devnet, copy at the strike-five bar, design at the Linear/Stripe
bar. Full-autonomy directive in force — nothing left to ember. The debt
ledgers this burns: the Night worklist + Queue below (distilled from
ASPIRATION_LEDGER, OMISSION_INDEX, SLIPPED_THROUGH_SWEEP, SPELUNK's 20).

## Current thrust

Night wave, 10 lanes live (IDs in SESSION_STATE.md header):
FINALIZATION (first local fill on 43080), FEE-TX2, FRACCHECK-2, SIMLIFE-3,
EXPLORER, HYGIENE, TRADE-4 (fires the first public trade on market19, then
keeps devnet alive), CLOSESEAL (E3: collector-keeps-capped), GRICE
(strike-five minimalism), plus claims-route map + narration-string sweep.

## Next 3 moves

1. DESIGN lane the moment GRICE lands (ember 02:30: "text is just too
   small / imbalanced / rethink the graphic design and iterate") — modular
   type scale, mono demoted to values-only, balanced grid; details in
   memory `dclutch-web-aliveness-patterns.md` DESIGN STRIKE entry.
2. Harvest each lane report as it lands; route load-bearing facts first
   (route-before-ritual); land follow-up lanes where sized work unblocks.
3. PUBLISH-4 cut once GRICE + DESIGN + TRADE-4 land: minimal copy, the
   redesign, and the traded market ship together; three-layer verify;
   credential sweep first. Then cohort-8 if warranted; morning report
   by 10am.

## For ember: planned but not launched (the list you asked for)

1. **General runtime-dispatch** — invocation V3 + accelerator bank paths;
   THE critical path: 12/14 actions are authored but none can execute
   until this. Then the candidate pair. (weeks-class)
2. **Effect-kernel AOT** — the real 164k-CU lever (transition-AOT
   measured near-worthless: 10,393 CU).
3. **Structured issuance session-split** — K=3 is packet-unissuable; the
   record-bound lift was measured useless; splitting issuance is the lift.
4. **Spline wire commit** — evaluator ported + corpus green + rounding
   ruled; remaining: schema-id bump + DCLTPGT1 slot.
5. **Funded FailNext recovery** (ruled: one-attempt is NOT forever;
   pre-mainnet mandatory). 6. **CEILINGS** (9 programs). 7. **GRICE-2**
   (operator consoles). 8. **CREATE-WIZARD**. 9. KAPPA-CAP, SEMANTIC-
   OWNER, AGENT-ARTIFACTS, P-007 seal emission, FRACCHECK-3 (9 commits),
   AlreadyCurrent dedup (5 sites), tail-model reconciliation (60x).
Pending only-you: Helius key rotation; the vm_compressor=2 boot-arg call.

## Night worklist (from BACKLOG's ledger digest, 03:5x)

Spawned: GEN-SEVEN (General order placement, worktree, L), TRIPWIRE (0017
per-family continuation test, S/M), TICKETCLI-2 (CLI ticket author, S/M),
BASIS-D (option-D wire-free front: Lean owner + corpus + de Boor port,
worktree, M/L), DESIGN (site typography, on GRICE's landed tree).
Ruled tonight (WAVE.md cf04501b): basis authority = option D; recovery =
one-attempt is NOT forever (funded FailNext chartered post-cohort-8).
Dissolved: D2-CONST already on lane/fee-tx2 (FEE-TX2 told it owns landing
the whole fee stack — fee-core's base never reached main).
Also spawned (04:2x, from SPELUNK's 20-drop haul): GATES (public-repo CI
never ran green on main + live-tree tiers + 4 orphaned reds/scripts +
VALIDATION_BACKLOG's demoted-tier references), AOT-MEASURE (the 33.9%
lever, never measured — U-014's first number). Routed: TRADE-4 told only
CategoricalQ1 founds (variety = semantics/buckets/shapes, never basis);
SIMLIFE-3 told to build the spend-based kill before devnet activity.
04:3x: 429 session-limit wave killed 13 lanes mid-flight — ALL resumed
warm per protocol; SIMLIFE-3/HYGIENE survived; both auxiliary sweeps had
finished (claims-route map + narration sweep, exports in scratchpad/*.md).
Queued (in order): RETIRE-1 (first full wind-down — spawns when
FINALIZATION lands the fill; fold in direct_begin_retiring_v1's missing
on-chain test); VERIFY-THEN-DROP ledger amendments; CREATE-WIZARD
(/create, behind DESIGN); KAPPA-CAP (worktree, cohort-8 rider);
SEMANTIC-OWNER; AGENT-ARTIFACTS; GRICE-2 (operator consoles' copy +
narration-sweep findings: scratchpad/narration-sweep-findings-20260831.md
— routeCensus meanings fix upstream in Rust doc comments); P-007 seal
layout emission (BASIS-D's template applies); continuation-test port (~20
tests off the demoted route); driver→kernel found() conversion (3-5h —
HOLD until SIMLIFE-3 lands, shared driver files); cliff-doctrine design
pass (Fable-class); FeeSole frame retirement (after fee stack lands);
codex's 605-line sha256 patch adjudication; sccache/workspace
consolidation (post-wave, REPO-scoped only); risk-roll design lane.
Morning report must include: the "planned but not launched" short list
ember asked for (never delivered) + Helius rotation reminder.
Deferred (do not re-litigate): protocol revenue, fee rates (M-26 ember's),
mainnet, CFTC, assurance park, dead-market deletion, monolith benchmark.

## Done-log (07:4x additions)

- GRICE-2 done (6 commits, web 1231/1 vs 1227/1 baseline): all eight
  consoles + explorer catalogue at both bars (~82 strings, ~100 CSS
  rules to tokens, zero sub-13px sentences). The routeCensus fix was
  upstream-but-not-as-predicted: the census tool flattened doc
  paragraphs — one fix cleaned four layers AND moved every refusal's
  REMEDY into its summary line (a refusal a reader can't act on is a
  mystery); Rust docs untouched. Two over-cuts self-caught and
  restored. Sole red: sbomVerify awaiting LIFECYCLE-REDEEM's lock
  commit.
- CEILINGS done (5 commits): ZERO hand-named refusal ceilings left —
  38 sites (the class was bigger than nine: shape-sweep found four more
  + one enum with NO ceiling a BAND_SPAN grep structurally couldn't
  see); red-proved on trading where FEE-TX2's gate compiled a planted
  16th variant GREEN; 263 codes before = after; 0 frame diagnostics.
  Debt named: occupancy walks' hardcoded offset lists (design — the
  sub-band table becomes registry-readable).
- ★ THE MARKET RESOLVED (LIFECYCLE-PAYER, 3 commits): Execute landed at
  EXACTLY the derived 1,220/1,232 bytes (VDA1g6wd…, 323,836 CU), Core
  ACCEPTED the verified terminal receipt (58qGA2MX…, 91,264 CU). Wall 10
  = 3 measured coupled changes (+96 bytes exact; the 49th row reproduced
  the failed key byte-for-byte); walls 11/14/15 fell behind it. Life
  table 35→44 acts CONSERVED. Stops RULED by orch: wall 12 (irreversible
  Registry record — approved on the scratch substrate) + walls 13/16
  (chain's hashed form is authoritative; evidence author fixes) →
  LIFECYCLE-REDEEM spawned: redemption + retirement + THE COMPLETE LIFE.
- FEEFIX done (fcc8b733, 3/3 local + runner exit 0): my handed suspects
  INNOCENT (host-side tools can't reach a program-test); cause =
  b74fabb1 retiring Custody slot 2 — the probe died in its own caller at
  3,158 CU and measured nothing. Rewired onto project_direct_fee_request
  _v1 (the sixth-seed single-builder rule) + a decode tripwire; tx2 now
  reaches Custody at 147,749 CU. CI row greens at the next subtree cut.
- ★ PUBLISH-5 LIVE (de399bf92, pages 33387895228 green, zero console
  errors): market22 renders as THE TRADED MARKET — /live links the exact
  trade signature, the explorer decodes it from chain (78 accounts).
  Caught a real hazard: the shared checkout's agent branch would have
  clobbered main — cut from origin/main in a disposable worktree.
  GATES's fixes greened the compute-margin row. Its two attributed
  live-tree debts both taken: seam restatement FIXED BY ORCH (964549dd —
  seeds from their owner + one new guard tripwired; seam PASS) and the
  3 fee-settlement reds → FEEFIX spawned (attribution open: host-side
  commits shouldn't touch a program-test).
- ★★ THE FIRST PUBLIC TRADE IS ON CHAIN (TRADE-7): sig 4YQLY9ts…, slot
  490,907,340, err None, 1,309,797 CU — local replay predicted it within
  115 CU. Fee settled by A STRANGER (participant-2, tx2 64c5Ev8T…,
  fee_owed 9,950→0) — the permissionless property live on a public
  chain. Conservation cell-by-cell: 10,000,000 before = after.
  Conviction: ONE BYTE — custody bump hint mined for the zero-fee slot
  shape; correct for every fill ever assembled, wrong for the first
  real fee (every operator fixture was economically zero-fee). Fixed +
  two-sided red-proof (ffa73ced); devnet fee-settlement arm reviewed +
  approved (genesis-authenticated, --i-mean-devnet). Stranded cohort-6
  seal UNCLOSABLE (ZeroBump body; deployed refusal CORRECT) — cohort-9
  queue. → PUBLISH-5 spawned: the trade reaches the site.
- LIFECYCLE-EXEC done (2d28333b): the four-receipt gate NEVER MOVED —
  the wall was at checkpoint load; solved by ADOPTION (each carried
  receipt re-verified from chain: 12 live tamper-refusals + a passing
  control; its own wrong first Ruling 4 caught by the real receipt);
  execute union 40→48 rows, reprovision ran green key-free. Life still
  18 acts: WALL 9 (producer+provisioner assert a fresh life) restated
  per-stage in §7.11, correctly left undone → LIFECYCLE-TABLE spawned
  (the §7.11 relaxation + execute → redeem → retire).
- ★ TRADE-6 done: ChildFrame CONVICTED at direct_inline_route_v3.rs:2212
  (the wall-10 fix reached only the producer; both hand-written models
  were wrong TOGETHER and agreed — "half a duplicate is worse than
  none"); fixed host-side, red-proven, no cohort-9. Nine of ten ladder
  stages; THE FIRST CAPABILITY SEAL EVER ON THIS DEVNET (2KhDV1DT…,
  720,187 CU — DCLTSEL1 proven on chain); market21/22 fixtures live
  (3127faf1). New distinct wall: 0x4001 Release at 771,347 CU in
  SIMULATION with every reachable candidate eliminated — an honest
  contradiction → TRADE-7 spawned with the instrumented-ELF replay
  method. Found: exactly ONE stranded seal (6hDpsgAo…, 7.6M lamports)
  and CloseSeal has NO host caller anywhere — TRADE-7 builds it.
- ★ LIFECYCLE-CLOCK done (df89640c, flagship 27/27, workspace 538/538):
  §7.6 band derived-then-built (per-FIELD exemption — only the 40 data
  bytes released, four fields keep byte-pins; monotonicity for the
  epoch fields; endpoints only from still-pinned rows), red-proofed both
  directions ON A MOVING CLOCK. RESOLUTION SUBMIT LANDED (4j9ipdXYKq…,
  slot 79,334, 155,790 CU; lifecycle = Submitted). Walls 6+7 fell too —
  wall 7 was a FABRICATED-VACANCY bug (observed_or_vacant read
  never-fetched as vacant; now refuses unobserved keys). Life table 18
  acts CONSERVED +0/+0. Wall 8 sized to the byte (execute 1,351 vs
  1,232 — nine extractable addresses missing from a frozen table)
  → LIFECYCLE-EXEC spawned (§7.9; the stage-gate relaxation designed
  first, guard preserved).
- LIFECYCLE-ALT done (4 commits): the deadlock broken RIGHT (reclaim
  floor pinned with two derived bounds, guard's refusals red-proofed
  live); §6's wall 1 WITHDRAWN (driver impatience — receipt landed on
  invocation one); wall 4 fixed (PacketTooLarge at 1233 vs 1232 — one
  byte, payload-less error); flagship ALT route END-TO-END for the first
  time (11 receipts, abandoned table closed, nothing stranded); 17-act
  table CONSERVED +0/+0. Life stops at WALL 5: the durable prestate
  byte-pins SysvarClock — provable only where time freezes; §7.5 designs
  the semantic band → LIFECYCLE-CLOCK spawned to implement + complete
  resolve→redeem→retire + THE COMPLETE LIFE TABLE.
- ★ COHORT-8 IS LIVE ON DEVNET (TRADE-5): all five roles at slots
  490,814,947–490,849,793 (resolution AlreadyCurrent — the new evidence
  kind, red-proved, its negative probe caught its own bug), deployment
  set 97d49888…, THE SEAL FIX DEPLOYED. Two vol-derived markets founded
  from measured $102.54 spot (market21 5w24EmP7… 6-cell, market22
  8Xky2yx3… 4-cell), activated, admitted, delegation == debit exact.
  Floor gate 1,263,176 with honest slack; TRADE-5 declined to quote a
  tail probability (its model vs cohort-7's published number disagree
  60x — reconcile queued). Spend 1.0694 SOL. THE TRADE PARKED at an
  unexplained ChildFrame on BOTH markets (width + encoding refuted;
  identical assembly passed locally) → TRADE-6 spawned: differential
  forensics local-vs-devnet, then fire it + seal closes + fixtures.
- REDPAIR done (693682de, suite 26/0, ELF byte-identical): late_custody
  = HARDENING (410320ac's pre-CPI crosscheck provably subsumes every
  destination class) — re-aimed onto an uninitialized Mint, a genuinely-
  late Custody refusal; nonselected_claims = code-corrected to Commit
  (39b75718 moved the class) with a sibling control proving no blanket
  shift. Map facts: Custody 0x6006 propagates VERBATIM; slot-pinned
  fixture env panics one fee-pair case (cache, not refusal).
- ★ EVIDENCE-BRIDGE landed (5ab322da/1dc1c362, 532/532, 14 red-proofs +
  7 on-chain both-ways): resolution goes CANNOT-BE-PLANNED → PLANNED.
  Design docs/design/EVIDENCE_REFRESH_V1.md — a collector-authored
  refresh generation over the advanceable set, eleven immutable pins
  byte-identical, extended-not-relaxed. Convicted en route: 13 pins not
  12 (claims_admission — admission mutates too), and direct_capability_
  root names TWO addresses (one provably empty forever — the all-or-none
  could never pass by construction). Behind it: 3 sized ALT-provisioning
  defects incl. a real deadlock (clock-tracking resume identity vs
  512-slot ALT expiry) → LIFECYCLE-ALT spawned to fix + complete the
  life (resolve→redeem→retire, full closing table). Substrate died on
  its own at 04:54, restored intact from ledger, conservation +0.
- SIMLIFE-3 done (20 commits, web 1228/0): the intricate world REAL —
  8 markets founded+activated on loopback, 288 censuses, conservation
  100%, settling histogram NON-DEGENERATE (heaviest 37%; root cause was
  units: price atoms vs cent cuts — the "same bucket" disease convicted);
  spend ceilings built; /population shows the run for ember's morning.
  Confirmed independently: WALL 7 (no second trade) + the evidence wall
  ("resolution produced against a market since ADMITTED to" — routed to
  EVIDENCE-BRIDGE: the refresh covers the mutation CLASS). 17GB reaped.
- PORT done (50a77e64/cb8a2447, suite 21/4 → 23/2): 8 tests ported
  (exhausting cases → GREEN ~1.30M), 13 stayed (continuation is their
  subject), nothing weakened, ceiling unmoved. Growth attributed: the
  SHARED hot path grew ~40k; top-level absorbed it via 0017-B's headroom.
  2 route-independent reds adjudicated-not-papered → REDPAIR spawned
  (early-refusal hardening-or-regression; Commit-vs-Transition code).
  Option-B retirement debt inventoried (3 files + tooling couplings +
  one dead leaf).
- ★ GEN-SEVEN-3 done (4 commits): GENERAL AT 12/14 ACTIONS — order wire
  repaired while free (fixed offsets + masked split digest), PlaceOrder/
  CancelOrder/ReleaseOrder authored with real escrow economics (15
  execution theorems, mutation witnesses each; 29 frame diagnostics → 0
  en route). Choices 6-10 in WAVE (1978849a). Critical path to execution:
  the runtime-dispatch unit (invocation V3 + accelerator bank paths) —
  weeks-class, THE top post-night charter; candidate pair behind it;
  CloseCandidate tag-14 ruling at the fourteen-cut.
- GATES triage closed (55f3bf9a live-tree, bcc4a2920 public main): both
  probe runners' macOS-only mktemp paths portable; postjoin cases routed
  to their own suites row (a demoted route must not gate the production
  tier — the right call); both inverted annotations rewritten ("if
  direct_hot_top_level is green the margin is intact"). Coordination
  routed to PORT.
- ★ FIRST FEE SETTLEMENT ON A LIVE FILL (RETIRE-1, slot 19651, 178,649
  CU): fee_owed 500,000→0, delegation spent to exactly zero, nonce gate
  lifted, verified three ways; built the missing DCLTDFS1 caller route.
  Closing table: net drift +0 across founding→fill→fee; mint-wide +0.
  begin_retiring test landed (35a40233). WALLED at resolve: activation
  creates the execution root OUTSIDE campaign evidence, nothing bridges
  (sanctioned path proven nonexistent; declined to forge/relax) →
  EVIDENCE-BRIDGE spawned (design-first refresh generation; then drive
  resolution→redeem→retire on the preserved substrate). Substrate ledger
  grows ~10GB/hr (~2 days runway).
- ★ PUBLISH-4 LIVE (38579989d → pages 33369083491 green, 35/0 three-layer
  verify): strike-five copy + full typographic rebuild + AGPL footer on
  clutch.dregg.pro; `checks` workflow FULLY GREEN for the first time.
  Its correction surfaced: the DEMOTED continuation route now exhausts
  1,399,850 CU at 4 outcomes (production margin gate PASSED same run —
  scoped to the harness-only tier) → PORT spawned (the packet-chartered
  ~20-test port; compute rescue stays unchartered). CI triage fixes
  routed to GATES (macOS mktemp path, POSTJOIN wiring, inverted
  annotations).
- direct_begin_retiring_v1's FIRST execution: 98,804 CU, 4 tests green,
  refusals 0x4002/0x4003 with signer discriminated BY COMPUTE (7,280 vs
  48,100); RETIRE-1 folding + driving the lifecycle.
- DESIGN-2 done (7 commits): the type system enforced across all 9 pages
  (mono=values, sentences sans ≥13px); THE SVG FIND — viewBox scaled
  chart labels to 3.2px on small panels, now measured-11px everywhere
  (useFigureScale); /population's 237px mobile overflow → 0 on 8 pages ×
  2 widths; nav 200px → 154px; market cards capped 620px; AGPL Source
  footer live site-wide. 32 before/after screenshots in scratchpad/design
  for ember. Its 2 attributed reds fixed by ME (20d2a9d7 — mirrors
  regenerated after the ticket-crate move; 18/18). Copy leaks on
  /population routed to SIMLIFE-3 (register notes render verbatim).
  → PUBLISH-4 SPAWNED: cut tonight's site to clutch.dregg.pro + first
  green public CI. Second cut follows the devnet trade.
  Queued: ChainExplorer.tsx 3 real TS errors under BigInt noise;
  mobile-nav affordance; operator consoles' type rules (GRICE-2).
- ★ THE FIRST FILL EVER (FILL-2, ~08:5x): sig 4hse1dNh… slot 7576,
  1,282,624 CU, conservation net EXACTLY 0, fee accrued as fee_owed
  500,000 on the maker root (the two-tx design proven on a fill); seal
  fix VERIFIED on chain (739,722 CU where 0x4008 died); walls 3/7-half/10
  down (wall 10 found BY the fill: producer said payer, chain says
  RentCredit refund wallet — fixed). Substrate preserved at
  ~/jobs/dclutch-fill2 (RPC 42888; SIGSTOPped PID 5377 watchdog hazard).
  Routed to TRADE-5: use current-main host tooling for the manifest.
  → RETIRE-1 SPAWNED: settle the real fee debt (first tx2 on a live
  fill), resolve, redeem, retire, closing conservation table +
  direct_begin_retiring_v1's first on-chain test.
- Cut wall + review (~08:3x): resolution's bytes are ALREADY the
  candidate (fee stack never touched it; 815,128B chain dump matches
  digest both sides), and the journal walker refuses every role behind
  a receipt-less already-current one. TRADE-5 refused to self-edit the
  checker; orchestrator REVIEWED + APPROVED a narrow AlreadyCurrentV1
  evidence kind (finalized-slot dump digest, red-proof both ways, never
  masquerades as a receipt, re-verified at activation). Custody upgraded
  (2oY5To6x…); ladder resumes.
- COHORT-8 CHAIN PHASE BEGUN (~08:1x): candidate dfb41be6, checked
  release green (ea7df51a…), floor gate GREEN 1,263,176 vs raised pin
  1,264,676 (51-CU red attributed to FEE-TX2's zero-slack test pin,
  control-reproduced, raised itemized per precedent). Custody upgrading
  (role 1/5); then resolution/claims/trading/core → publish → activate →
  refound 50bps → seal (first on-chain DCLTSEL1 test) → THE TRADE.
- GEN-SEVEN-2 done (1efac500/42c0a631/3250af18, all on main, Lean
  127/127, adapter 226/226, zero frame diagnostics): register bank
  widened ONCE for all seven (90→151, 40→45), OpenBatch+CloseBatch fully
  authored with execution theorems + mutation witness, accelerator heap
  un-tipped. Five choices recorded in WAVE (e46afa56). Five actions
  remain, two walls sized → GEN-SEVEN-3 spawned (order-record wire break
  while it's FREE, then the three order actions + escrow legs; candidate
  pair only if night allows).
- LIFT-1312 done (3be5072c): stop-condition fired HONESTLY — the 1,232B
  packet binds below the record bound (Structured K=3 already unissuable
  at 1,357B full-width; the lift would mint never-issuable descriptors).
  Landed: the four-author derivation replacing the bare literal, the
  coordinate cap solved from the formula, wall ordering as a checked
  assertion; release identity UNMOVED. Doctrine corrected (7e563666).
  Queued: session-split Structured issuance (the real K lift); operation
  counts into a Solana-free crate.
- 33-byte seed fixed (8af9e5fb+bd1370ce): worse than named — first
  derivation would have PANICKED, never caught because no_std claims-svm
  can't derive and no route exists yet. Renamed to 27 bytes + const
  asserts over all four claim-check domains (red-then-green) + a
  discriminating derivation test. Baseline hand-shrunk (3 debts paid,
  4 tripwires armed, no defect baselined). census+seam PASS — the public
  CI blocker is cleared; the next subtree cut turns it green.
- CompactIntentV2 red fixed (00bb24e8, TICKETCLI-2's own extraction, not
  pre-existing): --all-targets clean workspace-wide. Lesson ledgered:
  crate-boundary moves need --all-targets (test-only imports of a moved
  type escape lib-level verification).
- FEE STACK ON MAIN: a7d50d3a (fast-forward, 7 fee: commits, both Lean
  gates green, fee codec 183/183, hot fixture 18/18, zero overlap with
  held work). TRADE-5 told: PIN COHORT-8 HERE + the cold-worktree Lean
  gotcha (build CompiledPhysical first). Pre-existing --all-targets red
  (CompactIntentV2, operator lib test) routed to TICKETCLI-2.
- BRANCHLAND done: 50 unmerged branches → 3 (all live lanes). 47 retired
  as already-landed, three-way evidence each, tombstones-first
  (d8b1e95e); basis-d LANDED to main as ffdc63f1 (verified twice across
  a 12-commit main move); drift guard tools/branch-census/census.sh added.
  Host repo: 6 integrate/* branches zero-unique, recorded not pushed;
  local main 1712 stale — its one unlanded commit preserved on
  rescue/site-successor-explainer (old static site, likely superseded).
  The two /private/tmp mystery clones died in the 01:42 reboot (2 codex
  commits unrecoverable — morning note for ember).
- GATES done (9 commits): CI runs on public main FOR THE FIRST TIME
  (checks + rust verified end-to-end). Four gates were failing for
  reasons unrelated to what they gate — SBOM (50 phantom license fails),
  compute-margin (unset ELF var), custody (wrong repo root in subtree),
  suites (3 red rows, ZERO tests executed — per-row absence miscounted as
  protocol failure; rows now say NOT RUN). One real defect isolated:
  FRACTIONAL_CLAIM_CHECK_SEED_V1 = 33 bytes, underivable → FRACCHECK-2
  resumed to shorten it (free now, nothing can depend on it). Publication
  cut checklist grows: the next subtree cut turns public CI green.

- FEE-TX2 done on-branch: THE PAIR EXECUTES — tx1 fee-bearing fill
  1,280,996 CU (32/32, margin 104,003; floor 131 CU BELOW zero-fee), tx2
  169,590. Fee wall dead. Three defects fixed en route incl. FeeSource
  (maker A settling from maker B's delegation). Q1 answered: Trading's
  self-attestation is signing its own caller-authority PDA — derivation,
  not registration. RESUMED to land onto main NOW; TRADE-5 told to hold
  the cohort-8 pin for the landing hash (one cut carries seal fix + fee
  protocol; rate diversity unblocks). Queued: lane E (builders/panel),
  refusal mirrors, the unrunnable run-fee-second-transaction.sh.
- AOT-MEASURE done (b56c10f6..8b47f287 + evidence doc): transition-AOT
  saves 10,393 CU/invocation — 0.83% of floor, 29.5% of the (now-moot)
  fee gap. The "33.9% lever" contained ZERO TransitionVM CU; the real
  interpreter is the effect kernel (164,289 CU, 14x, unmeasured) → that's
  the future AOT charter if CU matters again. Debt: the AOT crate has
  never compiled for SBF (175 errors, 2-line cfg fix applied-not-committed).

## Done-log (07:2x additions)

- FRACCHECK-2 done (f3f47640..1aac6f43, 8 commits): the §17.4 burn leg
  PROVED on real bytes across a real SetAuthority; split-controller
  disjointness non-vacuous (admissions counted); escrow PDA program-
  derived both sides. Size corrected: RetireCoordinate is Trading-composed
  at 1 layer of 3; re-size 17 commits, 9 remain (~3x estimate; the
  48-account frame is a lane alone) → queued as FRACCHECK-3. Its campaign
  caught the /tmp wipe honestly (refused a mismatched litesvm .so,
  rebuilt bit-for-bit against the pinned audit row).

- TRADE-4 closed: NO trade — the deployed Trading ELF omits DCLTSEL1 from
  the heap-profile list (fix 8c216642 NOT ancestor of deployed a93256c1),
  so every seal write refuses 0x4008 on chain; host-side fix verified
  (grant ships), program-side needs a cut. ALSO: zero-fee markets can
  NEVER trade (direct_token_setup_v1 admits only 50bps — market19
  permanently unfillable; founding help was backwards, killed 3 markets;
  fixed 892aaa39/37fc1b91). Built market20 (DQd8WmU2…3rGW, 50bps, 7
  stages, frozen lookup). Spend 0.2366 SOL, deployer untouched.
- ORCHESTRATOR DECISION under the directive: CUT COHORT-8 NOW → TRADE-5
  spawned as sole devnet writer: pin candidate, floor-gate 32 seeds,
  publish/activate, refound 50bps diverse markets, seal write (verifies
  the fix on chain), manifest, FIRE THE FIRST TRADE, then close cohort-6/7
  stranded seals (CloseSeal rides this cut — first close ever).
  market19/20 become compost by standing stranding ruling.
- Queued: panel shows allowance as ceiling where chain demands equality
  (WALL4-species, TS side — after DESIGN-2 lands); GEN-SEVEN-2 job-dir
  hazard flagged by TRADE-4 (already warned).

## Done-log (07:0x additions)

- TICKETCLI-2 done (8e5c6979, b729d592): author descended into new crate
  dclutch-direct-ticket (3 real deps, signers behind `author` feature —
  operator links nothing that can sign); `dclutch ticket author`/`verify`
  work in the dist binary, sha256-identical to the TS vector incl.
  signature; refusals proven through the binary, none leave a ticket on
  disk. Queue: dawn dist cut v0.1.0-devnet.3 (CLI lockfile already
  rippled; dist-workspace needs no change). Named debt: third copy of the
  duplicate-key JSON reader wants a small owner crate.

- BASIS-D done: de Boor port landed on lane/basis-d-20260831 (aac98afd,
  wire-free, 19/28 Lean spline cases exact, 6-of-9 mutation red-proof;
  items 1-2 + kind-tag guard were already landed — verified, not redone).
  RULED (WAVE 76e2ca3f): spline rounding = cumulative-floor (zero-weight
  claims never take residue; measured superior 11 cases). Branch handed to
  BRANCHLAND to land when main's Cargo.lock quiets. Remaining sized:
  overflow envelope (~half day), alloc-free record path, both ride the
  future wire commit.
- /private/tmp wipe warning routed to GEN-SEVEN-2 + FILL-2 (persistent
  paths; BASIS-D lost its worktree to the reboot and rebuilt from git).

## Done-log (06:4x additions)

- CLIFF landed 6cb1269b: every fixed bound classified physics/purchasable/
  session-splittable; lift list ranked. Headline: finalizedRecordMaxBytes
  = 1312, a rationale-free Lean literal below its own account ceiling,
  generates Structured≤3/Rational≤3 and the 42-instruction cap. → LIFT-1312
  spawned (derive the value, regen 4 crates, red-then-green the cliff,
  measure). Also: MAX_OUTCOMES has FOUR authors (unify before widening);
  commit-don't-inline shortlist recorded in doc §4.
- memguard removed at ember's request (policy answer: macOS has no
  kill-don't-thrash knob; vm_compressor=2 boot-arg is the real option,
  needs recovery-mode security downgrade — ember's call, steps on ask).
- TRIPWIRE final: 09c1c8fc/46083e7a covered 2 families on the demoted
  route only — its Core founding-continuation work was NOT a duplicate.
  13-vs-14 resolved: docs were right when written (9c25e741 moved
  founding_v5 to bump-witness); count now carries its re-measuring command.

## Done-log (06:1x additions)

- EMBER STEER (in force): converge forward, integrate towards main, no
  drift, keep main current all night — no "sin-pleasing"/honesty-chasing
  meta-work. Recorded in route-before-ritual memory.
- LEDGER-TRUE landed bc1af4ad: CloseSeal(f253c4e0)/0017-B/tripwire/web-
  bucket CLOSED; fee band + E5 confirmed branch-only (SESSION_STATE
  corrected aea44159); family root-tails NOT landed (wave claim false);
  GEN-SEVEN consequence corrected (7 ACTIONS x 9 records, 68-record
  publication, nothing deployed → cheapest now — routed to GEN-SEVEN-2).
- TRIPWIRE landed 66f95de5: founding-continuation invoke-depth tripwire
  (Core's first dynamic coverage, red-proved via restored CPI →
  ReentrancyNotAllowed) + S-3 case with attribution control; found the
  vacancy predicate guards TWO places (2,589 vs 20,420 CU). Claims'
  13-site helper + Dealer/Rent sized and left (0017 §9).
- FINALIZATION closed WITHOUT a fill: crash wiped validator 43080 + all
  preserved artifacts (evidence survives in DIRECT_FILL_WALLS_2026_08_31).
  Landed 9c386c57..88a4e9c5: 23 named refusal variants (was 1 for 27
  sites), wall 6 fixed+verified, wall 8 fixed UNVERIFIED. Convicted:
  delegated_amount must EQUAL debit (single-use). Wall 7 debt: producer
  admits only vacant seller token → no market trades twice. Both routed to
  TRADE-4 (its trade + second fill depend on them). → FILL-2 spawned
  (restage, fix wall 7 + probe delegation, land the first local fill).
- BRANCHLAND spawned: adjudicate ~20 unmerged branches (land/retire-with-
  tombstone/hold), per the no-drift steer.

## Done-log (05:4x additions)

- THE MACHINE CRASHED (Chrome OOM → swap-lock; ember rebooted, went to bed).
  All 15 lanes died; 13 resumed warm from transcripts, SIMLIFE-3 restarted
  its world run, DESIGN's transcript was LOST — respawned as DESIGN-2
  inheriting its landed commit ab1e9c36 + uncommitted WIP (globals.css,
  layout.tsx, SiteFooter.tsx). Heavy builders throttled (nice, -j4).
- memguard installed at ember's request: ~/bin/memguard +
  ~/Library/LaunchAgents/com.ember.memguard.plist (running, pid-verified,
  victim selection dry-run matches live renderers). Kills largest Chrome
  renderer at 15s sustained critical pressure; never anything else; logs to
  ~/Library/Logs/memguard.log; removal one-liner in the script header.
- FINALIZATION told to verify validator 43080 survived the reboot before
  building on its substrate (likely did not — restage path named).

## Done-log (05:0x additions)

- HYGIENE closed 8/8 (abi:general-v5 was TWO defects incl. a TS requireZero
  that refused legal mined bumps; twins gate generalized to 130 pairs; SBOM
  red retired; 264.6 GiB reaped). New class finding queued: CEILINGS — the
  hand-named refusal-band ceiling stands in NINE other programs. lane.sh
  gap: cannot express a rename. New rule: diff each named path immediately
  before committing (named paths necessary, not sufficient).
- Killed 6 orphaned buffer-writer lease shells (62h, PPID 1, test debris);
  bounded the wait at its source (upgrade.rs, 900s → loud refusal, 6459c025).
- GEN-SEVEN refused correctly with a byte-verified sizing: the seven General
  actions are Lean-gated quadruples; bank widening re-digests all seven
  deployed settlement artifacts (byte 12). → GEN-SEVEN-2 spawned on the
  strongest model to author them (worktree; re-activation rides next cohort).
- CLOSESEAL landed E3 as code (60a21da6): stranger closes stranded seal,
  keeps rent, 5 real-ELF cases, S-3 tripwire, hostile codes 0x4009-0x400B.
  FOUND+FIXED: seal-WRITE outer dead since 08-30 (0x4008 unconditional,
  DCLTSEL1 never heap-declared) — hypothesis routed to TRADE-4 (may be its
  devnet refusal; remedy = cohort-8 cut, within its authority). First
  cohort-6 seal close queued to TRADE-4 via blocked.json.
- Fixture-mapper harvested → TRIPWIRE (13 sites not 14; founding test fills
  CORE's hole, not the shared helper's; waist lacks Rent/Resolution).
- Queue additions: CEILINGS (after FEE-TX2/FINALIZATION land — trading files);
  fee-core worktree reap (14.7GB, FEE-TX2's word at landing); lane.sh rename
  support; 2 mystery clones (8 commits, codex remote) for ember's morning.

## Done-log

- 02:00 goal adopted; wave already saturated (10 lanes + 2 sweeps), no idle
  capacity to fill without colliding in the shared tree.
- 02:30 ember design strike on /markets (type scale/imbalance) — recorded in
  memory, GRICE told to land fast, DESIGN lane queued on its tree.
- 03:05 ember steer: interesting/granular markets — SOL/USD always resolves
  into the same bucket. Rule routed to TRADE-4 + SIMLIFE-3: buckets centered
  on spot at founding, width ~ vol x window (genuine ex-ante uncertainty);
  vary question types; simulator treats one-bucket dominance as a bug.
- 02:50 EXPLORER landed (13d9359c): 53 summaries + 21 notes rewritten, 9
  notes deleted, 3 test pins renegotiated stricter, 177 in-scope green.
  Routed: HYGIENE's f346ba81 broad add swept EXPLORER's file — warned.
  Known: 10 web test files red from other lanes' in-flight strings
  (mostly GRICE's surfaces); full-suite green is PUBLISH-4's gate.

## Wave 3 — 2026-09-01 ~11:40, "swarm wide on protocol/frontend/etc completeness"

CI triage first (run 33501820165, at cut bfcdc390d). `checks` GREEN for the
first time. `rust` red in two jobs, and neither red is what its job title says:

- **programs tier**: the margin gate is GREEN — margin intact, not a compute
  finding. The red row is `--test activation`, 10 passed / 4 failed. Three die
  at `assert_activation_succeeds` with `Custom(16387)` = `Content` 0x4003; the
  fourth *correctly* expects `Root` 0x4002 and gets `Content` instead. One
  over-eager Content conjunct firing ahead of the tail-width check explains all
  four. → lane ACTIVATION-FOUR.
- **suites tier**: 2 of 7. `dealer` ran ZERO tests — stale lock, `--locked`
  refused to update it. That is the THIRTEENTH instance of the class, and it is
  **already fixed locally in 2213d16e**, just not cut yet. `postjoin` control
  still red = the known compute wall, ember's ruling.

Eight lanes launched (three from wave 2 still live: FIXTURE-DRIFT, LEGACY-V4,
BUY-PROJECTION):

| lane | row | territory |
|---|---|---|
| WEB-C12 | C-12 | `apps/dclutch-web` |
| SDK-CLI | C-12/13 | `packages/dclutch-{sdk,cli}`, `tools/dclutch-cli` |
| DEAD-ROUTE | C-00 | census: unraisable codes, undispatched routes, stale guides |
| RESOLUTION-C09 | C-09 | resolution-proof, pyth, source-* |
| GENERAL-C05 | C-05 | general-accelerator + general-hot |
| ACTIVATION-FOUR | C-04 | trading src (not dealer/) + `tests/activation.rs` |
| STRUCT-FRAC | C-08 | structured-v2, fractional-*, claims-sbf |
| RELEASE-DEVNET | C-13/14 | tools/release, ci, sbom, local-validator, devnet-* |

Contract coverage after this wave: every row has a lane or an adjacent lane
except **C-02** (compiler-shaped product entrance) and **C-11/C-15**, which the
contract itself says need ember's rulings before implementation.

### Wave 3 landings

**S5/FIXTURE-DRIFT** — convicted and fixed the four `activation` reds before the
lane I spawned for them arrived. My hypothesis (over-eager `Content` conjunct
shadowing the tail-width check) was the right shape, wrong mechanism. Real
cause, one line, `programs/dclutch-trading-sbf/src/outer.rs:2178` from
`7c7ad184`: the root-bump write took `Some(bump)` unconditionally and did
`scalars.get_mut(8).ok_or(Content)?`. A descriptor on the exact V2 8-scalar /
12-identity legacy bank has no slot 8, so **every legacy-bank descriptor was
unactivatable**, refused before the profile ever projected. Three authorities
say the ninth scalar is opt-in and the code contradicted all three. Repair
`d969d8f7` gates the write on `scalars.len() >= ACTIVATION_COMMON_SCALARS_V3`.
`--test activation` 10/4 → **14/0**; pinned canonical artifacts byte-identical.
Fifth red separately: the *test* was wrong, not the encoder (`04ad267d`).

This is THE CLASS in mirror image — a documented compatibility promise
("V2's exact 8/12 bank remains the complete legacy ABI") that had never once
been executed against a real 8-scalar descriptor, so the code quietly stopped
honouring it while every test stayed green.

**S6/LEGACY-V4** — five reds, three separate causes, none of them a fixture
drift: `Effect` ×3 (`5fbe0bd8` rebuilt the Consume effect as a root-independent
template and updated V5 only, leaving `release_v4` and the lab generator with
`[0x44; N]` filler), `Successor` ×1 (identity bank grew 1→6→9; two fixtures
still passed six), `Lifecycle` ×1 (`73ffb010` added `validate_plan_permissions`,
and the Series Consume profile grants no lamport permission to any coordinate
but root and Ticket). Fixed in `91e7c2ba`, production code untouched; `series::`
110/5 → **115/115**. The lab generator — not a test — was broken too and now
runs end to end, byte-identical to Trading.

### Open for ember, from these two

1. **The legacy eight-scalar bank cannot express a funded activation.** In an
   8-wide bank every scalar is common, so a funded activation must destroy a
   seeded register — which is why every codec-built bundle declares ≥9. Widening
   the one fixture that still exercises the legacy bank would change pinned
   canonical artifact bytes *and* retire the tree's only executed instance of
   that bank — the coverage that just caught the defect above. Whether V2 keeps
   a funded case is an ABI decision, not a lane's.
2. **`73ffb010`'s reach is wider than Series.** Any family whose AccountProfile
   omits lamport permissions can no longer carry a `Create` or `Close` plan at
   all. Already costs the `TicketAuthorship` pin its payer and RentCredit arms
   (now defense-in-depth only). **No family sweep has been done.** → routed S11.
3. **`Content` (0x4003) covers ~20 raise sites on one route**, which let a test
   pass vacuously for days: the refusal it meant to catch and the one it caught
   share a discriminant, so no assertion inside it could have caught the
   vacuity. Granularity decision → routed S3 (the site) and S11 (the class).

All 11 lanes re-routed to the letter's own S0–S11 briefs — I had been running
paraphrases of contract rows instead of the written registry. C-02 (the only
row with no lane) is queued for the next free subagent slot.

## RULINGS AWAITING EMBER — consolidated, 2026-09-01

Only decisions that are genuinely ember's. Two items that arrived as proposed
rulings were sent back as engineering, and are recorded at the end so nobody
re-escalates them.

**From the activation work**
1. **The legacy eight-scalar bank cannot express a funded activation.** In an
   8-wide bank every scalar is common, so funding must destroy a seeded
   register — which is why every codec-built bundle declares ≥9. Widening the
   one fixture that still exercises the legacy bank changes pinned canonical
   artifact bytes *and* retires the tree's only executed instance of that bank,
   which is the coverage that caught the defect. Does V2 keep a funded case?

**From the S11 debt/ownership census**
(`docs/evidence/DEBT_OWNERSHIP_LEDGER_2026_09_01.md`, commits `77ea7a6e`,
`3e21c006`)

2. **R-2** — `LiabilityBasisSbfErrorV2` keeps **10 unraisable discriminants**
   after the `DCLLBX02` banishment. Shrink (a decision-0007 sub-band
   renumbering touching every mirror), or annotate in place?
3. **R-3** — **Claims split/merge as user acts.** `claims.conserve` /
   `DCLCNS01` is the tree's one true orphan magic, and `CustodyRequired
   = 0x5006` is dead for the same reason: the outer route was never built. The
   owning crate says so itself and the SDK already publishes it as a wall, not
   a claim — so this is honest, not a stale claim. Build the route, or rule it
   out with a date.
4. **R-6** — U-001's explicit deletion / non-authoritative-AOT ruling for
   standalone family artifacts.
5. **R-8 — C-15 proper.** Does the accepted final public project include the
   FHE / MPC / specialized-batch / energy objective? **Nothing exists in code**
   — zero of the letter's eight charter items has any foundation. The
   2026-08-27 ruling ("dark-FHE is NOT a near/medium-term ambition") is a
   *horizon* ruling, and the contract says at `:175` not to infer from the old
   horizon park. Retained → a from-zero charter. Ruled out → a dated ruling
   plus removal of the contradictory claims.
6. **R-9** — either way: record *"the batch relation is small and specialized
   **on purpose**"* (`INTENT.md:118-120`) as a named `O-*` invariant with its
   reason. No invariant records it today, so a future "simplification" closes
   that door silently whichever way R-8 goes.

**Sent back as engineering, not ruled — do not re-escalate**
- **R-5, refusal granularity.** `Content` 0x4003 carries **2,086 raise sites,
  25.0% of the protocol total** (780 in `hot_v3.rs`; Trading 62.5% concentrated
  on one code). But **Claims already solved this class in-tree**: 1,489 sites
  over 141 codes, 15.2% max, via sub-bands. A policy with a working precedent
  inside the same repository is a decision with a template. → S3 (the site),
  S11 (the class).
- **R-10, "is the first market open on devnet?"** Six guides disagree and
  `deployments.ts` pins no market address. Answerable by measurement, not by
  ruling — S1/S10 is standing up a fresh cohort under the disposability ruling.

**Still open from earlier sessions**: the `derivation_policy` migration
(release event, protocol-wide); registered-Direct routing; Structured account
widths (proven, needs a release event); recovery ontology keep-or-cut; pinning
an exposure digest at founding; and the C-11 economic rulings the contract
itself says must precede implementation.

## ACTED ON UNDER YOUR EXISTING RULING — reverse this if I read it wrong

**`derivation_policy` is unblocked, and it was blocking item 1 of the letter's
entire continuation order.**

The Dealer wall is now convicted to exactly one predicate:
`descriptor.derivation_policy != entry.child_derivation_id()` in
`validate_selection`, reached from `authenticate_descriptor_root_selection`
(`hot_v3.rs:3319`). WAVE had reserved that predicate — *"EMBER RULES THE SCOPE;
no lane may start it"* — **because** changing it moves every capability-root
PDA, so existing markets cannot be migrated in place and must be re-founded.
Verified in source today, not inherited: the field is not a direct PDA seed,
but the `manifest` seed carries `child_derivation_id` and the
`capability_release` seed transitively covers the descriptor bytes at offset
144.

Migration cost was the whole reason for the reservation. Your 2026-09-01 ruling
answers it: devnet is disposable, tear down and redeploy fresh, abandon the old
cohort in place rather than migrating it. There are no mainnet markets. Under
that ruling **"existing markets must be re-founded" is not a cost, it is the
plan** — so the objection that reserved this predicate has been answered by the
person who reserved it.

So I gave the lane this authority and no more:
- **implement it and prove it on real ELFs as code**, on the only branch that
  extends (branch A is unrepresentable: `entry_index` is a write-once PDA seed
  and `validate_manifest` needs entries strictly ascending by `kind_id` while
  all nine Dealer selectors share one kind);
- **state explicitly what still binds the descriptor to its entry** once that
  comparison stops carrying the binding — if the answer is "nothing", it is a
  weakening and the lane stops;
- **no deploy, no push, no tag, no devnet SOL.** The release event stays your
  named act.

If that reading of your ruling is wrong, say so and the work is lost, not the
tree.

### Correction to something I told you

My `652900b3` — *"sbom: PASS — 59 manifests, zero unresolvable"* — was **a green
contingent on the dirty working tree, not on the committed state.** A
**fourteenth** stale lock (`trading-outer/Cargo.lock`, seventh workspace missed
by `2213d16e`'s sweep) was proven by detaching a worktree at HEAD and running
`cargo metadata --locked --offline`, which refused with the exact
`STALE_LOCK_MARKER` that `sbom_check.py:401` fails on. The manifest is in the
census at `SBOM.md:2448`. Fixed in `1d04082f`.

The general defect is worse than the instance: **the SBOM gate as run locally
measures the tree; CI measures the commit.** That is not flakiness, it is a
gate measuring the wrong object. Routed to S1/S10.

Also corrected: `reserved_claims is a cap` — which I relayed from an earlier
lane — is **refuted at two sites**. `direct-codec/src/successor.rs:1618` pins
Sell's to exactly `maximum_fill - filled`, `:1631` pins Buy's to exactly `0`,
and `registered_effect_artifacts_v4.rs:309` says so in prose. Exact equality.

### Added to your ruling queue

7. **Four vacuous guards on the sole production Resolution path**
   (`provider_v3.rs:137`, `:139`, `:153`, `:211-212`) compare values against
   things constructed from those same values moments earlier. **None is
   exploitable** — the real binds are `source_material_v3.rs:244` and
   `provider_v3.rs:368-369`. Deleting them changes the released ELF and
   invalidates in-flight digest and compute measurements for zero security
   gain. Keep-with-reason, or delete at the next release event? Precedent
   `a968858c`.
8. **Wall B, Direct.** Costed: an action tag on `LifecycleCurrentRentQuoteV5`
   changes `CURRENT_RENT_QUOTE_BYTES_V5` and every pinned V5 artifact digest.
   Strictly better than a per-action root entry, which would change the
   persisted root header. A union policy is refuted — `current_rent_quote` is a
   flat ordinal array, so a Sell would project the Buy's quotes.
9. **Two binaries named `dclutch`** are real and load-bearing; both now name
   the other for the other's verbs (`c018a9ba`). Merging or renaming a released
   binary is a release event, hence yours.
10. **Ten duplicated 8-byte magics tree-wide**, two of them live top-level
    selectors of the *same* Trading ELF (`DCLTDRS1`, separated only by data
    length and dispatch order). Renaming a released instruction is a release
    event. `--check-unique`, which `AGENTS.md` names as *the* gate, checks
    refusal bands only — **there is no magic-uniqueness gate.** Gate is being
    built regardless; the renames are yours.

### C-09: two clauses of one row are in tension

Permissionless completion is closed **because** any stranger may execute — and
every stranger who loses the first-valid race is `Submitted` forever, because
reclaim requires `Consumed` plus a certificate a loser never had.
**6,389,280 lamports stranded per loss**, and the clause that closes C-09's
liveness half manufactures the victims of its closure half.
`reclaim_after_unix_seconds` — the field that exists to bound exactly this
wait — is unreachable. Being built as a new instruction, not ruled.

## DEVNET DEPLOY AUTHORIZED — 2026-09-01, standing

Ember: *"don't defer to me, whenever and as often as you feel ready, please
deploy to devnet :) just ensure that you do a full redeploy 😂 including the
load simulator"*

Recorded in `AGENTS.md` (`6a68c9c1`), which previously told every lane it had no
deploy authority. Conditions attached: **full redeploy only** (whole cohort,
exact current sources, fresh identities, old cohort abandoned in place); **the
load simulator runs against the new cohort**, named by ember, part of the
deliverable not a follow-up; and **deploy from a commit, never the ambient
dirty tree** — eleven lanes are mid-edit and a deployment whose sources cannot
be named is unreproducible evidence, which C-14 forbids and which no amount of
devnet success repairs. Still not authorized: mainnet, push, tags, releases.

Groundwork established before handing it to S1/S10, so the lane spends its time
deploying rather than looking:
- deployer `~/jobs/dragons-clutch-devnet-20260819/keys/deployer.json` →
  `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`, verified;
- **43.742833706 SOL** on devnet;
- that key is the **retained upgrade authority on live cohort-8**, so those
  programs are mutable and closable — core holds 7.76 SOL at 1.1 MB, and
  Trading's ELF is 2,285,824 bytes, so seven roles may exceed the balance.
  Reclaiming from the abandoned cohort is sanctioned: ember's ruling is
  literally *"can't we tear everything down"*.
- the default solana config points at a nonexistent local payer and
  `~/.config/solana/id.json` holds 0 devnet SOL — pass `--keypair`/`--url`
  explicitly.

**One question the lane must settle rather than guess:** archived wall W1 says
the real life may need all seven roles revoked **immutable**, and immutable rent
can never be recycled — so recoverable-mutable and permanent-immutable may be
mutually exclusive by protocol design. W1 dates from 2026-08-27; re-measure at
HEAD rather than inherit it.

**This closes "the one act still awaiting ember."** The only remaining external
acts requiring ember are mainnet, push and tags.

### Two more for your queue, both from the Structured lane

11. **Materialize / Dematerialize / RedeemMaterializedTerminal: delete or
    drive.** Constructed nowhere, driven by no test, route labelled
    `ECONOMIC_SLICE_MIGRATION_ONLY`, and the 1,444-LOC
    `dclutch-claims-representation-codec` has **zero dependents**. The lane's
    phrase is the right one: **live, unexercised supply-moving code.** C-08's
    clause is already carried by Reconstitute/UnwrapStructured, so nothing is
    lost by cutting it — but unexercised code that can move supply is a risk
    surface, not neutral debt.
12. **Is a K = 2 product useful?** Executable full width is **K = 2**: at K = 3
    `IssueStructured`/`UnwrapStructured` measure 1,357 v0 bytes with a live ALT
    against a 1,232-byte packet. Selected actions fit at any K. The handoff
    letter says "use a proven K=2 route" — **no K=2 route exists**; every
    campaign in the tree is K=3. So C-08's "useful exact-denominator products"
    turns on whether two-outcome structured products are a product you want.

## The deploy: built, budgeted, held — and the budget decides something

**It does not fit.** Rent measured per role (`890,880 + 6,960·(45+n)`): registry
1.6334 · rent 0.9918 · custody 3.9784 · resolution 5.6052 · claims 9.4924 ·
trading 15.9105 · core 8.2627 = **45.8743 SOL**, ≈**45.9617** with fees,
records, profile and market. On hand **43.7428**. **Short 2.2189.**

ELFs grew ~40% since the old DEMO table (4.72 → 6.59 MB), so the remembered
"45 SOL buys one cohort" is stale.

**Reclaiming cohort-8 is therefore necessary, not optional** — its ProgramData
holds **43.1186 SOL** across seven accounts, measured. Authorized under your
*"tear everything down"* ruling; reclaim gives 86.86 total, surplus ~40.9.

**Mutable / ExactAuthority chosen over immutable, and W1 is REFUTED at HEAD** —
`programs/dclutch-core-sbf/src/infrastructure.rs:311-314` admits
`ExactAuthority`, live tree-wide, so the 2026-08-27 claim that the life requires
immutable roles does not hold. Decisive reason: **your grant is standing**, and
repeatable redeploys only work if rent comes back. Immutable is unaffordable
*and* unrecoverable.

Still held on R-13 — re-lettering changes magics, which changes ELFs, so the
rehearsal pack is not the deploy artifact. Deploy order is
Registry→Rent→Custody→Resolution→Claims→Trading→Core, each deploy→verify, then
nine records published from **observed** slots, profile init, and activation one
role per transaction (five together are 2,396,686 CU against a 1.4M ceiling).

### Reproducibility: all seven ELFs byte-identical

Two independent detached worktrees at `10527ddc`, `cmp` 7/7. SBOM regenerates
byte-identically with a red control (injected drift → `STOP` exit 1).

### The gate that ran nowhere and said otherwise about itself

There was **no `sbom` CI tier at all**; `web` excluded the test citing another
repo's job; and generated `SBOM.md:4` claimed it was *"wired into
`tools/gauntlet`"* when `grep -rn sbom tools/gauntlet/` returns nothing. **A
false claim inside a generated file, reprinting itself on every
regeneration** — the most self-sustaining form of this tree's recurring class.
Fixed in `9a32790b`.

And the gate measured the wrong object: same commit, **dirty tree `PASS` vs
clean worktree `STOP` exit 1**, with a **fifteenth** stale lock existing only
uncommitted (`69a0aa69`).

### Two more named walls

- `tools/release/private-validator-lifecycle/test_preflight.py`: **12 failures
  + 3 errors, ungated** — the `release` tier runs only the four `test-*.sh`.
  Red where nothing looks. To be fixed or gated **before** the deploy.
- **The two `dclutch` binaries overlap only on `help`**, and the near-misses are
  lethal: `market show` vs `markets show`; `--keypair` normal in one and
  *refused by name* in the other; env vars differing by one character
  (`DCLUTCH_RPC_URL` vs `DCLUTCH_RPC`). No doc disambiguates them and **no doc
  under `docs/` teaches building the TypeScript binary at all.** Nothing was
  missing — the earlier near-P0 was right to be doubted — but this is a genuine
  C-13 defect. Renaming a released binary is still yours.

### Your steer, convicted to three lines

*"SOL/USD always resolves into the same bucket"* is now a measured defect with a
gate that refuses it.

**The defect is live**: `apps/dclutch-web/components/CreateMarketWizard.tsx:117-119`
ships `cutDenominator=100` with cuts `12000/18000`;
`tools/local-validator/bootstrap/successor/src/market.rs:12180-12182`
`demo_market_input` is **byte-identical** to it; and
`crates/dclutch-source-contract/src/lib.rs:612` returns raw price atoms
**unrescaled**. The flagship authored market resolves into its top cell **100%
of the time** — those cut values measure as `0 / 0 / 0 / 0 / 10000 bp`.

**The gate** (`95316da7`): characteristic displacement is
`volatility_bps × √(window / 10,000 slots)` of spot, exact integers, with mass
over cells under a named `TriangularPlausibleBand` — triangular rather than
uniform, so a tight-centre profile is not punished for being good. A SOL/USD
market founded today (spot 100,000,000 atoms, 200 bp, one hour) gets cuts
`99,400,000 / 99,800,000 / 100,200,000 / 100,600,000` and shares
**3612 / 900 / 975 / 900 / 3612 bp** — admitted. The historical cent-scale cuts
are **refused `DegenerateOutcomePartition`**. Both in one test, so the refusal
is not a checker that refuses everything.

**Still ungated where it matters**: the live founding path calls the old
entrance, so the fresh cohort would found the same broken markets. Routed to the
release lane as deploy-critical.

### "Exhaustive and disjoint" was self-asserted — including in Lean

Three ways: `recheck` re-derived with the **same** `derive_shape`;
`recheck_categorical_approximation_v3` literally called `certify_…` again; and
the Lean `ResultDomain.selection_disjoint` is
`rw [← leftSelected, ← rightSelected]` — **a function-is-a-function tautology.**
Twelve hand-picked coordinates were the whole evidence.

Now proven by sweep against an **independently written** interval predicate with
permanent controls in the same run — 3,204 + 1,842 + 303 coordinates, every
region reached, a one-boundary convention flip detected 9/400 times (`29f35cdb`).

**A formal proof that restates its own hypothesis is worse than no proof**,
because it carries a formal method's authority.

### Two of C-02's seven outputs do not exist

**Source policy** — no Rust anywhere builds one from a description;
`RecoveryPolicyV1/V2::new` has **zero non-test callers**; production only
`decode`s operator hex. **Funding plan** — no record carries
`{target, deadline, abort}`; the only artifact is a component self-labelled
"wizard placeholders". Not named refusals: **absent constructors**, which is
worse, because a refusal tells a caller what is wrong and an absent constructor
looks like scope.

### Two more for your queue

13. **`MAX_CELL_EX_ANTE_SHARE_BPS_V1 = 9000`** — provisional. 90% refuses the
    convicted defect without refusing a legitimately lopsided binary market. The
    ceiling is a product decision; the constant is only the name the entrance
    defaults to.
14. **Where does the volatility anchor come from at founding?** The authoring
    path has no home for it — `spline_product.rs:44-65` has twenty fields and
    all are geometry: no spot, no window, no volatility. Reading it from the
    source's own founding observation is the obvious candidate, but which
    observation is authoritative is yours.

## I TOOK THE ONES THAT WERE MINE — 2026-09-01

You said not to defer. The queue had grown to fourteen; several were engineering
wearing a ruling's clothes. Decided below, each reversible, each with the reason
so you can overturn it in one line. **Six remain genuinely yours** and are
listed at the end.

**Settled — not actually decisions**

- **The legacy eight-scalar bank (was #1).** Not a choice: in an 8-wide bank
  *every* scalar is common, so a funded activation must destroy a seeded
  register. It is impossible **by construction**, not disallowed by policy. So
  there is nothing to rule — the property gets documented, and the one fixture
  that still exercises the legacy bank **stays**, because it is the tree's only
  executed instance of that ABI and it is what caught `d969d8f7`. Widening it
  would buy nothing and cost the coverage.
- **R-9, the batch-relation invariant.** Recording *"small and specialized on
  purpose"* (`INTENT.md:118-120`) as a named `O-*` invariant is transcription,
  not judgment. It gets written down whichever way R-8 goes — that was the whole
  point of raising it.
- **The volatility anchor at founding (was #14).** The only non-invented option
  is the source's own founding observation; anything else is a constant someone
  made up, which is the failure the partition gate exists to catch. Engineering,
  and routed as such.

**Decided, with reasons**

- **The four vacuous Resolution guards: KEEP, with the reason in the file.**
  None is exploitable, the real binds are identified, and deleting them changes
  the released ELF and invalidates in-flight digest and compute measurements for
  **zero** security gain. A known-vacuous guard kept *with its reason written
  down* is honest; kept silently is not. Reversible at any future release event.
- **Wall B, Direct: BUILD IT.** Same logic that unblocked `derivation_policy` —
  under your standing full-redeploy grant an ABI change costs nothing, since
  there is no state to migrate and the mirrors are generated. The action tag on
  `LifecycleCurrentRentQuoteV5` changes `CURRENT_RENT_QUOTE_BYTES_V5` and every
  pinned V5 digest, and it is strictly better than a per-action root entry,
  which would change the persisted root header. The union alternative is
  *refuted*, not merely worse: `current_rent_quote` is a flat ordinal array, so
  a Sell would project the Buy's quotes.
- **R-2, the ten unraisable `LiabilityBasisSbfErrorV2` discriminants.** Not
  yours either way — the answer follows from a fact nobody has established:
  whether the `DCLLBX02` route is gone or reserved. Routed to the lane to find
  out, then annotate-with-reason if reserved, delete if gone.
- **R-6 / AOT v3.** The measurable half is done: it can no longer report health
  it does not have, now that CI builds contract crates for the SBF target. What
  remains — delete the host-side measurement twin or keep it — is cheap either
  way and does not block anything, so it waits behind work that does.

**STILL YOURS — six, and they are all genuinely product or scope**

1. **R-8, C-15.** Does the accepted project include the FHE/MPC/specialized-batch/
   energy objective? Nothing exists in code; the old ruling was a *horizon*
   ruling and the contract says at `:175` not to infer permanence from it.
2. **R-3, Claims split/merge as user acts** — being answered by implementation,
   but if you do **not** want user-facing split/merge, say so now and the route
   stops rather than lands.
3. **Materialize / Dematerialize: delete or drive.** My recommendation is
   *drive or explicitly refuse* rather than leave it — unexercised
   supply-moving code with a 1,444-line codec and zero dependents is a risk
   surface, and C-08's clause is already carried by Reconstitute/UnwrapStructured
   so nothing is lost by cutting it.
4. **Is a K = 2 structured product useful?** Executable full width is K = 2;
   every campaign in the tree is K = 3 and does not fit a packet.
5. **`MAX_CELL_EX_ANTE_SHARE_BPS_V1 = 9000`.** 90% refuses the convicted
   degenerate partition without refusing a legitimately lopsided binary market.
   The number is a product decision; the constant is only the default.
6. **The two binaries named `dclutch`.** Renaming a released binary is
   user-facing. They overlap only on `help`, and the near-misses are lethal
   (`market show` vs `markets show`; `--keypair` normal in one and refused by
   name in the other; env vars differing by one character).

## R-13 CLOSED, gate GREEN, deploy released — 2026-09-01 ~09:30

Verified at HEAD `b74f5d5b`: `--check-unique` exits **0**. 288 magics, 271
distinct, 3 mirrored under one name, **9 collisions adjudicated**, 161 routes,
297 refusal codes, 0 unclassified positions.

**R-13 was fixed, not exempted around.** `RecordActionV1::Begin` moved 1 → **5**,
out of Registry's half, and two `const _: () = assert!` in
`programs/dclutch-registry-sbf/src/lib.rs` bind the action ranges disjoint **and**
the two widths distinct. The arithmetic accident that was the only thing between
a Record `Begin` and a Registry `Reauthenticate` — `BEGIN_RECORD_BYTES_V1 == 176`
against `REGISTRY_INSTRUCTION_BYTES_V1 == 16` — is now a compile-time fact.

**The exemption file is a register, not a mute switch**, and its three rules are
the reason: every entry must carry a `verdict` naming why *this* sharing cannot
mis-dispatch (an entry without one is refused by the gate); the `constants` list
pins the exact observed set, so a **third** constant claiming a listed value
re-fires the collision; and an entry whose magic no longer collides is reported
stale and must be deleted. The safety question is always *can one dispatcher see
both* — different ELFs never meet one, and a request paired with a receipt never
does either, because instruction data and account data are different channels.

The deploy hold is lifted; the ELF pack is being rebuilt at a post-adjudication
commit, since re-lettering moved the artifacts.

### Also landed while the gate was being taken green

- **R-2 answered by fact, as routed**: ten refusals outlived the route that
  raised them and no chain ever saw one (`32fc79d5`).
- **R-9 written** (`eaa4a1fa`): the batch relation is small on purpose, and
  nothing recorded that — now a named invariant.
- **The `.is_err()` discard fixed** (`f1121675`): the descriptor join was
  throwing away which of three things was wrong. That discard is what made one
  defect look like two and cost a full bisect.
- **The product V1 entrance has a successor**, so it can be deleted rather than
  kept as parallel authority (`bc237abd`).
- **The reference window has one author, and it is the Rust** (`544a0feb`) —
  the hand-agreeing constant in the simulator is gone.
- **The Studio renders the operator's real partition** (`11be4b01`); the browser
  had been decoding those cuts and discarding them.
- **The source policy is keyed to an identity the live entrance does not emit**
  (`5ba7f387`) — C-02's absent constructor, named with what it would need.

### Ruling 7 for you — provider breadth, with the premise corrected

The letter asked for *"provider breadth beyond the first real Pyth profile"*.
**There is no "first" — there are three executing profiles across two evidence
families**, and an explicit in-code ruling that *"the closed set is the point"*:
families are added to an enumerated set by decision, never registered into an
open one.

So the real question: **does C-09 want a third family** — Switchboard, ~13,000
lines by the tree's own precedent, gated on economics currently recorded as
*reported secondhand and unverified* — **or is breadth already satisfied** by
Pyth plus relayed, leaving the generic-header refactor as the remaining work?

I have authorized the refactor either way, because it is wrong today
independent of the answer: `provider_v3.rs:372` pins
`transport_profile_id` — a **provider-neutral field name** — to a **Wormhole
router ABI id**, so a provider without a router cannot satisfy the neutral
record. `PythReleaseV1` is 9 generic fields to 9 Pyth-shaped, and the two Pyth
releases already disagree on shape, so the tree is arguing for the decomposition
from inside the family with no second family needed.

### Ruling 8 for you — what may a program trust from its own instructions sysvar?

**The mechanism, proven at the runtime source.** `create_vm!` maps exactly
`invoke_context.get_compute_budget().heap_size` bytes, and **every read *or*
write above that is an access violation** (`solana-program-runtime-4.3.0-beta.2/
src/vm.rs`). **A program cannot observe its granted heap** — so any check
claiming to verify the grant is reading the *request*.

`require_extended_heap_admitted_v1` reads the request and its name says
`admitted`. The doctrine standing in for a guarantee — *"a request the runtime
would reject fails the transaction before the program runs"* — covers a request
the runtime **rejects**, not one it never **applied**.

**Two repairs, and picking is yours because they trade different things:**

1. **Cap the scratch at the default.** An unhonoured request degrades to a
   **named refusal** instead of an access violation — and **the extended heap
   becomes useless to the routes that need it**, `direct_hot_top_level` among
   them.
2. **Keep the ceiling.** Accept that a dishonoured request is an **abort**
   rather than a refusal, and stop the function's name promising otherwise.

Nobody picked. `f004904b` landed the honest half — the evidence, the fault
addresses, the `vm.rs` citation and this owed ruling are in the doc rather than
in a lane's memory, and the function no longer claims `admitted`.

**Unresolved fact that bears on it:** the same harness **honoured** one route's
65,536-byte request (`direct_hot_top_level`, 2/0, on a route documented to
exhaust 32 KiB) and **did not honour** another's, whose faults landed 776 below
the requested ceiling at both 65,536 and 262,144. Being chased before anyone
acts — if the request never reached the second transaction, the finding collapses
to a campaign defect.

### The 57 has a number behind it now: **2 fall, 55 are real work**

The never-executed block was assumed to be mostly an instrument hole — campaigns
that pass against real ELFs and emit no census evidence. **Measured, it is not.**
The register's own gap was the **10 already repaired** at `96ddf38f`, not a
reservoir behind them.

| campaign | reaches, of the 57 |
|---|---|
| `fractional-atomic` | **2** — escrow open and close, built as real Claims instructions |
| `user-position-admission` | **0** — its three routes were already blocked-with-a-reason |
| `general-hot` | **0** — drives `hot_v3` and the accelerator, both already witnessed |

So 8 blocked-with-reason move to *witnessed* once folded, 2 more move from
*never* to *witnessed*, and **55 routes have no campaign in the tree at all.**

**That is protocol work, not instrument work** — and it is now the largest
concrete thing C-00 and C-16 close against. A far more useful sentence than
"57 never-executed" ever was.

Two negative results recorded so nobody re-derives them hopefully:
`fractional-atomic` drives `FractionalExposureActionV2` and **zero**
`FractionalRetirementActionV3`, so those four routes are genuine gaps; and it
drives the *fractional* compaction and redemption, not the *native* ones.

`general-hot` was deliberately left unwired once tracing showed it reaches
nothing in the 57 — *"wiring it would have been motion."*

### Ruling 8, updated — the discrepancy resolved, and the alarm withdrawn

**The unresolved fact is resolved, and it was ours.**
`test.set_compute_max_units(1_400_000)` installs a **fixed**
`RuntimeConfig.compute_budget` for every transaction — **`heap_size` included at
the 32,768 default** — after which the per-transaction `RequestHeapFrame` is
**never consulted**. The Direct route leaves it `None`, so its bank derives the
budget per transaction and its 65,536 request **is** applied.

> Same harness, honoured one and not the other, **because one campaign forces
> the budget and the other refuses to.**

Proven end to end: with the forced budget removed, access violations **3 → 0**,
Trading CU **203,408 (fault) → 357,648**, and the hostile refusing **`0x4003`,
the code its assertion names.**

**The alarming consequence is withdrawn.** *"Every ProgramTest measurement ran on
a smaller heap"* was wrong — **in exactly the direction the lane had flagged as
unresolved.** The real validator was measured directly: a raw write at heap
offset 33,016 faults with no request and **succeeds** with a 33,792 request.
**No general sweep is owed.** The narrow one is enumerated: six campaigns call
`set_compute_max_units`, **one** drives an extended-heap route; the other five
have zero Trading/Hot references and their numbers stand.

**And a probe caution worth keeping:** the first validator probe allocated via
`vec!` and showed the frame not applied — but `entrypoint!` hardcodes the default
`BumpAllocator` at `HEAP_LENGTH = 32 * 1024`, so **it was measuring the
allocator, not the runtime.** *A probe that allocates cannot measure a granted
heap.*

**Ruling 8 still stands, and is now better evidenced.** A program cannot observe
its granted heap, so the check reading the request cannot establish the grant —
and there is a **concrete configuration in this tree** where a well-formed
request is accepted and not applied. The two repairs and their trade are
unchanged.

**Newly owed, and being done:** move the Rational campaign off the forced budget
onto wire-level CU limits, which is what a real caller does. ~10 sibling rows
lean on it and every CU and packet number that campaign publishes will move —
**numbers that move because the harness stopped lying to the program are the
correct numbers.**

## Your steer, answered in numbers — and ruling 9

**The gated entrance is live on the founding path** (`550e581b`), and the
numbers are:

- The historical default `[12_000, 18_000]@100` — **$120/$180 against a ~$150
  spot** — is **REFUSED, `DegenerateOutcomePartition`.**
- Centred `[14_800, 15_200]` compiles to shares **[3024, 3950, 3024] bps** —
  dominant cell **3,950 against the 9,000 ceiling**. Roughly **30/40/30**. *A
  question.*
- Width unchanged at 4 outcomes, so every coefficient vector still fits.

**The gate caught 13 fixtures founding bandless markets and 3 more founding
degenerate ones — including the sponsored devnet flagship, the same $120/$180.**

One open product question the lane declined to settle: `relayed.rs` has **zero
cuts** — degenerate by construction — and refusal lives where a partition is
*compiled*, not at parse, so a market that declares no partition stays ungated.
**Whether a zero-cut market is legal is yours.**

### Ruling 9 — the genesis manifest variant

**Markets are still not founded on cohort-9, and it is no longer the band.**
Founding needs a `SuccessorPlan`, which needs the checked release gate, which
stops here:

`create-infrastructure` refuses `infrastructure profile refused: InvalidLength`.
`CheckedInfrastructureV1` embeds a **`ProfileV2` by type**, and V2 exists to pin
the predecessor ids a succession carries. **A genesis has none.**

Closing it means either a genesis manifest variant or a version-polymorphic
profile field in the release-set contract — **a release-identity change, so a
release event and yours to schedule.** The lane closed the derivation half
(`bf5499da`, `derive-genesis-infrastructure-profile`, refusal suite 26 → 30);
this is the manifest half, and it declined to improvise it under a live cohort.

**That chain is the one thing between here and condition (b) of your deploy
grant.**

### Also this round

- **The genesis candidate now runs**, and found three walls by running: the
  pinned Node archive fetched and its **SHA-256 matched the script's own pin**;
  13 links built, freshness clean, 13 provenance descriptors, 10 role artifacts,
  the five-role release set, and the genesis profile derived at **144 bytes**.
- **A sixteenth stale lock** (`7de71e61`) — the successor lock could not resolve
  under `--locked` *transitively*, its manifest never naming the crate, so the
  candidate **died with no message at all.** The sbom tier then named it **and
  six more**, reported rather than swept.
- **`71a17ee5`**: `verify-spline-product-handoff.mjs` used exact key equality and
  the compiler had grown `partition_quality` — **the same class swept out of the
  campaign pack, unswept here, directly on the genesis path.** Found by diffing
  real key sets. Made *checked* rather than tolerated, with three red controls.
- **The CLI parity gate landed** (`06edd66e`) at a crate-wide true figure of
  **zero**, after a per-file first cut reported 24 of 34 disagreeing — **every
  one a false positive**, in three named shapes, all the lane's own.

### Ruling 10 — a policy floor raised after I joined can strand my exit

**Measured, not argued.** `locked_capital_floor` comes from the **selected
immutable descriptor**, and the planner applies it to the **poststate** of a
redemption. So an LP whose value has not moved can be **refused its exit by an
evolution it never agreed to.**

Pool residual `[40,100,160]`, redeeming 100 of 200: floor 0 pays, floor 60
refuses, and **the boundary is exact — 20 pays, 21 refuses** — so the floor is
demonstrably the thing doing the refusing.

The lane deliberately did **not** rule: *refusing to drain a pool below its floor
is a real purpose, and a stranded LP is a real cost.* The test states the fact so
it can be decided on evidence rather than on preference. **Yours.**

### And the rest of consent, in numbers (`c3de0f42`)

**An arriving LP cannot move my position.** Over 80 awkward pools: A withdraws a
slice, B doubles the pool proportionally **without asking A**, A withdraws the
same slice — **identical in every scenario, exactly**, because
`floor(2R·b / 2S) = floor(R·b / S)`.

**And the teeth check gave the strongest result of the session.** Minting
`total_shares + 1` for the same basket — **a one-share dilution** — makes the
corpus read **zero executions**: the planner refuses all 80.

> **Dilution by mis-minting is structurally impossible, not merely detected.**

**The anti-vacuity guard is what surfaced it** — without the guard that mutation
passes on no executions and teaches nothing. Second time this session a guard
has converted a silent vacuous pass into a positive structural finding.

The guard threshold was a guess (60); the measurement said 50, so it was set to
the **measured** count with the reason 30 skip named — the slice rounds to
nothing, or its complete sets cannot be split; both physical, neither able to
distinguish a dilution. *Lowering it without finding out would have been
weakening a guard to make a test pass.*

### Correction: the register reads 57, not 58 — and that is the interesting part

I reported **58** never-executed routes after the regeneration. Wrong: one grep
hit is the **legend** at `docs/reference/routes.md:27` that defines the term.
Counting table rows gives **57 of 161**.

**And 57 of 161 is exactly the C-16 document's measured figure.** The claim
register and the measured count have **converged** — which is the actual result
of the regeneration, and it is invisible to anyone reporting 58. A number off by
one is worse than an obviously wrong one, because it still reconciles against
nothing.

**What moved is not seven**: diffing the old register against the new, **ten
routes left and two entered**, net −8. The two entrants are two of the owed rows
the regeneration published — `DCRRCL01` and `DCLTPAB3`.

Both were carried through the third step rather than counted: `DCRRCL01`
**excluded** (sets none of the five destination fields anywhere in 2,120 lines);
`DCLTPAB3` a genuine candidate whose set-sites are **production, above the
`cfg(test)` line** — so the line-position filter correction **earned its keep on
the first new case it was applied to** — and then resolved **class 3, owned**:
the route binds `frame.account(4).key` against `request.refund_recipient` rather
than setting a destination at all.

**The candidate set survived intact**, and the positive control is why that can
be believed: all nine destination-setting modules were required to still appear
in the new 57 *before the join was run*, because a regeneration that silently
dropped the strongest candidate would make the join report clean **for the wrong
reason**. All nine present, same 18 routes.

So the architectural statement — *the code chose it, it was read from a record
that already existed, or the caller named it and the chain refused unless it
matched something written down earlier* — **gains a third instance from a route
that did not exist when the statement was written.** That is what separates an
architectural property from an artifact of one parse.

## RULING: C-15 — ember, 2026-09-01

**Ruled out of the accepted current project.** Ember, verbatim:

> *"privacy/FHE is a 'not yet' for sure for sure, that would be a much later
> version of Clutch, solana isn't ready for that kinda awesomeness onchain yet
> (we'd want to use minidregg, which isn't ready yet)."*

Dated, explicit, with the reason and the named prerequisite. This is a **scope
ruling on the accepted project**, not a third state: the FHE/MPC/energy
objective is **not in this Clutch**. The condition for revisiting is named —
a later version, on a substrate that can carry it, using minidregg.

**What this obliges now:**

1. **Remove contradictory claims.** Anything in the tree that implies the
   privacy ambition is in scope, planned, or partially built must say what this
   ruling says instead. C-15's row closes on this ruling rather than on code.
2. **`O-019` becomes load-bearing.** The invariant recorded earlier — *the batch
   relation is small and specialized **on purpose*** (`INTENT.md:118-120`) — is
   now the thing keeping the door open. A future "simplification" that widens the
   batch relation toward a general encrypted-exchange computer closes that door
   **permanently**, and closes it while nobody is looking, because the ambition
   it forecloses is no longer on any active list. That invariant is the whole
   reason the ruling is safe to make.

**Nothing may report the privacy horizon as deferred, future work, or
in-progress.** It is ruled out, dated, with a stated prerequisite — which is a
terminal state, and the difference matters.

## RULING 9 WITHDRAWN — ember, 2026-09-01. It was ceremony I added.

Ember: *"Blocked? Checked release plan? Predecessor? Release-identity change?
Sounds like fake bullshit ceremony you added around a completely greenfield
devnet that we can throw away at any time."*

**Correct, and I should have seen it.** I dissolved the `derivation_policy`
reservation and the magic re-lettering with **exactly this argument** — devnet is
disposable, there is no installed base, a release event costs nothing — and then
hit a third instance and called it ember's. *"Release-identity change"* is a
phrase from a world where deployments are durable. **There has never been a
predecessor.**

**And it is one defect, not two.** `create-infrastructure` refuses because
`CheckedInfrastructureV1` embeds a `ProfileV2` **by type**, and V2 exists to pin
predecessor ids. `prepare` refuses because its two admissible shapes are a
fabricated **genesis install** (slot 0, no authority — not a real cohort) and a
**checked deployment set** requiring Upgrade receipts a genesis has none of.

> **The release tooling cannot express "a real cohort with no predecessor."**
> The endorsed mutability is not the problem — it is what made the gap visible.

**Authorized as engineering**: a genesis manifest variant or a
version-polymorphic profile field, plus whatever `prepare` needs to admit a
freshly deployed cohort with an upgrade authority and no Upgrade receipts. The
derivation half already landed (`bf5499da`). Behind it the chain opens: records →
profile init → activation → representative markets through the **gated**
entrance → the load simulator, which closes condition (b) of the deploy grant.

**Ember's ruling queue is now four**, not ten: the LP consent floor (being worked
through), the recovery ontology, provider-family scope, and Claims split/merge if
they do not want it built.

## SESSION STATE FOR THE TAIL — written 2026-09-01 before a ~300k context trim

The durable record is already in this file, `WAVE.md`, `AGENTS.md`, the cut, and
`docs/evidence/ARCHITECT_SCHOLAR_2026_09_01.md`. What lives ONLY in context is the
lane map and the in-flight state. Written here so the tail can resume the head.

### Lane map (subagent id → role → state)

| id | lane | state |
|---|---|---|
| `adf64919848af8ebf` | S1+S10 release/devnet | **STOPPED mid-ladder** — was driving cohort-9 publication with the Helius key; 2 of 9 records had landed at last report; balance 44.968159503. **Re-observe chain before assuming anything.** |
| `a77a3bf076532a732` | S3 Direct | live — registered Sell executes (365,011 CU); Buy dies at `MINT_ACCOUNT` require-key (frame mint ≠ Realm collateral mint); wall A is a missing crosscheck, not a gate; then Sell/Buy life |
| `aabbb73d0de8a830c` | S7 Structured | live — four-wall chain: transition (empty satisfying set, convicted, unremoved), heap (closed), **Content/Route (operator flattens K span, composition wants AffineOnce; repair specified, unlanded)**, unknown behind it |
| `a80e9d86fecb37921` | S4 General | live — retracted the 258 wall (interleaved log); real wall is width-1 accelerator returns `Refused` ack — semantic, fresh |
| `a73a4576b1ff6ca7d` | S5 Dealer | live — two-LP life + consent in numbers; selector 9 blocked on register-116 (convicted, unlanded); equity-Add 591,781 CU unlocalized; `468f66b3` red on purpose |
| `a0cc1c74ccaa2fa3d` | S9 Web | stopped at clean line — trade + redemption stranger-operable; next is extracting stage one |
| `a01e0386d5c8e3c57` | S11 census | closed — 73/33/55 register, 55 owned per row, six C-16 instruments |
| `ad8ef8bc299739296` | S8 conservation | closed — 120/120 lamport, 51 atom, class 4 zero |
| `aad96e0729c3f62c6` | S2 resolution | closed — C-09 on evidence; doc-citations tool |
| `abd12366a7144cab0` | C-02 product | closed — band required, gate live, partition proven by sweep |
| `a177fc8443d8e8d4b` | S9 SDK/CLI | closed |
| `ae4f77d8550a57cda` | S6 Series | closed — recurrence landed, five-account geometry wall named |
| `a508ca5d2897899c6` | architect-scholar | closed — **overturned 6 of 8 coordinator calls**; its report is the verified execution list |

### Standing authority (all in AGENTS.md)
Devnet deploy: standing, full redeploy, from a commit. Cuts: `tools/cut.sh`, standing.
Helius key at `~/.helius-key` — use via file read, never echo/commit. NOT authorized:
mainnet, tags, releases, force-push. 1Password signing fails → `-c commit.gpgsign=false`.

### Ember's rulings today
- **C-15 ruled OUT** (dated; "not yet", later Clutch, needs minidregg). `O-019` keeps the door.
- **Ruling 9 WITHDRAWN** — ceremony; genesis path built (`61817d7a` `6b2257b6` `9d66c498` `35a94fba`).
- Remaining genuinely ember's: recovery ontology (scholar: silent source is already survivable;
  recovery buys *quality* not liveness); provider breadth (Switchboard structurally unlike our
  profile); Claims split/merge veto (default: build).

### Ember's last words before the trim
"Am I even gonna regret anything if I just say 'go for it boss'?" — answered: probably not;
execute the scholar-verified list, route new judgment through a reading pass. Then provided the
Helius key, which was de facto go on the ladder. **The scholar-verified list is the queue.**

### The scholar's corrections to take as fact
LP floor DISSOLVED (config digest is a PDA seed → raised floor = different pool; write down
"a position's address commits its terms"). Two binaries: rename the **TypeScript** one (Rust
is the published artifact). 9000 ceiling: has NO production reader and caps nothing. Scale's
owner is `ProductBasisV3::payout_scale`. Materialize: cut the *route*, not the codec, one
act. Heap: the one extended-heap route's justification cites CPIs deleted the same day.
K=2 fits by 3 bytes, which the packet builder spends on priority fee → not a harbour; K=3
needs an ABI change (drop three re-derived PDAs → K=5). Two gauntlet witnesses red at HEAD,
unread. Repair order for Structured: Content/Route first, coefficient guard LAST with a control.

## The scholar, reassessed — Fable, 2026-09-01

Ember asked whether we actually agree with the scholar rather than accepting its
table. Read in full; four load-bearing claims verified directly at HEAD `2dbba552`:

- **A1**: `config` is in `CapabilityRootSeedsV1::as_slices` and LP positions pin
  `child_root`. **Holds.** A raised floor is a different pool at a different address.
- **B2**: `packages/dclutch-cli/package.json:4` is `"private": true`;
  `tools/dclutch-cli/Cargo.toml` says *"still the distributed artifact"* with
  `[package.metadata.dist]`. **Holds — I was backwards.**
- **B5**: `FoundingBandV1` is `{anchor, denominator, volatility_bps, window_slots}`.
  No ceiling field. **Holds — I looked at the wrong type.**
- **B3**: `hot_v3.rs:4880-4884` says the Registry reauth CPI *"stopped"* after
  decision 0017. **Holds** — the extended-heap justification cites deleted code.

**Verdict: agree on substance, every item.** The reading is better than mine was on
every one it overturned, and on the ones it upheld (A4, C2) it added the composition
check and the repair order I had not done.

**Three things the scholar did not say, and one it got wrong:**

1. **Its own headline is miscounted.** *"Six of the thirteen dissolved"* — the table
   shows three (A1, B3, B4). The same arithmetic-slip class this session found in
   four other lanes. The scholar is an instrument, and instruments are not exempt.
2. **B4 + B6 combine into a new architectural tension that is not on any list.**
   K = 2 is the only executable Structured width (B4), and a width-2 market — *"the
   protocol floor, legal and the narrowest market this compiler can emit"* — has one
   ordinary cell that always scores 10,000 bps, so it **refuses the partition gate**
   (B6). The packet limit and the partition gate contradict each other at exactly the
   width that fits. That is a design question: does "degenerate" need a width term,
   or does a width-2 market need an exemption? **Added as ember's #4.**
3. **A2's cost estimate is undercut by its own foundation analysis.** "~5,000 lines"
   is the precedent figure, but the scholar also established that the state account
   already admits the `Recovery` phase, `active_attempt` has its ABI offset, `Resolved`
   accepts the terminal route, and the escrow is pinned — *"only the transition
   function is missing — no account format change, no ABI change, no migration."* The
   honest number is probably lower, and ember should hear both.
4. **C1's aside is a new finding in its own right**: on the generic settlement route
   the exposure identity check *compares the instruction to itself*
   (`exposure.rs:274` assigns `bundle_id` from `admission.selected_id`, which
   `terminal_settlement_v3.rs:393-401` sets to `input.exposure_id`). Guards whose two
   sides move together, again, and the scholar filed it as "named so the two are not
   confused" rather than as a finding. It is a finding.

**Ember's queue after reassessment — four, and the fourth is new:**
- Recovery as a capability child: sell backup feeds to markets that want them, at a
  cost between "one transition function" and ~5,000 lines?
- Provider breadth: **not** "which oracle" — Switchboard wraps Pyth on the assets a
  prediction market cares about. The real question is whether C-09 wants
  **non-price resolution sources** at all.
- Claims split/merge: build unless vetoed.
- **Width-2 markets versus the partition gate** (new, from B4 + B6).

## RULING: non-price resolution sources — ember, 2026-09-01

> *"non-price resolution seems awesome and great"*

**C-09 wants non-price resolution sources.** That answers the provider-breadth
question in the form the scholar reframed it — not "which oracle" but "does the
project want resolution on things that are not a price." Yes.

**What it does not yet decide, and is the next design unit:** *how*. Three
candidates, and the cheapest may already exist:

1. **The relayed-observation family already built** (19/19 on real ELFs) is a
   quorum of ed25519 signatures over an *observation*. If the observation payload is
   not price-shaped by construction, non-price resolution may be a **new relay
   attestor**, not a new family. Check before spending a family.
2. **Switchboard Surge / Oracle Quotes** — the scholar found its real differentiator
   is arbitrary non-financial data (any HTTP endpoint, off-chain computation, other
   contracts' state), and also that its current shape is a **same-transaction
   quote**, structurally unlike dClutch's capture-before-deadline, consume-after
   profile. A family here needs a design answer to that mismatch first, and costs
   15,000–17,000 lines by the honest precedent.
3. Something narrower than either.

Ember's queue is now **three**: recovery as a capability child, Claims split/merge
veto, width-2 markets versus the partition gate.

### Lane map after the trim — 2026-09-01, Fable

The scroll took every prior subagent transcript; none of the ids above can be
resumed. Relaunched from this file's tail block:

| id | lane | unit |
|---|---|---|
| `a97f67b570a29df69` | S1+S10 release/devnet | observe cohort-9's ladder from chain (2.02 SOL unaccounted), then resume to founding + simulator |
| `a5ec4792920591b66` | architect-scholar #2 | non-price resolution: is the relayed-observation payload price-bound? (`docs/evidence/NON_PRICE_RESOLUTION_DESIGN_2026_09_01.md`) |
| `a00611258f691f981` | S3 Direct | Buy `MINT_ACCOUNT` mismatch → registered crosscheck → C-04 clauses |
| `aa481a3dcafc5b3f2` | S7 Structured | Content/Route repair in `encode_effect` → walls behind it → coefficient guard LAST; two red witnesses |
| `af0d560c200fa8218` | S4 General | the width-1 `Refused` disposition; 258 has never run |
| `adefc90f75702203a` | S5 Dealer | authenticated `basis_scale` from `ProductBasisV3::payout_scale` (turns `468f66b3` green by the right owner) → register 116 → the 591,781 CU Add wall |

## Non-price resolution: it already exists, and this morning's gate broke it — 2026-09-01

**Candidate 1 holds, harder than the brief supposed.** Verified by me at HEAD
`4100e848`, four claims direct:

- `crates/dclutch-relay-contract/src/decode.rs:52-55` — `RelayedObservableV1` has
  **exactly one variant, `DbcMigrationProgressV1`**: *"a graduation proposition over
  a terminal window."* The relayed payload is an attested account snapshot
  (`wire.rs:48-57`: key, owner, lamports, data_len, inline, executable, tail_digest).
  **No price, exponent or confidence anywhere.** The product runtime has zero hits
  for "price" or "spot" in 840 lines.
- **dClutch has resolved a non-price market since the relayed family landed.** The
  only non-price market anyone built is already the non-price market.
- **A new relay attestor is a TOML file.** `tools/relayer/` is 12,759 lines with no
  DBC logic at all. Observable #2 ≈ 600–950 lines; #3 onward ≈ 350–480. Against the
  honest family precedent (15,000–17,000): **4–6%, then 2–3%.**

**The break, dated today.** `market.rs:3172` requires `founding_band`
unconditionally on the founding path — *"There is no default."* `relayed.rs:532`
declares `founding_band: None`, on purpose: *"it declares no belief rather than
fabricating one it would never be measured against."* `git log -S` returns exactly
one commit: **`550e581b`, this morning, 11:13.** The partition gate — ember's steer,
landing correctly for price markets — **bricked the founding path of the only
non-price market the tree has.** Same fact scholar #1 filed as B6.

**The repair is three units and no family**, and the second closes three things at
once:
- **R1** decompose `interpret_sealed_record_v1` off the hardcoded DBC positions
  (`decode.rs:275,281,381-383`), ~60–100 lines, before anyone authors #2.
- **R2** make the quality model a *family* (~250–470, once). The framing that makes it
  small: **a graduation market does have a belief — "P(graduates) = x" — it just is
  not a random walk around a positive spot.** So `founding_band` becomes a match on
  band kind, not an exemption. **This closes B6, and the B4/B6 width-2 tension, as a
  design rather than a ruling.**
- **R3** author observable #2 (~350–480).

**Ember's queue is now two**: recovery as a capability child; the Claims split/merge
veto. Width-2 versus the partition gate is absorbed by R2.

Switchboard stays a data source that could feed candidate 1 through a quote-sink
program the tree already knows how to pin — not next, not needed for the first
several non-price markets.

Scholar's one hedge, as a task: it traced the founding refusal by reading and
corroborated it by commit date and the band-free fixture, but **did not run a
founding and watch it refuse.** That is the build lane's red control before R2.
| `acbf4b5b36cee638f` | NON-PRICE build | red control (found the relayed market, watch it refuse) → R1 decompose the observable → R2 quality model as a family → R3 observable #2 |

## Cohort-9: complete through activation; the founding wall is ruling 9's fifth door

Re-observed from chain at HEAD `4b9bb468` by the relaunched release lane, spending
nothing: **substrate 7/7 · publication 9/9 · profile initialized (144-byte V1) ·
succession NOT executed — proven by reading the V2 PDA as a System-owned vacancy,
with a positive control · activation 5/5.** The 2.022449584 SOL is accounted to
**zero residue**: records + profile + activations, one fee, and a 2.000 SOL System
transfer to the campaign payer. Founder-key custody proven; keys moved out of
`/private/tmp` to `~/jobs/dclutch-cohort9-20260901/` (a reboot would have
reproduced the founder-nobody-holds defect on a delay).

**Founding: two stranded attempts, no market**, convicted in the pure planner for
zero lamports: `found.rs:548` requires a 224-byte V2 profile; this cohort committed a
healthy 144-byte V1; `2951b226` flipped that line, and cohort-9 is the first cohort
deployed after it. A genesis cannot construct a V2 — `ProtocolInfrastructureProfileV2::new`
refuses equal predecessor ids and `ArtifactReleaseIdV1::new` refuses zero.

**The lane called it "a release-identity change, ember's to schedule." It is not.**
It is the same defect ruling 9 was withdrawn for — *the release tooling cannot
express a real cohort with no predecessor* — one layer down, at the founding path,
after four doors were already opened today (`61817d7a` `6b2257b6` `9d66c498`
`35a94fba`). Applied the withdrawal rather than re-escalating: **build a genesis arm on
the founding path**, matching the manifest's schema-3 pattern. Both of the lane's
declined workarounds stay declined.

**New landmine, found by that lane and nobody before:** a cohort that revokes
authority on Registry and Rent before committing its V1 profile is **permanently
unfoundable, silently**, with no diagnostic until a founding fails sixty transactions
in. Cohort-9 retained `ExactAuthority` on all seven, so it is not trapped — the
"mutable is deliberate" argument was load-bearing for a reason nobody had stated. A
planning-time refusal is being added.

## Cohort-9 cannot be founded by any host change; cohort-10 is the path

The release lane read before building and found two things that overturn my
instruction. **The founding wall is on chain**: `core-sbf/src/found.rs:289,311` route
both Found paths through `authenticate_profile` — *"V2 only, and never a fallback"* —
and `2951b226` is an ancestor of the deployed `5ba7f387`. The host `AccountAuthority`
is a faithful mirror. A genesis arm on the host would have stranded a third mint to
learn a fact already in hand. **And the shape I prescribed — try-V2-then-V1 — is
refused by name in `PROFILE_UPGRADE_RULING_2026_08_31.md` §6** for the O-005
parallel-authority smell. Its stated reason assumed the in-place cohort-8→9 upgrade
and is stale; the failure mode it named is not. The lane declined to ship it. Third
correct refusal from that lane today.

**The shape that satisfies both**: a genesis-shaped V2 at the V2 PDA, written by
initialize, with two distinct domain-separated sentinels as predecessor ids. One
authentication path, vacancy still refuses, no layout change — the existing
constructor already takes it.

**The design unit, decided on a stated principle**: conjunct 6 ("one V2 per domain,
ever") would leave a cohort born at V2 unable ever to succeed its Registry — P-008
returning for exactly the clean-start cohorts. Of three shapes, a generation counter
puts a mutable field on an immutable profile, and a V2→V3 hop is the
one-more-layout-per-lifecycle-event pattern that produced this. **The vacancy rule
reads the sentinels**: genesis sentinels as predecessors = born at V2, succession
unspent; real release ids = succeeded, spent. Conjunct 6 becomes "one succession
per domain." No new field. Overturnable in one line if the constructor disagrees.

**This is a program change and reaches chain only by redeploy. Cohort-10, under
condition (a)** — full redeploy from a named commit, cohort-9 abandoned and its rent
reclaimed. Everything landed this round carries forward.

**Landed meanwhile (`6155219a`)**: the landmine is a wall. `prepare` refuses when
both Registry and RentCredit are observed already immutable — proved red, with its
own control (authority retained on one role must still prepare; it caught two
wrong-reason refusals while being written) and a real-world control (cohort-9's own
plan would not have been blocked). This lane submitted **zero transactions**.
| `a84c8bf71f714d16c` | CLEANUPS | five verified-and-decided defects from the scholar: false npm comments + rename the TS binary; cut the Materialize route as the N-11 reject; make 9000 a real ceiling; rename the heap check + measure `DCLTHOT3`'s true peak; the settlement exposure check that compares the instruction to itself |

## STANDING GOAL — ember, 2026-09-01

> *"Make dclutch the best version it can be, eliminating all protocol defects and
> making the operator console & UX excellent."*

Two halves. Protocol defects: eight lanes. Operator console and UX: the ninth,
below, on a named backlog of ten with `file:line` on each.

| `a2f3695f1c033fb62` | OPERATOR-UX | the wizard cannot call the real gate; the Studio evaluator is a mirror; the abort path shows `ProgramFailedToComplete` with no remedy; `basis_scale == 1` hardcoded in portfolio + PositionBars; `claims.conserve`'s missing second wall; runbook replay tier; the two-binaries doc page; 31 type errors; 223 CSS rules; redemption stage one |

## The sentinel vacancy rule is built (`c60b25e8`); cohort-10 is two host changes away

**Soundness rests on who may write the bytes: only Core writes a V2.** Genesis
initialization writes the two sentinels and only into a vacant System-owned PDA;
the ceremony writes real predecessor ids read from the live V1 and **can never write
a sentinel back.** A succeeded profile can never present as unspent, and the rule
cannot be forged from outside. Conjunct 6 is now **one succession per domain**, read
by a `profile_succession_state_v2` classifier. `process_initialize` commits both
profiles in one instruction (frame 14 → 15 — an ABI change reaching chain only by
redeploy; nothing deployed depends on the old shape).

**Contained, not hidden:** conjunct 6 used to be enforced *physically* by the System
program refusing a second `allocate`+`assign`; a born-at-V2 profile is already
Core-owned at exact width, so the succession now overwrites in place on that arm.
The conjunct-7 read-back belt is unchanged, and anything at the PDA that is not a
decodable Core-owned V2 of exact width classifies as `Succeeded` — **an account the
ceremony cannot read is never treated as space.**

**The proof the shape is right: `found.rs` and every on-chain reader are untouched.**
A genesis profile is simply a V2. That is what §6's no-fallback buys.

Hostiles named before they were run: half a forgery is still a forgery, so
`born_at_v2` requires **both** sentinels — proved red by weakening the conjunction
to a disjunction, which fails exactly that test and nothing else. 25 + 33 green.
The lane also repaired a comment its own change made stale — third recorded-wall
decay it found today, first that was its own.

**The seam, at file and line** (the lane declined to start a schema migration on
remaining budget, which was right — a half-done migration in a shared tree is worse
than a clean named seam):
1. `tools/local-validator/bootstrap/successor/src/plan.rs:1123` builds only the V1 —
   needs the genesis V2 body, the V2 PDA, and the fifteenth account; the profile
   pin's schema ripples into the campaign's initialize stage, the evidence emitters,
   the plan schema version, and the genesis manifest in
   `checked-release-candidate.sh`.
2. `market.rs:4290` maps `(no succession plan, V2 observed)` to a **refusal** — exactly
   what a born-at-V2 cohort presents. Needs a genesis arm, **and the `Predecessor`
   arm goes in the same change**: after `2951b226` it can never produce a foundable
   projection, and AGENTS.md forbids a superseded authority path beside its successor.

Then the full redeploy carrying `c60b25e8` → founding → load simulator. Deployer
unchanged at 42.945709919; that lane submitted zero transactions.
| `a88faeac520a279ee` | COHORT-10 | the two host changes as one schema migration (`plan.rs:1123` genesis V2 pin + fifteenth account; `market.rs:4290` genesis arm, `Predecessor` arm deleted) → full redeploy carrying `c60b25e8` → ladder → found the SOL/USD market with the proven founder key → load simulator; closes condition (b) |

### CORRECTION — the Structured operator was right; `composition_v3.rs` was wrong

Two entries above (the scholar's reassessment, and the Structured lane map row) say
the operator's route geometry was wrong on four grounds. **Reversed by the lane sent
to build the repair** — `claims_composition_v3.rs:639-641` refuses any
representation route that is not `Once`, and `AffineOnce` would bind K == N, which
the family exists to deny. The scholar's four *facts* held; the *inference* was
inverted. Third time today. See WAVE.md, "REVERSAL".

### Lane map delta — 20:15

- `acbf4b5b36cee638f` NON-PRICE — **closed, row complete.** Non-price resolution exists, is witnessed twice on real ELFs (rows 0 and 1, 24/24), the founding belief is a family (`SpotBand | StatedProposition`, mismatched pair unrepresentable), zero-cut and the width-2 tension closed as consequences, observable #2 cost 35 emitted Rust lines and 23 lines of TOML.
- `a0a5010a5119d7214` C-09 WITNESS — drive the fourteen unwitnessed resolution routes on real ELFs; bindings from observation only; structurally undrivable → `blocked.json` with reason and owner; recovery-ladder routes are ember's open ruling, not a fixture to fake.

### Owed, unowned, named: a Lean model for the AccountProfile V2 vocabulary

The Lean-emitted operation table is **V1**. The **V2** table — the one every profile
in the tree executes — is hand-written in `v2.rs` with no formal model. Twenty
operations, none modelled; a twenty-first (`ProjectDataDigest`, proposed at
`docs/design/PROJECT_DATA_DIGEST_V2.md`) would join them. Not a condition on the
digest; its own unit, for a lane with `formal/` authority and budget.

### Lane map delta — 20:45

- `af0d560c200fa8218` GENERAL — **closed, row exhausted pending others.** C-05 executes at both widths; every refusal names its conjunct; OpenBatch at `ProductIdentity`, blocked on the digest primitive (proposal handed to Direct) and the width-258 rows on the `BumpHeapV1` extraction (cleanups).
- `a42033e11f655c7c5` C-10 WITNESS — thirteen unwitnessed retirement-chain routes; get the journey a market so L8 stops reporting `inapplicable`; bindings from observation only; undrivable → `blocked.json` with reason and owner.

### Lane map delta — 20:55

- `a84c8bf71f714d16c` CLEANUPS — **closed.** Five landed; the sixth (`BumpHeapV1` extraction) **built, measured, discarded by its own control** — a lifted heap ceiling turned a named OOM into an unnamed access violation because the grant never arrived. Artifacts preserved under `scratchpad/h6/`; the budget-forcing question is with General. Two generated registers now stale on my side (`docs/reference/refusals.md:291` spells `HeapFrameNotGranted`; `capabilitySurfaceV1` lists the deleted codec) — regenerate from a clean worktree at HEAD when the lanes quiesce.

## PARSIMONY CLOSEOUT — 2026-09-01, end of session

**The attractor** — the tree this is trying to become:
*every rule is a tool, every fact has one author, and the ledger is the commit log.*

- **One adjudicated-exemption register**, implemented once — verdict required,
  set pinned, stale entries fail — used by seams, magics, blocked routes, and
  citations. Today: four implementations of the same three rules.
- **One browser boundary scaffold** — digest pin, constant-name canary, post-load
  width re-check — instantiated per compiled planner. Today: four hand-built
  instances (admission, payout operator, payout wasm, payoff evaluator).
- **`tools/lane.sh commit` as the only commit path.** It already exists. Today:
  186 lines of remembered rules added to `AGENTS.md`, two of which restated it.
- **`WAVE.md` carries only cross-lane *class* syntheses** (the instances of
  guards-whose-two-sides-move-together; instruments catching their authors).
  Per-defect records live in the commit and the evidence doc. Today: **+4,994
  lines**, most of them prose retellings of lane reports that already exist as
  commit messages and transcripts. A year of this is a write-only ledger.
- **`GOAL.md` is the ruling queue and standing authority.** Lane maps are
  session-scoped and belong in the scratchpad. Today: +1,488 lines doing three
  jobs.
- **Evidence docs are dated and never edited; a reversed verdict gets an
  in-place addendum** (the non-price lane's §9 is the model), never a second doc.

**Trajectory: the protocol is on track; the coordination layer is drifting.**
462 commits, ~30 new refusal codes, 5 new crates — every one a convicted defect or
a missing route, none decoration. But each finding now exists in four copies
(commit → lane report → WAVE entry → coordinator reply), four registers implement
one pattern, four boundaries implement one scaffold, and the rules that broke four
times tonight were the attention-enforced ones.

**The single change that most bends the curve:** stop writing per-report `WAVE.md`
entries, and route every lane through `lane.sh commit`. Both are behaviour, not
code. Landed tonight: the `AGENTS.md` commit prose deleted in favour of the tool
pointer, with the one limit `--only` genuinely has (two lanes, one file).

**Deletions rejected, and the invariant each one taught:**
- `WAVE.md` history — it is history; the direction is to stop adding, not to
  delete what lanes cite by line.
- `ARCHITECT_SCHOLAR_2026_09_01.md` despite two reversed verdicts — a dated
  reading is evidence of what was believed; the reversal belongs as an addendum
  (owed: §B4's coefficient verdict and §A3's "operator wrong").
- The lane map in this file — mid-session, eight lanes were relaunched pointing at
  it; moving it under them would have cost more than it saves.

**Threads under the attractor, for the next session:** the register library; the
boundary scaffold; the V2 AccountProfile vocabulary's Lean model; moving the lane
map to the scratchpad at session start rather than session end.

### Lane map delta — 21:20
- GENERAL: `crates/dclutch-sbf-bump-heap` landed (`fe254e9f`); accelerator **25/0 with the real frame**, first time; the Trading-side frameguard re-capture for the renamed symbol is **owed**. Fourth conjunct unblocked by `ProjectDataDigest` (`a5bb4390`, Direct). 
- DIRECT: wall C crossed, campaign completes behind the wall-A probe; on the registered crosscheck.

### Lane map delta — 21:30
- `a0a5010a5119d7214` C-09 WITNESS — **closed.** 12 of 14 witnessed on real ELFs; 2 blocked with reason and owner (recovery policy → ember's open ruling; `#AdmitTerminal` → a dead arm, deletion at `core-sbf/src/resolution.rs:263-272`, plus `#VerifyFundReady` and `#CloseFund` beside it — **Core owner, unassigned**).
- **New thread under the attractor: the legacy packet.** Thirteen routes over 1,232 bytes; the fix class is *commit-don't-inline*, same as Structured's K lift. Protocol-wide; nobody owns it yet.
- **Coverage figure to carry: 8 of 314 refusal codes have been observed firing on chain.**

### Lane map delta — 21:40
- `a2f3695f1c033fb62` OPERATOR-UX — **closed, ten of ten.** Three mirrors compiled away (Studio evaluator, wizard gate, SBF abort vocabulary), each digest-pinned and canaried by constant name. **The wizard's gate refuses a band the deleted check called fine** — spot dead centre at 200 bp is a 300-tick displacement inside one 6,000-tick cell; the old check measured distance from the band, the gate measures where the mass lands. Refusal registry was 26 codes stale in both twins. Runbooks tier found the repo's own "See it run" command unrunnable. Type errors 31 → 14, three hiding real defects. **Redemption stage one specified, not built** — two RPC rounds not three, three pure phases, in WAVE.md.
- `a07bc54d2e4753bc9` REDEMPTION-STAGE-ONE — extract `produce_wallet_terminal_input_v1` the way stage two went (three pure phases, two RPC rounds), WASM boundary, browser acquisition; `RedeemFlow` stops importing JSON. C-12's last unreachable capability.

### Lane map delta — 21:55
- DEALER: C-06's eight routes witnessed (`a78f03eb`) — 273 transactions, 423 observations admitted, 7 executed + 1 refused-only (rollback: no campaign drives an accepting one yet, named as such, not as blocked). Provenance caveat: ELFs and harness from different commits; **not C-14 evidence until a quiet-tree re-run.** Runner now gates on `cargo check` **and** a campaign-private `CARGO_TARGET_DIR` — five attempts died on shared-lock starvation and half-applied refactors. Next: the equity Add with the new codes; the rollback twin.

### Lane map delta — 2026-09-02 00:15 (after the 429 reboot; Fable)

Resumed by SendMessage, never relaunched: `a88faeac520a279ee` COHORT (cohort-10 abandoned
in place on a stale frame exemption, fixed `8ae2c9c9`; **cohort-11 deployed, genesis born at
V2 on chain**; resumed at the founding/candidate), `a07bc54d2e4753bc9` REDEMPTION-STAGE-ONE
(`d376896d` landed; on the WASM step), `af0d560c200fa8218` GENERAL (supply committed `a0ec6a2f`;
heap wall is **one allocation > 34,640 B at width 2 with half of 65,536 free** — sized from a
maximum; convicting), `a00611258f691f981` DIRECT (equity-Add `0x4003` window
`hot_v3.rs:3905-4052`, then C-04), `aa481a3dcafc5b3f2` STRUCTURED (own S7 dirty paths, then
redeem→retire on the Trading route), `a42033e11f655c7c5` WITNESS (L8 declarations, last 2 of 13).
`adefc90f75702203a` DEALER — **closed**: `accepted.rs` landed `8099f363` (had been +4,059 lines
uncommitted), custody reserve/rollback claimed `10e44fea`, selector 9 collapses into Direct's Add
(`b059666d`); accelerator frames measure 2,342–3,084 B → packet lane.

New: `a81fbac4395c47956` TREE-HYGIENE (52 dirty paths; HEAD is canonical under rustfmt
1.97.1/2024 — proved; drift is bare-`rustfmt` 2015 style; `rustfmt.toml` + CI fmt gate + proof-gated
restores + orphan landing + genref from a detached worktree). `a1d142591d150d69b` CORE (the dead
`VerifyFundReady`/`AdmitTerminal` arms + `retire_v1.rs`'s 28 never-named frame constants).
`ae46f8fc098a84c4b` PACKET (reading lane, Fable: `docs/design/PACKET_LIMIT_2026_09_01.md`, every
route measured). `a4208061e68425b0d` RIP (four zero-consumer crates: product-payoff-codec V1 +
its two Lean emitters, product-payoff-svm, product-payoff-v2-svm, pyth-contract).

**Rip census (ember: "rip and tear old code that piled up").** Method: `cargo metadata`
reverse-deps over 110 members + every sub-workspace manifest; then a one-pass tokenization of the
tree joined against 20,681 `pub` definitions. Scratch: `census/census.tsv` under this session's
scratchpad. **241 pub items are named exactly once in the tree (their definition).** They cluster:
`core-sbf/retire_v1.rs` (28, → CORE), `trading-sbf/projected_{hot_outer,open,realize}_*_v4.rs`
(7 entry points, zero callers — possibly not even `mod`-declared), `trading-sbf/series/*` (14 incl.
`process_close_v3`/`process_retire_v3`, never dispatched), the shadow accelerator surface (12 across
execution-strategy-contract + general-adapter-contract + series-shadow-sbf), `general-adapter-contract`
(21 incl. the three `evaluate_general_admitted_*_v3`), `direct-codec/lib.rs` register/schema
constants (7), `operator` (22 incl. `build_dealer_{equity,lp}_hot_instruction` — the campaign has
its own builders: **two authors**; `build_claim_check_escrow_close_v1` never called;
`compile_direct_hot_v0` never called → PACKET). Three programs are in no cohort AND not in the
release tool's ROLES: `dealer-sbf` (5,234 lines, carries the B.13 HoardPrincipal defect),
`direct-aot-sbf` (780), `product-runtime-v2-sbf` (694). Feature-gated families (`series-family`,
`dealer-family`) are by design; dead items inside them are not.

**Disk.** Data volume was at 30 GiB free. Reclaimed ~70 GiB from dead-session `/private/tmp`
worktrees (diffs captured to scratchpad `orphan-worktrees/` first). NOT mine and not touched,
for ember: `.claude/worktrees/` **122 GiB** (19 worktrees, 14 `target/` dirs; branches survive
`git worktree remove`; opforms/genseven2/cohort10 hold 1/1/2 dirty files), `~/jobs/dclutch-fill2`
**130 GiB**, `~/dev/dclutch/target` 64 GiB.

### Lane map delta — 2026-09-02 00:45

Goal re-issued by ember with cut + redeploy + cleanup authorization. Cut `88e44a373` at HEAD `4b7b2a0b`.

- COHORT `a88faeac520a279ee` — **closed.** Cohort-11 live from `8ae2c9c9`, genesis born at V2, **SOL/USD market OPEN at
  `ARuPAuyJbJoLdMWGDzSqvcV9py25EkmMj8ABnfKP56s`**, founder key held (`docs/evidence/COHORT11_GENESIS_FOUNDED_2026_09_01.md`).
  Population life NOT demonstrated: simulator stops at `BlockhashNotFound` after prefund; `frozen_routing_table_for` scans the
  whole ALT program. → POPULATION `ad6d48cc39f881e8e` (both defects + the demonstration, ≤2 SOL per step). Genesis release
  candidate re-run was killed mid-compile at load 118; queued in `05b0bc47`.
- CORE `a1d142591d150d69b` — **closed** at `60420ba6`: none of the four arms was reachable; `CloseFund` refuses AT DECODE with
  `0x301C UnsupportedAction` (wire variant kept: Resolution owns it, `core_effect.rs:1487`); `retire_v1` is live and binds
  every position by irrefutable slice pattern, so its 36 constants were vocabulary nothing read — deleted, 8 collided by name
  with different values in `claims/market_closure_v1`. **Root cause to carry: nothing depends on `dclutch-core-sbf` as a
  library, so `pub` exempts every dead item from every warning.** Owed: 12 `CloseFund` arms held by Rust totality across 9
  helpers (~400-line type-narrowing refactor, separate lane); cohort-12 redeploy after population life lands on cohort-11.
- GENERAL — heap wall closed `397ef013` (per-chunk 8,432 → 3,288; peak 51,912 → 41,624); OpenBatch N=2 now reaches
  **`0x4005 Commit` at 816,888 CU**; convicting.
- WITNESS `a42033e11f655c7c5` — C-10 13/13, L8 claim is a required argument (`061eaa39`). **Reopened**: journey's `#CloseFund`
  binding has been false since `a34ff595` (builds `build_resolution_close_fund_v3` against Core); `#AdmitTerminal` binding
  line owed by resolution-core-v3; stale `blocked.json` entry. Also found: `run-claims-extended.sh` cannot pass at HEAD (143
  binding problems, → Structured after redeem→retire); `tools/gauntlet/relayed-vertical` did not compile at HEAD (→ V0 lane
  fixes the manifest, hygiene adds the CI gate).
- PACKET `ae46f8fc098a84c4b` — **closed** `5a3e29da`. Two fix classes: v0 over a frozen ALT (client change) for everything;
  Structured full width needs K=5 ABI. Width-2 vs partition gate → **ember ruling, options A/B/C §7 (recommend A)**.
  `compile_direct_hot_v0` is a superseded v0 island → Direct deletes; registered family has no operator-side v0 builder.
- V0 `a7d3265e9f331e145`, EMISSION-WIRING `ab353370915da897e` (3 Lean-emitted files no crate compiles), DEALER reopened
  (the C-06 tier drives unshipped `dealer-sbf`; repoint, then delete 5,234 lines), RIP `a4208061e68425b0d` in flight.
- Disk: 30 → **304 GiB free**. Removed under authorization: `.claude/worktrees` (19, branches kept, 4 dirty files captured),
  cohort-8/fill2 source worktrees (5 unmerged commits preserved on `preserve/cohort8-src-20260831` — **unread; someone should
  read them**), fill2 probe ledger (128 GiB; RESULT/SUMMARY kept), smoke0 harness build. Kept: `~/dev/dclutch/target` (warm).

### Lane map delta — 2026-09-02 01:20

- DIRECT — **equity Add executes and commits** (`8ea9d11a`, 1,044,703 CU): `project_tail_count` returned 0 for any profile
  without a tail projection, so `require_tail_count_agreement_v3` was unsatisfiable by any honest Add — now `Option<u32>`,
  None ≠ 0. Dealer's localisation was wrong twice; the CU ladder found the phase in one build. Dealer's equity hostile was
  passing on that donor → routed. Next: C-04 clauses; delete `compile_direct_hot_v0`; operator-side registered v0 builder.
- GENERAL — `0x4005 Commit` convicted (`0ba29756`): General's release names System as an IDENTITY for the transition and
  never supplies it as a runtime ACCOUNT for the create — fourth "same party, both halves" instance. Fix = one coordinate in
  General's account profile, every reader of `general_account_profile_fixed_count_v3` moved in one series. In flight.
- STRUCTURED — island proven at HEAD in a clean worktree (45/45 without the ten dirty files, which are pre-session rustfmt
  drift → hygiene). redeem→retire wall: `custody-replay-no-open-vault` — the operator requires the Claims-role replay to
  count a vault opened under another role at founding; no route opens one. **Two-authority question → cross-boundary test
  (does the chain admit it?) before any change.** 27 bare `InvalidTerminal` sites split; two hostiles found refusing
  elsewhere than their authors believed (`d762f2fb`).
- EMISSION-WIRING — my "three emitted files no crate compiles" was FALSE (they are `#[path]`-included from test targets;
  census fixed in memory). The real defect was worse: 198 constants with 2–3 authors each; all now derive from Lean, zero
  value mismatches (`f6404a78`, `20b31177`, `5c95a58e`). Owed → same lane: split `InvalidRentQuote` (3 causes); `#[cfg(test)]`
  the effect/composition corpora in Lean (hbox).
- REDEMPTION — `723eed12`: input-wasm boundary landed, digest-pinned, positive load test. Stranger zero-CLI redemption: **no** —
  phase one takes an eleven-row address book; four `terminal_composition_*` digests have no chain pointer but are
  recomputable by `compile_native_basis_composition_v1` (already in the wasm closure). → phase zero, in flight.
- DEALER — tier retired (`1e6433bf`): `dealer-checkpoint` was already the shipped-ELF campaign. Step 2 (delete `dealer-sbf`)
  blocked on the shared root `Cargo.toml` → building `tools/lane.sh commit-patch` (HEAD + own hunk into the index, guard on
  the staged path set) so crate-lifecycle lanes stop serialising on one file. Register map ready.
- RIP — unit 1 done (`c774ad16`, `67cfccec`; magic-collisions and seam-audit registers caught; SBOM green at committed HEAD
  for the first time). Unit 2: projected v4 outer + five compositions, Series dead items, shadow surface, small contract
  constants under "wire the pin or delete what it pins", **ELF-identity control** per program crate.
- Hazards named by lanes: shared scratchpad filenames collide (use lane-unique); bare `rustfmt` on a crate root reformats
  the module tree and moves wasm digests through panic line numbers; a dirty index makes `git diff` lie about hunk maps.
- Cuts: `e9c99f00d`, `e1f04652f`, and this one.

### Lane map delta — 2026-09-02 02:00

- RIP unit 2 (`cbdecdb3`, `fc9ba16e`, `ac00e939`): projected Hot V4 deleted (4,132 lines; its outer imported a module that
  does not exist); 20 Series items; six shadow constants **wired** (the shadow accelerator auth read slots 0–5 as bare
  integers beside the named block — one frame, two authors); 5 contract constants deleted; ~40 unwired pins reported →
  unit 3. **ELF control refined:** raw digest moves through `.strtab` CGU names; the invariant is identical size and zero
  differing bytes outside `.strtab`, with a comment-only edit as the positive control. 11/13 ELFs byte-identical.
- **RULING FOR EMBER — the Series family.** `crate::series` in Trading (28 files) has no non-test consumer, no dispatch in
  `lib.rs`, no route; it is a compiled island inside the default feature set (not linked into the shipped ELF, so it costs
  nothing at runtime). Its accelerator `series-shadow-sbf` is shipped and carries a compiler release-id preimage nothing
  hashes (a certificate field compared by no validator). Options: (A) Series is on the roadmap → a lane finishes the
  dispatch and the shadow derivation, C-row added; (B) not on the roadmap → cut the island, the shadow program, and its
  Lean/registers (~30 files + 3,508 lines of program). No engineering action is forced today; the ruling decides which
  lane exists. Recommendation: A if recurring markets are a launch feature, B otherwise — the island has been unreachable
  since it was written.
- DEALER — `dealer-sbf` deleted (`e6b7bf1a`; programs 13→12, routes 161→160, refusals 316→305, band 0x7 tombstoned in
  0007); **`tools/lane.sh commit-patch`** (`4bb59211`) is the protocol answer for shared files (HEAD + own hunk into the
  index; refuses a non-empty index or a foreign path set). Tests it broke fixed `aa7f8892`. Found: `generate-refusal-registry.mjs`
  must run AFTER genref (refuses a code in a retired band). Next: the Position-identity join into Trading's pre-CPI
  authentication — the hostile spends 1.04M CU before the accelerator's guard can name it.
- V0 (`659d6f26` … `58ab1dcd`, 9 commits): 21 transactions across 5 campaigns to v0, all six C-09 routes fit; relay append and
  the retirement chain are **data-bound** (36 B headroom at 1,196; requests 744–864 B) — the lever there is the relay's CDI
  seam, not a table. Synthetic `packet_census` replaced by real submissions + `unique_account_locks` (36/64). Next: the
  founding chain, and one author for lookup addresses (two hand filters push the program id into the table — rent leak;
  `versioned-message-operator` pins conflict with the harnesses').
- WITNESS: five cohort-8 preserved commits all on main by patch-id (branch deleted); the admission rule was still written
  four times → `CheckedDeploymentDispositionV1::admits`, total match, truth-table tests (`40032bdf`). Now: Core's 12
  totality-held `CloseFund` arms → three-variant `ComposedResolutionActionV1`.
- EMISSION: red lifecycle test convicted by bisect (`e56aac73`, fixture predates `73ffb010`'s permission rule; the crate has
  no red for the first time since 09-01); **V2 vocabulary is Lean-owned** (`38b8429c`: 106 constants, 68 twinned, zero
  mismatches; 32/36/40/48 are four prefixes of one schema; a theorem exposes V1's 1–7 vs V2's 0–6 opcode tags). Next: three
  decoders derive; the profile×prestate admissibility table.
- GENERAL: convicting 3 stack-frame diagnostics on `execute_authenticated_hot_v3` (frameguard refuses to baseline) before
  the System coordinate lands.
- Cuts: `7365f1503`, `93aaeaf20`. Dirty paths 65 → 40. Disk 293 GiB free.

### Lane map delta — 2026-09-02 02:45

- DEALER lane `adefc90f75702203a` **died at the context ceiling** ("prompt is too long") mid-measurement; relaunched as
  `a929bd24b931c8297` with the full handoff (the hostile's ladder, the 5de38ef2 checkpoint, the two ruled-out ideas, the open
  number: the honest Add's plain-build cost). Lanes nearest the same ceiling: GENERAL (~890k tokens), DIRECT (~740k),
  STRUCTURED (~710k) — hand off on their next report if they stop mid-unit.
- GENERAL: `0x4005 Commit` closed (`e3298c9a`, System program is an account now); next `0x4018 AdmittedTransport`.
- WITNESS: Core narrowing landed `f6b84c56` (census unmoved; found the enumerator skips `let` initialisers → fixing it).
- RIP unit 3 `c00a2242`: the split_root defect class was EMPTY on measurement (110 sites classified); four pins wired red-first;
  ELF rule refined again (panic `Location.line` shifts in `.data.rel.ro` with column unchanged are line-shift artifacts).
  Unit 4: `validate_construction` never called in a live claims kernel; funding-list derivation.
- V0: rent leak closed (`c09beaa1`, one entry per table nothing could use, 222,720 lamports each); pin alignment blocked by a
  runtime panic recorded at the pin (`ba69ef0d`); founding chain already v0; DCLTGMF3 measured 2,129 → 460. Ruled: freeze the
  seven mutable tables (authority-redirectable routes are the defect; rent is the price). Ceiling check in `send_inner_with_signers`.
- POPULATION: cohort-11's market was founded at **30 bps** and the Direct setup requires exactly 50 → **can never fill**
  (fourth such market; stager now refuses). Fee rate has two authors → DIRECT. Cohort-12 + a 50 bps market + the trade
  waits on Direct's frame fix (still 3 at `be67416e`). Census bindings and the fill-boundary laws landed (`49c8fa92`, `be67416e`).
- EMISSION: admissibility table Lean-owned (`e692f8e4`, non-monotone in the profile number — a theorem states the exception);
  alias-width question answered safe and pinned (`f3694735`); now the decision-0012 admission rule's Lean corpus for Core.
- Cuts: `98648c089`, `bc666b7b2`, `18b260248`, `f5dfc7145`, `313df674c`, `b3977fd16`.

### Lane map delta — 2026-09-02 03:10
- GENERAL `af0d560c200fa8218` handed off at ~930k tokens (it convicted `0x4018` to `account_count` 12 vs 13 — host
  `bundle.rs:560` `logical.len()` vs chain `runtime_accounts.len()`; the bank digest is identical so the System account
  contributed no register; do NOT split the count) → **`a39412e2663ca5f8d`** with the full state. Standing debt named at its
  call site: `require_admitted_bank_matches_frame_v3` written, tested, unwired (the guard it replaced was x==x).
- DIRECT's frame fix landed `58b077f8` → POPULATION resumed: measure, candidate, cohort-12, 50 bps market, trade.
- STRUCTURED `d5a9fe9e`: the chain pays at `open_vault_count == 0` — the operator check was a wrong mirror, deleted; next wall
  "a width typed five times". EMISSION `d418cc8b`: decision-0012 slot-pin rule has a Lean-decided corpus replayed through Core.
  WITNESS `4879a54f`: enumerator walks diverging `let` initialisers; routes 160 → 167, seven never-counted rows behind four real
  guards. REDEMPTION `eb2c6e99`: phase zero — zero-CLI redemption for every document; recipient ATA default next.
- Next nearest the ceiling: DIRECT (~740k), STRUCTURED (~710k), WITNESS (~680k). Hand off on their next report if mid-unit.
- Cuts: `af15c2599`, `9172fb401`, `9a5e1831e`.

### Lane map delta — 2026-09-02 03:30
- DIRECT `a00611258f691f981` handed off at ~856k tokens → **`a8931c4eecc29c8fb`**. Landed: frame restored (`58b077f8`; the
  64-byte step was `ab5a63db`'s transport binding, not the tail-count law — seven shapes of the law all measured 3,904),
  fee rate single-author (setup reads the authenticated config; band ≤500 enforced once, 501 refuses `InvalidFee`),
  `compile_direct_hot_v0` deleted (`21c8a075`). Parked: `scratchpad/lookup-one-author.patch` (packet shrinks 1,198 → 1,167;
  read before landing). Successor's order: **the fee-bearing fill's 115,003 CU overage of the 1.4M ceiling** (the only
  reason cohort-11's trade had to be gross ≤199), the parked patch, then resolution / redemption / every close / portable-ticket.
- GENERAL successor owns the transport binding AND the 64 frame bytes it needs, as one unit.

### Lane map delta — 2026-09-02 03:50
- EMISSION `ab353370915da897e` stopped at its budget line → **`addb8204ef3d72bb0`** (GeneralV5Assurance corpus; then guard
  the 15 unguarded emissions to 0). Landed since 02:45: `a00fc7c9` (infrastructure PDA domain Lean-owned), `ac9864c5` (Dealer
  scenario solvency corpus — `maximumMerge` had no Lean name; a split one atom looser than least now reds). Trap recorded in
  memory: positional `#[cfg(test)]` re-binds on `mod` insertion.
- WITNESS `6a139c63`: ARuPAuyJ… (gen 1, Founding, aggregate vacant) and 3rBfDBpa… (gen 2, **Open**, aggregate exists) are both
  Core Markets; the admissions landed on the right one; both dated records corrected by addendum. `41005b27`: **67 of 305**
  refusal codes observed firing, derived from 21 campaigns' bindings (was hand-carried as 8 of 314). docs/reference green.
- Cuts: `100dfaf49`, `6eec81e5d`, `8fd18304b`, `2f32a838b`.

### Lane map delta — 2026-09-02 04:15
- STRUCTURED `aa481a3dcafc5b3f2` handed off at ~835k tokens → **`ae1559fc2d9501927`**. Landed: the host-side vault mirror
  deleted after the chain proved it (`d5a9fe9e`); the operator builds the Hot terminal request; the wall is ONE account of
  artifact geometry (`RATIONAL_TERMINAL_CLAIMS_ACCOUNT_COUNT_V3` 49 vs frame spec 50, typed in five places → one author,
  defect recorded); census 143 → 24 (`f4feee8c`: 70 rows were another campaign's evidence folded in). Successor: re-derive the
  per-index tables from the spec (49→50) so redeem→retire executes; Trading into the campaign's `programs.json`; then K=5.
- V0 `a7d3265e9f331e145` **closed** (13 commits): frame count **zero** across twelve builds at `6a139c63`; Dealer Hot rows
  measured (`ac24f70c`): all fit a v0 packet, **selector 1 needs 70 unique locks of 64** → Dealer's next unit after the CU.
  Relay append and retirement chain are data-bound (no third lever). Frozen-table proof comes from cohort-12's founding.
- WITNESS `a42033e11f655c7c5` **closed** at a clean line (`ac6d325d`: genref `--allow-dirty` has one author + a 9-assertion
  test). RIP `cbb1ebca`: a claims receipt commits to the validated construction, not a summary.
- Active: DIRECT `a8931c4eecc29c8fb`, GENERAL `a39412e2663ca5f8d`, DEALER `a929bd24b931c8297`, EMISSION `addb8204ef3d72bb0`,
  STRUCTURED `ae1559fc2d9501927`, REDEMPTION `a07bc54d2e4753bc9`, RIP `a4208061e68425b0d`, POPULATION `ad6d48cc39f881e8e`.
- Cuts: `4b029ac53`, `795dc7961`.

### Lane map delta — 2026-09-02 04:35
- RIP `a4208061e68425b0d` closed after unit 4 (`cbb1ebca`: the claims receipt commits to the validated construction — the
  copy compared counts to the DECLARED shape and took no tables at all; the "five hand conversions" were a wrapper in the
  wrong layer, deleted as a second name). Unit 5 (the hits==2 layer, 894 items, clusters first) → **`ae0b9b375646fb099`**.
- Stopped the closed V0 lane and the superseded first General lane (both were still registered as running).
- Cuts: `815bea319`.

### Lane map delta — 2026-09-02 05:05
- REDEMPTION `a07bc54d2e4753bc9` closed at ~795k → WEB **`afb43355b9428a30c`**. C-12 closed on the derivation side
  (`eb2c6e99` phase zero, `c0bd9f53` ATA default, `84bc6026` admission path, `d8b1f30f` resolved-market live test proven down
  to the chain's refusal on the OPEN market). **Finding: browser redemption has never worked** —
  `walletTerminalPayoutV3.ts:967` compares a deployment field that never existed, so every payout throws; it hid in the
  "14 type errors, none mine". Fix known (decode the activation cache through its owner); it reds two fixtures that only ever
  passed a length check. Successor: that fix + honest fixtures, the other 12 convicted errors (incl. the SDK's missing
  `sendRawTransaction`), the a11y 223, then the cohort-12 resolved-market run.
- Cuts: `8009a825c`, `d00396b4d`.

### Lane map delta — 2026-09-02 05:30
- GENERAL successor: **OpenBatch N=2 COMMITS** (`5afef490`, 862,319 CU, four real accelerator chunks) — the host mis-modelled
  the System program observation (upgradeable-loader view vs native loader); every logical coordinate is now asserted against
  the bank before submission. Transport binding **wired at zero frame cost** (`73f802f6`: the pair is re-derived at the join
  instead of travelling). N=258 walls: heap aborts at **N=14** (~4.5 KB/outcome) and scalars 163+6·(N−2) vs
  `MAX_HOT_SCALARS_V3` 512 — per-outcome cost and the ceiling's owner are the next unit. Frameguard baseline stale (2,688 vs 3,840).
- DIRECT successor: the fee-fill CU wall premise was **FALSE** at HEAD (no two-CPI branch; fee-bearing floor 1,299,128 is 131 CU
  below zero-fee; `81dfa412` corrects the population doc that consumed it). The real wall: **both margin-gate ratchets RED at
  HEAD** — the Direct Hot floor rose +31,199 CU since 08-31 (worst-seed margin 70,870 of 1.4M); largest non-CPI consumer
  `project_accounts_atomic` 144,016 CU (`2cb59a07` splits the band). Parked patch landed (`74e044cf`: the System program
  left the static set — the pin measured a packet the protocol never emits). Closes green. `direct_fee_settlement_v1::settle`
  is the deepest frame in Trading (4,032/4,096). Next: bisect the regression; resolution/redemption/portable-ticket; the frame.
- EMISSION successor: **unguarded emissions 15 → 0** (`e4d950a7`, `82d07aef`, `a528ac33`, `538576c5`, `f1b3e1d0`, `f00f137a`);
  `f49089cf` a header coordinate the codec read differently from the emitter. DEALER `074a30ed`: the equity Add's transcript
  was 9.5 MB of ELF with a two-author tail.
- Cuts: `307bb3526`, `eb3ee1060`, `64b9d9d73`.

### Lane map delta — 2026-09-02 06:00
- POPULATION `ad6d48cc39f881e8e` closed: **release candidate GREEN at `e39efbb0`** (first in this cohort line; four one-line
  defects between the frame fix and green, two of them the stale-lock class → `90cc4b24` adds a 28.7 s `locks` CI tier over
  70 workspaces). Cohort-12 staged whole (`~/jobs/dclutch-cohort12-20260902/`: keys, runbook at 50 bps, ELFs byte-identical
  twice) and stopped before the irreversible close. → COHORT-12 **`a3889c0783d45215b`** executes the spending half: close
  cohort-11, redeploy, ladder, found at 50 bps, admissions, a FEE-BEARING trade + settlement, census across the fill.
- DEALER `074a30ed`: the runtime transcript hashed **9.5 MB of loader programdata** (~4.75M CU) — the earlier "Add executes at
  1.04M" came from artifacts with frame diagnostics and is struck; loader bytes are identity, not prestate (four sites);
  `tail_count` had two authors. Honest Add now reaches the accelerator: `0xD001` with one code for the whole view → split.
  A disarmed hostile (`assert!(true || …)` from `ac24f70c`) must be re-armed when the Add executes.
- RIP unit 5 (`f49089cf`, `d07885d4`): 30 generated offsets wired where generic helpers hardcoded them; preimage→id pairs
  (94) verified by no gate → unit 6 adds it to census; 21 test-only shipped builders to adjudicate. Three operator geometry
  tests red since `e3298c9a` → GENERAL.
- EMISSION successor: unguarded emissions **0**; GeneralV5Assurance corpus (`02a0f461`); next the 224-byte cursor and the
  verified-candidate record (no Lean author), `le_numeric_id`, then a Lean-emitted TS register.
- Active: DIRECT `a8931c4eecc29c8fb` (bisecting +31k CU), GENERAL `a39412e2663ca5f8d`, DEALER `a929bd24b931c8297`, EMISSION
  `addb8204ef3d72bb0`, STRUCTURED `ae1559fc2d9501927`, WEB `afb43355b9428a30c`, RIP `ae0b9b375646fb099`, COHORT-12 `a3889c0783d45215b`.

### Lane map delta — 2026-09-02 06:45
- STRUCTURED successor closed at ~740k: **redeem→retire EXECUTES through the real Trading Hot route** (`b26d66f6`: the fiftieth
  account is the Resolution program; six hand-typed per-index tables collapse to one `declared()` match over roles; all seven
  ELFs byte-identical); campaign census 30 → 0 with Trading in its program map (`56fa0895`). K=5 sized, not begun →
  **`a9435d78baa5d7612`** in a detached worktree (ruling: mandatory-zero header fields carry no information, verify at every
  producer before taking the −64). Found: three wallet-payout CU budgets red since 08-27 → CLAIMS **`a372d0ae55f06b58a`**
  (zero-payout shape 223,244 → 316,946 in Claims' own frame; interval 08-27 → 08-31).
- DEALER `83f9e6e6`/`789b61fb`: the accelerator boundary has four named codes; the dealer accelerator **had no allocator**;
  the honest Add's accelerator returns an ACCEPTED ack at 469,516 CU; the hostile refuses `equity:Claims` re-armed. Wall: a
  1,392-byte bank through a 1,024-byte return-data limit → chunks that re-authenticate (General's OpenBatch pays the same at
  four chunks) → Dealer designs the scratch-page output channel once for both accelerators; General adopts.
- DIRECT: the +31k CU was already localized by `08294e17` (a process miss); +6,233 unattributed, bisecting; `settle` frame
  4,032 → 1,728 (`2c51ecd1`, the by-value move, not my suggested borrow which measured 5,504/14 diagnostics). C-04's last
  three clauses are real gaps: no Trading resolution route test for a Direct market, redemption host-only, ticket envelope
  never crosses a program → building them.
- Cuts: `6220f01b4`, `79abd2210`, `673d75170`.

### Lane map delta — 2026-09-02 07:30
- K=5 (`a9435d78baa5d7612`): **my mandatory-zero ruling was VOID** — `realm`/`collateral_recipient` are mandatory NONZERO for
  RedeemTerminal (request.rs:487-489, five producers). The packet reading's premise was also wrong by 69 B on the Hot route.
  **Ruled: −288 plus an action-conditional header as ONE revision** → Hot Issue/Unwrap K=3 at 1,197, Claims-direct K=6; the
  honest maximum, not a protocol-wide K=5. Distinctness debt named; witness provenance to correct.
- DEALER `76f4c9eb`: both channel proposals refuted (no page on the route; accelerator holds nothing writable; a page must be
  accelerator-owned; identities are not uniformly frame keys); **74% of every chunk is re-authentication** — one CPI fits,
  two never will. → design reading lane **`afe080030dc5b0ada`** (Fable): options a/b/c/d/e with the ladder, one recommendation.
- RIP unit 6 (`f4725f0e` preimage gate 99/99 red both ways; `10c56d61`): **`tools/relayer` `seed_deployment_slot` is never
  called — a relayer restart re-admits an upgrade it refused** → unit 7 wires it. Five operator builders are vertical-slice
  gaps (live routes, no campaign). **census `--check-unique` red at HEAD** from the BANDS move → EMISSION fixes bands.rs.
- EMISSION: 82/82 guarded; `9ecb8aec` identity order Lean-owned; `1d8b999a`/`1da89dfd` refusal bands emitted through TsEmit
  into both trees (twin identity 158/158, honest count); five TS modules one emitter away → in flight.
- WEB `8baf2c9f`: **14 type errors → 0**; the browser could never have submitted a trade (SDK lacked sendRawTransaction).
  DIRECT `ec451bbd`: fee completion executed for days and read NEVER-EXECUTED because nothing bound it.
- Cuts: `929eaa683`, `b1e05164d`.

### Lane map delta — 2026-09-02 08:15
- RIP `ae0b9b375646fb099` closed (5 commits): `d3ca2bec` **the relayer's refusal survives a restart** (seeded from the
  artifact manifest it already writes; unseeded control accepts the refused upgrade). Census gate green again with the
  preimage check (`3c0cf2d7` by EMISSION fixed bands.rs): 380 codes / 26 bands / 270 magics / 99 of 99 identities / 167 routes.
  Five vertical-slice gaps (live routes, operator builder, no campaign) → SLICES **`ae6c830801525563c`** (claim-check
  redemption + escrow close; dealer checkpoint rollback + custody rollback; delegated custody).
- DEALER `a929bd24b931c8297` closed: finalization is one-way (`7e135a7d`, Registry's four verbs); the flag belongs in the
  activation cache (70 → 46 locks). C-06 waits on the channel ruling (`afe080030dc5b0ada`). `frontier.rs` red at HEAD
  (`Break::RootPrestate` 0x4001 vs asserted 0x4002) — unowned.
- DIRECT `db4d5ff4`: the +6,233 bisects to `5de38ef2` — nine lines, eight comments, one empty macro moved the margin
  statistic by 4,836 → **the gate's key-independence claim is false** (a relink moves it through the release-set digest that
  seeds capability addresses; the eighth search is uncounted). Not pinned. Repairing the instrument, then the C-09 harness
  extension (Trading + Claims loaded; a Direct root resolved on real ELFs), then redemption through real Trading.
- GENERAL `c291f7a3`: heap = 59,376 + 528·(N−2), 480 of 528 is repetition across eleven full-width banks; OpenBatch reads no
  item tail → stride 0 makes N=258 cost what N=2 costs; OUTCOME is a second author (13 writers, always the index).
  Frameguard recaptured (`afa556f3`), gate runs again.
- Cuts: `d504d189f`, `ea37964a3`, `7968cae29`.

### Lane map delta — 2026-09-02 08:45
- CHANNEL reading (`65c6e524`, `docs/design/ACCELERATOR_OUTPUT_CHANNEL_2026_09_02.md`): every diet refuted by arithmetic
  (FrameReference fits only the Add; the honest minimum fits Add at exactly 880 and fails Remove/General; chunk-trust costs
  ≥160,904 CU/chunk and rests on unpinned return-data persistence). **Recommendation: an accelerator-owned, client-provisioned
  output page** appended to the admitted frame, ack = header only, Trading hashes the page against the digest the ack already
  carries — one CPI, no loop. → CHANNEL build lane **`a93f86b9a8b48aebf`** (contract first, inert; dealer then general
  accelerators; nothing switched on).
- **RULING FOR EMBER (decision 0003):** is an admitted accelerator that owns exactly one client-provisioned, digest-bound scratch
  page — written only inside its CPI, read only by Trading in that window, never read by any route — still the "stateless
  accelerator" 0003 admits, or does 0003 need an amendment saying so? Nothing flips to the new profile on devnet until this is
  answered. Recommendation: amend 0003 to say so; the invariant weakens from "owns no account" to "owns one scratch account no
  route ever reads", and the General census gets a stronger measurement (page == digest preimage).
- COHORT-12 in flight, progressing from its own stage logs: cohort-11 closed (+41.89 SOL), seven programs redeployed and
  verified, ladder complete, **market founded at 50 bps**, every routing table read back frozen (authority None, 56/62
  addresses) — the freeze's first on-chain proof; deployer 36.56 SOL; admissions and the fee-bearing trade next.

### Lane map delta — 2026-09-02 09:10
- EMISSION `addb8204ef3d72bb0` closed at ~720k → **`a33e56a925eb6b78b`**. Landed: census gate repaired (`3c0cf2d7`: bands
  read from the emitted table); all six V2 runtime records derive (`5fa46416`, 73 → 177 constants, all 73 prior values
  unchanged; a consistent move leaves every test green — Lean is the only catcher); protocolInfrastructure emitted to TS
  (`9533e300`; the scraper had already failed since `a00fc7c9`). 84/84 guarded; twin identity 157 (honest).
  Scope correction: coreFound is 39/95 (four Rust layouts have no Lean owner — successor's unit 1); directInlineV3's
  generator carries an eleven-way route-binding gate and must not be replaced as-is.

### Lane map delta — 2026-09-02 10:00
- **COHORT-12 LIVE** (`b4ddd2c8`, `docs/evidence/COHORT12_GENESIS_POPULATED_2026_09_02.md`): cohort-11 closed (+41.89 SOL),
  seven programs redeployed from `e39efbb0` (42.03 SOL, byte-identical on read-back three ways), ladder 33 tx / 0 errors,
  **SOL/USD OPEN at `EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1`** at 50 bps with cuts 9800/10200 (spot $100.04 read
  from three venues; the runbook's $150 assumption corrected), two admissions landed, census L1/L3/L4 hold. **Frozen tables
  proven on chain with cohort-11 as control** (four of its five still authority-held). Deployer 36.56 SOL.
  **The trade did not execute — a conflict between two of our rules:** checked execution needs a sealed upgrade set
  (`prepare --deployment-set-journal`, five receipt-backed Upgrades) and a full redeploy upgrades nothing; cohorts 7–8
  (upgrade lineage) sealed, 10/11/12 (full redeploy) None — **why no Direct fill has ever run on devnet.** Resolution: rule (a)
  forbids a PARTIAL deploy, not an upgrade of the whole set → upgrade all seven in place on cohort-12's ids from HEAD, seal,
  trade. Also: simulator preflight wrote a durable journal `--execute` resumed (0x4003 Content on the first attempt).
  Structural fix owed to the release tool: a genesis deployment's receipts should seal too. Retirement is owned-loopback only.
- WEB (`aeb01cd2`, `492d6bef`, `8baf2c9f`, `49f64be6`, `0b576d67`, `0f1d75b2`): payout fix convicted on chain against cohort-12;
  **the browser had shipped a closed cohort** (stubs executable, ProgramData absent; the live test asked the stub) → repointed
  to cohort-12 with ProgramData read; **no client could decode any Registry cache** (bump offset 12 vs five TS mirrors demanding
  zeros) → fixed both trees; 14 type errors → 0 (browser could not submit a trade); a11y 223 → 196. Next: converge
  operatorSurface onto the SDK owner (upgrade-authority binding, route-specific release admission); opacity in the survey.
- DIRECT `42840630`: a real relink moves the margin statistic by **17 CU** — the gate IS key-independent; db4d5ff4's inference
  retracted in place; the 4,836 is probable codegen on an inlining-sensitive route → symbol diff, then fix the boundaries.
- K=5 (worktree): Lean vocabulary + contract green (`1480a5e6`); K=6 corrected to **K=5** by the +12 price instruction; the
  back half is a positional RequestProfile register contract → continuing in the worktree.
- Cuts: `cb7594d9d`, `926d6e99c`, `9fca23065`.

### Lane map delta — 2026-09-02 10:45
- WEB (`3fa1a432`, `61f725a8`, `dfab77e1`): the browser's 377-line fork of the deployment surface is a re-export of the SDK
  owner (proven both ways against cohort-12 by flipping one bit of a real ProgramData header); `generate-capability-surface.mjs`
  had gone blind wherever a shim is — **71 modules recovered attribution**; the a11y survey composes opacity and found a second
  live failure; 196 → 194. Two wasm verifies wait on dirty deps (channel lane's execution-strategy-contract).
- UX READING **`a61b86f8e79649a76`** (Fable): walk the live site on cohort-12 as a stranger, an operator, and a reader; ranked
  ten with sizes; top three first → WEB's next unit.
- SLICES `7f60ccad`: the delegated-custody campaign drives the shipped builder and the defect it hid is chain evidence.
  COHORT-12 `2613e7d6`: the preflight no longer writes the journal execute resumes. EMISSION `c65d5cca`: two more browser
  ABIs emitted; the terminal action byte loses two authors.
- Cuts: `6bf9793ed`, `59d49c1ad`.

### Lane map delta — 2026-09-02 11:30
- K=5 lane `a9435d78baa5d7612` closed at ~755k → **`adfb985d94c714707`** lands it. In the worktree (`ce4b2cb1`): every
  target compiles; the register question collapsed (a register is a pipe between parent and child wire, removed at both ends);
  profile ceiling derived 3 → 6 (decision 0011 s3b corrected in place); structured request 968 → 576, selected 648 → 444,
  terminal 648 → 508; child max outcomes kept at 3 until K=4/5 execute. Owed: TS twins, extent re-pins from a run, the
  island, then the contiguous series onto the live tree.
- COHORT-12 (`12b470e3`): the upgrade route refused four ways, key-free, deployer unmoved — any upgrade mints a new
  release_set_id and strands the founded market; same-bytes upgrade refused by design; **Wall C: the checked candidate builds
  Trading with `hot-cu-profile` while cohorts deploy the ordinary build, so the gate can never seal any cohort's real bytes**;
  Wall D: the journal's `already-current` disposition (byte equality, no upgrade) has a reader and no writer. → the lane ships
  the ordinary link in the candidate, writes `already-current`, seals cohort-12 in place, trades.
- CLAIMS `5767be46`: **−100,414 CU** on the paying payout shape — the activation cache decoded twice per role (48,659 → 25,417
  per call × 3) and a full frame re-parse before the same parse; budgets moved with reasons; Custody's own +12,504 unlocated;
  the same 1,500-CU search-depth lottery under the Claims budgets → adopt Direct's modelled-attempts instrument.
- DIRECT `48c89c57`/`7af004ef`: the symbol diff is identical (0 of 948 functions changed) — the 4,836 was the MEASUREMENT: the
  gate's floor is a min over 32 seeds with unmodelled searches reseeded per relink (three pairs: +17 / 0 / +4,836). Fix
  written and parked (the last unmodelled search, the Claims caller authority bump); applies when the tree is green.
- WEB closed (`fc37c597`): a11y stops at 194 with the measured reason; two wasm verifies wait on execution-strategy-contract.
- Cuts: `ec4993abc`, `7b961160a`.

### Lane map delta — 2026-09-02 12:15
- GENERAL `ea4c46e0` (Lean-first, one commit, 21 files, tree kept green in a worktree): **OpenBatch N=258 executes and commits**
  at 861,225 CU with peak heap **flat at 58,324** (the old identity's intercept; 59,376 was the N=2 point) — the batch actions
  declare a zero item stride; emission census moved exactly six rows; both hazards closed with hostiles; OpenBatch/CloseBatch
  legacy-packet-safe at 258 without a table. Lane `a39412e2663ca5f8d` closed at ~700k → **`aefe5eec9ac4b7175`** (the two
  protocol-wide heap repetitions: the throwaway span-width banks and the projection rotation; OUTCOME design note).
- SLICES closed (`796a71c3`, `c4533f0f`, `7f60ccad`, `ba012c24`): the routes were already witnessed — the finding was two
  authors, and **one shipped bug**: `delegated_custody_transfer_cpi_v2` put the Realm content digest where Custody requires
  the raw-record PDA (refused `0x6004` on chain with the fix reverted; its only test asserted shape, not the frame). Owed:
  dealer-checkpoint's 6 census problems (the Add's refusal moved 0x4003 → 0x4004, two bindings name the old code) → the next
  Dealer lane; five claims-family CU rows red → CLAIMS; the re-ingest idempotency test waits on splitting `Transition 0x4004`.
- Disk: 132 → 202 GiB free after removing closed lanes' worktrees and targets under /private/tmp (dealer, core, old
  Structured, the heap probe, cohort-11's builds). Still held for live lanes: general-lane 26G, target-structured 13G, pop, s7-cu-target.
- Cuts: `43d7b2e30`.

### Lane map delta — 2026-09-02 13:00
- UX READING (`1d6af33a`, `docs/evidence/UX_WALK_COHORT12_2026_09_02.md`, 44 Chromium captures): **the cohort-12 market's
  Direct trading is founded but NOT activated; capability entry 0 must be activated by slot 492,091,890 (~9 h from ~05:00
  system) or the market can never trade** — the sealing work has a clock; COHORT-12 told. Top three → WEB: the site's
  editorial names six markets on a closed cohort and not the live one (front door, /live, /activity all refuse); trade step ①
  hidden behind the activation gate so a stranger cannot connect or join though the chain admits; the market's question,
  cuts and window not derived from the records (1,173 words, zero "$").
- DIRECT `d43cc47c`: the margin statistic is a constant (residual 2 CU across 32 draws; null pair zero; every floor fell by
  exactly 1,500 — the last unmodelled search was the Claims caller authority the fixture derived and discarded); pinned.
  `f3d555a6`: the C-09 harness activates Core in the Claims role and Custody in the Trading role — 3 of 5 with real roles,
  `0x8005` at 92 sites. Lane `a8931c4eecc29c8fb` closed near ceiling → **`a2b6881d3727766fd`** (0x8005 surfacing, five real
  roles, resolve a Direct market, redemption through real Trading, the ticket path).
- CLAIMS `0f69918c`: wallet-payout budgets green at their 08-27 pins; the raise reverted.
- Cuts: `79ca99b41`, `af50627a5`.

### Lane map delta — 2026-09-02 13:40
- WEB `afb43355b9428a30c` closed (11 commits; last `d106bff3`: the "permanent address" prose corrected — it had listed seven
  deleted ProgramData accounts as permanent) → **`abcbd5c6f9e5eace9`** takes the UX walk's top three AS DERIVATIONS (registry
  entry, question/cuts/window from the chain's records with the fixture as fallback; trade step ① outside the gate; the stale
  strings; /create's band from the live spot; the two wasm verifies when execution-strategy-contract is clean).
- CLAIMS `a216bbfc`/`e6142026`: Custody and Core each decoded the same cache more than once per frame (−26,222 and −24,820 CU);
  Custody's +12,504 was never a conjunct; every claims-family row green at its 08-27 pins; the Token-2022 audit digest refused
  by name before eight SBF builds. Next: frameguard and docs/reference from a detached worktree at HEAD; one author for the
  attempts model in program-test-evidence.
- DIRECT old lane closed: both margin ratchets **green and pinned** (`d580f4f7`, `a0852855`: fee-bearing 1,297,792, zero-fee
  1,297,923 + 1,500 slack; ±124 stated as residual noise).
- Cuts: `cd68a9ed9`.

### Lane map delta — 2026-09-02 14:15
- COHORT-12: **Direct capability ACTIVATED** (`2hr4RJJT…`, root `88jJTMmU…` exists, ~185,000 slots before the deadline) — the
  deadline's premise was wrong: activation needs only plan/market-input/report/payer; only the FILL needs the checked release.
  My coupling; the lane read the command's arguments before racing. Both release-tool fixes landed: **`28ff0823`** the
  candidate ships the ORDINARY trading link (profiled build kept as measurement; two new refusals proved red on cohort-12's
  real descriptor; candidate green, twelve links zero diagnostics); `devnet-deployment-set-already-current-v1` writer in-tree
  (key-free, byte equality against a fresh observation, refuses a bound receipt). Seal blocked at HEAD (the candidate certifies
  the commit it is handed; deployed Trading is e39efbb0's ordinary build) → **seal from a branch at e39efbb0 + the two tool
  commits**, then the fee-bearing trade; if the digests differ, cohort-13 from HEAD. Evidence `d4433646`. Deployer 36.56 SOL.
- Cuts: `af9aaa60f`.

### Lane map delta — 2026-09-02 15:00
- COHORT-12 `a3889c0783d45215b` closed (~670k). The seal branch reproduced the deployed Trading bytes to the byte
  (`b0cff55a…`, candidate green at `96a3b04e`), `require_rent_exempt` compares against `Rent::default()` not the live sysvar
  (cleared operationally, 0.2373 SOL; real fix owed), and **the root of why no devnet fill has ever run:**
  `PERMANENT_DEVNET_UPGRADE_TARGETS_V1` (`upgrade.rs:129`) hardcodes cohort-7/8's seven ids (now closed) and the capture family
  accepts no caller-supplied set — condition (a)'s fresh identities and the sealing machinery's permanent identities are
  mutually exclusive by construction. Evidence `0ea1366a`. Deployer 36.33 SOL.
- **RULING (under the standing goal; ember may reverse): the target set becomes an authenticated INPUT** from the plan the
  ladder already authenticated, with the journal's per-row chain re-read as the safety and an explicit refusal when plan and
  chain disagree; decision 0012 amended by one paragraph; the Lean admission model verified not to name the constant first.
  → SEAL **`a4794424565254512`**: the amendment, the rent fix, seal cohort-12 from the branch, the fee-bearing trade, settlement,
  ledger-census across the fill.
- Cuts: `c18efc5c9`.

### Lane map delta — 2026-09-02 15:30
- CHANNEL closed (`93bd4f60` contract — 33 lines inside an existing emission, zero census rows; `4f30d4ce` dealer; `0f53b668`,
  `a4c5add4` general): **the output-page transport is built for both accelerators, inert until a Strategy record names it**
  (ember's 0003 ruling). Measured: General OpenBatch N=2 whole bank in ONE CPI at 51,404 CU (one of its former four chunks was
  50,201); the equity Add runs its whole route in one CPI at 455,790 and exceeds the budget in the tail with **3,773 left** —
  the route's own weight, not the transport. Refusal census 383 → 386. `ScratchPageKindV2::Candidate` deleted (no producer
  ever). **INPUT scratch pages have no live producer — General cannot execute on a real chain today** → GENERAL successor.
- DEALER successor **`a5b1310baba889f9b`**: the equity route's weight (the accelerator's 131,202 artifacts phase re-authenticates
  records Trading already committed to — the double-decode class the substrate sweep did not cover), `frontier.rs` red
  (unowned twice), then the 70 → 46 lock design.
- Active: SEAL `a4794424565254512`, WEB `abcbd5c6f9e5eace9`, DIRECT `a2b6881d3727766fd`, GENERAL `aefe5eec9ac4b7175`, CLAIMS
  `a372d0ae55f06b58a`, EMISSION `a33e56a925eb6b78b`, STRUCTURED-landing `adfb985d94c714707`, DEALER `a5b1310baba889f9b`.
- Cuts: `5b4674565`.

### Lane map delta — 2026-09-02 16:00
- CLAIMS closed (`bb05b497`: the attempts model has one author in `program-test-evidence::pda_search`, pinned over the whole
  256-value bump domain; the claims-extended subtraction itself owed a census of every PDA search on the payout route). Two
  instrument findings: **the census reads only `*b"…"` magics — 51 Lean-emitted hex-array magics are invisible and a real
  collision hides there: `DCLTLBV2` claimed by RAMP_MAGIC_V2 and SPLINE_MAGIC_V2 in the shipping payoff codec** (genref blocked
  at HEAD by two adjudications that lost claimants the same way) → EMISSION, ahead of its unit; and **an exact frame ratchet
  cannot be recaptured by a bystander in a tree taking a program commit every ~4 minutes** (three correct recaptures, each
  invalidated) → FRAMEGUARD **`(new lane)`**: capture at a named commit, a `--since` mode naming who owes rows, recapture once
  with attribution.
- Cuts: `e045ae796`.

### Lane map delta — 2026-09-02 16:30
- STRUCTURED landing (`9785fd92` physical ABI v3, 45 files; `3510ab87` browser/SDK action-conditional read): **the packet wall
  for full-width Structured issue/unwrap is gone** — Claims-direct K=3 at **1,005** bytes on the live ALT (was 1,397); terminal
  Hot CU 1,003,826. Three defects the compile-only handoff could not see: a u64 written over the request magic (the emitter's
  hand-list and its theorem disagreed by one entry → one `emittedFields`, pinned both ways), three stale instruction counts
  (fixed lengths are the array's type now), three x==x conjuncts in `authenticate_asset`. Campaign 24/5: the terminal Hot
  redemption refuses `0x4004 Transition` AFTER the Claims child succeeds — a post-execution receipt check behind ~20
  `map_err(|_| Transition)` sites → surfacing treatment; a coordinate costs 74 not 72; witnesses owed a green re-pin.
- FRAMEGUARD lane id: **`a8afaad08d0507cc5`**.
- Cuts: `39c0a040e`.

### Lane map delta — 2026-09-02 17:00
- DIRECT successor (`b466486e`, `5dc77408`, `c346a650`, `aa11906e`): `0x8005` split into five named conjuncts (92 sites);
  five real roles **5/5** and the former universal donor reaches its subject; the conjunct was one literal
  (`trading_program: CUSTODY_PROGRAM_ID` at resolution_core_v3_lifecycle.rs:1961). **A market carrying a Direct root resolved
  on real ELFs** (root planted, not founded; Resolution proven to leave the root byte-identical); four hard-coded row
  literals were second authors of the manifest layout. **The ticket path end to end** from real ticket files through the
  Ed25519 program to hot_v3 executing (1,269,523 CU; one flipped signature bit fails it). Redemption premise corrected: the
  operator `build_wallet_terminal_payout_v3` has never built an instruction any chain executed — the 12 payout tests
  hand-build the frame with no Trading in the bank → byte-identity comparison of the two builders first.
- Cuts: `76a84c504`.

### Lane map delta — 2026-09-02 17:30
- GENERAL successor (`d3db8585`, `d5f988b4`, `1fee82fa`, `868a7f0c`): the width derivation's six throwaway banks are
  phase-local — slope **528 → 384 B/outcome**, tail-reading actions' honest maximum **N = 13 → 30** (N=31 refuses `Content`
  — the scratch allocator's "out of scratch" wants its own discriminant); batch flat at 50,516. The projection rotation is
  minimal (the pairs are the preplan arena rented early) — premise refuted. OUTCOME note landed (6 → 5 is 64 B/outcome but
  the register feeds the Claims affine-batch row; a wire question). **The input scratch page's producer cannot be written**
  (the bank carries CURRENT_SLOT so a caller-written page is stale one slot later — proven by a control; 1,072 B does not fit a
  packet; every reader requires it read-only). The defect: `classify_bank_transport_v2` classifies the RETURN-DATA bound and
  Trading uses it for the INPUT transport; every bank fits inline in one CPI (≤ 8,192 vs 10,240) → **delete the requirement**;
  inline input is being landed (four-artifact join, Lean-first). End state: inline input + the output page (ember's 0003).
- Cuts: `08dfa60f9`.

### Lane map delta — 2026-09-02 18:00
- WEB successor (`1f6d668f`, `95adbc7d`, `147afeb4`, `39205fba`, `6c67a2bc`): **the market page is a derivation** — question,
  cuts, outcomes, denominator and window read from the Product record's children through the founding's own decoders
  (the registry keeps only the coordinate name); re-capture 1,173 → **866 words, 0 → 16 "$", 23 → 0 hex ids**, "SOL/USD —
  which side of $98 and $102"; step ① renders outside the gate (live: tradable, no walls, root exists); the window offsets
  named in dclutch-source-contract (were bare 48/56) and emitted; /create's band fills from the last opened market's own
  record; every stale string; 390 px overflow. **Fact: EQnY…'s window closed 2026-09-02 08:18 UTC, nothing has resolved it**
  → SEAL: trade disposition, then resolve on devnet (sponsored push + flagship resolution), hand the resolved market to WEB
  and DIRECT. Owed: a `release` wall derived from the public cut; /workbench's 27 acts need phase declarations from the
  census; the list's per-card derivation; the wasm digests on clean codecs.
- Cuts: `3ca11322e`.

### Lane map delta — 2026-09-02 18:45
- STRUCTURED landing closed (~745k) → **`a34e3fac8f919aa78`**. `6ca28de0`: **the terminal Hot redemption EXECUTES** — the
  cause was entrypoint_adapter.rs:210's get_return_data_into_v1 lending the child REQUEST buffer (508 B after v3) to receive a
  592-B receipt; it grows once and re-reads. Measured, K=3, v0 + live ALT: IssueStructured 1,197 B / 720,278 CU, Denominate
  1,049 / 646,113, RedeemTerminal 1,137 / 1,007,425 — **every action of the family is submittable on common-Hot for the
  first time**; the 74-vs-72 formula derives (structured is 4 B wider than selected); packet and artifact walls agree at K=6.
  **A soundness gap** the v3 revision exposed by making a dead assertion true: a receipt Mint substituted for a coordinate
  Mint in the account metas COMMITS — the shard-Mint alias property left the wire and never entered the account frame →
  successor's first item; witness re-pin from green; the `Transition` split (380 sites, 14 files; five-build bisect makes
  the case).
- Cuts: `ff5af604f`.

### Lane map delta — 2026-09-02 19:15
- FRAMEGUARD (`bb28a578`, `65d3f9ee`, `11932b74`, `9059bb58`): the ratchet names its base (`run.sh --at <commit>`, a dirty tree
  refused, `accept` refuses two captures naming different commits); `frameguard.py owed` names the commits that touched
  program sources and left the ratchet red; **the frameguard CI tier had been red at its own hermetic controls since the
  13→12 link change and never built a link** (third instance of that drift); the checker was read out of the measured tree so
  old commits were unmeasurable. Recaptured `afa556f3 → 11932b74` with every one of 39 rows attributed. **Finding:** `9785fd92`
  grew `rational_representation_v2::prepare_and_execute` 2,624 → 3,456 stack bytes and measured none of it → STRUCTURED;
  deepest frame 4,032/4,096 in resolution-proof's `process_direct_funding_activation_v1` → DIRECT. Owed at HEAD: `6ca28de0`.
- Cuts: `def7055b6`.

### Lane map delta — 2026-09-02 19:45
- DEALER successor (`1ceb6653`, `4f4cafd6`): the accelerator searched six records it could read through the seal (−35,862 CU)
  and re-ran the Effect grammar hostile (−75,251); accelerator CPI 460,167 → 376,475; frontier.rs was a stale fixture (8 bytes
  where 45 belong) and `accepted.rs:8516` counted invocations structurally at zero — both fixed. **The last wall is one
  number: `authenticate_strategy_from_sealed_boxed_v3` spends 419,775 CU (30% of the budget), 370,983 of it hashing the
  744,840-byte accelerator ELF on every hot action**, because nothing bound the release's elf_digest to the observed account.
  Priced: the slot-pin swap takes the honest Add to **1,222,307 — EXECUTING and COMMITTING**, 177,393 headroom.
- **RULING (under the standing goal; ember may reverse): decision 0012 governs — `ArtifactRelease` finalization records a
  `DeploymentObservationV1` (one hash once), the hot path authenticates accelerator deployments by the slot pin.** Registry
  change + corpus extension + successor publication stage for cohort-13; cohort-12 unaffected. → DEALER lane.
- Lock design answered: the per-ACTION half is the seal's and free (the six coordinates are already observed under a
  write-once verdict); per-root needs the activation-cache flag; per-strategy is a third group.
- Cuts: `8eb24da70`.

### Lane map delta — 2026-09-02 20:15
- SEAL (`8e1f9850`, `615c243f`): decision 0012's Lean model verified to name no constant (not a formal change);
  `PERMANENT_DEVNET_UPGRADE_TARGETS_V1` retired for `DevnetUpgradeTargetsV1::authenticate`; rent reads the sysvar; **cohort-12
  SEALED at zero SOL** (five roles `equal: true`, seven already-current, `checked_upgrade_set` Some). **But the sealed set's
  `release_set_id` moved** because custody, claims and core derive their semantic release id from the GIT REVISION while
  Trading and Resolution use code-owned constants — the founded plan (e39efbb0) and the gate (96a3b04e, byte-identical
  sources) name different releases; the market is stranded in place (unfillable: missing activation cache for the new id;
  unresolvable: `--produce-input` needs the sealed plan). The window is NOT what refuses (source-level: the Direct fill never
  reads DCLTWIN1). Fix in flight: derive the id from what it identifies; then cohort-13 from a commit at or after the fix and
  the Dealer lane's Registry change, sealing in place with nothing stranded.
- Cuts: `e3835694a`.

### Lane map delta — 2026-09-02 21:00
- FRAMEGUARD closed (`322ddc81`): `owed` attributes through each link's cargo path-dependency closure (12 links over 64
  crates; dev edges excluded — following them put Trading inside the Claims closure); found the dealer accelerator compiles
  Trading as an ordinary dependency, so a Trading change moves its frames. Owed at HEAD: `6ca28de0` (Structured), `1ceb6653`
  (Dealer) — both told.
- DIRECT successor (`005358ab`, `08e93424`): the deepest frame in the tree, resolution-proof's
  `process_direct_funding_activation_v1`, **4,032 → 3,200** (its own split added zero bytes, measured); **the two payout
  builders agree byte for byte** — the campaign now checks its hand-built frame against `build_wallet_terminal_payout_v3`
  coordinate by coordinate before submitting the operator's; the one real difference is the operator refusing a cross-market
  position OFFLINE where the campaign paid the chain; winning-position payout executes with the operator's instruction
  (Claims 356,395 CU). Not yet on a bank whose Trading role is Trading (the campaign's role is a test caller wearing the hat —
  adding the ELF changes nothing; making the ROLE real does) → next, then ActivateCapability founding + ProviderCallerV3::Trading.
- Cuts: `2d1136c40`, `f74d6a6bd`.

### Lane map delta — 2026-09-02 21:45
- WEB (`6f0c55d3`, `5ba04250`, `2b0046fb`, `92fc365f`): the public cut carries a REQUIRED `checkedReleases` map and the
  market page raises a `release` wall when the market's set is absent — live: "TRADING CLOSED · RELEASE … 797e83ac…",
  confirmed independently by the seal lane's finding; /workbench's 27 acts are a NAMED LIST because the census carries no
  phase predicate at all → PHASE-CENSUS **`ae5974c220b124438`** (name the guards in the programs, carry them in the census,
  emit the SDK table); /markets derives every card's question in two observations for the page. Next: /create reads the
  price through the source-provider wasm; the cut ingests cohort-13's checked-release fragment.
- Cuts: `8cace5baa`.

### Lane map delta — 2026-09-02 22:15
- DIRECT successor closed (~730k) → **`aff206e2fee82d0ec`**. `d6c9a7b1`: the rational campaign's Trading role is the real
  Trading program for the wallet payout (asserted by decoding the activation cache out of the bank; **356,395 CU on both
  banks**, the Custody CPI +4,500 = three PDA bands — "a wallet payout does not enter the Trading role" is a number now);
  `61bffa34`: frameguard rows carried for three lanes' commits (5 rows over 1,885, every growth explained by its own commit's
  message); tightest frame now `authenticate_accelerator_invocation_v4` at 3,904 → DEALER told. Successor: the root founded
  through ActivateCapability + `ProviderCallerV3::Trading` on real ELFs (never run); the forwarding wall for the other 46 cases.
- Cuts: `cec798cbb`.

### Lane map delta — 2026-09-02 22:45
- SEAL (`0785bd52`, `2da012cd`, `9576aa48`): **the semantic release id derives from what it identifies** — the shipped ELF
  digest under a role-labelled domain for five roles, code-owned constants for Trading and Resolution; same bytes → one id;
  a 40-hex revision refuses by shape. The carry-forward carries its context's rent rate (the 0.2373 SOL top-up was never
  needed); `devnet_upgrade_dryplan` deleted with its CI row; `prepare` emits the cut's `checkedReleases` row verbatim;
  `validate_prepare` refuses a copied semantic-ids file. **Cohort-12 cannot be repaired backward** (its founded plan embeds
  the revision-hashed ids) → **COHORT-13 `ac90a2f483782f05d`**: close, redeploy from HEAD (or after the Dealer lane's
  Registry change if it lands within the hour), ladder, found at 50 bps with a ≥6 h window, activate, **seal in place**, two
  admissions, the fee-bearing trade and settlement, ledger-census across the fill.
- Cuts: `2ecb7d1eb`.

### Lane map delta — 2026-09-02 23:15
- WEB (`b71d46a6`, `acbdec39`, `a81bd8eb`, `62535976`): /create's founding observation is a PRICE read through
  crates/dclutch-source-provider-wasm's new `read_source_provider_price_update_v1` (the tree's one PriceUpdateV2 decoder; the
  receiver program checked inside the wasm; exact BigInt arithmetic; the band's centre moves, its width stays the author's);
  the cut ingests the seal tool's real `checkedReleases` fragment (a second, different release for a named set refuses; a
  pending cut refuses); the copy on EQnY… says "No checked execution release exists" rather than "waits on". Parked on:
  cohort-13's handoff, the phase table from PHASE-CENSUS, the two wallet-terminal wasm digests (dealer/direct codecs dirty).
- Cuts: `23b2941e1`.

### Lane map delta — 2026-09-03 00:15
- GENERAL successor closed (`a517d27c`, `6fa1a63b` …): **inline input landed** — `input_page_count` asks the declared
  transport, General declares no span, the accelerator reads the bank from CPI data, genesis pages gone; measured N=2 895,492 →
  **797,238 CU** (−98,254; the note predicted a rise — the mechanism was right, the sign never computed), 59 → 55 accounts,
  heap +11,004 attributed to the CPI buffer set. `ScratchExhausted 0x401E` replaces `Content` for out-of-scratch.
  `scratch_page_count` was never a page count → `admitted_invocation_count` across operator/SDK/web. **No program-test can
  found a General market** (the successor is a bin-only crate; no driver founds one) → COHORT-13 founds one on devnet and
  runs OpenBatch there. Owed: refusals.md rows (0x401E, 0x801x) to a worktree after the census fix.
- Landed by lanes not yet reported: `90a8563f`/`271ce0ed` (DEALER: the Registry records a release's deployment; **no route hashes
  an ELF; the equity Add COMMITS with 241,577 units spare**); `1accf9e7`/`a416cd8b` (STRUCTURED: a receipt may not back itself,
  in the account frame; the island's pins re-pinned); `7d24a851`/`4270eb65` (PHASE-CENSUS: the enumerator reads a route's
  phase gate from the guard's constant; routes.md carries admissible prestates); `5ace0cc8` (DIRECT: the Trading caller's
  provider route stops asking for records nobody can make); `8fc0f73b`/`72c488f7` (WEB: the cut goes pending before cohort-12
  closes; three test files had never been running).
- Cuts: `edaf8e7b8`, `ccff22cd3`, `632d972f8`, `e22b153a0`, `9aefc4c78`.

### Lane map delta — 2026-09-03 00:45
- DEALER reported (`90a8563f`, `271ce0ed`, `ada7aa54`): Registry `Finalize` for an ArtifactRelease observes the Program and
  ProgramData it names before the cursor closes (three named refusals 0x1013–0x1015; Lean `ReleaseObservation` with four
  theorems and a twelve-case corpus decided before Rust ran it); the operator's publication step derives the observation so
  every publisher supplies it; `CompleteElf` → `SlotPinnedRelease` at all three hot call sites:
  `authenticate_strategy_from_sealed_boxed_v3` 419,775 → **48,792 CU**; **the honest equity Add EXECUTES and COMMITS at
  1,158,123 CU, 241,577 spare**, post-state assertions holding; Trading's frames byte-identical across 887 rows. Owed: the
  `DeploymentSlotMismatch` discriminant (blocked on Structured's uncommitted 0x401F/0x4020); a newly reachable stage — a
  second LP Open after the Add refuses at OP_IDENTITY_EQ a=18 b=5 (`accepted.rs:8655`) → next.

### Lane map delta — 2026-09-03 01:15
- **COHORT-13 SEALED WITH AGREEING IDENTITIES** — the first full-redeploy cohort whose founded plan and sealed deployment set
  carry the same release-set id (`82a969dd…`), seven roles already-current, final set `e6829ff9…`, at zero SOL with no key
  opened; deployed from `315f1931`; the market is being founded from the SEALED plan, so it is reachable by both the fill and
  the resolution. The inline-input OpenBatch (a517d27c, 19 minutes after the pin) is cohort-14's — an in-place Trading upgrade
  would recreate cohort-12's defect; the General-manifest founding and an OpenBatch attempt against cohort-13's bytes are
  recorded as findings either way.

### Lane map delta — 2026-09-03 01:45
- STRUCTURED (`1accf9e7`, `a416cd8b`, `ac6893e3`, `8e71271a`): the "soundness gap" was NAMING — the chain refused the substituted
  Mint as `Identity 0x5002` all along; now `ReceiptAlias 0x500F` over the presented accounts before any derivation, the grammar
  conjunct restored with its second operand, three hostiles restored (campaign, Bearer, browser `distinct`). **Witnesses
  re-pinned from a green run: 47/47, no transaction over 1,232 in either campaign**, both exclusion lists deleted; a witness
  whose `> 1232` filter returned null when nothing was over went red saying "expected 1397, chain says null". `run-structured.sh`
  ran two of three tests — a `cargo test` filter matching nothing reports success. `Transition` split: 80 sites → `AccountData
  0x401F`, `ChildReceipt 0x4020`, `Width 0x4021` (twelve regex hits corrected by reading); 19 `ChildRefused` invoke sites next.
  Frameguard green; `prepare_and_execute`'s 3,456 is the v3 design and the eighth-largest frame, not the binding one.
  Two trading lib tests red on main → DIRECT (`semantic_join…`) and DEALER (`current_loader_slot…`).
- Cuts: `0e730bf8c`, `d8adf018c`, `7aaa6c36d`.

### Lane map delta — 2026-09-03 02:15
- COHORT-13 read before spending: **there is no devnet General path** — the General market compiler is loopback-only
  (`local_mutable.rs:1527` its only caller; the devnet commands attach only Direct), its docstring names the four lab facts a
  devnet General market replaces (accelerator artifact release, compiler, toolchain, translation validation), and activation
  refuses every external origin twice (`general_capability_activation.rs:296`). General is foundable in principle (acyclic
  entry identities). → GENERAL-DEVNET **`ae460c657082f0c97`**: deploy the General accelerator on devnet (not a sealed role),
  the devnet General market compiler reading those facts from the deployment, activation's devnet arm; OpenBatch on chain is
  cohort-14's (Trading with a517d27c). Cohort-13's founding continues (60 of ~186 tx).

### Lane map delta — 2026-09-03 02:45
- COHORT-13 pre-empted an overclaim: `ledger-census` hard-codes the class claim INAPPLICABLE for an external observer
  (`main.rs:1008-1017`), so L7/L8 cannot be judged by it whatever the chain does; the fill's census newly judges L2/L5/L6
  across a real crossing (chained through --prior) plus L1/L3/L4 — six laws over a real fill — with L7/L8 stated as
  inapplicable-by-construction. → CLASS-DELTA **`a0ad298ac91622479`**: `--declared-class-delta LABEL=I128` in the census,
  the simulator declaring the deltas it caused from the producer's manifest. Founding at 77 of ~186 tx (the frozen-table
  barriers cost the time, as expected).

### Lane map delta — 2026-09-03 03:15
- PHASE-CENSUS (`315f1931`, `7d24a851`, `4270eb65`, `d2195b57`, `20a45ea1`, `95dcc151`): **11 phase guards (10 Core, 1
  Custody) are one constant each** — `MarketAdmissionV1`, a 15-bit set over (Phase, Readiness) indexed by the Lean-emitted
  tags, behaviour identity asserted over all fifteen prestates; the census reads them structurally from the AST (12 gated
  routes of 169; attribution stops at the next route handler); routes.md carries a `phase` column (empty = no gate read);
  the SDK table `marketPhaseAdmissionV1.ts` is generated from routes.md with a --check twin; `evaluateCapabilityV1` returns
  wrong-phase / needs-chain / no-phase-gate BY NAME (8 of 27 acts name a route, 4 gated). Owed: its own frame rows (told);
  `market.found` reading READY on an open market is a context defect (`requiresMarket: false`) (told); the six-ELF
  resolution-core-v3 runner (→ DIRECT); 22 acts and 11 programs still ungated.
- Cuts: `02423de5c`, `578300a4e`, `378439d7c`.

### Lane map delta — 2026-09-03 03:45
- DEALER (`85017c63` DeploymentSlotMismatch 0x4022 with the flipped-bytes-and-moved-slot hostile at the boundary that owns
  the law; `b2147e83` frame rows): the second LP Open refuses because **the market-scoped rent credit fixes one refund
  beneficiary while the LP owner pays the position's rent** — `identity_eq(18, 5)` admits exactly one LP owner per market
  generation, passing only because the campaign made payer and beneficiary one key. **RULING (under the standing goal; ember
  may reverse): the refund follows the debit** — the beneficiary derives from the funding source (the credit's when the credit
  funds, the payer's when a payer is debited), one rule for every lifecycle-rent family; Lean first if the rule is emitted.
  → DEALER. The lock unit's frame move waits behind it.
- Cuts: `a2b98a32f`.

### Lane map delta — 2026-09-03 04:30
- DEALER `a5b1310baba889f9b` closed (~575k) → **`a5c802fed1b3cd9b3`** lands the refund-follows-the-debit rule from its proven
  patch (`scratchpad/RENT-RULE.patch`; NOT in Lean — StateLifecyclePolicyV5Abi is layout only; six fixtures encode the old law;
  the builder mirror at registers.rs:1481 still writes the credit's wallet unconditionally), then the second LP Open after a
  committed Add, then the lock frame move. `85017c63`: `DeploymentSlotMismatch 0x4022` with the flipped-bytes-and-moved-slot
  hostile at the boundary that owns the law.
- STRUCTURED (`5e292bbb`, `143dd997`, `f264b253`, `db50b4fa`): **`ChildRefused 0x4023`** at 16 of 19 CPI sites — the child's
  code packed with the runtime's own u64 into the log (System refusals named for the first time), Trading's family code on the
  wire; the ten dealer-checkpoint rows measured unmoved (105 tx, eleven refusals, all 0x4004); market.rs's site held with the
  reason (needs one devnet market run). Tool hazard found: `commit-patch` commits from the index without writing the hunks
  back to the working tree → fix in flight. A `--only` race with Direct broke main for four minutes; both fixes were right.
- CLASS-DELTA closed (`aeb316d4`, `bf59126d`): `--declared-class-delta LABEL=I128`, the simulator declares from the finalized
  fill document; L8 proven reachable and satisfiable through the census binary. COHORT-13 has the flags.
- Cuts: `31c3e304f`, `00096ac9f`.

### Lane map delta — 2026-09-03 05:00
- COHORT-13: **the founding LANDED** (sealed plan, agreeing identities) and the driver then exited 1 on a transient
  `getBlockTime -32004` after finalization, so the report has no `execution` block and activation refuses
  ("campaign report omitted execution", campaign.rs:368). No fill yet; the census is no longer among the reasons.
  **Third instance of one pattern:** `recovered_finalized_founding` has a schema field, two serialization sites and a refusal
  naming the owed repair, and its only two writers write the literal `false` — reader, schema and refusal built, producer
  never written (cohort-12's already-current writer; the permanent target set). Also: `capture_founding_poststates_v1` re-reads
  an account a later stage consumed, so a COMPLETED founding can never be resumed. → RECOVERY **`ad3313e1364efe1be`**: the
  producer, journal-based poststate comparison, non-fatal post-finalization reads, and the detector (a schema bool with only
  literal-false writers fails a test). Deadline: slot 492,460,566, ~55 h. Fallback: a second founding on the sealed plan.
- Evidence: `docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md`. Cuts: `41dd93143`.

### Lane map delta — 2026-09-03 (resumed 12:50 EDT)
- **The rent RULING above is REFUTED and corrected:** "the refund follows the debit" unconditionally is a theft vector —
  a maker replay root is a shared structure of the market and the same route admits a stranger as payer (measured 2026-08-31
  incident at direct_trade_producer.rs::maker_root_rent_beneficiary_v1); the `payer_debit > 0` conditional adds a griefing
  vector (one donated lamport refuses an owner's Open forever). Landed instead (DEALER `d190297d`, `648fad0a`, `5fc108bd`,
  `c60f853b`, frame rows `fa6fa482`, `7e8f6448`): **whose rent a state carries is a per-plan DECLARATION** — action-plan
  byte five `REFUND_SOURCE_CREDIT = 0 / PAYER = 1` (zero keeps every prior policy's bytes) — the kernel proves the named party
  is one the plan's funding admits; Direct requires `Credit` at both plan readers; Dealer LP declares `Payer`. Two more
  authors of the old law found by landing it (`apply_lifecycle_closes_v3` re-ran the equality at the mutation boundary;
  `direct::lifecycle` never said whose authority). **LP Open #1 → hostile Add → honest Add → LP Open #2 (671,787 CU, second
  owner) → second Add all COMMIT** with a real sponsor. Campaign 30/1; the last wall is the trade leg's V3 RequestProfile
  (borrowed-witness) unimplemented in the off-chain bundle engine (`registers.rs:802`). Lock frame move (70 → 64) unstarted.
- RECOVERY (`00793136`): `campaign --founding-only --execute --recover-finalized-founding` — six journal rows re-authenticated,
  one disposition per recorded poststate, a vacant account a later stage consumed is a pass by that record;
  `recoveredFinalizedFounding` is DEFINED as `recovery_to_complete.is_some()`; the boolean-producer detector landed and names
  the field. **Live: six stages further, then the same shape one verifier over** — `authenticate_open_market_poststate_v1`'s
  funding-ledger loop (market.rs:12468) reads ledgers LIVE against their Pending bytes while journals 3/4/5 exist to move them
  → RECOVERY-2 **`ac3d503c213dc2142`** (the comparison, not the order; siblings :6182/:6312; the detector this class lacks;
  atomic report writes — a refused run rewrote campaign-open.json). **Deadline slot 492,460,566; ~43 h at 12:48 EDT.**
- COHORT-13 lane closed (~550k) holding per `~/jobs/dclutch-cohort13-20260902/HOLD_STATE.md`; nothing spent since founding
  (deployer 32.4739, payer 1.6634). Activation root settled `4GzDzNxj…` (its `2dGxuxe5…` was the founding-PERMIT namespace,
  vacant by construction — caught by the web lane's second derivation).
- GENERAL-DEVNET closed (`66e300f3` accelerator deployed on devnet `8pgnyNvgd…` slot 491,959,038, 1.918 SOL; `325123e9`,
  `d9b5036d` `devnet-general-market` — only ONE of the four lab facts had an author to read (the accelerator release);
  compiler/toolchain/translation-validation became authenticated file inputs, owed; `a34bfb7b`, `a06f60bf` activation's
  devnet arm as a cluster VALUE). Finding: the accelerator identity is a seed of the Market PDA. OpenBatch on chain is
  cohort-14's (Trading with a517d27c); the accelerator's release belongs in `prepare` beside the seven roles'.
- DIRECT closed (`5b2565ad` **the Direct root founded through ActivateCapability on real Core/Trading/Registry, 329,736 CU**;
  `5ace0cc8` the Trading-caller provider route was unsatisfiable by schema (set_v2/V4 vs V1/V3) — fixed with a membership
  conjunct, reachable and not yet run; the forwarding wall is not a wall — common-Hot IS the forwarding; `60e26cf5` the
  six-ELF runner; `9efc24cf` the fee-band hostile).
- EMISSION closed (`c65d5cca` two TS modules Lean-owned — the terminal action byte had THREE authors; `b209be56`, `52bbd463`,
  `c131407b`, `77dad158` the four layouts owned, coreFound 39 → 82/95; `a1cb5217` four modules were outside the Lean lib root;
  `d0c0990f` the magic census reads emitted magics: 258 → 395 declared, **four collisions hidden not one** — `DCLTLBV2` is one
  family with two profiles (adjudicated as shared, not re-lettered); still red: `DCLTDMR1`, `d5e005d1…` (test-local
  re-declarations), `DCLTSTV3` (two record kinds on one value — a real wire question) → genref still blocked;
  `513f0d8e` request_profiles freshness was formatting).
- Housekeeping on resume: 20 working-tree files were stale pre-commit copies of a517d27c's parent (the commit-patch hazard
  67058c86 fixed) — restored to HEAD; nine real uncommitted edits remain (dealer-codec scenario_*, hot_v3.rs, ledger.rs,
  capability_seal_close.rs, relayed.rs) — owners unknown, left in place.
- Cuts: `348935fc2`, `877541ed3`.

### Lane map delta — 2026-09-02 13:30 EDT
- RECOVERY-2 closed (`be012a46`): `authenticate_open_market_poststate_v1` reads through `BoundaryRpcV1` — seven permanent
  facts vs one boundary-time fact (the Pending funding ledgers), later journals as named owners; `LaterFoundingStagesV1` is
  the one owner of the rule; the report writer refuses a replacement that drops evidence (the erased `founding_targets` was
  a literal Null in the first write, not a crash); detector `historical_boundary_reads` over the reconstruction path's call
  graph, red-proven twice. Live dry-run: all 32 poststates resolve; the funding ledger's real movement is owned by the later
  stages. Next candidate wall named: `authenticate_funding_readiness_route_v1(…, "accept")` at market.rs:11359.
  → COHORT-13 RESUME **`a293fa83f654b0965`**: rebuild at be012a46, recover, activate (root from `facts.root`; 4GzDzNxj… must
  become occupied), admissions, the fee-bearing fill, settlement, the census with the recorded flags. ~42 h to the deadline.
- Started beside the resume: MAGICS **`a3d44f2e3d858266a`** (DCLTDMR1, d5e005d1…, DCLTSTV3; genref green; the owed refusal
  rows for five lanes' codes; emission-guard COVERAGE row) and DEALER **`a525f0a87751fcc5a`** (the trade leg's V3
  borrowed-witness RequestProfile in the bundle engine at registers.rs:802; then the 70 → 64 lock frame move).

### Lane map delta — 2026-09-02 14:30 EDT
- MAGICS closed (`944ee9be`, `53f09fa0`, `68078ccc`, `318fc1a6`): `DCLTDMR1` and the DBC discriminator were second copies
  (tests now import the owner); **`DCLTSTV3` was two kinds that must not share** — the Series ticket state moved to `DCLTTSV3`
  Lean-first (the template magic is digested into PDA seeds; the ticket-state magic outlives nothing), `replay.rs` binds all
  five disjoint at compile time. Census 395/375 with 3 collisions → **393/376, none**; **genref exits 0 twice** (two passes
  are needed: two reference pages mirror generated TS) with all eleven owed codes present; `abi:route-census` had been dead
  since 1d8b999a moved the bands into generated_bands.rs — the browser's refusal attribution had no authority behind it.
  abi verifies 17/27; the ten left are one class (readers scraping Rust files for symbols that moved into emitted ones) →
  ABI-READERS **`acfb279c58bb1805e`**. **Fourth producer-missing instance:** `prepare_funding_artifacts_v5` allocates the
  Series ticket-state PDA as 64 zero bytes that `TicketStateV3::decode` always refuses; no route writes the magic — unowned
  (Series is loopback-only through cohort 13).
- DEALER (`27c1cb3d`): the bundle engine reads the V3 borrowed-witness profile from the one on-chain reader (a no-witness
  request committing through the fall-through was the conviction); LP Open 646,287 / Add 1,089,727 commit. **The trade leg's
  wall is in the ARTIFACTS:** selector 9's EffectV4 declares six optional-Custody route spans its RequestProfile never owns,
  and the 384-byte trade header is byte-for-byte full → an ABI change, Lean-first (in flight). **The lock-count frame move is
  superseded:** the seal-backed alias shape already ships for Direct and reaches the same count without moving
  HOT_FIXED_ACCOUNT_COUNT_V3 — measured LP-hot 54 → 48; the equity Add's 70 → 64 blocked by two undocumented distinctness
  copies (`parse_accelerator_readonly`; `require_record_pair` at admitted_composition_v3.rs:805 → 0x4017). Partial patch saved.
- Cuts: `2dca87965`, `46a4b1de1`, `7576f8e32`.

### 2026-09-02 15:00 EDT — THE FIRST DEVNET FILL
- COHORT-13 RESUME closed (`4d9b8d3f` third recovery wall — a producer gap: six journals, four projected rows, fixed on the
  template with a partition test; `236c77ad` evidence addendum): recovery wrote six corroborated rows at zero cost;
  **ACTIVATED** — `facts.root` = 4GzDzNxj… went from AccountNotFound to a 256-byte DCLTCRT1 owned by Trading (the
  pre-registered cross-check held); two stranger admissions; **THE FILL — 1,286,187 CU, landed and finalized** (the drift went
  the other way: 8.1% margin, not 5.9%); **fee settled** (`fee_owed after 0` read from chain); **CENSUS: L1–L8 ALL HOLD, no
  INAPPLICABLE** — 49c8fa92 and be67416e judged by a real fill for the first time; the census halted twice and was right both
  times (L1 by exactly the 201 unbound atoms; L5 by a grown tracked set). Cost **0.138988659 SOL** from the payer; deployer
  32.47385185 unmoved. Three host-tool defects repaired: `activate.sh` never passed `--execute` (a preflight printed
  "planned" and exited zero), `settle-fee.sh` had never parsed (bash 3.2 apostrophe), `build-sim-config.py` named the wrong
  plan and read bindings from a map the admissions cannot appear in. **HOLD_STATE.md's deadline slot was wrong** (real:
  492,169,598 — 9.2 h at preflight); my 40.8 h bound derived from it. The Direct session publishes its finalized evidence
  twice per invocation and trips its own create-only guard after the fill — named, unowned.
- Market 6t3ZnmRuxVKsB4NGrpiQurEwK52xSKVyNqY3tF1ner15 is Open, activated, filled. Next: resolve it when its window closes
  (relay + flagship resolution on devnet — never run there), redeem through the operator-built payout, the browser's gated
  redemption test; the site shows the fill; then close 13 and deploy cohort-14 (Trading ≥ a517d27c, Registry ≥ 90a8563f,
  the accelerator's release in `prepare`) for OpenBatch on chain.
- Cuts: `f0d03ccc2`.
- Started on the fill: RESOLUTION **`a48d6f19b063d2ccc`** (the window read from chain — six hours after staging, 1,800 s wide;
  relay with devnet-sponsored-push-v1, flagship-resolution-v1's phases, the winning position paid through
  `build_wallet_terminal_payout_v3`, the browser's gated redemption test) and WEB **`ac4d471f005f017db`** (the fill on the
  market page derived from the chain's records; cohort-13's simulator artifact ingested; a derived leaderboard).
- MAGICS closed (`2661f675`: a zero-diff frame capture; `frameguard owed` exit 0 — nobody owes). Disk: the previous session's
  scratchpad held 235 GB of closed lanes' targets and worktrees — removed (53 → 246 GiB free); kept `c13-recov` (reserved for
  the resolution lane) and three worktrees carrying UNCOMMITTED work from closed lanes: `seal-wt`, `wire2`, `wt-structured`
  under `…/3db4cac9…/scratchpad/` — to be read, not swept.

### Lane map delta — 2026-09-02 15:45 EDT
- DEALER closed (`27c1cb3d`, `f5d4912e`, `7cb080d2`, `f7754ece`): **selector 9's request declares the frame it executes in** —
  the six {0,14} Custody route-span counts at 384..389 (header `_BYTES_V4`, version 5; `scenario_route_span_counts_v3` derived
  once; the trade header has NO Lean owner — Rust is its author, an emission debt); all nine spans pass; the wall is now the
  account projection: `CrossItemAlias` at 16/28 … 20/32 — five roles the Claims fixed frame and a runtime Custody span both
  name — selector 9's AccountProfile declares no cross-frame alias partition. Unit 2 documented as superseded (7cb080d2): the
  seal-backed alias shape reaches LP-hot 48 / equity Add 64 without moving the frame; partial patch saved.
  → DEALER **`a4e108c1118546d0d`**: the dynamic-span-aware alias partition, then unit 2 from the patch as one series.
- The three "dirty" worktrees kept earlier were pre-landing snapshots (every changed file identical to or behind main) —
  removed; 251 GiB free.
- Cuts: `e328bc44f`, `c8638b219`, `213ac7589`, `40700d238`.
- ABI-READERS (`a6581142`): **abi verifies 27/27 web, 23/23 SDK** — eight readers now resolve the crate's own forwards to the
  emitted sources; the Lean emitter builds the modules its emitters import (every TS emitter imports TsEmit; no invocation
  built it); **two real drifts the red readers hid:** the browser said 117 Dealer identities (118 since 322de4b2), and its
  Claims validator zero-checked the byte that IS the record's PDA bump — **the browser refused every record carrying one.**
  Five dealer-codec files were unpinned-rustfmt noise (restored); three host files were the PINNED form of HEAD (landed as a
  fmt commit); the stale 482-line hot_v3.rs working copy saved as a scratchpad patch and restored to HEAD;
  `registered_terminal_artifacts_v4.rs` is 2,529 untracked, unwired lines of someone's in-flight sibling — left. Owed (lane
  resumed): the capability-surface generator reads tracked files; a bump-carrying record test; runtime_width.rs's cursor
  literals.
- ABI-READERS closed (`1ff89144` the capability surface reads what git tracks — HEAD's generator named three files no commit
  contained; `ef2b8f01` the Settlement Cursor's three authors become the emission, with the guard that a forward must name
  its own record's constant and offsets strictly increase — `methodOffset` had been answering for two layouts sharing six
  names; `38f94a1c` the bump-carrying record test, proven red against the pre-fix spans; `94c92a62` rows: zero moved).
  **27/27 and 23/23 in the live checkout.** WEB landed `0cae44b5` (the first crossing has a page), `1463a678`/`b2a7a83a`
  (cohort-13's simulator record; a chained census), `4cb950d1` — its report pending.
- Cuts: `cb3e9ecbb`, `70a5480cd`, `d63bcf7a4`.
- Started 16:05 EDT: EMISSION **`acf4254eebf7d47df`** (a Lean owner for the selector-9 trade header; coreFound 82 → 95;
  RefusalBandsV1 into the lib root; the reference's stale provenance lines; the Series ticket-state producer gap named as
  design debt) and PHASE-CENSUS **`a5b016449be284441`** (the 22 ungated acts across 11 programs: admissibility guards over
  every persisted state machine become per-route constants; the census reads them; the SDK table grows).
- WEB reported (`0cae44b5`, `1463a678`, `b2a7a83a`, `026fb8ac`, `4cb950d1`): **the market page shows the first crossing,
  derived** — signatures for the Market, each transaction's bytes decoded at directInlineV3's coordinates (both signed compact
  intents ride in the Hot instruction; no per-fill account), priced by the same preview the stepper uses; positions from the
  Position accounts; fee standing from the maker replays (both "settled"); leaderboard live; the single crossing drawn as
  one point. Instrument defect caught: a 429-emptied fill list read exactly like a never-traded market — refused reads are
  now counted before "no crossing yet" is accepted. /pulse re-ingested from cohort-13's census (two producer defects: cycle
  parsed from the stage name; `stage` had a reader and no producer). Front door's phase read off the Core account.
  Captures: market 1,319 words / 23 $ / 3 hex at 1280; 1,240 at 390. Owed (lane resumed): sbomVerify red for hours;
  three census fields dropped by simulator-series.mjs; the SDK absorption of two web-only modules. Note: a stale vite dep
  cache served cohort-12's Core for an hour — `rm -rf apps/dclutch-web/node_modules/.vite` after any deployments change.
- WEB closed (`73827e17` sbomVerify convicted: two refreshed program-test locks admitted their closures and nothing told the
  SBOM — failures 0 before and after, emitted to scratch and diffed before writing; `c9f8f587` the three carried fields, and
  two /pulse charts that had existed since v3 now draw; `d3131840` SDK absorption, twin identity **154 → 166**;
  `478a350e` **the package refused the first fill's transaction over one padded log line** — the web fixed it on 08-27 and
  the SDK never absorbed it, behind a twin exemption). README's milestone paragraph brought current.
- Cuts: `5e1ee8b9e`, `99bdf5822`, `98cb056e8`.

### Lane map delta — 2026-09-02 16:40 EDT
- DEALER closed (`efca6966`, rows `af24774c`): **the trade leg (selector 9) installs its bundle and COMMITS on real ELFs at
  376,030 CU** — a route span may declare a role as an alias of a fixed-frame coordinate (backward, representative its own,
  prestate AuthenticatedRouteAlias; the gate opened on the terms of an already-written, unreachable canonicality guard);
  four more walls fell behind it, each a reader seeing half a shape (trailing-span demand; an empty child request for a
  zero-bank V4 route; V3 base account starts under a V4 span; three fields hard-coded to zero). Page 1 at 93% of the ceiling
  is named debt. Next wall: the delivery activation refuses `CustodySbfError::Replay` — the composed effect expects the
  replay revision read before its own reservation advanced it (sequencing). Owed by the profile: the Custody frame's twelve
  non-endpoint roles as fixed coordinates ahead of every span. Unit 2 (the alias lock shape) still one-series-or-nothing.
  → DEALER **`a2bbdb8d4511b10bb`**: the sequencing wall, the profile shape, unit 2 as one series.
- Cuts: `f64c17f26`.
- EMISSION reported (`2bf7ef90` the selector-9 trade header Lean-owned — real author programs/dclutch-trading-sbf/src/dealer/
  v3_trade.rs; **23 of 28 coordinates were bare decimals written twice**, one selector offset had four authors; census 93/93;
  a name-table transposition the Rust cannot see reds the guard alone; `25792c06` rows; `c7b28ba6` **coreFound's "82 of 95"
  was computed by nothing** — the generator now derives 79 of 97 with an OWNED_FLOOR ratchet, and finds 18 unowned incl.
  dclutch-source-contract's WindowSpec, a whole unowned byte layout; `151849be` the Lean lib root was missing FIVE modules,
  not one, and a1cb5217's premise was wrong — the lakefile glob compiles them, the root's list is the entry point;
  `333043db` the Series ticket-state producer gap named beside the refusing decode — the write authority is granted and
  never exercised). Incident owned: `rm -f fg-a.json` deleted another lane's `fg-A.json` (macOS case-insensitivity) — the
  warning is in run.sh's header; per-lane capture subdirectories now. Lane resumed: the one owed capture, then WindowSpec.

### 2026-09-02 17:15 EDT — THE FIRST DEVNET RESOLUTION (by the failure walk)
- RESOLUTION closed (`d1ab23b2`, `68f0b3da`, `6ab7d66f`): the window (13:22:39–13:52:39 EDT, tolerance 0) had closed
  before the lane started — my "15:00–16:00" was inferred, not read — and the pinned Pyth account had moved past it, so an
  honest observation was unreachable by construction. **The failure walk landed** (37Ye9gaf…, 311,799 CU): Primary →
  FailureCommitted, selector 3, certificate 7S9tCjXT…, escrow debited exactly 1 lamport — after the certificate seat was
  prepaid by hand (a caller obligation the sponsored-push command never had). **Two protocol findings:** (1) a late capture
  (legal until end+max_age, no window conjunct) occupies the head and strands the market forever — Settle, CommitFailure and
  CloseHead all refuse after it; (2) under the failure selector an oracle outage converts into founder revenue (the founder
  holds all 500,000,000 failure claims; participant-2's 200 pay zero). The Market is still OPEN: terminal admission on devnet
  exists only inside flagship-resolution-v1's `accept`, whose input demands relayed-VAA coordinates a sponsored-push market
  never has. `dclutch-sponsored-push-exterior-input-v1` has no producer. Spend 2,866,519 lamports, payer only.
  → RESOLUTION-2 **`(spawned)`**: terminal admission for a sponsored-push market, the founder's payout through the operator,
  the browser's redemption test, the two producer gaps, the griefing-window design note (+ Lean-first repair if small).
- DEALER (`1f41f40a`): **the selector-9 trade DELIVERS** — the delivery's reservation is Custody's own.
- Cuts: `58c0d6594`, `c92480583`.
- PHASE-CENSUS successor closed (14 commits; f47c25fe, 90061c16, 9438c8a1, 69c7b91a …): **12 → 49 of 169 routes gated** —
  claims 19 guard sites (CorePhaseGateV3 deleted), resolution 5 Market + 4 Source, trading 10; a second machine
  (`SourceAdmissionV1`) read by one enumerator with a MACHINES table; **four enumerator over-claims fixed in the dangerous
  direction** (a one-sided `if` attributed to every route published Founding as the redeem routes' set; if/else intersected
  not united; or-pattern arms read as unconditional; children not reading the parent's gate); the abandon route's reclaim
  guard was inverted in the first draft. SDK table 48 rows + 4 other-machine; **live: `claims.redeem → wrong-phase` on
  cohort-13's Open market**, `source.provider → ready`. Owed → successor **`(spawned)`**: six Trading routes behind method
  calls; six unnamed machines (two read `Phase::Open` and mean something else); Registry's 11 routes have no persisted
  guard at all (a distinct fact). Correction: 90061c16's "seven Phase imports" is ten.
- Cuts: `4994ceb55`.

### Lane map delta — 2026-09-02 18:00 EDT
- DEALER closed (`1f41f40a`, `be488cdb`, `aa72e3a0`, `72fa345d`): the handed-down replay diagnosis was REFUTED by a probe
  (revisions agreed; a published reservation carries its records, not its poststate) — the delivery takes Custody's own
  reserve route and **the selector-9 delivery ACTIVATES and COMMITS at 227,742 CU**; the twelve-fixed-coordinates profile
  shape is priced as a wash for the campaign's frame (seven new, seven saved) and a win only for multi-route frames;
  **the convergence half of the alias lock shape is landed** (one declaration; counts byte-identical as control; frames
  953 → 952); the Dealer alias row measured LP-hot 54 → 48 / Add 70 → 64 / Remove 71 → 65 but needs three producers.
  Next: the partial Remove exhausts the 65,536 heap grant (completes at 131,072); `Content` behind it; two operator pins
  red since efca6966. **Incident:** a shared scratchpad worktree (`dw`, named in two of my prompts in sequence) was checked
  out from under a lane — ninety minutes lost; lanes now get private subdirectories by instruction.
  → DEALER **`(spawned, private scratch)`**: heap by measurement first, the Content localizer, the alias row's producers,
  the operator pins, Custody's 15-conjunct Replay split.
- RESOLUTION-2 landed `31d09aed2` — **cohort-13 REDEEMS**: the account terminal payout gets a public arm (report pending).
- Cuts: `167d522ff`, `28cea584f`, `36dbf847e`, `9206c4533`.
- EMISSION closed (15 commits; `eb8439b39` WindowSpecV1 had two authors at opposite ends — Rust literals at the head, a Lean
  cursor asserted as a bare literal at the tail — now one; `f2fa4392` the product admission's four layouts, whose existing
  two-sided byte vector has an env-var escape hatch (`DCLUTCH_WRITE_WIRE_VECTOR=1` greens a moved wire); `a25a2aa77` a
  regression of its own caught by route-binding.mjs, fixed by retiring two scrapers; `0f066f26d`/`ba96d8527` the Dealer
  profile vector as a test, then the census: **124 root-workspace integration targets are compiled by `--all-targets` and
  executed by no tier** (80 cheap, 33 lake, 11 ELF) — refused to wire them unmeasured; coreFound **79 → 91 of 97** by the
  instrument's count; census 95/95). Owed: six coreFound reads; the 700-line browser profile mirror's retirement.
  → CI-TARGETS **`(spawned)`**: measure the 80, wire the cheap ones into a budgeted tier, close the escape hatch.
- Cuts: `9fd134161`.

### 2026-09-02 18:45 EDT — THE FULL LIFECYCLE ON DEVNET
- RESOLUTION-2 closed (`f56b1d2d8` `devnet-sponsored-push-v1 --action admit-terminal` — the builder was separable, the
  command was not (four refusals); **Terminal at 95,854 CU**, winner 3; `31d09aed2` the Claims-role Custody replay's
  missing public producer (fifth instance) — **the founder's 500,000,000 failure claims PAID through
  `build_wallet_terminal_payout_v3` at 353,233 CU**, Hoard 500,000,000 → 0; participant-2's zero asserted from chain;
  `62a0b7fb5` the sponsored-push input producer reproduces the hand-authored document in all 42 fields, the certificate-seat
  prepay arm reads the rent sysvar; `66c78bf52` **the griefing window RETRACTED** — `normalize_authenticated_update` already
  refuses a publish_time outside the window as ProviderWindow; what it uncovered instead: `cadence_tolerance_seconds` is
  INERT on the two single-snapshot Pyth routes → PHASE-CENSUS (holds source-contract); `128394732` evidence, ledger to the
  lamport: 0.0144 SOL, deployer unmoved). **Founded → activated → filled → settled → resolved → Terminal → paid: every stage
  of a market's life has now run on devnet.** Browser redemption test RED at two measured bytes: the frame chunker splits
  by 32 keys where the node's cap is 4 MiB (first chunk 5.27 MB); a Token-2022 ATA carries ImmutableOwner (170 B) and the
  chain refuses every extension → REDEMPTION-UX **`(spawned)`**.
- EMISSION's final correction: be488cdb is doc-only (Dealer lineage, told); dcfb7deb is 307 lines of real code across three
  links (phase-census's scenario admission machine, told). Three attribution errors in one afternoon, all "a true statement
  about one thing generalised to a pair" — the ratchet answers per commit. A `Lane:` trailer on lane.sh commits → CI-TARGETS.
- Cuts: `d986da6b2`, `ba93e26e1`.
- CI-TARGETS closed (`47c0ed143` the `root-targets` tier: **80 never-executed targets measured at 69.5 s warm** — the "ten
  minutes" was a cold target dir; budget 8.00 s enforced on the committed tsv, not a stopwatch; a quarantined target going
  green fails the tier by name; `never-run-tests.py --check` red-proven five ways; the wire-vector escape hatch closed on both
  halves — a write branch refuses after it writes, and five reviewed digests are pinned in `wire-vector-pins.tsv` (five, not
  three: the SDK copy of the bump hints was generated and never checked); `f0a69b2a4` the `Lane:` trailer from $DCLUTCH_LANE,
  printed by `owed`). **Seven red: six are ONE live defect** — since 73ffb0108 (09-01) the Fractional V1 producer's grants
  never met the Close arm's DEBIT|WRITE, so `build_fractional_finalized_artifact_bundle_v1` errs for every input, and it has
  zero production callers; one stale test (`root_lifecycle_projection_v3`, General's frame after a517d27c).
  → QUARANTINE **`(spawned)`**.
- Cuts: `a9fb87828`.

### Lane map delta — 2026-09-02 19:30 EDT
- PHASE-CENSUS (3rd) closed (`dcfb7debb`, `c5485bf21`, `8bf97477f`, `9ca2cba65`, baselines `93d0134a4`/`b8938f4d3`): **49 → 65
  of 169 routes gated**; three machines named (Dealer checkpoint, Dealer reservation, projected Custody — 25 guard sites);
  a guard index over each program's first-party closure; gates inside `for` bodies were a live over-claim (a zero-effect
  scenario commits with the loop never entered); inherent-method indexing silently stopped eight Custody routes resolving
  (found by the third machine); Registry's eleven routes carry `no state machine` as a column value with two staleness
  checks. **LIVE: cohort-13 reads Terminal + Consumed and the evaluator flips four verdicts** (claims.redeem admitted;
  source.provider/ready/create-fund excluded) with nothing in the commits encoding it. **The handoff's "six Trading routes
  behind method calls" was wrong:** fourteen `pub fn`s in direct/{buy_escrow,sell_escrow,complementary,inline}.rs are
  linked into the Trading ELF and reached by no route — only unit tests call them → DIRECT-LAYER **`(spawned)`**: superseded
  plan layer to delete, or a family without dispatch to name. Owed: three machines (DirectRoot, Ticket, FundingLedger);
  two-arm `match` as a selection; derive where the surface decodes the machine; `source.close-fund` NO_ROUTE.
  Declined → SOURCE-TOLERANCE **`(spawned)`**: cadence_tolerance_seconds inert on the single-snapshot Pyth routes.
- Cuts: `d3caa5cf1`, `368da2065`.
- SOURCE-TOLERANCE closed (`0b0a05e93`, rows `4d132a371`): one author for the window predicate (`contains_observation`;
  the narrow site at provider_join_v2.rs:244 calls it); **the finding was sharper than briefed** — a positive tolerance
  cannot reach either single-snapshot Pyth route in any constructible state (three gates: `tolerating_cadence` refuses
  nonzero on a terminal window and is the sole mutator; `decode` routes through it; the obligation refuses non-Terminal
  windows), so the change is the identity everywhere reachable — proven by removing a gate in scratch: END+120 admitted,
  END+121 refused; cohort-13's real window runs through both joins (its actual publication, 616 s late, refuses on the
  schedule bound). The offchain preflight (flagship_resolution.rs:1941) had used the WIDE spelling — the more permissive of
  the two; they agree now. → TIDY **`(spawned)`**: the design note's §4 overstates twice; clippy red in
  source_admission_v1.rs (f6e9b8d08).
- Cuts: `7c7d93b24`, `0c338096d`.
- DIRECT-LAYER closed (`4d13fe2af`, rows `588d280f1`): **programs/dclutch-trading-sbf/src/direct/ was the pre-artifact plan
  generation of the registered Direct family — deleted (6,572 lines)**; three pieces of evidence (inline.rs called itself "an
  oracle, not a family dispatch authority" and hot_v3 grew that oracle from the codec; the landed dispatch 3e4ff9980 called
  neither escrow module; sell_escrow's header contradicted the measured Sell); **the ELF is byte-identical with and without
  it** (the SBF linker collected all of it — llvm-nm finds zero direct::* symbols; positive control 182 hot_v3 symbols after a
  first run with an empty $NM printed a confident zero); Trading frames 955 → 901; the one unreachable admission constant
  retired; census green by deletion. The untracked registered_terminal_artifacts_v4.rs is codex's WIP (letter :184-187,
  "preserve it unregistered"), a real sibling of actions 7/8, and does not compile since this morning's `refund_source`
  field — left. Owed: the codec's now-consumerless registered/complementary planners (a second wave, deliberately not
  taken while the draft sits in that crate). Pre-existing red: the extended-heap-profile admission test → DEALER (told).
- REDEMPTION-UX closed (`e0594084` the frame planner learns sizes then plans under both RPC bounds — cohort-13's frame 32+6
  with a 5.27 MB chunk → 24+14 under 4 MiB, sizing round 5,035 B; the "same context slot across chunks" assertion was
  unsatisfiable (finalized advanced 2 slots in 4 of 4 tries) → every earlier chunk re-read after the last, byte-identical;
  `e7ecfb2ef` **ImmutableOwner admitted** at crates/dclutch-token-svm state.rs:256 and the on-chain conjunct
  rational_terminal_v3.rs:642 with six other extensions still refusing at the same width, operator deriving, wasm regenerated,
  rows carried; `e05940843` the page tells the reader the failure outcome won and what it means per holder). **A real
  wallet's ATA still cannot be paid: Custody's `ExactTransferProfileV1` pins ExactBaseWidthsOnly inside the
  CollateralAdapterReleaseV1 preimage a realm pins on chain** — a THIRD adapter release at the cohort-14 boundary
  (docs/design/TOKEN_2022_IMMUTABLE_OWNER_DESTINATION_2026_09_02.md); cohort-13's 165-byte account was the only
  destination that cohort could ever pay. Live redemption test **4/4 green** on the resolved-and-paid market. Captures:
  1,005 words / 18 $ / 0 hex at 1280. Owed: stage two needs a lookup table someone signs.
- Cuts: `feab86c68`, `196484c79`.
- Started 20:15 EDT: COHORT-14 PREP **`a68057d74bd0c624f`** (deploys nothing): the third Custody adapter release admitting
  ImmutableOwner on the destination (a new release id — the old one's law stands for markets founded under it); the
  General accelerator's ArtifactRelease in `prepare` with its deployment observation; an in-window relay step (cohort-13's
  window closed with no relay inside it); the cohort-14 runbook in-tree — close 13 → deploy ≥ {a517d27c, 90a8563f,
  e7ecfb2ef, the adapter} → both a Direct and a General-manifest market → fill → census → honest resolution → ATA payout →
  OpenBatch on chain.

### Lane map delta — 2026-09-02 21:00 EDT
- DEALER closed (`43106855a`, `0f0d7f57b`, `d4ba8ea71`, `45b48a43e`, `4741cb1c2`, `d89ba826e`; rows `225d64a0a`, `6a95a17ac`):
  **the heap grant does not rise** — the partial Remove died 8,352 under the grant and the table said where: +13,064 inside
  `runtime_transcript_digest_v3`, dead on return (a 7,104-byte owned-observation bank the tree's own doc had named as debt);
  a borrowing digest, preimage byte-identical, peak 65,672 → **58,568** (6,968 under). **CU is not comparable across
  commits of this campaign** — 235 of 307 figures move in 1,500-multiples from random bump draws. `Content` split at its
  thirteen borrowed-witness sites (0x4024–0x4026) and **localized in one run to two first-party spellings of one fact**
  (V4 release: "the sole owner of every borrowed range"; v3_artifacts.rs:502: route 1 carries it) — never reached because
  every equity action ever run carried zero signed positions. Custody's 15-conjunct Replay split four ways (the prediction
  of which gate refused was wrong and is recorded); the operator's two red pins re-pinned from runs (−5 per declaring span);
  `hot-tail-table.py` rendered zero heap rows on any on-chain log; the extended-heap test is red only under the diagnostic
  feature. → DEALER **`(spawned)`**: the ownership ruling, the Remove committing, the alias row as one series.
  TIDY: genref for the seven new codes.
- Cuts: `998e451ca`, `18bdd7853`.
- QUARANTINE closed (`3308d380a`, `60e9b860a`): the Fractional grants derive from `plan_effect_permissions`, the ONE place
  the Close/Create masks are written (const-folded — zero frame rows); the grant alone did not build — `require_owner_anchor`
  refused an Exact coordinate with debit-or-write authority and no RequireOwner, found by probe because a second coarse
  discard hid it; two `is_err()` hostiles had passed for four days against a builder that refused everything. The General
  fixture had three stale things, one the span — `required_observations` now walks the action's own operations and
  `bank_width` reads the profile's stride. **root-targets tier PASS, 80 targets, zero quarantined.** Verdict: the Fractional
  V1 family is producer-missing, not dead (V4 is a separate compiler) → FRACTIONAL-V1 **`(spawned)`**: superseded or
  coexisting, decided by reading as the Direct layer was.
- Cuts: `0caa83e17`.
- TIDY closed (`1bdf5572f` a const match instead of an allow, plus the test the rewrite lacked — three existing tests were
  satisfied by a walk that stopped after its first entry; `84e37949f` the window note's §4 was false at its own read commit;
  `2fe2b9f84` both routeCensus.ts copies were stale AND disagreed on 36 rows — twins 167/167; `b0d3978c4` genref 334 → 341;
  `c036b627b` refusal-registry verifies green in both trees; `fbed0d888` rows). **Two structural gaps: no CI tier runs
  clippy at all** (Cargo.toml's deny table is enforced by a human), and **the SDK has no route-census generator or verify**.
  → CI-LINT **`(spawned)`**.
- Cuts: `647b856c0`.
- FRACTIONAL-V1 closed (`53d73d4ee`, `1967f8282`, `898c87f62`, `c5d2e41d6`): **SUPERSEDED — deleted as a closed cluster
  (6,428 lines, 5 modules, 7 targets, 3 orphaned deps)**; identical seven-action space to V4 at the same discriminants; only
  V4 is authenticated on chain and only V4 founds a market; the three actions V1 alone compiled have no handler in any
  generation. No SBF program linked the operator (the absent signal was checked against a crate that IS linked). The
  lifecycle fold's `Result<(), ()>` became a typed error and the file executed its own claim for the first time. Owed:
  `dclutch-fractional-claim-contract`'s artifacts.rs now has only its own test as a consumer (second wave, named);
  `tools/gauntlet/relayed-vertical` does not compile against today's successor sources and no tier builds it → COHORT-14 PREP.
- Scratch: closed lanes' private directories removed twice (53 → 23 GB; 158 GiB free).
- Cuts: `35c566e94`, `df4845c15`.
- CI-LINT closed (`fd6cd0603`, `b10bcdf02` the clippy tier — 22 s warm, root workspace only (the one with a lints table),
  budget skipped on a cold target; **`--keep-going` stops at a red library, so the first census reached 30 of 105 members** —
  the unit is the package and every run prints clean / red / never-reached; 74 clean (was 30), 11 packages of debt with
  owners in `clippy-debt.tsv`, 22 never reached, **37 members do not inherit the deny table at all**; the 39 refusal-band
  index walks were all inside `const _` so no frame moved; `3976ddeac` the SDK's route-census generator/verify — the web's
  script byte-identical three levels down, both abi-coverage censuses naming the module, red-proven by one byte).
  → CLIPPY-2 **`(spawned)`**: the 22 and the 37. Also: a test function nothing runs in compact_artifacts_v4.rs.
- Cuts: `422cb9136`.

### Lane map delta — 2026-09-02 22:00 EDT
- DEALER closed (`c3e14e096`, rows `d4fb2380d`; `9c133b27c`, `4113be161`): **the borrowed-witness ruling** — the design note
  had named the wrong second author; v3_artifacts.rs:502 validates a legacy V3 twin that never reaches the chain, the
  shipped base (v3_hot_artifact.rs:532) already declares V4 the owner, and kernel v4.rs:803 REFUSES a V4 program whose base
  route carries the bit — so the other spelling was never satisfiable; four readers of one fact became one (three were
  V3-only, each a wall behind the last). **The partial Remove executes its first Custody leg and reaches its Claims child;
  the next wall is compute: ~1.9 M needed against the 1.4 M ceiling** (Trading 1,399,692; the child entered with 94,426 and
  needs ~180 k; two Custody legs and the commit unreached). Five copies of the six alias pairs became one table; the Remove is
  no longer lock-bound. Found with controls: **`trading-outer` is RED at HEAD** (a helper's cfg omits `outer-only`), so the
  programs tier cannot be green; two dead bindings in the Claims preflight. → DEALER **`(spawned)`**: the gate, the dead
  bindings, the Remove's compute wall PRICED (route weight / two-transaction Remove / hoisted child) for a ruling, the alias row.
- Cuts: `f2fc96870`.
- COHORT-14 PREP closed (`d218b963d` the third adapter release **430369ce…** — preimage differs from release 1 in exactly bytes
  10 and 11; `d478c6a5c` a founding SELECTS the newest release, a reader ADMITS any production one — pinning one id would
  have refused every cohort-14 admission with a true sentence about the wrong conjunct; `f8257be53` **a 170-byte ATA
  destination pays on real ELFs, 369,366 CU, the only change 32 bytes in the Realm record**, cohort-13's release still
  refuses 0x6006; `86acf9918` the accelerator's ArtifactRelease as a tenth record in `prepare`, plus eight direct_market
  tests red since 2da012cd fixed (a fixture refusing before its Drop existed); `af928eea3` capture and settle are NOT one
  event — settle is legal only strictly after end+max_age, two hours later; the scheduler's dry-run on cohort-13's window
  says capture 13:23:39 EDT, settle 15:53:09; `4cc6aa2d0` tools/cohort14/ runbook, seal step 04 before founding 05,
  cost 42.26 SOL priced from cohort-13's measured lamports/byte; `f431bf5d1` relayed-vertical fixed — it is not dead and
  the journey tier now builds it). The unexplained −1.9178 SOL is the accelerator deploy (66e300f3), to the lamport.
  → COHORT-14 **`(spawned)`**: close 13, deploy, ladder + tenth record, seal, Direct + General markets, activate, fill,
  census, **OpenBatch on a real chain**, the in-window relay, an honest resolution, payout to a real ATA.
- Cuts: `db3ae5ddd`.
- CLIPPY-2 closed (`7efe71f83`, `b840a2361`, `a6bdf5246`, `31307bcc3`, `f486a7a40`): clean 72 → **89**, never-reached 22 → 9,
  inheriting 68 → **100/105** (the "37 without the table" were 35 hand-copied duplicates of it; the five that cannot inherit
  say why in their manifests — unsafe surfaces, two-workspace crates, dealer-codec's 76 sites); one character under 17 of
  the 22 (`matches!(12|13|14)` → `12..=14`); the census's opt-in ratio was substring-spoofable and spoofed itself (now
  tomllib); a test born without `#[test]` at e78fa027d whose commit claimed its criterion satisfied — wired, red-proven.
  Draining reveals: **dclutch-trading-sbf has 318 sites over 15 lints**, invisible until now — DEALER's. Owed: frame rows for
  the drain (fd6cd060 + b840a2361, jointly, one capture after the Dealer series settles); 7 debt rows with owners.
- Cuts: `9c55d5767`, `24f119b42`.

### Lane map delta — 2026-09-02 23:00 EDT
- DEALER closed (`a0d556b9e` trading-outer builds — `tests/activation.rs` 14/0 where it had no program; `c4e9bb063` the
  Remove's wall PRICED: the accelerator leg 538,821 of which **288,724 (69%) is spent before it evaluates anything**; Trading's
  prelude + the accelerator's = 45% of the transaction authenticating one view twice; the prelude MULTIPLIES per transaction
  so a split cannot save it; `b97ef3e4a` Claims gets its first CU instrument (`claims-cu-profile`): **a completing SignedDelta
  child spends 149,107 of 173,680 (85.9%) re-authenticating what Trading authenticated in the same instruction and 662
  (0.4%) applying deltas** — the Remove's child has never executed a single delta; rows carried, `owed` green). Owed: the
  `outer-only` composer-reachability ruling; Custody has no profiling feature; the alias row (its operator producer has no
  consumer at all — the campaign's green would be evidence of nothing).
- **RULING (under the standing goal; ember may reverse): a callee invoked by a PDA-signed CPI from Trading takes the facts
  the signer's seeds pin as established** — the release set, the role activation, the sealed records — verifying only the
  signer's derivation; the unpinned-caller history stays as a hostile. Decision 0012's argument one level down.
  → DEALER **`(spawned)`**: Claims' two re-authentications and the accelerator's prelude, then the Remove.
- Cuts: `570679f68`.
- DEALER closed (`0aa70478e`, `30d02f5c0`, `93120acfc`, rows `fa00e8f28`; 8 commits): **the ruling applied to Claims — the
  SignedDelta child 173,676 → 80,488** (parse 31k → 22k, releases 76k → 31k, product/basis 42k → 3k); the seeds needed
  nothing added (`role_request_digest = hash(instruction_data)` already covers the plan); the honest split: 45k was a
  redundant triple hostile-decode of one immutable account needing no ruling, 38k is the ruling's; **the Remove's Claims
  child executes and commits, and the transaction reached Custody's second route for the first time**; hostiles 0x5201/
  0x5202 each proven to reach their subject (a shared code would have proved nothing); a suite that read 48 FAILED had not
  run (a wrong Token-2022 build refused by the fixture digest). **Custody: 77–81% of an invocation is caller
  re-authentication** (the Token-2022 CPI is 105 CU) — the macro is now `crates/dclutch-cu-checkpoint`. The accelerator's
  prelude is a chain, so the repair is a MOVE (Trading passes the chain's outputs in the signed request, ~15k) — designed,
  priced, not built. → DEALER **`(spawned)`**: the move, Custody's rule, the Remove committing.
- Cuts: `6aaac305c`, `cac7dee28`.

### 2026-09-03 00:30 EDT — COHORT-14 DEPLOYED, SEALED, FILLED
- COHORT-14 closed (~660k; `ab0322d50`, `3e5e0b0be`, `0925a5e81`, `4c8ff809e`, `6ba66dc7f`, `e615593fc`; evidence
  docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md): cohort-13 closed (42.08 reclaimed, exact); seven programs
  from 8e96ec3f8 byte-identical three ways, 42.245 SOL against a 42.245 projection; ladder 36 tx, **ten record bodies incl.
  the accelerator's ArtifactRelease** (slot 491,959,038 read off the account); **sealed at 0.000000000 SOL** before founding;
  Direct market FgzbVSWV… founded (0.3366 SOL — cohort-13's to the lamport) with **the Realm carrying release 430369ce… — a
  wallet's ATA can be paid**; activated; admissions; **fill 1,284,573 CU** (−1,614 vs cohort-13 on a larger ELF); fee
  settled; **census L1–L6 + L8 HOLD**, L7 inapplicable by name. The prepay arm had only ever been planned — broke twice,
  fixed. **Two walls:** the General founding collided with the Direct one on a shared `founding-source-funder` identity
  (stranded, 0.81 SOL); and **no capture on this cohort can succeed — Pyth redeployed their devnet receiver at slot
  491,006,444 and every market's release record pins 487,855,452** (0x8014 ReleaseSuperseded; 4.36 days before cohort-13's
  founding — the true cause behind cohort-13's unobserved window); a plan-time preflight landed and is red today.
  Deployer 29.271727270, payer 1.708618131. → COHORT-14B **`(spawned)`**: per-market funder, re-mint the sponsored-push release
  against the live receiver, found a new Direct market and the General market, OpenBatch on chain, the in-window relay, an
  honest resolution, the first ATA payout.
- Cuts: `1397f512f`, `749e72a33`.
- DEALER closed (`9b5de611e` Custody decoded the activation cache three times per invocation — 121k → 74–78k per leg, the
  replay cursor kept entire with reasons; `742d7b7be` **the accelerator's prelude chain rides in the request: 256,650 →
  165,153, the invocation 399,484 → 329,984** — the design was right about the move and wrong about the channel: a witness
  inside hash(request_bytes) yields an address no producer can derive (the campaign refused 0x4001 proving it), so it rides
  after the request bound by `invocation_context` in the signed prefix; `authenticate_accelerator_invocation_v4` 3,904 →
  3,456; `4bfe5394b` the note). **Every one of the partial Remove's three child routes now executes and commits; it dies
  in the commit tail short by ~49,000** (next priced: a decode nothing reads 39k; inter-child frame builds ~108k; the
  permission bank 15–25k). Add headroom 367–374k. **My cleanup at 22:20 destroyed this lane's live scratch** (~700
  uncommitted lines and a capture pair) — replayed from context, 1.5 h lost; rule recorded in memory.
  → DEALER **`(spawned)`**: the last 49,000; frontier's real-ELF hostiles; v2_generator_fresh on hbox.
- Cuts: `a3a0222de`, `59d30010e`.

### 2026-09-03 01:30 EDT — THE PARTIAL EQUITY REMOVE COMMITS
- DEALER closed (`c81c94d91`, note `2e3595040`): **lp-hot, 1,397,966 of 1,399,700, success — all three child routes and the
  whole commit tail, 1,970 spare** — on a draw; three re-runs on the identical ELF set put it back twice and once cleared it
  AND the two LP final Removes behind it (never reached). Findings correcting the note: `acc-product-runtime` was 77% PDA
  SEARCH (draw-free ≠ search-free — the seeds are fixture digests) — the prelude witness now carries the eight bumps Trading
  derived (−17,467); **the Dealer family had never mined a bump hint** (only direct_inline did; −9,000 host-side); the
  shortfall was ~82,000 not 49,000 (the note sized the tail on the Add). The inter-child block decomposed: ~133,000 is the
  runtime's own CPI charge. A second wall reports the same 30/1 — a scenario-checkpoint page at 1,305,050 goes over on an
  unlucky draw before the Remove is reached, decided by the test caller's two unhinted searches. Add headroom 386,000.
  Owed: frame rows (ratchet RED at c81c94d91); v2_generator_fresh; the rejoin hostiles belong in accepted.rs not frontier.rs;
  Trading's own Product-graph walk (~18,000) wants `StateBumpsV1` (Core, Lean layout, migration → cohort-15).
  → DEALER **`(spawned)`**: rows first, the test caller hinted (a deterministic campaign), StateBumpsV1, the hostiles.
- Credential check: the Helius key is absent from the repo, its history and the cut; two local job-dir files that captured
  it (a founding log; cohort-13's sim-config) redacted in place; the staging generator now refuses to emit it (674a7873e).
- Cuts: `1d3deeb80`, `29cd4639c`, `ac1e1b655`.
- DEALER closed (`7f77a1085` rows; `cee27ff16` the page route's two searches hinted AND the membership split balances hashed
  BYTES — page 1 had swung 616,240 ↔ 1,305,100 across runs from an equal-count split over observations four orders of
  magnitude apart; widest page **1,182,094 to the digit in three runs**, headroom 217,756; `b312ce3c4` **StateBumpsV1 carries
  the Product graph's eight bumps as nibbles in four reserved bytes — no migration; Trading start → root-product 104,040 →
  ~87,000**, the fixture half load-bearing; `7ef3c82c0` sixth addendum). **The Remove reaches the commit tail short by
  41–59k; it committed once on a favourable draw.** Route-liveness finding: the selector-9 family is refused by the ADMITTED
  accelerator on every input (the witness keeps span widths empty; the bank has a consumer) — needs an accelerator-side
  derivation. `v2_generator_fresh` green on hbox's tmpfs. → DEALER **`(spawned)`**: the inter-child frame builds and the
  permission bank (the Remove on three consecutive runs), selector 9 through the admitted accelerator, the rejoin hostiles'
  seam.
- hbox's root filesystem was at 0 bytes free: 11.5 GB of my lanes' build outputs removed (sources, evidence, the warm .lake
  kept); 12 GB free now. ember: the rest of the disk is yours (~/dev 27G, ~/h1-ghost 23G, ~/snap 9.7G, ~/tmp 9.4G).
- Cuts: `bcf42759f`, `131b8131e`, `e4672668b`.

### Lane map delta — 2026-09-03 03:00 EDT
- COHORT-14B (in flight; `c21928a68`, `03bab8ddc`): **THE CAPTURE COMMITTED inside the window on the re-founded market — the
  first honest observation the protocol has taken on devnet**; L7 judged by failing first; the Direct capability seal is
  shared across markets; the settle waits out end+max_age (`run-settle-b.sh` running).
- DEALER closed (`9ade7439a`, `07184fa82`, `311e7fc55`, rows `9278f5181`/`241d2b684`, note `b02f5e0fc`): the "four inter-child
  frame builds" were two-thirds a SECOND record walk over ten addresses already derived — one walk now (−21,900), the
  permission byte kept from the decode already done (−13,705): **−33,351 draw-free**; **on one run of three the partial
  Remove COMMITS (10,377 spare) and the first LP final Remove with it** — the draw across runs (~96,000) is now four times
  the worst shortfall (23,887); its largest term is nine searches in the equity evaluator (~27,000) with a carrier already in
  the wire (four zero bytes at 476..480). Selector 9 derives its own span widths accelerator-side — the family the admitted
  accelerator refused on every input is reachable (no test yet submits it). Three of its own reds found by running whole
  crate suites. → DEALER **`(spawned)`**: the evaluator's bump bank (three consecutive commits), the strategy walk's mined
  tail, the reader-less span bank's deletion.
- Cuts: `7e158a1b2`, `26424a852`.

### 2026-09-03 04:00 EDT — C-06 CLOSES: THE DEALER CAMPAIGN IS 31 OF 31
- DEALER (`3c42f0ece` the equity evaluator's eleven searches and the planner's four hinted through the request's four
  reserved bytes; `40427e0f1` **the witness section nothing read deleted, Custody's common frame legible, the campaign
  31/31**; note `40c60d6f7` — "the ELF digest was 45,000 of it"; rows `eb0f16ada`, `6fe2e8ada`). LP Open → hostile Add →
  honest Add → LP Open #2 → Add #2 → the selector-9 trade → its delivery → the partial equity Remove → both LP final Removes:
  every action of the Dealer family executes and commits on real ELFs. The lane's turn was cut by a transient API 403 after
  landing; resumed in place for its report.
- Cuts: `4c6123e95`, `8607dc10b`, `53d24df8e`.
