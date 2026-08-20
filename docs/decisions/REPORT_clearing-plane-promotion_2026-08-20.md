# Decision report: clearing-plane promotion (D1 + D2, with D3 as its own section)

Register entries: `walk-plane-admission-treatment` (D1), `v3-promotion` (D2),
`v3-findings-bc-signoff` (D3) from
[`DECISION_REGISTER_2026-08-20.md`](DECISION_REGISTER_2026-08-20.md). D4
(admission-shape history: 10k quantum + batched folds, blessed 2026-08-19
~23:00) is treated as decided context and not re-litigated.

Owner: **ember** (D1, D3), **ember-after-evidence** (D2). This report decides
nothing; it assembles the evidence, runs the profile's own arithmetic against
the sealed tables, and recommends. Every number below was either read from a
sealed artifact or computed by executing the profile's own
`admission_math.py` / `policy.py` in this worktree (policy.py full check:
PASS; `test_admission_math` / `test_policy` / `test_terminal_profile` /
`test_terminal_admission`: all OK).

Evidence base: liveness profile sealed at `3bcdeec` (cycle D, manifest
`788581c` at 100/100, Persvati-attested 44/44), artifact root
`e8ba31d582be3939…` (1,914,432 B, runtime ref `2dbc9fc`), 15 bank suites /
77 tests, 57/57 profile tests. V3 findings commits read on
`codex/r3-direct-v3` (`6267fde`, `081bd81`).

---

## 1. What promotion means mechanically — and what it does not

### 1.1 The machinery

The liveness profile is a fail-closed derivation, not a status field. For a
promoted route, `derive()` (policy.py:246-489) computes from sealed bank
maxima, per route:

- **admission**: `required_headroom_cu = ceil(measured_cu * 5/4)` (the 25%
  headroom rule), rounded up to the 10,000-CU quantum; if the selected limit
  exceeds the 1,400,000-CU transaction ceiling the route is `STOP_HEADROOM`
  with **no lamport quote** — impossible envelopes are never clamped into
  prices (admission_math.py:101-137). The admission boundary is therefore
  **1,120,000 raw CU** (`maximum_raw_cu_with_requested_headroom`).
- **quote**: `external_fee_cap = 10,000 lamports (base-fee cap) +
  selected_limit_cu x 1 lamport/CU (priority cap)`;
- **reward**: `keeper_reward = external_fee_cap + 100,000 lamports tip`;
- **path/budget quotes** where a lifecycle exists (ResolutionWork's
  `resolution_path_quote`/`batched_resolution_path_quote`; Direct's
  `direct_work_budget_quote` prices Begin + k*Verify + Finalize +
  Settle/Lapse alternatives and reports rent principal separately —
  admission_math.py:450-519);
- **rent rows**: exact per-account rent against the probed rate
  (6,960 lamports/byte + 128-byte overhead), cross-checked against the
  historically executed probe and the byte-pinned post-probe rows;
- **live flags**: e.g. `occupation_v4_monolithic.live_action`,
  `direct_v2.live_v3`;
- **terminal admission**: `build_terminal()` rows validated by
  `terminal_admission.py`; a STOP row retires only by a named decision plus
  sealed evidence (C6).

**The walk plane is machine-held unpromoted.** All four families
(`general_epoch`, `clear_walk`, `candidate_selection`, `entitled_clearing`)
carry `"admission": "UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY"` in
evidence.json, and `derive()` **raises** if any family loses that declaration
(policy.py:373-384). The projection publishes
`general_clearing_walk.status = SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP`,
`admission_rows_derived: false`, `live_flags: "UNTOUCHED"`,
`decision_owner: "ember"` (policy.py:475-486). Promoting is therefore a
two-sided act by construction: the evidence declarations change **and**
`derive()`'s teeth are rewritten to derive rows instead of refusing — plus
the projection, the profile tests, and the manifest that binds them. No
single-file edit can promote silently.

**V3 is resident-unpromoted differently:** it is inside the sealed ELF and
routed (tags 36-46), but the profile carries **no V3 measurement family at
all** — no CU row, no rent/refund/close row, no terminal-admission
`REFUNDABLE_TRANSIENT` classification — and `direct_v2.live_v3` is pinned
`false` (evidence.json:1056, projection). The Direct STOPs in the profile
remain V2's. Promotion means adding sealed V3 families to the current
artifact root (the seal machinery explicitly supports extending the current
root with post-seal bank logs — policy.py:87-103), deriving the work-budget
quote, classifying the terminal rows, and flipping `live_v3`.

