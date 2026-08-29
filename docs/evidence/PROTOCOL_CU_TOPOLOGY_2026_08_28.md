# Protocol compute topology and checkpoint architecture

Date: 2026-08-28  
Audited accepted source: `4dc4bf7ad639c2d534e23432cf0c99c83f53d984`  
Scope: all seven permanent programs and the externally reachable successor
paths. This is an architecture audit, not a checked release or deployment
record.

## Executive result

The protocol does not have one protocol-wide compute problem. It has three
different shapes:

1. Publication, role activation, participant setup, provider transport,
   Direct setup, payout, and most retirement work are already incremental.
   Their durable state or exterior journals give each transaction one bounded
   job. These paths should be measured and finished, not redesigned.
2. Direct Hot and the atomic Open-market transition mint or move liabilities.
   They must remain one onchain rollback domain. Their lever is less repeated
   authentication and fewer ephemeral PDA searches, not a transaction split.
3. Controller funding before Open was independent of liability creation but
   was embedded in `DCLTPCB2`. That was the wrong atomic boundary. Accepted
   source now contains a 512-byte `DCLTCFP1` checkpoint and Trading/Resolution
   handlers for `Prepared -> CustodyStaged -> Open-or-abort`. The successor
   caller and fresh real-SBF evidence do not yet consume that split at this
   audited commit. Finishing and measuring this vertical slice is the first
   compute milestone.
4. The current V6 Core-wrapped Resolution lifecycle is a second independently
   measured ceiling failure. Its exact ProgramTest suite passed 0/3 at the
   1,400,000-CU maximum: the activation child consumed about 1,180,908 CU and
   the Core wrapper then exhausted; the close child consumed the approximately
   1,129,805 CU left after its wrapper and the transaction exhausted. The
   byte-sized V5-to-V6 controller-release constant change is not a causal
   explanation. Activation and terminal close need an incremental receipt
   boundary that removes the nested Core wrapper from their heavy mutation.

The latest real private-validator failure before the split was not marginal
noise: the transaction consumed the chain maximum. At `2b0e6c29`, outer Trading
consumed 1,399,494 of the 1,399,550 units available to it and the transaction
consumed 1,400,000. Its nested Resolution call consumed 623,608 of 623,664,
including 258,805 in Core. Moving that call, the Trading ledger creation, and
their planning into PREPARE should remove roughly 550,000-650,000 CU from the
Custody-staging transaction after allowing for checkpoint authentication. That
is an estimate; the new route has not yet produced a source-bound SBF result.

No new validator campaign was run for this report. Existing exact evidence
already identifies the boundary, while the accepted caller still constructs
the old combined route and the shared caller/runtime lanes are changing. A new
probe before the caller freezes would measure a transient program with no
accepted exterior.

## Evidence rules used here

- **Observed** means one named transaction or child invocation from a preserved
  real-SBF/validator/ProgramTest record. It is not a bound.
- **M-61** is reported only as pass count and the arithmetic mean over exactly
  20 seeds. No minimum draw is called a margin.
- **Estimated** means source-topology arithmetic or a range inferred from
  observed child costs. It must be replaced by exact source-bound measurement.
- ProgramTest evidence is not devnet evidence, and devnet evidence is not
  mainnet evidence.

The current Trading ELF has changed after the most recent M-61 run. The newest
exact pre-split result is therefore only a baseline: source `2b0e6c29`, Trading
ELF SHA-256
`675c9c45bde6089ef4b57daf770ece7d2bd33870a0043e42e5d0e2119c229d2a`,
**20/20 passed, 20-seed arithmetic mean 1,359,277 CU**. A post-split M-61 result
does not exist yet.

## Seven-program map

