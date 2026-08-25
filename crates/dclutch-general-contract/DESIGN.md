# General frequent-batch contract

`dclutch-general-contract` is the pure semantic owner of the optional General
venue. It is `no_std`, `no_alloc`, safe Rust, fixed-capacity, and uses checked
integer arithmetic. Solana accounts, hashing, signatures, token movement, and
transaction construction remain in the adapter boundary.

## Acyclic authority

`GeneralConfigV1` is a reusable, pre-Market, permanent generic finalized
record. Its schema label is `dclutch/schema/general-config-v1` and its exact
width remains 200 bytes. It contains the capacity profile, ClaimBasis, reviewed
General release, independently fixed generation, simplex scale, three slot
windows, order/page bounds, exact permissionless-continuation reward, and
outcome count. It deliberately contains no Market key, MarketIdentity digest,
manifest ID, or settlement mint.

Construction is acyclic and one pass:

1. hash canonical General config;
2. put that config digest in the capability manifest entry and hash the
   manifest;
3. put that manifest digest in MarketIdentity and derive/create Market;
4. activate General after authenticating Market -> manifest -> config.

The activated 136-byte `GeneralRootV1` is the sole later owner of the actual
Market account binding. It persists `{config_id, market, RentCredit,
generation, next_batch_sequence, open_batches, phase}`. Signed orders and
candidate submissions carry that Market key, and pure transitions compare it
to `root.market` plus config ClaimBasis/generation. The adapter verifies the
MarketIdentity digest only during activation; persisting both Market address
and digest in General state would create a redundant mismatch state.

The config never repeats settlement-asset identity. Market -> Realm -> Mint and
token release remains its sole authority. Finalized config consumers always
provide the exact raw record, matching vacant staging cursor, and shared trusted
Rent proof. General never creates or closes a config account.

The closed reviewed identities are hashes of:

- `dclutch/general/capability-kind/v1`;
- `dclutch/general/frequent-batch-release/v1`;
- `dclutch/general/child-schema/v1`;
- `dclutch/general/child-derivation/v1`; and
- `dclutch/schema/general-config-v1`.

## Derivation authority

Canonical ordered seed tuples are:

- root: `[general-root-domain, Market, generation-le, config-id]`;
- funding: `[general-funding-domain, Market, generation-le, config-id,
  release-id]`;
- batch: `[general-batch-domain, root, sequence-le]`;
- order replay: `[general-order-state-domain, Market, generation-le, owner,
  nonce-le, order-id]`;
- order custody: `[general-order-custody-domain, order-replay]`;
- order quote escrow: `[general-quote-escrow-domain, order-custody]`;
- candidate: `[general-candidate-domain, batch, candidate-id]`;
- immutable candidate page copy:
  `[general-candidate-page-domain, candidate, page-id]`;
- settlement cursor: `[general-settle-domain, candidate]`; and
- settlement quote escrow: `[general-settle-escrow-domain, cursor]`.

The capability contract remains owner of generic funding derivation. General
uses its exact Market/generation/manifest-entry/config/release tuple only after
the adapter authenticates the manifest digest.

## Stored candidate pages and data availability

A candidate cannot depend on a solver or index resupplying page bodies after
selection. `CandidatePageV1<N>` is an immutable, exact-width, content-addressed
record containing:

- zero-based page index;
- optional next page content ID, with all-zero terminal sentinel;
- exact leading execution count `1..=4`; and
- the exact signed order, authenticated order-state snapshot, and scalar fill
  for every execution.

Pages are built last-to-first. `page_id` is the domain-separated hash of the
canonical page bytes, which exclude their own ID. The candidate-exclusive PDA
also contains candidate ID, so identical content used by two candidates never
shares close authority.

`CandidateSubmissionV1<N>` binds the first page ID, exact claimed page and
execution counts, final score, final quote-debit numerator, exact-N final net
coefficient vector, simplex prices, page-Rent reserve, Market, ClaimBasis,
generation, batch, deadline, and submitter/RentCredit. `candidate_id` is the
hash of this noncircular canonical submission.

