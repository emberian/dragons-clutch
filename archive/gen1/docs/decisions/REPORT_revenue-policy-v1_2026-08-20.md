# Decision report — `revenue-policy-v1` (register B4a–B4f)

Status: **ANALYSIS / RECOMMENDATION.** Standalone report for the six §11
decisions of `docs/design/REVENUE_POLICY_V1.md` (PROPOSED / DESIGN-ONLY),
entries B4a–B4f of `docs/decisions/DECISION_REGISTER_2026-08-20.md`. This
report decides nothing and promotes nothing; it assembles the in-tree
evidence per decision, stress-tests the design's recommendations, and
proposes a decision order. Every citation below was read, not recalled.
The claim vocabulary of `CURRENT_TRUTH.md` §1 governs.

Evidence base: `docs/design/REVENUE_POLICY_V1.md` (whole);
`docs/reviews/FEE_ECONOMICS_FINDINGS_2026-08-19.md` §5–§6;
`docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md`; `docs/ECONOMICS.md` §2/§5/§6;
`docs/DEPLOYMENT_REVENUE_BOUNDARY.md` §3; the live seams
(`programs/clutch-sbf/program/src/instructions/resolution_work.rs`,
`crates/clutch-liveness/src/lib.rs:1128-1245`, the five `max_fee_atoms`
gates, the settlement blocker ledgers); and the liveness profile
(`research/liveness-policy-profile/policy.py`, `evidence.json`,
`terminal_profile.py` — reward quotes re-derived by running `derive()`
against the sealed evidence for this report).

---

## 1. B4a — `revenue-treasury-key`: the treasury key and its custody

**The decision.** Choose the sole authenticated revenue recipient key
(`RevenuePolicyV1.treasury`, REVENUE_POLICY_V1.md:91) and its custody, and
accept that recipient rotation is representable only as a new Realm: the
policy is a frozen const plus digest pinned at Realm creation, immutable
forever (D3, :76-83), existing Realms are zero-take forever with no retrofit
instruction (D4, :124-127), and rotation "is representable only as a program
upgrade, i.e. not representable in this immutable deployment" (:174-175 —
the design already leans on F2's immutable posture).

**Options.**

1. **Single custodial key** (hardware-backed, ember-held). Simplest;
   creation and withdrawal both work through the live owner-signed paths
   with a plain ed25519 signature.
