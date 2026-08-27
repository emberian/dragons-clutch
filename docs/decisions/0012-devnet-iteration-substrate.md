# Decision 0012: the devnet substrate is mutable and iterated, and the slot pin replaces the revocation

Status: **direction ruled by ember, 2026-08-27, live during SMOKE-0** — the
design below is the recommended execution of that ruling and awaits its
implementing lane. Nothing in this record has been implemented; the site map
in §4 is verified against HEAD (`11f249ff`).

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
