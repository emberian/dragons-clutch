# Dragon's Clutch handoff to Claude

Status date: 2026-08-19. The former long-form Claude snapshot is preserved in
Git history but is no longer a current-status document. Start here, then read
[`AGENTS.md`](AGENTS.md), [`PROJECT.md`](PROJECT.md),
[`CURRENT_TRUTH.md`](CURRENT_TRUTH.md), and
[`docs/SWARM_ROADMAP_2026-08-19.md`](docs/SWARM_ROADMAP_2026-08-19.md) in that
order. Use [`docs/V1_BACKLOG.md`](docs/V1_BACKLOG.md) as the deeper historical
queue after the roadmap. The roadmap is the current dependency and lane-routing
guide; this older handoff remains useful context but not a promotion ledger.

## Overnight convergence checkpoint

The shared worktree should be clean at the checked schema-v2 manifest endpoint.
Do not restart from the older `b5da74f` snapshot: it remains valid historical
evidence, but the current runtime/evidence identity and baseline have been
rebuilt, remeasured, resealed, and checked after the manifest-only commit.

The R1 evidence checkpoint is sealed:

- runtime/test ancestry `83e124d` produced two byte-identical ordinary builds
  of the exact 1,228,192-byte
  `bd20711b01828a745ce89de3aacb4b908cbcde32307b61be2c7d612bb8516b60`
  ELF; the committed current audit/log/profile seal is `b5700a9`;
- its audit report SHA-256 is `626a299d...e038`, its upstream 52-file ledger
  SHA-256 is `dbf55f8e...5f35`, zero diagnosed first-party symbols survive final
  LTO, and all 40,389 direct `r10` references are at most 4,096 bytes;
- `a5725a3d...` differs only in seven `.data.rel.ro` line-location bytes after
  a required rustdoc repair; executable sections and normalized instructions
  are identical, but the old CU rows are retained as historical rather than
  relabeled;
- Persvati independently attested exact `b5da74f` from a fresh archive and
  minimal hashed Git bundle: 39/39 gates PASS, 0 STOP, 492 files checked twice,
  95 evidence checksums verified. The durable job is
  `/home/ember/jobs/dragons-clutch-final-portable-attest-b5da74f-20260819-GMqkJL`;
- Hbox received and verified the exact `b5da74f` archive at
  `/tank/joshibot/dragons-clutch-r1-hbox-vHdulP`, but its independent SBF build
  is honestly `UNAVAILABLE/STOP`: the pinned Solana/Anza toolchain,
  platform-tools v1.53, Cargo config, and five locked Solana crates are absent.
  Do not install substitutes and call that reproduction;
- `b047415`/`345bc78`/`07d5efe` replace the four vacuous scalar batch Verus
  placeholders with narrowly reviewed proofs. They do not prove production
  dust completion, side equality, padding validation, coupled V1, or SBF; and
- `6dbe618` adds an independently red-teamed internal-only Terminal Lifecycle
  V2 model. It is not a live ABI, Token-2022/CPI path, rent-funding proof,
  migration, external-bearer lifecycle, or fractional-credit solution.

Glass now exposes those evidence planes and their negative boundaries while
remaining unbound/offline. The schema-v2 manifest is checked in: all 94
unique declared gates, cache/path-stable evidence records,
process-group timeout cleanup, exact Verus failure classification, current
default/mock ELF parsing, the sealed liveness gate, the signed committed walk,
and the central proof/model/tool surfaces match their declarations. The
manifest also passes `check --run-gates` after its manifest-only commit.

The first full 94-gate emission at `ec77d0b` matched 86 and contradicted eight;
those eight repair lanes are now closed. Commit `83e124d` splits the inert
default/`0x79` campaign from the explicitly non-production mock-source success
campaign. A quiet-tree full emission at `83e124d` then matched 93/94: the sole
remaining refusal was the strict liveness seal correctly detecting the new ELF
identity caused by the rustdoc-link repair. Commit `b5700a9` closes that final
identity gap with new same-ELF measurements and stack evidence. The subsequent
schema-v2 emission and post-commit full check are 94/94. Read
[`docs/implementation/BASELINE_MANIFEST_DIAGNOSTIC_2026-08-19.md`](docs/implementation/BASELINE_MANIFEST_DIAGNOSTIC_2026-08-19.md)
for the historical convergence trail.

