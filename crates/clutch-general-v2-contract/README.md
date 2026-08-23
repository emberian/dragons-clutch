# General V2 contract

This crate is the disabled, dependency-free account and identity contract for
the future General V2 vertical spine. It is `no_std`, uses no allocator, and
contains no Solana SDK, account-memory, CPI, token, clock, or signature code.

Nothing here is a live route. The SBF capability table remains fail-closed, and
allocating a tag does not authorize a handler. In particular, the current
Market, CandidateLifecycle V4, RelationV2 adapter, settlement adapter, and
counted Epoch codecs must not be described as General V2-compatible.

## Frozen account coordinates

| Semantic owner | Tag/version | Exact length |
|---|---:|---:|
| genesis-assisted `MarketRuntimeV3AccountV1` | `3/3` | 148 |
| counted `GeneralEpochV6AccountV1` | `11/6` | 321 |
| `ClearWorkV2` | `17/2` | `672 + 16*O + 8*N*O`, max 9,120 |
| sealed `CandidateFeedV2` | `18/2` | `538 + 8*O + 8*N + 24*A + 13*S`, max 6,970 |
| `CandidateWindowV4` | `24/4` | 565 |
| `CandidateFeedStageV2` | `25/2` | same active length as sealed feed |
| `AdmissionNodeV3AccountV1` | `0x77/1` | 743 |
| `EpochBudgetV2AccountV1` | `0x78/1` | 272 |
| immutable `MarketBindingV1` | `0x79/1` | 540 |
| counted-retirement Replay successor | `0x7a/1` | 132, owned by retirement/reference |
| immutable `EconomicDomainV2AccountV1` | `0x7b/1` | 297 |
| `SelectedCandidateV1AccountV1` settlement authority | `0x7c/1` | 789 |
| withdrawn `OwnerSettlementV1AccountV1` envelope | `0x81/1` | 292 |
| disabled presence-explicit `OwnerSettlementV2AccountV1` envelope | `0x81/2` | 292 |
| disabled selected fee-record envelope | `0x82/1` | 340 |
| disabled owner fee-carry envelope | `0x83/1` | 132 |
| disabled temporary payer-allocation envelope | `0x84/1` | 2,684 |
| disabled temporary recipient-allocation envelope | `0x85/1` | 2,644 |
| disabled treasury-ledger envelope | `0x86/1` | 148 |
| disabled buyer-first settlement cash-pot envelope | `0x87/1` | 260 |
| disabled combined FinalPot/virtual-budget envelope | `0x89/1` | 332 |

The successor `solana-layout` collision ledger reserves every coordinate above
as `ReservedDisabled`, records retirement's provisional
tombstones at `0x75/1` and `0x76/1` plus its permanent Position tombstone at
`0x75/2`, and proves its recorded rows internally
disjoint. Dealer owns `0x7d/1` and `0x7e/1`; Source/Series owns `0x7f/1` and
`0x80/1`. General does not reinterpret those coordinates. The `0x81/1`
owner-settlement coordinate stays withdrawn; `0x81/2` is the sole future row
and remains a reservation, not an executable capability. Their codecs and PDA
domains never alias. A complete legacy-account inventory cross-check remains
an activation gate.
The same coordinated block reserves the StructuredClaim descriptor at
`0x88/1` and a fresh General FinalPot at `0x89/1`. The latter now has a strict
332-byte outer codec around the canonical 328-byte combined FinalPot and
selected virtual-budget body, but no live route or retirement authority.
The numeric constants in this standalone crate describe matching codec bytes,
not a second allocation authority; the eventual adapter must add a parity gate
against the central registry when both crates are dependencies.
Active-width feeds have no inactive outcome/order/atom or
slice padding. A stage requires every not-yet-written active element to remain
zero; a sealed feed checks exact simplex sum, sorted positive atoms, primitive
mass scale, and live slice shape.

Fresh seed domains are exported for Market binding, Epoch, EconomicDomain,
Window, admission node, feed, work, budget, selected candidate, order page,
reservation, receipt, and final pot. The frozen FinalPot tuple is
`general-final-pot:v2`, Epoch PDA, final SettlementCandidateId. PDA derivation,
stored-bump checking,
program ownership, and generation authentication remain adapter obligations.
The first-spine tuples are exact ordered seeds:

