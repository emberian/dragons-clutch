# The reviewed capability-activation template, and what each family may do with it

2026-08-30, WALL22 lane. Wall #22 is "activation demands a V1-schema descriptor
while every family's ProgramSet stamps the V4 id." This document records the
root cause, the template that closes it, the checks that refuse a wrong bundle
before anything reaches a chain, and — family by family — whether an activation
is available, blocked, or impossible.

It exists because a wrong activation bricks a root **permanently**: the outer
writes `CapabilityRootHeaderV1 || <projected request>` and never decodes the
family tail, so a tail of the right width and the wrong content is admitted and
is undecodable forever. MEMBRANE declined on 2026-08-30 to author a second
activation bundle against an unreviewed template for exactly that reason. This
is the reviewed template.

## 1. The root cause, and why the obvious fix is the wrong one

`programs/dclutch-trading-sbf/src/outer.rs`, `authenticate_set_descriptor`:

```rust
if selected.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1 {
    return Err(TradingSbfError::UnsupportedContent.into());
}
```

Every action entry every family builds carries `v4::SCHEMA_RELEASE_ID`. The two
constants have distinct preimages (`b"dclutch/schema/capability-program-v1"` and
`…-v4`), so an all-V4 release refuses activation with `UnsupportedContent`
before any width or effect check is reached.

**The seam is correct and must not be relaxed.** Two independent reasons:

1. It is what keeps the activation outer family-neutral without a second
   descriptor decoder. The function's own header says so, and the property is
   load-bearing: the outer has no family decoder by design.
2. Accepting a V4 descriptor would not activate anything. A V4 descriptor's
   effect performs its action's work, not the root's creation, so it refuses one
   statement later at `effect.request_bytes() != root_state_bytes`
   (`prepare_effects`). "Teach the seam to accept V4" trades a clear refusal for
   a confusing one and closes nothing.

**The wall is a missing artifact, one per family**: a V1 descriptor, an
`AccountProfileV1`, and an `EffectProgramV2` whose projected request buffer IS
the family's exact initial root tail.

## 2. Where each family actually stands

MEMBRANE's measurement — that the wall is family-wide, not Direct-specific — is
correct. The four families are not, however, at the same wall.

| Family | Root tail type | Activation triple | Set entry | Verdict |
|---|---|---|---|---|
| Direct | `DirectRootStateV1`, 24 B, all constant | `dclutch-direct-codec::activation_bundle_v1` (`b45d3a2c`) | 4th entry, V1-stamped (`9d858f5a`) | **wall down** |
| General | `GeneralRootV2`, 128 B, constants + 3 seam-supplied fields | **landed here**: `dclutch-general-adapter-contract::activation_bundle_v1` | **landed here**: `build_general_activation_capable_program_set_v1`, eighth entry | **wall down**, and the records are byte-identical to a triple the real ELF has already activated; publication closure not wired |
| Rational | **none** | — | — | **blocked one layer deeper** |
| Structured | **none** | — | — | **blocked one layer deeper** |
| Fractional | `FractionalRootV1`-shaped, but three fields have no seam author | — | — | **impossible without Trading-side work** (ORCH ruling, `9f4d110a`) |

Rational and Structured are not missing an activation artifact. They are missing
the thing an activation creates. `root_state_bytes` is a free caller parameter in
both: every Rational call site passes the literal `64`
(`tools/local-validator/bootstrap/successor/src/rational_market.rs`), and
Structured's only values are fixture `8`s
(`crates/dclutch-bearer-v2-operator/src/test_open_fixture_v3.rs`). Both families'
lifecycle-policy headers say the root's "creation belongs to the activation
route" and then nobody authored what that route is supposed to create. **You
cannot write an activation effect for a tail that has no layout**, and authoring
one is a permanent root-ABI decision for that family, not something to do in
passing. Size, per family: one root state type with magic/version/phase and its
generated layout constants, a creation oracle in the style of
`general_root_creation_tail_v2`, the `root_state_bytes` parameter replaced by
that type's width at every call site, then this template, then the set entry and
its publication ripple.

## 3. The template

`crates/dclutch-capability-activation-codec`, one public constructor:
`build_activation_bundle_v1(ActivationBundleInputV1) -> ActivationBundleV1`.

### Family-varying — all of it in the input

| Field | What it is |
|---|---|
| `kind`, `config_schema`, `root_schema`, `derivation_policy`, `capacity_profile`, `root_state_bytes` | Inherited verbatim from the family's own action descriptor. Activation restates none of them. |
| `request_schema` | The **one** coordinate activation does not inherit. Reusing the action request schema would let an ordinary action request select the activation descriptor. |
| `constant_root_tail` | The family's canonical initial tail with every seam-supplied region zeroed. Derive it; never write a literal. |
| `seam_fields` | The regions the seam supplies, as `SeamScalar { offset, register }` / `SeamIdentity { offset, register }`. |
| `funding_ledger_slot_count` | Compartment rows the founding provisions in the selected ledger. |

