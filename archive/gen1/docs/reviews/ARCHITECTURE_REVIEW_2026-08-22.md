# Architecture review — 2026-08-22

## Verdict

Dragon's Clutch has a serious bounded Solana implementation, not a paper shell.
The recent wave exercised market creation, mock-source funding, general order
placement, freeze, streamed candidate checking, selection, entitlement, and
exact settlement. A prior run of `scripts/run_operator_trade.sh` at `e07c08a`
submitted and confirmed 54 of 54 local transactions, reloaded 1,177 account
images through the canonical codecs, and closed all six reported conservation
identities. That run is historical evidence, not evidence for the current tree,
and the second-pass review below invalidates its claim of complete root closure.

The architecture is nevertheless not complete in the sense of the original
product thesis. Its largest risks are boundary drift: model-level generality is
repeatedly described as adapter-level capability; a bounded capability profile
is described as the protocol; and generated evidence is allowed to outlive the
truth it is supposed to summarize. Those are repairable. The core exact-integer
and semantic-ownership decisions are worth preserving.

This review used the recovered session corpus, the tree beginning at `e07c08a`,
the last accepted Cycle-G seal and manifest, direct source inspection,
independent audit lanes, the static-client tests, and a fresh local operator
Trade run. The review/docs and Operator progress changes made afterward are not
attested by that artifact. This is an engineering review, not a formal
verification claim. Regulatory work is excluded.

## Second-pass findings — work resumed 2026-08-22

The pushed review checkpoint was not completion. A fresh architecture,
mechanism, rent, and local-validation pass found new release-blocking issues and
also clarified what the product actually is. The current audited-but-unsealed
repair wave therefore fails closed on two deletion routes while their versioned
successors are designed. None of this section is evidence for a deployed
artifact.

### P0 — Position zero did not prove reservation zero

Sell placement transfers reserved Eggs out of `PositionAccount` and into a
separate ACTIVE `ReservationAccount`. An all-in seller with no cash can therefore
have a locally economic-zero Position while a live order still owns all of its
Eggs. The old `ClosePosition` checked only local cash and Egg balances. The owner
could delete the Position, after which cancel, terminal release, and settlement
could no longer restore or update it; the surviving Replay account prevents a
simple Endow recreation.

The current audited source disables the final Position deletion after all
existing authentication and local-zero checks. A functional successor needs a
persisted owner/market outstanding-reservation counter (incremented at placement
and decremented exactly once at cancellation, terminal release, or consumption),
or another exhaustive aggregate proof. Requiring a resolved Market is not
sufficient because older epochs can still own reservations.

### P0 — deleting the epoch root enabled replay and stranded unlisted children

`CloseGeneralEpoch` deleted Epoch, Window, and their funding ledger. `InitEpoch`
accepts a caller-supplied `epoch_index` whenever those canonical PDAs are absent;
Market stores no monotone used-index authority. The same epoch identity and its
child PDA namespace could therefore be recreated after a nominal close.

The root close also checked only the Window's at-most-three retained candidates.
That list intentionally excludes sealed-unverified, refused, superseded, and
valid-but-noncompetitive candidates. A caller could delete the root while one
of those records or its roughly 50 KiB ClearWork remained. Every child close
authenticates the terminal Epoch, so the residual could then become permanently
uncloseable.

The current audited source disables root deletion. The keeper discovers and
closes every authenticated candidate and every full or growing ClearWork
independently, then stops with an explicit retained-root state rather than
retrying the disabled instruction. A functional successor needs both:

- a Market-owned monotone epoch cursor/generation or a durable versioned
  tombstone; and
- exhaustive persisted child counts, with candidate close requiring its
  canonical ClearWork to be absent and root retirement requiring every count to
  be zero.

The safe interim lock is 7,161,840 lamports for a ledgered Epoch + Window +
funding ledger (5,679,360 without the optional ledger). That is preferable to
identity replay or permanently stranded child principal.

