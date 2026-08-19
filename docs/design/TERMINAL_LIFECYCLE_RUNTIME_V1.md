# Terminal lifecycle runtime design V1

Status: **PROPOSED / MODEL-ONLY** — no ABI change, no migration, no runtime
transition is made by this document. Written 2026-08-19 by the R4 semantic
owner from the ground-truth map of `research/terminal-lifecycle-v2` (model at
`6dbe618`), the 37-row machine inventory
(`research/liveness-policy-profile/terminal_profile.py` /
`terminal_admission.py`), the ResolutionWork, artifact-stage, and Direct V3
funding precedents, and `research/fractional-redemption`. Roadmap R4 is the
requirements list; this document collapses its thirty forced decision points
into one coherent proposed architecture and names the genuinely open forks.
The claim vocabulary of `CURRENT_TRUTH.md` governs; nothing below promotes
any surface.

## 1. The one uniform primitive: TerminalIdentityV1

Every **new account version** carries one persisted header block, byte-layout
identical everywhere it appears:

```text
payer:            [u8; 32]  exact funding wallet, sole principal recipient
payer_principal:  u64       exact lamports debited after prefund normalization
donation_floor:   u64       monotone DonationLedger lower bound
generation:       u64       close/reopen and replay era
```

Semantics are exactly the `clutch-liveness` kernel plus the ResolutionWork
runtime discipline, now made uniform:

- Creation uses the artifact-stage transfer/allocate/assign pattern:
  `payer_principal = rent_shortfall`, and any pre-existing balance seeds
  `donation_floor` via `admit_prefunded` (payer never credited for a prefund;
  `payer == neutral_sink` refuses).
- Every mutating transition re-runs `observe`: surplus accretes monotonically
  into the donation compartment and can never be reclassified.
- Close pays exactly `payer_principal` to the **stored** `payer`, routes the
  entire remaining surplus through `terminal_split` to the neutral sink, zeroes
  lamports and data. Once, ever, per generation.

Decisions this freezes (numbering from the R4 decision map):

- **(8) Refund recipient = the exact stored payer**, not a separate immutable
  `refund_to`. Rationale: matches the two live precedents (ResolutionWork,
  Direct V3 model) and R3's rule that a refund owner is never inferred after
  creation; a `refund_to` indirection is an extra authority surface with no
  consumer. The V2 model's `refund_to != surplus_sink` check survives as
  `payer != neutral_sink`.
- **(9) Per-account actual payer**, never one market-wide recipient. Different
  wallets legitimately fund different accounts of one market.
- **(10) One frozen program-wide neutral sink: the incinerator**
  (`RESOLUTION_WORK_NEUTRAL_SINK_V1` generalizes). The V2 model's per-market
  `surplus_sink` is rejected: it adds a creation-time authority choice, a
  wrong-sink grief surface, and a plausible sweep target. Burning is the only
  disposition that is neutral by construction for both rent surplus and value
  surplus; nobody gains from anyone's donation, so donation-griefing is
  self-defeating. Falsifier for this choice: a demonstrated compartment whose
  burn provably destroys an *owed* balance (that would mean the compartment
  was misclassified as surplus, and the design must add an owed-ledger, not a
  sink).
- **(11) Artifact stages adopt the same header** in their next version. The
  `RENT.ARTIFACT_PREFUND_WINDFALL` blocker closes: a third-party prefund burns
  at close instead of enriching the funder. Existing stage versions keep their
  recorded rule and their blocker until sealed or reaped; no retroactive
  reinterpretation of already-created accounts.
- **(12) Replay/tombstone principal is separately prepaid at creation and
  never refunded** (V2-model precedent). Keeper budgets, where a family needs
  one, are a separately held WorkBudget-style compartment (Direct V3
  precedent), never commingled with principal or donations.

## 2. Classification dispositions for the 37 rows

Vocabulary and validator (`terminal_admission.py`) become normative — **(30)**
the machine inventory is the classification artifact of record, and
`claims_universal_no_stranded_value` remains hard-`False` permanently: the
four legacy rows and legacy mints make a universal no-stranded-value claim
false forever, and the checker should keep refusing anyone who says otherwise.

Proposed dispositions:

- **(1) `replay` → PERMANENT_TOMBSTONE**, with the separately prepaid
  never-refunded principal of §1 and `economic_assets_empty` provable (the
  tombstone stores a receipt, no value). This requires one deliberate validator
  amendment: `_bounded()` currently demands a numeric cardinality bound for
  permanent classes; tombstones are per-(market, owner, generation) — exact
  per-instance prepayment is the bound that matters. Amend the checker to
  admit `PERMANENT_TOMBSTONE` with `max_instances.kind == "PREPAID_UNBOUNDED"`
  rather than pretending a numeric cap exists. This is a checker semantics
  change and is called out as such; it must land with its own tests.