| Account | Ordered seeds after the program ID |
|---|---|
| MarketBinding | `general-market-binding:v1`, full `MarketInstanceV2Id` |
| MarketRuntime | `general-market-runtime:v1`, MarketBinding PDA |
| Epoch | `general-epoch:v2`, MarketBinding PDA, `epoch_index_le` |
| EconomicDomain | `economic-domain:v2`, Epoch PDA |
| Window | `general-window:v4`, Epoch PDA |
| Budget | `candidate-budget:v2`, Epoch PDA |
| AdmissionNode | `general-candidate-admission:v1`, Epoch PDA, Window ordinal LE |
| Feed/Stage | `candidate-feed:v2`, AdmissionNode PDA |
| ClearWork | `clear-work:v2`, AdmissionNode PDA |
| SelectedCandidate | `selected-candidate:v1`, Epoch PDA, final `SettlementCandidateId` |
| OwnerSettlement V2 | `owner-settlement:v2`, Epoch PDA, final `SettlementCandidateId`, semantic owner |
| selected fee record | `selected-fee-record:v1`, SelectedCandidate PDA |
| owner fee carry | `owner-fee-carry:v1`, selected fee-record PDA, semantic owner |
| temporary payer allocation | `owner-payer-allocation:v1`, selected fee-record PDA, semantic owner |
| temporary recipient allocation | `candidate-recipient-allocation:v1`, selected fee-record PDA |
| treasury ledger | `fee-treasury-ledger:v1`, selected fee-record PDA |
| settlement cash pot | `settlement-cash-pot:v1`, Epoch PDA, final `SettlementCandidateId` |

The Window assigns the ordinal atomically before deriving a node; no
submitter-selected commitment or address controls the final rank tie. Remaining
order-page/reservation/receipt/pot seed suffixes stay unallocated until their
complete successor handler contracts are frozen.
This fresh General V2 node identity deliberately supersedes ADR-0008's
submitter/commitment-derived `candidate-admission-v3` tuple; no handler may mix
the two derivations. PDA vectors and superseded implementation documents must
be synchronized before activation.

## Semantic owners and mandatory joins

- Product owns the exact `MarketGenesisProfileV2Id`, full
  `MarketInstanceV2Id`, recurring `SeriesPlanV5Id`, and exact
  `SeriesFundingTermsV2Id`. The Market binding stores each typed 32-byte
  identity. The legacy eight-byte Market nonce is not an injective lowering
  and is never accepted as Product provenance.
- `MarketGenesisProfileV2` selects exactly `PriceMeasurePolicyV1Id`. Product's
  policy admits QuantizedIntegerGrid V3 degrees zero through three. General V2
  admits the same range and pins witness schema V3 plus quantized semantics V1;
  there is no continuous fallback.
- `EconomicDomainV2AccountV1` is the single persisted owner of the canonical
  per-Epoch domain transcript. Its identity is FIPS 180-4 SHA-256 over
  `"dragons-clutch/economic-domain/v2\0" || encoded_transcript`; the crate
  constructs the transcript from typed fields rather than hashing caller
  bytes. Its coordinate bounds must exactly project the Genesis V2 bounds;
  NativeClaimBasis remains the owner of rows/map/knots/denominator and
  edge/ambiguity selectors. The SHA backend is an explicit adapter boundary.
- The price bridge must prove all three equalities: selected Genesis
  `PriceMeasurePolicyV1Id == RelationV2.price_policy_digest`; authenticated
  `NativeClaimBasisV1` exact-body ID equals the certificate `basis_digest`; and
  RelationV2's sole canonical price-semantics function over the projected
  domain and all 16 canonically padded prices produces the certificate
  `candidate_price_digest`. General's dependency-free mirror uses the exact
  RelationV2 transcript; dev-only differential tests cover widths 2 and 16,
  field mutations, padding, and simplex refusals. The EconomicDomain artifact
  digest and certificate body digest are separate bindings, never substitute
  price identities.