### P0 — ScoreV1 rewards a risk-free complete-set wash

The primary candidate score sums
`p_i * (S - p_i) * direct_flow_i`. It is not invariant to adding a constant
complete-set payoff, although that component carries no contingent risk. At the
binary midpoint, two distinct keys crossing `q` lots of `(1,1)` earn
`50,000,000q` primary-score units while the composite fee correctly charges the
cash-equivalent basket zero. A genuine one-percent-tail Egg trade of the same
size earns only `990,000q`. Same-owner overlap does not help against two keys,
and the later `distinct_owners` component rewards key fragmentation again.

This does not break conservation, but it makes candidate selection economically
gameable. Keep the frozen ScoreV1 only as an experimental profile. The
implemented ScoreV2-Q economic prefix is exactly invariant to constant
complete-set shifts; its later fields select the min-zero, lower-churn
representative. It does not use pubkey count and has executable quotient, tail,
padding, refinement, overflow, and representation tests. Its precise claim is
**representation-neutral after owner-blind admission**, not person-neutral:
an honest two-key cross and one controller using two keys can be byte-identical.
Owner-tagged normalization, price quality, and fee/reward composition remain
promotion gates before it controls a public market.

### P0 — degree-two/three price admission is not a no-arbitrage certificate

For degree zero and one, simplex membership has a representing-measure result.
For multi-span degree two and three, the implemented finite moment-cone family
is necessary but not sufficient for membership in the true spline moment cone.
The current code says so, but public descriptions were broader. Until a complete
witness or deliberately sufficient inner representation exists, the first
coupled product profile should admit degree zero/one only. Degree two/three
evaluation and redemption can remain useful research capabilities without being
marketed as a fully coherent public price surface.

### What the instrument is, and whether it is good

The compelling primitive is a fully collateralized call auction over a finite
basis of bounded state-contingent claims. Degree-zero Eggs are categorical
Arrow-like claims; degree-one Eggs are overlapping hats. Coefficient vectors
express crash, range, tent, and capped-directional payoffs, and one portfolio
order removes execution leg risk. Settlement nevertheless credits separate Egg
balances: there is no transferable tent wrapper.

That kernel is good for a narrow wedge: recurring four-to-eight-state terminal
price or drawdown hedges for thin crypto assets, preferably in stable collateral.
It is not yet a good live venue. The shipped interaction is one hard-coded local
Friday terminal-price fixture; there is no product-path compiler, recurring
Series, real solver incentive, funded liquidity mechanism, wallet transaction
client, live source identity, or same-market source-to-redemption demonstration.
The public site now calls the Friday statistic terminal rather than TWAP,
describes basis prices rather than a unique continuous density, and distinguishes
an atomic portfolio order from a single transferable asset.

### Validation and rent conclusions

The exact pushed checkpoint had no coherent full default/mock SVM run: three
tests still encoded pre-repair candidate-retention semantics. Critical signed
validator lanes (`run_general_committed`, keeper crash/resume, paces dry-run,
Operator Trade/replay) were also absent from the baseline manifest. The current
keeper gate now survives a deliberate mid-walk kill, resumes from chain state,
closes every currently safe leaf, and reaches the same fail-closed `Blocked`
state across a second fresh restart while retaining the replay anchors. A
cloned Pyth validator is useful substrate. More importantly, the captured
deployed router and receiver now execute through exact Upgradeable Loader
accounts in a feature-gated local bank. At `169a1ba`, the router first persists
a Verified locally signed 13-of-19 synthetic VAA. In a later transaction, real
`PostUpdate` and Clutch append execute adjacently and atomically. Missing
adjacency refuses with the archive unchanged; wrong Config or feed rolls back
both the receiver-created update and archive. At `5ab10b0`, the one-record
archive seals and resolves payout cell 1 because the entire admitted
conservative interval `[99,980,929, 100,019,071]` lies in that cell. The
Program/ProgramData bytes are captured deployment bytes; guardian and Config
state is freshly initialized local fixture state. This is real provider-program
and ABI/crypto execution over a synthetic local observation, not devnet price
evidence. Redemption and a multi-boundary shared window remain separate claims.

