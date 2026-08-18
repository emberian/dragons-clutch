# Dragon's Clutch research agenda

Status: hypothesis ledger. Research may narrow or reject features; it does not
silently promote them into protocol semantics.

## 1. Method

Each workstream produces four things:

1. an exact question and bounded hypothesis;
2. a reference model or primary-source dossier;
3. a falsifier, adversarial corpus, or comparison arm;
4. a decision record stating promote, narrow, redesign, or reject.

Negative results are first-class. A smaller exact protocol is preferable to a
larger design supported by names, simulations, or unexamined solver assumptions.

## 2. R1: finite state-space compiler

### Question

What smallest closed Source/Window/Statistic/Partition language expresses useful
crypto-native bounded risk while preserving canonical, exhaustive, disjoint,
ordered, nonempty state partitions?

### Hypothesis

Terminal/TWAP bands, extrema/drawdown/variance regimes, synchronized relative
performance, and a small categorical protocol-state enum cover a useful V1 wedge
without arbitrary bytecode.

### Artifacts and falsifiers

- mathematical grammar and exact unit system;
- equivalence/canonicalization procedure and boundary corpus;
- approximation error for standard crash/range/tail payoff shapes;
- semantically equal but byte-distinct attempts;
- unit, scale, empty-cell, interval-ambiguity, and product-partition attacks.

Promotion requires constructive exhaustiveness/disjointness evidence and a user-
meaningful product family. Remove variants that cannot meet both.

## 3. R2: basis assets and hybrid representation

### Question

Does internal Position accounting plus optional Token-2022 materialization reach a
better cost/composability point than internal-only or always-materialized claims?

### Hypothesis

The hybrid preserves ordinary asset escape and external venue compatibility while
avoiding `O(n)` token accounts/CPIs on the native path.

### Artifacts and falsifiers

- exact supply/solvency model including direct external burns;
- two incompatible synthetic Realm profiles;
- lifecycle and hostile CPI tests;
- measured cost of three representation controls;
- orphan, donation, mint-profile, and reconciliation attacks.

Reject or redesign if supply ownership cannot stay singular or Token-2022 profile
semantics make the generic Realm abstraction dishonest.

## 4. R3: specialized batch relation

### Question

Which transparent coupled clearing relation is expressive enough for useful
single-Egg and payoff-shape trading but simple enough to verify exactly on Solana?

### Hypothesis

A closed relation built from simplex validation, bounded dot products, virtual
complete-set conversion, typed asset folds, and deterministic div/rem allocation
can outperform unrelated per-Egg books in coherence without a generic matching VM.

### Artifacts and falsifiers

- canonical `BatchRelationV1` domain and witness;
- exhaustive tiny-book oracle;
- curve/dual/LP candidate constructors as non-authoritative controls;
- exact score, page-resume, allocation, fee, and settlement models;
- withheld, fragmented, self-crossed, tied, invalid, and single-atom-mutated books;
- coupled relation versus independent per-Egg venue control.

The first success may be single-Egg coupled clearing only. Portfolio intents earn
promotion only when their verification and allocation relation stays bounded.

## 5. R4: optimality certificates and tractable fragments

### Question

Can an admitted divisible intent fragment provide a compact primal/dual or other
certificate that establishes a stronger result than best submitted candidate?

### Hypothesis

Some single-Egg or restricted proportional portfolio fragments may have exact
dual certificates or total-unimodularity structure, but unrestricted all-or-none
baskets do not belong in V1.

### Artifacts and falsifiers

- explicit optimization problem and rational/integer domains;
- counterexample search for integrality gaps;
- certificate size and SBF verification measurements;
- proof that the certificate objective exactly matches the public score;
- refusal corpus for unsupported order families.

Until all of these close, documentation uses “best valid submitted candidate.”

## 6. R5: shared authenticated path monoids

### Question

Which path statistics have compact conservative associative summaries that remain
authenticatable, repairable, retainable, and useful across many Markets?

### Hypothesis

Terminal/TWAP/extrema plus a carefully proven drawdown or variance bound can share
one feed substrate; arbitrary path queries cannot.

### Artifacts and falsifiers

- summary algebra and every parenthesization test;
- exact coverage, ambiguity, repair, generation, and retention model;
- synthetic source adapter before real adapter dossiers;
- information-bound and storage/work measurements;
- unsafe midpoint, missing bucket, source-upgrade, page-recycle, and common-mode
  exposure attacks.

