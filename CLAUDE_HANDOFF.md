# Dragon's Clutch handoff to Claude

Status date: 2026-08-19. The former long-form Claude snapshot is preserved in
Git history but is no longer a current-status document. Start here, then read
[`AGENTS.md`](AGENTS.md), [`PROJECT.md`](PROJECT.md),
[`CURRENT_TRUTH.md`](CURRENT_TRUTH.md), and
[`docs/SWARM_ROADMAP_2026-08-19.md`](docs/SWARM_ROADMAP_2026-08-19.md) in that
order. Use [`docs/V1_BACKLOG.md`](docs/V1_BACKLOG.md) as the deeper historical
queue after the roadmap. The roadmap is the current dependency and lane-routing
guide; this older handoff remains useful context but not a promotion ledger.

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
Production source ingestion, other post-resolution consumers, final-LTO stack
repair, and total fragment policy remain open, so do not call native settlement
generally available.

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
consumers, arbitrary fragments, or the shared ELF to release status.

`clutch-liveness` is now a safe, fixed-memory, host-tested pure admission
kernel for component reserves, zero-fee work, shared source/archive accounting,
replay-safe terminal ownership, anti-spam bounds, and fee carry. It does not
measure maxima, move lamports, authenticate accounts, or establish a runtime
liveness guarantee.

`research/fractional-redemption` now makes the smooth-exit policy fork exact.
Resolved lots use `lcm_i D/gcd(D,w_i)`; persistent numerator credits conserve
one market aggregate but cannot make the final sub-denominator residue
disappear without subsidy, forfeiture, or a finer unit. This is model-only and
does not select or implement either production policy.

The narrow Verus result still concerns only the exact executable body of
`prepare_internal_transfer` under its recorded assumptions. Rocq currently
provides definitions/typechecking, not a completed independent theorem
inventory.

## The next hard path

The dependency-ordered route is:

1. finish one clean joined evidence baseline without absorbing another lane's
   dirty files;
2. bind immutable Terms-selected basis mode through Split/Merge/materialize/
   dematerialize instead of reconstructing every active Kernel as finite preset;
3. audit other post-resolution consumers, repair the shared final-LTO stack
   survivors, and freeze the total fragment/credit policy;
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

## Things not to infer

- “Source-joined native point Resolve and exact-lot internal/bearer redemption
  execute in SBF” does not mean production source ingestion, other consumers,
  arbitrary fragment liveness, or the shared final ELF is complete.
- “Typed artifact” does not mean authenticated source or archive.
- “Funded order reservation” does not mean a settled venue.
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