Resume R1 in this exact order:

1. run `git status --short` and refuse unrelated dirty bytes;
2. run a fresh Persvati portable attestation from the final manifest commit;
3. preserve the manifest/runtime seal while selecting the next dependency-
   unblocked R2/R3 lane; and
4. rerun the manifest only after another accepted source or evidence change.
   No Hbox SBF claim exists until the exact missing toolchain/dependency closure
   is supplied and recorded.

Direct V3 remains isolated at
`/Users/ember/jobs/dragons-clutch-r3-direct.BDnrsh` on
`codex/r3-direct-v3`. Commits `529878d`, `1e8b8a3`, and `1241399` freeze the
account, DirectBatchPolicyV3 artifact, and intent codecs only. Do not
cherry-pick or route them merely because codec tests pass. The active model
correction must first close ReservationV2/page/grid authentication, exact
Position asset release for every abort/lapse, strict clock-boundary ownership,
DonationLedger observation, durable selected-slot/count projection, and the
V4 placement phase gate; then it needs a fresh read-only re-audit.
The model correction plus the three audit-RED blocker fixes are committed at
`b49c497` on that branch (worktree clean). All three blockers closed with
independent two-sided verification: the order-body constant is 107 and a
cross-crate tripwire asserts model/live page-digest and order-set equality
(red at 99, demonstrated); zero-envelope reservations refuse at creation so
every abort/lapse release path stays reachable; and the reservation authority
carries the epoch-bound 96-byte `direct_policy_v3_id` enforced by all three
state validators, disjoint from the relation-side legacy digest. Research
37/37 and layout 177/177 plus ResolutionWork 10/10 pass and strict Clippy is
clean on both crates. Still open, recorded in the design doc: Settle does not
embed the economic Position-transfer kernel; the `verify_lease` self-sink
check is tautological at its own layer; `FROZEN_EMPTY` does not pin admission
fields. Status remains MODEL-ONLY / DESIGN with no dispatcher, SBF route, or
migration. Preserve this checkpoint rather than inferring live authority.

## What changed after the prior handoff

The project corrected a major product-model narrowing. Native settlement is
not limited to one-hot categorical Eggs:

- degree zero is the categorical partition;
- degrees one through three are native open-clamped B-spline Eggs;
- exact coefficients over those Eggs are the native payoff algebra; and
- degree-zero baskets are only a compatibility lowering when they approximate
  a smooth target.

This is not aspirational prose alone. The repository now has an exact safe-Rust
degree-zero through degree-three evaluator, an independent exact-rational
oracle, a conservative reference-resolution seam, an exact-rational shape
compiler lab, a window-semantics lab, a pure occupation accumulator, and a
separate Lean construction model. The production path now also selects v3 for
smooth Terms and executes source-joined degree-one through degree-three point
Resolve plus exact-lot internal and positionless bearer redemption in real SBF.
Production source ingestion, other post-resolution consumers, and total
fragment policy remain open, so do not call native settlement generally
available. The exact sealed R1 ELF has closed its first-party final-LTO stack
gate; that is an artifact-specific result, not a semantic or system release
claim.

The controlling semantic note is
[`docs/design/NATIVE_AND_LOWERED_SEMANTICS.md`](docs/design/NATIVE_AND_LOWERED_SEMANTICS.md).

## What is now genuinely executable

The local SBF adapter has crossed several formerly open value/liveness seams:

1. `WithdrawCash` transfers exact unreserved Position cash from the pooled
   Hoard through Token-2022 while preserving reserved cash and locked backing.
2. Actual outcome-mint supply and bearer token possession are authoritative.
   Ordinary holder burns are recognized as claim forfeiture, and a transferred
   winning Egg can redeem from a wallet with no Clutch Position.
3. Typed staged transport can create exact policy, grid, and Terms artifacts
   through begin/write/seal, survive a bank restart, abort/reap, and idempotent
   reseal. Artifact stage/final creation is also pre-fund safe as of `e7d975b`.
   It is deliberately not a generic source/archive uploader.
   The same public path now creates Realm/Profile and the initial degree-
   selected market plane from a bank with no injected Clutch account.
