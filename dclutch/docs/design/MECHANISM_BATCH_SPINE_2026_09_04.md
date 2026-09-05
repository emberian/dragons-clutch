# The frequent batch as the clearing spine of every family

Design lane BATCH-SPINE, 2026-09-04. Tree `/Users/ember/dev/dclutch`, read at
`7741969c0` through `b019d2450`; the pricing harness ran in a detached scratch
worktree at `4467e1f6d` with its own target directory and is not committed. No
program changed. Every path:line is HEAD's. The companion notes this composes
with — JOINT-CLEARING (the clearing rule and the price selection among
max-volume clearings) and SCORING-DEALER (the Dealer as a bounded-loss
schedule) — had not landed when this was written; every place this note needs
one of them says so by name under *owed to*.

**The unification, in one sentence.** Every transfer of claims between two
parties in dClutch is one General candidate — an exact simplex price vector
over the Market's K outcomes and a set of executions of limit orders, balanced
by exactly one complete-set move — verified by `Candidate.valid`
(`formal/dclutch-semantics/DClutchSemantics/GeneralClearing.lean:301-308`),
chosen as the best valid submitted candidate, and settled through Custody
compartments under the journey's laws L1–L8. The five families differ only in
**how a candidate is produced and how wide its collection window is**:

| family | what it is under the spine | window |
|---|---|---|
| General | the frequent batch: an open solver set competes to clear a sealed order set | `collectionSlots` per Market policy |
| Direct | a batch of two whose collection is one transaction, both parties signed — the RFQ | one transaction |
| Dealer | a participant that places one demand schedule per batch; it is why every batch clears | the batch's |
| Series | a schedule of Markets, each carrying a General capability whose windows are the occurrence's | one batch per occurrence |
| Structured | the K of the joint clearing; Issue/Unwrap are the one batch-of-one that is self-clearing, through the complete-set move | one transaction |

This is not an import. The twelve-item ceiling that ember pasted back with
approval names exactly these four venues — *"frequent batch auctions; RFQs;
schedule-compiled passive liquidity; a formally admitted convex cost-function
maker"* (`docs/INTENT.md` §4, ledger M-3 item 6) — and the thesis sentence
says *"clears bounded portfolio programs through interchangeable verified
venues"* (§2). The batch relation is *"small and specialized on purpose"* as a
future FHE target (§1). The spine is the tree's own aspiration read back to it.

And most of it already exists. `GeneralClearing.lean` is already a
uniform-price joint clearing with complete-set minting inside the batch:
`Candidate.prices` is one `PriceVector` per candidate (`:128-135`), every
execution is priced from it by one named rounding boundary (`roundedQuoteFor`,
`:206-219`), and `Materialize` performs *the only aggregate liability change a
fully collateralized categorical Market can admit* — none, mint q, or merge q
(`:263-274`). Decision 0006 makes a family a `CapabilityProgramSet` selected by
proved derivation, never a branch in the executor. So "Direct becomes a batch"
is a program-set change and a Lean conjunct, not a dispatch change. What this
note adds is the honest list of what is missing for the spine to be *frequent*
and *total*, what each family gives up, and the price.

## 1. The clearing, exactly

### 1.1 Cadence: per batch, set by the Market's policy triple

The cadence is the triple `(collectionSlots, selectionSlots, settlementSlots)`
in `GeneralConfigV3` (`GeneralConfigV3Abi.lean:32-43`), immutable per
capability at founding and — after decision 0024's amendment — governable
through the named parameter surface. `OpenBatch` derives the windows from it
and refuses a caller-chosen one:

```text
collection_close = now + collectionSlots
settlement_close = collection_close + selectionSlots + settlementSlots
```

(`GeneralTransitionV3.lean:651-653`, `checkedAddInto`, overflow refused). The
selection window is the interval `[collection_close, collection_close +
selectionSlots)` and is **not persisted** on the batch record, which carries
only `collection_close_slot` and `settlement_close_slot`
(`crates/dclutch-general-adapter-contract/src/collection_v1.rs:365-367`). §2(d)
returns to that.

**Not per slot, and not per N slots.** A batch is `9 + 4M` Hot transactions
for `M` filled orders, of which `10 + 3M` are cursor-dependent and land
strictly after their predecessor (§4). At one slot per dependent step — the
optimistic floor — a batch of eight is 34 slots after its window closes; the
CU ceiling never binds the cadence, the dependency chain does. So the rule is:

- **General**: `collectionSlots` is the product's frequency and is free; the
  floors are `selectionSlots ≥ R + 3` and `settlementSlots ≥ 2M + 4` where `R`
  is the execution-row count and `M` the admitted-order bound (`max_orders`,
  `collection_v1.rs:369`), each with a margin for confirmation latency. A
  spot-band market at `max_orders = 32` with `150 / 72 / 136` slots has a
  60-second collection and a 143-second open-to-terminal latency.
- **Series**: one batch per occurrence. `OpenBatch` is permissionless and
  runs at `scheduledSlot` (`SeriesOccurrenceV3.lean:46-47`), `collectionSlots`
  is the occurrence's window, and the occurrence's capability manifest names a
  General capability rather than a Series-specific evaluator (§3.1, the
  series-shadow rows).
- **Direct (RFQ)**: the window is the transaction. Both tickets are signed
  before it is built; there is nothing to collect.