| Permanent program | Compute-bearing responsibility | Current boundary | Evidence / risk |
|---|---|---|---|
| Registry | Begin/Append/Finalize immutable records; first-admission whole-ELF authentication; one-role activation; recurring role reauthentication | Publication is paged; activation is one role per transaction | Accepted control observations: Core 565,457; Claims 535,732; Trading 828,069; Resolution 351,936; Custody 259,058 CU. One draw each. Correctly below the ceiling. |
| Rent | Create, sweep, and close lifecycle RentCredit state | Separate lifecycle operations | No current source-bound CU series. It must never source value from Hoard principal. No architecture change indicated. |
| Core | Market state; ProjectFound/Found/Open; provider execution entry; begin-retiring and final aggregate retirement | Found/Open liability mutations are atomic; final retirement is still a three-child atomic close | Found31 observed 237,041 CU. Historical founding Core stage observed 433,129. ProjectFound inside the failed private run observed 258,805. Final retirement is unmeasured and is the next credible split candidate. |
| Claims | Aggregate/Position ownership, admission, claims mutations, terminal payout, market closure | Participant admission and each wallet payout are independent; aggregate closure is a child of final Core retirement | Claims Position admission observed 236,191. Paying payout 354,370; owner-paid 354,369; zero payout 223,244 CU. No ceiling pressure. |
| Trading | Cross-program composition, controller-funding checkpoint, Direct setup/Hot, capability lifecycle | PREPARE/stage/Open split exists in programs; Direct Hot and Open remain atomic | Pre-split controller staging exhausted 1.4M. Direct Hot historical M-61 baselines are near the ceiling. Trading is the program whose ELF changes require the next M-61 and frame gate. |
| Resolution | Controller-owned funding ledger; provider/Pyth submit/execute/reclaim; relayed observations; Source/certificate state | Funding creation is moving to PREPARE; provider is already Submit -> Execute -> Reclaim; relayed records are Create -> Append -> Seal -> Consume/Failure -> Retire | Focused fixed-payer pre-market caller fell 861,269 -> 813,778 CU; Resolution-exclusive work 512,659 -> 474,174 after duplicate-auth removal. Current V6 Core-wrapped lifecycle is 0/3 at 1.4M: activation child about 1,180,908 CU; close child used the approximately 1,129,805 CU left by its wrapper. |
| Custody | Replay/vault lifecycle and Token-2022 movement | Each ordinary operation is independent; founding stage composes three calls; final retirement composes two closes | Ordinary observed transactions are 139,746-158,020 CU. Pre-split founding calls observed 355,209, 98,172, and 113,313 CU at `2b0e6c29`. |

System, Token-2022, address-lookup-table, Pyth Receiver, and Pyth Router calls
consume transaction CU but are not permanent dClutch role programs. Exterior
reconciliation consumes no chain CU; only the transactions it observes do.

## Path topology

### 1. Publication, infrastructure, and activation

The Registry already implements the right incremental form:

```text
record: Begin -> Append page(s) -> Finalize
release: initialize Core infrastructure -> ActivateRole(Core) -> ... -> ActivateRole(Custody)
```

Publication owns a staging cursor with the sponsor/refund identity, exact
length, digest, liveness window, and append progress. A crash resumes the next
page or expires/refunds the cursor. No partial publication is readable as a
finalized record.

Activation deliberately hashes one deployed ELF on first admission. It cannot
authenticate five multi-hundred-kilobyte programs inside one transaction, so
the activation cache is written one role at a time and is unreadable as a
complete release until every slot is present. Recurring role authentication
uses the cache plus Loader-v3 slot/authority facts and avoids another full ELF
hash where policy permits.

Observed transaction-publication examples in the preserved private run were
10,002-30,171 CU for Begin/Append/Finalize shapes. They are single observations,
not budgets. First-admission activation remains the expensive but correct
operation. Caching an unverified claimed ELF digest would weaken admission.

**Decision:** keep these boundaries. Add source-bound CU rows for every
publication operation and the five activations at convergence; do not create a
new activation checkpoint.

### 2. Founding PREPARE, Custody stage, Open, and abort

#### Accepted program architecture

`crates/dclutch-capability-contract/src/controller_funding_checkpoint.rs` is
the single semantic owner of the fixed 512-byte `DCLTCFP1` record. Its PDA is
derived from release set, Market, generation, manifest, and canonical funding
list. It persists only:

- `Prepared` revision 1: both exact Pending ledgers exist;
- `CustodyStaged` revision 2: the ordered four-account Custody ladder digest
  and staging slot are committed.

It binds the Found request and ProjectFound receipt digests, both controller
ledger keys and initial byte digests, canonical masks, funding source,
RentCredit, terminal Lock digest, and expiry. There is no persisted
`OpenConsumed`: Open closes the checkpoint last and returns the terminal fact.

The intended topology is:

```text
PREPARE (DCLTCFQ1)
  Resolution Pending ledger -> Trading Pending ledger -> checkpoint Prepared LAST

STAGE (DCLTPCB2)
  authenticate Prepared -> Custody Initialize -> OpenHoard -> OpenSource
  -> checkpoint CustodyStaged LAST

OPEN (DCLTGMF2)
  Lock -> Core Found -> Custody Realize -> Claims Founding -> Core Open
  -> checkpoint close LAST

PREPARED ABORT (DCLTCFA1, after expiry)
  Resolution ledger refund + Trading ledger refund -> checkpoint close LAST

STAGED ABORT (DCLTPCA1, after expiry)
  exact Custody source abort FIRST -> both ledger refunds in canonical
  lowest-selected-bit order -> checkpoint close LAST
```

Every arrow inside one line is one Solana transaction and therefore one
rollback domain. PREPARE failure returns all three accounts to their vacant
System prestates. STAGE failure leaves the checkpoint Prepared. Open failure
leaves the entire staged prestate intact and abortable. Abort failure leaves
the prior checkpoint phase intact.

#### Accepted-source gap

At the audited commit, program handlers and semantic owners have landed, but
`tools/local-validator/bootstrap/successor/src/market.rs` still builds and
journals the old combined `DCLTPCB2` route. Its accepted census still asserts
62 complete keys for that combined route and 59 for `DCLTGMF2`. Those are not
censuses of the new PREPARE/stage/abort packets.

The program constants now expose these physical frames:

| Route | Physical accounts in the instruction | Accepted compiled unique-key evidence |
|---|---:|---|
| PREPARE | 49 | missing |
| Custody STAGE | 88 physical positions with aliases | missing; old combined caller was 62 complete keys |
| PREPARED ABORT | 17 | missing |
| STAGED ABORT | 35 | missing |
| Open | dynamic physical frame | old caller was 59 complete keys; post-checkpoint caller missing |

All five callers need exact v0 packet, signer, writable, unique-key, and
64-admitted/65-refused censuses. Physical position count is not Solana's lock
count; only the compiled de-duplicated complete-key list is.

#### Compute inference

The source-bound failed pre-split run at `2b0e6c29` observed:

| Component | Observed CU |
|---|---:|
| Custody Initialize | 355,209 |
| Custody OpenHoard | 98,172 |
| Custody OpenSource | 113,313 |
| Core ProjectFound nested in Resolution | 258,805 |
| Resolution total | 623,608 of 623,664 available |
| Trading outer | 1,399,494 of 1,399,550 available |
| Whole transaction | 1,400,000, failed |

The three Custody calls sum to 566,694 CU. PREPARE moves the entire 623,608-CU
Resolution call and Trading ledger creation out of STAGE. Checkpoint and live
ledger authentication add work back. The responsible estimates are:

- STAGE: approximately 750,000-850,000 CU; **550,000-650,000 CU removed** from
  the failed combined transaction.
- PREPARE: approximately 900,000-1,100,000 CU, using the focused 813,778-CU
  whole Resolution caller as the lower anchor before Trading-ledger and
  checkpoint creation.
- Open: expected to remain in the historical 1.2M-1.3M class plus checkpoint
  authentication/close. It must be measured, not inferred safe.
- Prepared and staged abort: unknown. The old Custody-only unwind was 159,496
  CU; both new routes additionally authenticate/close two ledgers and the
  checkpoint.

At the default Solana rent schedule, a 512-byte checkpoint requires
4,454,400 lamports (`(512 + 128) * 3,480 * 2`). The executor must use the live
Rent sysvar and report exact top-up/refund arithmetic. That principal returns
to the checkpoint's immutable RentCredit when Open or abort closes it.