Commit `361eafd` additionally runs that provider seam through signed JSON-RPC
transactions on a pinned, patched, listener-audited Agave validator. The clean
committed campaign confirms 13 transactions: real router verification, two
exact atomic rollback negatives, adjacent receiver PostUpdate plus Clutch
append, archive seal, and categorical resolution. This removes in-process
`ProgramTest` as the only execution substrate for the seam. It still uses
synthetic local guardians and a synthetic observation, so it does not establish
provider availability, a network-signed upgraded 3-of-5 payload, or production
source admission. Exact hashes and compute units are in
`LOCAL_REAL_PYTH_SIGNED_RPC_2026-08-22.md`.

The current repair wave passed three byte-identical artifact builds, including
a relocated Cargo home: ELF
`193c08723eaefeff9a1c2aa53c9e3feb58960a919fb0bbb7ca5da3bd817aa95b`,
2,082,320 bytes, with dependency/syscall, loader-shape, and final-LTO frame
checks green. It costs 14.49529272 SOL of persistent loader rent; ten SOL is
insufficient by 4.49529272 SOL before fees. Static deduplication removed 54,344
bytes from `a6381fbe…`; eliminating the redundant CreateMarket decoder round
trip removed another 23,408 bytes / 0.16291968 SOL. Getting under ten SOL still
requires an ELF no larger than 1,436,444 bytes, another 645,876-byte reduction.
Larger wins require
capability profiles and active-width account formats, not weakened exactness:
binary ClearWork alone can shrink by 33,376 bytes per candidate, and receipt
pages can remove most of the per-receipt account overhead. Historical rent
evidence remains immutable; current-tree inventories must separately correct
Direct Epoch V4 to 673 bytes and include the 404-byte SourceSpec V2.

After freezing that source, the complete default production-inert SVM profile
(one unreachable fixture release; no production release) passed 165 tests with
zero failures against the audited `193c0872…` ELF. The
separately compiled `non-production-mock-source` profile passed 168 tests with
zero failures against its distinct 2,110,240-byte ELF `342fdfcb…`. The profile
distinction is part of the claim: the latter exercises funded laboratory
source/value paths and is not production-source evidence. Signed Operator gates
and a current manifest/second-host seal still remain before this wave can be
considered a replacement evidence baseline.

## Decisions to keep

1. **Eggcrate as a narrow semantic owner.** Safe, fixed-layout, allocation-free,
   total kernel code with explicit refusals is the right center. Solana account
   memory, token CPI, source SDKs, and runtime behavior belong in the adapter.
2. **Exact scaled integers and one named rounding boundary.** The rounding pot
   and refusal routes are complexity with a purpose. Replacing them with float
   or per-call convenience rounding would make conservation unauditable.
3. **Immutable Realm and Terms facts.** Collateral, source procedure, basis,
   price grid, and failure behavior should not be mutable venue policy.
4. **Portfolio-native coupled clearing.** A simplex price and one checked
   relation are stronger than independent binary books that can contradict one
   another.
5. **Static clients as untrusted projections.** The explanatory site, offline
   Glass inspector, loopback Operator Bench, and eventual transaction client
   should remain separate products with different trust and capability labels.
6. **Solver-agnostic checked candidates.** The relation, not an operator,
   decides validity. The intended product vocabulary remains “best valid
   submitted candidate.” The current unsealed repair makes that statement true
   over candidates whose full verification completes before the shared
   deadline; the remaining fairness limit is stated below.

## Priority findings

### Repaired P0 — unverified claims no longer control retention

The accepted Cycle-G candidate registry admitted at most three records during
`SealCandidate`, before the streamed relation has checked them. Admission and
displacement compare caller-claimed score components. A malicious or merely
wrong high claim can therefore fill the three slots or evict a verified valid
candidate; `FinalizeSelection` ignores every non-`VERIFIED` record and can then
lapse the epoch. Equal-component candidates also cannot use the eventual
full-width verified digest to compete for admission. The shared submission and
finalization deadline leaves no guaranteed verification interval.

