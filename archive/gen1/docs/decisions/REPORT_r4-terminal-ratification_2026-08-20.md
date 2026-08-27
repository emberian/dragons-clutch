# Decision report — `r4-terminal-ratification` (register C1 + C2 + C3, C4 folded in, C6 map as appendix)

Status: **ANALYSIS / RECOMMENDS, DECIDES NOTHING.** Fan-out report 3 of the
2026-08-20 decision register (`docs/decisions/DECISION_REGISTER_2026-08-20.md:358-531,
1034-1036`). Every cite below was read, not recalled; the executable inventory
was executed (`build_terminal()` run 2026-08-20: **47 rows, 14 blocking ids,
status STOP**). The claim vocabulary of `CURRENT_TRUTH.md` §1 governs; nothing
here promotes any surface.

The ratification object is `docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md`
(PROPOSED at ad3ece9, written 2026-08-19 against the then-37-row inventory).
The inventory has since grown to 47 rows across T2-3/T2-6/T2-7/T2-8
(`GOAL.md:95-96,150-151,231-237`), and two of this report's findings are
exactly about what that growth did to the design's language.

---

## 0. Findings (the load-bearing ones, first)

- **F1 — Half of this ratification is already shipped bytes.** Decisions
  (8)/(9)/(10) — stored-payer refund, per-account payer, frozen incinerator
  sink — are the merged Direct V3 runtime inside the sealed ELF:
  `close_funded_account` pays exactly the recorded principal to the exact
  stored payer and routes every other live lamport to the sink
  (`programs/clutch-sbf/program/src/instructions/direct_selection_v3/common.rs:286-344`),
  and the sink is a frozen program constant
  (`DIRECT_NEUTRAL_SINK_V3: Pubkey = incinerator::ID`,
  `programs/clutch-sbf/program/src/instructions/direct_selection_v3.rs:62`;
  generalizing `RESOLUTION_WORK_NEUTRAL_SINK_V1`,
  `programs/clutch-sbf/program/src/instructions/resolution_work.rs:66`).
  "Amend the sink" is therefore not a paper edit: it forks the ELF identity,
  invalidates the merged V3 close routes and their in-repo tests, and reopens
  the per-market-sink grief surface the design closed. The realistic options
  are ratify or *defer* — a true amendment is a reseal-cycle engineering
  decision and should be priced as one.

- **F2 — "The four legacy rows" no longer denote what the design meant.**
  §2(6) declares "the four `legacy.*` rows" PERMANENT_INFRA with no reap ABI
  ever, on the argument that they hold "no owed value, only unrecoverable
  rent... the recorded price of the prototype"
  (`TERMINAL_LIFECYCLE_RUNTIME_V1.md:109-114`). That was written in the 37-row
  era. Today the same four rows are the **live general clearing plane's**
  account shapes: the T2-6 plane "reuses the epoch/candidate/feed/clear-work
  families below at their exact widths... so no new row arises from that
  reuse" (`research/liveness-policy-profile/terminal_profile.py:132-138`), and
  the rows track the *current* probe widths, not frozen prototype bytes
  (`legacy.clear_work` moved 48,750 → 48,004 → 50,054 with T2-1/T2-6,
  `legacy.candidate` 305 → 337; `terminal_profile.py:175-188`). Ratifying
  §2(6) verbatim would declare the walk plane's per-clear-job **349,266,720
  lamports (~0.349 SOL)** and per-feed **44,502,240 lamports** permanent by
  fiat — directly contradicting `TerminalClosure`, which stands as the
  recorded rent-reclaim blocker for exactly those accounts
  (`orders_batch/settlement.rs:762-766`;
  `docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md:368-370`).
  **The declaration needs a scope amendment** (§2.3 below): permanent for
  legacy outcome mints and for instances created by the superseded
  direct-batch prototype plane; explicitly *not* for general-plane instances
  of the same byte shapes, whose close story is TerminalClosure + versioned
  families.

- **F3 — Variant A's falsifier has already fired, in-tree, from C4's own
  ratification object.** §8 states A's falsifier: "one admitted market shape
  whose resolution can legitimately need the archive after the horizon — its
  existence forces B" (`TERMINAL_LIFECYCLE_RUNTIME_V1.md:227-229`).
  `EvidenceOnlyRecoveryV1` — the other decision in this very ratification
  packet — is that shape: `RECOVERY_DORMANT` is indefinite ("A later
  caller-funded submission of valid evidence may still resolve the market,"
  `docs/implementation/FAILURE_PAYOUT_DECISION_V1.md:23-28`), and its own
  minimum authority 9 demands "source/archive reference ownership **through
  the last unresolved or retryable market**; dormant recovery cannot outlive
  the evidence objects it needs" (`FAILURE_PAYOUT_DECISION_V1.md:339-341`).
  No frozen maturity horizon covers a market that never lapses. Ratifying C4
  and Variant A together is ratifying a contradiction. Consequence analyzed
  in §2.4; recommendation: **B (conditional) or explicit deferral — never A
  as written.**