#### Admission tests required before this seam is frozen

- PREPARE checkpoint-last whole-transaction rollback at each child failure.
- STAGE refuses Prepared substitution, ledger digest/mask/order substitution,
  wrong ProjectFound receipt, expiry, and all four Custody poststate changes.
- Open refuses Prepared, expired CustodyStaged, changed ladder bytes, advanced
  ledgers, and alternate funding-list order; Open closes the checkpoint last.
- Prepared abort has a fixed frame with no optional Custody accounts.
- Staged abort proves pre-expiry fee-only rollback; after expiry Custody abort
  precedes both ledger refunds and the checkpoint close.
- Crash recovery observes exact chain phase and polls the one durable signed
  packet. It never recreates ledgers, repeats principal movement, re-signs, or
  blind-resubmits an ambiguous packet.

If staged abort itself approaches the ceiling, the safe fallback is a new
`CustodyAborted` checkpoint phase followed by permissionless canonical ledger
closures. Do not split it pre-emptively: the current atomic abort is simpler and
likely far below the ceiling. Measure first.

### 3. Participant admission and collateral

The onchain admission is a 27-account Trading outer with one Claims CPI.
Trading authenticates the wallet signer and request-bound Trading authority;
Claims remains the sole writer of Position/admission state and returns the
typed receipt. Claims Position admission has an observed 236,191-CU baseline.

Collateral provisioning is already a separate Token-2022 transaction with its
own durable signed-packet journal. The accepted local fixture explicitly owns
100,000,000 atoms apart from founding principal; public/devnet profiles must
not invent that allocation. Direct consumes finalized participant evidence,
not the founding fixture directly.

**Decision:** do not split Claims admission. Finish one caller-backed private
run and add exact outer CU, packet, and balance evidence. Admission and
collateral may finalize independently because admission mints no trade
liability and Direct cannot start until both finalized receipts join.

### 4. Direct setup and Hot

Direct setup is already incremental and crash-safe:

```text
InitializeReplay -> create/extend/freeze/activate ALT -> capability seal
-> token setup -> Hot
```

The replay-setup and token-setup journals use
`Prepared -> Dispatching -> Submitted -> Finalized`; recovery may resend only
the identical already-signed packet in Dispatching and is poll-only after
Submitted. The action journal similarly freezes message bytes, packet digest,
signature, fee, poststate, and CU.

Hot cannot safely be split after a trade begins. It changes order/position,
Claims, Custody, and token facts whose partial visibility would strand one
party's asset or mint an unmatched liability. Setup and authentication can be
pre-staged, but the economic commit remains one transaction.

Historical work reduced a 2,949,172-CU implementation to a passing near-ceiling
path. The accepted pre-split M-61 baseline named above passed 20/20 with a
20-seed mean of 1,359,277 CU. Another accepted historical build passed 20/20
with a mean of 1,358,801 CU. These are different ELFs and must not be subtracted
as a code delta.

The remaining duplicate/variance sites are:

- request-bound child caller-authority PDA searches in
  `commit-lifecycle-closes` (historically 24,001 CU of cross-seed spread);
- root/product PDA searches (historically about 6,000 CU of spread);
- request/lifecycle preplan searches (historically about 4,500 CU);
- repeated immutable record and live resource authentication across setup and
  Hot. Cross-transaction repeats are necessary; repeated decoding/hashing
  inside one Hot invocation is not.

Concrete Hot optimizations, in order:

1. Carry every immutable record bump that has a semantic owner in that record,
   as already done for manifest/program-set/config. Expected removal:
   5,000-15,000 CU mean plus less variance.
2. Replace multiple ephemeral child caller derivations with one
   transaction-bound Hot invocation authority only if every child envelope
   still binds its own role, request digest, and batch digest. Expected removal:
   10,000-30,000 CU; adversarial confused-deputy tests are mandatory.