2. **Program-multisig authority** (e.g. a Squads-style vault PDA named as
   `treasury`). Survivable custody; but both duties of the key — Position
   creation via `Intent::Endow` (signer must equal the Position owner,
   `genesis.rs` Endow doc: "the signer must equal the requested Position
   owner") and cash withdrawal — would then be CPI-signed. Whether the
   Endow/withdrawal paths admit a CPI-signed owner is **unverified in this
   report**; it must be demonstrated before a PDA treasury is chosen.
3. **Ratify the shape now, defer the byte value.** Decide permanence
   (D3/D4), the custody requirements, and the `validate()` refusals —
   `treasury` equal to the incinerator or zero bytes refuses (:105-107) —
   and bind the exact pubkey only at the first fee-bearing Realm creation.

**Evidence.**

- **Blast radius is bounded by construction.** The treasury key controls
  revenue only. On Plane C the treasury Position is an ordinary
  `PositionAccount` inside the Hoard liability ledger; Hoard
  `collateral_atoms` never moves on any fee credit (REVENUE_POLICY_V1.md
  §5), and no Position can spend another Position's cash. On Plane L the
  vault sweep is "authenticated by identity against the record, never by
  who cranks" (:170-175) — the key never signs sweeps at all. Compromise
  loses accrued and future revenue of the pinned Realms; it can never touch
  principal or liveness.
- **Loss is permanent per Realm generation.** D4 means a lost treasury key
  strands the revenue stream of every Realm pinned to it, forever; the only
  recovery is new Realms with a new policy const. This is the argument for
  custody that survives people and devices (multisig or equivalent), and
  it is priced in deliberately — any rotation authority is exactly the
  "silently redirect" surface `DEPLOYMENT_REVENUE_BOUNDARY.md:69-71`
  forbids.
- **The key is not on any implementation critical path.** The register
  ranks B4a "first of the six; nothing else in the cluster can land without
  it." That is true of *landing a fee-bearing Realm*, not of deciding or
  building: the const shape, `validate()`, the record family, and every
  §10 falsifier can be authored and tested against a test key. The real
  byte value is needed exactly once — at the first fee-bearing Realm's
  creation — and no fee-bearing Realm can exist until the fee base (B1)
  and a rate exist, both explicitly undecided (REVENUE_POLICY_V1.md §11).
- **Legal perimeter.** A treasury receiving real-money fees is a Track
  question (`DEPLOYMENT_REVENUE_BOUNDARY.md` §2/§5; register F6). The key
  *choice* is engineering; *activation* stays behind Gate L0 either way.

**Interactions.** F2 (`upgrade-posture`): the D4 permanence story is only
as strong as the immutable-deployment posture; if F2 chose a time-bounded
upgrade authority, "rotation is unrepresentable" would silently become
"rotation is a program upgrade," and the filings/Terms language must say
whichever is true. F6/G-cluster: activation, not choice. B4b: the same key
signs per-Market Position creation, so custody form directly sets the
operational cost of D6 (see §2).

**Recommendation.** Option 3 with option 2 as the target: **ratify
permanence (D3/D4) and the custody requirements now — multisig-grade,
survivable, blast-radius-documented — and defer the exact pubkey to the
first fee-bearing Realm creation**, with one named precondition: an
executable demonstration that the Endow and withdrawal paths admit the
chosen custody form's signature (CPI-signed PDA or plain key) before the
byte value is pinned.

**Strongest counterargument.** Deferring the byte value leaves the §10.7
no-silent-redirect falsifier and the record-family tests running against a
test key, and a later "just swap in the real key" step is exactly where an
unreviewed hot key slips in. If that risk is judged unacceptable, choose
option 2 now and eat the CPI verification cost first.

**Execution cost.** Zero program code. One custody act (key ceremony or
multisig setup), one recorded pubkey, plus the CPI-signer demonstration
(one svm-test) if a PDA treasury is chosen.

---

## 2. B4b — `revenue-plane-c-shape`: treasury Position versus pot family

**The decision.** Ratify D6 — fee atoms credit an ordinary
`PositionAccount` owned by the treasury key, one per Market, on the
existing seeds (`seeds.rs` `[SEED_POSITION, market, owner]`) — versus a
standalone `RevenuePotV1` family; if D6, confirm the treasury authority
will run the owner-signed Endow-path creation per Market
(REVENUE_POLICY_V1.md §5, :193-241).

**Options.**

1. **D6 treasury Position** (design-recommended).
2. **Standalone pot family** — program-owned per-market fee pot.
3. Rejected in-design and not re-opened here: Hoard counter field (violates
   `solana-layout/src/lib.rs:1017`), direct external transfer at settlement
   (externally-owned account in the hot path, real token movement per
   fill).

**Evidence for D6, stress-tested.**

The design's three inheritance claims hold up against the tree:

- *Conservation inherited:* a fee is a ledger transfer inside the Hoard
  liability plane; Hoard `collateral_atoms` "is not a fee or liveness
  balance" and never moves. This is the hardest boundary-doc requirement
  (`DEPLOYMENT_REVENUE_BOUNDARY.md:67`) satisfied by construction rather
  than by a new proof.
- *Terminal lifecycle inherited:* Positions already have owner-paid rent,
  close/reopen generations, and an inventory row
  (`terminal_profile.py:50-51`). **Zero new account families and zero new
  terminal rows on Plane C** — against the standing TerminalClosure
  blocker (both ledgers: `orders_batch/settlement.rs` `SETTLEMENT_BLOCKERS`
  and `portfolio_settlement.rs` `PORTFOLIO_RUNTIME_BLOCKERS_V1`, each
  carrying TerminalClosure as deliberately standing), *not adding rows* is
  worth more than any convenience a pot family buys. A pot family would
  re-derive close semantics next to a plane whose rent-reclaim story is
  already the recorded debt.
- *Withdrawal inherited:* the audited cash-withdrawal path; no bespoke
  sweep instruction exists on Plane C at all — one fewer authority surface
  than the Plane L vault itself.

The settlement seam already asks for exactly this: "Fees need a frozen fee
base and a named recipient" (`orders_batch/settlement.rs:571`), and the
admission-side refusal (fee-bearing epoch refuses while the Market's
treasury Position is absent) is refusal-first and stranding-free — a
Market whose treasury never elects to collect simply stays zero-fee.

**Stress points the design does not state, found by this review:**

1. **Owner-close grief.** The treasury Position is owner-closable like any
   Position. If the treasury closes (or closes-and-reopens, bumping the
   generation) mid-epoch on a fee-bearing epoch, every settlement that must
   credit it refuses — the fee recipient can halt *other parties'*
   settlement. The buyer/seller gates already pin `close_state == 0` and
   the reservation-bound `position_generation`
   (`direct_selection.rs:900-907`); the treasury Position needs the
   analogous discipline **plus a close-precondition: a treasury Position
   serving an unsettled fee-bearing epoch refuses close.** That is
   consistent with the terminal design's economic-close-before-rent-close
   rule (TERMINAL_LIFECYCLE_RUNTIME_V1.md §3(16)) and is a small, real
   obligation D6 ratification should name. A program-owned pot family
   would not have this surface — it is the one genuine advantage of
   option 2, and a close-precondition neutralizes it.
2. **Per-Market creation ops.** D6 requires one owner-signed Endow per
   Market the treasury elects to collect on. Under multisig custody (§1)
   that is one proposal per Market. Tolerable at V1 market counts, and the
   failure mode is soft (uncollected Markets stay zero-fee), but it couples
   B4a's custody form to Plane C operations; a Template/Series world (A5)
   would need this revisited.
3. **Account-list and CU width:** one extra account per settlement
   instruction and per release (the terminal-ceil atom credits the treasury
   Position at reservation release, REVENUE_POLICY_V1.md §6). Against
   measured settlement rows (cancel 282,868 CU / place 185,807,
   `GOAL.md:327-329`) one account and one ledger write is minor; no gate
   flips on it.

**Interactions.** B1 (fee-base report): §8.3 already constrains any base to
exact rational quotes and `u128` arithmetic; D6 is base-agnostic — nothing
here preempts the fork. C1/C5: the no-new-terminal-rows property is the
direct interaction with TerminalClosure; D6 keeps Plane C out of C5's debt.
B4e: rebate netting assumes makers' Positions are in the settlement account
list, which is D6's own plane.

**Recommendation.** **Ratify D6**, with two named riders: (a) the
close-precondition of stress point 1 becomes part of the §10.3 no-stranding
falsifier (hostile walk includes a mid-epoch treasury-close attempt that
must refuse); (b) the D6 confirmation explicitly includes the per-Market
Endow duty under whatever custody form B4a selects.

**Strongest counterargument.** The owner-close grief surface: D6 puts an
owner-controlled account inside other people's settlement path, and the
mitigation is a new close-precondition on the oldest, most-shared account
family in the program — a semantics change to Positions generally, not just
treasury ones, unless gated on "serves a fee-bearing epoch." If that
precondition turns out to need per-Position bookkeeping to evaluate, the
zero-new-families claim erodes and option 2 deserves a second look.

**Execution cost of the decision itself:** zero code. Execution cost when
Plane C lands (all already §8/§9 obligations, not new ones): the admission
refusal, the account-list additions, the close-precondition, and the §10
conservation/no-stranding extensions.

---

## 3. B4c — `revenue-plane-l-disposition`: vault versus burn, and whether ResolutionWork charges should exist at all

**The decision.** Two halves: (i) L1 per-Realm `RevenueVaultV1`
(design-recommended D5) versus L0 burn for any nonzero lamport charge;
(ii) **whether the five ResolutionWork charge fields should ever be
nonzero at all** — the register flags this as "an economics call, not a
plumbing one," and it is the half with teeth.

**The live seam, precisely.** All five charges are hardcoded zero with the
stated reason "Every protocol charge is zero because V1 has no
authenticated fee sink" (`resolution_work.rs:357`): `begin_charge`,
`fold_base_charge`, `fold_per_record_charge`, `finalize_charge`,
`abort_charge` (:370-377). `validate_release_cost_shape` pins all five to
zero and all three rewards to their frozen consts (:796-812); the three
transition paths carry `require(charge == 0, ...)` refusals (:997, :1282,
:1489); Begin freezes the schedule digest per Work (`cost_schedule_digest`,
Work stores `costs`), so in-flight Works keep the schedule they were sold.
Charges, where they existed, would debit "only from prepaid budget"
(`AbortTransitionV1.charge` doc, :346-347), and the terminal arithmetic
already folds `donation + charges_paid + charge` to the incinerator
(:1283-1295) — L0 burn is nearly free to implement.

**Keeper economics from the liveness profile's reward schedule** (the
evidence the register asks this section to analyze; derived by running
`policy.derive()` on the sealed `evidence.json`):