### 1.2 What promotion does NOT mean

The claim vocabulary caps it (CURRENT_TRUTH section 1, PROFILE-ADMITTED): an
exact measured route clearing one selected finite compute/rent/reward policy
**does not** extrapolate to unmeasured shapes and **does not** imply
inclusion, keeper participation, system terminality, or a global
`LivenessPolicy` (the projection keeps
`complete_liveness_policy: NOT_EMITTED_STOP` regardless). Specifically it is
not:

- **deployment** — Track C devnet authorization and F1's opt-level choice
  are a separate plane; promotion changes no deployment fact and deployment
  promotes no row;
- **value** — the default registry stays empty, Endow keeps refusing `0x79`,
  `source_value_admission` stays `FAIL_CLOSED_STOP`; only E3 (the registry
  flip, reserved to ember in every authorization statement) touches value;
- **production sources** — R2 identity freeze/cutover (E2) is untouched;
- **fees** — every quote below is a zero-protocol-fee quote; the five
  `max_fee_atoms == 0` gates are cluster B's, not this decision's;
- **a terminality result** — `claims_universal_no_stranded_value` is
  hard-`False` by design (terminal_profile.py:302) and stays so on every
  rung of this ladder.

---

## 2. The promotion ladder, per plane, with the arithmetic run

### 2.0 Policy inputs (sealed, D4-blessed)

Headroom 5/4 (25%), quantum 10,000 CU, ceiling 1,400,000 CU, base-fee cap
10,000 lamports, priority cap 1,000,000 micro-lamports/CU, keeper tip
100,000 lamports. Reward per admitted route:
`10,000 + selected_limit_cu + 100,000` lamports.

### 2.1 General walk plane (tags 47-59) — sealed admission arithmetic

Run against the sealed cycle-D tables (25 route maxima over 111
observations), using the profile's own `quote_route`:

| route (sealed max CU) | required 5/4 | limit (10k) | keeper reward (lamports) | status |
|---|---:|---:|---:|---|
| InitEpoch (42,557) | 53,197 | 60,000 | 170,000 | PASS |
| PlaceOrder single (190,534) | 238,168 | 240,000 | 350,000 | PASS |
| PlaceOrder portfolio (191,350) | 239,188 | 240,000 | 350,000 | PASS |
| FreezeEpoch 1pg/4 (233,568) | 291,960 | 300,000 | 410,000 | PASS |
| FreezeEpoch 2pg/17 (478,009) | 597,512 | 600,000 | 710,000 | PASS |
| FreezeEpoch 3pg/40 (717,829) | 897,287 | 900,000 | **1,010,000** | PASS |
| Walk pass 1, small book (297,878) | 372,348 | 380,000 | 490,000 | PASS |
| Walk pass 2, small book (290,626) | 363,283 | 370,000 | 480,000 | PASS |
| Walk pass 1, 40-order (400,428) | 500,535 | 510,000 | 620,000 | PASS |
| Walk pass 2, 40-order (309,006) | 386,258 | 390,000 | 500,000 | PASS |
| AdvanceSlices (177,754) | 222,193 | 230,000 | 340,000 | PASS |
| CompleteClearWork, walk (127,085) | 158,857 | 160,000 | 270,000 | PASS |
| SubmitCandidate stage (35,605) | 44,507 | 50,000 | 160,000 | PASS |
| WriteFeedFills (9,653) | 12,067 | 20,000 | 130,000 | PASS |
| WriteFeedSlices (9,894) | 12,368 | 20,000 | 130,000 | PASS |
| SealCandidate incl. displacing (64,170) | 80,213 | 90,000 | 200,000 | PASS |
| FinalizeSelection 3-retained winner (49,230) | 61,538 | 70,000 | 180,000 | PASS |
| FinalizeSelection digest-tie (39,462) | 49,328 | 50,000 | 160,000 | PASS |
| FinalizeSelection honest lapse (20,695) | 25,869 | 30,000 | 140,000 | PASS |
| CompleteClearWork, selection (127,931) | 159,914 | 160,000 | 270,000 | PASS |
| FreezeEntitlement (100,052) | 125,065 | 130,000 | 240,000 | PASS |
| EntitleSlice single (204,577) | 255,722 | 260,000 | 370,000 | PASS |
| EntitleSlice portfolio pair (246,173) | 307,717 | 310,000 | 420,000 | PASS |
| SettlePage direct slice (54,834) | 68,543 | 70,000 | 180,000 | PASS |
| SettlePage portfolio full pair (225,739) | 282,174 | 290,000 | 400,000 | PASS |

