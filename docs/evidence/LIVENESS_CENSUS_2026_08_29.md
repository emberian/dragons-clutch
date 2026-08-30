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
   **Qualified by ESCROW `fdd189ac`: this is the canonical *description* of the
   property, not a deployed proof of it.** `WorkRewardV1` occurs nowhere outside
   its own crate (grep-verified: `candidate_v1.rs`, `escrow_v1.rs`,
   `escrow_v1/tests.rs`), and `GeneralCandidateV1`'s only consumer outside the
   crate is a harness test
   (`programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs:1084`)
   — **no program `src/` reaches it**. So "the standard every row is held to"
   has no SBF dispatcher behind it, and a lane told to copy it will find nothing
   to copy. The tree's one *deployed* funded permissionless crank is record
   `Abort` (R6, `c365179c`); it is the template, and
   `docs/design/FUNDED_CRANK_V1.md` §1 sets out why. First observed by the
   claim-check design (`docs/design/CLAIM_CHECK_COMPACTION_V1.md` §6.2) and
   re-verified here.

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
| R2 | **A market founded with a recovery policy has no terminal at all** once its provider goes silent: the failure walk refuses `recovery_policy.is_some()` (`source_resolution_v2.rs:466-468`) and the ladder's only call site is inside `#[cfg(any())]` (`resolution-proof lib.rs:225-232`; `funded.rs:19` admits it). Both shapes were foundable per `core resolution.rs:711-719`. | RED → **WELDED** | **`12d0deb5`** (LIVE-2) — `CreateFund` refuses a recovery material, `CoreSbfError::RecoveryWalkUnavailable` `0x3011`. The shape is no longer foundable; the build half (a live ladder) is still queue Q2 |
| R3 | **One sleeping holder blocks retirement forever**: zero-supply gate (`market_closure_v1.rs:669-681`, 0x5503) + empty-hoard gate (`custody lib.rs:950`), zero Clock reads in the whole terminal path. No escheat, no deadline. | RED | queue Q3 — ruling |
| R4 | **A vanished dealer bricks the market**: `ScheduleReplacement` is the sole liveness-vault refill and is dealer-gated (`dealer-codec lib.rs:1642`); `Fill`/`Unwind` refuse under one `work_reward` (`:1756,:1873`); exhausted vault ⇒ inventory never zero ⇒ `Retire` refuses (`:1914-1918`). No force-exit in the 8-action enum; **entry has no on-chain route either** (dealer-sbf allocates nothing). Sibling veto: one LP equity share blocks obligation close (`v3_obligation.rs:204-206`). | RED | queue Q4 |
| R5 | **The allocated founding permit stranded after its founder-chosen expiry** — the ruled non-expiring Open still expired in code (`generic_founding_v1.rs:1747` pre-edit), and the refund route only accepts an unallocated permit (`series_permit_expiry.rs:365-370`). | RED | **FIXED `a16d1b0b`** |
| R6 | **The tree's one caller-funded cleanup bounty is dead code**: every record `Begin` force-prepays it (`record-contract lib.rs:417`), `prepare_abort_v1` pays the caller at expiry (`lib.rs:1699-1714`), and the dispatcher never admits action 4 (`record_v1.rs:45-57`). Abandoned publications strand raw+cursor+bounty forever. | RED | **FIXED `c365179c`** (this lane) |
| R7 | **`AppendPage` is sponsor-pinned** (`record_v1.rs:113,354`) — an abandoned multi-page publication cannot be completed by anyone else (and pre-R6 could not be reclaimed either). With R6 fixed, bounded by expiry+abort. | ORANGE (was RED) | R6 bounds it |
| R8 | **A missed capability activation deadline has no counterparty**: slot stuck `Pending` (activate refuses past deadline `capability.rs:523-531`, close requires `Active` `:532-536`), ledger needs `all_closed()` (`funding.rs:1877`); the only status writers are `funding.rs:1774,1873`. Principal + shared ledger strand. ~~also blocks Market opening (`funding.rs:1971-1975`)~~ — **STRUCK by LIVE-2: that limb is dead code.** `AuthenticatedFundingLedgerV2::validate_market_open` (`funding.rs:1964-1979`) has no non-test callers, and its V1 twin (`:1174-1194`) is reached only from `MarketOpeningReadinessV1::advance` (`capability-contract lib.rs:771`), which has zero consumers outside its own crate. Neither market-open path is reachable from any SBF program. The strand is real; the opening-blockage was not. | RED (narrowed) | queue Q5 — lapse verb |
| R9 | **Custody DELIVERY (`activate_batch`) has no deadline and no funding**: no Clock account in the ACT frame, only exit post-`Commit` (rollback/cleanup refuse `Committed`), caller unpaid and pays a never-closed receipt PDA (`custody dealer_reservation_v1.rs:917,1080,1346-1352,1418-1450`). | RED | queue Q4 (with dealer) |
| R10 | **Claims-role Custody replay has no closer**: created permissionlessly (`custody_replay_v1.rs:385-386`), `CloseReplay` demands the role program's caller-authority, and claims-sbf issues no `CloseReplay` anywhere (grep-verified). One replay rent strands per wallet-redemption market. | RED (small) | queue Q6 |
| R11 | **`DCLTPCA1` pre-commit abort is `refund_owner`-only** (`ABORT_SIGNERS`, `projected_custody_bootstrap_v1.rs:261,2909-2914`; re-pinned in `custody projected.rs:1037-1040`) — a lost beneficiary key strands the staged principal exactly as if the route did not exist, against the route's own "way back out" docs. | RED (lost-key) | frozen program; queue Q7 |
| R12 | Submitted-never-consumed provider update strands (reclaim requires `Consumed`, **`programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:941`** — this row cited the wrong crate; the codec crate has no line 941); ~~keyless-PDA prefund mismatch strands~~ **withdrawn by LIVE-2, replaced by R13**; activation caches have no close route (56 sites, 0 closers — reproduced structurally, but see Q8c: not safely buildable before Q1). | RED (×2, **not small** — re-costed) | queue Q8a / Q8c |
| R13 | **NEW (LIVE-2): a 1-lamport front-run indefinitely blocks position admission AND position close — and close is on the retirement path.** Both SBF checks compare a keyless, off-curve, system-owned PDA's live balance to a caller-*declared* snapshot: `accounts.position.lamports() != request.observed_position_lamports` at Admit (`claims-sbf protocol_position_v2.rs:1043-1044`) and at Close (`:597-598`). Anyone may send one lamport to either PDA between the snapshot and the transaction landing, and the route refuses. Repeatable by anyone for ~1 lamport plus a fee, every slot, forever. This is what B21 was reaching for and missed: the *contract* is dust-tolerant by design (`<`, not `!=` — `claims-svm protocol_position_v2.rs:475-476`, pinned by `prepaid_creation_is_dust_tolerant_but_never_underfunded` at `:1400-1411`), so the defect is not the strand B21 described but a griefing verb against admission and against retirement. | **RED — ADMISSION HALF CLOSED `6b52ee52`; CLOSE HALF OPEN** | **Admit (`:1043-1044`) welded**: the comparison is now a floor (`live < declared` refuses), so underfunding still refuses and a donation no longer blocks. Shippable as ONE ELF because the admission route performs no lamport arithmetic — it allocates and assigns — so the persisted admission body and the emitted receipt are byte-identical to what an exact declaration produced, and no downstream re-derivation can observe the change. Red-before/green-after at `0x5145`, attack executed by a funded stranger rather than simulated. **Close (`:597-598`) NOT welded, deliberately**: a bare `<` there is worse than the bug, because `close_pair` (`:1227-1247`) does not sweep the live balance — it zeroes both PDAs and ABSOLUTELY ASSIGNS `rent_credit = rent_before + declared_total`, so relaxing the guard alone destroys lamports and hard-fails in the runtime rather than refusing. Fixing it changes what the receipt carries, and **trading-sbf re-derives rather than passes through** (`direct/sell_escrow.rs:497-523` rebuilds the entire expected receipt and compares all 19 fields; `claims_composition_v3.rs:1042` and `dealer/v3_lifecycle.rs:257` `validate_request`), so it is a TWO-ELF change over five binding sites. `6b52ee52` already asserts the stale close refusing at `0x5146`, so the remaining half has a red-before waiting for it. |
| Y1 | **Systemic: every genuine expiry/cleanup route pays a creation-fixed beneficiary, never the caller** — series permit expiry (**core-sbf** `series_permit_expiry.rs:417`), controller-ledger cleanups (**trading-sbf** `projected_custody_bootstrap_v1.rs:1274-1319`), capability close (**core-sbf** `capability.rs:582`), rent sweep (**rent-sbf** `lib.rs:418-419`, zero signers on the path, beneficiary pinned by `wallet.key != state.refund_wallet()` at `rent-contract lifecycle_v2.rs:837`), dealer checkpoint cleanup (**trading-sbf**) — where the beneficiary is even *forbidden* to pay its own fee (`dealer_scenario_checkpoint_v1.rs:1747` refuses `beneficiary.is_signer`), and the codec agrees in words: *"a cleanup caller cannot redirect rent or any other lamports"* (`dealer-codec scenario_checkpoint_v1.rs:707-708`). **Two corrections by ESCROW `fdd189ac`.** (a) *The programs*: this row read as a tree-wide smear; the sites are **3 trading-sbf / 2 core-sbf / 1 rent-sbf**, so three-fifths of the pattern is behind the frozen program — which is why it survived. (b) *A fabricated quotation, struck*: this row and the `RentCredit sweep` register row both attributed to the gauntlet the phrase "a pure donation of a transaction fee" at `journey/stages.rs:748-758`. **The gauntlet does not contain that phrase** — `grep -n donation` over that file returns nothing, and the only two occurrences in the repository were these two rows quoting each other. `stages.rs:746-771` is an *assertion*, not a lament: it fails the journey unless the fee payer moves by exactly `-fee`. The finding was right; the evidence was invented. | YELLOW | **pattern P1 below; ruled + sized in `docs/design/FUNDED_CRANK_V1.md`** — and **none of the five is mechanical**: three frozen, one needs a `CapabilityManifestV1` ABI change, one needs a Lean re-proof (`MarketCore.lean:368-369` defines the refund as the whole balance) |
| Y2 | **Systemic: retirement is 100% permissionless and 100% unfunded** — several routes refuse all signers (`begin_retiring.rs:57`, `retire_v1.rs:1190-1255`, `resolution.rs:576`), every recovered lamport flows to the creation-fixed `refund_wallet` (`retire_v1.rs:1725,1739-1740`), and two act-points are caller-*negative* (replay handoff `custody …handoff:486-492`; Claims replay creation D4). | YELLOW | pattern P1 |
| Y3 | **Perverse incentive at the resolution deadline**: success-settle and failure-walk open at the same instant; the failure walker is paid the bounty while the success settler is net-negative (`resolution-codec v2.rs:385-393` exempts success from `work_paid != 0`; both producers hardcode 0 — `sponsored_push_v1.rs:1274,1280`, `provider_v3.rs:306,312`). | YELLOW (adversarial) | queue Q9 — build half open |
| Y3b | **The griefing verb inside Y3, split out**: anyone could retire — and thereby CLOSE — a `Collecting` or `Sealed` relay record (`relay-contract record.rs:751-753`). A sealed record is one permissionless `ConsumeRecord` from resolving the market successfully, and consumption is reachable only through that success path, so a transaction fee bought the deletion of the honest outcome and forced the bounty-paying failure walk. Not unfunded liveness — funded **anti**-liveness. | RED (re-scored from YELLOW) → **WELDED** | **`04f00387`** (LIVE-2) — retiring a non-`Consumed` record now requires the Market to carry a terminal receipt; `ResolutionError::RecordStillConsumable` `0x8016`. Strand-free: consume is permissionless and the funded failure walk terminalizes with no identified party — and R2's weld removed the one shape that could never terminalize |

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
| B13 | Expired-`Pending` capability slot | **nobody.** Activate refuses past deadline (`capability.rs:523-531`; contract `funding.rs:1760`), Close requires `Active` (`capability.rs:532-536`; `funding.rs:1853`); the only status writers are Pending→Active (`funding.rs:1774`) and Active→Closed (`:1873`); `ActivationDeadlineElapsed` returned 4 places, handled 0 | prepaid principal strands AND the shared ledger never reaches `all_closed()` (`funding.rs:1877,2021`) — sibling residue and ledger rent strand with it. ~~an expired `Pending` entry also blocks Market opening~~ **STRUCK by LIVE-2**: `funding.rs:1964-1979` has no non-test callers and its V1 twin (`:1174-1194`) is reached only from `MarketOpeningReadinessV1::advance` (`capability-contract lib.rs:771`), which has no consumers outside its own crate — neither path is reachable from any SBF program | — | **RED** (strand real; opening-blockage withdrawn) |
| B14 | `series_permit_expiry` (`core series_permit_expiry.rs:167`) | anyone (0 signers) | this *is* the bound for series permits | no — permit lamports to `rent_credit` (`series_permit_expiry.rs:417`) | YELLOW |
| B15 | Controller funding `Prepare` (`trading projected_custody_bootstrap_v1.rs:275`) | `funding_source` wallet must sign (`:298`) | nothing staged; bounded by B16 | — | ORANGE |
| B16 | Controller funding expiry abort (`projected_custody_bootstrap_v1.rs:594`) | anyone — any signer refused (`:624`) | bounded by `expiry_slot` | no — principal→funding_source, rent→rent_credit | YELLOW |
| B17 | Resolution `pre_market_funding_abort_v1` | Trading PDA (request-derived, no wallet identity) | bounded by expiry | no — to funding_source/rent_credit (`pre_market_funding_abort_v1.rs:460`) | YELLOW |
| B18 | `CreateFund`/`VerifyFundReady` (`core resolution.rs:562`) | anyone — route forbids all signers (`resolution.rs:576`) | market never reaches Ready | no | YELLOW |
| B19 | Generic Founding `Found` (`generic_founding_v1.rs:274,812`) | Trading caller-authority PDA must sign | nothing founded; pre-commit refund executed on real SVM (`controller_funding_split_abort.rs`) | — | ORANGE |
| B20 | User position admit (`trading user_position_admission_v1.rs:33`) | the position owner and only the owner signs (`user-position-admission-contract lib.rs:235`, enforced `user_position_admission_v1.rs:138,159`) | that user simply has no position | own value | GREEN-SELF |
| B21 | Prefunding vacant Position/Admission PDAs | anyone | **CORRECTED by LIVE-2.** The contract does NOT demand an exact match: it refuses only underfunding (`observed < principal`, `claims-svm protocol_position_v2.rs:475-476`), documents the tolerance at `:790`, and pins it with `prepaid_creation_is_dust_tolerant_but_never_underfunded` (`:1400-1411`); over-prefund is fully recovered because close sweeps the entire balance of both PDAs to `rent_credit` (`claims-sbf protocol_position_v2.rs:1227-1229`), and underfunding never reaches chain because the operator tops up (`operator/user_position_admission_v1.rs:656-665`). What remains is narrow: lamports sent to a vacant PDA whose owner **never** admits have no drain, since nothing but Admit ever touches an empty system-owned position PDA. The serious defect at these lines is R13, not this one | — | **YELLOW** (was RED; the strand is real but narrow — see **R13** for the live attack) |
| B22 | Activation-cache close / rent reclaim | **no route exists** (56 non-test `ACTIVATION_PDA_DOMAIN_V1` sites, zero closers; re-drive to another set refused, `registry-contract activation.rs:425`) | 1288 bytes rent sunk per release set forever | — | **RED** (**not small** — LIVE-2: every consumer authenticates the cache by ownership (`core-sbf release.rs:135-140`), there is no market refcount anywhere, and an unrefcounted close route is R1's brick weaponized. See Q8c: **Q1 first**) |

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
| Relay retire (`relay-contract record.rs:751-753`) | anyone — but a non-`Consumed` record now requires the Market to carry a terminal receipt (`04f00387`, `0x8016`) | rent sits until the market terminalizes, then anyone reclaims it | no — market beneficiary; the **free DoS vs the success path is closed** | YELLOW (unfunded, no longer adversarial) |
| Terminal admission into Core `AdmitTerminal` (`core resolution.rs:357,576`) | anyone — zero signers admitted | certificate exists but market never terminalizes ⇒ holders cannot claim | no — `beneficiary == [0;32]` | YELLOW |
| Pre-market funding (`pre_market_funding_v1.rs:249,265`) | Trading CPI + funding wallet signs | market never founds | n/a — it is the deposit | ORANGE |
| Pre-market abort (`pre_market_funding_abort_v1.rs`, `controller_funding_checkpoint.rs:613`) | Trading CPI PDA, post-expiry | bounded by `expiry_slot` | no — principal→funder, rent→rent_credit | ORANGE |
| Recovery-policy market at deadline | — | **neither success capture nor failure walk is admissible** (R2) — stuck in Primary forever. **No longer reachable**: `CreateFund` refuses the shape (`12d0deb5`, `0x3011`), so no new market can be founded into it. The row stays RED as a description of the state, not of a reachable founding | — | **RED, unreachable** |

### D. Payout, redemption, retirement, rent recovery

| act-point | who | if nobody | caller paid? | class |
|---|---|---|---|---|
| Holder redemption (`terminal_settlement_v3.rs:577-584`, `signed_delta_v3.rs:516-520`) | the position owner, own signature only | value strands forever + blocks retirement (R3) | caller **is** the payee (`rational_terminal_v3.rs:362`) | GREEN-SELF |
| Fractional shard redemption (`fractional_atomic_v3.rs:1114-1120`) | the shard actor, own signature | as above | caller is the payee (`:790`) | GREEN-SELF |
| Rational terminal (`rational_terminal_v3.rs:280`) | Trading/lifecycle PDA (CPI) | value strands with the representation | payee is `header.actor`, a fixed identity **not the caller** | **RED** (identified party) |
| Claims-role Custody replay first-use (`custody_replay_v1.rs:385-386`) | anyone (payer unconstrained) | redemption blocked until paid; unblockable by anyone | no — caller pays replay rent | YELLOW |
| `begin_retiring` (`core begin_retiring.rs:57-58`) | anyone — route **refuses any signer** | market sits in Terminal | no lamports move at all | YELLOW — **but this row understated the CONSEQUENCE, and the miss was a RED.** Until `f6b53cc9` every holder-redemption route gated the Core phase on exact equality with `Phase::Terminal`, so this permissionless verb did not merely advance a phase: it **permanently destroyed every holder's redemption right and bricked the market**, since retirement needs zero outstanding supply (`market_closure_v1.rs:669-681`) and redemption is the only thing that drives supply toward zero. Found by the compaction design lane, not by this census; welded at five sites in `f6b53cc9` with coverage in `552097c7`. The lesson for this table: 'no lamports move' is not the same question as 'what does this transition cost someone', and only the second one finds this class. |
| Direct root begin-retiring (`direct_begin_retiring_v1.rs:92-96`) | anyone — refuses any signer | Direct root stays Open; blocks root closure | no | YELLOW |
| Resolution `CloseFund` (`resolution.rs:359-367,576`) | anyone — refuses any signer | retirement blocked | no — beneficiary pinned to RentCredit | YELLOW |
| `CloseCapability` (`capability.rs:343-382`) | anyone — refuses any signer | `outstanding_capabilities != 0` ⇒ retire refuses | no — rent to RentCredit | YELLOW |
| Retirement replay handoff (`retirement_replay_handoff_v1.rs:66-67`; custody `:486-492,532-550`) | anyone as PAYER | Trading-role replay never becomes Core-role ⇒ retire refuses | **negative** — caller pays new replay rent, recovered rent → RentCredit | YELLOW |
| Joined / checkpointed retire (`retire_v1.rs:1190-1255,2579`) | anyone (direct refuses all signers; continuation needs a Registry PDA) | market stuck Retiring; all recovered rent locked | no — 100% to creation-fixed `refund_wallet` (`:1725,1739-1740`) | YELLOW |
| Claims market closure (`market_closure_v1.rs:315-317,669-681`) | Core PDA (CPI, reachable only from the permissionless retire) | retirement blocked | no — aggregate → RentCredit | YELLOW |
| Position/admission close (`protocol_position_v2.rs:774-785`; `dispatch.rs:141-156` refuses all family signers) | Trading PDA (CPI); **owner is readonly non-signer** — no owner veto | position rent strands; unrecoverable after retirement | no — → RentCredit | YELLOW |
| RentCredit sweep (`rent-sbf lib.rs:356-367`; `lifecycle_v2.rs:826-850`) | **anyone** — zero signers anywhere on the path (`SweepAccountsV2::parse`, `rent-sbf lib.rs:356-367`, performs no signer check; the contract only *forbids* the credit signing, `lifecycle_v2.rs:832-837`) | surplus sits, reclaimable later — **so this is a *surplus* route, not a closing one**, the tree's only instance (`FUNDED_CRANK_V1.md` §3.1) | no — `refund_wallet` is creation-fixed, read from the credit's own bytes and pinned at `lifecycle_v2.rs:837`; there is no caller account in the frame at all. **Quotation struck by ESCROW `fdd189ac`**: this row previously cited the gauntlet saying "a pure donation of a transaction fee" at `journey/stages.rs:748-758`. That phrase does not occur in the gauntlet or anywhere else in the tree except this census. What is there is the opposite kind of statement — `stages.rs:762-771` *asserts* the payer moves by exactly `-fee` | YELLOW |
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

3. **R2 / `12d0deb5`** (LIVE-2, on the 16:36 ORCH ruling) — `core: refuse to
   found a market whose failure walk is dead code`. Core `CreateFund` is the
   action that mints the `SourceResolutionStateV2`, and that state has no
   terminal when its material carries a recovery policy: the exhaust transition
   refuses the material outright and the ordered ladder meant to serve it has
   its only call site under `#[cfg(any())]`. The weld is one conjunct on
   **creation only** — `VerifyFundReady`, `CloseFund` and `AdmitTerminal` stay
   admissible, because a weld may not take routes away from a state that
   already exists. New refusal `CoreSbfError::RecoveryWalkUnavailable` `0x3011`;
   the off-chain builder mirrors it so the refusal lands at the desk rather than
   the validator. The predicate `recovery_walk_has_a_live_route` is the whole of
   the revert when Q2's build half lands. Control: `cargo check`; two new
   hostiles green under nextest — the first re-executes the *premise* (a
   no-recovery material walks one second past the deadline, a recovery material
   is refused there and at `i64::MAX`) so it goes red the day the ladder becomes
   live; clippy clean on all four touched files; `cargo build-sbf` produces the
   ELF on hbox.
   *Cost stated plainly*: a founder who stages this shape now loses the stage-1
   outlay at `CreateFund` rather than discovering at the deadline that the
   market cannot resolve. A pre-trade founding refusal in place of a post-trade
   permanent strand is the trade, and it is the point.

4. **Y3b / `04f00387`** (LIVE-2) — `resolution: a live market's evidence is not
   a stranger's to delete`. `RetireRecord` is permissionless and CLOSES the
   account; it admitted `Sealed`. A sealed record is one permissionless
   `ConsumeRecord` from resolving the market successfully, and consumption is
   reachable only through that success path, so one transaction fee bought the
   deletion of the honest outcome and forced the market onto the bounty-paying
   failure walk. Retiring a non-`Consumed` record now requires the Market to
   carry a terminal receipt — a fact already in the `RetireRecord` frame and
   already decoded for its rent beneficiary, so no frame or ABI moved. New
   refusal `ResolutionError::RecordStillConsumable` `0x8016`.
   *This is where the two welds compose*: the refusal is "not yet", and what
   makes it "not yet" rather than "never" is that every market terminalizes —
   which is true precisely because R2's weld removed the one shape that could
   not. Control: the contract hostile performs the attack on both live phases,
   asserts no bytes were written, and walks both ways out; the real-SVM campaign
   `the_record_transport_runs_create_append_seal_and_retire` **was performing
   this attack as its happy path** and now asserts the refusal by code on the
   compiled adapter before terminalizing and running its unchanged lamport
   conservation.

All four rode cohort-7 territory (Core founding permit edit is the ruling's
named one-conjunct change; registry rides cohort-7 per the 14:43 freeze;
`12d0deb5` and `04f00387` are flagged cohort-critical in their commit messages
and posted to SPINE-2) — none is on the devnet cut's critical path, and
trading-sbf and the successor founding path were untouched throughout.

## The pattern behind the YELLOW rows — P1

**Every genuine expiry/cleanup/retirement route in the tree is permissionless
but pays a creation-fixed beneficiary, never the caller.** The verb is therefore
*permissible rather than live* — the tree's own words
(`candidate_v1.rs:290-295`). The protocol has exactly three caller-funded verbs
that make a permissionless act actually live: the funded failure walk (C6), the
General candidate work escrow, and — now — the record cleanup bounty (R6). Every
other permissionless route is a donation of a transaction fee, which the
gauntlet states outright (`journey/stages.rs:748-758`).

The **fix template** is **two** shapes, not one, and the choice between them is
forced rather than stylistic — **ruled and sized in
`docs/design/FUNDED_CRANK_V1.md`**, which supersedes this paragraph's earlier
claim that the template was "uniform and already proven three times".

- **Prepaid-at-creation** — carve a small, fully-refundable **work escrow** at
  the value's creation, sized to the exact cranks its lifecycle needs, each
  drawing one pre-debited reward to whoever performs it, the remainder refunding
  to the funder. This shape *may* refuse for underfunding, which is safe only
  because creation force-prepays it. Available only where a creation act already
  signs, already pays rent, **and can be made to refuse** — which an existing
  route being converted after the fact almost never can.
- **Residual-at-close** — the reward is a capped slice of lamports already
  leaving: `min(floor, residual)`. It can *never* refuse, which is mandatory
  rather than nice: a crank that refuses for money is an unturned crank, i.e.
  the same defect through the funding door. **Every conversion in the Y1 set is
  this shape.**

Two corrections to what this paragraph claimed. (1) `WorkRewardV1`
(`candidate_v1.rs:237-249`) is the canonical *description* of the property but
**has no deployed SBF dispatcher** — its only non-crate consumer is a harness
test — so it is not one of the three proofs. The tree's one **deployed** funded
permissionless crank is record `Abort`, and it is the template. (2) The floor
must be **derived from the Rent sysvar, never written as a literal** — the
deployed route does exactly this (`registry-sbf record_v1.rs:348,362`), and the
one literal in the tree is `COMPACTION_CRANK_REWARD_LAMPORTS_V1`
(`claims-svm claim_check_v1.rs:108`), 14.8× below it.

Applying either shape turns a YELLOW into GREEN without drawing on any Hoard.
The per-route sizing — **measured, not estimated** — is in `FUNDED_CRANK_V1.md`
§9, and its headline is that **none of the Y1 sites is mechanical**: each sits
behind a different gate.

## Costed queue — the RED and YELLOW gaps not closed in this lane

Ordered by whether they are a **ruling** (a policy decision only ember/ORCH can
make), a **weld** (refuse a currently-reachable unsafe state), or a **build**
(a bounded implementation). File:line evidence is in the ranked table above.

| Q | gap | kind | cost / shape |
|---|---|---|---|
| Q1 | Upgrade bricks every live market's exit (R1) | ruling | choose one: ship `Immutable` production releases (zero code, loses upgradability); OR a market re-point route that re-authenticates a new release set and rewrites `selected_release_set` under a governance gate (large — touches every pin site); OR exempt the retirement/refund paths from the slot pin (medium — `retire_v1`/`begin_retiring`/`CloseFund` read the deployed ELF without the `>` slot check). Exit-exemption is the smallest that preserves both upgradability and no-strand. |
| Q2 | Recovery-policy market has no terminal (R2) | ~~weld~~ **DONE `12d0deb5`** + build (still open) | weld: **landed**. The seam was `CreateFund`, not "founding" generally — that is the action minting the state with no exit, and welding it lands the refusal before any position can be sold. build (open, large): resurrect `funded::process_funded_transition` from `#[cfg(any())]` and give `RecoveryAdvanced`/`Exhausted` live routes. Deleting `recovery_walk_has_a_live_route` (core-sbf `resolution.rs`) is the whole of the un-weld, and the hostile beside it goes red on the day the ladder works, which is the reminder. |
| Q3 | Sleeping holder blocks retirement forever (R3) | ruling | choose the terminal-value policy: (a) escheat-after-window — a post-terminal deadline after which unredeemed payout sweeps to a funded beneficiary and the market may retire (needs a Clock read in the closure path + a holder-notice window); or (b) perpetual-but-funded — accept the strand but fund a keeper to retire the emptied shell. (a) is the protocol-coherent one (bounded, no identified-party dependency). |
| Q4 | Dealer vanish bricks market; no entry route; LP veto (R4) | ruling + build | the Dealer family needs: an on-chain entry route (build, large); a force-exit/quiescence-timeout for an unresponsive dealer (ruling: what threshold, who is paid); and a bounded LP-share redemption or forced-buyout so one share cannot veto retirement. This is the least-live family — a full design pass, not a patch. |
| Q5 | Capability activation deadline has no counterparty (R8) | build — **re-costed by LIVE-2, ~110-130 hand-written lines + a Lean re-proof, TWO ELFs** | The good news this entry missed: **no `Lapsed` status is needed and `all_closed()` needs no new arm.** A `Pending → Closed` lapse satisfies every existing invariant free — `close_slot_in_place` already zeroes `raw.remaining` (`funding.rs:1874`), which makes the `Closed` conservation check `released == quote` hold, and the native refund is already the full prepaid quote for a `Pending` slot (`:1878`). That keeps the Lean-modelled `Status` inductive untouched (`formal/.../CapabilityFundingLedgerV2.lean:26-30`) and reduces the funding side to relaxing one conjunct at `funding.rs:1853` plus threading `current_slot` — about 15 lines. The bad news it also missed, and it dominates: **`Action` is Lean-generated** (`market-core-codec/src/generated.rs:1`), and `outstanding_capabilities` mis-accounts — `activate_capability_child` increments (`generated.rs:1081-1084`) and `close_capability_child` does `checked_sub(1)` (`:1109-1112`), so a slot that never activated cannot be routed through `CloseCapability` without underflowing or silently decrementing a live capability. A distinct action is therefore required: edit `formal/dclutch-semantics/DClutchSemantics/MarketCore.lean` + `EmitMarketCoreRust.lean`, regenerate, re-prove — and because `Action` is a Core↔Trading contract branched on at `trading-sbf/src/outer.rs:209,228,548`, **trading-sbf must ship too**. Applying P1 here (a caller crank fee) is a *separate, larger* project: the refund destination is hardcoded to `state.rent_beneficiary` (`capability.rs:582`) and carving a caller fee is a `CapabilityManifestV1` ABI change. ELFs: core-sbf **and** trading-sbf. |
| Q6 | Claims-role Custody replay has no closer (R10) | build — **re-costed by LIVE-2: NOT small, and it carries a hazard; downstream of Q3** | **Correction 1 — this entry's first option does not exist.** "Issue a `CloseReplay` from within the retirement flow" is not available: Custody's replay ops require a `CallerAuthority` PDA derived under `request.caller_program`, and Custody separately authenticates that `caller_program` is the Registry-activated program for `request.caller_role` (`claims-sbf custody_replay_v1.rs:9-16` states this). **Only claims-sbf can mint a Claims-role caller authority.** That is precisely why the Trading-role replay needed `retirement_replay_handoff_v1` (463 lines) rather than a direct close — so option (a) is a *second handoff of that size*, not a small addition. **Correction 2 — the standalone route is 300-400 lines.** It is a faithful mirror of `custody_replay_v1::process`: a new 48-byte wire, the aggregate and core-rent-refund authentication reused, a 12-account frame (Custody's `CloseReplay` frame is 10 = 9 common + `RentRefund`), CPI plus receipt postcheck; the request's `expected_revision` must come from the live replay and its `payer` must be zero (`custody-contract lib.rs:727-742`). **Correction 3 — the hazard.** `CustodyReplayV1` is the anti-replay cursor and the Claims payout **reads** it (`rational_terminal_v3.rs:284,374`, `expected_custody_replay_revision`). Creation is permissionless from any payer. Custody's only guard on `CloseReplay` is `open_vault_count != 0` (`custody-contract lib.rs:915`) — and **claims-sbf never opens or closes a vault anywhere** (grep-verified: zero `OpenVault`/`CloseVault` sites in `programs/dclutch-claims-sbf` and `crates/dclutch-claims-svm`), so that guard is **vacuous for this role**. An ungated closer is therefore a close-and-recreate primitive resetting `next_revision` to 1, available to anyone at any time. **Answer before building: does any Claims redemption's safety rest on the cursor, or only on position state?** **Correction 4 — the ordering is stronger than stated.** The gate must be the aggregate's terminal zero-supply predicate (`market_closure_v1.rs:669-681`), not `Phase::Retiring`: redemption must stay possible during Retiring or zero supply is never reached. So **Q6 inherits Q3** — a sleeping holder blocks the replay-rent recovery exactly as it blocks retirement. **Q6 is downstream of an ember ruling.** ELF: claims-sbf. |
| Q7 | `DCLTPCA1` pre-commit abort is refund_owner-only (R11) | ruling | the lost-key strand is inherent to a signature-gated refund. Options: a post-expiry permissionless fallback that sends principal to the same fixed `refund_owner` account (no key needed to *trigger*, only to *receive*) — mirrors the record-abort shape. Frozen program; rides a post-cut window. |
| Q8 | Provider-update / prefund-mismatch / activation-cache strands (R12) | build (×3) — **all three re-costed by LIVE-2; none is small, and one must not be built at all yet** | See Q8a/Q8b/Q8c below. They are independent of each other but each is larger than "small", and Q8c is actively dangerous before Q1 is ruled. |
| Q8a | Provider reclaim requires `Consumed` | build, ~250-350 lines, one ELF | **The file citation was wrong**: `:941,953` are in `programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs`, not the codec crate (which has no line 941). **Relaxing the conjunct is impossible**: `ProviderReclaimRequestV3::decode` refuses `terminal_sequence == 0` and any zero identity, and `certificate` is one of them (`resolution-codec provider_transport_v3.rs:221-227`) — a `Submitted` lifecycle has *both* at zero by construction (`:465-491`), so **the wire physically cannot encode an abandon-reclaim**. It needs a new magic, a new request type, a new 17-account frame, and — unlike the `Consumed` route — a **new terminality conjunct**, because reclaim reads neither source nor market state and `reclaim_after_unix_seconds` is submitter-chosen (validated only `>= publish_time`, `:470`). Without that gate the new route would let anyone destroy a live consumable update inside the resolution window. Confirmed unreachable-forever otherwise: the only writer of `Consumed` is `consume` (`:365-376`), whose sole route requires `market.phase == Open && readiness == Consumed` (`provider_instruction_v3.rs:507-508`). ELF: resolution-proof-sbf. |
| Q8b | Prefund mismatch (B21) | **the stated defect does not exist; a worse, unreported one does** | **The census is wrong that "admission demands exact match."** The contract is explicitly dust-*tolerant* — `observed_position_lamports < position_rent_principal` refuses, `<` not `!=` (`claims-svm protocol_position_v2.rs:475-476`), documented at `:790` and pinned by `prepaid_creation_is_dust_tolerant_but_never_underfunded` (`:1400-1411`). Over-prefund is fully recoverable: close sweeps the whole balance of both PDAs to `rent_credit` (`claims-sbf protocol_position_v2.rs:1227-1229`). Under-prefund never reaches chain (the operator tops up, `operator/user_position_admission_v1.rs:656-665`). **What is real, and unreported: a 1-lamport front-run grief on BOTH Admit and Close.** The *SBF* checks are exact against a caller-declared snapshot — `accounts.position.lamports() != request.observed_position_lamports` at Admit (`:1043-1044`) and at Close (`:597-598`). Both PDAs are keyless, off-curve and system-owned, so **anyone can send 1 lamport between snapshot and landing and force a refusal, indefinitely, for ~1 lamport plus a fee per slot** — and **Close is on the retirement path**. That is a cheap repeatable liveness attack against market retirement; it belongs in the ranked table, not as a footnote to a defect that isn't there. **Fix cost, corrected**: `f581af6b`'s `replay_rent_normalization` (`custody-sbf lib.rs:742-770`) is the right pattern and the Claims frame already carries `RentCredit` at index 24, so no account-count change — but the ~45-line estimate is **too low**, because `ProtocolPositionCloseReceiptV2::new` binds the declared lamports to the actual rent delta: `rent_credit_before + (observed_position + observed_admission) == rent_credit_after` (`claims-svm protocol_position_v2.rs:877-884`). Tolerating dust therefore means changing the **receipt contract**, and those receipts are consumed by **trading-sbf (frozen)**. Real shape: relabel `observed_*` as floors, record the swept truth in the receipt evidence, re-check every receipt consumer. ELF: claims-sbf, plus a receipt-ABI review that reaches trading-sbf. |
| Q8c | Activation cache has no close route (B22) | **do not build — build Q1 first** | The "56 sites, zero closers" claim reproduces (55 non-test plus the definition at `registry-contract activation.rs:20`), and zero closers is structural, not just grep: the Registry program has exactly two typed instructions plus three magic sub-dispatchers (`registry-sbf lib.rs:160-190`) and nothing anywhere resizes, reassigns or drains a cache. But **"RED (small)" is wrong and the direction is backwards.** Every consumer authenticates the cache by *ownership* (`core-sbf release.rs:135-140,227-241,329-331`), so closing one flips the owner to System and refuses them all — including **Core `Retire` on both legs** (`retire_v1.rs:488,732`), Claims market closure (`market_closure_v1.rs:476`), Trading `BeginRetiring` (`direct_begin_retiring_v1.rs:671`) and eleven more. **And there is no refcount**: the cache layout is header + release-set id + 5 role slots with 4 reserved bytes (`activation.rs:37-42`), and Registry cannot see how many markets pin a release set because `selected_release_set` is per-market and lives in Core. So an unrefcounted close route **is R1's brick, weaponized**: anyone funding one transaction could permanently disable retirement for every market on that release set. Safe shape: fold it into the Q1 ruling — a `CloseActivation` gated on a Core-maintained `markets_on_release_set == 0`. Order: **Q1 ruling → refcount → close route.** ELFs: registry-sbf **and** core-sbf, not one. |
| Q9 | Resolution deadline pays more for the worse outcome (Y3) | ruling + build | fund a Success compartment parallel to the Failure `Bounty` so capture/settle pays its caller (build — add the compartment + non-zero `work_paid` on success producers + schema conjunct), and gate relay `RetireRecord` on a non-active phase to kill the DoS (weld, small). Removes the perverse incentive at the deadline. |
| Q10 | Direct expiry/invalidation unwind is unpaid, on the retirement path (F) | build | a maker-funded per-record crank fee carved from `rent_principal` at registration (P1 shape); the close plan already computes `unclassified_donation`/`total_rent_credit` (`successor.rs:2230-2238`). Frozen program (trading-sbf); rides a post-cut window. |

**Priority read**: Q2 (weld) and Q6/Q8 (small builds) are cheap and unblock or
de-risk immediately. Q3 and Q1 are the two that most directly contradict the
charter's one-sentence differentiator — a market that *cannot retire* and a
market that an upgrade *bricks* are both identified-/no-party liveness
dependencies with permanent stranding — and both need a ruling before a build.
Q4 (Dealer) is the largest and least urgent (Dealer is not on the devnet cut).