- A Direct final `SettlementCandidateId` is the verified RelationV2 economic
  digest. A CoveredDealer final ID is the checked dealer-economic digest. The
  feed, work, verdict, score, selected entitlement, cleanup, and settlement
  must bind one identical final ID. CoveredDealer stays disabled until its
  atomic facility/vault/quote/verdict/receipt/transfer adapter exists.
- The canonical descending score has 88 active bytes in a 96-byte account
  container: risk `u64` big-endian; complemented cash `u64`; complemented churn
  `u64`; complemented final `SettlementCandidateId`; complemented 32-byte
  first-admitted coordinate. The latter is 24 zero bytes plus Window-owned
  one-based ordinal. Node validation checks both embedded identities and the
  eight canonical zero padding bytes. The current CandidateLifecycle V4 uses a
  node-ID suffix and is therefore incompatible.
- `clutch-general-v2-runtime` owns the fixed domain-separated General selector
  IDs for the already-owned owner-blind RelationV2 and ScoreV2-Q semantics. A
  live adapter must derive those IDs rather than accepting arbitrary nonzero
  relation/score labels, and must persist the runtime-returned rank rather than
  caller-supplied score components.
- Duplicate candidates are economically bounded by per-node bond, rent, and
  verification capitalization. Progress rewards pay checked work from that
  node's compartment; they are not duplicate solver prizes. The root Budget
  has one solver-prize compartment, claimable only under the counted
  SelectedCandidate authority to its copied solver destination; the historical
  source Node need not remain live. The first-admitted ordinal resolves exact
  duplicate ranks without a submitter-grindable key.
- Before finalization, Window exclusively owns the working best node/final
  ID/rank/ordinal. Finalize is allowed at or after submission close only after
  exhaustive terminal counts prove no admitted node remains undecided. It
  atomically creates `SelectedCandidateV1`, retains its sealed Feed as the
  settlement-data authority, increments the Epoch selected-artifact count,
  zeroes every Window working-best field, and stores only the artifact pointer.
- The selected identity and economic/certificate bindings are immutable, while
  `next_slice_index` and `entitlement_state` are mutable monotone settlement
  progress. Consumers must not cache the whole account as immutable.
- Epoch `candidate_bundles` counts AdmissionNodes: increment at commitment and
  decrement on permissionless reverse-head node deletion. The pure cleanup
  classifier authenticates the Window head, terminal node, canonical Feed/Work
  presence, and live selected artifact. Before finalization it refuses the
  working-best node. After finalization the selected source Node may close only
  through the distinct transition that leaves its Feed under the counted
  SelectedCandidate; non-selected terminal bundles with Work absent may close
  Node and Feed together. Cleanup is delayed until submission close, validates
  the exact ordinal/predecessor reverse head, and returns paired Epoch/Window
  post-counts. Epoch separately counts the selected artifact until every
  retained-Feed slice has been materialized into counted entitlement state;
  retirement also authenticates the exact Epoch-derived Budget and requires
  its solver prize paid or explicitly neutralized before atomically closing the
  Feed/artifact and decrementing the count. Every
  cleanup path coalesces credits by destination before moving lamports; an
  unavailable submitter is never required.
- Node, Feed, Work, Window, EconomicDomain, and SelectedCandidate each carry a
  complete rent owner/principal/donation compartment. Candidate funding is a
  checked split: commitment pays only Node rent, bond, and node-close reward;
  reveal pays exact active-width Feed/Work rent, feed-close reward, and exact
  price/order/slice/completion/work-close rewards. The API separately reports
  both transition debits, three account allocations/balances, and an auditable
  lifetime total so no handler can double-charge commitment capital.
  EpochBudget separately prepays selected-artifact rent and records its original
  payer; its root-close compartment remains fully present in every live Budget.
  A keeper top-up, future fee, or refundable principal is not liveness funding.

## Action allocation and pure payload contracts

General family `74/1` retains local action names `1..=38`. Strict allocation-free
payload decoders and pure poststate owners now exist for the identity-lab spine
at actions 2, 6, 7, 8, 9, 10, 14, and 15, permissionless reverse-head cleanup
at action 20, and the separately authenticated one-way solver-prize claim at
action 21. Action 32 owns bounded terminal ClearWork closure and its paired
Epoch Work-count decrement. This pure crate does not provide an account-meta
handler or itself activate a runtime route; those remain separate adapter and
release obligations.