### Family-invariant — fixed in the template, never restated by a family

The two-account profile (vacant root at `ACTIVATION_ROOT_ACCOUNT_V2`, the
selected Trading `FundingLedgerV2` at `ACTIVATION_FIRST_FUNDING_ACCOUNT_V2`); the
three profile operations (`RequireKey` on the root, `RequireOwner` on the ledger,
`ProjectDataU64` of the parked rent quote); the rent transfer; the register banks
(`ACTIVATION_COMMON_SCALARS_V2` = 8, `ACTIVATION_COMMON_IDENTITIES_V2` = 12, the
rent quote at scalar 8, constants from scalar 9 up); the ascending write order;
and the three finalized schemas.

### The question a family has to answer before it can have an activation at all

**Is every byte of the initial root tail either a constant the family publishes,
or a seam-seeded register the outer fills in before any artifact runs?** There is
no third source: the effect kernel has no arithmetic and the activation frame
holds only the root and the funding ledgers.

- Direct: yes, all constant.
- General: yes — Market (identity 4), config id (identity 8), generation
  (scalar 1), everything else constant.
- Fractional: **no.** The PDA bump is derived after the effect runs; `terms` is a
  digest over Market-carrying bytes the config is deliberately free of; the rent
  beneficiary is unnamed. That is an impossibility, not a backlog item.

## 4. The checks that refuse before anything is on chain

### In the constructor, so a bricking bundle cannot be held

`build_activation_bundle_v1` runs the **real effect kernel** —
`project_with_aliases_and_requests_atomic`, the same evaluator the outer runs —
over the effect it has just built, with probe seam registers no plausible mistake
could coincide with, and returns `Err(ProjectedTailMismatch)` rather than a
bundle if the projected request buffer is not the declared tail byte for byte.
`ProjectedRentMismatch` covers the lamport half. TRADE's brick-safety gate was a
test in `b45d3a2c`; here it is a precondition of obtaining the artifact.

### Refusals a hand-rolled bundle did not have

| Refusal | What it catches |
|---|---|
| `TailFieldRegisterOutOfBank` | A field reading a register at or above the seam bank (scalars ≥ 8, identities ≥ 12). Those hold whatever this bundle's transition put there, and this transition loads only constants — so the field would compose a **silent zero** into the root. Same species as the `GeneralUnwrittenMagic` campaign, caught at authoring. |
| `TailFieldOverwritesConstant` | The tail declared two sources for one byte. |
| `TailAlignment` | A nonzero constant byte no aligned eight-byte write can place. There is no request write narrower than `u64`, so such a tail cannot be composed at all. |
| `TailFieldGeometry` | Fields descending, overlapping, unaligned, or off the tail. |
| `RootWidth` | The declared width disagrees with the tail, is zero, or exceeds `ACTIVATION_MAX_ROLE_REQUEST_BYTES_V2`. |
| `RegisterGeometry` | The composed banks exceed what the seam will run. |

### The completeness check, General's own

`GeneralActivationBundleErrorV1::RuntimeVaryingByteNotDeclared`. The constant tail
is not written down: it is `general_root_creation_tail_v2(market, config, gen)`
with the three declared field regions blanked, computed **twice** from two
unrelated triples and required to agree. Any byte that varies with those inputs
and is not inside a declared field is a runtime-varying byte nobody declared, and
composing it as a constant would produce roots that disagree with
`GeneralRootV2::active` on every market but one. A family that later grows a
market-derived root field learns it here.

### At the set

`build_general_activation_capable_program_set_v1` re-authenticates the bytes it
just wrote with `authenticate_general_program_set_v3`, requires the profile to be
`SettlementWithActivation`, requires the canonical activation request to select
exactly the descriptor it was handed, and requires each of the seven action
requests to select its own V4 descriptor. A caller cannot obtain a set that does
not activate, or one whose activation entry aliases an action's descriptor.

## 5. The evidence

**The template is the reviewed artifact, not a fourth opinion of it.**
`dclutch-direct-codec`'s
`activation_bundle_v1::tests::the_family_neutral_template_reproduces_this_sealed_bundle_byte_for_byte`
rebuilds Direct's sealed activation triple out of
`dclutch-capability-activation-codec` and compares the account profile, the
transition, the effect and the descriptor — all four records and all three
digests — to `build_direct_activation_bundle_v1`'s own output. They are equal. If
that test goes red the template has drifted from the thing that was reviewed and
no family may use it until that is explained.

**General's artifacts agree with General's own root author.**
`the_real_artifacts_compose_exactly_what_general_root_active_composes` runs the
built bundle through the real effect kernel for three unrelated
`(market, config_id, generation)` triples — including `generation = u64::MAX` —
and requires the projection to equal `general_root_creation_tail_v2(...)` byte
for byte and to decode as `GeneralRootV2::active(...)`. That is how a data-defined
activation is reviewed: by reading its OUTPUT with the family's own decoder, not
its instruction list.