**Every sealed walk-plane row clears the 25% rule at the 10k quantum — 25/25
PASS**, worst at FreezeEpoch 3-page/40-order, which consumes only 64% of
the 1,120,000 raw-CU admission boundary. Compute is not the walk plane's
problem.

**Rent is.** The rent ledger a full promotion would have to publish, from the
terminal inventory (no close route exists for ANY row; TerminalClosure is a
standing recorded blocker):

| per general epoch | lamports |
|---|---:|
| epoch (legacy.epoch.v2 shape, 328 B) | 3,173,760 |
| epoch.window (231 B) | 2,498,640 |
| clear_work (50,054 B) | 349,266,720 |
| epoch.final_pot (262 B) | 2,714,400 |
| **fixed floor per epoch** | **357,653,520 (~0.358 SOL)** |
| + per page (4,012 B) | 28,814,400 |
| + per order reservation (570 B) | 4,858,080 |
| + per candidate (feed 6,266 B + record 337 B) | 47,738,640 |
| + per settlement receipt (217 B, up to 416/selected) | 2,401,200 |

The sealed campaign's 3-page / 40-order / 3-candidate shape with four
receipts prices at **791,240,640 lamports, ~0.79 SOL per epoch, all of it
currently unreclaimable** — receipts are stamped exhausted in place, consumed
reservations persist as archive, and nothing closes ClearWork, feeds, pages,
window, or pot. Keeper transaction rewards are noise next to this: even
forty driven transactions at the worst quote total ~0.04 SOL. A
full-admission promotion today would be arithmetically green and would
publish a per-epoch cold outlay that is ~95% permanently stranded rent. That
is the honest number, and it is a terrible one.

**Ladder for the walk plane:**

- **W0 — stay evidence-only.** The current state: sealed
  `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`, derive() teeth armed, headline
  capped at SBF-EXECUTED.
- **W1 — partial admission: CU/quote rows without live flags.** derive()
  computes the 25-row table above into the projection under a new
  declaration (e.g. `ADMISSION_ROWS_NO_LIVE_FLAGS`), keeps
  `live_flags: UNTOUCHED`, keeps the family status a STOP, and publishes the
  stranded-rent ledger alongside. Precedent exists inside the profile
  already: V2's select route is quoted PASS inside a family-level STOP, with
  the comment "a passing select quote does not promote the subsystem"
  (policy.py:355-365). **Gated on A1/A2**: the register is explicit that any
  admission row for tags 47-59 is blocked on the policy freeze and the
  window pin — a quote against a PROPOSED `CANDIDATE_WINDOW_SLOTS` would be
  a quote against an unfrozen lifecycle schedule.
- **W2 — full admission with keeper quotes and live flags.** Requires
  everything W1 requires plus rent/close rows that can only exist after
  TerminalClosure (C5) lands under a ratified R4 (C1), plus the wider
  evidence in section 3. Publishes per-route keeper rewards as operational
  promises, lifecycle path quotes, and `REFUNDABLE_TRANSIENT`
  classifications for the rows TerminalClosure closes.

### 2.2 Direct V3 (tags 36-46, two-order venue)

V3 has **no sealed CU rows at all**; what exists is the branch-campaign
table (DIRECT_SELECTION_V3_DESIGN.md, pre-syscall software-SHA runtime) and
two syscall-era headline rows recorded in CURRENT_TRUTH.md:307. Running the
same quote arithmetic on both generations is itself decision-relevant:

| row | measured CU | quote | status |
|---|---:|---|---|
| Freeze V4, **syscall era** | 383,909 | limit 480,000, reward 590,000 | PASS |
| Submit replacement, **syscall era** | 203,128 | limit 260,000, reward 370,000 | PASS |
| Submit replacement, **pre-syscall** | 1,127,892 | required 1,409,865 > ceiling | **STOP_HEADROOM** |
| Freeze V4, pre-syscall | 1,023,401 | limit 1,280,000, reward 1,390,000 | PASS |