- Frozen runtime rewards (`resolution_work.rs:85-94`): Fold **1,160,000**
  lamports per admitted call, Finalize **1,510,000**, Abort **860,000** —
  each priced as selected CU envelope (1,050,000 / 1,400,000 / 750,000 CU)
  at the 1 microlamport/CU priority cap, plus the 10,000-lamport base-fee
  cap and the 100,000-lamport keeper tip.
- Profile-derived route quotes after the syscall reseal (measured CU
  5-13x cheaper; `GOAL.md` reseal record): Fold measured 88,433-95,721 CU,
  selected limit 120,000, external fee cap **130,000** lamports, required
  keeper reward **230,000**; Finalize 164,730 CU -> 320,000 required; Abort
  46,899 CU -> 170,000 required. `runtime_schedule_matches_policy: true` —
  the frozen schedule covers the requirement with roughly **5x margin**
  (a Fold keeper nets at least 1,030,000 lamports over the capped external
  cost).
- Path totals per Work (32 records): singleton plan success rewards
  **7,680,000** lamports, worst-abort 7,530,000, rent principal
  **10,801,920**, payer cold outlay **18,711,920** (~0.0187 SOL); batched
  plan [12,12,8] success rewards 3,830,000, cold outlay **14,861,920**.

What this schedule says about charging resolution:

