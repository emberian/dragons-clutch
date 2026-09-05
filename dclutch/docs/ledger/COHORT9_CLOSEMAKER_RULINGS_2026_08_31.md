# CloseMakerReplay — semantic choices and rulings (CLOSEMAKER, 2026-08-31)

WAVE-ready record of every semantic choice the cohort-9 CLOSEMAKER lane made
building wall 22's missing decrement, as amended by
`COHORT9_PLAN_REVIEW_2026_08_31.md` item 1 (binding). Each entry states the
choice, the authority it rides on, and whether it awaits ember. The
orchestrator holds the veto window on all of them.

## R1. The Retiring-semantics amendment (review §8.3) — RECORDED, veto window

Makers wind down INSIDE Retiring. All four begin-retiring count gates are
relaxed to the Lean ordering (`beginRetiring` phase-only → `closeMaker` gated
on Retiring → `rootClosable` gated on zero): the native ELF gate
(`prepare_retiring_tail`), both operator stages, and a fourth site the
review's evidence list missed — the begin-retiring BUNDLE's transition
bytecode carried its own `scalar_eq(count, 0)`, release content, now dropped
(the begin-retiring descriptor id moves; rides the same cut, which re-digests
everything anyway). Both physical-close count gates stand untouched
(`terminal_retirement_v1.rs`, the native-close bundle's released
`scalar_eq(count, 0)`), and the invariant is proved in Lean
(`retired_requires_zero_open_makers`): the gate moved, the invariant did not.
Safe because `consume_nonce_v2` refuses every non-Open phase — Retiring
already stops trading. Within cohort-9's granted authority ("any bumps and
any/all breaks needed to make things live"); recorded here inside the veto
window.

## R2. The fee-debt gate (review amendment 2) — IMPLEMENTED AS DIRECTED

Lean first: `MakerRoot.feeOwed`, `closeMaker` refuses `feeOwed ≠ 0`,
`recordFeeOwed`/`settleFeeOwed` mirror the chain's exact-amount discipline,
and `consumeNonce` now models the E5 lockout on existing roots (a spec
addition beyond the review's minimum, mirroring the landed
`successor.rs:1197` behavior — the model with the field but without the
lockout would be a new divergence). Theorems, each with a `native_decide`
witness: `close_conserves_fee_receivable` (a close is never the event that
ends a nonzero obligation — the fee-conservation theorem in this model's
vocabulary), `debtor_close_refuses`, `settle_is_exact`,
`outstanding_fee_locks_consumption`; both conserved theorems re-proved.
Chain: `close_maker_replay_v2` refuses `SuccessorError::FeeOwedOutstanding`;
the route names it `TradingSbfError::CloseMakerFeeOutstanding` (0x4011).
Red-proofs both ways, unit and on-chain.

## R3. The donation slice (review §8.1) — PROVISIONAL, NEEDS EMBER

Implemented: the WHOLE observed balance (principal + `unclassified_donation`)
credits the immutably recorded `rent_owner`, which is exactly the landed Lean
plan (`MakerClosePlan.totalCredit` to `rentOwner`, refund conservation
proved) and the landed encoder (`MakerReplayClosePlanV2.total_credit`,
doc-commented "credited to the beneficiary RentCredit" since before this
lane). The permissionless closer's reward is the named constant
`DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1 = 0`, deliberately zero until ruling 1
carves one from the donation slice alone.

The charter's other offered shape — REFUSING a nonzero donation until ruled —
was rejected on CloseSeal's own documented lesson: anyone can transfer 1
lamport into a Trading-owned PDA, so the refusal would let a griefer strand
any replay (and the market behind it) permanently for ~nothing, defeating the
cohort's deliverable. Paying the recorded owner is the landed spec's answer
and under-pays nobody; a later ruled carve is a strict subtraction from the
donation slice. **Ember rules the final payee/carve (funded-crank cap
recommended by the review); the one-constant change then rides any later
ELF.**

## R4. Route shape: lifecycle-native with a fifth ProgramSet entry — RECORDED