The pre-syscall worst row sits **above** the 1,120,000 raw-CU admission
boundary; the syscall-era row sits far below it. The same instruction
straddles the admission rule across runtime generations — which is precisely
why the profile refuses to relabel old measurements and why promotion on
unsealed numbers would be a guess. Only two of ~21 V3 rows have any
syscall-era figure, and none is sealed.

**V3's rent story is the inverse of the walk plane's.** Every transient V3
account records its exact payer principal and closes physically via
`close_funded_account` (Settle closes seven accounts; three Lapse phases,
displacing Submit, and AbortUnfrozen close the rest) — in code, exercised by
svm-tests, **unsealed** (`DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`,
terminal_profile.py:78-130). Per epoch:

- structurally stranded (by design, no close handler): terminal Epoch V4
  672 B + final policy artifact 96 B = **7,127,040 lamports (~0.0071 SOL)**
  (`DIRECT.EPOCH_RECEIPT_RENT_PERSISTS`,
  `DIRECT.POLICY_ARTIFACT_RENT_PERSISTS`);
- closable-in-code, pending sealed measurement: window + work budget + two
  reservations + receipt + pot = **23,406,480 lamports (~0.0234 SOL)**, plus
  4,287,360 per candidate (submitter-paid, closes on displacement or
  settle).

Indicative work-budget quote via the profile's own
`direct_work_budget_quote`, using the **pre-syscall** rows as an upper
bound: spendable reserve 4,640,000 lamports (worst path: selected lapse),
persistent budget = rent + spendable = **28,046,480 lamports (~0.028 SOL)
per epoch**, status PASS. The sealed post-syscall figure would come in well
under this. Compare ~0.79 SOL/epoch stranded for the walk plane: V3's
publishable cold outlay is ~30x smaller and mostly refundable.

**Ladder for V3:**

- **V0 — stay resident-unpromoted.** Current state.
- **V1 — partial admission: sealed CU rows without `live_v3`.** A bank
  measurement campaign against the resident V3 in the **current sealed ELF**
  — new logs extend the e8ba31d5 root (the mechanism the T2-6/7/8 logs
  already used), families added to evidence.json, quotes derived, `live_v3`
  stays false. Evidence-only; no ELF fork.