3. Audit `hot_v3` for a second decode/hash of bytes already represented by an
   authenticated borrowed view in the same invocation. Pass views, not a new
   persisted DTO. Expected removal is unknown and must be phase-profiled.

Every Hot optimization changes the Trading ELF and therefore requires a fresh
frame diagnostic and M-61 result stated as pass count plus 20-seed mean.

### 5. Provider/Pyth and Resolution

The real-provider route is already three transactions:

| Phase | Resolution frame | Durable fact |
|---|---:|---|
| Submit | 38 accounts | Receiver-owned update plus `ProviderUpdateLifecycleV3::Submitted` |
| Execute | 47 accounts from Core or 51 from Trading | Source state and terminal Resolution certificate |
| Reclaim | 18 accounts | update rent refund and closed lifecycle |

Submit parses the exact Pyth `PostUpdateParams`, invokes the pinned Receiver,
then authenticates update owner, authority, slot, publish time, rent, fee, and
treasury delta before writing the lifecycle. Execute consumes an already-posted
fully verified update; it does not submit or reclaim it. Reclaim requires the
terminal certificate and returns the exact update rent.

This is the right economic boundary. An update can exist without resolving a
Market, and a failed execution remains safely reclaimable after its terminal
condition. A paid offchain bearer API is not part of this onchain path.

#### Current V6 Core-wrapper ceiling

The existing `Core -> Resolution` lifecycle envelope does more than provider
transport. Core first authenticates the Market, activation cache and both
deployments, Source/manifest/recovery coordinates, funding prestate, and the
request-bound caller. Resolution then authenticates the same release, Market,
Source records, manifest, ledger, and action before mutating. After the CPI,
Core re-reads and hashes the live Source/ledger/certificate or closure receipt
before it accepts the acknowledgement and possibly updates Core readiness.

That belt-and-suspenders shape is correct for one atomic CPI, but it has no CU
room. In the current exact V6 ProgramTest run:

- `resolution_core_v3_lifecycle` passed **0/3** at the exact 1,400,000-CU
  ceiling;
- the activation child consumed about **1,180,908 CU**, after which the Core
  wrapper exhausted;
- the close child consumed the approximately **1,129,805 CU remaining** after
  its wrapper and the transaction exhausted;
- changing the V5 controller-release identity to V6 is a byte-sized constant
  change and does not explain a six-figure failure.

These are observed single-path results, not M-61 evidence. They establish that
optimizing a few hashes inside the wrapper is not an adequate architecture.

#### Incremental activation owner

Do not persist a second copy of FundingLedger or Source truth. Resolution's
existing accounts remain semantic owners; add only a typed, rent-bearing
transition receipt in `dclutch-resolution-codec`:

```text
Resolution ActivateFunding (direct, no nested Core wrapper)
  authenticate Core Market Founding+Prepaid or Open+Consumed
  authenticate release/manifest/Pending ledger/Source
  activate the three exact rows
  -> FundingActivationReceiptV1 LAST

Core AcceptFundingActivation (no Resolution CPI)
  authenticate receipt owner/PDA/digest and live Active ledger/Source
  if Founding+Prepaid: commit Readiness::Ready LAST
  if Open+Consumed: preserve consumed readiness and only admit the receipt
  -> close or mark receipt consumed LAST
```

The receipt binds release set, Market/generation, Source material/state,
manifest and selected mask, Pending prestate digest, Active poststate digest,
three entry indices/revisions, exact native principal/rent, Resolution release,
activation slot, and request digest. Resolution creates it only after the live
poststate exists. Core accepts it only if those exact bytes are still live.

A crash after activation is safe: the Market remains not-ready in the
readiness-ladder case, so it cannot Open; acceptance is permissionless and
idempotent. For an already Open atomic founding, provider execution must require
the exact activation receipt before it can spend the active rows. Upgrade
admission must refuse while an unconsumed activation receipt exists, or the
new release must explicitly preserve this recovery ABI. An expiry abort may
close/refund only while Core has not consumed the receipt and the Market has
not advanced.