Remove a statistic when its information requirement cannot be honestly retained.

## 7. R6: failure payout and sabotage

### Question

What finite payout policy handles irreparable missing/ambiguous evidence without
creating an intolerable incentive to cause failure?

### Candidate arms

- equal/refund-like vector;
- compatible-cell-only vector;
- explicit invalid-data cell;
- delayed or alternate-source policy with frozen bounds.

### Required analysis

Enumerate the maximum wealth transfer from source failure for every outcome
portfolio and prior market vector. Include holder/source influence, censorship,
repair bounty, common-mode affected collateral, divisibility, and dust. There is
no canonical V1 policy until this work closes. Narrowing V1 to sources/terminal
facts with stronger completeness is an acceptable result.

## 8. R7: exact fee geometry and allocation

### Question

Does the simplex-dispersion fee improve economic neutrality and resistance to
representation games enough to justify its added explanation and arithmetic?

### Comparison arms

- zero fee;
- flat cash notional;
- decomposed single-Egg uncertainty fee;
- atomic portfolio dispersion fee;
- each with several maker/executor/treasury splits.

### Falsifiers

Complete-set addition, identical-payoff partition refinement, fragmentation,
carry resets, self-cross/Sybil rebates, counterparty grouping, per-page rounding,
route leakage, and volatile collateral denomination. Prefer the simplest control
meeting the measured floors.

## 9. R8: prepaid liveness

### Question

Can every finite mandatory future job be admitted with a worst-case booking that
survives zero future volume, maintainer disappearance, and reward-token collapse?

### Artifacts and falsifiers

- exact job graph and protected-pool state machine;
- reverse-Dutch bounty laboratory;
- O(1) shared-feed subscription reimbursement model;
- duplicate work, theft, congestion, outage, censorship, and cleanup traces;
- conditional-liveness wording tied to measured landing-cost bounds.

Future fees and Hoard principal are excluded from the hypothesis.

## 10. R9: Verus/SBF/Rocq/Lean evidence chain

### Question

Can one small safe-Rust kernel be verified under pinned Verus and compiled with
identical executable semantics under the pinned Anza SBF toolchain?

### Artifacts and falsifiers

- six-component E1 kernel and prohibited-shortcut audit;
- source/proof/vector/ELF digest chain;
- annotated/erased/unannotated resource comparison;
- independent Rocq reachable-state model and evaluator;
- optional Lean finite-relation/vector reproduction;
- property-directed mutations and cross-runtime differential corpus.

Reject single-source Verus if executable divergence, proof-only public
preconditions, assumptions, or impractical SBF resource use is required.

## 11. R10: static self-verifying client

### Question

Can an ordinary user inspect exact payoff semantics and prepare every necessary
transaction from a reproducible static bundle without a privileged backend?

### Artifacts and falsifiers

- strict generated wire contracts and release manifest;
- exact transaction/postcondition preview;
- untrusted RPC/index validation and unknown-semantics refusal;
- offline/IPFS/GitHub Pages artifact identity;
- malicious client/RPC, cache-upgrade, supply-chain, and accessibility tests.

Wallet approval remains explicit. The client neither holds a key nor schedules
background transactions.

## 12. R11: regulatory architecture as a stop gate

### Question

Which exact product, facility, clearing, intermediary, operator, sanctions,
money-transmission, state, and affiliate classifications apply to a proposed
deployment, and is any viable path available?

### Engineering contribution

Maintain factual state/funds/control diagrams, immutable-versus-discretionary
authority inventory, exact release tracks, source/manipulation analysis, audit
trail, conflict alternatives, and a mainnet gate that cannot be closed by code.

### Boundary

Research documents are issue spotting, not advice or permission. Counsel and any
relevant agency process must evaluate then-current facts. A meeting, public
comment, pending request, another project’s status, source release, proof, audit,
or operatorless aspiration is not relief.

## 13. Publication sequence

Potential research notes should appear in falsifiability order:

1. finite partition/payoff algebra and canonical vectors;
2. hybrid supply/solvency theorem and cost frontier;
3. specialized batch relation with exhaustive tiny-book results;
4. shared accumulator monoid and information lower bound;
5. failure-sabotage and liveness-capitalization studies;
6. Verus/SBF compatibility and cross-model evidence report;
7. static-client reproducibility and hostile-projection report.

Every publication states novelty conservatively, cites primary prior art, includes
negative results, identifies unproved/refinement boundaries, and preserves the
AGPL and fixture-provenance record.