- **V2 — full admission: `live_v3` true, work-budget quote, terminal rows.**
  Additionally seals close/rollback measurements (retiring
  `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED` "exactly as DIRECT.TOP3_SELECT_CU_STOP
  retired" — the profile documents this exact route), classifies the
  closable rows `REFUNDABLE_TRANSIENT`, publishes the two structural
  stranding rows honestly as permanent capitalization, and derives the
  work-budget quote. Needs the wider evidence in section 3. **Not blocked on
  A1/A2**: `DIRECT_POLICY_V1` is already a frozen const with its two-order
  vacuity argued per selector, and `MAX_RETAINED_CANDIDATES = 3` is sealed;
  the PROPOSED pin (`CANDIDATE_WINDOW_SLOTS`) belongs to the general plane
  only.

Note the asymmetry the ladders expose: **V3 can reach full admission with
measurement work alone; the walk plane cannot reach it without new program
source** (close routes), i.e. an ELF fork and a full reseal cycle.

---

## 3. Evidence each rung requires that does not exist yet

### Walk plane

- **W1**: nothing evidentiary — the rows are sealed. What is missing is
  decisional: A1 (`GENERAL_CLEARING_POLICY_V1` freeze, including the R-b
  rounding-boundary exercising-test check the 08-19B drift review demanded)
  and A2 (window pin). Optionally, the in-flight general-plane **signed
  validator walk** (GOAL next-3 item 1) is the strongest devnet-free
  evidence upgrade and would land under W1's same declaration.
- **W2**, all missing:
  1. **Wider grids**: sealed shapes stop at 3 pages / 40 orders / 3
     candidates. No page-count sweep, no order-count sweep toward the
     structural maxima, no CU-vs-shape curve to justify extrapolating the
     FreezeEpoch and walk rows (the profile never extrapolates; every
     admitted shape must be measured).
  2. **Tie campaigns**: exactly one tie shape is sealed (the
     beyond-128-bit-digest FinalizeSelection row). Full-width exact-tie and
     displacement-order campaigns are host-level only.
  3. **Second bank profile**: the V3 precedent — one bank profile was held
     insufficient for promotion; the same standard applied here demands a
     second independent profile before live flags.
  4. **Rent/close rows**: cannot exist — no account in the plane has a
     close path. Requires C1 (R4 ratification: TerminalIdentityV1 header,
     economic-close-strictly-before-rent-close, incinerator sink) and then
     the TerminalClosure engineering unit (C5), then sealed close/rollback
     bank measurements. **R4 trap to resolve before ratifying C1 as
     written**: R4 section 2(6) declares the four `legacy.*` rows
     PERMANENT_INFRA with **no reap ABI ever** — but the walk plane
     *reuses* exactly those families (epoch.v2, candidate, candidate_feed,
     clear_work) as its live shapes, and the terminal inventory scopes the
     live walk instances under those same rows
     (terminal_profile.py:131-188). Ratified without a carve-out, C1 would
     convert the walk plane's ~0.79 SOL/epoch stranding from "missing
     engineering" into "permanent by decision." The rows must be split
     (walk-plane-instance rows with a close story vs. genuinely legacy
     instances) either before C1 or inside it.
  5. **Path quotes**: a worst-case transaction-count model for
     freeze-to-settle (the walk's analog of `resolution_path_quote`) —
     currently not designed; the sealed campaign records observed
     transaction counts, not a bounded plan.

### V3

- **V1**: the syscall-era CU campaign itself, sealed — all ~21 rows, on the
  current artifact, as bank logs under the e8ba31d5 root.
- **V2**, additionally:
  1. **Close/rollback measurements** for every close route (Settle's seven
     closes, three Lapses, displacing Submit, AbortUnfrozen 0/1/2), sealing
     the refund-to-exact-payer and surplus-to-incinerator behavior svm-tests
     assert.
  2. **Wider grids**: the campaign covers five candidates on an 11-tick
     grid; 64-tick behavior is model+host only.
  3. **Exact-tie and reordered-retained-account** bank cases (model+host
     only today).
  4. **A second bank profile** (the profile's own stated standard for
     "enough").
  5. Not required and not obtainable from D2: retiring
     `DIRECT.EMPTY_FROZEN_NO_LAPSE` — that blocker is V2's (attached to the
     V2-family rows) and outlives V3 promotion either way until a V2 lapse
     lands or C6 disposes of the V2 families.

---

## 4. D3 — the V3 findings B/C sign-off

**What was closed, and by whom.** While findings B and C were ember-pending,
the codex lane closed both unilaterally on `codex/r3-direct-v3`
(lane-collision record at GOAL.md:793-799):

- **`6267fde` — "Bind Direct V3 verification to frozen authority"
  (finding B, the `verify_lease` tautology).** Before: `verify_lease`
  validated the candidate lease's account ledger against a neutral sink
  derived **from the lease's own `donation` field** — the supplied bytes
  authenticated themselves — and took the economic coordinates (quantity,
  buy/sell index, outcome) from the candidate being verified. After: a
  `DirectVerificationFactsV3` struct is re-derived from the frozen page and
  Epoch (limits, quantity, indices, outcome, submission window), the
  expected sink comes from the lifecycle's frozen
  `authority.neutral_lamport_sink`, the submitted slot is checked against
  the frozen window, and acceptance requires the full re-verified candidate
  to equal the persisted one (`domain.verify(input)? != candidate`
  refuses). Adversarial tests added: wrong facts, swapped indices, wrong
  sink, out-of-window slot.
- **`081bd81` — "Pin empty Direct V3 replay authority" (finding C,
  FROZEN_EMPTY admission-field pinning).** Before: the FrozenEmpty phase
  invariant did not pin `seen_competitive_ticks`,
  `competitive_admission_count`, `competitive_admission_transcript`,
  `work_budget_balance`, or `work_rewards_paid`, so an "empty" frozen epoch
  could carry ghost admission history or a partially spent work budget.
  After: all five are pinned (zero / initial balance) in the FrozenEmpty
  arm; a four-arm test
  (`frozen_empty_refuses_ghost_admission_or_work_history`) refuses each
  ghost shape as NonCanonical.

**Whether the closures hold.** Three facts, verified in this worktree:

1. **The commits are not ancestors of `main`.** The V3 merge (`fb72b34`)
   came from the rebased successor branch; `git merge-base --is-ancestor`
   fails for both. Signing off on "the commits" alone would ratify bytes
   the mainline does not carry.
2. **The closure content is present on `main` where it matters**:
   `DirectVerificationFactsV3`
   (research/batch-policy-identity/src/direct_lifecycle_v3.rs:554), the
   closed `verify_lease` (:3720-3760), the FrozenEmpty pins (:2346, :2354),
   and both tests. Filtered runs pass on main's tree:
   `verified_input_binds_grid_tick_candidate_id_score_and_digest` OK,
   `frozen_empty_refuses_ghost_admission_or_work_history` OK.
3. **The SBF route mirrors the substance.** `reverify_retained`
   (programs/clutch-sbf/program/src/instructions/direct_selection_v3/staged.rs:710)
   sources every economic coordinate from the frozen page's orders — not
   from the candidate — and `reverify_decoded_candidate`
   (research/batch-policy-identity/src/direct_window_v1.rs:292-363) is a
   full-field equality pin including digests; the candidate decoder takes
   its sink from the program's frozen `sink_hash()`, not from account
   bytes. One nuance, not a hole: the model's re-verification also
   re-checks the submission window per call, while the program enforces the
   window at submission; worth a negative case in the V1 measurement
   campaign, not a reopen.

**Finding for ember:** the closures are sound, adversarially tested, and
live on main; recommend **ratify the content**. Separately rule on the
process: an ember-pending finding closed unilaterally is a governance
precedent — the clean rule to record is "a lane may land the fix, but the
finding's status stays ember-pending until ember's sign-off, and the
register carries it" (which is exactly what GOAL and this register did).
Refusing the closures on process grounds would re-open two real hardenings
to protest a procedural fault the ledger already caught.

---

## 5. Interactions

- **A1/A2 (policy freezes)** gate every walk-plane rung above W0: admission
  rows for tags 47-59 under a PROPOSED policy const or window pin would
  quote an unfrozen lifecycle. One freeze act can cover both. V3 is not
  gated (DIRECT_POLICY_V1 frozen, retention bound sealed).
- **C1/C5/C6 (terminal)**: W2's rent/close rows require R4 ratification
  plus the TerminalClosure unit; TerminalClosure blocks 8 of the profile's
  STOP rows. The R4 section-2(6) legacy-permanent clause collides with the
  walk plane's reuse of the legacy families (section 3, W2 item 4) —
  resolve the carve-out inside C1. V3's V2 rung retires
  `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED` by sealed measurement (C6's
  documented route) and leaves the two structural V3 stranding rows
  standing as published permanent capitalization.
- **Cluster B (fees)**: every quote either promotion publishes is a
  zero-fee quote; FeeCarryAccount stays a standing blocker retiring only
  via B. Promotion neither waits for nor prejudges the fee-base fork — but
  the cancel-costs-more-than-place inversion (282,998 vs 186,012 sealed) is
  already in the evidence for B1 to read.
- **F1 (deployment)**: deployment is authorized independently (Track C) and
  faucet-blocked; ember's 08-20 directive is maximize local validation. The
  in-flight signed validator walk is the evidence class devnet would
  otherwise provide. Promotion should not wait on devnet (D1 option 3,
  refused below), and devnet must not be read as promotion.
- **G1 (Aug-24 filing freeze)**: the filing drafts carry freeze-sensitive
  V3 successor-selection sentences (GOAL.md:674-676). If any rung lands
  before the filing freeze, the filings' V3/walk sentences must be updated
  at the freeze — a reason to decide the cheap rungs (which change claims,
  not bytes) before Aug 24 or explicitly after it.
- **E2/E3 (R2)**: untouched by every rung; no promotion narrows the `0x79`
  boundary.

---

## 6. Recommendation per plane, with counterarguments

### Walk plane (D1): rung W1 — partial admission, CU/quote rows without live flags — contingent on A1/A2 freezing; otherwise hold at W0. Refuse W2 until R4 + TerminalClosure + second-profile evidence exist.

Rationale: the compute arithmetic is uniformly green (25/25) and sealed —
deriving the rows adds machine-checked teeth to a claim currently made in
prose, at near-zero cost and zero new claim strength (the family stays
STOP). Full admission would be arithmetically green too, and that is the
trap: it would publish keeper quotes against a lifecycle whose every account
strands its rent (~0.79 SOL/epoch), converting a known engineering gap into
an operational promise. The profile's own precedent (V2's quoted select
inside a STOP family) is exactly W1's shape.

