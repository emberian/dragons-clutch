# V1 dependency-ordered execution backlog

Status date: 2026-08-19. Checkboxes record accepted evidence, not effort and not
authorization. [`../CURRENT_TRUTH.md`](../CURRENT_TRUTH.md) is the status and
claim boundary; [`../PROJECT.md`](../PROJECT.md) is the product brief.

Do not skip a gate because later code already exists. Later work remains useful
model/prototype material, but it cannot promote the lifecycle while an earlier
value, authority, or reachability dependency is open.

## 0. Preserve the real foundation

- [x] Canonical product brief separates categorical primitives, portfolios,
  collateral Realms, protected Hoard principal, and an untrusted static client.
- [x] Host semantic crates exist for claim transitions, source-neutral
  accumulation, and the coupled batch relation.
- [x] Fixed Solana layouts, hostile-byte tests, an SBF processor, real local-bank
  Token-2022 cases, and a loopback differential exist.
- [x] A hand-written Lean model contains checked semantic-plane theorems with an
  explicit unproved Rust/refinement boundary.
- [x] The batch relation has bounded exhaustive/adversarial host campaigns and a
  bounded-memory streaming verifier.
- [x] The repository has semantic vectors, a manifest generator, an artifact
  audit, and local-only harnesses.
- [x] None of the items above is classified as a complete protocol, verified
  Rust, blank-bank lifecycle, release, deployment, or legal authorization.

## 1. Integrate one honest baseline

This is the immediate coordination gate while parallel lanes are active.

- [x] Commit the source-admission kernel, settlement preflight, signed runner,
  narrow transfer refinement, deterministic invariant campaign, and artifact-
  audit tool as separate semantic-owner changes.
- [ ] Review and commit the pooled-custody repair and claim-compiler work; treat
  generated audit evidence as evidence for the exact final ELF only, never as
  durable source input.
- [ ] Resolve every staged/unstaged conflict; format and run focused checks in
  each owning crate.
- [ ] Regenerate the SBF harness oracle for the new custody semantics. A stale
  differential whose two sides still encode the old economics is a failure.
- [ ] Run the loopback bring-up and isolated real Token-2022 bank suite on the
  integrated tree; archive exact output and ELF digest.
- [ ] Run the signed committed harness and preserve its honest
  `genesis_assisted`/stranded-value labels until later gates close.
- [ ] Run the final-ELF dependency/symbol audit on the exact ELF produced by the
  gate.
- [ ] Emit and commit a clean schema-v2 `MANIFEST.baseline.json`, then prove it
  still checks after the manifest-only commit.

Acceptance: one clean commit identity, one content identity, one SBF ELF, and
one noncontradictory evidence set. A dirty `--allow-dirty` manifest is diagnostic
only.

## 2. Close pooled custody and cash exit

Dependency: gate 1.

- [ ] Freeze the market-wide equation
  `custody = locked + free_cash + reserved_cash + unowned_surplus` and exact
  local deltas in one normative design/test owner. Reserved Eggs remain in the
  claim-supply identity, not this collateral equation.
- [ ] Make `Endow` an authenticated exact Token-2022 actor-to-Hoard deposit.
- [ ] Make Split, Merge, and internal Redemption token-neutral accounting
  reclassifications.
- [ ] Keep `collateral_cap` a bound on locked claim backing, not on unrelated
  deposited free cash.
- [ ] Define reserved cash/claims now, even if order settlement lands later; no
  implicit or unenumerated ownership term.
- [ ] Implement `Withdraw` for unreserved free cash with exact Hoard-to-owner
  CPI, profile/authority admission, replay, and late-failure rollback.
- [ ] Treat unsolicited Hoard inflow as unowned surplus. Prove a one-atom
  donation cannot block Merge, Withdraw, or either redemption mode and cannot be
  swept as fees/treasury.
- [ ] Exercise at least two positions and show their operations cannot spend one
  another's free or reserved cash.

Acceptance: focused host tests, real Token-2022 bank tests, and committed-state
reloads close every term and kill deliberate debit/credit/authority mutations.

## 3. Establish one outcome-token truth and both claimant exits

Dependency: gate 2 for the payout path.

- [ ] Decide and document the ordinary holder-burn policy: either the real token
  program refuses out-of-band burns, or a permissionless repair/donation
  transition safely recognizes reduced liability.
- [ ] Remove or redefine `ExternalAccount`/`SupplyLedger.external_supply` so no
  stale shadow can globally brick an outcome after a valid token-program action.
- [ ] Keep actual mint supply and actual token-account possession authoritative
  at the composability boundary.
- [ ] Implement possession-authorized `RedeemExternal` for a holder with no
  originating Position: burn exactly the winning Egg quantity, pay exact
  collateral, reduce locked backing exactly once, and replay safely.
- [ ] Preserve direct transferability before resolution; a recipient wallet must
  be able to redeem after resolution.