- **F4 — The permanent-rent findings cost 7,127,040 lamports per direct
  epoch, forever, and the design already contains the correct instrument for
  them.** Epoch V4 (672 B, 5,568,000 lamports) is written by *every* terminal
  route and closed by none (`terminal_profile.py:97-107`; `write_epoch_v4`
  call sites: `freeze_abort.rs:202,484`, `staged.rs:360,495`,
  `terminal.rs:330,629`; no close call site exists), and the 96-byte
  DirectBatchPolicy V3 final (1,559,040 lamports) is epoch-context-addressed
  (`seeds.rs:259-265` binds epoch id + digest), so identical bytes accrue one
  permanent copy per epoch. Disposition options costed in §3; the §2(1)
  PERMANENT_TOMBSTONE + `PREPAID_UNBOUNDED` validator amendment is the honest
  V1 answer, with a versioned shrink (~79–87% reduction) recorded as
  successor work.

- **F5 — TerminalClosure's reclaim target dwarfs the permanent-rent rows by
  ~56× per epoch.** A minimal one-candidate walked general epoch strands
  ~401,383,200 lamports (~0.401 SOL: window 2,498,640 + pot 2,714,400 + one
  receipt 2,401,200 + feed 44,502,240 + clear-work 349,266,720), all
  *reclaimable* under TerminalClosure — against the direct plane's 7,127,040
  permanent. Worst case adds up to 416 receipts × 2,401,200 ≈ 0.999 SOL per
  selected candidate (`terminal_profile.py:148-172`; MAX_SLICES bound at
  `:158-160`). This is the economic weight behind the C5 ordering
  recommendation (§4.2): TerminalClosure first.

- **F6 — Arm A and C4's lot-scaling compose; ratifying them together narrows
  Arm A's accepted downside.** Arm A governs raw-unit mints
  (redemption-boundary exact-or-refuse); `EvidenceOnlyRecoveryV1` gives *new*
  native markets lot-scaled bearer units (`L = D`,
  `FAILURE_PAYOUT_DECISION_V1.md:36-41,249-266`), under which one token atom
  is a whole economic lot and sub-lot fragments cannot arise at the bearer
  layer. The abandoned-fragment-keeps-market-alive cost Arm A accepts openly
  (§5, design :184-187) is thereby bounded to the raw-unit families; it does
  not grow with the new profile.

---

## 1. What ratification actually unlocks

The design "promotes nothing by itself"
(`TERMINAL_LIFECYCLE_RUNTIME_V1.md:231-233`); what it unlocks is *lane
starts* — every terminal lane is currently parked on it (register C1,
`DECISION_REGISTER_2026-08-20.md:387-389`):

1. **The five interim landable steps** (§9.1–.5, design :243-251):
   the `PREPAID_UNBOUNDED` validator amendment; EXTERNAL_OWNER_STATE rows for
   holder token accounts; the TerminalIdentityV1 header codec (already landed
   as a research crate at eb1215a — 16 tests incl. four falsifiers,
   `GOAL.md:734-740`, `research/terminal-identity-v1/README.md:1-13` — whose
   "PROPOSED pending ratification" banner this decision converts); the
   sub-lot blocker conversion; and the per-family versioned layout lanes.
2. **TerminalClosure (C5) gets its specification.** The standing blocker
   demands close routes that "prove every reservation, receipt, and pot
   consumed exactly once and reclaim their rent" plus the post-freeze/lapse
   release path (`orders_batch/settlement.rs:762-766`;
   `portfolio_settlement.rs:980-988`). Without a ratified header + close
   order + sink, that lane would have to invent a second funding-ledger
   convention beside `DirectFundingLedgerV3` — exactly the drift the uniform
   header exists to prevent.
3. **B4f's rows adopt the header this design defines.** RevenuePolicy V1's
   record/vault rows carry the TerminalIdentityV1 header from day one and the
   profile "gains this row (and §4's vault row) before any implementation
   lane starts" (`docs/design/REVENUE_POLICY_V1.md:130-139`); its stated
   motivation is precisely the permanent-rent blockers of F4.
4. **Retirement (decision-half) of the decision-owned blocking ids** — the
   map is Appendix A. Summary: ratification as recommended retires or
   converts the decision half of **8 of 14** ids
   (`CLAIM.SUBLOT_FRAGMENT_NO_TOTAL_EXIT`,
   `HOARD.RESIDUAL_DISPOSITION_UNSELECTED`, `RENT.ARTIFACT_PREFUND_WINDFALL`,
   `TOKEN.OUTCOME_MINT_PERMANENT`, `DIRECT.EPOCH_RECEIPT_RENT_PERSISTS`,
   `DIRECT.POLICY_ARTIFACT_RENT_PERSISTS`, and — route-authorization only —
   `RENT.ACCOUNT_REFUND_UNOWNED`, `DIRECT.ACCOUNT_REFUND_UNOWNED`), selects
   the retirement route for one (`SOURCE.NO_TERMINAL_RELEASE`, via the §8
   choice), and leaves five untouched because they are evidence- or
   E-cluster-owned (`DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`,
   `DIRECT.EMPTY_FROZEN_NO_LAPSE`, `DIRECT.CANDIDATE_RENT_PERSISTS`,
   `PROFILE.STORAGE_INVENTORY_INCOMPLETE` (partial),
   `SOURCE.DEFAULT_REGISTRY_EMPTY`). No id retires by prose alone: each
   needs its named decision **plus sealed evidence** (register C6), and
   `claims_universal_no_stranded_value` stays hard-`False` permanently by
   design (`terminal_profile.py:302`; design §2(30)).