Candidate page accounts are created from candidate-owned reserved Rent. Page
creation increments the exact live page-child count. System-owned,
nonexecutable, data-empty precreation dust cannot block creation: dust reduces
the top-up, displaced candidate reserve goes to the immutable RentCredit, and
page surplus also goes there. Allocated/live/wrong-owner targets remain
refused by the adapter.

Verification starts at `submission.first_page_id`, reads the authenticated
onchain page account, and advances only to that record's `next_page_id`.
Finishing requires the terminal sentinel, exact claimed page/execution counts,
all claimed page children present, zero page-Rent reserve, and exact equality
with every claimed aggregate before enforcing complete-set and quote
conservation. Caller-supplied successor IDs or page bodies are not instruction
authority.

Collection and distribution each restart at the same stored first page and
walk the same immutable links. Distribution atomically closes the page after
using it a second time. Rejected, expired, and losing candidates expose a
permissionless page-close transition. Candidate close refuses any live page
child and is the only transition that decrements the batch's exact open
candidate-child count.

## Candidate-owned liveness capital

Attacker-created candidates never consume founding General liveness and do not
impose unfunded cleanup. Candidate admission escrows five exact native
components on the candidate state account:

- current candidate-state Rent;
- exact aggregate page Rent committed by the submission;
- exact current Rent for the settlement cursor, Position, and quote escrow;
- verification work `(P + 2) * reward` for `P` page verifications, finish, and
  consideration;
- selected-settlement work `(2P + 4) * reward` for begin, `P` collection
  pages, materialize, `P` distribution pages, finish, and settlement close;
- unspendable cleanup work `(P + 1) * reward` for every possible page close
  plus candidate close.

These compartments are persisted separately and are not interchangeable even
when total lamports match. Every transition authenticates physical candidate
lamports as current Rent plus all exact remaining compartments. A selected
candidate cannot enter `Applying` unless verification is exhausted and the
entire selected-settlement and cleanup reserves are intact.

On the selected path each distribution pays one settlement reward and one page
cleanup reward while closing that page and routing its observed lamports to
RentCredit. Settlement close verifies zero Position/token inventory, exhausts
only the selected-settlement compartment, and routes all three temporary
account balances to RentCredit; candidate close consumes the final cleanup
reward. On rejected/expired/losing or partial chains, each actually created
page close consumes one cleanup reward. Candidate close consumes one more and
refunds unused page Rent and unused verification/settlement/overprovisioned
cleanup capital only to the immutable RentCredit. Thus unbounded submissions
remain funded by their submitters rather than a global candidate-count cap,
Hoard, or future fees.

Batch account Rent/work is separately prepaid when the batch opens. It owns
current Rent plus exactly three rewards for collection close, selection close,
and terminal retirement. Safe precreation dust reduces the opener top-up and
any surplus routes to the root's permanent RentCredit. General activation
covers only finite activation/root/funding obligations; it does not pretend to
capitalize future unbounded batches or candidates.

## Two-pass physical settlement

The best valid submitted candidate is selected by highest checked preference
score, with lexicographically smaller candidate ID breaking ties. No optimality
claim is made without a checked certificate.

Every valid candidate is globally balanced:

```
net_claims = [k, k, ..., k]
total_quote_debit_numerator = k * price_scale
```

Pages need not be locally balanced. Settlement therefore has three physical
phases:

1. Collection pages consume only negative quote/claim deltas from authenticated
   order custody into cursor-owned quote escrow and Position inventory. They
   emit no owner output.
2. One materialization action proves the complete first replay and performs
   the sole Market/Realm/collateral-vault complete-set split (`k>0`), merge (`k<0`), or no
   supply mutation (`k=0`).
3. Distribution pages replay the same stored records, emit only positive
   outputs, and close each page. Finish requires the complete second replay,
   zero remaining output obligations, zero observed physical quote/claim
   custody, and zero page children.

`SettlementCursorV1<N>` is deliberately minimal. It persists candidate ID,
phase, two linked-page replay cursors, and remaining positive output
obligations. It does not persist Market, generation, owner, escrow,
RentCredit, complete-set delta, quote inventory, or claim inventory. Those are
owned by candidate/root/account derivations or physical token/Position state.
Every transition accepts observed physical balances and returns exact required
postconditions. In collection, expected inputs derive from accumulated outputs
minus replay net deltas; in distribution, physical custody equals remaining
outputs; Finished is zero.