Action 16 also has one deliberately narrow pure transition: at or after
submission close it may terminalize only an unrevealed `Committed` node as
`ExpiredCommitment` and increment the Window's matching count. It performs no
lamport movement and does not infer revealed/Work expiry semantics.

Actions 24 and 25 have strict disabled payload facts only. Action 24 is the
96-byte selector `epoch || selected_candidate || owner`. Action 25 is renamed
`AccountReceiptEnd` with no compatibility alias and uses the 160-byte selector
`epoch || selected_candidate || owner_settlement || receipt ||
receipt_accounting_id`. Slice, order, side, price, and completion are owned
solely by the authenticated receipt and selected-order projection, never by
caller bytes. The accounting ID is persisted and replay-checked separately
from every later Egg-delivery transition ID.

Action 38 `FinalizeOwnerSettlement` has a strict disabled 192-byte selector
`epoch || selected_candidate || owner_settlement || position ||
settlement_cash_pot || finalized_owner_row_data_id`. It exists separately because a
last receipt fragment may leave a credit-bearing owner waiting for earlier
buyer or merge liquidity. Net owner debits are admitted into the pot first;
credits refuse and retry without consuming replay or liveness when liquidity
is absent. The request identity must equal the adapter-authenticated data ID of
the canonical finalized 288-byte row; it is not copied into the row. The
one-way row state and in-place fee finalization receipt own persistent replay.
No live SBF success transition exists: creating the 288-byte semantic body requires the
complete authenticated filled-order set, exactly one selected-fee row per
participating owner, checked candidate totals, and a canonically derived owner
order-set digest. General V2 does not yet expose that complete projection.
The 292-byte outer row stores only tag/version, that semantic body, bump, and
zero flags. Its pre-fund-safe creation plan must atomically update the separate
rent ledger that owns payer principal, refund recipient, and donation sink.

The capability-disabled fee composer now rederives the V2 row/Position/pot
realization, derives purpose Replay V3 in the same plan, uses the finalized row
data ID as Replay transition identity, and uses the exact deleted `0x84` payer
outer data ID as distinct Replay evidence. The existing `0x83/1` carry is
reallocated in place to exact 500-byte `0x83/2`; payer principal and hostile
prefunding remain separate refund/neutral-sink transfers. The SBF seam freezes
strict PDA, outer-version, data-ID, byte-postimage, close, and lamport-delta
checks, but exports no dispatch route until the authoritative rent ledger and
exhaustive signed-envelope loader can mint the pure plan.

Action 26 is renamed `ConsumeDirectReceiptEggs` and has the exact disabled
96-byte selector `epoch || receipt || delivery_transition_id`. The imported
pure planner requires both real ends already accounting-latched and both owner
rows already finalized by action 38, then
atomically stages a distinct delivery latch, both Positions, and both
Reservations while treating terminal owner rows as read-only finalization
evidence. It moves only internal native Eggs; cash conversion remains
owner-terminal. The action stays disabled until the direct receipt can project
an exact Settlement-compartment liveness receipt, call ordinal, quote ceiling,
keeper payment, and payer refund.

Actions 36 `ConsumeVirtualSplitReceiptEggs` and 37
`ConsumeVirtualMergeReceiptEggs` each have a distinct strict 96-byte disabled
selector `epoch || receipt || delivery_transition_id`. They are not aliases
for action 26 or for each other. A future handler must bind one checked
selected-candidate witness and transition ID across the 328-byte FinalPot's
embedded virtual budget, Hoard/aggregate supply, one real receipt end, Position,
Reservation, distinct delivery latch, and Settlement liveness mutation. Split
authenticates a finalized buyer row and the exact remaining split cash; merge
authenticates an AccountingComplete state-zero seller row because its proceeds
must exist before seller finalization. Neither row is rewritten. No inventory-only
account, rent owner, close authority, or action is allocated, and neither route
is executable yet.