- **(2) `artifact.*.final`, `realm`, `profile`, `token.hoard_immutable_owner`
  stay PERMANENT_INFRA.** Content-addressed finals and identity roots are
  legitimately immortal. New-version Market/Hoard/Kernel/Supply/Resolution
  rows move to **REFUNDABLE_TRANSIENT** under §1 headers with the §4 close
  order; current-version rows stay where they are until the versioned family
  exists.
- **(3) Add EXTERNAL_OWNER_STATE rows** for Token-2022 holder token accounts
  (bearer claims) and any future owner-created ATA the protocol reads. The
  inventory currently omits them entirely; they are value-bearing state the
  protocol must never close, sweep, or rent-account for.
- **(4) Unbounded rows gain admitted bounds** as part of their next version:
  position/replay per (market, owner[, generation]) with the tombstone rule
  above; source rows per (feed, window) bounded by retention policy from the
  R2 design; order/direct rows bounded by their epoch's frozen grid and
  candidate caps. No row leaves UNCLASSIFIED_STOP by prose; each needs its
  bound in the account's own fields or seeds.
- **(5) `direct.candidate.v2` reconciles with the validator's
  BOUNDED_BY_ACCOUNT_FIELD special case** when Direct V3's account family
  lands (the V3 model already binds candidates to the frozen grid); no rename
  of the live V2 row.
- **(6) The four `legacy.*` rows and legacy outcome mints are declared
  PERMANENT_INFRA, no migration** (V2-model `LegacyStop` precedent). A
  deprecation/reap ABI for them is rejected as pure debt: they hold no owed
  value, only unrecoverable rent, and any reap authority invented now would be
  a sweep right. Their rent is the recorded price of the prototype.

## 3. Economic compartments

- **(13)(14) Hoard surplus gets a ledger, not a field guess.** The next
  HoardAccount version adds `donation_atoms` (token-plane donations observed
  by supply synchronization) and `forfeiture_atoms` (collateral released by
  claim-burn recognition in `claim_truth`). Both are monotone counters
  seeded at migration-creation from zero; they never mint claims, never fund
  liveness, and never pay anyone before terminal.
- **(15) Terminal surplus disposal requires zero liability and burns.** The
  V2-model `dispose_surplus` shape survives with the sink frozen to the
  incinerator: after `L = 0` and every Position closed, `H − L − P` residue
  (donations + forfeiture + dust) burns before Hoard close. No treasury, no
  operator, no pro-rata redistribution — the fractional-redemption doc's
  five-way impossibility argument applies to every "fairer" alternative.
- **(16) Economic close strictly precedes rent close**, per compartment:
  reservations released or consumed → free/reserved cash withdrawn → internal
  balances redeemed/merged → external supply zero (redeemed or burned) →
  fractional carry resolved per §5 → Hoard surplus burned → rent refunds in
  §4 order. A close handler that cannot prove its compartment empty refuses.
- **(17) The failure/low-volume payout rule is NOT frozen here.** This
  document reserves only its interface: any failure vector must be a
  pre-frozen member of the immutable PayoutSet (existing
  RESOLUTION_EVIDENCE_PLAN rule), and equal-payout-is-not-neutral stands.
  The rule itself belongs to the economics frontier lane with its own owner.
- **(18) Abandoned claims are never confiscated — therefore some markets
  never retire.** That is a deliberate, honest outcome: a market with an
  abandoned live claim keeps its Hoard collateral, its mint, and its
  tombstones alive at recorded rent cost. "Retirement guaranteed" is not a
  product promise and must never become one via a sweep.

## 4. Close order (runtime form)

The V2 model's order, expressed with PDAs instead of its exact-ledger array —
**(27)(28)** the on-chain representation is per-account close via §1 plus
dependency checks that read live PDA absence/zeroed state; the model's
`check_exact_rent_ledger` is a model-side invariant that the runtime
approximates by construction (each close handler requires its dependents
already closed) and the hostile terminal walk verifies end-to-end:

```text
zero-claim Positions (owner-paid rent back to each position payer)
  -> zero-supply new-version outcome Mints (MintCloseAuthority, §6)
  -> Supply ledger
  -> Resolution record
  -> Kernel
  -> Hoard (liability zero, surplus burned)
  -> Market
  -> Replay tombstones persist (prepaid, never refunded)
```