Counterarguments, answered:

- *"Promote fully now; fees are zero and the rows pass."* The 25% rule
  admits transactions, not lifecycles. A live flag on a plane with no close
  path publishes a cold outlay that is ~95% unrecoverable and invites
  keepers into work whose terminal step does not exist. The profile was
  built to refuse exactly this shape of claim.
- *"Stay at W0; W1 buys nothing."* W1 converts "someone ran the arithmetic
  once in a report" into "the sealed derivation computes and re-verifies
  the rows on every check, and refuses if a family regresses." It also
  forces the stranded-rent ledger into the published projection, which is
  the number every later decision (C5 ordering, B pricing) should be
  staring at.
- *"Wait for devnet paces."* Devnet adds inclusion-reality, which
  PROFILE-ADMITTED explicitly never claims; it adds no admission row, and
  it is faucet-blocked with no SOL coming. The validator walk (in flight)
  is the strictly better near-term evidence and needs no promotion to land.

### Direct V3 (D2): rung V1 now — commission the sealed syscall-era measurement campaign against the resident ELF — with V2 (full admission, live_v3 true) as the declared target once the section-3 evidence seals. Refuse promotion on current evidence; refuse abandoning V3 in favor of D1.

Rationale: the missing rows are measurements, not decisions; the campaign is
evidence-only against the current sealed artifact (no ELF fork — the same
extend-the-root mechanism T2-6/7/8 used). The pre-/post-syscall straddle
(worst row 1,127,892 CU -> STOP_HEADROOM vs 203,128 -> PASS) is a live
demonstration that unsealed numbers cannot ground admission. V3's close
routes exist in code with exact-payer refunds, so V2 is reachable by
measurement alone — making V3 the cheapest full-admission precedent in the
tree, exactly as ResolutionWork was for the staged-route pattern, and the
only clearing plane that can publish a mostly-refundable cold outlay
(~0.028 SOL upper bound per epoch, ~0.0071 SOL permanent).

