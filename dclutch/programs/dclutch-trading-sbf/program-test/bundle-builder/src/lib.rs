//! Family-generic chain-fixture builder derived from the emitted artifacts.
//!
//! A family ProgramTest campaign is `artifact set + request corpus`; everything
//! else is derivation. This crate is the derivation, made executable:
//!
//! - **Derived from the artifact set alone**: every finalized artifact record
//!   account (raw + staging PDAs of the Registry, bytes = the artifacts), the
//!   validated-artifact seal (key and body), artifact identities, and the
//!   fixed Hot frame those records occupy.
//! - **Derived from the [`AccountProfileV2`]**: logical-to-physical packing,
//!   alias resolution, per-account writable/signer privileges, exact data
//!   widths, and rent funding. The campaign never states an alias, a
//!   privilege, or a width; the builder refuses a binding that contradicts
//!   the profile.
//! - **Derived by running the emitted artifacts host-side**: the projection
//!   register file (account projection, rent quotes, native signatures,
//!   request projection), the lifecycle preplan (which yields the addresses of
//!   every account the lifecycle will create), the transition fold, and the
//!   effect projection (which yields the projected child-request bank). The
//!   caller-authority coordinates fall out of the bank: each child frame's
//!   first account is the PDA of that invocation's projected request digest.
//!   No family campaign restates transition arithmetic ever again.
//! - **Corpus** (the choices the artifacts do not determine): the family
//!   request bytes and their signatures, keypairs, the semantic prestate
//!   accounts (Market state, Claims aggregates and Positions, Custody replay,
//!   RentCredit, token accounts, product/realm record contents), scenario
//!   scalars (generation, slot, revisions, balances), and the environment
//!   facts of the release waist (program identities, activation cache,
//!   release-set identity).
//!
//! The doctrine this executes: *the fixture is never the authority*. Where the
//! reference hand-built fixture (`direct-hot`) reproduced the transition's
//! arithmetic to state the four Custody child requests, this builder executes
//! the emitted transition and effect programs through the same shared kernels
//! the chain runs, so a fixture field is either corpus or the output of the
//! real artifact semantics.

#![forbid(unsafe_code)]

pub mod admitted;
pub mod artifacts;
pub mod bundle;
pub mod frame;
pub mod general;
pub mod registers;
pub mod routes;

