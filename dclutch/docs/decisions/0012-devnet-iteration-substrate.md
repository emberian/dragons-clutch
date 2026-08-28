# Decision 0012: the devnet substrate is mutable and iterated, and the slot pin replaces the revocation

Status: **direction ruled by ember 2026-08-27; LANDED the same evening.**
The admission is live at `0e34c036` (lane PIN-0012): no wire change, one
greppable admission predicate (`require_slot_pinned_release_v1`), one digest
owner (`slot_pinned_release_elf_digest_v1`), eight banded
`ReleaseSuperseded` refusal discriminants, and the census regenerated at 209
codes. The producer mints policy from observation at `636230ef`
(DEVNET-DRIVER); the external campaign driver exists at `d94dc438`..`1040e918`
(DRIVER), which also measured this record's W1 as a live refusal (activation
on a mutable substrate, custom error 0x1004) before the admission landed.
PIN-0012's yield flags three deviations for ember's veto (the Pinned*
renames of zero-caller symbols, host relaxation by named sibling rather than
flag, and release-tool's strict gate left strict deliberately) and one site
this record's §4 map MISSED: `crates/dclutch-shadow-accelerator-auth-v4`
carried its own unconditional authority refusal AND a silent hash fallback —
without converting it the fast path was dead for Trading. Named debt: the
CU claim (the life fits a mutable substrate) is argued and unit-tested, not
end-to-end measured — DEPLOY-1 re-measures the 20-seed sweep. **That debt is
closed below: 20/20 on the mutable substrate, and the pin costs 73 CU.**

**Closing sweep, same evening (lane POST-0012).** PIN-0012's spawnable
leftovers are closed and two of them were worse than recorded.

- **The TS host mirrors are converted** (`abe1e70d`). `localSuccessor.ts:102/121`
  and `infrastructure.ts:141`, in `apps/dclutch-web` and `packages/dclutch-sdk`
  alike, hard-required slot zero and a `None` authority — the browser and the
  CLI refused any iterated substrate before reading an account. They now mirror
  the contract by name (`requireSlotPinnedReleaseV1`, `slotPinRefusalV1`), and
  the supersession sentence is READ OUT of the generated refusal registry
  rather than written in the browser, so a client cannot become a second
  authority on what the protocol means. Recorded as Blocker D in
  `docs/design/DEVNET_DEMO_DEPLOY.md` §7 — it was a deploy-day blocker nobody
  had listed. **A second defect surfaced there that is not about 0012 at all**:
  `localSuccessor.ts` still carried `requireZero(bytes, 13, 32)` on the
  ProgramData header — the check `releaseRegistry.ts` removed the same day
  after a live measurement, because Loader V3 leaves a revoked program's former
  key inert at `[13..45]`. It passed only because the local genesis writes that
  tail zeroed, so it would have failed on the first revoked role read from a
  real cluster — which is exactly how the immutable ceremony ends.
- **Deviation 3 is closed the way PIN-0012 recommended** (`7bb9a075`): a
  manifest FIELD, not a decode flag. `require_immutable` and
  `CapabilityAcceleratorMustBeImmutable` both call the contract's own
  `require_slot_pinned_release_v1` now, and both checked manifests carry a
  DERIVED `evidence_class` naming which substrate they are evidence for. The
  strictness did not disappear, it moved to where a reader can act on it;
  substitution is still refused by identity, because the policy and the
  authority are inside the artifact bytes the id hashes.
- **The Lean statements are corrected** (`d91763a3`, `0392c9f3`), and the
  finding is bigger than the fix: `ProtocolInfrastructure.lean` **was not
  imported by anything**, so `lake build` had never elaborated the two
  theorems that stated the opposite of this decision. `a7de18e5` makes library
  membership a lakefile glob so a new orphan is structurally impossible
  (93 → 120 jobs, zero red, zero `sorry`). See ledger M-64.
- **The CU debt was NOT closed by that sweep, and the reason is the interesting
  part.** The 20-seed sweep is now a real tier (`9db549ef`,
  `tools/gauntlet/hot-cu/run-hot-cu.sh`) and measured HEAD at 20/20, mean
  1,345,302 of 1,400,000. **That number could not be evidence for this record's
  claim.** `waist::release` built every release `Immutable` and
  `waist::immutable_programdata` wrote the authority option as `None`, so
  `slot_pinned_release_elf_digest_v1` always took its `Immutable` arm — the
  arm delegated *unchanged* to `immutable_release_elf_digest_v1`, which never
  hashed anything. The `ExactAuthority` arm this decision exists to add was not
  constructible by the fixture at all, so the Hot tail never paid the hash it
  was supposed to have saved. Corroborating rather than merely asserting it:
  the pre-`0e34c036` sweep meaned 1,366,177 against this one's 1,345,302, and
  that ~20,875 gap is about fourteen bump iterations, inside M-61's ±46,000
  draw — a 700k effect could not have hidden there. Ledger M-63 records the
  general form.

- **The CU debt is now CLOSED, and the number is 73 CU** (lane
  POST-0012-EXACTAUTH, `d20837fd`/`57138ba8`/`49393605`). `FixtureSubstrateV1`
  makes the `ExactAuthority` arm constructible — with an `ImmutablePinned`
  control that runs the *identical* digest arm at a *different* release
  identity, because the policy byte, the bound authority and the bound slot all
  live inside `ArtifactReleaseV1::to_bytes` and therefore move every PDA the
  Registry derives on chain. Three arms, one trading ELF
  (`7facb8e58e45843f…`), one clean archive of `57138ba8`, seeds 0..19:

  | arm | PASS | MEAN of 1,400,000 |
  |---|---|---|
  | `immutable` | 20/20 | 1,345,302 CU |
  | `immutable-pinned` (control) | 20/20 | 1,353,477 CU |
  | `slot-pinned` (this decision's arm) | 20/20 | 1,355,575 CU |

  **The means are not the answer and reading them as one would have produced a
  false null.** `slot-pinned − immutable` is +10,273 of mean; the control, which
  executes the same code, is +8,175 of mean by itself, leaving a
  difference-of-differences of +2,098 that is smaller than the redraw it sits
  on. Because all three arms ran the same seeds against the same ELF, M-61's own
  decomposition can be solved **per seed** instead of averaged over —
  `delta = n × 1,500 + c`, where `c` is the constant and the only real quantity:

  - control vs `immutable`: **c = 0** (exactly zero on 18 of 20 seeds, never
    past 6 CU) — the method validating itself, with an 8,175 CU mean gap fully
    accounted for as bump-search depth;
  - `slot-pinned` vs `immutable`: **c = +73 CU**, on all twenty seeds (67…77).

  So this record's headline holds, in the direction that matters: **the market
  life fits on a mutable substrate, 20/20, and the `ExactAuthority` arm runs it
  73 CU — 0.005% of the ceiling — behind the immutable one.** Mutable went from
  *refused* to *admitted at parity*, and parity is now a measured quantity.

  **What is still argued, stated plainly.** The *~700,000 CU saving* is not
  measured and cannot be by this instrument. Post-0012 **neither** arm hashes on
  the Direct Hot route: both reach
  `authenticate_activated_current_deployment`, which reuses the
  activation-bound digest. The megabyte hash lives in the *uncached*
  `authenticate_deployment` (`shadow-accelerator-auth-v4/src/deployment.rs`,
  the `hash(programdata_view.elf())` branch), and the silent fallback that would
  have taken a mutable role there was converted by PIN-0012 — so the
  counterfactual is not constructible in-tree, and ~700k remains an argument
  from ELF size rather than a figure this sweep produced. The arms also measure
  the **Hot tail on the Direct profile**, which is the route SMOKE-0's W1 wall
  was about, not every route in the market life. Two on-chain refusals
  accompany the number
  (`tests/slot_pin_supersession.rs`): the pin holding executes the whole
  canonical Direct Hot action, and the pin broken refuses with
  `RegistryError::ReleaseSuperseded` (`0x100D`) after 51,574 CU, moving no
  material state and never entering Trading — `refusals.md` also bands Trading's
  `0x4007`, but on this route the Registry authenticates the role deployments
  before it forwards, so `0x4007` is unreachable behind `0x100D`.

A sharpening found in the tree itself while implementing: the contract
already anticipated this design. `ArtifactReleaseV1::slot_mismatch_refusal`
(`crates/dclutch-registry-contract/src/artifact.rs`) names a strictly-later
observed slot on an `ExactAuthority` release `ReleaseSupersededByUpgrade`,
and its doc records a fact **sharper** than the monotonicity argument below:
the Loader V3 refuses an `Upgrade` in the ProgramData's own recorded slot
("Program was deployed in this block already"), and refuses the `Close` a
redeploy would need in the same way — so even the same-slot corner is closed
by the loader itself, not merely by slot ordering.

## Context: the ruling

SMOKE-0 (`docs/evidence/DEVNET_SMOKE_0.md`) measured wall W1: the market life
requires all seven roles revoked immutable, and immutable rent — ~31.7 SOL at
current artifact sizes — can never be recycled. The lane recommended spending
it once. **Ember refused the premise**: devnet SOL arrives by faucet at a few
SOL per day, so ~32 SOL is *days of accumulation*, and the deploy is not a
one-shot event — it is something the project will iterate "a million times."
The ruling, verbatim in effect: *the devnet substrate does not need to be
fully trustless if full trustlessness prevents iteration.*

So the devnet substrate's requirements are, in priority order:
1. Iterating a deployment must cost fees, not rent.
2. The protocol running on it must be the SAME protocol — no devnet fork of
   any contract, no feature-flag second truth.
3. Whatever trust is added must be disclosed, structural, and fail-closed —
   never silent.

## The insight: the loader's slot write is the invariant immutability was buying

Everything immutability buys at run time reduces to one sentence: *the ELF
digest that activation hashed is still the digest of the deployed bytes.*
Immutability guarantees it by making redeployment impossible. But the Loader
V3 gives a second, cheaper guarantee: **every `Upgrade` writes the current
slot into ProgramData** (`UpgradeableLoaderState::ProgramData { slot, .. }`),
and there is no path to different bytes at the same program id without
`Upgrade` — a closed program id can never be redeployed (measured,
runbook §4.3; and the runbook's own local measurement shows the slot moving:
"deploy landed at slot 167 and its redeploy at 531", §7 blocker A).

Therefore, for a release that binds a deployment slot:

```text
observed ProgramData slot == release.deployment_slot
    ⟹  no Upgrade since the release observed the deployment
    ⟹  the bytes are the bytes first admission hashed
```

**Digest reuse gated on slot equality is sound for a MUTABLE deployment.**
The slot comparison is already made on every cached read
(`authenticate_current_deployment` refuses `DeploymentSlotMismatch`); what
changes is only that an `ExactAuthority` release may then reuse the
activation-bound digest instead of re-hashing a megabyte ELF — the ~700k-CU
cost that put the market life over the 1.4M ceiling on mutable roles
(SMOKE-0 W1).

What this does NOT protect, named plainly: the upgrade authority (ours) can
still ship new bytes. The protection is that **every open market refuses the
instant it happens** — the slot moved, so every cached authentication returns
`DeploymentSlotMismatch` until a new release generation is published,
activated, and new markets founded on it. Fail-closed, disclosed (the
`ExactAuthority` policy and the authority key are IN the on-chain record for
any reader), and exactly requirement 3. A third party cannot upgrade at all
without the authority's signature.

## The iteration economics this buys

- Deploy the seven roles **once, mutable, never revoke**. ~31.7 SOL parked as
  working capital — recoverable in full (minus the 36-byte residues) whenever
  the substrate is retired. SMOKE-0 measured the recycle path end to end.
- Iterate by **`Upgrade`, not redeploy**: the buffer drains into the existing
  ProgramData inside the upgrade instruction, so an iteration costs
  transaction fees (~0.007 SOL measured for a full write ladder) plus
  transiently held buffer rent. New slot → mint new record bodies from the
  observed ProgramData → publish → activate → found. The record/activation
  layer is already generation-shaped; nothing there changes.
- The final public demo substrate, when ember chooses to make one, uses the
  full immutable ceremony unchanged — this decision adds a mode, it retires
  nothing.

## §4 What has to change, the complete verified site map

The `Immutable`-gating sites at HEAD, by role in the design:

**The one load-bearing generalization** (the CU wall):
- `crates/dclutch-registry-activation-auth-v1/src/lib.rs:242` —
  `cached_role_deployment_observation_v1`: extend the digest-reuse arm to
  `ExactAuthority` when the observed slot equals the release's bound slot
  AND the observed authority equals the release's bound authority; keep the
  full re-hash refusal otherwise. This is the single implementation both the
  Registry reauthenticate route and every role adapter's cache read share
  (that convergence was a4cedae9's point, and it pays off here: one edit).
- `crates/dclutch-registry-contract/src/immutable_registry.rs:380-391` —
  `immutable_release_elf_digest_v1`: either grow a slot-pinned sibling
  (`slot_pinned_release_elf_digest_v1(release, observed_authority,
  observed_slot)`) or generalize in place; the refusal name
  `MutableRegistryRelease` stays for the arm that still refuses.

**The infrastructure admission sites** (accept `ExactAuthority` under the
same slot pin; Registry/Rent/Core records may then be mutable):
- `programs/dclutch-core-sbf/src/infrastructure.rs:281` (Core init) and the
  `require_pinned_immutable_deployment` path below it.
- `crates/dclutch-registry-contract/src/immutable_registry.rs:293` (the
  immutable-registry input validation).

**The per-family cache-read mirrors** (each re-states the immutable-only
fast-path guard; each takes the same slot-pin arm):
- `programs/dclutch-trading-sbf/src/execution_strategy_v2.rs:517`
- `programs/dclutch-resolution-proof-sbf/src/provider_instruction_v3.rs:554`
- `programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:470`
- `crates/dclutch-product-runtime-v2-operator/src/found.rs:583, :609`
- `crates/dclutch-market-retirement-v1-operator/src/lib.rs:707`

**The host/browser mirrors** (relax behind an explicit choice, default
strict): `crates/dclutch-operator/src/infrastructure.rs:428`,
`crates/dclutch-release-tool/src/infrastructure.rs:402`,
`crates/dclutch-release-tool/src/capability_execution.rs:282`
(`CheckedInfrastructureV1::validate` and relatives — a checked release for
the iteration substrate should say `evidence_class` names a mutable
substrate, mirroring the `loader-state-carrying-an-observed-retained-
authority` precedent from blocker B's fix).

**The producer**:
`tools/local-validator/bootstrap/successor/src/plan.rs:740,760` — mint the
policy FROM the observation: observed authority present ⟹
`ExactAuthority(observed)`, absent ⟹ `Immutable`. The plan-time refusal
SMOKE-0 exercised ("Trading ProgramData account upgrade authority is not the
one this plan authenticates against") remains for a mismatch against what
the caller *declared*, so nothing silently accepts an unexpected key.

**Formal/evidence**: sweep `formal/` and the gauntlet witnesses for
statements pinning the Immutable-only fast path (LEANGUARD owns the Lean
side); the census/witness rows that assert the mutable refusal flip to
asserting the slot-pin refusal (`DeploymentSlotMismatch` on a moved slot —
which c5d791e's prime-slot rehearsal already knows how to exercise).

## What stays refused, deliberately

- Digest reuse with a moved slot: refused (that IS the design).
- A release claiming `Immutable` over a ProgramData carrying an authority:
  refused (unchanged).
- A wire-supplied "trust me" digest with no slot pin: does not exist.
- Weakening anything about finalized Registry records: out of scope; records
  stay immutable and content-addressed.

## Sizing and the lane

One lane, protocol-tier review (this touches the authentication seam):
kernel/contract edits are small (the two registry-contract sites + the one
activation-auth arm), the mirrors are mechanical, the producer edit is
small, and the test surface is the real work — every site needs its
positive (slot-pinned mutable accepted), its negative (moved slot refused),
and the CU re-measurement proving the life fits on a mutable substrate.
Trigger: ember's veto window on this record closes. Pairs naturally with
the devnet-driver lane (SMOKE-0 delta 2), which is what makes the substrate
reachable at all.