4. `PlaceOrder` creates and funds one canonical reservation from exact free cash
   or internal Eggs. `CancelOrder` tombstones and releases that reservation once.
5. Market resolution has its own replay domain; it no longer consumes an
   arbitrary owner's command sequence.
6. One narrow `SettlePage` seam consumes a pre-frozen same-page, full-fill,
   direct single-Egg, zero-fee receipt against a selected candidate, canonical
   CandidateFeed, and two ACTIVE reservations. A separate pre-fund-safe
   `SubmitDirectPage` seam creates one deterministic SUBMITTED Candidate/feed
   from a frozen two-order/two-outcome equal-limit direct page, but leaves Epoch
   FROZEN and score/digest zero and unverified. Completion, selection, receipt,
   and end-to-end reachability remain STOPs.
7. Routed ResolutionWork V1 executes Begin/Fold/Finalize/Abort against sealed
   archive bytes and closes/refunds its Work and Reserve. Its exact measured
   rows clear the selected liveness profile; no unmeasured shape or system path
   inherits that admission.

The current frozen runtime source and test ancestry is `83e124d`; the current
profile/artifact evidence is sealed at `b5700a9`. Two ordinary builds are
byte-identical at 1,228,192 bytes with SHA-256
`bd20711b01828a745ce89de3aacb4b908cbcde32307b61be2c7d612bb8516b60`.
The seal reports zero diagnosed first-party final-LTO survivors and no direct
`r10` reference above 4,096. The preceding `a572...` seal remains historical;
the current same-ELF bank rows were remeasured rather than borrowed.

The default source registry is empty. Endow therefore refuses
`SourceReleaseUnavailable` (`0x79`) before owner-plane allocation or token CPI;
mock success belongs to a distinct `non-production-mock-source` ELF. Direct V2
full top-three Select reaches exactly 1,400,000 CU and rolls back. Direct V3 at
`ef32495` remains MODEL/DESIGN only.

The clean-source joined committed walk used source HEAD `c05fe84` and ELF
SHA-256
`70c33c1cd44b475745b0562a79d9107f1d2101cbf698ebd6c233ca167ebab2e6`.
It submitted 22 freshly signed transactions to a loopback validator; all 22
reached confirmed commitment, including two expected refusals. It reloaded 18
watched accounts, completed internal and bearer exits, withdrew both owners'
cash, and ended both Position cash fields and the Hoard token amount at zero.
A deliberately corrupted terminal Hoard expectation made the gate fail on
committed bytes.

Quote that as **local, signed, sequential, SBF-executed, genesis-assisted
evidence**. Eleven program-owned prerequisites were injected. The walk does not
authenticate its source history, create the full feed/epoch plane, clear an
order batch, settle receipts, or establish deployment safety.

## Formal and host claim boundary

At committed `8c929a9`, `lean/DragonsClutch/BSpline.lean` contains 159 counted
declarations including 116 theorems, with no `sorry`, `admit`, axiom, `unsafe`,
`native_decide`, or `implemented_by`. It closes the uniform stored-knot/pane/
BasisFuns linkage and canonical largest-remainder existence/uniqueness in the
separate model. It does not prove Rust parser/control-flow/Fraction/selection-
loop equivalence, arbitrary nonuniform degree-one linkage, Solana account
codecs, CPI, SBF compiler, or runtime.

At `be8eba3`, eight Lean-computed vectors match digest-bound production
evaluator outputs and five actual-source mutants compile/execute/go red. This
is finite executable refinement evidence, not a Verus invocation or a
universal theorem about the Rust evaluator or SBF.

The Rust B-spline evaluator, reference seam, compiler research, window model,
occupation crate, and isolated 319-byte native Resolution codec are supported
by their named evidence planes. Preserve their exact labels. The v3 account and
source-joined point/internal/bearer transitions now have focused SBF evidence;
that does not promote production source ingestion, other post-resolution
consumers, arbitrary fragments, or the complete program to release status.

`clutch-liveness` is now a safe, fixed-memory, host-tested pure admission
kernel for component reserves, zero-fee work, shared source/archive accounting,
replay-safe terminal ownership, anti-spam bounds, and fee carry. It does not
by itself measure maxima, move lamports, authenticate accounts, or establish a
runtime liveness guarantee. The separate ResolutionWork profile binds one
measured runtime route; it does not emit a global `LivenessPolicy` or prove
protocol-wide no-stranding.