- [ ] Define losing external-claim cleanup without granting a collateral or
  surplus claim.

Acceptance: a real-bank test transfers a winning Egg to a fresh wallet, resolves,
redeems externally, and closes mint supply, token balance, liability, payout,
and Hoard accounting. A separate direct-burn campaign either refuses at the
token program or follows the specified live repair path.

## 4. Make every required account permissionlessly constructible

Dependency: the state equations and authority meanings in gates 2-3.

- [ ] Make `CreateMarket` create its eight canonical state PDAs from absent
  accounts using authenticated payer funds, System-program CPI, exact rent,
  `invoke_signed`, and atomic rollback.
- [ ] Add a permissionless initializer for each later user's Position/replay
  state (and any remaining external-account state if gate 3 retains it).
- [ ] Create and bind Realm/Profile/source-spec/feed state without fixture-owned
  program bytes.
- [ ] Create Epoch, pages, CandidateFeed, ClearWork, candidate, pot, receipt,
  and resolution state through bounded/resumable instructions.
- [ ] For ClearWork growth beyond one-instruction limits, make every intermediate
  allocation state tagged, rent-correct, resumable, and economically inert.
- [ ] Add duplicate, wrong-bump, pre-funded, close/reopen, partial-create, rent,
  and late-CPI rollback regressions.

Acceptance: a blank local bank contains only system/sysvar/token programs,
the Clutch ELF, funded ephemeral signers, and a chosen synthetic collateral mint.
Every other program-owned account is created by committed public instructions;
genesis injection is zero.

## 5. Join a concrete authenticated source to resolution

Dependency: source/feed account construction in gate 4.

- [ ] Select exactly one first V1 source profile from primary official material;
  pin program id, loader/deployment generation, exact data account semantics,
  parser source/version, orientation, scale, finality, retention, and canonical
  bucket rule.
- [ ] Establish that at most one finalized record can qualify for each bucket.
  If the selected source cannot establish historical uniqueness, reject it.
- [ ] Implement hostile-byte parsing with no allocation, floats, unchecked casts,
  panics, caller-provided clock, or caller-selected fallback.
- [ ] Create immutable SourceSpec and versioned Feed/archive accounts; recompute
  their commitments in-program.
- [ ] Replace caller-authored `FeedAdvance` bytes with authenticated source
  admission against the canonical Clock sysvar.
- [ ] Replace the independent Resolve buffer with the exact sealed archive
  commitment accepted by the feed.
- [ ] Separate market-global resolution replay from owner redemption replay.
- [ ] Test wrong key/owner/program/deployment/parser/generation/grid/sequence,
  stale/future time, confidence overflow, archive substitution, gaps, ambiguous
  boundary intervals, upgrades, and rollback.

Acceptance: two syntactically valid but distinct record sets for the same bucket
cannot both qualify; Resolve consumes only the committed admitted history. The
claim remains “authenticated to the selected source under its named
assumptions,” never “oracle-free.”

## 6. Fund and settle the coupled onchain venue

Dependencies: gates 2-5.

### 6.1 Order reservation and epoch truth

- [ ] Placement atomically moves exact cash/claim quantity into a reservation
  owned by `(market, epoch, owner, generation, order)`.
- [ ] Cancellation/expiry releases only unused reservation and cannot race a
  frozen epoch.
- [ ] Freeze one complete page set, populated count, live count after
  tombstones, price grid, full policy preimage/digest, and full-width identities.
- [ ] Prove owner-tag interning/bijection or replace it with an authenticated
  lossless owner representation.

### 6.2 Candidate and checkpoint

- [ ] Replace relation `u64` identity placeholders with full onchain identities
  or a proved injective encoding.
- [ ] Give the streaming checkpoint a stable versioned hostile-byte codec; never
  persist `repr(Rust)` enums/bools by cast.
- [ ] Run the streaming relation against the exact frozen live order set and
  compare its verdict with the host relation.
- [ ] Commit the bounded candidate submission set, verify candidates, and select
  the best valid submitted candidate under the frozen total order.

### 6.3 Entitlement and lazy consumption

- [ ] Before resolution, freeze a complete immutable pot/receipt entitlement set
  covering every accepted fill and refund. An executor may not omit unfavorable
  legs.
- [ ] After that freeze, allow each entitlement to be consumed exactly once even
  if the market has resolved; resolution must not create or alter an entitlement.
- [ ] Transfer exact claims and consideration, release only unused reservations,
  and close final pot/receipt/accounting identities in every consumption order.
- [ ] Prove rollback on late state/CPI errors and idempotence under replay.

Acceptance: a committed multi-owner local-bank epoch containing single-Egg and
portfolio intents reserves, freezes, verifies, selects, resolves, lazily settles
in several permutations, refunds, and closes with no stranded owned asset.

## 7. Prepay liveness and freeze economics

Dependencies: exact instruction/account/resource shapes from gates 4-6.

