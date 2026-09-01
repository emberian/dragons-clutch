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
