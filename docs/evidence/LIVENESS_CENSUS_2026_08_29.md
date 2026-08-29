# Liveness census — every "someone must act" point, 2026-08-29

One question, applied to every lifecycle stage of every family and the market
spine:

> **Who must act for value or state to progress here, what happens if nobody
> does, and is the actor paid?**

This is CYCLE 3 charter item 3 — permissionless completion universalized. The
protocol's one-sentence differentiator is *no liveness dependency on any
identified party*, and this register is the point-by-point proof or the
precisely named gap. Method follows `SEAM_AUDIT_2026_08_29.md`: routes read,
not docs; every claim carries a `file:line`; verdicts are per-row.

## Classification

| class | meaning |
|---|---|
| **GREEN** | permissionless **and** the caller is funded (prepaid bounty / work escrow / rent flows to the caller) |
| **GREEN-SELF** | permissionless and the natural actor is the direct economic beneficiary acting on their own value (a holder redeeming their own payout needs no bounty) |
| **YELLOW** | permissionless but the caller is unfunded — the verb is *permissible rather than live*; progress depends on altruism |
| **ORANGE** | role-gated, but bounded — an expiry, refund, or funded permissionless fallback means absence delays but cannot strand |
| **RED** | an identified party must act and their absence can strand value or state permanently |

Two composition rules used throughout:

1. **A role-gated healthy path + a funded permissionless fallback is the
   protocol's designed shape** (never trust a keeper, fund a bounty). The
   healthy leg is scored ORANGE on its own row; the *system* row for the stage
   is GREEN if the fallback is funded and reachable from every state the
   healthy leg can abandon.
2. **YELLOW is not "almost GREEN".** The tree itself states the doctrine:
   *"Gen-2's consideration was permissionless and UNPAID, which makes a verb
   permissible rather than live: a valid candidate nobody cranked before the
   selection window closed never competed at all"*
   (`crates/dclutch-general-adapter-contract/src/candidate_v1.rs:290-295`).
   A YELLOW row is a named gap, not a rounding error.

## The three proven GREEN exemplars (the standard every row is held to)

1. **The funded failure walk** —
   `crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs`. The bounty is a
   *manifest quote, not a walk-time argument* (`:171-179`): disclosed before the
   market opened, prepaid into the resolution funding ledger's Failure
   compartment (`:385-397`), paid to whoever walks the pre-disclosed failure
   outcome after the deadline (`:3023-3066`), spent exactly once (`:3127-3131`,
   `the_bounty_cannot_be_collected_twice` `:3203`), and
   `ResolutionCertificateV2::validate_shape` refuses `work_paid == 0` — a walk
   that could not be paid for could not have encoded its certificate.
2. **Completion-only founding** — the 13:30 ORCH ruling. The one-shot founding
   permit has NO post-commit refund; the guarantee is a permissionless
   NON-EXPIRING Open. Stage-1 admission IS the Open-satisfiability guarantee:
   everything Open consumes is bound at FoundAndPermit, so completion is always
   available to everyone and value only moves forward.
3. **The candidate work escrow** —
   `crates/dclutch-general-adapter-contract/src/candidate_v1.rs`. Submission is
   permissionless and unbonded; the submission funds *exactly the cranks its
   own life requires* — one per execution row, one consideration, one cleanup
   (`:282-295`); every permissionless transition returns a `WorkRewardV1` whose
   compartment **has already been debited** (`:237-249`); unspent escrow
   returns to the solver; the solver signs "only to own the escrow and its
   refund — not to be authorized. Anyone may submit" (`:297-298`).

The two fix templates for closures in this lane:
- **Dealer cleanup beneficiary** —
  `crates/dclutch-dealer-codec/src/scenario_checkpoint_v1.rs:709-719`:
  permissionless strictly after expiry, value to a creation-fixed beneficiary
  the caller cannot redirect. (Conserving; note it pays the *beneficiary*, not
  the caller — pairing it with a caller-directed crank fee is what upgrades
  YELLOW to GREEN.)
- **The failure-walk bounty** — above.

---

## Ranked findings

~90 act-points censused across six territories. Every RED was re-verified at
the routes by the census author, not just reported by a sweep.