Moving the 1,180,908-CU child into its own transaction removes roughly the
219,000 CU currently spent or reserved by the surrounding wrapper from that
transaction. The separate Core acceptance should be a low-hundreds-of-thousands
receipt/live-poststate check. Both estimates require exact measurement.

#### Incremental terminal close owner

`CloseFund` does not itself change Core from Retiring to Retired; it produces
one component that aggregate retirement later authenticates. Therefore the
heavy close does not need to be nested under Core merely to return an
acknowledgement.

First try one direct Resolution close authorized by the live Core
`Retiring + Consumed` state. Resolution already authenticates Core and
Resolution deployments, Market derivation, certificate, Source, manifest,
ledger, beneficiary, and release. The immutable beneficiary and typed
`SourceClosureReceiptV3` are enough for aggregate retirement to consume the
result. Removing the wrapper makes approximately 270,000 more CU available to
the child, but the failed child did not prove its own full cost, so standalone
fit is unknown.

If direct close still approaches the ceiling, add a transient
`ResolutionCloseCheckpointV1` owned only by Resolution:

```text
Prepared
  binds terminal Source/certificate, Active ledger, exact three-row close plan,
  beneficiary, refund arithmetic, release set, Market/generation
-> SourceClosed
  closes/retires Source to the immutable beneficiary, records exact digest/refund
-> LedgerProgress(mask, digest, remaining lamports)
  closes one or more canonical rows without changing the beneficiary
-> terminal SourceClosureReceiptV3 LAST; checkpoint closes LAST
```

The checkpoint is coordination state, not a second Source or FundingLedger
truth. Every step reauthenticates its prior checkpoint digest and live resource
digest. Recovery is permissionless, order is canonical, and no caller chooses
a refund. At this point Core is Retiring, the terminal certificate exists, and
no further resolution liability can be minted; a crash can strand work but not
redirect value. The upgrade gate must either prove no live close checkpoints
or preserve their recovery ABI.

This removes the Core wrapper entirely from the heavy close transaction
(approximately 270,000 CU of available headroom) and, if row closure is paged,
keeps each ledger mutation bounded. Total lifecycle CU may increase; the goal
is safe per-transaction reachability, not pretending the work vanished.

The likely duplicate is immutable Source/Product/provider graph
authentication in Submit and Execute. It crosses a transaction boundary, so
Execute cannot trust the exterior journal. A future
`VerifiedProviderObservationV1` may be useful only if Resolution owns it
onchain and binds update digest, release set, Market/generation, Source graph,
Product graph, provider release/config, expiry, refund identity, and live Source
prestate. Expected Execute reduction: 75,000-200,000 CU, low confidence. It
adds one rent-bearing account and one cleanup path. Do not add it until exact
Submit/Execute/Reclaim CU shows a real ceiling problem.

### 6. Relayed observations and reconciliation

Resolution's relayed family is already incremental:

```text
CreateRecord -> AppendObservation* -> SealRecord
             -> ConsumeRecord or CommitDeadlineFailure -> RetireRecord
```

Create binds the Market, provider release, relayer key set, account set,
observed slot, and rent beneficiary. Append authenticates one adjacent native
signature and folds the observation. Seal records an m-of-n bitmap. Consume is
the only path that writes Source/certificate state; it reauthenticates the
28-account Market, Source, provider, Product, record, and account-set graph.
Deadline failure is a distinct terminal transition. Retire returns record rent.

Append and Seal must authenticate their own signer and record because they are
independent transactions. The best candidate if Consume becomes expensive is
a Resolution-owned `VerifiedRelayedObservationV1` created after Seal, followed
by a smaller Source consume. It must bind the exact sealed record digest,
signer bitmap/threshold, decoding rules, all account-set entries, Source
prestate, Market/generation, expiry, and refund. Estimated Consume reduction:
100,000-300,000 CU, low confidence. Crash before consume leaves no liability;
expiry closes only the verification account and never the sealed record's rent.

Exterior reconciliation is read-only. It verifies finalized signatures,
packets, fees, CU, return data, account history, and closure facts. It adds no
onchain CU and must not be merged with the relay's semantic state owner.

### 7. Resolution, payout, redemption, and retirement