That was a protocol liveness/correctness defect, not wording polish. The
accepted artifact selects the **best verified retained candidate**, not the
best valid submitted candidate.

The current unsealed source implements the layout-preserving successor.
`SealCandidate` now has exactly four accounts and only freezes the feed;
`CompleteClearWork` receives the window, Clock, and exact retained-record suffix
and atomically admits only a fully recomputed `FullScoreV1`. A noncompetitive
valid record remains `VERIFIED`, and displacement preserves the old feed for
terminal closure. Seven candidate-selection, two lifecycle, and four cone-gate
bank tests cover writable/excess Seal shapes, unverified exclusion, full-width
digest ordering, displacement, deadline refusal, tampering, and lapse.

A complete fairness guarantee for every feed sealed before submission close
still needs separate submission-close and verification/finalization deadlines,
hence a new window version. The current repaired claim is “best valid submitted
candidate among those fully verified before the shared deadline.” Increasing
the registry size would not close this remaining scheduling limit.

### Repaired P0 — receipt bound; max direct witness now executes

Candidate feeds and the relation admit up to 416 pairing slices, but
`SettlementReceiptAccount::validate` refuses every `slice_index >= 128`.
Therefore an otherwise valid selected witness using slice 128 or later cannot
mint its receipt. The current unsealed source aligns the receipt bound with
`MAX_SLICES`, with direct codec tests at 415/416.

The fixed-book SBF refutation campaign now holds the maximum four-page/64-order
book constant and measures direct per-slice Entitle at 745,595 CU for one slice,
763,615 at 128, 763,755 for slice index 128 in a 129-slice witness, and 803,935
at 416. The prior rough projection above 1.4M was wrong because its source rows
co-varied page count and witness width. This closes the direct max-witness
capacity concern and executes the corrected slice-128 receipt. It does not yet
cover maximum-page portfolio full-pair, virtual, or inexact routes; those remain
separate settlement-envelope measurements, not reasons to relabel this result.

### Repaired P0 — liveness ledgers and the current Fold plan are measured

Onchain ResolutionWork pays the base reward once per successful **Fold call**,
while the sealed liveness model's batched path priced one external reward per
transaction and hard-coded `runtime_schedule_matches_policy: true`. The runtime
minimum-deposit rule conservatively budgets 32 one-record folds. At the sealed
constants it requires 49,431,920 lamports (rent 10,801,920 + 32 × 1,160,000 +
the 1,510,000 finalize maximum), or 49,661,920 when the model's external Begin
quote is included—not the optimization handoff's 15,291,920 cold-outlay number.

The current profile now derives the 49,431,920 protocol prefund separately
from named-plan payouts/refunds and external keeper budget. For eight Fold(4)
calls in a `[6,2]` transaction plan, successful runtime payout is 10,790,000 and
refund is 38,641,920; the old 15,291,920 figure is retained only as explicitly
invalid. An identity-bound current-tree campaign executes those transactions at
514,332 CU / 1,228 bytes and 171,765 CU / 704 bytes. Their external Fold budget
is 1,090,000 lamports; measured Begin + folds + Finalize is 1,610,000. It proves
byte equality with eight singleton calls and whole-transaction rollback on an
invalid fourth call. The row remains `UNSEALED_CURRENT_TREE`; Cycle G's sealed
unmeasured row is not relabeled. This repairs the accounting authority without
exposing Hoard principal or future fees. Wider folds still require a versioned
reward/minimum-deposit decision, not just a constant edit.

### P0 — one live status had several competing owners