| # | finding | class | fix state |
|---|---|---|---|
| R1 | **An `ExactAuthority` upgrade permanently bricks every live market on the old release set, including its exit.** Cache PDA keyed by release-set content id (`registry lib.rs:513`) so a refresh mints a new account; `state.identity.selected_release_set` is write-once with no re-point route; `retire_v1.rs:488,732` gate retirement on the same slot-pinned cache; `ReleaseSupersededByUpgrade` fires on any retained-authority upgrade (`registry-contract artifact.rs:273`; `plan.rs:1422`). Core `lib.rs:127-134` (decision 0012) narrates a stall; the routes make it permanent. | RED | queue Q1 — ruling |
| R2 | **A market founded with a recovery policy has no terminal at all** once its provider goes silent: the failure walk refuses `recovery_policy.is_some()` (`source_resolution_v2.rs:466-468`) and the ladder's only call site is inside `#[cfg(any())]` (`resolution-proof lib.rs:225-232`; `funded.rs:19` admits it). Both shapes foundable per `core resolution.rs:711-719`. | RED | queue Q2 — weld shut |
| R3 | **One sleeping holder blocks retirement forever**: zero-supply gate (`market_closure_v1.rs:669-681`, 0x5503) + empty-hoard gate (`custody lib.rs:950`), zero Clock reads in the whole terminal path. No escheat, no deadline. | RED | queue Q3 — ruling |
| R4 | **A vanished dealer bricks the market**: `ScheduleReplacement` is the sole liveness-vault refill and is dealer-gated (`dealer-codec lib.rs:1642`); `Fill`/`Unwind` refuse under one `work_reward` (`:1756,:1873`); exhausted vault ⇒ inventory never zero ⇒ `Retire` refuses (`:1914-1918`). No force-exit in the 8-action enum; **entry has no on-chain route either** (dealer-sbf allocates nothing). Sibling veto: one LP equity share blocks obligation close (`v3_obligation.rs:204-206`). | RED | queue Q4 |
| R5 | **The allocated founding permit stranded after its founder-chosen expiry** — the ruled non-expiring Open still expired in code (`generic_founding_v1.rs:1747` pre-edit), and the refund route only accepts an unallocated permit (`series_permit_expiry.rs:365-370`). | RED | **FIXED `a16d1b0b`** |
| R6 | **The tree's one caller-funded cleanup bounty is dead code**: every record `Begin` force-prepays it (`record-contract lib.rs:417`), `prepare_abort_v1` pays the caller at expiry (`lib.rs:1699-1714`), and the dispatcher never admits action 4 (`record_v1.rs:45-57`). Abandoned publications strand raw+cursor+bounty forever. | RED | **FIXED `c365179c`** (this lane) |
| R7 | **`AppendPage` is sponsor-pinned** (`record_v1.rs:113,354`) — an abandoned multi-page publication cannot be completed by anyone else (and pre-R6 could not be reclaimed either). With R6 fixed, bounded by expiry+abort. | ORANGE (was RED) | R6 bounds it |
| R8 | **A missed capability activation deadline has no counterparty**: slot stuck `Pending` (activate refuses past deadline `capability.rs:523-531`, close requires `Active` `:532-536`), ledger needs `all_closed()` (`funding.rs:1877`); the only status writers are `funding.rs:1774,1873`. Principal + shared ledger strand; also blocks Market opening (`funding.rs:1971-1975`). | RED | queue Q5 — lapse verb |
| R9 | **Custody DELIVERY (`activate_batch`) has no deadline and no funding**: no Clock account in the ACT frame, only exit post-`Commit` (rollback/cleanup refuse `Committed`), caller unpaid and pays a never-closed receipt PDA (`custody dealer_reservation_v1.rs:917,1080,1346-1352,1418-1450`). | RED | queue Q4 (with dealer) |
| R10 | **Claims-role Custody replay has no closer**: created permissionlessly (`custody_replay_v1.rs:385-386`), `CloseReplay` demands the role program's caller-authority, and claims-sbf issues no `CloseReplay` anywhere (grep-verified). One replay rent strands per wallet-redemption market. | RED (small) | queue Q6 |
| R11 | **`DCLTPCA1` pre-commit abort is `refund_owner`-only** (`ABORT_SIGNERS`, `projected_custody_bootstrap_v1.rs:261,2909-2914`; re-pinned in `custody projected.rs:1037-1040`) — a lost beneficiary key strands the staged principal exactly as if the route did not exist, against the route's own "way back out" docs. | RED (lost-key) | frozen program; queue Q7 |
| R12 | Submitted-never-consumed provider update strands (reclaim requires `Consumed`, `provider_transport_v3.rs:941`); keyless-PDA prefund mismatch strands (`protocol_position_v2.rs:1043` vs `:414-417`); activation caches have no close route (56 sites, 0 closers). | RED (small ×3) | queue Q8 |
| Y1 | **Systemic: every genuine expiry/cleanup route pays a creation-fixed beneficiary, never the caller** — series permit expiry (`series_permit_expiry.rs:417`), controller-ledger cleanups (`projected_custody_bootstrap_v1.rs:1274-1319`), capability close (`capability.rs:582`), rent sweep (`rent-sbf lib.rs:418-419`; the gauntlet itself: "a pure donation of a transaction fee", `journey/stages.rs:748-758`), dealer checkpoint cleanup — where the beneficiary is even *forbidden* to pay its own fee (`dealer_scenario_checkpoint_v1.rs:1747` refuses `beneficiary.is_signer`). | YELLOW | pattern P1 below |
| Y2 | **Systemic: retirement is 100% permissionless and 100% unfunded** — several routes refuse all signers (`begin_retiring.rs:57`, `retire_v1.rs:1190-1255`, `resolution.rs:576`), every recovered lamport flows to the creation-fixed `refund_wallet` (`retire_v1.rs:1725,1739-1740`), and two act-points are caller-*negative* (replay handoff `custody …handoff:486-492`; Claims replay creation D4). | YELLOW | pattern P1 |
| Y3 | **Perverse incentive at the resolution deadline**: success-settle and failure-walk open at the same instant; the failure walker is paid the bounty while the success settler is net-negative (`resolution-codec v2.rs:385-393` exempts success from `work_paid != 0`; both producers hardcode 0 — `sponsored_push_v1.rs:1274,1280`, `provider_v3.rs:306,312`). Plus a free griefing verb: anyone may retire a `Collecting`/`Sealed` relay record (`relay-contract record.rs:751-753`). | YELLOW (adversarial) | queue Q9 |