Resolution exterior is Submit -> Execute -> Reclaim. Wallet payouts are one
transaction per Position. That is good scaling: paying payout is about 354k CU
and the zero branch about 223k, so batching would create a ceiling and couple
unrelated wallets without buying atomic safety.

Terminal exterior already orders six protocol transactions:

```text
CoreBeginRetiring
-> DirectBeginRetiring
-> ResolutionCloseFund
-> DirectCloseCapability
-> RetirementReplayHandoff
-> AggregateRetirement
```

Every stage has `Planned -> SignedNotSubmitted -> Submitted -> Finalized`
journal semantics. Resolution rent prepayment is an operational prerequisite,
not a seventh protocol mutation.

The final AggregateRetirement remains monolithic. It has a 46-account closure,
authenticates Registry continuation/deployments, invokes Claims market closure,
Custody Hoard-vault close, and Custody replay close, then closes Core Market and
RentCredit last. There is no accepted whole-transaction CU measurement.

This is the second strong checkpoint candidate because the prerequisites
already prove zero Claims liabilities, zero outstanding capabilities, closed
Resolution funding, and terminal Source evidence. At that point incremental
closure cannot mint or strand a user liability.

Proposed Core-owned `MarketRetirementCheckpointV1`:

```text
Prepared
  -> ClaimsClosed(receipt digest, refund, revision)
  -> HoardVaultClosed(receipt digest, refund, replay revision)
  -> CustodyReplayClosed(receipt digest, refund, revision)
  -> Core/Rent close LAST with one terminal RetirementReceipt
```

The checkpoint binds the existing retirement bundle digest, release set,
Market/generation, Source closure receipt, exact Claims/Custody prestates,
RentCredit, canonical refund arithmetic, and every expected post revision.
Recovery is permissionless and only advances the next exact closure. There is
no expiry refund to an arbitrary caller; every lamport destination is already
immutable. Estimated reduction from the current final transaction is
300,000-500,000 CU, while each child closure should remain in the existing
roughly 140,000-250,000-CU class. Measure the monolith first; implement this
only if its headroom or account geometry warrants it.

## Duplicate work inventory

| Site | Duplicate or monolith | Safe treatment |
|---|---|---|
| Registry activation | Whole ELF hash per role | Required once on first admission; already split by role. Never cache an unverified claimed digest. |
| PREPARE/STAGE/Open | Manifest, Found/Lock, ledger, and checkpoint hashes recur across transactions | Required cross-transaction authentication. Reuse one borrowed decode within a transaction; do not introduce a second DTO. |
| Old combined DCLTPCB2 | Resolution/Core pre-market creation plus three Custody stages plus Trading ledger creation | Remove from STAGE through `DCLTCFP1`; this is the accepted P0 split. |
| Participant outer/Claims | Request digest and receipt authentication on both sides of CPI | Required caller/callee boundary; small. |
| Direct setup and Hot | Market/root/record authentication repeats across transactions | Required across setup transactions; within Hot, pass authenticated views and stored canonical bumps. |
| Resolution Core lifecycle | Core preauth -> Resolution repeats release/Market/Source/manifest/ledger auth -> Core postauth | Direct Resolution activation plus typed receipt/Core accept; direct or checkpointed terminal close. Current exact V6 suite is 0/3 at 1.4M. |
| Provider Submit/Execute | Source/Product/provider graph repeats | Transport split is sound. Add a verified-observation checkpoint only if exact Submit/Execute CU still warrants it after the lifecycle split. |
| Relay Append/Seal/Consume | Key-set/record graph repeats | Required per signer mutation; optional verified sealed-observation checkpoint before Consume. |
| Aggregate retirement | Three child closes plus final Core/Rent close in one 46-account transaction | Measure, then split through a Core-owned terminal checkpoint if needed. |

## Prioritized convergence plan

### P0 — finish the already-selected controller-funding split

1. Update the successor/operator caller to emit PREPARE, STAGE, Open,
   PreparedAbort, and StagedAbort as distinct exact packets.