At review start, `CURRENT_TRUTH.md` said it superseded `GOAL.md` and
`CODEX_HANDOFF.md`, while its snapshot still said the Cycle-G manifest and
Persvati attestation were owed. `GOAL.md` recorded that they closed;
`CODEX_HANDOFF.md` called the protocol capability-complete; and the root README
predated the general settlement/operator wave. The live documents are being
repaired in this review, while older reports remain named historical snapshots.

The last accepted `MANIFEST.baseline.json` ran 101 gates with 100 matching
expectations, yet its free-form `claims.not_attested` prose still says Direct V3
and terminal closure are missing. The manifest checker does not detect that
semantic drift. A green manifest is therefore not a sufficient status reader.

**Decision:** `CURRENT_TRUTH.md` owns current implementation/evidence status;
`PROJECT.md` owns the product; this review owns the new queue;
`GOAL.md` and handoffs are historical trails. Generated manifests may bind
bytes and gate results, but free-form claims need an independently checked
source or must not pretend to be current status.

### P0 — the public evidence tree crossed the greenfield boundary

Two linked manipulation-cost tables were byte-for-byte copies of
`degg-research` outputs. They arrived without an explicit import decision, their
generator, or a destination provenance manifest. Two design documents also
reproduced exact Breadstuffs Lean declarations while claiming that no code
moved. The current review removes the tables and rewrites the declarations as
source-independent mathematics; `docs/PROVENANCE.md` preserves the exact
digests and source commits so the incident is auditable.

No prohibited runtime import or dependency was found. Public release still
needs a third-party notice bundle, pinned generator environments, and an
advisory-database audit. The Pages workflow is now manual-only and validates
the static tree before upload, but its action dependencies still need full
commit pins before an authorized publication. Greenfield review must cover
documentation and published fixtures, not just Cargo graphs.

### P0 — collateral genericity stops at the adapter

`programs/solana-layout/src/collateral.rs` admits legacy SPL Token and
Token-2022 profiles. The documented DREGG dogfood profile is legacy SPL Token.
But `token::require_drivable_collateral` and
`instructions::market_init::admit_collateral` reject every non-Token-2022
collateral and require `ImmutableOwner`. Market creation uses the same token
program role for collateral custody and Token-2022 outcome mints.

Consequences:

- the data model is collateral-program-generic;
- the current deployable adapter is not;
- the repository's offline DREGG reference Realm can be created as state but
  cannot found a market through this adapter;
- adding a DREGG-specific exception would violate the architecture.

**Better successor:** a versioned market-creation/account ABI with two explicit
program roles: the Realm-selected collateral token program and the protocol's
claim-token program. Keep Token-2022 Eggs if desired, but let the collateral CPI
adapter implement separately reviewed legacy and Token-2022 profiles. Preserve
the current V1 decoder and addresses; do not mutate a frozen layout in place.

### P0 — “capability-complete” hides honest admission restrictions

General settlement contains deliberate `NotYetImplemented` refusals. Portfolio
slice consideration must divide the price scale exactly. For inexact single-Egg
orders, the selected candidate is admitted only when each participating owner
has exactly one filled order, because the relation rounds at the owner boundary
while the consumption seam realizes per-order ledgers.

These are not accidental stubs and must not be deleted. They are, however,
material product restrictions. The general successor is an owner-aggregated
entitlement ledger (or a new relation version whose one rounding boundary is
per order). The current claim should be “complete over its admitted settlement
domain,” not unqualified capability completeness.

### P1 — M2's proposed locator solves only half of the complexity

`EntitleSlice` is O(book + witness), not merely O(book):

- `locate_pair` walks every page to authenticate the frozen set and translate
  live ranks past tombstones into stored slots;
- `scan_witness` walks every declared slice to recompute both order totals,
  pair multiplicity, and exclusivity;
- virtual slices perform the analogous `scan_end_total`;
- inexact conversions may scan every fill to count participating orders.

A rank-to-page/slot table alone cannot make the route O(slice), and a locator
carried by an untrusted instruction is not authenticated by a page header. The
successor needs two checked indexes: a frozen-set location index and a
candidate-bound adjacency/aggregate index. They can be built during the already
authenticated walk, sealed into a versioned account, and consumed by
entitlement. Tests must mutate each coordinate, total, and adjacency boundary.