Counterarguments, answered:

- *"Promote on current evidence."* Against the profile's own one-profile
  precedent, on rows of which only two have any current-generation figure,
  none sealed. No.
- *"Let the general plane supersede V3 (D2 option 3)."* The general plane
  cannot reach full admission without new program source and R4; V3 can
  reach it with measurement. Killing the cheap promotion to wait on the
  expensive one inverts the project's own gate ordering, and the goal names
  "V3 atomic promotion" explicitly. The two planes are not substitutes: V3
  is the epoch-atomic two-order venue (a decided venue property); the walk
  plane is the general book.
- *"The epoch-atomic no-cancel window is a reason to hold."* It is a
  recorded venue property, already decided and documented at the merge;
  promotion publishes it, it does not relitigate it.

---

## 7. Execution cost per rung

| rung | prerequisite decisions | engineering | seal cost |
|---|---|---|---|
| W0 / V0 | none | none | none |
| **W1** | A1 + A2 (ember act) | rewrite the derive() teeth to row-derivation under a new declaration; extend test_policy; projection update | evidence-only: no ELF change; manifest re-emission + post-commit check (cycle-D precedent: 100/100 first try); attestation optional |
| **V1** | none | one bank-measurement lane (~21 rows, both SVM profiles) writing logs under the e8ba31d5 root; evidence.json families + derive() rows; tests | evidence-only: same as W1; no ELF fork |
| **V2** | ember's "enough profiles" standard (the ember-after-evidence core of D2) | V1 + close/rollback measurement suite + 64-tick / exact-tie / reordered-retained bank cases + second bank profile; terminal-row reclassification; flip `live_v3` | evidence-only, but the largest measurement campaign since the resolution one; manifest cycle + attestation warranted |
| **W2** | A1/A2 + C1 (with the legacy-rows carve-out) + C5 ordering + the same "enough profiles" standard | TerminalClosure close handlers (**new program source, hence new ELF identity**), wider-grid/tie campaigns, second profile, path-quote design | **full reseal cycle**: artifact audit, complete bank re-measurement (every CU row moves), 100-gate manifest emission, portable attestation — the most expensive act in this cluster |

Ordering that follows: **V1 is startable today and is the highest
evidence-per-cost move in cluster D.** W1 waits only on A1/A2. V2 waits on
V1's results plus ember's profile-count standard. W2 waits on the terminal
cluster and should not be scheduled until C1/C5 land.

---

*Compiled 2026-08-20 against sealed root `e8ba31d582be3939…` (liveness seal
`3bcdeec`, manifest `788581c`); all derivations re-run with the profile's own
admission_math/policy machinery in this worktree (policy.py PASS, four
unittest files OK; the findings B/C tests re-run filtered on main's tree).*
