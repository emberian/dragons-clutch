# Decision 0020: an ArtifactRelease is bound to its deployment once, at finalization, and hot routes authenticate by slot pin

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible on request).** Previously: PROVISIONAL — ruled by the orchestrator on 2026-09-02 under ember's**
standing goal, landed the same morning in two halves, and reversible by ember at
the cost §7 states**. The ruling is `GOAL.md:3103-3105`, carrying the standing
formula *"RULING (under the standing goal; ember may reverse)"*. It is decision
0012's argument spent, not extended. Landed at `90a8563f2` (registry half) and
`271ce0edb` (hot half), both 2026-09-02 07:03, with `85017c63a` (07:37) giving
the slot pin a refusal of its own.

## 1. The question

Decision 0012 proves that an observed ProgramData slot equal to a bound one
means the release's bound ELF digest is the digest of the deployed bytes. It
proves that **about a bound digest somebody checked** — and almost nobody did.
Role activation checked one. A certificate-pinned artifact, which is every
admitted-AOT and shadow-AOT accelerator, had no such moment, and the crate said
so in its own words:

> a finalized `ArtifactRelease` record proves only its own content identity.
> Nothing has bound its `elf_digest` to the account being observed.
> — `crates/dclutch-shadow-accelerator-auth-v4`, quoted in `90a8563f2`

So every hot reader made up the difference by **hashing the complete observed
ELF on every action**: 370,983 CU of a 1,399,700 budget, over the Dealer
accelerator's 744,840 bytes, inside a strategy authentication that cost 419,775
CU — 30% of a whole transaction — *"to re-learn a fact that cannot change"*
(`90a8563f2`, `GOAL.md:3099-3102`).

## 2. The ruling, verbatim

> **RULING (under the standing goal; ember may reverse): decision 0012 governs —
> `ArtifactRelease` finalization records a `DeploymentObservationV1` (one hash
> once), the hot path authenticates accelerator deployments by the slot pin.**
> — `GOAL.md:3103-3105`

## 3. What it changed in the trust model

**The comparison moves to `Finalize`.** A `Finalize` for an `ArtifactRelease`
now carries the Program and ProgramData the record names, and
`observe_artifact_release_deployment_v1` compares them **before the staging
cursor closes** — before, because *"a finalized record is permanent and that is
the last moment the protocol can still say no"* (`90a8563f2`). The comparison
itself is `ArtifactReleaseV1::authenticate_deployment`, which already owned all
eight conjuncts and had had no on-chain caller that hashed for it.

**The hot path stops hashing.** `CurrentDeploymentAuthenticationV2::CompleteElf`
is deleted. Its three call sites — sealed hot, unsealed hot, shadow AOT — take
the slot pin: one `u64` compare over an account the frame already carries. The
variant is renamed `SlotPinnedRelease`
(`programs/dclutch-trading-sbf/src/execution_strategy_v2.rs:243`) *"because the
name was the argument: it said what the route DID, and what the route did was
compensate for a precondition that no longer fails to hold"* (`271ce0edb`).

So the trust model gains a moment — a permanent record is made only after the
chain agreed it describes a real deployment — and loses a per-action
recomputation of that same agreement.

## 4. What it saved, measured

Measured on real ELFs, own worktree, own target dir, zero frame diagnostics on
every build (`271ce0edb`):

| span | before | after |
| --- | ---: | ---: |
| `authenticate_strategy_from_sealed_boxed_v3` | 419,775 | **48,792** |
| honest Dealer equity Add, whole transaction | over the 1,399,700 ceiling | **1,158,123** |

**The honest Add executes and commits** — `after-commit` reached, transaction
`success`, **241,577 CU of headroom**, collateral conserved at 90/10/0, share
supply minted to 10, the LP Position at revision 2, the three Claims accounts
byte-identical. The hostile still refuses `equity:Claims` by name through the
accelerator's own family log. Campaign 30 passed / 1 failed, unchanged in count.

