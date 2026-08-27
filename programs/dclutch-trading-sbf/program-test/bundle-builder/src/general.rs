//! The General bundle, sketched to the builder's boundary.
//!
//! Evidence that the builder generalizes: what a GEN-HOT campaign supplies,
//! what it inherits, and the exact seams that stand between this crate and
//! General's first hot bundle. The full campaign is the next lane's job on
//! the decomposed `hot_v3`; this module names its build surface.
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
//! # The boundary, named
//!
//! 1. **Nonzero dynamic fixed spans.** General's AccountProfile is the
//!    spans generation with `dynamic_fixed_span_count() == 1` (the
//!    candidate-page span; its width is a request-owned common scalar).
//!    This crate currently passes `&[]` everywhere ([`crate::profile_ops`]).
//!    The seam is mechanical: derive `span_counts` the way
//!    `hot_v3::authenticate_dynamic_span_widths` does — project the
//!    request-owned selector scalars once from the family bytes, validate
//!    against each span's finite congruence — then thread the vector through
//!    `profile_ops`, the engine's projection branches (already split on
//!    `uses_dynamic_fixed_spans`), `expand`-time packing, and the effect's
//!    `account_count(tail, scalars)`.
//! 2. **Execution-strategy extras.** Direct's interpreted strategy adds no
//!    accounts after the fixed frame. A General run under a scratch-page or
//!    admitted-AOT strategy inserts strategy evidence accounts at
//!    `HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3`, which [`crate::bundle`] does
//!    not yet model. The first GEN bundle (the zombie refusal executed
//!    through the real runtime path, then first hot success) can run
//!    interpreted; the extras seam opens when a GEN campaign wants the
//!    accelerated disposition.
//! 3. **Core/Resolution route authorities.** [`crate::routes`] derives no
//!    authority for `FixedRole::Core`/`Resolution` frames. General's declared
//!    routes do not need them; the seam matters for founding/resolution
//!    families, and the rule to mirror lives in `core_composition_v3` /
//!    `resolution_composition_v3::prepare`.
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
//!                  profile, transition, effect, lifecycle, strategy)
//!                  + one-entry ProgramSetV2 + manifest + GeneralConfigV3
//! fixed corpus   = Market (CoreState), composite root, four Product records
//! bindings       = General's runtime self-coordinates: payer(s), RentCredit,
//!                  claims aggregate + positions, realm + custody replay +
//!                  vault/token rows, program/programdata restatements with
//!                  chain views — the same classes Direct bound, at General's
//!                  coordinates (state_artifacts_v3 names them)
//! derived        = records, seal, packing, privileges, funding, created
//!                  states, caller authorities — identical machinery
//! ```