`research/fractional-redemption` now makes the smooth-exit policy fork exact.
Resolved lots use `lcm_i D/gcd(D,w_i)`; persistent numerator credits conserve
one market aggregate but cannot make the final sub-denominator residue
disappear without subsidy, forfeiture, or a finer unit. This is model-only and
does not select or implement either production policy.

The narrow executable-body Verus result still concerns only
`prepare_internal_transfer` under its recorded assumptions. A separate pinned
scalar batch shadow now proves only allocation decomposition/per-fill bounds,
unique tick selection, a whole-fill partition conditional on accepted side
equalities, and a zero-suffix fold identity. The runner hash-enforces its Verus,
Z3, and shipped vstd artifacts and four semantic mutants go red. It does not
import or refine the production batch body. Rocq currently provides
definitions/typechecking, not a completed independent theorem inventory.

## The next hard path

The dependency-ordered route is:

1. preserve the sealed R1 artifact/stack/bank evidence and produce the missing
   schema-v2 release baseline;
2. (landed at `3a81b38`, inside the frozen runtime) KernelAccount v2 binds
   the immutable Terms-selected basis mode through Split/Merge/materialize/
   dematerialize; the successor gate is per-degree blank-bank joined
   lifecycle evidence;
3. audit other post-resolution consumers and freeze the total fragment/credit
   policy;
4. build permissionless SourceSpec/Feed/archive construction and authenticate a
   single pinned source/parser/deployment generation against the Clock sysvar;
   preserve the now-tested prefund-safe allocation/assignment discipline for
   every new PDA family;
5. build the full blank-bank Epoch/page/candidate/checkpoint/pot/receipt plane;
6. complete/score candidate submissions, close the candidate window, select a
   winner, freeze the complete reservation and entitlement sets, construct the
   receipt/pot path, and join it to `SettlePage`, then add the remaining
   partial/portfolio/virtual/fee shapes plus exact lapse, refund, and cleanup;
   and
7. measure and prepay every mandatory action before freezing fee or failure
   economics.

The current critical STOP is not a missing mathematical shape. It is the joined
authority and lifecycle path from authenticated source bytes through persisted
native resolution and funded settlement.

Terminal closure is also not global: empty frozen Direct V2 epochs can strand
Reservations; no general program-account close exists outside the named
transient lanes; outcome mints have no `MintCloseAuthority`; Hoard donations,
claim-burn forfeiture, and fractional fragments lack terminal disposition; and
most accounts lack an authenticated rent-payer/donation split.

## Things not to infer

- “Source-joined native point Resolve and exact-lot internal/bearer redemption
  execute in SBF” does not mean production source ingestion, other consumers,
  arbitrary fragment liveness, or the shared final ELF is complete.
- “Typed artifact” does not mean authenticated source or archive.
- “Funded order reservation” does not mean a settled venue.
- “ResolutionWork clears its measured liveness profile” does not mean a global
  `LivenessPolicy`, production source, deployment, terminal closure, or
  no-stranding result.
- “Public creation from an absent PDA” does not mean squatting-resistant
  permissionless creation. Artifact stages/finals and the initial market plane
  now handle admitted pre-funding, and Market/Reservation/later-owner families
  have focused real-bank regressions. Apply the same rule to every new family.
- “22-step committed walk” does not mean blank bank, operatorless lifecycle,
  audit, devnet, mainnet, or legal authorization.
- “Partition of unity proved in Lean” does not mean the Rust or ELF is formally
  verified.
- Future fee volume, DREGG appreciation, treasury discretion, donations, or
  Hoard principal never capitalize mandatory liveness.
- “Best valid submitted candidate” must remain the clearing phrase unless a
  checked optimality certificate for the exact selection claim exists.

## Working rules

Check `git status --short` before editing; this workspace may have active
parallel owners. Commit only named files in your lane. Local commits are
ordinary work; pushing, tagging, publication, deployment, public RPC, real
signing/funds, market creation, regulator contact, or filing require explicit
current authorization.

When a claim changes, update `CURRENT_TRUTH.md` and `docs/V1_BACKLOG.md` from
the produced evidence, not from an agent summary. A refusal is part of the
product until a stronger authenticated rule replaces it.