What ratification does **not** unlock: any promotion (the hostile terminal
walk remains the R4 exit, design :233-241); `SOURCE.DEFAULT_REGISTRY_EMPTY`
(that is E3's registry flip, `DECISION_REGISTER_2026-08-20.md:670-693`); any
fee surface (cluster B); `terminal_status` leaving STOP.

## 2. The ratification elements

### 2.1 The frozen program-wide incinerator sink (decision 10) — **ratify**

The alternatives, each against the design's own falsifier ("a demonstrated
compartment whose burn provably destroys an *owed* balance ⇒ the compartment
was misclassified as surplus; add an owed-ledger, not a sink,"
design :56-59):

- **Per-market `surplus_sink`** (the V2 model's shape): rejected in-design
  (:52-56) — a creation-time authority choice, a wrong-sink grief surface, a
  plausible sweep target. Reversing this now also reverses shipped bytes (F1).
- **Treasury as sink:** inverts the neutrality argument — "nobody gains from
  anyone's donation, so donation-griefing is self-defeating" (:55-56) holds
  *only* for a burn. A treasury sink makes every prefund/donation a revenue
  event and resurrects the windfall class the header exists to kill
  (`RENT.ARTIFACT_PREFUND_WINDFALL`). RevenuePolicy V1 itself refuses
  unowned owed compartments and routes revenue through authenticated charges,
  not surplus capture (`REVENUE_POLICY_V1.md:105-110`).
- **Pro-rata / "fairer" redistribution:** the five-way impossibility applies
  (`docs/implementation/FRACTIONAL_REDEMPTION.md:329-350` — pay zero erases,
  pay one overpays, sink confiscates *owed* value, pro-rata recurses,
  cross-market violates identity); design §3(15) invokes it verbatim (:127-133).
- **Refund surplus to payer:** is precisely the prefund-windfall bug
  (`terminal_profile.py:28-38` — four stage rows STOP on it today).

Three independently decided planes already converge on the incinerator:
ResolutionWork (`resolution_work.rs:66,1041`), Direct V3
(`direct_selection_v3.rs:62`; enforcement `common.rs:33`,
`terminal.rs:49-52`), and the failure-payout residue rule
(`FAILURE_PAYOUT_DECISION_V1.md:23-28,193-196`). The falsifier remains live
and correctly aimed: if a burn ever provably destroys an owed balance, the
fix is an owed-ledger, and the profile's checker semantics
(`terminal_admission.py:59-79` requiring `FROZEN_NEUTRAL_SINK` donation
disposition) would refuse the misclassification.

**Counterargument to carry:** burning is irreversible and "neutral" only
under the classification being right; the protection is that
`close_funded_account` refuses when the live balance cannot cover principal +
prior donations (`common.rs:310-316` — deficit refuses before any byte
moves), and the terminal-identity crate's falsifier suite pins
deficit-refusal and exact close conservation
(`research/terminal-identity-v1/README.md:63-71`).

### 2.2 Fractional Arm A, live-until-aggregated (C2) — **ratify**

The impossibility floor is proved twice over, independently: the
fractional-redemption model (five-way argument, `FRACTIONAL_REDEMPTION.md:
329-350`) and the R4 economics model's irreducible STOP ("for `D>1` and
distinct owners holding positive residues `a,b` with `a+b=D`, no integer
payout vector gives both owners their exact share" —
`research/terminal-economics-r4/MODEL_BOUNDARY.md:67-81`; README result at
:17-27). Given that floor, the three options:

- **Arm A** enforces at the redemption boundary only (live behavior:
  `RemainderRequired` refusal before mutation), exposes the exact lot
  `L(w_i) = D/gcd(D, w_i)` post-resolution, and accepts openly that an
  abandoned sub-lot fragment keeps its market non-retirable forever
  (design :170-193). It requires **no new account plane** and converts
  `CLAIM.SUBLOT_FRAGMENT_NO_TOTAL_EXIT` from "no policy selected" to
  "policy selected: live-until-aggregated" (:184-187) — closing the decision
  while keeping the row honest.
- **Arm B (numerator credits) now** requires the separately capitalized
  remainder reserve and a new account plane; without the reserve "the final
  sub-`D` residue cannot terminate honestly, so no B implementation may claim
  total exit" (:188-193; `MODEL_BOUNDARY.md:77-81`). The economics model
  exists precisely so a future B is buildable as a versioned successor — it
  is not thrown away by ratifying A.