### P1 — fixed bounds need an explicit capacity-profile architecture

The current artifact structurally fixes 16 outcomes/knots, 64 orders in four
pages, 416 witness slices, and degree at most three. The executed scale evidence
does not cover their full Cartesian maximum. The new direct Entitle campaign
does combine 64 orders/four pages with 416 slices, but other route families,
portfolio/virtual/inexact shapes, and the outcome/basis axes remain separately
measured. Fixed bounds are appropriate for SBF, wire size, and total functions.
Treating either the constants or separately measured maxima as one proven
product envelope is not.

Define a versioned capacity profile that names all coupled widths, account
formats, CU envelopes, and supported basis family. A larger profile can be a
separate program artifact or versioned account family. Do not add dynamic
allocation to Eggcrate, and do not promise arbitrary width without a measured
transaction/account construction.

### P1 — the 2.15 MB program is a release and maintenance risk

The program workspace compiles one ELF containing direct clearing, general
clearing, two source generations, resolution work, terminal closure, artifact
upload, token integration, and laboratory-conditioned registry code. Source is
split into modules, but deployment capability is not. This increases rent,
audit surface, rebuild time, and the blast radius of unrelated changes.

Do not immediately split into many CPI-coupled programs: atomicity, account
locks, upgrade coordination, and CPI overhead may be worse. First measure
deployable feature profiles with identical semantic owners:

1. transparent core plus one source generation;
2. general clearing as an optional sibling artifact;
3. legacy/direct compatibility only where a real migration needs it.

If dead-code elimination cannot produce meaningful profile separation, then
evaluate a small kernel adapter plus venue/source siblings with explicit CPI
postcondition checks.

### P1 — the payoff compiler is not in the product path

The original crown jewel is compilation from objective state/path predicates
and payoff shapes into immutable Terms, basis coefficients, and market-creation
artifacts. Today `research/bspline-shape-compiler` and the Glass unsigned Terms
preview are useful but separate. The operator demo founds one hard-coded Friday
market.

Promote a deterministic, versioned host compiler only after its output is
checked by onchain Terms validation. The compiler may remain untrusted; its
certificate and canonical bytes must be independently checkable. This gives
users generality without moving a large compiler onchain.

### P1 — source integration is locally real and production-incomplete

The adapter now authenticates the exact reviewed seven-account `post_update`
shape, discriminator, effective privileges, writable update, and canonical
Clock owner. Its laboratory receiver writes the 134-byte update in the same
transaction, and rollback covers both update and archive. A V2 archive carrying
normal nonzero-confidence intervals resolves degree-zero categorical Terms
through the canonical accumulator/reference authority: the 14-account bank
success costs 166,465 CU, while boundary ambiguity and legacy buffer shapes
refuse without state change.

The default artifact still has no production release; its sole default row is
the fabricated off-curve fixture. A separate, unmistakably non-production
feature pins captured deployed receiver/router binaries and their exact
Upgradeable Loader Program/ProgramData bodies. The real router first persists
a Verified locally signed 13-of-19 synthetic VAA. In a later transaction, the
real receiver's `PostUpdate` and Clutch append execute adjacently and atomically.
Missing adjacency refuses with the archive unchanged; wrong Config or feed
rolls back both the receiver-created update and archive. The one-record archive
then seals and resolves payout cell 1 because the entire admitted conservative
interval `[99,980,929, 100,019,071]` lies in that cell.
This closes the provider-program seam through one-bucket resolution over a
synthetic local observation; it does not establish devnet price data, provider
availability, the current upgraded 3-of-5 trust substrate, a semantic feed
profile, redemption, or a multi-boundary shared window. Production identity
constants, feed choice, stability interval, and trust floor remain deliberately
unpinned. This is a release/profile boundary, not a reason to weaken source
authentication.