The capability-disabled fee envelopes at `0x82` through `0x86` add only an
exact outer tag/version, the constructor-checked inner fee codec, a stored PDA
bump, and zero flags. The separately authenticated runtime/rent ledger owns
funding, refundable principal, and hostile-prefund disposition; these semantic
accounts do not duplicate that truth. The selected record is candidate-scoped;
carry and payer allocation are keyed by
`(selected fee record, owner)`; recipient allocation and treasury ledger are
selected-record scoped. No General action is assigned to these accounts yet,
and no fee-bearing value movement becomes executable from their reservation.

The disabled `0x87/1` account wraps the exact 256-byte buyer-first cash-pot
body. It segregates buyer consideration, selected fees, rounding price units,
and exactly one typed virtual-cash direction while owner rows are realized.
`Split` retains terminal cash, `Merge` contributes opening proceeds, and
`None` requires zero virtual cash; the exact conservation equation is buyer
debit plus opening merge minus seller credit equals rounding plus terminal
split. Allocation
completion is not retirement authority: no action may close the pot or move
value until the matching complete Egg/reservation transition and later
FinalPot disposition owner are both authenticated.

Action 20's strict 96-byte payload is `epoch || node || selected_candidate`.
The selected field is all zero exactly when the Epoch and Window authenticate
that no selected artifact exists; otherwise it is the actual artifact PDA and
the adapter must decode that account. Fixed Feed and Work metas use their
derived PDA identities even when the adapter proves canonical account absence.
The pure transition refuses `ExpiredUnverified`: its remaining Work funding,
refund, and penalty semantics are not inferred from another terminal class.
Action 32 reuses the exact 64-byte `epoch || node` payload. It admits only a
terminal phase-3, zero-order/zero-slice Work belonging to a VerifiedValid or
VerifiedRefused node, returns the decremented Epoch and `close_work = true`,
and coalesces Work rent principal to its recorded payer, hostile donation floor
to the immutable neutral sink, and the present-funded close reward to keeper.
The intended phases are:

| Actions | Intended phase |
|---:|---|
| 1-6 | bind Product Market, initialize Epoch/domain/budget/pages, orders, freeze |
| 7-10 | funded commitment, staged reveal, seal, exact-size work creation |
| 12-16 | advance RelationV2 checks, verdict, best-valid selection, inferred expiry |
| 20-21 | permissionless dependent cleanup and the unique selected solver claim |
| 23-30 | unused root funding and existing entitlement/settlement retirement |
| 31-34 | reverse-head node/work/Epoch/position retirement |

Old names 11 (`GrowClearWork`), 17 (`MarkWorkClosed`), 18
(`ClaimCandidateBond`), 19 (`ClaimCandidateWork`), and 22
(`CloseCandidateIndexPage`) must remain permanently disabled: active-width
creation removes growth, state-dependent close removes marker/claim races, and
the reverse-linked admission node removes the candidate-index page.

No other action may activate merely because its number is allocated. Each
needs a strict payload codec, ordered account metas with signer/writable/owner
requirements, alias rules, exact prefund/rent arithmetic, generation/counter
transitions, refusal rollback tests, and a capability-specific evidence gate.

## Remaining activation blockers

The next runnable vertical slice still needs a streaming RelationV2 accumulator
contract in addition to the raw SHA checkpoint, Product-successor authentication,
exact account-meta codecs, and
atomic retirement transitions that include EconomicDomain and the counted
SelectedCandidate artifact. Product/policy fields are labeled 32-byte slots in
this dependency-free crate; the adapter must authenticate and convert the
actual Product typed IDs, and hostile swap tests remain required.
Every deletion adapter must also reject actual account-key aliasing with its
rent payer/refund/reward destinations; the Node codec locally rejects its own
persisted key as rent payer, while other account keys remain external inputs.
Portable and native runtime SHA paths still need frozen differential vectors;
the host transcript path is already differential-tested against RelationV2. No
existing V1 account may be reinterpreted to fill these gaps.

Run independently:

```sh
cargo test --manifest-path crates/clutch-general-v2-contract/Cargo.toml
cargo clippy --manifest-path crates/clutch-general-v2-contract/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo doc --manifest-path crates/clutch-general-v2-contract/Cargo.toml --no-deps
```