- **Resolution-time per-Position lot refusal** (V2-model): rejected for the
  runtime — transferred bearer fragments make per-Position lots unenforceable
  in principle, and refusing resolution "would strand everyone to punish one
  fragment" (:179-183).

Plus F6: with C4 ratified in the same act, new-profile markets are lot-scaled
(`L = D`) and cannot generate bearer-layer fragments at all — Arm A's
accepted cost stops growing.

**Counterargument to carry:** "some markets never retire" is a real product
consequence and must appear in bearer-facing Terms (STOP 2's residue,
`CURRENT_TRUTH.md:385-387`); ratification does not write that language — it
is the remaining item under STOP 2 after the blocker conversion.

### 2.3 Legacy rows + legacy mints PERMANENT_INFRA (decision 6) — **ratify with a scope amendment** (F2)

Two halves with different truth values today:

- **Legacy outcome mints (82-byte bare mints):** genuinely unrepairable — "a
  retroactive close story is unrepresentable in their bytes and will not be
  invented" (design :201-203); Token-2022 extensions cannot be added in
  place (`terminal-economics-r4/MODEL_BOUNDARY.md:28-30`). Ratify verbatim.
  The no-reap argument is the project's own: any reap authority invented now
  is a sweep right (:112-113).
- **The four `legacy.*` account rows:** the 37-row-era referent (the dead
  direct-batch prototype's epoch/candidate/feed/clear-work) has been
  overtaken — the same rows now *are* the live walk plane's shapes at current
  widths (F2, `terminal_profile.py:132-138,173-188`). Blanket permanence
  would pre-concede TerminalClosure's entire reclaim target
  (~0.4–1.4 SOL/epoch, F5) and contradict the standing blocker ledger the
  runtime itself carries (`settlement.rs:762-772`).

**Recommended ratification language:** "PERMANENT_INFRA, no migration/reap
ABI ever" applies to (i) legacy outcome mints and (ii) instances of the four
families created by the superseded direct-batch plane; general-plane
instances of the same byte shapes are *not* covered — their disposition is
the §9.5 versioned families plus TerminalClosure close handlers, and the
inventory should split the rows by plane at the next profile emission so the
declaration is machine-checkable rather than prose-scoped. (Since nothing is
deployed, "instances" are today a bank-evidence-only population; the split is
cheap now and expensive after a devnet deployment exists.)

**Counterargument to carry:** the row split adds inventory surface, and one
could instead argue the four rows should stay one row each with permanence
never declared (leave them UNCLASSIFIED_STOP until versioned families land).
That is defensible but leaves §2(6) unratified and the prototype's rent
formally undecided; the scoped declaration records the decision the design
actually argued for without capturing the live plane.

### 2.4 Section-8 reference ownership: maturity horizon (A) vs refcount (B) — **the real fork; select B conditionally, or defer explicitly; never A as written** (F3)

The rows at stake, from the 47-row inventory: `feed` (124 B / 1,753,920
lamports, PER_SOURCE_FEED, unbounded), `source.spec` (292 B / 2,923,200,
PER_SOURCE_SPEC, unbounded), `source.archive` (2,560 B / 18,708,480,
PER_SOURCE_WINDOW, unbounded) — all STOP on `SOURCE.NO_TERMINAL_RELEASE` +
`RENT.ACCOUNT_REFUND_UNOWNED` (`terminal_profile.py:54-59`). The archive is
the third-largest per-instance rent in the inventory after `legacy.clear_work`
and `legacy.candidate_feed`.

- **Variant A (maturity-horizon reap)** requires R2 to freeze a maximum
  admitted market maturity (design :218-221,226-227) — a design that does
  not exist yet (`docs/OPEN_QUESTIONS.md:64` P1 archive-retention row open;
  register E6). But the disqualifying problem is F3: under
  `EvidenceOnlyRecoveryV1`, a `RECOVERY_DORMANT` market never lapses and may
  be repaired by later caller-funded evidence at any time
  (`FAILURE_PAYOUT_DECISION_V1.md:23-28`), and minimum authority 9 makes
  archive retention *through the last unresolved market* normative
  (:339-341). Any finite horizon either strands dormant markets unresolvable
  (breaking the ratified promise) or is infinite (not a horizon).
  A is rescuable only by re-specifying late recovery as **archive
  re-creation** (public create/append/seal exists on the mock ELF,
  `GOAL.md:780-788`) — which shifts the burden from on-chain rent to
  off-chain data availability: the provider's retention horizon is
  *undocumented and unmeasured* (`docs/design/SOURCE_PROVIDER_V1_SELECTION.md:
  185-187`), post-cutover historical payloads need a billed API key (:181-183,
  register E5), and reaped-window re-creation would need its own
  generation/replay semantics so a reap is not a deterministic-refusal
  tombstone (design §4 :165-168 currently points the other way).
- **Variant B (per-archive refcount)** is exactly the mechanism that
  implements authority 9: increment at market creation, decrement at
  resolution/lapse; an archive referenced by a dormant market lives
  indefinitely — which is the ratified non-confiscatory outcome, not a bug
  (design §3(18)). The named costs: (i) *griefability* — quantified, the
  grief is economically self-defeating for rent-stranding: pinning one
  archive (18.7M lamports) requires creating a referencing market whose own
  plane costs ~34M lamports (market 5,943,840 + hoard 1,642,560 + kernel
  9,625,680 + supply_ledger 3,208,560 + resolution ≥ 2,039,280 + up to 8
  mints × 1,461,600; `terminal_profile.py:39-49`) — the griefer strands
  ~1.8× more of their own rent than the victim's, per pin; (ii) *a new
  failure compartment* — the shared counter can desync; the mitigation is
  holding it in a §1-headed account (the design's own fallback, :226-227)
  and making the count a checked conservation quantity in the hostile
  terminal walk.

**Recommendation:** ratify the rest of the design now and select **B** as
the working variant (counter in a §1-headed account), with the recorded
escape: if R2's retention design later (a) admits archive re-creation with
explicit generation semantics and (b) measures a provider retention horizon
that covers it, a versioned A-family may supersede. If ember prefers not to
select under an incomplete R2 input, **defer §8 explicitly** — but record the
fired falsifier either way, so A cannot be selected later without answering
F3. Deferral keeps `SOURCE.NO_TERMINAL_RELEASE` unretirable and blocks the
R2 retention design's completion (register C3 blocked-on-it,
`DECISION_REGISTER_2026-08-20.md:437-438`).

### 2.5 Failure-payout ratification (C4) — **ratify both recorded decisions**

The two decisions taken 2026-08-19 by codex lanes while ember-pending
(`GOAL.md:751-760`; `docs/OPEN_QUESTIONS.md:8-26` records both as "Decided"):

- **`EvidenceOnlyRecoveryV1`** — no numeric data-failure payout; recoverable
  dormancy; residue to the incinerator. A veto must answer the equal-sum
  argument: any fixed fallback unequal to a still-possible completion has
  both a gainer and a loser, so a fallback is neutral iff it is ordinary
  evidence resolution (`FAILURE_PAYOUT_DECISION_V1.md:58-81`, executable to
  `D = 32`); the nine-row candidate table rejects every alternative on
  solvency or incentive grounds (:198-215). The selected rule's honest cost
  is stated in its own row: "capital may remain locked indefinitely" (:210).
- **Lot-scaled bearer units** (`L = D` first profile, no persistent remainder
  credits, imported nonzero numerator is a terminal STOP; :36-45,249-281) —
  the encoding that avoids creating the fractional problem for new markets
  (F6), consistent with the R4 frame (design §3(17) deliberately reserved
  this rule's interface and left the rule to the economics lane, :134-138).

Consequences to carry with the ratification, not as veto grounds: (i) the
indefinite-dormancy property is what fires Variant A's falsifier (F3) — the
two ratifications must be made *jointly consistent* by the §8 choice; (ii)
late recovery's practical reachability depends on provider retention and the
E5 ToS/legal lane; (iii) governance: these were ember-pending decisions
closed unilaterally by an agent lane — the same precedent D3 rules on
(`GOAL.md:790-799`); ratifying on the merits should say explicitly that it
does not bless the process.

## 3. The permanent-rent findings under each option (F4)

The findings (recorded at the T2-3 merge: "NO close route — permanent rent by
design," `GOAL.md:231-237`):

| row | bytes | rent (lamports) | scope | mechanism |
| --- | ---: | ---: | --- | --- |
| `direct.epoch.v4` | 672 | 5,568,000 | PER_DIRECT_EPOCH, unbounded | every terminal route (settle, 3 lapses, abort) ends in `write_epoch_v4`; no handler closes it (`terminal_profile.py:97-103`) |
| `artifact.direct_batch_policy_v3.final` | 96 | 1,559,040 | PER_EPOCH_CONTENT_DIGEST | PDA binds epoch id + digest (`seeds.rs:259-265`): identical bytes accrue one permanent copy per epoch (`terminal_profile.py:103-107`) |

**Cost: 7,127,040 lamports ≈ 0.00713 SOL per direct epoch, forever.** Scaling
(cadence is operator-chosen, so illustrations only): one epoch/hour ≈
0.171 SOL/day of permanent rent; one epoch per ~400 s (the general plane's
proposed `CANDIDATE_WINDOW_SLOTS = 1000` duration, register A2) ≈
1.54 SOL/day.

Dispositions:

- **Permanent BY DESIGN (recommended for V1).** The Epoch V4 *is* the durable
  receipt: it holds the replay/generation facts that make post-close
  recreation attempts deterministic refusals (design §4 :165-168) — deleting
  it entirely would reopen the replay surface. The design already contains
  the honest classification for exactly this shape: §2(1)
  PERMANENT_TOMBSTONE with the `PREPAID_UNBOUNDED` validator amendment
  ("exact per-instance prepayment is the bound that matters," :80-88;
  today's checker demands a numeric cap, `terminal_admission.py:37-43,80-98`,
  which is why the amendment is interim step 1). Under this option the two
  blocking ids convert from "rent persists, un-chosen" to "policy selected:
  prepaid durable receipt" — same shape as the sub-lot conversion — and the
  rows classify with `economic_assets_empty` provable. Caveat the tombstone
  rule imposes: §1(12) tombstone principal is "separately prepaid at creation
  and never refunded" (:65-68) — the epoch payer's recorded principal for
  the V4 becomes explicitly non-refundable, which is a *bearer-facing cost
  statement*, not a silent strand.
- **Close path (rejected for V1, recorded as successor).** A full
  `CloseEpochV4` contradicts the replay argument. The coherent successor is a
  **shrink-to-tombstone**: a versioned Epoch V5 terminal route that resizes
  the settled 672-byte receipt to a compact tombstone and refunds the
  difference. Costed with the probe's own rent formula
  (`(128 + bytes) × 6,960`, `TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md:378`):
  a 0-byte tombstone (precedent: `resolution.reserve.v1`,
  `terminal_profile.py:190`) leaves 890,880 permanent (−87.5%); an 84-byte
  replay-shaped one leaves 1,475,520 (−79.3%). For the policy artifact, the
  successor split is one content-addressed permanent const plus the digest
  already carried per-epoch — amortizing 1,559,040 to once per *policy*
  rather than per epoch. Both are ABI/wire changes to a sealed intent:
  reseal-cycle items, per-family versioned lanes under §9.5, never
  retroactive reinterpretation (§1(11) rule, :63-64).
- **Do nothing / leave un-chosen:** the ids stay in the roster, and — the
  decisive argument — RevenuePolicy V1 already treats these two ids as the
  named anti-pattern its own rows are designed to avoid ("unplanned
  permanent rent is exactly what the DIRECT.EPOCH_RECEIPT_RENT_PERSISTS /
  POLICY_ARTIFACT_RENT_PERSISTS blockers look like after the fact,"
  `REVENUE_POLICY_V1.md:135-139`; `GOAL.md:186` "no new permanent-rent
  rows"). The project's stated direction is that permanent rent is
  acceptable only when *declared and prepaid*; leaving the choice open is
  the one indefensible option.

So: **Epoch V4 receipts and per-epoch policy finals stay permanent BY
DESIGN for V1** — declared, prepaid, tombstone-classified via the interim-1
amendment — at 7,127,040 lamports/epoch, with the shrink successor recorded
(reclaiming ~79–87% when a versioned family next forces a reseal anyway).

## 4. Interactions

### 4.1 RevenuePolicy terminal rows (B4f)

C1 is upstream of B4f in both directions: the rows adopt the header C1
defines, and B4f's own precondition is that `terminal_profile.py` gains both
rows *before* any implementation lane starts (`REVENUE_POLICY_V1.md:130-139,
479-481`). Deferring C1 therefore blocks the whole revenue cluster's
implementation start, not just terminal lanes. Conversely nothing in B4
feeds back into C1's content — the header is revenue-agnostic.

### 4.2 TerminalClosure implementation order (C5)

Recommend **C5 option 1: TerminalClosure first** among the standing
engineering units (`DECISION_REGISTER_2026-08-20.md:486-489`), on three
grounds: (i) F5 — it is the largest rent story in the system (~0.4–1.4
SOL/epoch reclaimable vs 0.007 permanent); (ii) it is C1's runtime half for
the walk plane — the close handlers are the first production consumers of
the ratified header outside Direct V3, and its scope already includes the
lapse/post-freeze release path the settlement ledger records ("a lapsed
epoch's ACTIVE reservations stand under the same open row... no
post-freeze/lapse release or expiry path exists yet,"
`settlement.rs:762-766`); (iii) it retires evidence-halves of
`PROFILE.STORAGE_INVENTORY_INCOMPLETE` (walk-plane rows) and feeds the
reservation-expiry racing question (`TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md:
368-371`) that otherwise stays unanswerable. PartialFillLedger and
VirtualPot widen the product; TerminalClosure is the only unit that makes
the plane's *lifecycle* honest, and D1 (walk-plane promotion) lists the
terminal rows among its standing evidence gaps.

### 4.3 The walk plane's unclosed accounts

`epoch.window` / `epoch.final_pot` / `epoch.receipt` are created by tags
49/58/59 and closed by no handler (`terminal_profile.py:132-172`); consumed
reservations persist as archive. Under ratification these are **not** legacy
and **not** permanent (F2 scoping): they are the §9.5 versioned-family +
TerminalClosure population. One sequencing note: their close handlers should
land *with* the header (one lane), not before it — a close route without the
header repeats the V1/V2 direct plane's `ACCOUNT_REFUND_UNOWNED` mistake of
closing without a recorded refund owner.

### 4.4 The V3 close precedent and D2

`close_funded_account` + `DirectFundingLedgerV3` is the ratification's
existence proof: every V3 transient closes with exact principal/surplus
split, and only a *sealed measurement* separates those rows from
REFUNDABLE_TRANSIENT (`terminal_profile.py:80-97` — "retiring that id takes
a sealed measurement, exactly as DIRECT.TOP3_SELECT_CU_STOP retired"). The
close/lapse/abort routes are exercised in
`programs/clutch-sbf/svm-tests/tests/direct_selection_v3.rs` (all three
lapse phases, zero/one/two-abort prefix, transients asserted absent at
:1819-1831, :2103-2250). D2's measurement campaign should include the close
rows so `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED` retires in the same seal.

## 5. Recommendations (summary) and counterarguments

| element | recommendation | strongest counterargument, answered |
| --- | --- | --- |
| Incinerator sink (10) | **Ratify** | "Burn may destroy owed value" — the falsifier stays live; misclassification ⇒ owed-ledger, and `close_funded_account` refuses deficits before bytes move (§2.1) |
| Refund/payer decisions (8)(9)(12) | **Ratify** (ride the same act) | `refund_to` indirection has no consumer and adds an authority surface; the two live precedents agree (design :41-49) |
| Arm A (C2) | **Ratify** | "Markets can be un-retirable forever" — accepted openly, bounded by C4's lot-scaling (F6), and the alternatives are a capitalized reserve (Arm B, versioned successor) or punishing everyone (rejected V2 rule) |
| Legacy-permanent (6) | **Ratify with scope amendment** (F2) | "Amendment delays the act" — the amendment is one paragraph plus a profile row split; ratifying verbatim quietly declares the live walk plane's rent permanent |
| §8 variant (C3) | **B conditionally, or explicit deferral; not A as written** (F3) | "A is simpler and was recommended" — its stated falsifier is fired by C4's indefinite dormancy; A survives only with admitted archive re-creation + a measured provider horizon, neither of which exists |
| Failure payout (C4) | **Ratify both** | A veto must defeat the equal-sum theorem and the five-way impossibility; note the process (ember-pending closed by an agent lane) separately under D3 |
| Permanent-rent rows | **Permanent BY DESIGN for V1** (tombstone + `PREPAID_UNBOUNDED`), shrink successor recorded | "0.007 SOL/epoch forever compounds" — costed: the successor reclaims 79–87% and rides an already-forced reseal; deleting the receipt reopens replay |

## 6. The implementation wave ratification unlocks, sized

**Wave 0 — same act / same day (no program source, no ELF identity change,
no reseal):**
- §9.1 validator amendment (`PREPAID_UNBOUNDED` for PERMANENT_TOMBSTONE) +
  tests — `terminal_admission.py`/`test_terminal_admission.py`, small.
- §9.2 EXTERNAL_OWNER_STATE rows for holder token accounts — profile edit +
  tests, small.
- §9.4 blocker conversions: `CLAIM.SUBLOT…` → "policy selected"; the two
  permanent-rent ids → "prepaid durable receipt" (per §3); legacy-scope
  row split (F2) — profile + docs.
- Ratification records: CURRENT_TRUTH STOP 2/7 language, OPEN_QUESTIONS
  updates, the C6 map committed as decided.
- §9.3 is already done (eb1215a) — its banner flips from "PROPOSED pending
  ratification" to ratified-interim.
  ~3–4 small lanes, each independently landable.

**Wave 1 — evidence only (no program source):**
- Seal the V3 close/rollback measurements (retires
  `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`; rides D2's campaign if commissioned).
- B4f rows enter the profile when/if B4 decides (gated on cluster B, not R4).

**Wave 2 — program source (forks ELF identity; batch into one reseal
cycle):**
- **TerminalClosure** (C5 first, §4.2): close handlers + post-freeze/lapse
  release for reservation/receipt/pot/epoch-window, with the header — the
  largest single unit.
- MintCloseAuthority at mint initialization for new markets (§6) —
  `MintPolicy::outcome` admits exactly the close-authority extension bit.
- Hoard vNext: `donation_atoms`/`forfeiture_atoms` ledgers + `dispose_surplus`
  burn (§3(13)-(15)).
- §9.5 per-family versioned layouts (header + bound + close handler), one
  family per lane, sequenced with R1/R2/R3; the §8-B refcount account rides
  the source-family lane.
  ~5–8 engineering units; every one changes closure bytes, so they ride one
  reseal cycle together (the R1 precedent: closure-byte changes fork the ELF
  identity), plausibly the same cycle E2/E3 forces.

**Exit (unchanged by this report):** the hostile terminal walk (§9) over the
versioned families on a real bank — donations, holder burns, fractional
fragments, all lapse phases, rent refunds, stale replay, deterministic
recreation, ending in the exact declared account set. Only it turns rows
REFUNDABLE_TRANSIENT and starts `terminal_status` moving off STOP.

---

## Appendix A — the 14 blocking ids: retirement map (C6)

Roster executed from `build_terminal()`
(`research/liveness-policy-profile/terminal_profile.py:253-261`); "decision
half" means what this ratification retires, "evidence half" what remains.
No id retires by prose (register C6).

| # | blocking id | rows carrying it | R4 element | decision half (this act) | evidence half (remains) |
| --- | --- | --- | --- | --- | --- |
| 1 | `CLAIM.SUBLOT_FRAGMENT_NO_TOTAL_EXIT` | global (roster) | §5 Arm A (design :184-187) | **converts** to "policy selected: live-until-aggregated" (interim §9.4) | bearer-facing Terms language (STOP 2, `CURRENT_TRUTH.md:385-387`) |
| 2 | `DIRECT.ACCOUNT_REFUND_UNOWNED` | order.page, order.reservation.v1, direct.candidate.v2/window.v1/receipt/final_pot (`terminal_profile.py:60-77`) | §1 header + §2(2)(4) versioned families | route authorized (§9.5 lanes may start) | versioned layouts + close routes + sealed bank evidence; V3's `DirectFundingLedgerV3` is the pattern proof |
| 3 | `DIRECT.CANDIDATE_RENT_PERSISTS` | direct.candidate.v2 | §2(5) BOUNDED_BY_ACCOUNT_FIELD reconciliation | none (rides V3 family landing) | V2-row disposition + sealed evidence (V3 candidates already close on displacement/settle/lapse) |
| 4 | `DIRECT.EMPTY_FROZEN_NO_LAPSE` | order.page, order.reservation.v1, direct.epoch.v3 | — (V2's recorded blocker) | none | a lapse lands for V2 or its rows are dispositioned; V3 `LapseEmpty` exists, measurement unsealed (D2) |
| 5 | `DIRECT.EPOCH_RECEIPT_RENT_PERSISTS` | direct.epoch.v4 | §2(1) tombstone + `PREPAID_UNBOUNDED` (§3 this report) | **converts** to "prepaid durable receipt" if permanent-by-design is selected | validator amendment + tests; shrink successor (Epoch V5) recorded |
| 6 | `DIRECT.POLICY_ARTIFACT_RENT_PERSISTS` | artifact.direct_batch_policy_v3.final | §2(2) + §3 this report (`seeds.rs:259-265`) | **converts** same as #5 | successor content-only addressing recorded; wire change = reseal item |
| 7 | `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED` | direct.candidate.v3/window.v3/work_budget.v1/reservation.v2 | — (evidence-owned) | none (ratification makes the close semantics the ratified rule) | sealed bank measurement of close/rollback, "exactly as DIRECT.TOP3_SELECT_CU_STOP retired" (`terminal_profile.py:91-97`) |
| 8 | `HOARD.RESIDUAL_DISPOSITION_UNSELECTED` | global (roster) | §3(13)-(15) burn-only disposal | **retires the selection** (burn, incinerator) | Hoard vNext ledgers + `dispose_surplus` runtime + bank evidence |
| 9 | `PROFILE.STORAGE_INVENTORY_INCOMPLETE` | position, replay, epoch.window/final_pot/receipt, legacy.* | §2(3) EXTERNAL_OWNER_STATE (interim §9.2) | **partially** — the named holder-account omission closes | walk-plane close paths (TerminalClosure) + full inventory completion |
| 10 | `RENT.ACCOUNT_REFUND_UNOWNED` | position, replay, feed, source.spec, source.archive | §1 header on next versions; replay via §2(1) | route authorized | versioned families + evidence; source rows additionally gated on §8/E2 |
| 11 | `RENT.ARTIFACT_PREFUND_WINDFALL` | 5 artifact `.stage` rows | §1(11) header adoption (design :60-64) | **retires the policy** (prefund burns at close) | stage-vNext lane + sealed evidence; existing stage versions keep their recorded rule until sealed or reaped |
| 12 | `SOURCE.DEFAULT_REGISTRY_EMPTY` | global (roster) | — none | none — this is E3's registry flip, ember's separate go | the full E2→E3 chain |
| 13 | `SOURCE.NO_TERMINAL_RELEASE` | feed, source.spec, source.archive | §8 variant (this report §2.4) | **route selected** if B (or explicitly deferred) | R2 retention design + archive close route + sealed evidence |
| 14 | `TOKEN.OUTCOME_MINT_PERMANENT` | global (roster; token.outcome_mint row) | §6 MintCloseAuthority on new mints; legacy declared permanent | **converts**: new-family half gains a close story; legacy half declared permanent by design | mint-init change (reseal) + close-at-authoritative-zero bank evidence |

Deliberately unretirable, by design and permanently:
`claims_universal_no_stranded_value = False` (`terminal_profile.py:302`) —
the legacy rows and mints make the universal claim false forever, and the
checker keeps refusing anyone who says otherwise (design §2(30)).

---

*Report compiled 2026-08-20 in an isolated worktree from the register's
cluster-C entries and every cited artifact; `build_terminal()` executed; no
tree changes besides this file.*