**General's shippable artifacts ARE the ones the real Trading ELF has already
run.** `programs/dclutch-trading-sbf/program-test/tests/activation.rs`
(`Campaign::General`) has, since GEN-ART, run the real ELF over a hand-built
General triple and decoded the account it creates as a real `GeneralRootV2`.
Those were fixture bytes, because until now the only General activation
artifacts in the tree were the fixture functions in that file.
`the_shippable_general_bundle_is_the_triple_this_file_runs_on_the_real_elf` is
the join, and it passes: the account profile, the transition and the effect are
BYTE-IDENTICAL to the shippable bundle's, and the descriptor differs at exactly
one 32-byte field — the request schema, which the fixture invented as `id(0x23)`
before General published one — asserted byte by byte so that "nearly the same
descriptor" is not something anyone has to take on trust. The activation seam
never reads a descriptor's request schema (`validate_selection` joins kind,
capacity, root schema and derivation policy against the manifest entry, and no
more), so the difference cannot change what the ELF did.

Counts: `dclutch-capability-activation-codec` 8/8,
`dclutch-general-adapter-contract` `activation_bundle_v1` 7/7,
`dclutch-direct-codec` `activation_bundle_v1` 7/7 (was 6; the byte-identity gate
is the seventh), trading program-test `activation` join 1/1.

## 6. One seam change, and it is publication rather than logic

`outer.rs`'s private `MAX_RUNTIME_SCALARS_V2` / `MAX_RUNTIME_IDENTITIES_V2` /
`MAX_ROLE_REQUEST_BYTES_V2` now come from `activation_registers_v2`, beside the
register coordinates they bound. Same values, one authority. An off-chain builder
refuses an oversized profile against the seam's own numbers instead of a second
copy of them — which is the reason that module exists, in its own words: "an
author who cannot name them writes the numbers down a second time, and the two
authorities drift." No behaviour moved.

## 7. What a family does next, in order

1. Confirm a root tail type exists with a creation oracle. If not, stop — that is
   the real blocker and it is a root-ABI decision.
2. Answer §3's question for every byte. If any byte has neither a constant nor a
   seam register, stop and say so; that family's activation is not authorable.
3. Reserve a selector no request can produce, in the family's own selector width,
   and a request schema of its own.
4. Call `build_activation_bundle_v1`. It refuses rather than returning a bundle
   that would brick.
5. Add the V1-stamped entry to the family's set builder, and relax the set
   authenticator's entry count and blanket-V4 assertion to admit exactly that one
   coordinate.
6. Publish the three records through the family's release closure and prove the
   activation on program-test or a local validator before any cluster.

## 8. What this lane did NOT verify

- **No activation was executed on any cluster, and I started no validator.** The
  ELF evidence is inherited, not re-run: the campaign that creates a real
  `GeneralRootV2` on `solana-program-test` was already green, and this lane
  joined the shippable records to it by byte identity rather than executing it
  again. The join is exact and the argument is short, but it is a transitive
  claim and worth naming as one. What has never run anywhere is a General
  activation driven from a PUBLISHED release rather than a fixture — that waits
  on the publication closure below.
- **The fixture triple is now a second author of records that have a first one.**
  `account_profile(Family::General)`, `transition_program(Family::General)` and
  `effect_program(Campaign::General)` produce exactly what the shippable builder
  produces. Deleting them in favour of the builder is correct and was not done
  here, because `Campaign::GeneralUnwrittenMagic` deliberately omits the two
  constant words and needs a path that can express a wrong artifact.
- ~~**General's publication closure is not wired.**~~ **WIRED THE SAME DAY
  (GENPUB, `b09c4ee9`..`50f68bb5`).** `general_selected_release_v1` emits
  `GeneralReleaseProfileV1::SettlementWithActivation` and publishes the three
  records under the labels `activation-account-profile`, `activation-effect`
  and `activation-descriptor` — three and not four, because
  `CapabilityProgramV1` carries the activation transition inside the
  descriptor, so a fourth record would be a second author for it. Eight
  entries, not seven: `authenticate_set_descriptor` admits only
  `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1` while every action descriptor is
  stamped `v4::SCHEMA_RELEASE_ID`, so the seven-entry profile is **unfoundable**
  rather than smaller. One loopback run compiled 68 publication records where
  it compiled 65, and all five shared/activation records read back
  Registry-owned and byte-identical at the PDA derived from
  `(schema, sha256(body))`. What is still not wired is downstream and is not
  General's: no root is created, because the founding refuses first at `0x5182
  ClaimsFoundingSbfErrorV5::Release` on the DCLTGMF3 Open leg — family-
  independent, with a Direct control refusing identically. See
  `docs/evidence/GENERAL_PUBLICATION_CLOSURE_2026_08_30.md`.
- **No CU measurement.** Nothing here runs on chain, and the seam's cost is
  unchanged: the descriptor, profile and effect are the same shapes the seam
  already interprets.
- **Direct's bundle bytes did not move**, and no Direct release, market input or
  sealed campaign was touched. The byte-identity test is the proof of that, not a
  claim about it.