- [ ] Enumerate every mandatory create, observe, archive, repair, candidate,
  finalize, entitlement, settlement, refund, withdrawal, and cleanup action.
- [ ] Measure CU, transaction/account bytes, rent, prioritization assumptions,
  and maximum repetitions on final SBF paths.
- [ ] Capitalize worst-case unfinished work at admission under zero future
  volume; later fees are not liveness backing.
- [ ] Keep principal, owner cash, reservations, rent, liveness endowment, fees,
  and treasury in nonaliasing ownership phases.
- [ ] Decide failure payout and repair incentives under sabotage.
- [ ] Freeze the fee base, rate/carry/rounding, recipient split, executor cap,
  and withdrawal rules only after exact adversarial and fragmentation models.
- [ ] Preserve fee-free external venue interoperability for standard
  materialized Eggs.

Acceptance: every admitted market can finish without future traders, token-price
appreciation, treasury discretion, or access to Hoard principal.

## 8. Generality without new liability primitives

This can proceed in parallel with gates 4-7 after primitive accounting is fixed.

- [ ] Freeze categorical primitive Eggs as the only ordinary external claim
  liability.
- [ ] Specify a compiler from categorical, range, triangle, capped-linear, and
  finite Gaussian-like payout requests into exact nonnegative integer portfolios
  over primitive Eggs.
- [ ] Emit approximation, rounding, support, maximum-payout, and collateral
  certificates with canonical bytes.
- [ ] Verify the certificate in bounded Rust and reproduce its mathematics in
  the independent model.
- [ ] Keep unsupported degree, negative payoff, nonexhaustive partition,
  overflow, and approximation-budget cases refusing.
- [ ] Reserve native fractional payout vectors for an explicitly frozen
  ambiguity/failure policy; do not make them a dependency of normal trading.

Acceptance: the compiled portfolio is no more collateral-intensive than its
stated bound, redeems exactly according to its integer coefficients, and has a
reproducible maximum-error certificate against the requested finite payout.

## 9. Formal, adversarial, and artifact promotion

These gates cover different planes; none substitutes for another.

- [ ] For each accepted Verus result, record theorem, production-source digest,
  tool/config/dependency digests, assumptions, red mutations, and exact
  unverified call/runtime boundary.
- [ ] Extend the Lean theorem inventory only for nonvacuous model properties;
  keep model/Rust correspondence manual unless a checked refinement lands.
- [ ] Decide Rocq's role and either prove named independent theorems or stop
  presenting definitions/typechecking as an active proof lane.
- [ ] Run deterministic invariant campaigns over kernel, batch, layout, source,
  custody, and settlement; minimized failures become permanent vectors.
- [ ] Run malformed account, alias, signer, owner, PDA, replay, donation, burn,
  close/reopen, source-substitution, candidate omission, and late-CPI campaigns.
- [ ] Build the final ELF twice on one machine and inspect the final unstripped
  image, not only pre-LTO diagnostics.
- [ ] Rebuild independently from pinned dependency sources and compare ELF bytes.
- [ ] Produce SBOM, dependency licenses/notices, fixture provenance, source
  offer, theorem/assumption inventory, vectors, gate logs, and static-client
  digest in a release-candidate bundle.
- [ ] Obtain an independent security review and resolve findings without
  weakening refusals.

Acceptance: every public correctness sentence can point to an artifact that says
exactly that much and no more.

## 10. Static Glass and release candidate

Dependencies: frozen program/layout/source semantics and gate 9 artifacts.

- [ ] Generate client wire contracts from one schema owner.
- [ ] Bind expected program id, exact ELF, upgrade authority/immutability state,
  Realm/Profile/source/parser/layout versions, and client bundle in a checked
  manifest.
- [ ] Validate untrusted RPC/index projections locally and reject unknown or
  ambiguous semantics.
- [ ] Keep secrets, privileged APIs, analytics, and Dragon-operated truth out of
  the static bundle.
- [ ] Add exact transaction previews, malicious-RPC tests, CSP headers,
  accessibility, responsive, keyboard, screen-reader, and offline/IPFS tests.
- [ ] Produce an `offline_candidate` from a fresh clone. Do not call it deployed
  or official.

## 11. Gate L0: human/legal/public-network boundary

Engineering may prepare factual architecture, threat, control, and evidence
materials. Before any author-affiliated public-network or real-value path:

- [ ] exact product, users, entity, control, collateral, source, fee,
  compensation, affiliate, client, upgrade, and deployment facts are frozen;
- [ ] qualified counsel has delivered written analysis for those exact facts;
- [ ] any required registration, partnership, no-action, exemptive, or other
  relief is effective and applicable;
- [ ] security, surveillance/incident, conflicts, capitalization, and release
  evidence are complete; and
- [ ] the user gives separate explicit current authorization naming the exact
  network/deployment act.

A meeting request, public comment, pending filing, proof suite, audit, devnet
result, immutable source, or another person's relief does not close Gate L0.
