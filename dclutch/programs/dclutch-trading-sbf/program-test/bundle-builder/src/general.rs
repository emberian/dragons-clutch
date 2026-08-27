//! The General bundle, and the boundary as GEN-HOT measured it.
//!
//! This module was written as a sketch to the builder's boundary. GEN-HOT went
//! to execute against it and found the boundary is in a different place, so
//! what follows is the measured version, with the executed evidence named
//! beside each claim (`tests/general_dynamic_spans_v1.rs`).
//!
//! # What General already satisfies
//!
//! - **Descriptor generation**: General's V3 line descriptors are
//!   `CapabilityProgramV4` (`general_activation_v3.rs` pins the equivalence
//!   with `initialize_root_account_v1`), so [`crate::artifacts`] consumes the
//!   set unchanged: ten byte strings in, records/action/seal out.
//! - **Request profile**: `RequestProfileV1` (unsigned) — the engine's
//!   simplest arm. No Ed25519 evidence in the scenario at all.
//! - **Routes**: General's effect declares `Claims` and `Custody` roles
//!   (`general-adapter-contract/src/effect_artifacts_v3.rs`), both already
//!   dispatched by [`crate::routes`]; the Custody raw-layout context read and
//!   the disabled-route shadow requests apply as-is.
//! - **Root**: `CapabilityRootHeaderV1 || GeneralRootV2`, corpus content
//!   exactly like Direct's, best produced by
//!   `plan_general_capability_activation_v3` (which also fills the record
//!   bumps the header now carries).
//! - **Lifecycle**: the V5 current-rent-quote policy generation
//!   (`encode_general_state_lifecycle_v5_atomic`), which the engine's preplan
//!   already runs; General's created states (primary/terminal, payer and
//!   RentCredit coordinates from `state_artifacts_v3`) arrive by adoption the
//!   way Direct's maker replays do.
//! - **The eighth-set-entry gap does not block the campaign**: like
//!   `direct-hot`, a ProgramTest campaign encodes its own one-entry
//!   `CapabilityProgramSetV2` selecting General's descriptor. The
//!   exactly-seven rule gates *live release activation*, not a Hot bundle.
//!
//! # The boundary, measured
//!
//! 0. **The effect artifact is the wrong schema, and it is first.** General's
//!    descriptor names `dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID` for its
//!    EffectProgram, and `process_hot_execution_v3` accepts exactly one effect
//!    schema — `v4::SCHEMA_RELEASE_ID_V4` (`decode_sealed_effect_v4` /
//!    `decode_selected_effect_v4` both refuse anything else with
//!    `UnsupportedContent`). Nothing General emits today can enter the Hot
//!    executor, and this refusal sits *ahead* of every seam below: the effect
//!    is decoded to get the register geometry the span widths come from.
//!    Executed as `the_general_effect_artifact_is_v3_and_the_v4_hot_path_refuses_it`.
//!    The migration is an EffectV4 envelope (zero spans, zero borrowed ranges)
//!    over the same V3 program — which moves the effect digest, so the
//!    descriptor, the seal and the ProgramSet identity all move with it.
//!
//! 1. **Nonzero dynamic fixed spans — THREADED, and not the span this module
//!    named.** General's AccountProfile is the spans generation with
//!    `dynamic_fixed_span_count() == 1`, but that span is *not* a
//!    "candidate-page span whose width is a request-owned common scalar". It is
//!    the trailing Trading-owned **authenticated scratch-page** span
//!    (`account_rules_v3::general_scratch_page_span_v3`), selector
//!    `scalar::INPUT_SCRATCH_PAGE_COUNT`, and **no General RequestProfile
//!    writes that register** — the seven of them are checked in
//!    `the_sole_general_span_selector_is_not_request_owned`. So the width can
//!    never come from projecting the family request. It comes from the
//!    canonical register-bank geometry:
//!    `classify_bank_transport_v2(general_hot_scalar_count_v3(N),
//!    GENERAL_HOT_COMMON_IDENTITIES_V3)`.
//!
//!    [`crate::registers::derive_dynamic_span_widths`] is that rule, host-side,
//!    phase for phase against `hot_v3::authenticate_dynamic_span_widths_v3`,
//!    and the widths now thread through [`crate::profile_ops`], the engine's
//!    projection and effect-permission arms, and `expand`-time packing. The
//!    Direct reproduction is unmoved (it declares zero spans and still packs
//!    byte-identically); General's real profile expands to
//!    `general_account_profile_fixed_count_v3(action) + page_count` at N = 1, 4
//!    and 258.
//!
//! 2. **Execution-strategy extras — NOT optional, and not a campaign's choice.**
//!    This module said a first GEN bundle "can run interpreted; the extras seam
//!    opens when a GEN campaign wants the accelerated disposition". It cannot.
//!    An AccountProfile-only span — one no request writes — is admissible under
//!    exactly one disposition: `StrategyDispositionV2::AdmittedAot`
//!    (`authenticate_dynamic_span_widths_v3`, the `else` arm). General's
//!    profile therefore *forces* the accelerated disposition, and the emitted
//!    strategy is `AdmittedAot` accordingly. Executed as
//!    `a_profile_only_span_refuses_every_disposition_but_admitted_aot`.
//!
//!    What that costs the builder, exactly. [`crate::bundle`] emits
//!    `fixed(39) ++ runtime[5..]`. An admitted-AOT bundle is
//!    `fixed(39) ++ extras(8) ++ admitted_caller_authorities(page_count)
//!    ++ runtime[5..]`, where the eight extras are the Registry-authenticated
//!    Strategy/Certificate/Admission/ArtifactRelease/Loader observation plus the
//!    accelerator program and its ProgramData
//!    (`admitted_composition_v3`, `execution_strategy_v2`), and the authority
//!    count is `admitted_caller_authority_count_v3` — the same page count as the
//!    span. The campaign must also deploy the real accelerator ELF, because
//!    Trading CPIs into it as the sole candidate authority. None of that is
//!    modelled here yet, and it is the next builder-side lane.
//!
//! 3. **Core/Resolution route authorities.** [`crate::routes`] derives no
//!    authority for `FixedRole::Core`/`Resolution` frames. General's declared
//!    routes do not need them; the seam matters for founding/resolution
//!    families, and the rule to mirror lives in `core_composition_v3` /
//!    `resolution_composition_v3::prepare`.
//!
//! 4. **Receipt dependencies at build time.** The engine resolves invocation
//!    geometry including receipt dependencies but derives nothing from them
//!    (they are execution-time verification). If a family's *frame content*
//!    ever depends on a prior child's receipt, the builder cannot know it
//!    statically — no current family does.
//!
//! # The GEN campaign, shaped
//!
//! ```text
//! artifact set   = GeneralArtifactBytesV3 (descriptor, profile, request
//!                  profile, transition, effect **in a V4 envelope**, lifecycle,
//!                  admitted-AOT strategy) + one-entry ProgramSetV2 + manifest
//!                  + GeneralConfigV3
//! fixed corpus   = Market (CoreState), composite root, four Product records
//! strategy frame = eight authenticated extras + one Trading caller authority
//!                  per bank page + the deployed accelerator ELF
//! bindings       = General's runtime self-coordinates: payer(s), RentCredit,
//!                  claims aggregate + positions, realm + custody replay +
//!                  vault/token rows, program/programdata restatements with
//!                  chain views — the same classes Direct bound, at General's
//!                  coordinates (state_artifacts_v3 names them)
//! derived        = span widths, records, seal, packing, privileges, funding,
//!                  created states, caller authorities — identical machinery
//! ```