/// Spans-aware profile queries.
///
/// A spans-typed profile (artifact profile 13) answers alias and geometry
/// queries only through its `_with_dynamic_spans` variants, even at zero
/// declared spans. Every query here takes the authenticated span widths —
/// empty for a profile that declares none, and derived by
/// [`crate::registers::derive_dynamic_span_widths`] otherwise, never stated by
/// a campaign. A non-spans profile is refused a nonempty width vector, exactly
/// as `hot_v3::expand_runtime_accounts_v3` refuses one.
pub mod profile_ops {
    use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountGeometryV2};

    use crate::BuilderError;

    /// Refuse span widths a non-spans profile cannot carry.
    fn checked(profile: AccountProfileV2<'_>, spans: &[u32]) -> Result<bool, BuilderError> {
        let dynamic = profile.uses_dynamic_fixed_spans();
        if dynamic {
            if spans.len() != usize::from(profile.dynamic_fixed_span_count()) {
                return Err(BuilderError::Profile(line!()));
            }
        } else if !spans.is_empty() {
            return Err(BuilderError::Profile(line!()));
        }
        Ok(dynamic)
    }

    /// Logical account count at this tail.
    pub fn logical_count(
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        spans: &[u32],
    ) -> Result<usize, BuilderError> {
        if checked(profile, spans)? {
            profile.logical_account_count_with_dynamic_spans(tail_count, spans)
        } else {
            profile.logical_account_count(tail_count)
        }
        .map_err(|_| BuilderError::Profile(line!()))
    }

    /// Physical account count at this tail.
    pub fn physical_count(
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        spans: &[u32],
    ) -> Result<usize, BuilderError> {
        if checked(profile, spans)? {
            profile.physical_account_count_with_dynamic_spans(tail_count, spans)
        } else {
            profile.physical_account_count(tail_count)
        }
        .map_err(|_| BuilderError::Profile(line!()))
    }

    /// Canonical representative coordinate of one logical coordinate.
    pub fn representative(
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        spans: &[u32],
        coordinate: usize,
    ) -> Result<usize, BuilderError> {
        if checked(profile, spans)? {
            profile.representative_with_dynamic_spans(tail_count, spans, coordinate)
        } else {
            profile.representative(tail_count, coordinate)
        }
        .map_err(|_| BuilderError::Profile(line!()))
    }

    /// Packed physical ordinal of one logical coordinate.
    pub fn ordinal(
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        spans: &[u32],
        coordinate: usize,
    ) -> Result<usize, BuilderError> {
        if checked(profile, spans)? {
            profile.physical_account_ordinal_with_dynamic_spans(tail_count, spans, coordinate)
        } else {
            profile.physical_account_ordinal(tail_count, coordinate)
        }
        .map_err(|_| BuilderError::Profile(line!()))
    }

    /// Kernel-owned geometry of one physical ordinal.
    pub fn geometry(
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        spans: &[u32],
        physical_ordinal: usize,
    ) -> Result<PhysicalAccountGeometryV2, BuilderError> {
        if checked(profile, spans)? {
            profile.physical_account_geometry_with_dynamic_spans(
                tail_count,
                spans,
                physical_ordinal,
            )
        } else {
            profile.physical_account_geometry(tail_count, physical_ordinal)
        }
        .map_err(|_| BuilderError::Profile(line!()))
    }
}

use solana_program::pubkey::Pubkey;

/// Stable refusal from any stage of bundle construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderError {
    /// An artifact refused decoding or was internally inconsistent.
    Artifact,
    /// The account profile refused the supplied binding geometry
    /// (the payload is the refusing source line).
    Profile(u32),
    /// A corpus binding contradicted an artifact-declared fact
    /// (the payload is the refusing source line).
    Binding(u32),
    /// The host projection pipeline refused (see the stage name).
    Projection(&'static str),
    /// Dynamic fixed-span width derivation refused (see the stage name); the
    /// stage names mirror `hot_v3::authenticate_dynamic_span_widths_v3`.
    Spans(&'static str),
    /// The lifecycle preplan refused or derived an unusable plan.
    Lifecycle(&'static str),
    /// A child route's request kind is not yet understood by the builder
    /// (the payload is the refusing source line).
    ///
    /// Twenty-two sites publish this, which during a wall era makes it a
    /// universal donor: `routes.rs` refuses here for a request that does not
    /// decode, a frame width that does not match, a role the activation cache
    /// does not name, and a plan whose geometry the builder cannot express.
    /// The line is the difference between "some route" and one call site.
    UnsupportedRoute(u32),
    /// Arithmetic or width joins failed.
    Arithmetic,
}

/// Externally installed release-waist and deployment identities.
///
/// These are environment facts owned by the enclosing Registry campaign; the
/// builder consumes them and never creates a second release truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaistFactsV1 {
    /// Registry program owning finalized records and activation.
    pub registry_program: Pubkey,
    /// Current Trading program (the Hot executor).
    pub trading_program: Pubkey,
    /// Current Core program.
    pub core_program: Pubkey,
    /// Current Claims program.
    pub claims_program: Pubkey,
    /// Current Custody program.
    pub custody_program: Pubkey,
    /// Exact immutable execution-release-set content identity.
    pub release_set: [u8; 32],
    /// Current complete activation cache account.
    pub activation_cache: Pubkey,
    /// Trading interpreter semantic release (a seal seed; decision 0005).
    pub trading_semantic_release: [u8; 32],
}