The close follows the begin-retiring/native-close family pattern: a codec
bundle (`close_maker_bundle_v1`) whose descriptor/profile/effect are
release-finalized records, a fifth entry in the Direct lifecycle ProgramSet,
and a native Trading route (`direct_close_maker_v1`) dispatched by magic
`DCLTDMC1`. Selector `0xffff_ff04` — `0xffff_ff03` is fee settlement's wire
discriminant (never a set entry) and is skipped so the namespaces can never
alias. `DirectExecutionActionV3::CloseMakerReplay = 11` remains Hot-request
vocabulary only (wall 22's "entry 11"); the Hot dispatch still refuses every
non-InlineOrdinary action, and the real close is the reserved-high lifecycle
entry, which is where every landed lifecycle action lives. This preserves the
family invariant that every root-tail writer is release-authenticated — and
it is what makes the review's blast-radius adjudication hold: four-entry
markets (21/22) stay exactly as unretirable as adjudicated, no better, no
worse.

**The decrement itself is release content**: the close transition bytecode is
`nonzero(count)` + `sub_into(count, 1, post)` and its effect writes the count
word — the released artifacts are now structurally CAPABLE of decrementing,
by exactly one, only against a Retiring header. The ELF independently derives
the same poststate through `close_maker_replay_v2` (which owns the
replay-side refusals the artifacts never see: `live_count`, `fee_owed`) and
commits only on agreement. Two authors, one equality.

## R5. No expected-state digests on the close wire — RECORDED

The close request carries only the coordinate (market, maker, generation).
Sibling closes rewrite the root's count word, so a pinned digest would let
each close grief the next submission — `fee_settlement_v1`'s own argument for
an undeadlined permissionless route. Every economic value is derived from
program-owned state (the replay's recorded `rent_owner`/`rent_principal`);
the commit re-checks that the root bytes it rewrites are the exact bytes it
planned from, which is the pin's whole guarantee without its griefability.
The rent destination must be the recorded owner as a plain System wallet
(CloseSeal's beneficiary shape, minus the signature — the destination is
program-recorded, not caller-chosen).

## R6. New refusal codes 0x400F–0x4012 — RECORDED

`CloseMakerFrame` (0x400F), `CloseMakerReplayAccount` (0x4010, double-close
refuses here by absence), `CloseMakerFeeOutstanding` (0x4011),
`CloseMakerLiveIntents` (0x4012). Band assertions extended through `ALL`; the
generated refusal registries (SDK/web) need a regen pass before the cut.

## R7. ZeroBump rider (review item 2) — IMPLEMENTED AS DIRECTED

`SealedDescriptorClosureV1::decode_defunct` — every conjunct of `decode`
verbatim, the bump byte required zero (`Error::NotDefunct` otherwise); both
public decoders are one private function selecting a bump population, so they
cannot drift. The close request's first reserved byte becomes the bump
candidate (candidate 0 = the ordinary arm, byte-identical to the deployed
wire); the defunct arm reproduces the address from the candidate through the
SAME out-of-line reproduction the ordinary arm uses, and the live-release
refusal — where the close's soundness lives — is untouched. Disjointness
proved three ways in the doc-comment and witnessed by a one-byte-flip test.
The probe (`dclutch-release-tool seal-probe --account PATH --live-release
ID32`) uses the real contract crate, and was exercised against the archived
cohort-8 LIVE seal dump: `decode_defunct=refused:NotDefunct`, `verdict=DOA` —
the control that makes the eventual PASS trustworthy. **The gate probe
against `6hDpsgAo…` itself is a cut-time act with a fresh fetch (review
condition (b)); no dump of it exists on disk and this lane holds no devnet.**

## R8. What the cut inherits from this lane

Founding on the five-entry set (release tool, bootstrap payload with
serde-defaulted close-maker trio, program-test fixtures) is automatic — every
caller builds through `build_direct_inline_ordinary_lifecycle_program_set_v1`.
Cut gates this lane feeds: gate 1 (Lean green, no sorry) DONE; gate 2
red-proofs DONE both ways (unit + on-chain: fee refusal, live-intent refusal,
begin-retiring ADMITS count > 0, double close by absence) except the spline
and ZeroBump-on-devnet rows owned elsewhere; gate 9's first leg (found → …
→ begin retiring over standing makers → close replay(s) → zero count) is the
program-test `close_maker_drains_the_count_wall_22_stopped_at`, which also
executes the RELEASED native-close transition bytecode over the drained tail
— the exact five-place gate wall 22 died on, green from the release bytes.
The physical close itself (CloseCapability on the preserved substrate) and
the ZeroBump devnet close remain cut-time acts.