1. **Keepers are not the payers, so a charge buys no keeper discipline.**
   Charges debit the prepaid budget the market's payer deposited at Begin;
   keeper margins are untouched. A resolution charge is therefore a tax on
   the market's liveness prepayment, full stop.
2. **The protected-pools table already forbids the destination.**
   `docs/ECONOMICS.md` §2: the Market-liveness pool's *forbidden* payments
   are "Claims or treasury." A nonzero ResolutionWork charge swept to a
   RevenueVault is exactly a liveness-pool-to-treasury payment unless the
   deposit is partitioned at Begin into a liveness compartment and a
   separate fee-prepayment compartment — new accounting the design's own §4
   flags as a re-derived terminal reconciliation with its own conservation
   test. `DEPLOYMENT_REVENUE_BOUNDARY.md:66-67` says the same: "Every sink
   is outside Hoard principal and prepaid liveness."
3. **It is anti-liveness in the admission arithmetic.** Liveness principle
   2 (`ECONOMICS.md` §1) books every mandatory job's worst-case cost at
   admission, with expected fees counted as zero (§3). Charges strictly
   raise `B_SOL[j]` for the one mandatory job pipeline, raising every
   market's admission bar — a tax collected precisely on the flow the
   protocol most needs to stay cheap and permissionless.
4. **The revenue is negligible anyway.** Even charges sized like the
   (generous) frozen rewards would collect on the order of 10M lamports,
   about 0.01 SOL per resolved market — per-market-fixed,
   volume-independent, and irrelevant to the §6 break-even inequality,
   which is denominated in trading volume (Plane C). No maintainer is
   funded by Plane L at any plausible magnitude.

**L1 versus L0, given the above.** If charges are permanently zero, there
is nothing to dispose of, and building the vault now is dead machinery: a
new account family, a new terminal row, a re-derived mid-life-departure
reconciliation (REVENUE_POLICY_V1.md §4, named obligation), and a sweep
instruction — all with no payer. If a future genuinely *optional* Plane-L
service flow appears (not the mandatory resolution pipeline), L1 is the
right shape for it — L0 burn "is a deterrent policy, not a RevenuePolicy"
(:144-151) and funds nobody, and the D1 owed/surplus classification says
revenue must never ride the incinerator.