**Frequent needs one more thing the tree does not have.** The selection
state is seeded by the General root alone —
`GENERAL_SELECTION_STATE_RECIPE_V3 = [domain, root, "selection", bump]`
(`state_seeds_v3.rs:211-216`, *"One per General root"*) — its phase is written
`Open` only at creation (`runtime_selection.rs:343-349`), and no action in the
fifteen writes it `Open` again. After the first `Freeze` a root's selection is
`Frozen` forever, and `consider` refuses a second batch's candidate by
`batch_id` (`:363`). **General as built is one call auction per Market**, not
a frequent batch. The repair is one seed: key the selection by the batch
identity register, exactly as the settlement cursor is keyed by the candidate
(`:222-228`, *"two candidates under one root can never settle into the same
account"*). That also makes "one clearing per batch" a property of an address
(§2, hostile 2) and lets batch `b+1` collect while `b` settles, which is what
makes the steady-state frequency `1 / collectionSlots` rather than the whole
triple.

### 1.2 Admission and its window

An order is admitted only by moving its exact worst case into escrow
(decision 0010 §2; `collection_v1.rs:750-780`), and `PlaceOrder` carries these
conjuncts, each a refusal by name:

| conjunct | where | refusal |
|---|---|---|
| `current_slot < collection_close_slot` | `collection_v1.rs:819`; `GeneralTransitionV3.lean:806` | `OutsideWindow` |
| `admitted_slot == current_slot`, `released_slot == 0` | `:828` | `Substitution` — a replayed encoding cannot re-enter a later window |
| `batch_id`, `market`, `generation`, `outcome_count` equal the batch's | `:831-834` | `Substitution` |
| `valid_until_slot == settlement_close_slot` | `:841`; Lean `:815` (`scalarEq`) | `Expired` — *an order that expires before settlement closes is a promise the batch cannot keep* |
| `order_count < max_orders` | `:844` | `BatchFull` |
| escrow covers `quote_reserve` and every `claim_reserve(i)` | `:763-776` | `Unfunded` |

The fourth row is a fact the rest of this note leans on: **an order lives
exactly one batch.** Its validity is pinned to the batch's settlement close,
so "good till cancelled" across batches does not exist in General; a maker who
wants to rest across batches re-places, and cancellation is `CancelOrder`
while collecting (`:893-902`). `CloseBatch` is permissionless once the window
has passed or the batch is full (`close_is_permissionless`, `:1003-1005`);
`SubmitCandidate` is admitted in `[collection_close, settlement_close)`
(Lean `:720-721`); `VerifyCandidateRow` and the consideration are
permissionless cranks paid from the solver's own escrow (0010 §1).

### 1.3 The uniform price per outcome

A candidate carries one `PriceVector` — `coordinates.sum = scale`,
`PriceVector.validFor` (`GeneralClearing.lean:133-135`) — and every execution
of every order is priced from it: `quotesCanonical` requires the order's
candidate-wide debit and credit to equal `roundedQuoteFor prices order lots`
(`:237-243`). Debits round up, credits round down, so *per-fill rounding
cannot spend collateral which was not collected* (`:206-210`). The selection
objective is lexicographic — maximize filled lots, minimize quote surplus,
minimize candidate id (`:326-360`) — and the surplus under one uniform price
vector is rounding dust plus whatever the price leaves on the table, which is
why "minimize surplus" is the right second key.

**Two things the kernel does not yet state, and the spine needs both.**

1. **The seller has no reservation price.** `Order` bounds the debit side only
   — `maxQuoteDebitPerLot` (`:150-158`) — and `AuthenticatedOrderTermsV2` is
   `max_lots` and `max_quote_debit_per_lot`
   (`crates/dclutch-general-adapter-contract/src/runtime_verify.rs:220-231`);
   nothing in Rust or Lean names a credit floor. A pure sell — `deliverPerLot
   = e_i`, `receivePerLot = 0` — is credit-rounded at whatever price the
   candidate names, so a seller with a limit of 40 clears at 10 and the
   verifier admits it. A Direct sell ticket's `limitPrice` has nowhere to go.
   **Owed to JOINT-CLEARING**: `minQuoteCreditPerLot` on `Order` and its
   conjunct in `quotesCanonical`, and the same field on the order record
   (`192 + 16N` → `200 + 16N`).
2. **The price among max-volume clearings is the solver's.** The objective
   fixes the volume and prefers a small surplus, but the set of price vectors
   that achieve both is an interval per outcome, and the winning solver picks
   inside it. The property the spine needs from the clearing rule is that the
   price be a function of the order set — *solver-independent* — so that the
   series is a fact about the book and not about who solved. **Owed to
   JOINT-CLEARING**: the canonical rule. The composition point is one more
   `SelectionCriterion` in the interpreter (`:324-333`, *"new reviewed
   criteria extend this interpreter"*), placed after `minimizeQuoteSurplus`
   and before `minimizeCandidateId`; this note's recommendation, if the
   clearing note has no stronger reason, is minimal L1 distance from the
   previous batch's price vector, which makes the series continuous where the
   book is indifferent.

### 1.4 Settlement as complete-set claims

Settlement is the streamed machine of `GeneralClearing.lean:507-640`, executed
one Hot action per step: `Collect` moves each order's escrow into the
candidate's settlement inventory (`Settlement(order_id) → Settlement(candidate_id)`,
0010 §2), `Materialize` performs the single mint or merge against the Hoard,
`Distribute` pays each order from inventory, `Close` routes the exact quote
remainder to the configured beneficiary and requires every inventory slot
zero (`:507-520`, `:564-567`; `close_routes_all_quote`, `:631`). L3 (supply-vector agreement) and
L4 (full collateralisation) are re-read from chain at every stage boundary
(`tools/gauntlet/journey/src/ledger.rs:25-36`), so a settlement that minted
without collecting or paid without inventory is a red law, not a silent one.

### 1.5 What a Direct ticket becomes, and what is lost

A `CompactIntentV2` (`DirectIntentV2Codec.lean:33-52`) maps onto `Order`
field by field:

| ticket field | order field | note |
|---|---|---|
| `side`, `outcome` | `receivePerLot = e_i` (buy) or `deliverPerLot = e_i` (sell) | a single-outcome portfolio order |
| `maximumFill` | `maxLots` | exact |
| `limitPrice` (buy) | `maxQuoteDebitPerLot` | exact |
| `limitPrice` (sell) | **no field** | §1.3 item 1 — owed |
| `validFrom`, `validThrough` | admission slot; `valid_until_slot == settlement_close` | the batch's window replaces the ticket's; a ticket valid past the batch is one batch's order |
| `lifecycle = IOC` | by construction | one order, one batch |
| `lifecycle = FOK` | **no field** | `executionValid` admits partial lots (`:222-224`); an `allOrNone` bit is owed — the Lean already states the predicate once for Direct (`Direct.lean:145-149`) |
| `lifecycle = GTC` | re-place per batch, or the registered record | §3.1 |
| `feeBasisPoints` | the Market's policy rate | a batch has one fee policy; the ticket's own rate was a Direct-only degree of freedom |
| `nonce`, `generation`, `market`, `collateralAccount` | order PDA seeds `(market, generation, owner, nonce, order_id)` | replay is address occupancy (0009 §4) |
| a named taker | **never existed** | nothing lost; a `counterparty` field is what makes the RFQ honest (below) |

**What the batch honors exactly**: the limit, the size, the outcome, the
replay guard, and — with the two owed fields — the seller's floor and
all-or-none. **What it cannot honor**: *immediacy*. A taker who sees a
resting ticket at 40 and wants it now waits for the window and takes the
batch price — never worse than their own limit, but not now, and not
necessarily 40. Under Budish–Cramton–Shim that is the point: inside a batch
there is no value to being first. But the tree's batch is tens of seconds
(§1.1), not BCS's hundred milliseconds, and a trader whose information decays
in seconds is genuinely worse off.

**The disposition, decided by reading.** The inline Direct fill today is two
signed tickets and an *unsigned* matcher request that picks `executionPrice`
anywhere in `[sellerLimit, buyerLimit]` (`DirectOrdinaryV3.lean:522-524`;
`DirectRegisteredFillV4.lean:15-17`, *"the matcher request is deliberately
unsigned"*). Read as a venue, that is two different products wearing one
route:

- **A public pool of resting bearer tickets that anyone may match** is a
  limit order book with a permissionless matcher, and the race to be that
  matcher is the latency race the spine removes. `docs/INTENT.md` §4 refuses
  the order book, the web app already refuses to render one, and the registered
  Direct branch — GTC records on chain, matched by an unsigned request,
  refusing everything but its two creations on every chain
  (`programs/dclutch-trading-sbf/src/hot_v3.rs:5880-5907`;
  `docs/evidence/C16_REHEARSAL_2026_09_04.md` §8 item 17) — is that book in
  embryo. **Deleted.** Its makers are General orders.
- **Two parties who have already agreed** — an RFQ, an OTC cross, a founder
  seeding a stranger — need no window and no third party. **Direct survives as
  the RFQ venue: a batch of two.** Two amendments make it a batch rather than
  a bilateral match: the price is *derived*, not the matcher's — replace the
  two `scalarLe` conjuncts with `executionPrice = (sellerLimit + buyerLimit) /
  2` (one instruction in `DirectOrdinaryV3.lean`, the equal split; a fill at an
  agreed price has `sellerLimit = buyerLimit` and is unchanged) — and the
  ticket gains an optional `counterparty` (`CompactIntentV3`, 140 → 172 bytes,
  preimage 172 → 204), so a maker who wants a specific taker can say so and a
  bearer ticket stays bearer. A batch of two has a uniform price by arithmetic
  and no speed advantage because it has no third party.

What Direct keeps that General cannot give: an off-chain resting instrument
that costs nothing until it fills. A General order is a rent-bearing record,
an escrow vault and an escrow Position, all created by the maker's own `Place`
transaction before any fill (0010 §2, *"the cost is named"*). Carrying signed
tickets into a candidate instead — `AdapterBoundary.orderSignaturesAuthenticated`
(`GeneralClearing.lean:69`) is the named hook — would put the maker's
collateral back where the maker can spend it between placement and `Collect`,
which is the credit regression 0010 closed. Refused for that reason, and the
refusal is the same one twice.

## 2. Properties, with proof sketches

**(a) No speed advantage inside a batch.** The batch never enumerates its
orders (*"a batch is a window, not a ledger"*, 0009 §4); the record it mutates
on admission is a count and a sum (`order_count`, `committed_quote_reserve`),
both permutation-invariant over the admitted set; each order is its own PDA
keyed by content; the candidate is a set of executions over those records;
the objective is a function of the candidate; settlement order is manifest
order over identities (`le_numeric_id`, 0009 §5), not admission order. Sketch:
for any two admission sequences that are permutations of one another inside
the window, the batch state after `CloseBatch` is identical, the reachable
candidate set is identical, and therefore the selected candidate and every
settlement transfer are identical. A leader who reorders `Place` transactions
within its four slots changes nothing; one who delays a `Place` to the next
leader changes nothing unless it crosses the boundary, which is (d)(ii).

**(b) Uniform price per outcome per batch.** By type: one `PriceVector` per
`Candidate`; every execution priced from it (`quotesCanonical`); one
candidate per settlement cursor (`GENERAL_SETTLEMENT_STATE_RECIPE_V3` keyed by
candidate); `InitializeSettlement` requires the frozen selection's best
candidate (`require_certificate`, `plan.rs:453`); one frozen selection per
batch once §1.1's seed lands. Two executions of one batch at two prices would
need two candidates settling under one batch, which is two settlement cursors
naming one frozen best — refused at the seed.

**(c) The price series is the forecast.** The sequence `p_b ∈ Δ^K`, one
simplex per settled batch, is what an AI trader reads: a probability vector
time series whose every point is a market-clearing price of fully
collateralized limit orders under exactly one complete-set move, and — with
the scoring dealer in every batch — always defined and always bounded by the
dealer's schedule depth. What makes it a *chain fact* rather than an indexer's
opinion: today the vector lives only in the candidate account
(`runtime_verify.rs:348`, `price(index)` on the candidate tail), and
`CloseCandidate` reclaims that account to the solver (0010 §1). **Owed**: the
frozen price vector persisted at `Close` on the batch record or the terminal
record, so a reader with the batch records alone has the series. Until then
the series is readable only while candidates stay open, which is a property
of a solver's patience.

**(d) What remains manipulable, and its bound.**

- *(i) Early freeze — the batch-boundary manipulation, and it is live today.*
  `Freeze`'s prelude is phase, a nonzero best and a nonzero verified revision
  (`GeneralTransitionV3.lean:585-592`); `freeze_selection` takes a revision and
  no slot (`plan.rs:414-430`). A solver who submits a thin valid candidate,
  cranks its consideration, and freezes in the same slot excludes every
  fuller candidate that would have beaten it — `closed_selection_is_immutable`
  (`GeneralClearing.lean:432-436`) then protects the wrong thing. Bound: one
  conjunct, `scalarLe (collection_close + configSelectionSlots) currentSlot`,
  from registers `Freeze` already has (`.batchCollectionCloseSlot`,
  `.configSelectionSlots`); the hostile is a freeze one slot before the
  selection window ends, proved red against HEAD before it is green.
- *(ii) Last-slot placement.* Orders are on-chain records and the window is
  long, so a trader reads the book and places in the last slot. In a
  uniform-price auction the only gain from that is demand reduction — shading
  inframarginal units — and the classical non-strategy-proofness of the
  uniform-price format. It is bounded by the price impact the scoring dealer
  sets: with the dealer's schedule of liquidity `b` in every batch, one order
  of size `q` moves the outcome price by at most `q / b` in the LMSR's
  worst region (**owed to SCORING-DEALER**: the exact slope at its chosen
  discretization). The window's length is a product choice against exactly
  this: a shorter collection shortens the read.
- *(iii) Size in a thin batch.* A large order in an otherwise empty batch
  clears at its own limit. The dealer is always in the batch, so no batch is
  thinner than the dealer's schedule; the bound is the same `q / b`.
- *(iv) Solver discretion.* Until JOINT-CLEARING's price rule lands, the
  price among max-volume clearings is chosen by whichever valid candidate wins
  the `candidateId` tie, which is a hash lottery over solver-chosen prices.
  Bounded by §1.3 item 2; not by anything today.
- *(v) Leader censorship at the boundary.* A leader can drop a `Place` in its
  own slots. Bound: a window of at least eight slots spans two leaders, and
  the order lands with the next one before the close.

**The three hostiles.**

1. *An order admitted after the batch closed.* Refused `OutsideWindow` at
   `collection_v1.rs:819` and by the Lean conjunct `:806`; the slot it is
   compared against is the trusted-environment register Trading seeds from
   `Clock` and refuses on disagreement
   (`docs/design/GENERAL_INPUT_TRANSPORT_2026_09_02.md`, *"a page is valid
   for exactly one slot"*, measured `0x4018` at 501,968 CU). What is owed is
   the SVM hostile itself with the discriminant named: the exact
   `GeneralCollectionErrorV1::OutsideWindow` code, one slot late, red-then-green.
2. *Two clearings of one batch.* Two candidates settling under one batch need
   two settlement cursors naming one frozen best; the cursor is keyed by
   candidate and `InitializeSettlement` requires the frozen best. A second
   `InitializeSettlement` for the same candidate finds its PDA occupied. What
   is NOT refused today is the case the note cares about most: a second batch
   under the same root, whose selection is the same account — it is refused
   by `batch_id` mismatch, which is correct and makes the second batch
   unclearable forever. The hostile after §1.1's seed change: two batches
   under one root, each frozen, each settled, neither able to name the other's
   candidate.
3. *A settlement that does not match the clearing.* Every `Collect` and
   `Distribute` row is authenticated against the certificate's manifest
   (`SettlementManifestV2`, `runtime_verify.rs:825`) and against the order's
   own escrow (`authenticate_collect_from_escrow_v1`,
   `tools/gauntlet/general/bindings.json` row 4); `Materialize`'s direction and
   quantity come from the authenticated complete-set move, the one artifact
   patched at runtime (0010 §2a item 1); L3 and L4 re-read the chain at every
   boundary. Under the output page the census law gets stronger: *page bytes
   == digest preimage* alongside every runtime observation (decision 0028 §3
   condition 3).

## 3. The migration

### 3.1 Every route, with its disposition

`docs/reference/routes.md` renders **178 rows for 163 census routes**; the
fifteen extra rows are witness sub-rows (the six Custody rows bound from
`claims-custody/custody-bindings.json`, the two from `dealer-checkpoint`, the
two `hot_v3` crosscheck sub-rows, and five action rows the census folds). All
178 are tabled so nothing is skipped. Dispositions: **SURVIVE** (untouched by
the spine), **SURVIVE†** (survives amended), **PARTICIPANT** (becomes a batch
participant's route), **DELETE**.

| count | disposition |
|---:|---|
| 156 | SURVIVE |
| 6 | SURVIVE† |
| 3 | PARTICIPANT |
| 13 | DELETE |

The reason for the 156 is one sentence, stated once: founding, custody,
Positions, resolution, retirement and Registry are the *settlement and
resolution* layers, and the spine changes only *clearing*. Their rows say
which layer.

**claims** (33)

| route | disposition | reason |
|---|---|---|
| `claims/affine_batch_v2::process` | SURVIVE | the Claims move General's settlement rows already use |
| `claims/claim_check_compaction_v1::process_compaction` | SURVIVE | redemption |
| `claims/claim_check_compaction_v1::process_open_escrow` | SURVIVE | redemption |
| `claims/claim_check_redemption_v1::process_escrow_close#CloseEscrow` | SURVIVE | redemption |
| `claims/claim_check_redemption_v1::process_redemption#else` | SURVIVE | redemption |
| `claims/custody_replay_v1::process` | SURVIVE | terminal custody replay |
| `claims/founding_v5::process` | SURVIVE | founding mints the complete set; unchanged |
| `claims/fractional_atomic_v3::process` | SURVIVE | Structured/fractional custody; Wrap/Unwrap are self-clearing batches of one and need no counterparty |
| `claims/fractional_claim_check_v1::process_fractional_compaction` | SURVIVE | redemption |
| `claims/fractional_claim_check_v1::process_fractional_redemption` | SURVIVE | redemption |
| `claims/fractional_retirement_v3::process` | SURVIVE | retirement |
| `claims/market_closure_v1::process_checkpoint_handoff` | SURVIVE | retirement |
| `claims/market_closure_v1::process` | SURVIVE | retirement |
| `claims/process_begin#Begin` | SURVIVE | retirement |
| `claims/process_coordinate#RetireCoordinate` | SURVIVE | retirement |
| `claims/process_core_effect` and its five `#…` arms | SURVIVE | Core effect at founding/redemption; six rows |
| `claims/process_finish#Finish` | SURVIVE | retirement |
| `claims/process_instruction` | SURVIVE | entry |
| `claims/process_open#WholeUnwrap` | SURVIVE | fractional custody |
| `claims/process_terminal#TerminalZeroBurn` | SURVIVE | fractional terminal |
| `claims/protocol_position_v2::process` | SURVIVE | Positions |
| `claims/rational_lifecycle_v2::process` | SURVIVE | Structured lifecycle |
| `claims/rational_representation_v2::process` | SURVIVE | IssueStructured/UnwrapStructured: the self-clearing batch of one (§0); K ≤ 3 packet-bounded per 0029 item 7 |
| `claims/rational_representation_v2::process_replay_close` | SURVIVE | Structured |
| `claims/series_founding_transport_v1::process` | SURVIVE | Series founding of an occurrence's Market |
| `claims/signed_delta_v3::process` | SURVIVE | the Dealer LP Remove's Claims suffix; LP lifecycle survives |
| `claims/sparse_native_transfer_v1::process` | SURVIVE | Positions |
| `claims/terminal_settlement_v3::process` | SURVIVE | resolution payout |

**core** (36) — every row **SURVIVE**: `activate_capability_child`,
`authenticate_no_recovery_entries`, `begin_retiring`, `capability::process`
(both arms), `close_capability_child`, `commit_checkpoint` (both magics),
`execute_provider_v3`, `finish_checkpoint_retirement`, `found::process`,
`found::project`, `generic_founding_v1`, `infrastructure::process_initialize`,
`infrastructure_v2`, `open_market`, `process_found#FoundAndPermit`,
`process_instruction` and its `#CloseCapability` / `#Retire` / `#else` arms,
`process_open#Open`, `resolution::*` (seven rows), `retire_v1::*` (three rows),
`retirement_replay_handoff_v1`, `series_consume`, `series_open`,
`series_permit_expiry`, `series_permit_expiry_precommit_v1`. Reason: founding,
resolution admission, retirement and the Series ticket machine; an occurrence
still founds a Market through `series_consume` exactly as today — what changes
is which capability that Market's manifest names.

**custody** (27)

| route | disposition | reason |
|---|---|---|
| `custody/dealer_reservation_v1::process` | DELETE | the Dealer scenario reservation; the dealer's schedule order escrows at admission like every order |
| `custody/reserve#Reserve` | DELETE | same chain |
| `custody/rollback#Rollback` | DELETE | same chain |
| every other custody row (24) | SURVIVE | compartments, projected custody, vault open/transfer/close, replay, retirement handoff — the settlement layer |

**dealer-accelerator** (3)

| route | disposition | reason |
|---|---|---|
| `dealer-accelerator/process_instruction` | PARTICIPANT | survives as the evaluator of the LP lifecycle (Open/Add/Remove/Close, 31/31 on real ELFs) and of the dealer's schedule bank; selector 9 (the scenario trade) is deleted with `DealerScenarioTradeV4Abi.lean` |
| `dealer-accelerator/set_return_data#ChunkedBankV2` | PARTICIPANT | transport |
| `dealer-accelerator/set_return_data#OutputPageV3` | PARTICIPANT | transport; the equity Remove's 3-chunk bank is the strongest single argument for the page |

**direct-aot** (1)

| route | disposition | reason |
|---|---|---|
| `direct-aot/process_instruction` | DELETE | the Direct AOT evaluates a two-ticket fill; the RFQ candidate is two execution rows verified by the General accelerator; deleted after the RFQ rides General's verifier (cohort-17) |

**general-accelerator** (1): `general-accelerator/process_instruction` —
**SURVIVE**, the spine's evaluator.

**product-runtime-v2** (1): **SURVIVE** — K and the result domain are the
Product's; the joint clearing's K is `Product.outcomeCount`.

**registry** (11), **rent** (4): **SURVIVE** — release identity and rent are
not clearing.

**resolution** (29): every row **SURVIVE** — the declared source, window,
certificate, selector, recovery ladder and escrow are orthogonal to how
claims changed hands before terminal.

**series-shadow** (2)

| route | disposition | reason |
|---|---|---|
| `series-shadow/process_instruction` | DELETE | a Series occurrence is a Market carrying a General capability; there is no Series-specific selection to evaluate. Composition with decision 0029 item 1 (BUILD A): the SERIES lane's dispatch should select a General capability per occurrence, at which point the shadow ELF and its unhashed compiler release-id preimage (the `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:2655-2662` defect) have nothing to do |
| `series-shadow/entrypoint::evaluate_selected_and_publish#accepted` | DELETE | same |

**trading** (30)

| route | disposition | reason |
|---|---|---|
| `trading/dealer_scenario_checkpoint_v1::…create_v1` | DELETE | the seven-step taker-against-curve chain (create, page, evaluate, reserve, commit, rollback, cleanup) is replaced by one `PlaceOrder` of a schedule order per batch |
| `…page_v1` | DELETE | same |
| `…evaluate_v1` | DELETE | same |
| `…reserve_v1` | DELETE | same |
| `…commit_v1` | DELETE | same |
| `…rollback_v1` | DELETE | same |
| `…cleanup_v1` | DELETE | same |
| `trading/direct_replay_setup_v1` | SURVIVE† | the RFQ venue's replay ledger; unchanged bytes, changed meaning (no public pool) |
| `trading/direct_token_setup_v1` | SURVIVE† | RFQ |
| `trading/direct_fee_settlement_v1` (`DCLTDFS1`, settled permissionlessly on devnet) | SURVIVE† | the RFQ's fee leg |
| `trading/direct_begin_retiring_v1` | SURVIVE† | RFQ root retirement |
| `trading/direct_close_maker_v1` | SURVIVE† | RFQ maker close |
| `trading/hot_v3::process_hot_execution_v3` (entry) | SURVIVE | the one executor; the spine adds no branch to it (0006) |
| `… ⊳ hot_v3::prepare_direct_inline_hot_crosscheck_v3` (sub-row) | SURVIVE† | the inline fill's admission, with the price conjunct derived and the registered branch (`hot_v3.rs:5880-5907`, `DirectRegisteredFillV4.lean`) deleted — that branch is not a route id, which is why the DELETE count above does not include it |
| `… ⊳ hot_v3::try_authenticate_series_expiry_premarket_v1` (sub-row) | SURVIVE | Series permit expiry |
| `trading/hot_v3::process_capability_seal_v1`, `…_close_v1` | SURVIVE | seals |
| `trading/generic_founding_stages_v1::*` (2), `generic_market_founding_v1` | SURVIVE | founding |
| `trading/projected_custody_bootstrap_v1::*` (5) | SURVIVE | founding custody |
| `trading/outer::process_capability_lifecycle#else`, `trading/process_instruction` | SURVIVE | entry |
| `trading/user_position_admission_v1` and its `#Admit` / `#Close` arms | SURVIVE | Positions |

### 3.2 The Lean modules that move

| module | move |
|---|---|
| `GeneralClearing.lean` | gains `Batch` and the window (0010 §6 item 6 says it has neither); `Order.minQuoteCreditPerLot` and `allOrNone` (§1.5); the dealer's `ScheduleOrder` — a demand schedule verified by the same `roundedQuoteFor` at the clearing price (**owed to SCORING-DEALER**: the schedule's shape); the price criterion (**owed to JOINT-CLEARING**) |
| `GeneralTransitionV3.lean` | `Freeze` gains the selection-window conjunct (§2(d)(i)); the selection state is keyed by batch (§1.1); `Close` persists the price vector (§2(c)) |
| `GeneralConfigV3Abi.lean`, `GeneralControllerAbi.lean` | unchanged; fifteen actions stay fifteen |
| `DirectOrdinaryV3.lean` | `:522-524` become one derived-price instruction; the FOK/IOC predicate stays |
| `DirectIntentV2Codec.lean` | → V3 with `counterparty` |
| `DirectRegisteredFillV4.lean`, `DirectProgram.lean`, `DirectProgramV2.lean` | deleted with the registered branch (the V1/V2 program modules are already superseded by V3 and are counted only by the emission census) |
| `Direct.lean`, `DirectLifecycle*.lean`, `DirectSuccessor*.lean`, `DirectControllerCodec.lean` | survive for the RFQ |
| `DealerLiquidity.lean` | `OutcomeCurve` bands become the schedule order's discretization; the LP machine survives |
| `DealerScenarioTradeV4Abi.lean`, `DealerScenarioCheckpointV1Abi.lean`, `DealerScenarioReservationStateV1Abi.lean`, `DealerScenarioCollateral.lean`, `DealerScenarioSolvency.lean`, the netting corpus | deleted with the checkpoint chain |
| `DealerLiquidityAbi.lean`, `DealerTradingProfile.lean` | survive for the LP lifecycle |
| `SeriesOccurrenceV3.lean`, `Series.lean`, `SeriesEscrowV3.lean`, `SeriesReplay*.lean`, `SeriesTicketStateV3Abi.lean` | unchanged: an occurrence realizes a Market; its `capabilityManifestId` names a General capability |
| `StructuredV2.lean`, `RationalRepresentationV2.lean` | unchanged |

### 3.3 The census laws that need a new statement

The journey's L1–L8 hold as they are; the spine adds four the pipeline can
evaluate from chain state at a batch's `Close`:

- **L9 batch conservation.** `Σ debits − Σ credits − mint + merge − surplus_paid = 0`
  over one batch, in atoms — the kernel's `quoteBalances` and
  `close_routes_all_quote` restated as a chain law the observer re-derives.
- **L10 uniform price.** Every execution row settled under one batch is priced
  by one persisted price vector; the observer recomputes `roundedQuoteFor` from
  the vector and the order and requires equality.
- **L11 one clearing.** At most one settlement cursor reaches `Terminal` per
  batch id, and its candidate is the frozen best of that batch's selection.
- **L12 window.** Every admitted order's `admitted_slot` is strictly below its
  batch's `collection_close_slot`, and every `Freeze` slot is at or past
  `collection_close_slot + selectionSlots`.

And the General accelerator's own law is strengthened, not weakened, under the
page: *every runtime observation unchanged, plus page bytes == digest preimage*
(0028 §3). All four are observation-only and ride cohort-16 as a reader with no
program change.

### 3.4 Frame and packet

- **The 880-byte return.** General's bank is `(151 + 6K)·8 + 45·32` bytes
  (`hot_candidate_v3.rs:61-65`): 2,744 at K = 2, 3,272 at K = 13 — four chunks
  — 4,088 at K = 30 — five. The page makes every action one CPI and removes
  `chunks − 1` caller authorities from the frame. **Not a prerequisite for the
  spine**: every action fits chunked at K ≤ 30 with at least 300k of headroom
  (§4). **A prerequisite for the Dealer as participant only if its schedule
  evaluation stays in its own accelerator** — the scenario bank was 4,472
  bytes, six chunks, and never fit (`ACCELERATOR_OUTPUT_CHANNEL` §1); if the
  schedule is a plain order verified by General, no accelerator is involved.
  This note's recommendation follows 0028's read: the page for cohort-16, the
  chunk cost remeasured first.
- **Packets.** `PlaceOrder` at K = 3 is inside the N = 1 campaign's 811–868
  legacy bytes (`GENERAL_ACCELERATOR_CAMPAIGN` §N=1); at N = 258 the family
  needs the v0 table it already has. The RFQ fill is 1,167 bytes v0 with 61
  unique keys, three from the 64-lock wall (`PACKET_LIMIT` §2a), and the
  `counterparty` field adds 32 bytes of *request*, not of keys.
- **The order record** grows by 16 bytes for the seller floor and the
  all-or-none bit; `GeneralOrderV1` re-digests, so `order_id` does.

### 3.5 The cohort sequence

**Cohort-16** — the agenda says no program moves, and the spine needs none to
begin: PROGRAMS-16C's four-action run on one founded Market is the spine's
first physical batch, and its measurements replace the derived column of §4;
L9–L12 as an observation pipeline; the early-freeze hostile written and proved
**red** against the cohort-16 ELFs (a freeze one slot before the selection
window ends is admitted); the price-vector persistence gap recorded as a
stale-claim tripwire (a reader of the series must say which candidates are
still open). Cohort-16 already carries the family lifecycle policy (16B), the
founding changes of 0025 and 0027, and the page if ember rules 0028.

**Cohort-17** — the program moves, each a Lean change first: (1) the `Freeze`
window conjunct; (2) the selection keyed by batch; (3) `minQuoteCreditPerLot`
and `allOrNone` on `Order` and the record; (4) the RFQ price instruction and
`CompactIntentV3`; (5) the registered Direct branch deleted with its Lean and
codec; (6) the dealer's schedule order and the checkpoint chain deleted (with
SCORING-DEALER); (7) the Series dispatch selecting a General capability and
the shadow deleted (with SERIES); (8) the price vector persisted at `Close`.
Every one re-digests a General artifact and so is a cohort boundary; batching
them is the same argument decision 0010 §5 made for the EffectV4 envelope.

## 4. The CU price

Measured on real ELFs unless marked. The harness (scratch, uncommitted) reads
the page and order geometry from the generated ABI — 32 executions per page,
64 pages per candidate — and the figures below from the evidence lines cited.

**Inputs.** `OpenBatch` through Trading Hot with the family policy: **674,333 /
666,011 / 680,789 CU at N = 2 / 13 / 258** (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4563`, PROGRAMS-16B) —
flat in N because the batch actions declare a zero item stride. The
accelerator's whole N = 2 bank in one CPI: **51,404**; one of the four chunks
it replaces: **50,201** (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:3006`). Accelerator-side cost per settlement
action at N = 1 and N = 258 (`GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md:380-400`):
Consider 36,113 / 74,877; Freeze 32,659 / 65,070; InitializeSettlement 61,753 /
164,970; Collect ~57,787 / ~147,496; Materialize 53,171 / 141,402; Distribute
~57,756 / ~145,666; Close 61,334 / 155,786. One delegated Custody leg under
Hot: **182,386**, the Direct fill's implied fee leg, reproduced to the CU
across two ELF sets (`DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md:212`). A Claims
child move has no isolated figure; the devnet custody replay at 91,911
(`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4547`) stands in, **provisional**.

**Derived.** Trading's own share of a Hot action — the floor every transaction
pays before any family evaluation — is `674,333 − 4 × 50,201 = 473,529` CU.
The Dealer note's phase table sums Trading's spans to about 410k before its
accelerator (`DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md`, *"What the Remove
actually spends"*), which is consistent. The eight collection and candidate
actions have no Hot measurement except `OpenBatch`; they are priced at the
settlement family's accelerator cost plus their declared legs
(`GENERAL_SEVEN_ACTION_TOPOLOGY_2026_08_28.md` §3), **derived**.

**Per action at K = 3**, chunked transport | output page:

| action | chunked | page | legs |
|---|---:|---:|---|
| OpenBatch (measured) | 674,333 | 523,730 | — |
| CloseBatch, Freeze, CloseCandidate | 605,173 | 506,440 | — |
| Consider | 619,185 | 509,943 | — |
| SubmitCandidate, InitializeSettlement | 723,753 | 536,085 | — |
| VerifyCandidateRow | 707,469 | 532,014 | — |
| Close | 904,191 | 717,984 | 1 Custody |
| Materialize | 963,254 | 801,683 | 1 Custody + 1 Claims |
| PlaceOrder, CancelOrder, ReleaseOrder, Collect, Distribute | ~981,700 | ~806,300 | 1 Custody + 1 Claims |

Every row fits; the worst, `PlaceOrder`, has 418k of headroom chunked at K = 3
and 313k at K = 30. **The CU ceiling does not bind any action of the spine at
the widths the product ships.** The binding K wall is heap, not compute — the
thirteen per-outcome actions at 384 B/outcome cap near N = 30
(`ITEM_OUTCOME_REGISTER_2026_09_02.md`), and the batch actions at zero stride
run to 258.

**Per batch**, `M` orders each filled by one row, chunked at K = 3:

| M | transactions | sequential | CU total | CU per order |
|---:|---:|---:|---:|---:|
| 1 | 13 | 13 | 10.08 M | 10.08 M |
| 2 | 17 | 16 | 13.73 M | 6.86 M |
| 8 | 41 | 34 | 35.64 M | 4.46 M |
| 32 | 137 | 106 | 123.31 M | 3.85 M |
| 136 (`max_orders` today) | 553 | 418 | 503.18 M | 3.70 M |

`transactions = 9 + 4M`; `sequential = 10 + 3M` (the `Place`s are the only
parallel step). K moves these by under 2% up to 13 and under 12% at 30. The
page takes each order's four transactions from 3.65 M to 2.95 M and the fixed
nine from 6.42 M to 5.13 M — about a fifth, and not what decides fit.

**Against the bilateral fill.** The Direct inline fill on devnet is one
transaction of **1,137,522 CU** (cohort-15) to **1,281,582** (cohort-14) for
two orders — 570k–640k per order. A batch of two is 17 transactions and
**12.1×** that; at scale the spine costs **6.4× per order** chunked and
**5.2×** with the page. That is the honest price of removing the race: each
order pays the Trading floor four times (Place, Verify, Collect, Distribute),
and the floor is 474k. The levers, in order of size: the 0022/0023 carriers
that keep cutting the floor; rows per transaction on `Collect` and `Distribute`
(three rows fit under the ceiling: `474k + 3 × 274k = 1.30 M`), which takes
the chain to `9 + 2M + 2⌈M/3⌉`; and the page.

**The cadence the ceiling allows.** None of the above is a per-slot number.
At one slot per dependent step the optimistic floor after the collection
window is `10 + 3M` slots — 14 s for eight orders, 42 s for thirty-two, 167 s
for a full batch — and a confirmation-bound client pays two to three times
that. The frequency is `1 / collectionSlots` once batches pipeline (§1.1);
the latency is the triple. A one-second batch is not on this substrate, and
was never the product: the race the spine removes — the taker snipe and the
leader's ordering of fills — is removed at any window of one slot or more.

## 5. For ember

Yes, this is the product's coherently extrapolated shape, and the tree half
knew it: the twelve-item ceiling already names the frequent batch, the RFQ,
schedule-compiled liquidity and the convex maker as the four venues, and
`GeneralClearing.lean` already is a uniform-price joint clearing with the
complete-set move inside the batch. The single decision that commits us is:
**every transfer of claims between two parties is a verified General
candidate — including the bilateral one — and a resting order rests in a
batch, never in a public pool of bearer tickets.** Saying it deletes the
registered Direct branch, the Dealer's seven-step checkpoint chain and the
Series shadow, keeps Direct as the RFQ with its price derived rather than
matched, and turns the Dealer into a schedule that is in every batch; it does
not touch founding, custody, resolution or retirement. Two things it costs
that should be said plainly: General as built is one call auction per Market
until the selection is keyed by batch, and a batch costs five to six times
the compute of a bilateral fill per order, so the spine's frequency on Solana
is tens of seconds, which is right for a forecast series and wrong for a
latency product we refused on the first day.