2. Give each mutation its own durable signed-packet journal and exact
   finalized-poststate recovery. The chain checkpoint, not the local file,
   decides which mutation is next.
3. Replace old 62/59 assertions with route-specific compiled complete-key
   censuses; preserve devnet's `<=64` gate and prove 65 refuses.
4. Run focused hostile tests, all shipped-link frame diagnostics, then one
   source-bound private-validator seed through Founding and participant.
5. Only after the caller and ELF freeze, run M-61 and report pass count plus
   20-seed mean.

Exit condition: PREPARE, STAGE, and Open finalize on a fresh validator; both
abort shapes pass their exact expiry/rollback probes; participant admission and
collateral finalize; no transaction approaches the 1.4M ceiling without a
named follow-up.

### P0 — split the measured V6 Resolution lifecycle ceiling

1. Introduce the Resolution-owned `FundingActivationReceiptV1`; execute the
   heavy activation directly and make Core accept the typed receipt/live
   poststate without a child CPI.
2. Route CloseFund directly from authenticated Retiring state to the existing
   `SourceClosureReceiptV3`. If it remains near the ceiling, page it through the
   transient `ResolutionCloseCheckpointV1` and canonical ledger progress.
3. Preserve one semantic owner for Source, FundingLedger, and terminal closure;
   checkpoint fields are digests/coordinates/progress, never copied state.
4. Prove crash recovery, exact immutable refund arithmetic, late substitution
   rollback, unconsumed-receipt upgrade refusal, and terminal aggregate receipt
   consumption.
5. Rerun the exact three-test ProgramTest suite and record whole/child CU for
   every new phase before the broad gate.

Exit condition: the V6 lifecycle is 3/3 under the chain ceiling, activation can
resume between Resolution commit and Core acceptance, close can resume at every
persisted phase, and aggregate retirement consumes exactly one canonical
closure receipt.

### P1 — recover stable Hot headroom without splitting the trade

Profile the post-split Trading ELF by phase, carry canonical immutable bumps,
reduce ephemeral caller derivations with exact role/request/batch binding, and
remove only same-invocation duplicate decode/hash work. Target at least 30,000
CU of 20-seed mean headroom beyond the accepted baseline, but acceptance is the
measured M-61 result rather than that target.

### P1 — measure final AggregateRetirement

Record whole and child CU, account locks, packet width, and rent/refund
arithmetic. If it is over roughly 1.1M or has less than 200k deterministic
headroom, implement the Core-owned retirement checkpoint above. Otherwise keep
the simpler atomic close.

### P2 — provider/relay compute campaigns

Add exact rows for provider Submit/Execute/Reclaim and relay
Create/Append/Seal/Consume/DeadlineFailure/Retire. Only then decide whether a
verified-observation checkpoint earns its extra account, rent, and cleanup
surface.

### P3 — measurement coverage, not new architecture

Add source-bound CU/packet/lock evidence for participant admission, collateral,
each payout branch, all six terminal stages, publication pages, Rent lifecycle,
and role activation. These paths are already structurally incremental.

## Required final evidence matrix

For every mutating route, the convergence dossier should carry:

- exact source commit and every changed ELF digest;
- frame diagnostic over every shipped link, not build exit status;
- physical metas, compiled unique complete keys, signer/writable union, ALT
  membership, message bytes, and signed packet bytes;
- observed whole-transaction and child CU;
- for M-61 routes only, pass count and 20-seed arithmetic mean;
- every created account's width, live Rent minimum, payer debit, fee, refund,
  and final lamport destination;
- semantic-owner state/receipt digest and exact phase transition;
- crash before send, lost send response, Submitted restart, finalized restart,
  expiry, and hostile substitution outcomes;
- explicit proof that no failed/partial transition stranded a liability,
  principal, rent, or token account.

The architectural milestone is not merely a green build. It is a caller-backed
private-validator lifecycle in which every multi-transaction boundary has an
onchain semantic owner, every atomic liability transition stays atomic, every
failure has a deterministic compensation path, and the current Trading ELF has
fresh frame and M-61 evidence.