Page replay has no fixed array of full receipts/effects. The whole-page method
replays and commits the linked cursor plus one page reward atomically, returning
only compact inventory postconditions. `execution_plan` independently replays
the same prefix and returns one indexed receipt/custody effect at a time. The
adapter must use the persisted page for both calls and compare the custody
transition to that one-execution plan. This preserves exact page authority
while keeping every no-alloc kernel and SVM frame bounded.

Quote transfer uses one canonical-prefix Euclidean carry. For each strict
order-ID execution:

```
combined = -exact_quote_debit_numerator + prior_carry
quote_delta_atoms = floor_euclid(combined / price_scale)
next_carry = rem_euclid(combined, price_scale)
```

Both replays must finish with carry zero and exact candidate aggregates. There
is no dust account or second rounding boundary.

## Orders, funding, and terminal absence

Orders bind actual Market key, ClaimBasis, generation, batch, Ed25519/SVM
`OwnerKeyV1`, nonce, expiry, lot cap, exact coefficient vector, and debit limit.
`order_id` hashes the canonical signing preimage excluding `order_id` itself.
Worst-case quote reserve is
`ceil(max(0, max_debit_per_lot_numerator) * max_lots / price_scale)`; claim
reserve for outcome `i` is `max(0, -coefficient[i]) * max_lots`. Admission
atomically creates replay, native Position custody, and quote escrow. Cancel,
receipt consumption, and close each couple replay to exact physical release.

`GeneralFundingV1` owns only the activation-derived native liveness/work/bounty
compartments. Capability service maps to liveness, work to work, and bounty to
bounty. Provider/liquidity and all Realm-collateral funding are refused. Rent
and creation are native. Remaining/spent/refunded conservation is checked per
compartment; no compartment borrows from another. Hoard principal and future
fee revenue are never funding.

Batch/root retirement uses exact persisted child counters. No caller boolean,
index, or absence attestation can close a live batch, candidate, page,
settlement, or root. Permanent RentCredit receives all closed-account Rent.

## Exact widths and wire actions

- config 200; root 136; General funding 144; batch 144; order replay 96;
- order `200 + 8N`; order custody `192 + 8N`; receipt `176 + 8N`;
- candidate submission `224 + 24N` (272 at N=2, 608 at N=16);
- candidate state `440 + 40N` (520 at N=2, 1,080 at N=16);
- candidate page `56 + M * (304 + 8N)` for exact `M=1..4`
  (696/920 for M=2 at N=2/N=16);
- minimal settlement cursor `304 + 40N` (384 at N=2, 944 at N=16).

The ordered account frames are closed as well. `C` means GeneralConfig raw,
`V` its staging vacancy; `R/Q/M` mean Realm/ClaimBasis/Manifest raw and
`Vr/Vq/Vm` their matching vacancies. Repeated parenthesized roles occur once
per exact page execution `E`:

- Activate: `[Activator, Market(w), R, Q, M, C, Vr, Vq, Vm, V, Mint, Token,
  CapabilityFunding(w), Root(w), GeneralFunding(w), RentCredit(w), System,
  Rent, Clock]`;
- OpenBatch: `[WorkActor, Market, C, V, Root(w), Batch(w), RentCredit(w),
  System, Rent, Clock]`;
- LockBatch/LockSelection: `[WorkActor, C, V, Root, Batch(w), Rent, Clock]`;
- AdmitOrder: `[OwnerPayer, Market, R, Q, C, Vr, Vq, V, Mint, Token, Root,
  Batch, OrderState(w), OrderCustody(w), OwnerPosition(w), QuoteSource(w),
  QuoteEscrow(w), RentCredit(w), System, Rent, Clock]`;
- CancelOrder: `[OwnerSigner, Market, R, Q, C, Vr, Vq, V, Root, Batch,
  OrderState(w), OrderCustody(w), OwnerPosition(w), QuoteEscrow(w),
  QuoteDestination(w), Mint, Token, RentCredit(w), Rent, Clock]`;
- CloseOrder is CancelOrder without owner signer/Clock and with Market first;
- SubmitCandidate: `[Submitter, C, V, Root, Batch(w), Candidate(w),
  RentCredit(w), System, Rent, Clock]`;