**Clean on their own terms**: the funded failure walk (C6, caller paid atomically,
`relay_transport_v1.rs:1504`); Dealer `Fill`/`Unwind` (`work_reward` to the
signing ACTOR, `dealer-codec lib.rs:1828,1904`, `dealer-sbf lib.rs:2031` — the
tree's second real caller-funded verb); the General candidate work escrow;
holder redemption and user admission (GREEN-SELF, owner-signed,
`signed_delta_v3.rs:516-520`, `user_position_admission_v1.rs:138,159`); and the
retirement *mechanics* (no identified party anywhere — the gap is funding, not
permission).

## Census register

One row per act-point, grouped by lifecycle territory:

- **A. Founding, permits, record publication, Series** — below
- **B. Activation, admission, registry caches after upgrades** — below
- **C. Provider capture, resolution, terminal admission** — below
- **D. Payout, redemption, retirement, rent recovery** — below
- **E. Dealer, Custody delivery, checkpoint cleanup** — below
- **F. Trade spine** — below

### A. Founding, permits, records, Series

| act-point | who | if nobody | caller paid? | class |
|---|---|---|---|---|
| prefund Market/permit/Claims PDAs (exact-lamport System transfers; `generic_founding_v1.rs:838-841`, `founding_v5.rs:854-869`) | anyone | founding can't start; prefunds recoverable only by a later founding | no | YELLOW |
| RentCredit create (`rent-contract lifecycle_v2.rs:796-812`) | anyone (payer signs, picks `refund_wallet` immutably) | founding refuses | no | YELLOW |
| record Begin/Append/Finalize/Abort | see B3-B6 | — | — | see B |
| `DCLTPCB2` bootstrap (stage the prestate) | funder + payer sign (`projected_custody_bootstrap_v1.rs:196`) | nothing staged | n/a — it IS the deposit | ORANGE |
| `DCLTCFQ1` funding prepare | `funding_source` signs (`:298-306`) | nothing staged; bounded by expiry aborts | n/a | ORANGE |
| `DCLTGFP1` stage-1 FoundAndPermit | Trading caller-authority PDA (`generic_founding_v1.rs:274-276,812-815`) — economically the founder | staged principal waits; pre-commit refund family bounded by expiry | no | ORANGE |
| `DCLTGMO1` / Series stage-2 **Open** | **anyone** — sole signer is the request-derived Trading PDA; the stage-2 "market-owner precheck" pins program ownership, not identity (`generic_founding_stages_v1.rs:409-415`) | **was RED** (permit expired + no refund for allocated permits) — **now completion-only forever** (`a16d1b0b`) | no — permit rent to fixed RentCredit (`:640,1918-1931`) | YELLOW (was RED) |
| `DCLTPCA1` pre-commit abort | **`refund_owner` only**, post-expiry (R11) | staged principal strands on a lost key | no | **RED** |
| `DCLTCF1A`/`DCLTCF2A` ledger cleanup | anyone — any signer refused (`projected_custody_bootstrap_v1.rs:607-632`) | bounded by expiry | no — principal→funding_source, rent→RentCredit (`:1274-1319`) | YELLOW |
| Series Consume (`series_consume.rs:492-495,894`) | Trading caller PDA, window `[scheduled_slot, retry_through]` | ticket expires; prefund refundable via permit expiry | no | ORANGE |
| Series permit expiry (`series_permit_expiry.rs:107-161`, zero signers) | anyone | this IS the bound (pre-allocation only) | no — all lamports → RentCredit (`:417-430`) | YELLOW |
| `expiry_slot` selection | founder, unbounded — no upper-bound check anywhere (`Found` only checks `clock.slot > request.expiry_slot()`, `generic_founding_v1.rs:397`; Open checks `current_slot > intent.expiry_slot()` pre-`a16d1b0b`) | the founder alone picks the deadline governing both the (now non-expiring) Open and the earliest cleanup | n/a | note |

### B. Activation, admission, registry caches (sweep + direct verification of every RED)

| # | act-point | who can act | if nobody acts | caller paid? | class |
|---|---|---|---|---|---|
| B1 | Registry `ActivateRole` ×5 (`programs/dclutch-registry-sbf/src/lib.rs:194`) | anyone — only pin is the fee payer signing (`lib.rs:588`) | cache never decodes; markets on that release set unopenable | no — payer is *debited* 1288 bytes of cache rent (`lib.rs:557-558`) | YELLOW |
| B3 | Record `Begin` (`record_v1.rs:180`) | anyone with a sponsor wallet | nothing staged | no — sponsor debited rent **and a forced nonzero cleanup bounty** (`crates/dclutch-record-contract/src/lib.rs:417`) | YELLOW |
| B4 | Record `AppendPage` (`record_v1.rs:345`) | **committed sponsor only** — signer required (`record_v1.rs:113`) and pinned to the cursor (`:354`) | a multi-page publication abandoned after page 1 can never be completed by anyone else — and (B6) never reclaimed | — | **RED** |
| B5 | Record `Finalize` (`record_v1.rs:395`) | anyone, no signer at all (`record_v1.rs:146-160`) | record unfinalized ⇒ activation refuses | no — entire cursor balance incl. the bounty goes to the committed sponsor (`record_v1.rs:404,432,611-618`) | YELLOW |
| B6 | Record `Abort` — the funded expiry cleanup | **nobody — route not dispatched.** `RecordActionV1::Abort = 4` (`record-contract lib.rs:772`) falls to `_ => Err` in the dispatcher (`record_v1.rs:45-57`) | the forced prepaid bounty is unreachable forever; abandoned cursors strand | the contract pays `cleanup_recipient: abort_actor` — **the caller** — at expiry (`lib.rs:1699-1714`), the only caller-funded payout construct in the tree, and it is dead code | **RED** → **fixed in this lane, see closures** |
| B7/B8 | Registry continuation / hot continuation (`continuation_v1.rs:24`, `hot_continuation_v2.rs:47`) | anyone (0 signers) | stall until someone cranks | no | YELLOW |
| B9 | Post-upgrade cache refresh (re-release + re-activate) | anyone — activation observes chain state, needs no authority signature | **every existing market bricks, including exit** (see B10) | no | **RED** |
| B10 | Re-point a live market at a new release set | **no route exists.** Cache PDA keyed by release-set content id (`registry lib.rs:513`); `state.identity.selected_release_set` write-once, 30+ compare-only sites; `retire_v1.rs:488,732` gate retirement on `authenticate_roles` over the slot-pinned cache; `ReleaseSupersededByUpgrade` fires on any `ExactAuthority` upgrade (`registry-contract artifact.rs:273`); deployments retain `ExactAuthority` whenever an upgrade authority exists (`tools/local-validator/bootstrap/successor/src/plan.rs:1422`) | permanent — Core `lib.rs:127-134` (decision 0012) narrates a stall; the routes make it forever | — | **RED** |
| B11 | Core `ActivateCapability` (`capability.rs:70,186`) | anyone — every named account explicitly non-signer (`capability.rs:379`), authority compared by address (`:842`) | slot stays `Pending` until the deadline, then B13 | no | YELLOW |
| B12 | Core `CloseCapability` (`capability.rs:190`) | anyone | prepaid principal held | to `state.rent_beneficiary`, not caller (`capability.rs:582`) | YELLOW |
| B13 | Expired-`Pending` capability slot | **nobody.** Activate refuses past deadline (`capability.rs:523-531`; contract `funding.rs:1760`), Close requires `Active` (`capability.rs:532-536`; `funding.rs:1853`); the only status writers are Pending→Active (`funding.rs:1774`) and Active→Closed (`:1873`); `ActivationDeadlineElapsed` returned 4 places, handled 0 | prepaid principal strands AND the shared ledger never reaches `all_closed()` (`funding.rs:1877,2021`) — sibling residue and ledger rent strand with it; an expired `Pending` entry also blocks Market opening (`funding.rs:1971-1975`) | — | **RED** |
| B14 | `series_permit_expiry` (`core series_permit_expiry.rs:167`) | anyone (0 signers) | this *is* the bound for series permits | no — permit lamports to `rent_credit` (`series_permit_expiry.rs:417`) | YELLOW |
| B15 | Controller funding `Prepare` (`trading projected_custody_bootstrap_v1.rs:275`) | `funding_source` wallet must sign (`:298`) | nothing staged; bounded by B16 | — | ORANGE |
| B16 | Controller funding expiry abort (`projected_custody_bootstrap_v1.rs:594`) | anyone — any signer refused (`:624`) | bounded by `expiry_slot` | no — principal→funding_source, rent→rent_credit | YELLOW |
| B17 | Resolution `pre_market_funding_abort_v1` | Trading PDA (request-derived, no wallet identity) | bounded by expiry | no — to funding_source/rent_credit (`pre_market_funding_abort_v1.rs:460`) | YELLOW |
| B18 | `CreateFund`/`VerifyFundReady` (`core resolution.rs:562`) | anyone — route forbids all signers (`resolution.rs:576`) | market never reaches Ready | no | YELLOW |
| B19 | Generic Founding `Found` (`generic_founding_v1.rs:274,812`) | Trading caller-authority PDA must sign | nothing founded; pre-commit refund executed on real SVM (`controller_funding_split_abort.rs`) | — | ORANGE |
| B20 | User position admit (`trading user_position_admission_v1.rs:33`) | the position owner and only the owner signs (`user-position-admission-contract lib.rs:235`, enforced `user_position_admission_v1.rs:138,159`) | that user simply has no position | own value | GREEN-SELF |
| B21 | Prefunding vacant Position/Admission PDAs | anyone | wrong amount ⇒ lamports strand in a keyless system PDA — admission demands exact match (`claims protocol_position_v2.rs:1043` vs `:414-417`) and no drain route exists | — | **RED** (small, bounded blast radius) |
| B22 | Activation-cache close / rent reclaim | **no route exists** (56 non-test `ACTIVATION_PDA_DOMAIN_V1` sites, zero closers; re-drive to another set refused, `registry-contract activation.rs:425`) | 1288 bytes rent sunk per release set forever | — | **RED** (small) |

Territory-B systemic note: **every live expiry route pays a creation-fixed
beneficiary, never the caller** (B12, B14, B16, B17; dealer checkpoint cleanup
is the same shape). Bounded and conserving — the Dealer-cleanup template —
but altruism-dependent. The tree's one caller-funded payout construct was B6,
and it was unreachable until this lane's closure below.

### F. Trade spine (walked directly)

| act-point | who can act | if nobody acts | caller paid? | class |
|---|---|---|---|---|
| Direct registered-intent **fill** | anyone may submit compatible signed intents (untrusted matcher); makers/takers signed for their own value | intent rests until expiry; a trade not happening strands nothing | **no protocol pay to the submitter** — fees go to the config-pinned `fee_recipient` (`programs/dclutch-trading-sbf/src/direct/buy_escrow.rs:287`); matcher compensation is off-protocol | GREEN-SELF |
| Direct registered-intent **cancel** | maker only — `RegisteredTerminalEvidenceV2::Cancel` requires reauthenticating the exact signed intent (`crates/dclutch-direct-codec/src/successor.rs:2162-2163`, enforced `:2198-2199`) | falls through to expiry | own value | GREEN-SELF |
| Direct registered-intent **expiry unwind** | **anyone**, strictly after `valid_through` (`successor.rs:2164-2168`, enforced `:2200`) | escrowed collateral + record rent sit; a live record holds `open_maker_root_count` up and blocks Direct quiescence | **no** — collateral returns to the maker's signed destination (`programs/dclutch-trading-sbf/src/direct/buy_escrow.rs:541-542`), rent to the record's creation-fixed `rent_owner` (`successor.rs:2234`); the stranger who cranks it earns nothing | **YELLOW** |
| Direct registered-intent **invalidation unwind** | **anyone**, for any nonce below the maker's signed minimum-live threshold (`successor.rs:2169-2170`, enforced `:2201-2202`) | as above | as above — caller unpaid | **YELLOW** |
| Direct replay-account creation | anyone may fund; `RentRefund` role added by `f581af6b` refunds dust above exact rent to a named beneficiary, so public dust cannot brick creation (`crates/dclutch-custody-contract/src/frame_spec_v1.rs:12,111,214`) | position simply not materialized | n/a (griefing fix, not a bounty) | GREEN-SELF |
| General candidate **submit** | anyone (`candidate_v1.rs:297-298`) | batch settles on whatever candidates exist | escrow refund of unspent | GREEN |
| General candidate **verify / consider / cleanup cranks** | anyone; windows+counters gate, never identity (`candidate_v1.rs:274-276`) | candidate never competes — but the crank is prepaid, so the verb is live | **yes** — `WorkRewardV1` pre-debited per crank (`:237-249`), Verification + Cleanup compartments (`:251-258`) | **GREEN** |
| Relayed observation (healthy leg) | expected key-set member only (`crates/dclutch-relay-contract/src/lib.rs:127`); relayer selects/interprets nothing (`lib.rs:8-14`) | bounded: the funded failure walk becomes available at the deadline | relayer is off-protocol compensated; the *fallback* walker is bounty-paid | ORANGE (system row GREEN via exemplar 1) |
| Client journals (operator crash-safety) | n/a — journals are client conveniences, never authorities (`crates/dclutch-operator/src/direct_inline_route_v3.rs:2116-2130`) | every intermediate ON-CHAIN state has its own row in this register; a lost journal strands nothing the chain can't re-derive | n/a | benign |

Notes on the two YELLOW rows: the unwind verbs are conserving (the Dealer
cleanup shape — value to creation-fixed destinations, caller cannot redirect)
but unpaid, **and they sit on the retirement critical path**:
`DirectRootStateV1::require_closable`
(`crates/dclutch-direct-codec/src/successor.rs:616-623`) refuses physical root
closure until phase is `Retiring` AND `open_maker_root_count == 0`, and a maker
root only closes when its last live record does (`close_live`, `:2209`). So a
vanished maker's expired record blocks the Direct child's closure — and with it
market retirement — until a stranger cranks an unwind nobody pays for.
Fix shape: a per-record crank fee carved from `rent_principal` at registration
(maker-funded, exactly the candidate_v1 work-escrow pattern); the close plan
already computes `unclassified_donation`/`total_rent_credit`
(`successor.rs:2230-2238`), so the fee has a natural ledger seat. Costed as Q10
in the queue below.

### C. Provider capture, resolution, terminal admission

| act-point | who | if nobody | caller paid? | class |
|---|---|---|---|---|
| Sponsored capture (`sponsored_push_v1.rs:106,221`) | anyone — sole signer is an unconstrained System wallet; no sponsor/keeper identity anywhere (`pyth-svm sponsored_push.rs:111-152` has no sponsor field) | window closes at `end+max_age`; falls to the failure walk | no — caller pays candidate+head rent, `work_paid: 0` hardcoded (`:1274,1280`) | YELLOW |
| Sponsored settle (`sponsored_push_v1.rs:855,985`) | anyone (no signer pin) | market stalls in Primary; failure walk races it | **net-negative** — caller pays receipt rent, no close route for it | YELLOW |
| Close candidate/head (`sponsored_push_v1.rs:1421`) | anyone — zero signers admitted | rent sits | no — refund pinned to the capture payer (`:1479,1531`) | YELLOW |
| Provider submit/consume (`provider_transport_v3`, `provider_instruction_v3`) | anyone (self-declared submitter/resolver) | no update; market stalls until failure walk | no — caller tops up certificate rent | YELLOW |
| Provider reclaim (`provider_transport_v3.rs:941,953`) | anyone | **requires `Consumed`** — a submitted-never-consumed update strands (R12) | no — refund to submit-time recipient | **RED** (small) |
| **Failure walk** `CommitDeadlineFailure` (`relay-contract frame.rs:125,345-368`) | **anyone** — Worker signs, no key comparison | this is the only live terminal-failure route for a no-recovery market; value strands without it | **yes — bounty to the caller atomically** (`relay_transport_v1.rs:1504`, computed `:1184-1187`); `validate_shape` refuses `work_paid==0` | **GREEN** |
| Relay fill / seal (`relay_transport_v1.rs:717-720,794`) | pinned key-set member (1-of-n fill, m-of-n seal) | record never seals; failure walk still fires | no | ORANGE |
| Relay consume (`relay_transport_v1.rs:899`, worker discarded) | anyone once sealed | sealed record unconsumed; failure walk still fires | no | YELLOW |
| Relay retire (`relay-contract record.rs:751-753`) | anyone, any phase but Retired | rent sits | no — market beneficiary; **free DoS vs the success path** | YELLOW (adversarial) |
| Terminal admission into Core `AdmitTerminal` (`core resolution.rs:357,576`) | anyone — zero signers admitted | certificate exists but market never terminalizes ⇒ holders cannot claim | no — `beneficiary == [0;32]` | YELLOW |
| Pre-market funding (`pre_market_funding_v1.rs:249,265`) | Trading CPI + funding wallet signs | market never founds | n/a — it is the deposit | ORANGE |
| Pre-market abort (`pre_market_funding_abort_v1.rs`, `controller_funding_checkpoint.rs:613`) | Trading CPI PDA, post-expiry | bounded by `expiry_slot` | no — principal→funder, rent→rent_credit | ORANGE |
| Recovery-policy market at deadline | — | **neither success capture nor failure walk is admissible** (R2) — stuck in Primary forever | — | **RED** |

### D. Payout, redemption, retirement, rent recovery

| act-point | who | if nobody | caller paid? | class |
|---|---|---|---|---|
| Holder redemption (`terminal_settlement_v3.rs:577-584`, `signed_delta_v3.rs:516-520`) | the position owner, own signature only | value strands forever + blocks retirement (R3) | caller **is** the payee (`rational_terminal_v3.rs:362`) | GREEN-SELF |
| Fractional shard redemption (`fractional_atomic_v3.rs:1114-1120`) | the shard actor, own signature | as above | caller is the payee (`:790`) | GREEN-SELF |
| Rational terminal (`rational_terminal_v3.rs:280`) | Trading/lifecycle PDA (CPI) | value strands with the representation | payee is `header.actor`, a fixed identity **not the caller** | **RED** (identified party) |
| Claims-role Custody replay first-use (`custody_replay_v1.rs:385-386`) | anyone (payer unconstrained) | redemption blocked until paid; unblockable by anyone | no — caller pays replay rent | YELLOW |
| `begin_retiring` (`core begin_retiring.rs:57-58`) | anyone — route **refuses any signer** | market sits in Terminal | no lamports move at all | YELLOW |
| Direct root begin-retiring (`direct_begin_retiring_v1.rs:92-96`) | anyone — refuses any signer | Direct root stays Open; blocks root closure | no | YELLOW |
| Resolution `CloseFund` (`resolution.rs:359-367,576`) | anyone — refuses any signer | retirement blocked | no — beneficiary pinned to RentCredit | YELLOW |
| `CloseCapability` (`capability.rs:343-382`) | anyone — refuses any signer | `outstanding_capabilities != 0` ⇒ retire refuses | no — rent to RentCredit | YELLOW |
| Retirement replay handoff (`retirement_replay_handoff_v1.rs:66-67`; custody `:486-492,532-550`) | anyone as PAYER | Trading-role replay never becomes Core-role ⇒ retire refuses | **negative** — caller pays new replay rent, recovered rent → RentCredit | YELLOW |
| Joined / checkpointed retire (`retire_v1.rs:1190-1255,2579`) | anyone (direct refuses all signers; continuation needs a Registry PDA) | market stuck Retiring; all recovered rent locked | no — 100% to creation-fixed `refund_wallet` (`:1725,1739-1740`) | YELLOW |
| Claims market closure (`market_closure_v1.rs:315-317,669-681`) | Core PDA (CPI, reachable only from the permissionless retire) | retirement blocked | no — aggregate → RentCredit | YELLOW |
| Position/admission close (`protocol_position_v2.rs:774-785`; `dispatch.rs:141-156` refuses all family signers) | Trading PDA (CPI); **owner is readonly non-signer** — no owner veto | position rent strands; unrecoverable after retirement | no — → RentCredit | YELLOW |
| RentCredit sweep (`rent-sbf lib.rs:356-367`; `lifecycle_v2.rs:826-850`) | **anyone** — zero signers | surplus sits, reclaimable later | no — pinned `refund_wallet`; "a pure donation of a transaction fee" (`journey/stages.rs:748-758`) | YELLOW |
| RentCredit close (`rent-sbf lib.rs:492,587-597`) | Core PDA only (CPI at retire) | rent locked | no — → `refund_wallet` | YELLOW |
| **Sleeping holder** | — | zero-supply gate (`market_closure_v1.rs:669-681`) + empty-hoard gate (`custody lib.rs:950`), no Clock in the whole path ⇒ **retirement blocked forever** | — | **RED** (R3) |
| **Claims-role replay closer** | — | no route in claims-sbf issues `CloseReplay` (grep-verified) ⇒ one replay rent strands per wallet-redemption market | — | **RED** (R10) |

### E. Dealer, Custody delivery, checkpoint cleanup

| act-point | who | if nobody | caller paid? | class |
|---|---|---|---|---|
| Checkpoint Create `DCLTDCP1` (`dealer_scenario_checkpoint_v1.rs:256-262,297`) | the dealer in the request must sign | nothing starts | no | ORANGE (the sole hard identity gate) |
| Checkpoint Page/Evaluate `DCLTDPG1`/`DCLTDEV1` | anyone (no signer) — **but the Trading-owned artifacts have no creation route** (`:1360-1366`; only the campaign stages them) | dies at expiry → cleanup | no | YELLOW (blocked upstream) |
| Custody Reserve (`custody dealer_reservation_v1.rs:199`) | anyone (PAYER) | nothing locks; expires | no — caller pays escrow+state+receipt+batch rent, none refunded | YELLOW |
| Rollback (`dealer_reservation_v1.rs:311-322`, reverse-order `scenario_checkpoint_v1.rs:592-597`) | anyone, strictly post-expiry | escrow strands; cleanup blocked until all reservations reversed | no — escrow rent → fixed `refund_beneficiary` | YELLOW |
| Checkpoint Cleanup (`scenario_checkpoint_v1.rs:709-719`; on-chain `dealer_scenario_checkpoint_v1.rs:1744-1750`) | **anyone except the beneficiary** (`beneficiary.is_signer` refused) | 944-byte rent strands | no — rent → fixed beneficiary the caller can't be | YELLOW |
| Checkpoint Commit `DCLTDCM1` | anyone (no signer) | reserved collateral rolls back at expiry (bounded) | no | YELLOW |
| **Custody DELIVERY `activate_batch`** (`dealer_reservation_v1.rs:917,1080,1109-1121`) | anyone (ACT_PAYER) | **counterparty collateral strands — no Clock in the frame, no deadline, only exit post-Commit** | no — escrow rent → fixed beneficiary; caller pays a never-closed receipt PDA (`:1418-1450`) | **RED** (R9) |
| Dealer Fill / Unwind (`dealer-codec lib.rs:1828,1904`, `dealer-sbf lib.rs:2031`) | anyone signing as ACTOR | market stalls | **yes — `work_reward` to the signing ACTOR** | **GREEN** / GREEN-SELF |
| Dealer EnterTerminal / ActivateReplacement / Retire | anyone signing | market never terminalizes / funding split / vaults strand | no — fixed policy beneficiaries; vaults never closed | YELLOW |
| **Dealer ScheduleReplacement** (`dealer-codec lib.rs:1642`) — the sole liveness-vault refill | **`policy.dealer_id` only** | **liveness vault drains ⇒ Fill/Unwind die ⇒ inventory never zero ⇒ Retire refuses forever** | n/a | **RED** (R4) |
| Dealer entry (Policy/Candidate/State creation) | **no on-chain route** (dealer-sbf allocates nothing) | no dealer can be created on chain | n/a | **RED** (R4) |
| Dealer obligation close (`trading dealer/v3_obligation.rs:204-206`) | requires zero equity shares AND zero obligations | one sleeping LP share blocks dealer+market retirement forever | n/a | **RED** (R4) |

## Closures landed in this lane

Two RED gaps were small-and-unowned enough to close in place; the rest are
design rulings or larger reshapes, queued below.

1. **R5 / `a16d1b0b`** — `core: make the allocated founding permit
   completion-only`. Deleted the one conjunct (`current_slot > intent.expiry_slot()`
   in `authenticate_permit`, `generic_founding_v1.rs`) that made the 13:30
   ruling's non-expiring Open still expire in code. The pre-allocation refund
   family (`series_permit_expiry`) is disjoint by account state, so no route
   pair can race; the consumed-permit replay refusal is untouched and still
   pinned by SPLIT's `d60fbfb9` hostile. Control: `cargo check`; core-sbf unit
   tests 24/24; touched file clippy-clean.

2. **R6 / `c365179c`** — `registry: dispatch record Abort (action 4) — the
   funded cleanup`. Wired the contract's complete-but-undispatched
   `prepare_abort_v1` into the registry dispatcher. This is the only
   caller-funded payout construct in the tree, prepaid on every publication and
   previously dead. `process_abort` observes the accounts, calls the contract,
   binds every returned obligation to the exact SVM accounts, and applies the
   two closes with an exact-conservation postcheck; it handles both the early
   sponsor-signed branch (bounty withheld, actor≡sponsor aliasing) and the
   expired permissionless branch (bounty to the caller). Control: registry-sbf
   clippy-clean; 17/17 lib tests single-threaded and under `cargo nextest run`
   (the repo's canonical process-per-test runner); `cargo build-sbf` produces
   the ELF; two new tests + three hostiles with the pinned `Record` code.
   *Caveat*: `cargo test --lib` at ≥4 threads SIGSEGVs inside the pre-existing
   leaked-`Box` `AccountInfo` test helper (its `resize`/`assign` do runtime
   pointer math unsafe under in-process parallelism); the added tests only push
   the record module past that thread-count threshold. nextest is clean.

Both rode cohort-7 territory (Core founding permit edit is the ruling's named
one-conjunct change; registry rides cohort-7 per the 14:43 freeze) — neither is
on the devnet cut's critical path, and trading-sbf and the successor founding
path were untouched.

## The pattern behind the YELLOW rows — P1

**Every genuine expiry/cleanup/retirement route in the tree is permissionless
but pays a creation-fixed beneficiary, never the caller.** The verb is therefore
*permissible rather than live* — the tree's own words
(`candidate_v1.rs:290-295`). The protocol has exactly three caller-funded verbs
that make a permissionless act actually live: the funded failure walk (C6), the
General candidate work escrow, and — now — the record cleanup bounty (R6). Every
other permissionless route is a donation of a transaction fee, which the
gauntlet states outright (`journey/stages.rs:748-758`).

The **fix template** is uniform and already proven three times: carve a small,
prepaid, fully-refundable **work escrow** at the value's creation, sized to the
exact cranks its lifecycle needs, each crank drawing one pre-debited
`WorkRewardV1` to whoever performs it, the remainder refunding to the funder.
Applying it turns each YELLOW into GREEN without drawing on any Hoard. The
per-route costing is in the queue.

## Costed queue — the RED and YELLOW gaps not closed in this lane

Ordered by whether they are a **ruling** (a policy decision only ember/ORCH can
make), a **weld** (refuse a currently-reachable unsafe state), or a **build**
(a bounded implementation). File:line evidence is in the ranked table above.

| Q | gap | kind | cost / shape |
|---|---|---|---|
| Q1 | Upgrade bricks every live market's exit (R1) | ruling | choose one: ship `Immutable` production releases (zero code, loses upgradability); OR a market re-point route that re-authenticates a new release set and rewrites `selected_release_set` under a governance gate (large — touches every pin site); OR exempt the retirement/refund paths from the slot pin (medium — `retire_v1`/`begin_retiring`/`CloseFund` read the deployed ELF without the `>` slot check). Exit-exemption is the smallest that preserves both upgradability and no-strand. |
| Q2 | Recovery-policy market has no terminal (R2) | weld (now) + build (later) | weld: refuse founding a market with `recovery_policy.is_some()` until the ladder is live (small — one conjunct at the founding admission). build: resurrect `funded::process_funded_transition` from `#[cfg(any())]` and give `RecoveryAdvanced`/`Exhausted` live routes (large). Weld first — a market that cannot resolve must not be foundable. |
| Q3 | Sleeping holder blocks retirement forever (R3) | ruling | choose the terminal-value policy: (a) escheat-after-window — a post-terminal deadline after which unredeemed payout sweeps to a funded beneficiary and the market may retire (needs a Clock read in the closure path + a holder-notice window); or (b) perpetual-but-funded — accept the strand but fund a keeper to retire the emptied shell. (a) is the protocol-coherent one (bounded, no identified-party dependency). |
| Q4 | Dealer vanish bricks market; no entry route; LP veto (R4) | ruling + build | the Dealer family needs: an on-chain entry route (build, large); a force-exit/quiescence-timeout for an unresponsive dealer (ruling: what threshold, who is paid); and a bounded LP-share redemption or forced-buyout so one share cannot veto retirement. This is the least-live family — a full design pass, not a patch. |
| Q5 | Capability activation deadline has no counterparty (R8) | build | a permissionless post-deadline `LapseCapability` verb: a `Pending` slot past `activation_deadline_slot` transitions to a terminal `Lapsed`, its prepaid principal refunds (apply P1 — a small crank fee to the caller), and `all_closed()` admits `Lapsed` so the ledger can close. Bounded, ~one route + one status writer + the `all_closed` predicate. |
| Q6 | Claims-role Custody replay has no closer (R10) | build | issue a `CloseReplay` for the Claims role from within the retirement flow (it already closes the Core-role replay), or from claims-sbf under a caller-authority. Small; must land before the aggregate is closed (ordering). |
| Q7 | `DCLTPCA1` pre-commit abort is refund_owner-only (R11) | ruling | the lost-key strand is inherent to a signature-gated refund. Options: a post-expiry permissionless fallback that sends principal to the same fixed `refund_owner` account (no key needed to *trigger*, only to *receive*) — mirrors the record-abort shape. Frozen program; rides a post-cut window. |
| Q8 | Provider-update / prefund-mismatch / activation-cache strands (R12) | build (×3, small) | provider reclaim on a superseded update; a drain route for an over/under-prefunded Position/Admission PDA; an activation-cache close route. Each small and independent. |
| Q9 | Resolution deadline pays more for the worse outcome (Y3) | ruling + build | fund a Success compartment parallel to the Failure `Bounty` so capture/settle pays its caller (build — add the compartment + non-zero `work_paid` on success producers + schema conjunct), and gate relay `RetireRecord` on a non-active phase to kill the DoS (weld, small). Removes the perverse incentive at the deadline. |
| Q10 | Direct expiry/invalidation unwind is unpaid, on the retirement path (F) | build | a maker-funded per-record crank fee carved from `rent_principal` at registration (P1 shape); the close plan already computes `unclassified_donation`/`total_rent_credit` (`successor.rs:2230-2238`). Frozen program (trading-sbf); rides a post-cut window. |

**Priority read**: Q2 (weld) and Q6/Q8 (small builds) are cheap and unblock or
de-risk immediately. Q3 and Q1 are the two that most directly contradict the
charter's one-sentence differentiator — a market that *cannot retire* and a
market that an upgrade *bricks* are both identified-/no-party liveness
dependencies with permanent stranding — and both need a ruling before a build.
Q4 (Dealer) is the largest and least urgent (Dealer is not on the devnet cut).