Every step is permissionless once its preconditions hold: authority is the
precondition set, not a signer identity. Replay tombstones retain the
consumed-ID and generation facts that make post-close recreation attempts
deterministic refusals.

## 5. Fractional redemption — the fork, decided for V1

**(19)–(22) Arm A, post-resolution enforcement, voluntary aggregation.**

- One token atom remains one raw claim. Redemption of a non-divisible
  quantity keeps refusing `RemainderRequired` before mutation (live behavior).
- After resolution the exact per-outcome lot `L(w_i) = D/gcd(D, w_i)` is
  derived and exposed (research crate already computes it); holders aggregate
  voluntarily. No credit account plane in V1.
- The V2 model's resolution-time per-Position lot refusal is **rejected** for
  the runtime: transferred bearer fragments make per-Position lots
  unenforceable in principle, and refusing resolution over a state the
  runtime legitimately admits would strand everyone to punish one fragment.
  Enforcement stays at the redemption boundary.
- Consequence accepted openly (§3.18): a sub-lot fragment abandoned forever
  keeps its market non-retirable. The `CLAIM.SUBLOT_FRAGMENT_NO_TOTAL_EXIT`
  blocker converts from "no policy selected" to "policy selected: live-until-
  aggregated," which closes the *decision* while keeping the row honest.
- Arm B (authenticated numerator credits) is a versioned successor design,
  admissible only with its separately capitalized remainder reserve and its
  own account plane — the fractional-redemption model's impossibility result
  stands as the falsifier: without that reserve, the final sub-`D` residue
  cannot terminate honestly, so no B implementation may claim total exit.

## 6. Outcome mints

**(23)(24)(25)** New-market outcome mints are created with TLV space and
`MintCloseAuthority` at initialization (mint length grows from the bare 82;
`MintPolicy::outcome` admits exactly the close-authority extension bit and no
other). The close authority is the Market PDA; close is permissionless once
`authoritative_supply == 0` and the supply ledger agrees. Legacy 82-byte
mints are PERMANENT_INFRA — a retroactive close story is unrepresentable in
their bytes and will not be invented.

## 7. External bearer state

**(26)** The runtime keeps its live Materialize/Dematerialize/RedeemExternal
surface; the V2 model's blanket `ExternalBearerStop` is a modeling boundary,
not the product. The terminal consequence is already encoded above: a mint
closes only at authoritative zero, so outstanding bearer supply keeps its
market alive. Holder token accounts are EXTERNAL_OWNER_STATE (§2.3) — never
closed, never swept, never rent-attributed by the protocol.

## 8. Source/artifact reference ownership

**(29)** Two variants, deliberately not decided here (the R2 retention design
owns half the inputs):

- **A. Maturity-horizon reap:** an archive may close only after a frozen
  horizon beyond its window's maturity bucket, sized so every market that
  could reference it must have resolved or lapsed. Simple, no per-reference
  state; couples archive lifetime to the longest admitted market maturity.
- **B. Reference counting:** each market creation increments a per-archive
  refcount; resolution/lapse decrements. Exact, but adds a griefable shared
  counter and a new failure compartment.

Variant A is recommended if R2 freezes a maximum market maturity; otherwise B
with the counter held in a §1-headed account. Falsifier for A: one admitted
market shape whose resolution can legitimately need the archive after the
horizon — its existence forces B.

## 9. Promotion gates and the hostile terminal walk

This design promotes nothing by itself. The R4 exit remains the roadmap's:
a hostile terminal walk covering donations, holder burns, fractional
fragments, all lapse phases, rent refunds, stale replay, and deterministic
recreation attempts, ending in the exact declared account set — run against
the versioned account families this document proposes, on a real bank, with
byte/lamport rollback checks. The walk's account-set assertion is the
runtime replacement for the model's exact-ledger invariant. Until that walk
exists and passes, every current row keeps its present classification and
every blocker stands.

Interim, mechanically landable steps (each its own lane, in order):
1. the validator amendment for PREPAID_UNBOUNDED tombstones (§2.1) with tests;
2. EXTERNAL_OWNER_STATE rows for holder accounts in the inventory (§2.3);
3. the TerminalIdentityV1 header codec + hostile-byte tests as a research
   crate module (no runtime wiring);
4. the fractional decision record: convert the sub-lot blocker per §5;
5. per-family versioned account layouts, one family per lane, each with the
   header, its bound, and its close handler — gated on R1/R2/R3 sequencing.