**Interactions.** B4d (below): this decision determines whether Plane L
has any content, which is what the sequencing decision sequences. C1: the
burn-only surplus rule and the owed-compartment falsifier
(TERMINAL_LIFECYCLE_RUNTIME_V1.md §1(10)) are what force the vault shape
*if* a charge ever exists. C5/C6: a vault would add a terminal row and a
mid-life-departure reconciliation to the closure story. Fee-base report
(B1): unaffected — the planes never mix, no lamport/atom conversion exists
anywhere (`ECONOMICS.md:160-161` consistency note in the design).

**Recommendation.** **Decide that the five ResolutionWork charges are a
permanent zero — zero as frozen policy, not as placeholder** — and record
it in the source comment (the current rationale "V1 has no authenticated
fee sink" becomes wrong the moment any sink exists; the true rationale is
the protected-pools row and the anti-liveness argument above).
Simultaneously **ratify L1-over-L0 as the disposition of record for any
future nonzero lamport charge on an optional service flow, and build
neither vault nor record for Plane L now.** The `lamport_sink` member of
the §3 const stays, documented as reserved.

**Strongest counterargument.** Cost recovery is not illegitimate:
resolution consumes shared source infrastructure (archives, feeds) whose
retention (C3/E-cluster) has real ongoing cost, and a small
Begin-denominated charge is the only in-protocol hook that scales with
market count rather than volume. If the project ever needs
market-count-denominated revenue, this is the seam — and deciding
"permanent zero" now, under D3-style permanence instincts, forecloses it
for every Realm created meanwhile. Answer on record: the schedule digest is
frozen per Work, not per Realm — a `RESOLUTION_WORK_COST_VERSION_V2`
sibling const (REVENUE_POLICY_V1.md §4.1) can introduce a charge for *new*
Works without breaking any in-flight promise, so "permanent zero" today is
reversible-by-versioning in a way D4's Realm pins are not. The
counterargument therefore costs little to defer to.

**Execution cost.** Recommendation as stated: one comment/docs change
recording the decision; no bytes move (the zeros are already the byte
truth). The rejected path (vault now) costs: one new family + terminal row
+ reconciliation derivation + conservation tests + sweep instruction.

---

## 4. B4d — `revenue-sequencing`: Plane L before Plane C

**The decision.** Ratify D2: sequence Plane L (lamports) before Plane C
(collateral atoms), because L needs no candidate ABI change, no relation
change, no carry, and no fee base — converting "no authenticated fee sink"
from universal blocker into solved precedent (REVENUE_POLICY_V1.md:66-72).

**Evidence.** The dependency asymmetry D2 states is real and verified:
Plane C requires everything in §5-§8 — the treasury Position discipline,
the reservation V-next carry fields hosting the `IntentFeeCarry` kernel
(`clutch-liveness:1128-1245`: frozen denominator, exact fragment
accumulation, terminal-ceil close charging one atom exactly once, reopen
refused — implemented, tested, zero consumers, findings §5), a fee-bearing
`FrozenPolicyV1` sibling (the `FeeBaseV1` member exists with exactly
`None`/`FlatNotional`, `relation_v1.rs:217-225`, digest-folded at
`code()`), **and a new candidate version** — the compact candidate body
carries no fee field of any kind, so fee-bearing candidates are an ABI
change, not a flag (findings §6). Plane L requires a versioned cost
schedule and a destination account, nothing else.

**But the sequencing is only as meaningful as Plane L's content.** This is
the stress-test result: D2's premise is that Plane L will carry a nonzero
charge. Under this report's B4c recommendation (charges permanently zero),
Plane L is vacuous — there is no charge to land, so "L first" sequences an
empty plane, and the "solved precedent" never gets a live test surface.
The durable kernel of D2 is not "L before C"; it is **"the policy object
before either plane"**: the `RevenuePolicyV1` const, `validate()`, digest,
and the per-Realm `RevenuePolicyRecordV1` family are plane-neutral, needed
by both, and landable without the fee base — and on Plane C the
authenticated destination is an ordinary Position, which needs no new
machinery at all (D6). The precedent argument survives in that form.