### P2 — local operation works but hides long protocol waits

The fresh Trade run passed, but the 260-slot freeze window plus the fixed
1,000-slot candidate window made a single interaction take several minutes.
The browser receives exact clock events; the console previously continued to
say “walking” throughout the selection wait, and the README did not set the
expectation. Progress must name the current gate and target slot. Local time
warping may be a separate fast laboratory profile, never evidence for real-clock
liveness.

## Optimization and repair order

1. **Completed in current unsealed source:** replace claim-ranked Seal admission
   with verified-only Complete admission and add prefill, displacement, and
   deadline adversarial campaigns. A successor window with separate submission
   and verification/finalization deadlines remains the fairness generalization.
2. **Completed in current unsealed evidence:** resolve the 128-versus-416
   receipt contradiction
   and execute a true maximum-book/max-witness direct Entitle campaign. Preserve
   the narrower claim; portfolio full-pair, virtual, and inexact maxima remain.
3. **Completed in current unsealed evidence:** repair ResolutionWork liveness
   economics and mechanically compare protocol prefund, named-plan
   payouts/refunds, and external keeper costs. Execute the Fold(4) `[6,2]`
   composed transactions, packet boundary, singleton equivalence, and hostile
   rollback without relabeling sealed evidence.
4. Fix the status/control plane and make measurements reproducible from one
   documented command.
5. Design the two-index Entitle successor; do not land a locator-only ABI.
6. Prototype partial ClearWork decode behind a new codec version. Preserve the
   hostile-byte corpus and whole-state resume weld before considering it an
   optimization.
7. Combine FreezeEpoch's repeated authenticated page walks before reaching for
   a staged ABI.
8. Revisit stored-bump derivation only as an explicit trust-boundary change:
   prove canonical construction/bump preservation, name the upgrade/genesis
   assumption, define the changed refusal, and use the pinned upstream helper.
9. Introduce capacity and deployable capability profiles before increasing
   constants or splitting programs.
10. Build the collateral-program-generic market successor and the untrusted
   payoff compiler as product capabilities.

## Frontend and documentation architecture

Present four surfaces, not one ambiguous “frontend”:

| Surface | Current capability | Trust statement |
| --- | --- | --- |
| Microsite (`site/`) | Literate explanation, no network dependency | Publication only; no chain state or trading |
| Glass (`apps/static-client`) | Offline inspection and unsigned intent bytes | Untrusted, no RPC/wallet/signing/submission |
| Operator Bench (`apps/operator` + `operatord`) | Local signed loopback trading/watch harness | Test daemon holds ephemeral local signers; not a wallet or deployment client |
| Future transaction client | Not built | Must derive accounts from chain state, display exact release identity, and never treat an index as authority |

The README should lead with what works now, what does not, a five-minute map of
the architecture, and exact local commands. Deep proof and evidence history
belongs behind links. The microsite should gain a concise status/try-it-locally
page and keep its existing literate explanations.

At review start, Operator Bench's loopback mutation endpoint accepted arbitrary
JSON POSTs without Host, Origin, media-type, or session-capability checks. That
must stay closed before generalization. Browser behavior also needs a real DOM
test: the local trade script tests the HTTP/event lifecycle, not JavaScript
interaction or visual rendering.

This review adds those loopback request guards, a process-local
capability cookie, exact decimal-string/`BigInt` display support, persistent
slider interaction, and basic accessibility/source tests. The event schema
still serializes many Rust `u64` values as JSON numbers, so exact generalized
display is not closed until those fields cross the daemon boundary as canonical
decimal strings and receive schema tests. Browser/visual QA also remains open
because no in-app browser runtime was available for this review.

## Release boundary

No release manifest, deployed program, official frontend URL, production source
identity, nonzero fee policy, or value-bearing market exists. Local Trade
success is SBF-executed loopback evidence. Devnet faucet requests, deployment,
and funding mutate an external system and require a current explicit act and
scope; they are not a continuation of this offline review by implication.