Clearing the budget also **reached a stage no run in the campaign's history
had**, which is how the rent question in decision 0021 was found:
`accepted.rs:8655` builds a second LP Open against the obligation the equity Add
just mutated and the builder refuses — localized to operation 12,
`OP_IDENTITY_EQ`, `a=18 b=5`, with every bank width agreeing (`271ce0edb`).

## 5. The hostiles that guard it

**Three accusations, deliberately not one**, *"because an operator acts on them
differently"* (`90a8563f2`), all in `programs/dclutch-registry-sbf/src/lib.rs`:

- `ArtifactReleaseDeploymentFrame` **0x1013** (`:156`) — the deployment is not
  in the frame, or a deployment was attached to a record naming no address. The
  schema decides which shape is required, not the frame's width, so a caller
  cannot buy the cheap shape by omitting accounts.
- `ArtifactReleaseNotDeployed` **0x1014** (`:170`) — identity, ProgramData link,
  Loader ownership or executability: there is no such deployment to describe.
- `ArtifactReleaseElfMismatch` **0x1015** (`:186`) — one is there and its slot,
  its complete ELF digest or its upgrade authority is not what the record
  claims.
- beside the pre-existing `ReleaseSuperseded` **0x100D** (`:114`), which already
  meant *the named authority moved the substrate forward*.

`85017c63a` adds `DeploymentSlotMismatch` **0x4022**
(`programs/dclutch-trading-sbf/src/lib.rs:434`) *"with the
flipped-bytes-and-moved-slot hostile at the boundary that owns the law"*
(`GOAL.md:3253-3254`) — the slot pin acquires a name of its own rather than
borrowing one.

**The rule is Lean-owned, not restated in Rust.**
`formal/dclutch-semantics/DClutchSemantics/ProtocolInfrastructure.lean:447`
gains `ReleaseObservation`, its outcome in the adapter's own conjunct order, and
four theorems: admission is exactly presence-and-agreement, every refusal is
named, supersession agrees with the slot pin's own naming, and the vectors
decide every outcome. Twelve Lean-decided cases are emitted and replayed through
the real `authenticate_deployment` and the real three-way partition, byte-gated
against their emitter — *"Lean's `native_decide` pinned the outcome list before
Rust ever ran it, and Rust agreed on all twelve"* (`90a8563f2`).

**Frames verified unmoved** (`271ce0edb`): 887 rows both sides, deepest still
`outer::process_close` at 3,968, and
`hot_v3::authenticate_accelerator_invocation_v4` — the tightest first-party
frame in the tree at 3,904 of 4,096 — byte-identical.

## 6. What was given up

The `CompleteElf` variant and its three call sites. Nothing else: the
finalization-time observation is strictly additional evidence, and the slot pin
was already the tree's stated law under decision 0012.

## 7. The cost of reversal

The Dealer equity Add goes back over the compute ceiling and stops committing.
Every accelerator hot action re-acquires a ~371,000 CU per-action ELF hash, on a
route whose whole margin is 241,577. The finalization-time observation would
also have to be unwound from Lean — four theorems and a byte-gated twelve-case
corpus — and four refusal discriminants withdrawn from an append-only band.

## Evidence pointers

`GOAL.md:3099-3105`, `:3238`, `:3253-3254`; commits `90a8563f2`, `271ce0edb`,
`85017c63a`; `programs/dclutch-registry-sbf/src/lib.rs:114`, `:156`, `:170`,
`:186`; `programs/dclutch-trading-sbf/src/execution_strategy_v2.rs:243`;
`programs/dclutch-trading-sbf/src/lib.rs:434`;
`formal/dclutch-semantics/DClutchSemantics/ProtocolInfrastructure.lean:447-475`;
`tools/local-validator/bootstrap/successor/src/runtime.rs:1437`, `:2590`;
`docs/decisions/0012-devnet-iteration-substrate.md`.

**Confirmed, 2026-09-04 15:50 EDT.** Ember, having read the docket that listed this ruling under "M1–M6: a word if any should be reversed; silence is not a ruling": "you aren't waiting on me for rulings are you? i was reading the docket and contemplating it, but overall find your takes reasonable." Taken as confirmation; reversible on request.