- CreatePage: `[WorkActor, C, V, Root, Batch, Candidate(w), Page(w),
  RentCredit(w), System, Rent]`;
- VerifyPage: `[WorkActor, Root, Batch, C, V, Candidate(w), Page,
  (OrderState)E, Rent, Clock]`;
- FinishCandidate: `[WorkActor, Root, Batch, C, V, Candidate(w), Rent,
  Clock]`;
- ConsiderCandidate: `[WorkActor, Root, C, V, Batch(w), Candidate(w), Rent,
  Clock]`;
- BeginSettlement: `[WorkActor, Market, R, Q, C, Vr, Vq, V, Mint, Token,
  Root, Batch(w), Candidate(w), Cursor(w), SettlementPosition(w),
  SettlementQuoteEscrow(w), RentCredit(w), System, Rent, Clock]`;
- CollectPage: `[WorkActor, Market, R, Q, C, Vr, Vq, V, Mint, Token, Root,
  Batch, Candidate(w), Cursor(w), SettlementPosition(w),
  SettlementQuoteEscrow(w), Page, (OrderState(w), OrderCustody(w),
  OwnerPosition(w), QuoteEscrow(w))E, Rent]`;
- Materialize: `[WorkActor, Market(w), R, Q, C, Vr, Vq, V, Mint, Token,
  CollateralVault(w), Root, Batch, Candidate(w), Cursor(w),
  SettlementPosition(w), SettlementQuoteEscrow(w), Rent]`;
- DistributePage: `[WorkActor, Market, R, Q, C, Vr, Vq, V, Mint, Token,
  Root, Batch, Candidate(w), Cursor(w), SettlementPosition(w),
  SettlementQuoteEscrow(w), Page(w), RentCredit(w),
  (OwnerPosition(w), QuoteDestination(w))E, Rent]`;
- FinishSettlement: `[WorkActor, Market, R, Q, C, Vr, Vq, V, Mint, Token,
  Root, Batch(w), Candidate(w), Cursor(w), SettlementPosition(w),
  SettlementQuoteEscrow(w), Rent]`;
- CloseSettlement is FinishSettlement plus writable RentCredit and no distinct
  physical-output roles;
- ClosePage: `[WorkActor, C, V, Root, Batch, Candidate(w), Page(w),
  RentCredit(w), Rent]`;
- RejectCandidate: `[WorkActor, Root, Batch, C, V, Candidate(w), Rent, Clock]`;
- ExpireSettlement changes only Batch to writable in the preceding frame;
- CloseCandidate: `[WorkActor, Root, Batch(w), C, V, Candidate(w),
  RentCredit(w), Rent]`;
- CloseBatch: `[WorkActor, C, V, Root(w), Batch(w), RentCredit(w), Rent]`;
- Quiesce: `[Root(w)]`; and
- CloseGeneral: `[Market(w), C, V, Root(w), GeneralFunding(w),
  RentCredit(w), Rent]`.

Signer/writable/executable bits are exact. Accounts in one frame are pairwise
distinct except that repeated executions may name the same `OwnerPosition`
and/or the same `QuoteDestination`; those are the two physical destinations
canonically selected by the signed order owner, and the adapter accumulates
their page effects before proving the final account state. Cross-role aliases
and aliases among replay, custody, escrow, page, parent, record, vacancy, and
sysvar roles are refused. There is one shared readonly Rent sysvar, and each
generic raw record has its own readonly system-vacant staging cursor. No
caller-authored status account is accepted.

Page-consuming instructions carry only `{candidate_id, page_id}` after the
16-byte family header: 80 bytes exact. `CreateCandidatePage` alone carries the
canonical page bytes so the adapter can hash and persist them. The closed
action set additionally includes `CloseCandidatePage`, `RejectCandidate`, and
`ExpireSettlement`. All decoders reject short, trailing, reserved,
width-substituted, unknown-tag, zero-ID, and noncanonical optional fields.

The adapter must dispatch exact `N=2..16`, authenticate every content hash and
PDA, use the contract's ordered frames/codecs directly, verify owner signatures,
execute returned Position/token/Market/Hoard movements atomically, and pay each
returned work/cleanup debit to the frame's permissionless actor. Static clients
and indexes remain untrusted projections.