**Interactions.** B4c (content of Plane L — the chain); B1 (Plane C's
remaining prerequisites are exactly the fee-base fork plus rate, so C's
start date is owned by cluster B1/B2/B3 regardless of what B4d says); C1
(the record family's header rides B4f).

**Recommendation.** **Ratify D2's dependency analysis, amended to match
B4c's outcome:** the sequencing of record becomes *policy object and
record family first; Plane C second; Plane L never (while charges are
zero)*. If B4c instead keeps a future charge open, D2 stands as written.
Either way the decision is a declaration, not a lane.

**Strongest counterargument.** Landing Plane L with a token nonzero charge
(even 1 lamport on Abort) would exercise the entire owed-compartment
machinery — vault credit, sweep, terminal reconciliation — on the cheapest
possible plane before Plane C bets the settlement ABI on the same
concepts. That is real de-risking D2 was designed to buy, and "policy
object first" does not buy it. It falls to the same answer as §3: a token
charge still crosses the liveness-pool-to-treasury line in
`ECONOMICS.md` §2, and de-risking against that constraint means building
the deposit partition — the expensive part — for a 1-lamport rehearsal.

**Execution cost.** Zero. This is a sequencing declaration; the lanes it
orders are costed in their own sections.

---

## 5. B4e — `revenue-split-vector`: 60/0/40 with `AllRestingMakers`, versus waiting

**The decision.** Freeze the V1 vector — maker rebate 60, executor 0
(deferred, D9), treasury 40, denominator 100, residual to treasury,
terminal-ceil rounding — with the trivially-true `AllRestingMakers`
standing-maker predicate; versus holding Plane C until the real
standing-maker definition (OPEN_QUESTIONS P2: "at least one full frozen
Epoch is the leading candidate") is decided (REVENUE_POLICY_V1.md §7,
:294-321; `docs/ECONOMICS.md:141-168`).

**Options.**

1. **60/0/40 + `AllRestingMakers` now** (design-recommended).
2. **Hold Plane C for the standing-maker definition** (A4 backlog,
   register rank 5).
3. Not proposed anywhere and not invented here: a nonzero executor share —
   no authenticated executor identity exists in the atom plane (direct
   keeper rewards are lamports from a WorkBudget, a Plane-L object), so an
   executor share without an executor account plane would be undeliverable
   by construction.

**Evidence.**

- **Envelope compliance becomes structure.** The published envelope —
  at most 15 executor, at least 25 treasury (`ECONOMICS.md:150-152`, prose
  today; findings §4.8: no Rust implements the split, the only Rust fee
  allocator in the tree allocates by LP capital-time weight with no
  consumers) — becomes `validate()` refusals:
  `executor_num * 100 > 15 * split_den` and
  `treasury_num * 100 < 25 * split_den` refuse to digest
  (REVENUE_POLICY_V1.md:101-105). 60/0/40 satisfies both.
- **The Sybil bound tightens.** A Sybil controlling taker, maker, and
  executor recovers at most 75% under 60/15/25 (`ECONOMICS.md:157-159`);
  with the executor share folded into treasury the bound is **at most
  60%** (REVENUE_POLICY_V1.md:311-313), and the §10.2 falsifier demands it
  be *measured* at that bound on the byte plane. Wash cycling is strictly
  costly only under the terminal-ceil close (`ECONOMICS.md:158-163`,
  corrected per findings §4.1) — which `IntentFeeCarry.close` implements
  exactly (one ceiling atom, once, ever, per generation) and the repo's
  own fixture prices honestly (FEE-001: a 1-atom fee on 1 atom of
  consideration — 10,000 bp on the smallest fill).
- **The trivial predicate is consistent with the B3 descope.** The real
  function of a standing-maker predicate is market quality — rewarding
  genuine liquidity provision over rebate-harvesting. B3 (its own register
  entry) proposes declaring the market-quality axes out of scope for V1
  selection because the simulator that could measure them does not exist.
  A V1 that selects its fee base without market-quality evidence has no
  basis for selecting a market-quality predicate either; pinning
  `AllRestingMakers` and letting a stricter predicate arrive as a sibling
  const *with evidence* is the same epistemic posture applied twice.
- **Structure does not foreclose:** a stricter predicate is a sibling
  policy const, not a mutation (:307-310).

**Interactions.** A4/OPEN_QUESTIONS P2 (the predicate's eventual real
definition); B1 (the split applies to whatever base wins; nothing here
constrains the fork); B4b (rebates are netted at settlement into maker
Positions — D6's plane); D4 permanence (below).

**Recommendation.** **Freeze 60/0/40 with `AllRestingMakers`** as the V1
vector, executor share deliberately deferred until an authenticated
executor account plane exists (D9). Do not hold Plane C hostage to a
rank-5 backlog row.

**Strongest counterargument.** D4 makes the trivial predicate *permanent
per Realm*: every fee-bearing Realm created under this const rebates 60%
to all resting makers forever — sibling consts rescue only future Realms.
If rebate-harvesting under `AllRestingMakers` turns out to be cheap (Sybil
maker legs against own taker flow, evading the same-authority self-cross
refusal with fresh keys), the 60%-recovery bound plus terminal-ceil plus
network cost is the entire defense, permanently, for those Realms. This is
acceptable now only because **zero fee-bearing Realms can exist until B1
and a rate are decided** — the blast radius of a wrong predicate today is
empty. Whoever creates the *first* fee-bearing Realm should re-read this
paragraph on that day.

**Execution cost.** The decision: zero code. Landing (with Plane C): the
§3 const + `validate()` + digest tests; the §10.6 split-exactness
falsifier (relation fee vector vs the Python lab's `allocate_fee` on the
shared corpus, red tests for out-of-envelope consts).

---

## 6. B4f — `revenue-terminal-rows`: classification of the two Realm-lifetime rows

**The decision.** Accept the two new account rows — `RevenuePolicyRecordV1`
(§3) and `RevenueVaultV1` (§4), both one-per-Realm, both carrying the
`TerminalIdentityV1` header (payer, payer_principal, donation_floor,
generation) — as Realm-lifetime rows, or demand a stricter bound before
any implementation lane starts; either way `terminal_profile.py` gains
both rows *first* (REVENUE_POLICY_V1.md:128-139, 154-178, 479-481).

**Evidence — the rent shapes this is defending against.** The inventory's
after-the-fact precedents, with numbers
(`research/liveness-policy-profile/terminal_profile.py`):

- `DIRECT.EPOCH_RECEIPT_RENT_PERSISTS` — `direct.epoch.v4`, 672 bytes,
  **5,568,000 lamports rent, per epoch, unbounded count** (:115-116):
  every terminal route ends in `write_epoch_v4` and no handler closes one,
  so principal is unreclaimable and accrues with every epoch.
- `DIRECT.POLICY_ARTIFACT_RENT_PERSISTS` —
  `artifact.direct_batch_policy_v3.final`, 96 bytes, **1,559,040 lamports
  per epoch**: epoch-context-addressed, so one permanent copy of identical
  bytes accrues per epoch with no close route (:125-127).
- The V3 families entered the inventory *after* they existed and were
  classified retroactively into STOP (`PLANNED_VS_BUILT_2026-08-19.md:92-98`;
  the long apologia at `terminal_profile.py:78-114`) — the exact
  admission-shape history the rows-before-implementation rule exists to
  not repeat.

The proposed rows do not have the dangerous shape:

- **Bound:** exactly 1 per Realm each (FIXED), versus the per-epoch
  unbounded accrual above. The tightest bound representable already.
- **Size/rent estimate** (rent = (128 + bytes) x 6,960 lamports, matching
  every table row): record about 160-200 bytes -> **~2.0-2.3M lamports**;
  vault about 100-140 bytes -> **~1.6-1.9M lamports**. For scale, a Realm's
  existing permanent footprint is realm 70 B / 1,378,080 + profile 100 B /
  1,586,880 — the two rows roughly double a fee-bearing Realm's permanent
  rent, once, at creation, paid by the Realm creator.
- **Honesty of "Realm-lifetime":** the `realm` row is PERMANENT_INFRA with
  no close route, so Realm-lifetime means *permanent in practice*. The
  header is what keeps that honest rather than hopeful: close stays
  admissible (principal to the stored payer, surplus burned) rather than
  unrepresentable, so if Realm classification ever tightens, these rows
  tighten with it without an ABI change. That is the difference between
  these rows and the four `legacy.*` rows whose bytes make close
  unrepresentable forever (TERMINAL_LIFECYCLE_RUNTIME_V1.md §2(6)).

**On "demand a stricter bound":** there is no stricter bound to demand —
cardinality is already 1-per-Realm, and a close-route demand is a demand
on the *Realm* family (PERMANENT_INFRA), not on these rows. The
substantive demand is the one the design already concedes: both rows land
in `terminal_profile.py` with bounds **before any implementation lane
starts** (§3, §10.3), so the inventory leads the bytes for once.

**Interactions.** C1 (`r4-terminal-ratification`): the header these rows
adopt is C1's to ratify; if C1 amends `TerminalIdentityV1`, these rows
track it — B4f should not front-run the header definition. C6: acceptance
adds no blocking id (contrast every V3 family); refusing headers would
manufacture two future rent-persist blockers on purpose. B4c: under this
report's §3 recommendation the vault row is deferred along with the vault
— B4f's acceptance then covers the record row now and the vault row
conditionally, if Plane L ever gains content.

**Recommendation.** **Accept both rows as proposed** — TerminalIdentityV1
headers, 1-per-Realm bounds, Realm-creator-funded — **conditional on C1's
ratification of the header, and with the profile-rows-first rule binding**:
no implementation lane starts until `terminal_profile.py` carries both
rows (the vault row marked contingent on B4c). PASS on a permanent-shaped
row "means only that permanent capitalization is stated honestly"
(`terminal_profile.py` module doc) — that is exactly the claim being
accepted, nothing stronger.

**Strongest counterargument.** Accepting a "Realm-lifetime" class
normalizes permanent rows that *carry* close headers they will never
exercise — a polite fiction that could spread (every future family claims
admissible-close while nothing ever closes). Answer: the alternative —
rows without headers — is the recorded-and-regretted V3 shape, and the
fiction is falsifiable: the §10.3 hostile walk must actually exercise
sweep-and-close on both rows, which a header-less row cannot even attempt.

**Execution cost.** Two rows in `terminal_profile.py` +
`terminal_admission.py` handling for the Realm-lifetime shape (small; the
validator already special-cases classes), landing before any lane. No
program bytes.

---

## Proposed DECISION ORDER

Three of the six are independent and decidable today on in-tree evidence;
two are chained; one waits on cluster C's header.

**Independent (any order, same sitting):**

- **B4c** (charge existence + L1/L0) — but decide it *first* among the
  six, because its outcome rewrites B4d and scopes B4f's vault row. All
  inputs are in-tree (protected-pools table, reward schedule, path
  quotes).
- **B4a** (permanence + custody requirements; byte value deferred to first
  fee-bearing Realm, CPI-signer demonstration as the named precondition).
- **B4b** (D6 treasury Position, with the close-precondition rider).

**Chained:**

- **B4d** immediately after B4c (it is a one-line ratification whose
  content B4c determines: "policy object first, C second, L while-zero
  never" under this report's recommendations).
- **B4e** any time — decidable independently today (recommended: same
  sitting) — but it *lands* only inside the §3 const, i.e. after B4a's
  custody form and B4b's shape are fixed.

**Gated:**

- **B4f** decides with or after **C1** (`r4-terminal-ratification`), since
  its rows adopt the header C1 ratifies; and it must *complete* (profile
  rows landed) before any revenue implementation lane starts. If C1 is
  ratified in the same morning sweep, all six close together.

Downstream, per the findings' dependency order ("decide the destination
before the base"): closing B4a-B4f unblocks **B3 -> B1 -> B2** — the
fee-base fork becomes startable against a real destination, and the five
`max_fee_atoms == 0` gates (`orders_batch.rs:910`,
`orders_batch/settlement.rs:435`, `:571` seam, `direct_selection.rs:908-909`,
`:1759`) plus the `FeeCarryAccount` portfolio blocker
(`portfolio_settlement.rs`) retire only through that cluster, never through
this one. Nothing in this report relaxes any gate.
