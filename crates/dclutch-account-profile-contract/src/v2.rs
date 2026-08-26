//! Runtime-tail account projection profiles.
//!
//! One authenticated V2 profile owns a fixed account prefix and one account
//! rule/operation template repeated for an authenticated `u32` item count.
//! Item count is supplied by Product state, never by request bytes. The kernel
//! validates all checked affine widths before its first loop and copies to the
//! candidate output only after every account, alias, relation, and projection
//! accepts.

use core::convert::TryInto;

use dclutch_effect_kernel::v2::AccountPermission;

use super::{
    AccountObservationV1, EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
    EFFECT_PERMISSION_WRITE_DATA,
};

/// Safe, allocation-free typed AccountProfile V2 artifact encoder.
pub mod encode;
/// Profile 14 fixed-data prestate predicate semantics.
pub mod profile14;

#[path = "v2/generated_profile14.rs"]
#[allow(dead_code, missing_docs)]
mod generated_profile14;

pub use generated_profile14::{
    FIXED_DATA_PREDICATE_ARTIFACT_PROFILE, FIXED_DATA_PREDICATE_BYTES,
    FIXED_DATA_PREDICATE_COUNT_OFFSET, FIXED_DATA_PREDICATE_HEADER_BYTES,
    FIXED_DATA_PREDICATE_PROFILE_ID, FIXED_DATA_PREDICATE_PROFILE_PREIMAGE,
};
pub use profile14::{FixedDataPredicateKindV2, FixedDataPredicateV2};

/// Canonical runtime-tail profile magic.
pub const MAGIC: [u8; 8] = *b"DCLTAP02";
/// Finalized-record schema label for runtime-tail account profiles.
pub const SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/account-profile-v2";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE`].
pub const SCHEMA_RELEASE_ID: [u8; 32] = [
    0x4b, 0x66, 0x56, 0x93, 0x89, 0x0c, 0x76, 0x23, 0xb5, 0x65, 0x2b, 0x82, 0xe8, 0x5b, 0x26, 0x4a,
    0xc1, 0xa5, 0x26, 0xe7, 0x6a, 0x3d, 0x8e, 0x3c, 0x8c, 0x1d, 0xd4, 0xd4, 0x6c, 0xc8, 0xe7, 0xfc,
];
/// Canonical runtime-tail profile schema.
pub const VERSION: u16 = 2;
/// Canonical runtime-tail physical profile.
pub const ARTIFACT_PROFILE: u16 = 2;
/// Runtime-tail physical profile with affine account lengths and selected windows.
pub const SELECTED_WINDOW_ARTIFACT_PROFILE: u16 = 3;
/// Selected-window profile with checked narrow integer projections into `u64` scalars.
pub const TYPED_SCALAR_ARTIFACT_PROFILE: u16 = 4;
/// Typed-scalar profile with an optional trusted current-slot environment scalar.
pub const TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE: u16 = 5;
/// Trusted-environment profile with lifecycle-bound alternative account prestates.
pub const LIFECYCLE_PRESTATE_ARTIFACT_PROFILE: u16 = 6;
/// Lifecycle-prestate profile admitting explicit adapter-authenticated
/// variable-width, readonly fixed-prefix records.
pub const ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE: u16 = 7;
/// Variable-data successor with an optional trusted current-executing-program identity.
pub const TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE: u16 = 8;
/// Trusted-program successor admitting only non-owning aliases of an earlier
/// adapter-authenticated variable-data representative.
pub const ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE: u16 = 9;
/// Variable-alias successor deriving one protected support width by counting
/// nonzero `u64` rows in an authenticated immutable descriptor tail.
pub const NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE: u16 = 10;
/// Successor profile with one authenticated physical representative per alias
/// group, route-local privilege subsets, and an optional trusted System
/// Program identity supplied by the adapter.
pub const AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE: u16 = 11;
/// Successor profile deriving an exact ordered sparse `u64` support into a
/// descriptor-specialized flat common scalar row bank.
pub const NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE: u16 = 12;
/// Route-alias successor with one checked dynamic account span inserted into
/// an otherwise fixed logical account sequence.
pub const DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE: u16 = 13;
/// Exact V2 header width.
pub const HEADER_BYTES: usize = 32;
/// Exact profile-8 header width including the trusted role-identity declaration.
pub const TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES: usize = 36;
/// Exact account-rule width.
pub const RULE_BYTES: usize = 16;
/// Exact projection-operation width.
pub const OPERATION_BYTES: usize = 16;
/// Little-endian trusted-environment scalar-coordinate offset.
pub const TRUSTED_ENVIRONMENT_SCALAR_OFFSET: usize = 28;
/// Trusted-environment kind-tag offset.
pub const TRUSTED_ENVIRONMENT_KIND_OFFSET: usize = 30;
/// Trusted-environment reserved-byte offset.
pub const TRUSTED_ENVIRONMENT_RESERVED_OFFSET: usize = 31;
/// Little-endian trusted current-executing-program identity-coordinate offset.
pub const TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET: usize = 32;
/// Trusted current-executing-program kind-tag offset.
pub const TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET: usize = 34;
/// Trusted current-executing-program reserved-byte offset.
pub const TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET: usize = 35;
/// Exact profile-11 header width including one trusted builtin identity.
pub const AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES: usize = 40;
/// Little-endian trusted builtin identity-coordinate offset.
pub const TRUSTED_BUILTIN_IDENTITY_OFFSET: usize = 36;
/// Trusted builtin kind-tag offset.
pub const TRUSTED_BUILTIN_KIND_OFFSET: usize = 38;
/// Trusted builtin reserved-byte offset.
pub const TRUSTED_BUILTIN_RESERVED_OFFSET: usize = 39;
/// Exact profile-13 fixed header width before the canonical span table.
pub const DYNAMIC_FIXED_SPAN_HEADER_BYTES: usize = 48;
/// Number of canonical dynamic-span table entries.
pub const DYNAMIC_FIXED_SPAN_COUNT_OFFSET: usize = 40;
/// Reserved zero bytes after the span-table count.
pub const DYNAMIC_FIXED_SPAN_RESERVED_OFFSET: usize = 42;
/// Exact width of one dynamic-span table entry.
pub const DYNAMIC_FIXED_SPAN_ENTRY_BYTES: usize = 20;
/// Base-logical insertion coordinate within one span entry.
pub const DYNAMIC_FIXED_SPAN_ENTRY_INSERTION_OFFSET: usize = 0;
/// Common scalar selecting one span count.
pub const DYNAMIC_FIXED_SPAN_ENTRY_COUNT_SCALAR_OFFSET: usize = 2;
/// First rule template owned by one span entry.
pub const DYNAMIC_FIXED_SPAN_ENTRY_RULE_START_OFFSET: usize = 4;
/// Number of account rules repeated per selected span item.
pub const DYNAMIC_FIXED_SPAN_ENTRY_RULE_STRIDE_OFFSET: usize = 6;
/// Inclusive minimum admitted span count.
pub const DYNAMIC_FIXED_SPAN_ENTRY_MIN_OFFSET: usize = 8;
/// Inclusive maximum admitted span count.
pub const DYNAMIC_FIXED_SPAN_ENTRY_MAX_OFFSET: usize = 12;
/// Positive congruence step for admitted counts.
pub const DYNAMIC_FIXED_SPAN_ENTRY_STEP_OFFSET: usize = 16;

const TRUSTED_ENVIRONMENT_NONE: u8 = 0;
const TRUSTED_ENVIRONMENT_CURRENT_SLOT: u8 = 1;
const TRUSTED_EXECUTING_PROGRAM_NONE: u8 = 0;
const TRUSTED_EXECUTING_PROGRAM_CURRENT: u8 = 1;
const TRUSTED_BUILTIN_NONE: u8 = 0;
const TRUSTED_BUILTIN_SYSTEM_PROGRAM: u8 = 1;

const OP_REQUIRE_KEY: u8 = 0;
const OP_REQUIRE_OWNER: u8 = 1;
const OP_PROJECT_KEY: u8 = 2;
const OP_PROJECT_OWNER: u8 = 3;
const OP_PROJECT_LAMPORTS: u8 = 4;
const OP_PROJECT_DATA_U64: u8 = 5;
const OP_PROJECT_DATA_IDENTITY: u8 = 6;
const OP_PROJECT_DATA_U32: u8 = 7;
const OP_PROJECT_TAIL_COUNT_U32: u8 = 8;
const OP_PROJECT_DATA_U64_AFFINE: u8 = 9;
const OP_PROJECT_DATA_IDENTITY_AFFINE: u8 = 10;
const OP_SELECT_DATA_WINDOW: u8 = 11;
const OP_PROJECT_DATA_U64_SELECTED: u8 = 12;
const OP_PROJECT_DATA_IDENTITY_SELECTED: u8 = 13;
const OP_PROJECT_DATA_U64_SELECTED_AFFINE: u8 = 14;
const OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE: u8 = 15;
const OP_PROJECT_DATA_U16: u8 = 16;
const OP_PROJECT_DATA_U8: u8 = 17;
const OP_PROJECT_NONZERO_U64_TAIL_COUNT: u8 = 18;
const OP_PROJECT_NONZERO_U64_TAIL_ROWS: u8 = 19;

/// Encode one fixed-account `u8` data projection into a common `u64` scalar.
///
/// The destination receives the exact zero-extended byte. All arguments are
/// validated before `output` is mutated.
pub fn encode_project_data_u8_operation_v2(
    output: &mut [u8],
    account: u16,
    register: u16,
    data_offset: u32,
) -> Result<()> {
    if output.len() != OPERATION_BYTES || data_offset.checked_add(1).is_none() {
        return Err(Error::InvalidLength);
    }
    let mut candidate = [0_u8; OPERATION_BYTES];
    *candidate.first_mut().ok_or(Error::InvalidLength)? = OP_PROJECT_DATA_U8;
    candidate
        .get_mut(2..4)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&account.to_le_bytes());
    candidate
        .get_mut(6..8)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&register.to_le_bytes());
    candidate
        .get_mut(8..12)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&data_offset.to_le_bytes());
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Encode one fixed-account `u16` data projection into a common `u64` scalar.
///
/// This operation is admitted by [`TYPED_SCALAR_ARTIFACT_PROFILE`] and its
/// [`TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE`] successor. The destination scalar
/// receives the zero-extended little-endian value. All arguments are validated
/// before `output` is mutated.
pub fn encode_project_data_u16_operation_v2(
    output: &mut [u8],
    account: u16,
    register: u16,
    data_offset: u32,
) -> Result<()> {
    if output.len() != OPERATION_BYTES || data_offset.checked_add(2).is_none() {
        return Err(Error::InvalidLength);
    }
    let mut candidate = [0_u8; OPERATION_BYTES];
    *candidate.get_mut(0).ok_or(Error::InvalidLength)? = OP_PROJECT_DATA_U16;
    candidate
        .get_mut(2..4)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&account.to_le_bytes());
    candidate
        .get_mut(6..8)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&register.to_le_bytes());
    candidate
        .get_mut(8..12)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&data_offset.to_le_bytes());
    output.copy_from_slice(&candidate);
    Ok(())
}

/// Stable hostile-decode or projection refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Bytes or caller-owned banks did not have their exact checked width.
    InvalidLength,
    /// Magic did not identify the V2 account profile.
    InvalidMagic,
    /// The schema version or artifact profile is unsupported.
    UnsupportedProfile,
    /// Header, rule, or operation reserved bytes were nonzero.
    NonCanonicalReserved,
    /// The profile declared no physical account or register bank.
    EmptyProfile,
    /// Privilege bits were not signer/writable/executable only.
    InvalidPrivileges,
    /// Effect permissions were unknown or required a readonly account.
    InvalidEffectPermissions,
    /// A debit/data-write account lacked an authenticated owner relation.
    EffectOwnerUnanchored,
    /// An alias was forward, cross-item, or otherwise noncanonical.
    InvalidAlias,
    /// An operation tag or fixed/item space tag was unknown.
    UnknownOperation,
    /// An operation carried a noncanonical inactive field.
    NonCanonicalOperation,
    /// An account or register coordinate exceeded its declared address space.
    InvalidCoordinate,
    /// Exact accounts, register banks, or permission output had another width.
    WidthMismatch,
    /// Runtime privileges did not match the authenticated rule.
    PrivilegeMismatch,
    /// Runtime observations did not realize the canonical alias partition.
    AliasMismatch,
    /// A distinct representative reused another representative's key.
    CrossItemAlias,
    /// Runtime account data had another exact length.
    DataLengthMismatch,
    /// An authenticated key or owner relation was false.
    IdentityMismatch,
    /// A projected data field exceeded its authenticated account width.
    DataOutOfBounds,
    /// Two profile operations targeted the same output register.
    DuplicateProjection,
    /// Trusted-environment tag, reserved byte, or scalar coordinate was invalid.
    InvalidTrustedEnvironment,
    /// A projection attempted to overwrite a trusted-environment scalar.
    TrustedEnvironmentOverwrite,
    /// Trusted-role tag, reserved byte, or identity coordinate was invalid.
    InvalidTrustedExecutingProgram,
    /// A projection attempted to overwrite the trusted current-program identity.
    TrustedExecutingProgramOverwrite,
    /// A lifecycle-bound alternative prestate was malformed or used outside profile 6.
    InvalidLifecyclePrestate,
    /// A variable-width prestate was malformed, unauthenticated, or used outside profile 7.
    InvalidVariableDataPrestate,
    /// An authenticated `u64` tail had no nonzero row.
    EmptyNonzeroTail,
    /// A route alias asserted independent state or invalid privilege semantics.
    InvalidRouteAlias,
    /// A trusted builtin tag, reserved byte, or identity coordinate was invalid.
    InvalidTrustedBuiltin,
    /// A projection attempted to overwrite a trusted builtin identity.
    TrustedBuiltinOverwrite,
    /// Derived sparse support did not exactly fill the artifact-owned row bank.
    SupportRowCountMismatch,
    /// Dynamic fixed-span geometry or its authenticated count was invalid.
    InvalidDynamicSpan,
    /// An opaque-data observation asserted local data authority.
    InvalidOpaqueDataPrestate,
    /// A fixed-data predicate was malformed, noncanonical, or targeted an ineligible rule.
    InvalidFixedDataPredicate,
    /// A live account did not satisfy an authenticated fixed-data predicate.
    FixedDataPredicateMismatch,
}

/// Result alias for runtime-tail profiles.
pub type Result<T> = core::result::Result<T, Error>;

/// Trusted runtime environment supplied outside account/request projection.
///
/// The neutral kernel names only the semantic observation and its destination.
/// It does not name an SVM sysvar or accept a caller-supplied Clock account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedEnvironmentV2 {
    /// No trusted-environment scalar is declared.
    None,
    /// Current trusted runtime slot, seeded by the outer before projection.
    CurrentSlot {
        /// Common scalar coordinate receiving the trusted slot.
        destination: u16,
    },
}

/// Trusted current role identity supplied by the authenticated outer.
///
/// This declares only a common identity destination. The outer is responsible
/// for seeding the Registry-authenticated current executing program before
/// account projection; caller suffix accounts never supply this fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedIdentityEnvironmentV2 {
    /// No trusted role identity is declared.
    None,
    /// Registry-authenticated current executing program.
    CurrentExecutingProgram {
        /// Common identity coordinate receiving the authenticated program ID.
        destination: u16,
    },
}

/// Trusted immutable builtin identity supplied by the runtime adapter.
///
/// The neutral kernel names the semantic role and destination only. It does
/// not contain an SVM public key or accept an instruction-supplied account as
/// authority for this value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedBuiltinIdentityV2 {
    /// No builtin identity is declared.
    None,
    /// Canonical System Program identity used as the owner of vacant accounts.
    SystemProgram {
        /// Common identity coordinate receiving the adapter-trusted value.
        destination: u16,
    },
}

/// Route-local privilege requirements for one logical account coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteAccountPrivilegesV2 {
    bits: u8,
}

impl RouteAccountPrivilegesV2 {
    /// Whether the child route requires the account to sign.
    pub const fn signer(self) -> bool {
        self.bits & 1 != 0
    }

    /// Whether the child route requires writable access.
    pub const fn writable(self) -> bool {
        self.bits & 2 != 0
    }

    /// Whether the physical account must be executable.
    pub const fn executable(self) -> bool {
        self.bits & 4 != 0
    }
}

/// Scalar or identity register kind inspected for profile write authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionRegisterKindV2 {
    /// Scalar register bank.
    Scalar,
    /// Identity register bank.
    Identity,
}

/// Common or current-item register space inspected for write authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionRegisterSpaceV2 {
    /// Common register prefix.
    Common,
    /// Current Product-item stride.
    Item,
}

/// One typed logical register destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionTargetV2 {
    /// Scalar or identity bank.
    pub kind: ProjectionRegisterKindV2,
    /// Common or item-relative register space.
    pub space: ProjectionRegisterSpaceV2,
    /// Local register coordinate.
    pub index: u16,
}

impl TrustedEnvironmentV2 {
    /// Current-slot destination, when selected.
    pub const fn current_slot_destination(self) -> Option<u16> {
        match self {
            Self::None => None,
            Self::CurrentSlot { destination } => Some(destination),
        }
    }
}

impl TrustedIdentityEnvironmentV2 {
    /// Current-executing-program destination, when selected.
    pub const fn current_executing_program_destination(self) -> Option<u16> {
        match self {
            Self::None => None,
            Self::CurrentExecutingProgram { destination } => Some(destination),
        }
    }
}

impl TrustedBuiltinIdentityV2 {
    /// Trusted System Program destination, when selected.
    pub const fn system_program_destination(self) -> Option<u16> {
        match self {
            Self::None => None,
            Self::SystemProgram { destination } => Some(destination),
        }
    }
}

/// Canonical account-alias space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AliasKindV2 {
    /// This coordinate is its own physical representative.
    SelfCoordinate,
    /// This coordinate aliases one fixed-prefix coordinate.
    Fixed,
    /// This coordinate aliases one earlier coordinate in the same item.
    SameItem,
}

/// Exact account-data prestate admitted by one rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountPrestateV2 {
    /// The account must have its declared exact data width.
    Exact,
    /// The lifecycle policy admits either vacant data or the declared live width.
    LifecycleBound,
    /// The runtime adapter authenticated the exact nonempty record body and
    /// the profile declares a checked fixed prefix rather than its full width.
    AdapterAuthenticatedVariableData,
    /// This fixed-prefix coordinate is a non-owning route alias of an earlier
    /// adapter-authenticated variable-data representative. It inherits that
    /// representative's exact observation and carries no independent width or
    /// authentication bit.
    AdapterAuthenticatedVariableDataAlias,
    /// This later fixed logical coordinate borrows every account fact from one
    /// earlier physical representative while declaring only its own child-route
    /// privilege subset.
    AuthenticatedRouteAlias,
    /// Key, owner, lamports, and privileges are authenticated, while account
    /// data is semantically opaque to this profile and receives no projection
    /// or local effect authority.
    AuthenticatedOpaqueReadonlyData,
}

/// Exact data-shape contract for one unique physical representative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalAccountDataGeometryV2 {
    /// The account body has exactly `bytes` bytes.
    Exact {
        /// Exact account-data byte width.
        bytes: usize,
    },
    /// Lifecycle policy admits either vacant data or exactly `live_bytes`.
    VacantOrExact {
        /// Exact byte width after lifecycle creation or authentication.
        live_bytes: usize,
    },
    /// The adapter authenticates a variable body with this checked minimum.
    AdapterAuthenticatedVariable {
        /// Checked fixed-prefix byte width.
        minimum_bytes: usize,
    },
    /// The profile authenticates no account-data semantics for this identity.
    Opaque,
}

/// Kernel-owned geometry for one packed physical account representative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalAccountGeometryV2 {
    logical_representative: usize,
    privileges: RouteAccountPrivilegesV2,
    rule: AccountRuleV2,
    data: PhysicalAccountDataGeometryV2,
}

impl PhysicalAccountGeometryV2 {
    /// Canonical logical self-coordinate represented by this physical account.
    pub const fn logical_representative(self) -> usize {
        self.logical_representative
    }

    /// Exact physical signer/writable/executable privilege union.
    pub const fn privileges(self) -> RouteAccountPrivilegesV2 {
        self.privileges
    }

    /// Semantic-owner rule for this unique representative.
    pub const fn rule(self) -> AccountRuleV2 {
        self.rule
    }

    /// Exact, lifecycle, variable-prefix, or opaque data geometry.
    pub const fn data(self) -> PhysicalAccountDataGeometryV2 {
        self.data
    }
}

/// One descriptor-owned dynamic account span inserted into a fixed sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicFixedSpanV2 {
    insertion_coordinate: u16,
    count_scalar: u16,
    rule_start: u16,
    rule_stride: u16,
    minimum: u32,
    maximum: u32,
    step: u32,
}

impl DynamicFixedSpanV2 {
    /// Base logical coordinate at which expanded span items begin.
    pub const fn insertion_coordinate(self) -> u16 {
        self.insertion_coordinate
    }

    /// Common scalar that supplies the selected account width.
    pub const fn count_scalar(self) -> u16 {
        self.count_scalar
    }

    /// First account-rule template owned by this entry.
    pub const fn rule_start(self) -> u16 {
        self.rule_start
    }

    /// Width of the account-rule template cycled across the selected span.
    pub const fn rule_stride(self) -> u16 {
        self.rule_stride
    }

    /// Inclusive minimum admitted account width.
    pub const fn minimum(self) -> u32 {
        self.minimum
    }

    /// Inclusive maximum admitted account width.
    pub const fn maximum(self) -> u32 {
        self.maximum
    }

    /// Positive finite-congruence step.
    pub const fn step(self) -> u32 {
        self.step
    }

    /// Require one selected width to lie in the descriptor's exact finite
    /// congruence and contain a whole number of rule templates.
    pub fn validate_count(self, count: u32) -> Result<()> {
        if self.step == 0
            || count < self.minimum
            || count > self.maximum
            || !(count - self.minimum).is_multiple_of(self.step)
            || !count.is_multiple_of(u32::from(self.rule_stride))
        {
            Err(Error::InvalidDynamicSpan)
        } else {
            Ok(())
        }
    }

    /// Read and validate the declared common scalar without accepting the
    /// supplied account-vector length as width authority.
    pub fn count_from_scalars(self, scalars: &[u64]) -> Result<u32> {
        let count = u32::try_from(
            *scalars
                .get(usize::from(self.count_scalar))
                .ok_or(Error::InvalidCoordinate)?,
        )
        .map_err(|_| Error::InvalidDynamicSpan)?;
        self.validate_count(count)?;
        Ok(count)
    }
}

/// Hostile-decoded account rule template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRuleV2 {
    privileges: u8,
    effect_permissions: u8,
    alias_kind: AliasKindV2,
    prestate: AccountPrestateV2,
    alias_index: u16,
    data_length: u32,
    data_item_stride: u32,
}

/// Unique fixed-prefix source selected as the authenticated runtime tail width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TailCountProjectionV2 {
    account: u16,
    register: u16,
    data_offset: u32,
}

/// One fixed authenticated descriptor tail whose nonzero `u64` rows derive a
/// common support-width scalar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonzeroU64TailCountProjectionV2 {
    account: u16,
    destination: u16,
    tail_offset: u32,
}

/// One fixed authenticated descriptor tail projected as exact ordered sparse
/// `(outcome, coefficient)` rows into a flat common scalar bank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonzeroU64TailRowsProjectionV2 {
    account: u16,
    count_destination: u16,
    rows_destination: u16,
    tail_offset: u32,
    row_scalar_stride: u16,
}

impl NonzeroU64TailRowsProjectionV2 {
    /// Fixed adapter-authenticated descriptor account.
    pub const fn account(self) -> u16 {
        self.account
    }

    /// Protected common scalar receiving exact positive K.
    pub const fn count_destination(self) -> u16 {
        self.count_destination
    }

    /// First common scalar in the artifact-owned row bank.
    pub const fn rows_destination(self) -> u16 {
        self.rows_destination
    }

    /// Exact byte offset at which Product-N `u64` coefficients begin.
    pub const fn tail_offset(self) -> u32 {
        self.tail_offset
    }

    /// Exact common-scalar stride between row starts.
    pub const fn row_scalar_stride(self) -> u16 {
        self.row_scalar_stride
    }
}

impl NonzeroU64TailCountProjectionV2 {
    /// Fixed adapter-authenticated descriptor account.
    pub const fn account(self) -> u16 {
        self.account
    }

    /// Common scalar receiving the positive nonzero-row count.
    pub const fn destination(self) -> u16 {
        self.destination
    }

    /// Exact byte offset at which the `tail_count` `u64` rows begin.
    pub const fn tail_offset(self) -> u32 {
        self.tail_offset
    }
}

impl TailCountProjectionV2 {
    /// Fixed-prefix account containing the independently authenticated Product body.
    pub const fn account(self) -> u16 {
        self.account
    }

    /// Common scalar receiving the canonical `u32` outcome count.
    pub const fn register(self) -> u16 {
        self.register
    }

    /// Checked byte offset inside the authenticated Product body.
    pub const fn data_offset(self) -> u32 {
        self.data_offset
    }
}

impl AccountRuleV2 {
    /// Exact signer/writable/executable bits.
    pub const fn privileges(self) -> u8 {
        self.privileges
    }

    /// Route-local privilege subset declared for this logical coordinate.
    pub const fn route_privileges(self) -> RouteAccountPrivilegesV2 {
        RouteAccountPrivilegesV2 {
            bits: self.privileges,
        }
    }

    /// Exact debit/credit/data-write permission bits.
    pub const fn effect_permissions(self) -> u8 {
        self.effect_permissions
    }

    /// Canonical alias space.
    pub const fn alias_kind(self) -> AliasKindV2 {
        self.alias_kind
    }

    /// Fixed or same-item alias coordinate.
    pub const fn alias_index(self) -> u16 {
        self.alias_index
    }

    /// Exact or lifecycle-bound account-data prestate.
    pub const fn prestate(self) -> AccountPrestateV2 {
        self.prestate
    }

    /// Exact account-data width.
    pub const fn data_length(self) -> u32 {
        self.data_length
    }

    /// Additional exact account-data bytes per authenticated runtime item.
    pub const fn data_item_stride(self) -> u32 {
        self.data_item_stride
    }

    fn permission(self) -> AccountPermission {
        AccountPermission::new(
            self.effect_permissions & EFFECT_PERMISSION_DEBIT_LAMPORTS != 0,
            self.effect_permissions & EFFECT_PERMISSION_CREDIT_LAMPORTS != 0,
            self.effect_permissions & EFFECT_PERMISSION_WRITE_DATA != 0,
        )
    }
}

/// Hostile-decoded borrowed runtime-tail profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountProfileV2<'a> {
    artifact_profile: u16,
    fixed_accounts: u16,
    item_account_stride: u16,
    fixed_operations: u16,
    item_operations: u16,
    common_scalars: u16,
    item_scalar_stride: u16,
    common_identities: u16,
    item_identity_stride: u16,
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    dynamic_fixed_span_count: u16,
    fixed_data_predicate_count: u16,
    bytes: &'a [u8],
}

impl<'a> AccountProfileV2<'a> {
    /// Hostile-decode and prevalidate one complete V2 profile.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..8) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        let artifact_profile = read_u16(bytes, 10)?;
        if read_u16(bytes, 8)? != VERSION
            || !matches!(
                artifact_profile,
                ARTIFACT_PROFILE
                    | SELECTED_WINDOW_ARTIFACT_PROFILE
                    | TYPED_SCALAR_ARTIFACT_PROFILE
                    | TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
                    | LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                    | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
                    | TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
                    | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                    | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                    | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                    | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
                    | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                    | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
            )
        {
            return Err(Error::UnsupportedProfile);
        }
        if !matches!(
            artifact_profile,
            TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
                | LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
                | TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
                | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
                | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
        ) && read_u32(bytes, 28)? != 0
        {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
            artifact_profile,
            fixed_accounts: read_u16(bytes, 12)?,
            item_account_stride: read_u16(bytes, 14)?,
            fixed_operations: read_u16(bytes, 16)?,
            item_operations: read_u16(bytes, 18)?,
            common_scalars: read_u16(bytes, 20)?,
            item_scalar_stride: read_u16(bytes, 22)?,
            common_identities: read_u16(bytes, 24)?,
            item_identity_stride: read_u16(bytes, 26)?,
            trusted_environment: decode_trusted_environment(bytes, artifact_profile)?,
            trusted_identity_environment: decode_trusted_identity_environment(
                bytes,
                artifact_profile,
            )?,
            trusted_builtin_identity: decode_trusted_builtin_identity(bytes, artifact_profile)?,
            dynamic_fixed_span_count: decode_dynamic_fixed_span_count(bytes, artifact_profile)?,
            fixed_data_predicate_count: profile14::decode_predicate_count(bytes, artifact_profile)?,
            bytes,
        };
        if value.fixed_accounts == 0
            || (value.common_scalars == 0
                && value.item_scalar_stride == 0
                && value.common_identities == 0
                && value.item_identity_stride == 0)
            || (value.item_account_stride != 0
                && value.item_scalar_stride == 0
                && !value.uses_dynamic_fixed_spans())
        {
            return Err(Error::EmptyProfile);
        }
        if value
            .trusted_environment
            .current_slot_destination()
            .is_some_and(|destination| destination >= value.common_scalars)
        {
            return Err(Error::InvalidTrustedEnvironment);
        }
        if value
            .trusted_identity_environment
            .current_executing_program_destination()
            .is_some_and(|destination| destination >= value.common_identities)
        {
            return Err(Error::InvalidTrustedExecutingProgram);
        }
        if value
            .trusted_builtin_identity
            .system_program_destination()
            .is_some_and(|destination| destination >= value.common_identities)
            || value
                .trusted_builtin_identity
                .system_program_destination()
                .is_some_and(|destination| {
                    value
                        .trusted_identity_environment
                        .current_executing_program_destination()
                        == Some(destination)
                })
        {
            return Err(Error::InvalidTrustedBuiltin);
        }
        value.validate_dynamic_fixed_spans()?;
        let rules = usize::from(value.fixed_accounts)
            .checked_add(usize::from(value.item_account_stride))
            .ok_or(Error::InvalidLength)?;
        let operations = usize::from(value.fixed_operations)
            .checked_add(usize::from(value.item_operations))
            .ok_or(Error::InvalidLength)?;
        let expected = rules
            .checked_mul(RULE_BYTES)
            .and_then(|width| {
                operations
                    .checked_mul(OPERATION_BYTES)
                    .and_then(|ops| width.checked_add(ops))
            })
            .and_then(|body| value.header_bytes().checked_add(body))
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        value.validate_rules()?;
        value.validate_operations()?;
        value.validate_fixed_data_predicates()?;
        value.validate_tail_count_projection()?;
        Ok(value)
    }

    /// Fixed-prefix account count.
    pub const fn fixed_account_count(self) -> u16 {
        self.fixed_accounts
    }

    /// Selected physical profile discriminator.
    pub const fn artifact_profile(self) -> u16 {
        self.artifact_profile
    }

    /// Whether the selected profile owns descriptor-authenticated dynamic fixed spans.
    pub const fn uses_dynamic_fixed_spans(self) -> bool {
        matches!(
            self.artifact_profile,
            DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
        )
    }

    /// Whether the selected profile supports authenticated physical route-alias packing.
    pub const fn supports_route_alias_packing(self) -> bool {
        matches!(
            self.artifact_profile,
            AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
        )
    }

    /// Whether the selected profile owns fixed-data prestate predicates.
    pub const fn uses_fixed_data_predicates(self) -> bool {
        self.artifact_profile == FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
    }

    /// Accounts repeated per authenticated item.
    pub const fn item_account_stride(self) -> u16 {
        self.item_account_stride
    }

    /// Common scalar-bank width.
    pub const fn common_scalar_count(self) -> u16 {
        self.common_scalars
    }

    /// Per-item scalar-bank stride, whose slot zero is canonical item index.
    pub const fn item_scalar_stride(self) -> u16 {
        self.item_scalar_stride
    }

    /// Common identity-bank width.
    pub const fn common_identity_count(self) -> u16 {
        self.common_identities
    }

    /// Per-item identity-bank stride.
    pub const fn item_identity_stride(self) -> u16 {
        self.item_identity_stride
    }

    /// Trusted runtime observation and its common-scalar destination.
    pub const fn trusted_environment(self) -> TrustedEnvironmentV2 {
        self.trusted_environment
    }

    /// Common scalar that the outer must seed from its trusted current slot.
    pub const fn trusted_current_slot_scalar(self) -> Option<u16> {
        self.trusted_environment.current_slot_destination()
    }

    /// Trusted current-role declaration and its common-identity destination.
    pub const fn trusted_identity_environment(self) -> TrustedIdentityEnvironmentV2 {
        self.trusted_identity_environment
    }

    /// Common identity that the outer must seed with the authenticated program ID.
    pub const fn trusted_current_executing_program_identity(self) -> Option<u16> {
        self.trusted_identity_environment
            .current_executing_program_destination()
    }

    /// Trusted builtin identity declaration.
    pub const fn trusted_builtin_identity(self) -> TrustedBuiltinIdentityV2 {
        self.trusted_builtin_identity
    }

    /// Common identity that the outer must seed with the trusted System Program.
    pub const fn trusted_system_program_identity(self) -> Option<u16> {
        self.trusted_builtin_identity.system_program_destination()
    }

    /// Descriptor-owned dynamic account span, when selected by profile 13.
    pub const fn dynamic_fixed_span_count(self) -> u16 {
        self.dynamic_fixed_span_count
    }

    /// Decode one canonical dynamic-span table entry.
    pub fn dynamic_fixed_span(self, index: u16) -> Result<DynamicFixedSpanV2> {
        if !self.uses_dynamic_fixed_spans() || index >= self.dynamic_fixed_span_count {
            return Err(Error::InvalidDynamicSpan);
        }
        let offset = usize::from(index)
            .checked_mul(DYNAMIC_FIXED_SPAN_ENTRY_BYTES)
            .and_then(|body| DYNAMIC_FIXED_SPAN_HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        Ok(DynamicFixedSpanV2 {
            insertion_coordinate: read_u16(
                self.bytes,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_INSERTION_OFFSET)?,
            )?,
            count_scalar: read_u16(
                self.bytes,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_COUNT_SCALAR_OFFSET)?,
            )?,
            rule_start: read_u16(
                self.bytes,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_RULE_START_OFFSET)?,
            )?,
            rule_stride: read_u16(
                self.bytes,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_RULE_STRIDE_OFFSET)?,
            )?,
            minimum: read_u32(
                self.bytes,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_MIN_OFFSET)?,
            )?,
            maximum: read_u32(
                self.bytes,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_MAX_OFFSET)?,
            )?,
            step: read_u32(
                self.bytes,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_STEP_OFFSET)?,
            )?,
        })
    }

    /// Project every selected span width from its declared common scalar.
    ///
    /// The caller-owned output changes only after every scalar, range,
    /// congruence, and rule-template divisibility check succeeds.
    pub fn dynamic_span_widths_from_scalars(
        self,
        scalars: &[u64],
        output: &mut [u32],
    ) -> Result<()> {
        if output.len() != usize::from(self.dynamic_fixed_span_count) {
            return Err(Error::WidthMismatch);
        }
        let mut index = 0_u16;
        while index < self.dynamic_fixed_span_count {
            self.dynamic_fixed_span(index)?
                .count_from_scalars(scalars)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        index = 0;
        while index < self.dynamic_fixed_span_count {
            let width = self
                .dynamic_fixed_span(index)?
                .count_from_scalars(scalars)?;
            *output
                .get_mut(usize::from(index))
                .ok_or(Error::WidthMismatch)? = width;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }

    fn validate_dynamic_fixed_spans(self) -> Result<()> {
        if !self.uses_dynamic_fixed_spans() {
            return if self.dynamic_fixed_span_count == 0 {
                Ok(())
            } else {
                Err(Error::InvalidDynamicSpan)
            };
        }
        if self.dynamic_fixed_span_count == 0 {
            return if self.item_account_stride == 0 && self.item_operations == 0 {
                Ok(())
            } else {
                Err(Error::InvalidDynamicSpan)
            };
        }
        if self.item_account_stride == 0 || self.item_operations != 0 {
            return Err(Error::InvalidDynamicSpan);
        }
        let mut prior_insertion = None;
        let mut expected_rule_start = 0_u16;
        let mut index = 0_u16;
        while index < self.dynamic_fixed_span_count {
            let span = self.dynamic_fixed_span(index)?;
            let rule_end = span
                .rule_start
                .checked_add(span.rule_stride)
                .ok_or(Error::InvalidDynamicSpan)?;
            if span.insertion_coordinate > self.fixed_accounts
                || prior_insertion.is_some_and(|prior| prior > span.insertion_coordinate)
                || span.count_scalar >= self.common_scalars
                || span.rule_start != expected_rule_start
                || span.rule_stride == 0
                || rule_end > self.item_account_stride
                || span.minimum > span.maximum
                || span.step == 0
            {
                return Err(Error::InvalidDynamicSpan);
            }
            let mut prior = 0_u16;
            while prior < index {
                if self.dynamic_fixed_span(prior)?.count_scalar == span.count_scalar {
                    return Err(Error::InvalidDynamicSpan);
                }
                prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            prior_insertion = Some(span.insertion_coordinate);
            expected_rule_start = rule_end;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        if expected_rule_start != self.item_account_stride {
            return Err(Error::InvalidDynamicSpan);
        }
        Ok(())
    }

    fn dynamic_span_for_rule_template(self, rule_index: u16) -> Result<DynamicFixedSpanV2> {
        let mut index = 0_u16;
        while index < self.dynamic_fixed_span_count {
            let span = self.dynamic_fixed_span(index)?;
            let end = span
                .rule_start
                .checked_add(span.rule_stride)
                .ok_or(Error::InvalidDynamicSpan)?;
            if rule_index >= span.rule_start && rule_index < end {
                return Ok(span);
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Err(Error::InvalidDynamicSpan)
    }

    /// Whether any account projection or trusted-environment seed writes `target`.
    ///
    /// This is a static artifact inspection. It does not execute projections or
    /// infer physical aliases.
    pub fn writes_register(self, target: ProjectionTargetV2) -> Result<bool> {
        if target.kind == ProjectionRegisterKindV2::Scalar
            && target.space == ProjectionRegisterSpaceV2::Common
            && self.trusted_current_slot_scalar() == Some(target.index)
        {
            return Ok(true);
        }
        if target.kind == ProjectionRegisterKindV2::Identity
            && target.space == ProjectionRegisterSpaceV2::Common
            && self.trusted_current_executing_program_identity() == Some(target.index)
        {
            return Ok(true);
        }
        if target.kind == ProjectionRegisterKindV2::Identity
            && target.space == ProjectionRegisterSpaceV2::Common
            && self.trusted_system_program_identity() == Some(target.index)
        {
            return Ok(true);
        }
        let expected = (
            target.kind == ProjectionRegisterKindV2::Identity,
            target.space == ProjectionRegisterSpaceV2::Item,
            target.index,
        );
        let mut fixed = 0_u16;
        while fixed < self.fixed_operations {
            if self
                .operation(false, fixed)?
                .writes_target(self, expected)?
            {
                return Ok(true);
            }
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_operations {
            if self.operation(true, item)?.writes_target(self, expected)? {
                return Ok(true);
            }
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(false)
    }

    /// Borrow complete canonical profile bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    fn header_bytes(self) -> usize {
        if self.uses_fixed_data_predicates() {
            return FIXED_DATA_PREDICATE_HEADER_BYTES
                + usize::from(self.dynamic_fixed_span_count) * DYNAMIC_FIXED_SPAN_ENTRY_BYTES
                + usize::from(self.fixed_data_predicate_count) * FIXED_DATA_PREDICATE_BYTES;
        }
        if matches!(
            self.artifact_profile,
            TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
                | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
                | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        ) {
            if self.uses_dynamic_fixed_spans() {
                DYNAMIC_FIXED_SPAN_HEADER_BYTES
                    + usize::from(self.dynamic_fixed_span_count) * DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            } else if self.artifact_profile == AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE {
                AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES
            } else {
                TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES
            }
        } else {
            HEADER_BYTES
        }
    }

    /// Return one fixed rule or one repeated item-template rule.
    pub fn rule(self, item_template: bool, index: u16) -> Result<AccountRuleV2> {
        let count = if item_template {
            self.item_account_stride
        } else {
            self.fixed_accounts
        };
        if index >= count {
            return Err(Error::InvalidCoordinate);
        }
        let ordinal = if item_template {
            usize::from(self.fixed_accounts)
                .checked_add(usize::from(index))
                .ok_or(Error::InvalidLength)?
        } else {
            usize::from(index)
        };
        let offset = ordinal
            .checked_mul(RULE_BYTES)
            .and_then(|body| self.header_bytes().checked_add(body))
            .ok_or(Error::InvalidLength)?;
        decode_rule(self.bytes, offset, self.artifact_profile)
    }

    /// Resolve one expanded physical account to its representative coordinate.
    pub fn representative(self, tail_count: u32, coordinate: usize) -> Result<usize> {
        if self.dynamic_fixed_span_count != 0 {
            return Err(Error::InvalidDynamicSpan);
        }
        let width = account_width(self, tail_count)?;
        if coordinate >= width {
            return Err(Error::InvalidCoordinate);
        }
        let fixed = usize::from(self.fixed_accounts);
        if coordinate < fixed {
            let index = u16::try_from(coordinate).map_err(|_| Error::InvalidCoordinate)?;
            return representative_from_rule(self.rule(false, index)?, coordinate, 0);
        }
        let stride = usize::from(self.item_account_stride);
        if stride == 0 {
            return Err(Error::InvalidCoordinate);
        }
        let tail_offset = coordinate
            .checked_sub(fixed)
            .ok_or(Error::InvalidCoordinate)?;
        let item = tail_offset / stride;
        let local = tail_offset % stride;
        let item_start = fixed
            .checked_add(item.checked_mul(stride).ok_or(Error::InvalidCoordinate)?)
            .ok_or(Error::InvalidCoordinate)?;
        representative_from_rule(
            self.rule(
                true,
                u16::try_from(local).map_err(|_| Error::InvalidCoordinate)?,
            )?,
            coordinate,
            item_start,
        )
    }

    /// Number of logical account coordinates after checked affine expansion.
    pub fn logical_account_count(self, tail_count: u32) -> Result<usize> {
        account_width(self, tail_count)
    }

    /// Number of logical coordinates after expanding every canonical profile-13 span.
    pub fn logical_account_count_with_dynamic_spans(
        self,
        tail_count: u32,
        span_counts: &[u32],
    ) -> Result<usize> {
        dynamic_account_width(self, tail_count, span_counts)
    }

    /// Resolve one profile-13 logical coordinate to its physical representative.
    pub fn representative_with_dynamic_spans(
        self,
        tail_count: u32,
        span_counts: &[u32],
        coordinate: usize,
    ) -> Result<usize> {
        dynamic_representative(self, tail_count, span_counts, coordinate)
    }

    /// Number of unique physical representatives in canonical logical order.
    ///
    /// Profile 11 permits the outer to supply one `AccountInfo` per returned
    /// representative. Earlier profiles retain their existing one-observation-
    /// per-logical-coordinate contract.
    pub fn physical_account_count(self, tail_count: u32) -> Result<usize> {
        let logical = account_width(self, tail_count)?;
        if !self.supports_route_alias_packing() {
            return Ok(logical);
        }
        let mut count = 0_usize;
        let mut coordinate = 0_usize;
        while coordinate < logical {
            if self.representative(tail_count, coordinate)? == coordinate {
                count = count.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(count)
    }

    /// Number of unique physical representatives for profile 13.
    pub fn physical_account_count_with_dynamic_spans(
        self,
        tail_count: u32,
        span_counts: &[u32],
    ) -> Result<usize> {
        let logical = dynamic_account_width(self, tail_count, span_counts)?;
        let mut count = 0_usize;
        let mut coordinate = 0_usize;
        while coordinate < logical {
            if self.representative_with_dynamic_spans(tail_count, span_counts, coordinate)?
                == coordinate
            {
                count = count.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(count)
    }

    /// Canonical physical representative ordinal for one logical coordinate.
    pub fn physical_account_ordinal(
        self,
        tail_count: u32,
        logical_coordinate: usize,
    ) -> Result<usize> {
        let representative = self.representative(tail_count, logical_coordinate)?;
        if !self.supports_route_alias_packing() {
            return Ok(logical_coordinate);
        }
        let mut ordinal = 0_usize;
        let mut coordinate = 0_usize;
        while coordinate < representative {
            if self.representative(tail_count, coordinate)? == coordinate {
                ordinal = ordinal.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(ordinal)
    }

    /// Canonical physical ordinal for one expanded profile-13 coordinate.
    pub fn physical_account_ordinal_with_dynamic_spans(
        self,
        tail_count: u32,
        span_counts: &[u32],
        logical_coordinate: usize,
    ) -> Result<usize> {
        let representative =
            self.representative_with_dynamic_spans(tail_count, span_counts, logical_coordinate)?;
        let mut ordinal = 0_usize;
        let mut coordinate = 0_usize;
        while coordinate < representative {
            if self.representative_with_dynamic_spans(tail_count, span_counts, coordinate)?
                == coordinate
            {
                ordinal = ordinal.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(ordinal)
    }

    /// Logical self-representative coordinate for one physical ordinal.
    pub fn physical_representative_coordinate(
        self,
        tail_count: u32,
        physical_ordinal: usize,
    ) -> Result<usize> {
        if physical_ordinal >= self.physical_account_count(tail_count)? {
            return Err(Error::InvalidCoordinate);
        }
        if self.artifact_profile != AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE {
            return Ok(physical_ordinal);
        }
        let logical = account_width(self, tail_count)?;
        let mut ordinal = 0_usize;
        let mut coordinate = 0_usize;
        while coordinate < logical {
            if self.representative(tail_count, coordinate)? == coordinate {
                if ordinal == physical_ordinal {
                    return Ok(coordinate);
                }
                ordinal = ordinal.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Err(Error::InvalidCoordinate)
    }

    /// Logical self-representative coordinate for one profile-13 physical ordinal.
    pub fn physical_representative_coordinate_with_dynamic_spans(
        self,
        tail_count: u32,
        span_counts: &[u32],
        physical_ordinal: usize,
    ) -> Result<usize> {
        if physical_ordinal
            >= self.physical_account_count_with_dynamic_spans(tail_count, span_counts)?
        {
            return Err(Error::InvalidCoordinate);
        }
        let logical = dynamic_account_width(self, tail_count, span_counts)?;
        let mut ordinal = 0_usize;
        let mut coordinate = 0_usize;
        while coordinate < logical {
            if self.representative_with_dynamic_spans(tail_count, span_counts, coordinate)?
                == coordinate
            {
                if ordinal == physical_ordinal {
                    return Ok(coordinate);
                }
                ordinal = ordinal.checked_add(1).ok_or(Error::InvalidLength)?;
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Err(Error::InvalidCoordinate)
    }

    /// Kernel-owned packed-account geometry for one physical ordinal.
    pub fn physical_account_geometry(
        self,
        tail_count: u32,
        physical_ordinal: usize,
    ) -> Result<PhysicalAccountGeometryV2> {
        let representative =
            self.physical_representative_coordinate(tail_count, physical_ordinal)?;
        let rule = expanded_rule(self, representative)?;
        Ok(PhysicalAccountGeometryV2 {
            logical_representative: representative,
            privileges: RouteAccountPrivilegesV2 {
                bits: representative_privileges(self, tail_count, representative)?,
            },
            rule,
            data: physical_data_geometry(rule, tail_count)?,
        })
    }

    /// Kernel-owned packed-account geometry for one expanded profile-13 ordinal.
    pub fn physical_account_geometry_with_dynamic_spans(
        self,
        tail_count: u32,
        span_counts: &[u32],
        physical_ordinal: usize,
    ) -> Result<PhysicalAccountGeometryV2> {
        let representative = self.physical_representative_coordinate_with_dynamic_spans(
            tail_count,
            span_counts,
            physical_ordinal,
        )?;
        let rule = expanded_rule_with_dynamic_spans(self, tail_count, span_counts, representative)?;
        Ok(PhysicalAccountGeometryV2 {
            logical_representative: representative,
            privileges: RouteAccountPrivilegesV2 {
                bits: representative_privileges_with_dynamic_spans(
                    self,
                    tail_count,
                    span_counts,
                    representative,
                )?,
            },
            rule,
            data: physical_data_geometry(rule, tail_count)?,
        })
    }

    /// Route-local privilege subset for one expanded logical coordinate.
    pub fn route_privileges(
        self,
        tail_count: u32,
        logical_coordinate: usize,
    ) -> Result<RouteAccountPrivilegesV2> {
        if logical_coordinate >= account_width(self, tail_count)? {
            return Err(Error::InvalidCoordinate);
        }
        Ok(expanded_rule(self, logical_coordinate)?.route_privileges())
    }

    /// Route-local privilege subset for one profile-13 logical coordinate.
    pub fn route_privileges_with_dynamic_spans(
        self,
        tail_count: u32,
        span_counts: &[u32],
        logical_coordinate: usize,
    ) -> Result<RouteAccountPrivilegesV2> {
        Ok(
            expanded_rule_with_dynamic_spans(self, tail_count, span_counts, logical_coordinate)?
                .route_privileges(),
        )
    }

    /// Return the unique fixed-prefix Product tail-count projection, if affine.
    pub fn tail_count_projection(self) -> Result<Option<TailCountProjectionV2>> {
        let mut found = None;
        let mut index = 0_u16;
        while index < self.fixed_operations {
            let operation = self.operation(false, index)?;
            if operation.opcode == OP_PROJECT_TAIL_COUNT_U32 {
                if found.is_some() {
                    return Err(Error::DuplicateProjection);
                }
                found = Some(TailCountProjectionV2 {
                    account: operation.account,
                    register: operation.register,
                    data_offset: operation.data_offset,
                });
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(found)
    }

    /// Return the sole authenticated nonzero-`u64` tail projection selected
    /// by profile 10, if present.
    pub fn nonzero_u64_tail_count_projection(
        self,
    ) -> Result<Option<NonzeroU64TailCountProjectionV2>> {
        let mut found = None;
        let mut index = 0_u16;
        while index < self.fixed_operations {
            let operation = self.operation(false, index)?;
            if operation.opcode == OP_PROJECT_NONZERO_U64_TAIL_COUNT {
                if found.is_some() {
                    return Err(Error::DuplicateProjection);
                }
                found = Some(NonzeroU64TailCountProjectionV2 {
                    account: operation.account,
                    destination: operation.register,
                    tail_offset: operation.data_offset,
                });
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(found)
    }

    /// Return the sole authenticated ordered nonzero-`u64` row projection
    /// selected by profile 12, if present.
    pub fn nonzero_u64_tail_rows_projection(
        self,
    ) -> Result<Option<NonzeroU64TailRowsProjectionV2>> {
        let mut found = None;
        let mut index = 0_u16;
        while index < self.fixed_operations {
            let operation = self.operation(false, index)?;
            if operation.opcode == OP_PROJECT_NONZERO_U64_TAIL_ROWS {
                if found.is_some() {
                    return Err(Error::DuplicateProjection);
                }
                found = Some(NonzeroU64TailRowsProjectionV2 {
                    account: operation.account,
                    count_destination: operation.support_count_destination()?,
                    rows_destination: operation.register,
                    tail_offset: operation.data_offset,
                    row_scalar_stride: operation.support_row_scalar_stride()?,
                });
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(found)
    }

    fn operation(self, item_template: bool, index: u16) -> Result<Operation> {
        let count = if item_template {
            self.item_operations
        } else {
            self.fixed_operations
        };
        if index >= count {
            return Err(Error::InvalidCoordinate);
        }
        let rule_count = usize::from(self.fixed_accounts)
            .checked_add(usize::from(self.item_account_stride))
            .ok_or(Error::InvalidLength)?;
        let ordinal = if item_template {
            usize::from(self.fixed_operations)
                .checked_add(usize::from(index))
                .ok_or(Error::InvalidLength)?
        } else {
            usize::from(index)
        };
        let offset = rule_count
            .checked_mul(RULE_BYTES)
            .and_then(|rules| {
                ordinal
                    .checked_mul(OPERATION_BYTES)
                    .and_then(|ops| rules.checked_add(ops))
            })
            .and_then(|body| self.header_bytes().checked_add(body))
            .ok_or(Error::InvalidLength)?;
        Operation::decode(self.bytes, offset)
    }

    fn validate_rules(self) -> Result<()> {
        let mut lifecycle_bound = false;
        let mut adapter_authenticated_variable_data = false;
        let mut adapter_authenticated_variable_data_alias = false;
        let mut authenticated_route_alias = false;
        let mut fixed = 0_u16;
        while fixed < self.fixed_accounts {
            let rule = self.rule(false, fixed)?;
            if self.artifact_profile == ARTIFACT_PROFILE && rule.data_item_stride != 0 {
                return Err(Error::NonCanonicalReserved);
            }
            validate_rule(
                rule,
                false,
                fixed,
                self.fixed_accounts,
                self.artifact_profile,
            )?;
            if matches!(
                self.artifact_profile,
                AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                    | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                    | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
            ) && rule.alias_kind != AliasKindV2::SelfCoordinate
                && !matches!(
                    rule.prestate,
                    AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
                        | AccountPrestateV2::AuthenticatedRouteAlias
                )
            {
                return Err(Error::InvalidRouteAlias);
            }
            self.require_owner_anchor(false, fixed, rule)?;
            lifecycle_bound |= rule.prestate == AccountPrestateV2::LifecycleBound;
            adapter_authenticated_variable_data |=
                rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableData;
            if rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableDataAlias {
                let representative = self.rule(false, rule.alias_index)?;
                if representative.prestate != AccountPrestateV2::AdapterAuthenticatedVariableData
                    || representative.alias_kind != AliasKindV2::SelfCoordinate
                    || representative.alias_index != 0
                {
                    return Err(Error::InvalidVariableDataPrestate);
                }
                adapter_authenticated_variable_data_alias = true;
            }
            if rule.prestate == AccountPrestateV2::AuthenticatedRouteAlias {
                let representative = self.rule(false, rule.alias_index)?;
                if representative.alias_kind != AliasKindV2::SelfCoordinate
                    || representative.alias_index != 0
                    || rule.privileges & !representative.privileges != 0
                {
                    return Err(Error::InvalidRouteAlias);
                }
                authenticated_route_alias = true;
            }
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_account_stride {
            let rule = self.rule(true, item)?;
            if self.artifact_profile == ARTIFACT_PROFILE && rule.data_item_stride != 0 {
                return Err(Error::NonCanonicalReserved);
            }
            validate_rule(rule, true, item, self.fixed_accounts, self.artifact_profile)?;
            if self.uses_dynamic_fixed_spans() {
                let span = self.dynamic_span_for_rule_template(item)?;
                if rule.alias_kind == AliasKindV2::Fixed
                    && rule.alias_index >= span.insertion_coordinate
                {
                    return Err(Error::InvalidAlias);
                }
            }
            if matches!(
                self.artifact_profile,
                AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                    | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                    | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
            ) && rule.alias_kind != AliasKindV2::SelfCoordinate
            {
                return Err(Error::InvalidRouteAlias);
            }
            self.require_owner_anchor(true, item, rule)?;
            lifecycle_bound |= rule.prestate == AccountPrestateV2::LifecycleBound;
            adapter_authenticated_variable_data |=
                rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableData;
            adapter_authenticated_variable_data_alias |=
                rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableDataAlias;
            authenticated_route_alias |=
                rule.prestate == AccountPrestateV2::AuthenticatedRouteAlias;
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        match self.artifact_profile {
            LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                if lifecycle_bound && !adapter_authenticated_variable_data =>
            {
                Ok(())
            }
            ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
                if adapter_authenticated_variable_data =>
            {
                Ok(())
            }
            TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE => Ok(()),
            ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                if adapter_authenticated_variable_data
                    && adapter_authenticated_variable_data_alias =>
            {
                Ok(())
            }
            NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE if adapter_authenticated_variable_data => {
                Ok(())
            }
            NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE if adapter_authenticated_variable_data => Ok(()),
            AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE if authenticated_route_alias => Ok(()),
            DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE => Ok(()),
            FIXED_DATA_PREDICATE_ARTIFACT_PROFILE => Ok(()),
            LIFECYCLE_PRESTATE_ARTIFACT_PROFILE => Err(Error::InvalidLifecyclePrestate),
            ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE => {
                Err(Error::InvalidVariableDataPrestate)
            }
            ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE => {
                Err(Error::InvalidVariableDataPrestate)
            }
            NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE => Err(Error::InvalidVariableDataPrestate),
            NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE => Err(Error::InvalidVariableDataPrestate),
            AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE => Err(Error::InvalidRouteAlias),
            _ if !lifecycle_bound
                && !adapter_authenticated_variable_data
                && !adapter_authenticated_variable_data_alias
                && !authenticated_route_alias =>
            {
                Ok(())
            }
            _ if lifecycle_bound => Err(Error::InvalidLifecyclePrestate),
            _ => Err(Error::InvalidVariableDataPrestate),
        }
    }

    fn require_owner_anchor(self, item: bool, account: u16, rule: AccountRuleV2) -> Result<()> {
        if rule.effect_permissions
            & (EFFECT_PERMISSION_DEBIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA)
            == 0
        {
            return Ok(());
        }
        if rule.prestate == AccountPrestateV2::LifecycleBound {
            return Ok(());
        }
        let count = if item {
            self.item_operations
        } else {
            self.fixed_operations
        };
        let mut index = 0_u16;
        while index < count {
            let operation = self.operation(item, index)?;
            if operation.opcode == OP_REQUIRE_OWNER
                && operation.account_item == item
                && operation.account == account
            {
                return Ok(());
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Err(Error::EffectOwnerUnanchored)
    }

    fn validate_operations(self) -> Result<()> {
        let mut fixed = 0_u16;
        while fixed < self.fixed_operations {
            let operation = self.operation(false, fixed)?;
            operation.validate(self, false, fixed)?;
            self.require_dynamic_span_count_not_overwritten(operation)?;
            self.validate_lifecycle_operation(operation)?;
            self.require_unique_projection(false, fixed, operation)?;
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_operations {
            let operation = self.operation(true, item)?;
            operation.validate(self, true, item)?;
            self.require_dynamic_span_count_not_overwritten(operation)?;
            self.validate_lifecycle_operation(operation)?;
            self.require_unique_projection(true, item, operation)?;
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        if self.artifact_profile == NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
            && self.nonzero_u64_tail_count_projection()?.is_none()
        {
            return Err(Error::NonCanonicalOperation);
        }
        if self.artifact_profile == NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
            && self.nonzero_u64_tail_rows_projection()?.is_none()
        {
            return Err(Error::NonCanonicalOperation);
        }
        Ok(())
    }

    fn require_dynamic_span_count_not_overwritten(self, operation: Operation) -> Result<()> {
        let mut index = 0_u16;
        while index < self.dynamic_fixed_span_count {
            let destination = self.dynamic_fixed_span(index)?.count_scalar;
            if operation.writes_target(self, (false, false, destination))? {
                return Err(Error::InvalidDynamicSpan);
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }

    fn validate_lifecycle_operation(self, operation: Operation) -> Result<()> {
        let rule = self.rule(operation.account_item, operation.account)?;
        if rule.prestate != AccountPrestateV2::LifecycleBound {
            return Ok(());
        }
        if matches!(
            operation.opcode,
            OP_REQUIRE_OWNER | OP_PROJECT_OWNER | OP_PROJECT_LAMPORTS | OP_SELECT_DATA_WINDOW
        ) || operation.is_selected_projection()
            || operation.opcode == OP_PROJECT_TAIL_COUNT_U32
        {
            return Err(Error::InvalidLifecyclePrestate);
        }
        Ok(())
    }

    fn selected_window(self, account: u16) -> Result<(u16, Operation)> {
        let mut found = None;
        let mut index = 0_u16;
        while index < self.fixed_operations {
            let operation = self.operation(false, index)?;
            if operation.opcode == OP_SELECT_DATA_WINDOW && operation.account == account {
                if found.is_some() {
                    return Err(Error::NonCanonicalOperation);
                }
                found = Some((index, operation));
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        found.ok_or(Error::NonCanonicalOperation)
    }

    fn selected_item_stride(self, account: u16) -> Result<u32> {
        let mut found = None;
        let mut index = 0_u16;
        while index < self.item_operations {
            let operation = self.operation(true, index)?;
            if operation.is_selected_affine_projection() && operation.account == account {
                if found.is_some_and(|stride| stride != operation.data_stride) {
                    return Err(Error::NonCanonicalOperation);
                }
                found = Some(operation.data_stride);
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        found.ok_or(Error::NonCanonicalOperation)
    }

    fn fixed_scalar_projection_index(self, register: u16) -> Result<u16> {
        let mut index = 0_u16;
        while index < self.fixed_operations {
            if self.operation(false, index)?.projection_target()? == Some((false, false, register))
            {
                return Ok(index);
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Err(Error::NonCanonicalOperation)
    }

    fn validate_tail_count_projection(self) -> Result<()> {
        let affine = (self.item_account_stride != 0 && !self.uses_dynamic_fixed_spans())
            || self.item_operations != 0
            || self.item_scalar_stride != 0
            || self.item_identity_stride != 0
            || self.has_affine_data_length()?;
        let projection = self.tail_count_projection()?;
        if (affine
            || matches!(
                self.artifact_profile,
                NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
            ))
            && projection.is_none()
        {
            Err(Error::NonCanonicalOperation)
        } else {
            Ok(())
        }
    }

    fn has_affine_data_length(self) -> Result<bool> {
        let mut fixed = 0_u16;
        while fixed < self.fixed_accounts {
            if self.rule(false, fixed)?.data_item_stride != 0 {
                return Ok(true);
            }
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_account_stride {
            if self.rule(true, item)?.data_item_stride != 0 {
                return Ok(true);
            }
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(false)
    }

    fn require_unique_projection(self, item: bool, index: u16, operation: Operation) -> Result<()> {
        let target = operation.projection_target()?;
        let support_rows = operation.opcode == OP_PROJECT_NONZERO_U64_TAIL_ROWS;
        if target.is_none() && !support_rows {
            return Ok(());
        }
        if let Some(destination) = self.trusted_current_slot_scalar()
            && operation.writes_target(self, (false, false, destination))?
        {
            return Err(Error::TrustedEnvironmentOverwrite);
        }
        if let Some(destination) = self.trusted_current_executing_program_identity()
            && operation.writes_target(self, (true, false, destination))?
        {
            return Err(Error::TrustedExecutingProgramOverwrite);
        }
        if let Some(destination) = self.trusted_system_program_identity()
            && operation.writes_target(self, (true, false, destination))?
        {
            return Err(Error::TrustedBuiltinOverwrite);
        }
        if item && !operation.register_item {
            return Err(Error::DuplicateProjection);
        }
        let count = if item {
            self.item_operations
        } else {
            self.fixed_operations
        };
        let mut prior = 0_u16;
        while prior < index && prior < count {
            let prior_operation = self.operation(item, prior)?;
            if let Some(target) = target {
                if prior_operation.writes_target(self, target)? {
                    return Err(Error::DuplicateProjection);
                }
            } else {
                let mut register = 0_u16;
                while register < self.common_scalars {
                    let target = (false, false, register);
                    if operation.writes_target(self, target)?
                        && prior_operation.writes_target(self, target)?
                    {
                        return Err(Error::DuplicateProjection);
                    }
                    register = register.checked_add(1).ok_or(Error::InvalidLength)?;
                }
            }
            prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }
}

fn decode_trusted_environment(bytes: &[u8], artifact_profile: u16) -> Result<TrustedEnvironmentV2> {
    if !matches!(
        artifact_profile,
        TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
            | LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
            | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
            | TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
            | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
            | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
            | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
            | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
            | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
            | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
    ) {
        return Ok(TrustedEnvironmentV2::None);
    }
    let destination = read_u16(bytes, TRUSTED_ENVIRONMENT_SCALAR_OFFSET)?;
    let kind = byte(bytes, TRUSTED_ENVIRONMENT_KIND_OFFSET)?;
    if byte(bytes, TRUSTED_ENVIRONMENT_RESERVED_OFFSET)? != 0 {
        return Err(Error::InvalidTrustedEnvironment);
    }
    match kind {
        TRUSTED_ENVIRONMENT_NONE if destination == 0 => Ok(TrustedEnvironmentV2::None),
        TRUSTED_ENVIRONMENT_CURRENT_SLOT => Ok(TrustedEnvironmentV2::CurrentSlot { destination }),
        _ => Err(Error::InvalidTrustedEnvironment),
    }
}

fn decode_trusted_identity_environment(
    bytes: &[u8],
    artifact_profile: u16,
) -> Result<TrustedIdentityEnvironmentV2> {
    if !matches!(
        artifact_profile,
        TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
            | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
            | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
            | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
            | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
            | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
            | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
    ) {
        return Ok(TrustedIdentityEnvironmentV2::None);
    }
    let destination = read_u16(bytes, TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET)?;
    let kind = byte(bytes, TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET)?;
    if byte(bytes, TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET)? != 0 {
        return Err(Error::InvalidTrustedExecutingProgram);
    }
    match kind {
        TRUSTED_EXECUTING_PROGRAM_NONE if destination == 0 => {
            Ok(TrustedIdentityEnvironmentV2::None)
        }
        TRUSTED_EXECUTING_PROGRAM_CURRENT => {
            Ok(TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination })
        }
        _ => Err(Error::InvalidTrustedExecutingProgram),
    }
}

fn decode_trusted_builtin_identity(
    bytes: &[u8],
    artifact_profile: u16,
) -> Result<TrustedBuiltinIdentityV2> {
    if !matches!(
        artifact_profile,
        AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
            | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
            | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
    ) {
        return Ok(TrustedBuiltinIdentityV2::None);
    }
    let destination = read_u16(bytes, TRUSTED_BUILTIN_IDENTITY_OFFSET)?;
    let kind = byte(bytes, TRUSTED_BUILTIN_KIND_OFFSET)?;
    if byte(bytes, TRUSTED_BUILTIN_RESERVED_OFFSET)? != 0 {
        return Err(Error::InvalidTrustedBuiltin);
    }
    match kind {
        TRUSTED_BUILTIN_NONE if destination == 0 => Ok(TrustedBuiltinIdentityV2::None),
        TRUSTED_BUILTIN_SYSTEM_PROGRAM => {
            Ok(TrustedBuiltinIdentityV2::SystemProgram { destination })
        }
        _ => Err(Error::InvalidTrustedBuiltin),
    }
}

fn decode_dynamic_fixed_span_count(bytes: &[u8], artifact_profile: u16) -> Result<u16> {
    if !matches!(
        artifact_profile,
        DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
    ) {
        return Ok(0);
    }
    if artifact_profile == DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        && bytes
            .get(DYNAMIC_FIXED_SPAN_RESERVED_OFFSET..DYNAMIC_FIXED_SPAN_HEADER_BYTES)
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(Error::InvalidDynamicSpan);
    }
    read_u16(bytes, DYNAMIC_FIXED_SPAN_COUNT_OFFSET)
}

/// Caller-owned flat banks for failure-atomic V2 projection.
pub struct ProjectionRegistersV2<'a> {
    /// Immutable scalar input.
    pub input_scalars: &'a [u64],
    /// Immutable identity input.
    pub input_identities: &'a [[u8; 32]],
    /// Scratch scalars that may change on refusal.
    pub scratch_scalars: &'a mut [u64],
    /// Scratch identities that may change on refusal.
    pub scratch_identities: &'a mut [[u8; 32]],
    /// Candidate scalar output, changed only on success.
    pub output_scalars: &'a mut [u64],
    /// Candidate identity output, changed only on success.
    pub output_identities: &'a mut [[u8; 32]],
}

/// Validate expanded observations and atomically project one authenticated tail.
pub fn project_atomic(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    accounts: &[AccountObservationV1<'_>],
    registers: ProjectionRegistersV2<'_>,
) -> Result<()> {
    let account_count = account_width(profile, tail_count)?;
    let scalar_count = affine_width(
        profile.common_scalars,
        profile.item_scalar_stride,
        tail_count,
    )?;
    let identity_count = affine_width(
        profile.common_identities,
        profile.item_identity_stride,
        tail_count,
    )?;
    if accounts.len() != account_count
        || registers.input_scalars.len() != scalar_count
        || registers.scratch_scalars.len() != scalar_count
        || registers.output_scalars.len() != scalar_count
        || registers.input_identities.len() != identity_count
        || registers.scratch_identities.len() != identity_count
        || registers.output_identities.len() != identity_count
    {
        return Err(Error::WidthMismatch);
    }
    validate_accounts(profile, tail_count, accounts)?;
    profile14::validate_observations(profile, &[], accounts)?;
    registers
        .scratch_scalars
        .copy_from_slice(registers.input_scalars);
    registers
        .scratch_identities
        .copy_from_slice(registers.input_identities);
    inject_indices(profile, tail_count, registers.scratch_scalars)?;
    apply_operations(
        profile,
        tail_count,
        accounts,
        registers.input_identities,
        registers.scratch_scalars,
        registers.scratch_identities,
    )?;
    registers
        .output_scalars
        .copy_from_slice(registers.scratch_scalars);
    registers
        .output_identities
        .copy_from_slice(registers.scratch_identities);
    Ok(())
}

/// Validate and project profile 13 using an exact descriptor-ordered span-width bank.
///
/// Each width must already be present in its declared protected common scalar;
/// account-vector length is never accepted as width authority. Fixed operation
/// coordinates are shifted by the checked cumulative widths before projection.
pub fn project_dynamic_fixed_spans_atomic(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    accounts: &[AccountObservationV1<'_>],
    registers: ProjectionRegistersV2<'_>,
) -> Result<()> {
    require_dynamic_span_counts(profile, span_counts)?;
    let account_count = dynamic_account_width(profile, tail_count, span_counts)?;
    let scalar_count = affine_width(
        profile.common_scalars,
        profile.item_scalar_stride,
        tail_count,
    )?;
    let identity_count = affine_width(
        profile.common_identities,
        profile.item_identity_stride,
        tail_count,
    )?;
    if accounts.len() != account_count
        || registers.input_scalars.len() != scalar_count
        || registers.scratch_scalars.len() != scalar_count
        || registers.output_scalars.len() != scalar_count
        || registers.input_identities.len() != identity_count
        || registers.scratch_identities.len() != identity_count
        || registers.output_identities.len() != identity_count
    {
        return Err(Error::WidthMismatch);
    }
    let mut index = 0_u16;
    while index < profile.dynamic_fixed_span_count {
        let span = profile.dynamic_fixed_span(index)?;
        if span.count_from_scalars(registers.input_scalars)?
            != *span_counts
                .get(usize::from(index))
                .ok_or(Error::InvalidDynamicSpan)?
        {
            return Err(Error::InvalidDynamicSpan);
        }
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    validate_accounts_with_dynamic_spans(profile, tail_count, span_counts, accounts)?;
    profile14::validate_observations(profile, span_counts, accounts)?;
    registers
        .scratch_scalars
        .copy_from_slice(registers.input_scalars);
    registers
        .scratch_identities
        .copy_from_slice(registers.input_identities);
    inject_indices(profile, tail_count, registers.scratch_scalars)?;
    apply_operations_with_dynamic_spans(
        profile,
        tail_count,
        span_counts,
        accounts,
        registers.input_identities,
        registers.scratch_scalars,
        registers.scratch_identities,
    )?;
    registers
        .output_scalars
        .copy_from_slice(registers.scratch_scalars);
    registers
        .output_identities
        .copy_from_slice(registers.scratch_identities);
    Ok(())
}

/// Authenticate only the fixed prefix and return its unique Product-owned count.
///
/// The outer must independently authenticate the source account as the exact
/// finalized Product Runtime V2 graph selected by Core before using this
/// descriptor-level projection. This function does not make AccountProfile a
/// second Product decoder or semantic owner.
pub fn project_tail_count_atomic(
    profile: AccountProfileV2<'_>,
    fixed_accounts: &[AccountObservationV1<'_>],
    registers: ProjectionRegistersV2<'_>,
) -> Result<u32> {
    let projection = profile
        .tail_count_projection()?
        .ok_or(Error::NonCanonicalOperation)?;
    let ProjectionRegistersV2 {
        input_scalars,
        input_identities,
        scratch_scalars,
        scratch_identities,
        output_scalars,
        output_identities,
    } = registers;
    project_atomic(
        profile,
        0,
        fixed_accounts,
        ProjectionRegistersV2 {
            input_scalars,
            input_identities,
            scratch_scalars: &mut *scratch_scalars,
            scratch_identities: &mut *scratch_identities,
            output_scalars: &mut *output_scalars,
            output_identities: &mut *output_identities,
        },
    )?;
    u32::try_from(
        *output_scalars
            .get(usize::from(projection.register))
            .ok_or(Error::InvalidCoordinate)?,
    )
    .map_err(|_| Error::InvalidCoordinate)
}

/// Expand exact effect permissions for one authenticated tail.
pub fn derive_effect_permissions(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    output: &mut [AccountPermission],
) -> Result<()> {
    if output.len() != account_width(profile, tail_count)? {
        return Err(Error::WidthMismatch);
    }
    for (coordinate, permission) in output.iter_mut().enumerate() {
        let authority_coordinate = if expanded_rule(profile, coordinate)?.prestate
            == AccountPrestateV2::AuthenticatedRouteAlias
        {
            profile.representative(tail_count, coordinate)?
        } else {
            coordinate
        };
        let rule = expanded_rule(profile, authority_coordinate)?;
        *permission = rule.permission();
    }
    Ok(())
}

/// Expand exact effect permissions for profile 13 after dynamic-span insertion.
pub fn derive_effect_permissions_with_dynamic_spans(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    output: &mut [AccountPermission],
) -> Result<()> {
    if output.len() != dynamic_account_width(profile, tail_count, span_counts)? {
        return Err(Error::WidthMismatch);
    }
    for (coordinate, permission) in output.iter_mut().enumerate() {
        let rule = expanded_rule_with_dynamic_spans(profile, tail_count, span_counts, coordinate)?;
        let authority_coordinate = if rule.prestate == AccountPrestateV2::AuthenticatedRouteAlias {
            profile.representative_with_dynamic_spans(tail_count, span_counts, coordinate)?
        } else {
            coordinate
        };
        *permission = expanded_rule_with_dynamic_spans(
            profile,
            tail_count,
            span_counts,
            authority_coordinate,
        )?
        .permission();
    }
    Ok(())
}

fn validate_accounts(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    accounts: &[AccountObservationV1<'_>],
) -> Result<()> {
    for (coordinate, account) in accounts.iter().copied().enumerate() {
        let rule = expanded_rule(profile, coordinate)?;
        let representative = profile.representative(tail_count, coordinate)?;
        let expected_privileges = if profile.supports_route_alias_packing() {
            representative_privileges(profile, tail_count, representative)?
        } else {
            rule.privileges
        };
        if account.privileges() != expected_privileges {
            return Err(Error::PrivilegeMismatch);
        }
        let variable_representative =
            rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableData;
        let variable_alias =
            rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableDataAlias;
        let route_alias = rule.prestate == AccountPrestateV2::AuthenticatedRouteAlias;
        if (variable_representative && !account.adapter_authenticated_variable_data())
            || (variable_alias && account.adapter_authenticated_variable_data())
            || (route_alias && account.adapter_authenticated_variable_data())
            || (!variable_representative
                && !variable_alias
                && !route_alias
                && account.adapter_authenticated_variable_data())
        {
            return Err(Error::InvalidVariableDataPrestate);
        }
        let exact_data_length = exact_rule_data_length(rule, tail_count)?;
        match rule.prestate {
            AccountPrestateV2::Exact if account.data().len() != exact_data_length => {
                return Err(Error::DataLengthMismatch);
            }
            AccountPrestateV2::LifecycleBound
                if !account.data().is_empty() && account.data().len() != exact_data_length =>
            {
                return Err(Error::DataLengthMismatch);
            }
            AccountPrestateV2::AdapterAuthenticatedVariableData
                if account.data().is_empty()
                    || account.data().len() < exact_data_length
                    || !account.adapter_authenticated_variable_data() =>
            {
                return Err(Error::InvalidVariableDataPrestate);
            }
            AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
                if account.data().is_empty() || account.adapter_authenticated_variable_data() =>
            {
                return Err(Error::InvalidVariableDataPrestate);
            }
            AccountPrestateV2::AuthenticatedRouteAlias => {}
            _ => {}
        }
        let canonical = accounts
            .get(representative)
            .copied()
            .ok_or(Error::InvalidCoordinate)?;
        let canonical_rule = expanded_rule(profile, representative)?;
        if account.key() != canonical.key()
            || account.owner() != canonical.owner()
            || account.lamports() != canonical.lamports()
            || account.data() != canonical.data()
            || account.privileges() != canonical.privileges()
            || (variable_alias
                && (canonical_rule.prestate != AccountPrestateV2::AdapterAuthenticatedVariableData
                    || !canonical.adapter_authenticated_variable_data()))
            || (!(variable_alias || route_alias)
                && account.adapter_authenticated_variable_data()
                    != canonical.adapter_authenticated_variable_data())
        {
            return Err(Error::AliasMismatch);
        }
        if representative == coordinate {
            let mut prior = 0_usize;
            while prior < coordinate {
                if profile.representative(tail_count, prior)? == prior
                    && accounts.get(prior).ok_or(Error::InvalidCoordinate)?.key() == account.key()
                {
                    return Err(Error::CrossItemAlias);
                }
                prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
            }
        }
    }
    Ok(())
}

fn validate_accounts_with_dynamic_spans(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    accounts: &[AccountObservationV1<'_>],
) -> Result<()> {
    if accounts.len() != dynamic_account_width(profile, tail_count, span_counts)? {
        return Err(Error::WidthMismatch);
    }
    for (coordinate, account) in accounts.iter().copied().enumerate() {
        let rule = expanded_rule_with_dynamic_spans(profile, tail_count, span_counts, coordinate)?;
        let representative =
            profile.representative_with_dynamic_spans(tail_count, span_counts, coordinate)?;
        let expected_privileges = representative_privileges_with_dynamic_spans(
            profile,
            tail_count,
            span_counts,
            representative,
        )?;
        if account.privileges() != expected_privileges {
            return Err(Error::PrivilegeMismatch);
        }
        let variable_representative =
            rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableData;
        let variable_alias =
            rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableDataAlias;
        let route_alias = rule.prestate == AccountPrestateV2::AuthenticatedRouteAlias;
        if (variable_representative && !account.adapter_authenticated_variable_data())
            || ((variable_alias || route_alias) && account.adapter_authenticated_variable_data())
            || (!variable_representative
                && !variable_alias
                && !route_alias
                && account.adapter_authenticated_variable_data())
        {
            return Err(Error::InvalidVariableDataPrestate);
        }
        let exact_data_length = exact_rule_data_length(rule, tail_count)?;
        match rule.prestate {
            AccountPrestateV2::Exact if account.data().len() != exact_data_length => {
                return Err(Error::DataLengthMismatch);
            }
            AccountPrestateV2::LifecycleBound
                if !account.data().is_empty() && account.data().len() != exact_data_length =>
            {
                return Err(Error::DataLengthMismatch);
            }
            AccountPrestateV2::AdapterAuthenticatedVariableData
                if account.data().is_empty()
                    || account.data().len() < exact_data_length
                    || !account.adapter_authenticated_variable_data() =>
            {
                return Err(Error::InvalidVariableDataPrestate);
            }
            AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
                if account.data().is_empty() || account.adapter_authenticated_variable_data() =>
            {
                return Err(Error::InvalidVariableDataPrestate);
            }
            AccountPrestateV2::AuthenticatedRouteAlias
            | AccountPrestateV2::AuthenticatedOpaqueReadonlyData => {}
            _ => {}
        }
        let canonical = accounts
            .get(representative)
            .copied()
            .ok_or(Error::InvalidCoordinate)?;
        let canonical_rule =
            expanded_rule_with_dynamic_spans(profile, tail_count, span_counts, representative)?;
        if account.key() != canonical.key()
            || account.owner() != canonical.owner()
            || account.lamports() != canonical.lamports()
            || account.data() != canonical.data()
            || account.privileges() != canonical.privileges()
            || (variable_alias
                && (canonical_rule.prestate != AccountPrestateV2::AdapterAuthenticatedVariableData
                    || !canonical.adapter_authenticated_variable_data()))
            || (!(variable_alias || route_alias)
                && account.adapter_authenticated_variable_data()
                    != canonical.adapter_authenticated_variable_data())
        {
            return Err(Error::AliasMismatch);
        }
        if representative == coordinate {
            let mut prior = 0_usize;
            while prior < coordinate {
                if profile.representative_with_dynamic_spans(tail_count, span_counts, prior)?
                    == prior
                    && accounts.get(prior).ok_or(Error::InvalidCoordinate)?.key() == account.key()
                {
                    return Err(Error::CrossItemAlias);
                }
                prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
            }
        }
    }
    Ok(())
}

fn representative_privileges(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    representative: usize,
) -> Result<u8> {
    if profile.representative(tail_count, representative)? != representative {
        return Err(Error::InvalidAlias);
    }
    let representative_rule = expanded_rule(profile, representative)?;
    let executable = representative_rule.privileges & 0x04;
    let mut union = executable;
    let logical_count = account_width(profile, tail_count)?;
    let mut coordinate = 0_usize;
    while coordinate < logical_count {
        if profile.representative(tail_count, coordinate)? == representative {
            let rule = expanded_rule(profile, coordinate)?;
            if rule.privileges & 0x04 != executable {
                return Err(Error::InvalidRouteAlias);
            }
            union |= rule.privileges & 0x03;
        }
        coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(union)
}

fn representative_privileges_with_dynamic_spans(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    representative: usize,
) -> Result<u8> {
    if profile.representative_with_dynamic_spans(tail_count, span_counts, representative)?
        != representative
    {
        return Err(Error::InvalidAlias);
    }
    let representative_rule =
        expanded_rule_with_dynamic_spans(profile, tail_count, span_counts, representative)?;
    let executable = representative_rule.privileges & 0x04;
    let mut union = executable | (representative_rule.privileges & 0x03);
    if representative_rule.effect_permissions != 0 {
        union |= 0x02;
    }
    let logical = dynamic_account_width(profile, tail_count, span_counts)?;
    let mut coordinate = 0_usize;
    while coordinate < logical {
        if profile.representative_with_dynamic_spans(tail_count, span_counts, coordinate)?
            == representative
        {
            let rule =
                expanded_rule_with_dynamic_spans(profile, tail_count, span_counts, coordinate)?;
            if rule.privileges & 0x04 != executable {
                return Err(Error::InvalidRouteAlias);
            }
            union |= rule.privileges & 0x03;
        }
        coordinate = coordinate.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(union)
}

fn inject_indices(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    scalars: &mut [u64],
) -> Result<()> {
    // Fixed-geometry profiles may still bind the Product-owned runtime count
    // without declaring any per-item register bank. There is no item-index
    // destination to seed in that shape.
    if profile.item_scalar_stride == 0 {
        return Ok(());
    }
    let mut item = 0_u32;
    while item < tail_count {
        let offset =
            item_register_index(profile.common_scalars, profile.item_scalar_stride, item, 0)?;
        *scalars.get_mut(offset).ok_or(Error::InvalidCoordinate)? = u64::from(item);
        item = item.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_operations(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    accounts: &[AccountObservationV1<'_>],
    input_identities: &[[u8; 32]],
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<()> {
    let mut fixed = 0_u16;
    while fixed < profile.fixed_operations {
        profile.operation(false, fixed)?.apply(
            profile,
            None,
            tail_count,
            None,
            accounts,
            input_identities,
            scalars,
            identities,
        )?;
        fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    let mut item = 0_u32;
    while item < tail_count {
        let mut operation = 0_u16;
        while operation < profile.item_operations {
            profile.operation(true, operation)?.apply(
                profile,
                Some(item),
                tail_count,
                None,
                accounts,
                input_identities,
                scalars,
                identities,
            )?;
            operation = operation.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        item = item.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_operations_with_dynamic_spans(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    accounts: &[AccountObservationV1<'_>],
    input_identities: &[[u8; 32]],
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<()> {
    require_dynamic_span_counts(profile, span_counts)?;
    let mut fixed = 0_u16;
    while fixed < profile.fixed_operations {
        let operation = profile.operation(false, fixed)?;
        let account_index =
            dynamic_runtime_coordinate_for_base(profile, span_counts, operation.account)?;
        operation.apply(
            profile,
            None,
            tail_count,
            Some((account_index, profile.rule(false, operation.account)?)),
            accounts,
            input_identities,
            scalars,
            identities,
        )?;
        fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Operation {
    opcode: u8,
    account_item: bool,
    account: u16,
    register_item: bool,
    register: u16,
    data_offset: u32,
    data_stride: u32,
}

impl Operation {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        let account_space = byte(bytes, add(offset, 1)?)?;
        let register_space = byte(bytes, add(offset, 4)?)?;
        if account_space > 1 || register_space > 1 || byte(bytes, add(offset, 5)?)? != 0 {
            return Err(Error::NonCanonicalOperation);
        }
        Ok(Self {
            opcode: byte(bytes, offset)?,
            account_item: account_space == 1,
            account: read_u16(bytes, add(offset, 2)?)?,
            register_item: register_space == 1,
            register: read_u16(bytes, add(offset, 6)?)?,
            data_offset: read_u32(bytes, add(offset, 8)?)?,
            data_stride: read_u32(bytes, add(offset, 12)?)?,
        })
    }

    fn support_count_destination(self) -> Result<u16> {
        u16::try_from(self.data_stride >> 16).map_err(|_| Error::InvalidCoordinate)
    }

    fn support_row_scalar_stride(self) -> Result<u16> {
        u16::try_from(self.data_stride & u32::from(u16::MAX)).map_err(|_| Error::InvalidCoordinate)
    }

    fn validate(
        self,
        profile: AccountProfileV2<'_>,
        item_body: bool,
        operation_index: u16,
    ) -> Result<()> {
        if !item_body && (self.account_item || self.register_item) {
            return Err(Error::NonCanonicalOperation);
        }
        let account_bound = if self.account_item {
            profile.item_account_stride
        } else {
            profile.fixed_accounts
        };
        if self.account >= account_bound {
            return Err(Error::InvalidCoordinate);
        }
        let account_rule = profile.rule(self.account_item, self.account)?;
        if matches!(
            account_rule.prestate,
            AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
                | AccountPrestateV2::AuthenticatedRouteAlias
        ) {
            // Route aliases exist only so Effect may select the same already
            // authenticated physical record in a child frame. AccountProfile
            // cannot project or require through a second logical authority.
            return Err(Error::InvalidVariableDataPrestate);
        }
        if account_rule.prestate == AccountPrestateV2::AuthenticatedOpaqueReadonlyData
            && !matches!(
                self.opcode,
                OP_REQUIRE_KEY
                    | OP_REQUIRE_OWNER
                    | OP_PROJECT_KEY
                    | OP_PROJECT_OWNER
                    | OP_PROJECT_LAMPORTS
            )
        {
            return Err(Error::InvalidOpaqueDataPrestate);
        }
        let identity = matches!(
            self.opcode,
            OP_REQUIRE_KEY
                | OP_REQUIRE_OWNER
                | OP_PROJECT_KEY
                | OP_PROJECT_OWNER
                | OP_PROJECT_DATA_IDENTITY
                | OP_PROJECT_DATA_IDENTITY_AFFINE
                | OP_PROJECT_DATA_IDENTITY_SELECTED
                | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE
        );
        let register_bound = if identity {
            if self.register_item {
                profile.item_identity_stride
            } else {
                profile.common_identities
            }
        } else if self.register_item {
            profile.item_scalar_stride
        } else {
            profile.common_scalars
        };
        if self.register >= register_bound {
            return Err(Error::InvalidCoordinate);
        }
        match self.opcode {
            OP_REQUIRE_KEY | OP_REQUIRE_OWNER | OP_PROJECT_KEY | OP_PROJECT_OWNER
            | OP_PROJECT_LAMPORTS => {
                if self.data_offset != 0 || self.data_stride != 0 {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_DATA_U64
            | OP_PROJECT_DATA_IDENTITY
            | OP_PROJECT_DATA_U32
            | OP_PROJECT_DATA_U16
            | OP_PROJECT_DATA_U8
            | OP_PROJECT_TAIL_COUNT_U32 => {
                if self.data_stride != 0 {
                    return Err(Error::NonCanonicalOperation);
                }
                if self.opcode == OP_PROJECT_TAIL_COUNT_U32
                    && (item_body || self.account_item || self.register_item)
                {
                    return Err(Error::NonCanonicalOperation);
                }
                if matches!(self.opcode, OP_PROJECT_DATA_U16 | OP_PROJECT_DATA_U8)
                    && !matches!(
                        profile.artifact_profile,
                        TYPED_SCALAR_ARTIFACT_PROFILE
                            | TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
                            | LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                            | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
                            | TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
                            | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                            | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                            | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                            | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
                            | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                            | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
                    )
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_DATA_U64_AFFINE => {
                if !item_body
                    || self.account_item
                    || !self.register_item
                    || self.data_stride < 8
                    || self.data_offset.checked_add(8).is_none()
                    || (profile.artifact_profile == ARTIFACT_PROFILE
                        && self.data_offset.checked_add(8).is_none_or(|end| {
                            end > profile
                                .rule(false, self.account)
                                .map(|rule| rule.data_length)
                                .unwrap_or(0)
                        }))
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_DATA_IDENTITY_AFFINE => {
                if !item_body
                    || self.account_item
                    || !self.register_item
                    || self.data_stride < 32
                    || self.data_offset.checked_add(32).is_none()
                    || (profile.artifact_profile == ARTIFACT_PROFILE
                        && self.data_offset.checked_add(32).is_none_or(|end| {
                            end > profile
                                .rule(false, self.account)
                                .map(|rule| rule.data_length)
                                .unwrap_or(0)
                        }))
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_SELECT_DATA_WINDOW => {
                if !matches!(
                    profile.artifact_profile,
                    SELECTED_WINDOW_ARTIFACT_PROFILE
                        | TYPED_SCALAR_ARTIFACT_PROFILE
                        | TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
                        | LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                        | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
                        | TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
                        | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                        | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                        | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                        | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
                        | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                        | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
                ) || item_body
                    || self.account_item
                    || self.register_item
                    || self.data_stride == 0
                    || self.data_offset.checked_add(self.data_stride).is_none()
                    || profile.fixed_scalar_projection_index(self.register)? >= operation_index
                {
                    return Err(Error::NonCanonicalOperation);
                }
                let item_stride = profile.selected_item_stride(self.account)?;
                let rule = profile.rule(false, self.account)?;
                let row_bytes = rule
                    .data_length
                    .checked_sub(self.data_offset)
                    .ok_or(Error::NonCanonicalOperation)?;
                if row_bytes == 0
                    || row_bytes % self.data_stride != 0
                    || rule.data_item_stride
                        != (row_bytes / self.data_stride)
                            .checked_mul(item_stride)
                            .ok_or(Error::NonCanonicalOperation)?
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_DATA_U64_SELECTED | OP_PROJECT_DATA_IDENTITY_SELECTED => {
                let (_, window) = profile.selected_window(self.account)?;
                let width = if self.opcode == OP_PROJECT_DATA_U64_SELECTED {
                    8
                } else {
                    32
                };
                if !matches!(
                    profile.artifact_profile,
                    SELECTED_WINDOW_ARTIFACT_PROFILE
                        | TYPED_SCALAR_ARTIFACT_PROFILE
                        | TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
                        | LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
                        | TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
                        | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                        | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                        | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                        | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
                        | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                        | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
                ) || item_body
                    || self.account_item
                    || self.register_item
                    || self.data_stride != 0
                    || self
                        .data_offset
                        .checked_add(width)
                        .is_none_or(|end| end > window.data_stride)
                    || profile.fixed_scalar_projection_index(window.register)? >= operation_index
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_DATA_U64_SELECTED_AFFINE | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE => {
                let (_, window) = profile.selected_window(self.account)?;
                let width = if self.opcode == OP_PROJECT_DATA_U64_SELECTED_AFFINE {
                    8
                } else {
                    32
                };
                if profile.artifact_profile != SELECTED_WINDOW_ARTIFACT_PROFILE
                    || !item_body
                    || self.account_item
                    || !self.register_item
                    || self.data_stride < width
                    || self.data_offset < window.data_stride
                    || self.data_offset.checked_add(width).is_none_or(|end| {
                        end > window
                            .data_stride
                            .checked_add(self.data_stride)
                            .unwrap_or(0)
                    })
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_NONZERO_U64_TAIL_COUNT => {
                if profile.artifact_profile != NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                    || item_body
                    || self.account_item
                    || self.register_item
                    || self.data_stride != 8
                    || self.data_offset != account_rule.data_length
                    || account_rule.prestate != AccountPrestateV2::AdapterAuthenticatedVariableData
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_NONZERO_U64_TAIL_ROWS => {
                let count_destination = self.support_count_destination()?;
                let row_stride = self.support_row_scalar_stride()?;
                let row_registers = profile
                    .common_scalars
                    .checked_sub(self.register)
                    .ok_or(Error::InvalidCoordinate)?;
                if profile.artifact_profile != NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
                    || item_body
                    || self.account_item
                    || self.register_item
                    || profile.item_scalar_stride != 0
                    || count_destination >= profile.common_scalars
                    || count_destination >= self.register
                    || row_stride < 2
                    || row_registers == 0
                    || row_registers % row_stride != 0
                    || self.data_offset != account_rule.data_length
                    || account_rule.prestate != AccountPrestateV2::AdapterAuthenticatedVariableData
                {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            _ => return Err(Error::UnknownOperation),
        }
        if item_body
            && self.register_item
            && self.register == 0
            && matches!(self.projection_target()?, Some((false, true, 0)))
        {
            return Err(Error::DuplicateProjection);
        }
        Ok(())
    }

    fn is_projection(self) -> bool {
        matches!(
            self.opcode,
            OP_PROJECT_KEY
                | OP_PROJECT_OWNER
                | OP_PROJECT_LAMPORTS
                | OP_PROJECT_DATA_U64
                | OP_PROJECT_DATA_IDENTITY
                | OP_PROJECT_DATA_U64_AFFINE
                | OP_PROJECT_DATA_IDENTITY_AFFINE
                | OP_PROJECT_DATA_U64_SELECTED
                | OP_PROJECT_DATA_IDENTITY_SELECTED
                | OP_PROJECT_DATA_U64_SELECTED_AFFINE
                | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE
                | OP_PROJECT_DATA_U32
                | OP_PROJECT_DATA_U16
                | OP_PROJECT_DATA_U8
                | OP_PROJECT_TAIL_COUNT_U32
                | OP_PROJECT_NONZERO_U64_TAIL_COUNT
        )
    }

    fn projection_target(self) -> Result<Option<(bool, bool, u16)>> {
        if !self.is_projection() {
            return Ok(None);
        }
        let identity = matches!(
            self.opcode,
            OP_PROJECT_KEY
                | OP_PROJECT_OWNER
                | OP_PROJECT_DATA_IDENTITY
                | OP_PROJECT_DATA_IDENTITY_AFFINE
                | OP_PROJECT_DATA_IDENTITY_SELECTED
                | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE
        );
        Ok(Some((identity, self.register_item, self.register)))
    }

    fn writes_target(
        self,
        profile: AccountProfileV2<'_>,
        target: (bool, bool, u16),
    ) -> Result<bool> {
        if self.opcode != OP_PROJECT_NONZERO_U64_TAIL_ROWS {
            return Ok(self.projection_target()? == Some(target));
        }
        let (identity, item, register) = target;
        if identity || item {
            return Ok(false);
        }
        if register == self.support_count_destination()? {
            return Ok(true);
        }
        let stride = self.support_row_scalar_stride()?;
        let Some(relative) = register.checked_sub(self.register) else {
            return Ok(false);
        };
        if register >= profile.common_scalars || stride == 0 {
            return Ok(false);
        }
        Ok(relative % stride < 2)
    }

    fn is_selected_affine_projection(self) -> bool {
        matches!(
            self.opcode,
            OP_PROJECT_DATA_U64_SELECTED_AFFINE | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE
        )
    }

    fn is_selected_projection(self) -> bool {
        matches!(
            self.opcode,
            OP_PROJECT_DATA_U64_SELECTED
                | OP_PROJECT_DATA_IDENTITY_SELECTED
                | OP_PROJECT_DATA_U64_SELECTED_AFFINE
                | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        self,
        profile: AccountProfileV2<'_>,
        item: Option<u32>,
        tail_count: u32,
        resolved_account: Option<(usize, AccountRuleV2)>,
        accounts: &[AccountObservationV1<'_>],
        input_identities: &[[u8; 32]],
        scalars: &mut [u64],
        identities: &mut [[u8; 32]],
    ) -> Result<()> {
        let (account_index, rule) = if let Some(resolved) = resolved_account {
            resolved
        } else {
            let account_index = if self.account_item {
                item_account_index(profile, item.ok_or(Error::InvalidCoordinate)?, self.account)?
            } else {
                usize::from(self.account)
            };
            (account_index, expanded_rule(profile, account_index)?)
        };
        let account = accounts
            .get(account_index)
            .copied()
            .ok_or(Error::InvalidCoordinate)?;
        let scalar = || {
            register_index(
                profile.common_scalars,
                profile.item_scalar_stride,
                self.register_item,
                item,
                self.register,
            )
        };
        let identity = || {
            register_index(
                profile.common_identities,
                profile.item_identity_stride,
                self.register_item,
                item,
                self.register,
            )
        };
        match self.opcode {
            OP_REQUIRE_KEY => require(
                account.key()
                    == *input_identities
                        .get(identity()?)
                        .ok_or(Error::InvalidCoordinate)?,
            ),
            OP_REQUIRE_OWNER => require(
                account.owner()
                    == *input_identities
                        .get(identity()?)
                        .ok_or(Error::InvalidCoordinate)?,
            ),
            OP_PROJECT_KEY => write_identity(identities, identity()?, account.key()),
            OP_PROJECT_OWNER => write_identity(identities, identity()?, account.owner()),
            OP_PROJECT_LAMPORTS => write_scalar(scalars, scalar()?, account.lamports()),
            OP_PROJECT_DATA_U64 => {
                let value = projected_data_field(account.data(), rule, self.data_offset, 8)?
                    .map(|bytes| {
                        bytes
                            .try_into()
                            .map(u64::from_le_bytes)
                            .map_err(|_| Error::DataOutOfBounds)
                    })
                    .transpose()?
                    .unwrap_or(0);
                write_scalar(scalars, scalar()?, value)
            }
            OP_PROJECT_DATA_U32 | OP_PROJECT_TAIL_COUNT_U32 => {
                let value = projected_data_field(account.data(), rule, self.data_offset, 4)?
                    .map(|bytes| {
                        bytes
                            .try_into()
                            .map(u32::from_le_bytes)
                            .map(u64::from)
                            .map_err(|_| Error::DataOutOfBounds)
                    })
                    .transpose()?
                    .unwrap_or(0);
                write_scalar(scalars, scalar()?, value)
            }
            OP_PROJECT_DATA_U16 => {
                let value = projected_data_field(account.data(), rule, self.data_offset, 2)?
                    .map(|bytes| {
                        bytes
                            .try_into()
                            .map(u16::from_le_bytes)
                            .map(u64::from)
                            .map_err(|_| Error::DataOutOfBounds)
                    })
                    .transpose()?
                    .unwrap_or(0);
                write_scalar(scalars, scalar()?, value)
            }
            OP_PROJECT_DATA_U8 => {
                let value = projected_data_field(account.data(), rule, self.data_offset, 1)?
                    .and_then(|bytes| bytes.first().copied())
                    .map_or(0, u64::from);
                write_scalar(scalars, scalar()?, value)
            }
            OP_PROJECT_NONZERO_U64_TAIL_COUNT => {
                let expected = u64::from(tail_count)
                    .checked_mul(8)
                    .and_then(|tail| u64::from(self.data_offset).checked_add(tail))
                    .and_then(|width| usize::try_from(width).ok())
                    .ok_or(Error::DataLengthMismatch)?;
                if account.data().len() != expected {
                    return Err(Error::DataLengthMismatch);
                }
                let mut count = 0_u64;
                let mut row = 0_u32;
                while row < tail_count {
                    let offset = u64::from(row)
                        .checked_mul(8)
                        .and_then(|tail| u64::from(self.data_offset).checked_add(tail))
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or(Error::DataOutOfBounds)?;
                    let end = offset.checked_add(8).ok_or(Error::DataOutOfBounds)?;
                    let value = u64::from_le_bytes(
                        account
                            .data()
                            .get(offset..end)
                            .ok_or(Error::DataOutOfBounds)?
                            .try_into()
                            .map_err(|_| Error::DataOutOfBounds)?,
                    );
                    if value != 0 {
                        count = count.checked_add(1).ok_or(Error::InvalidLength)?;
                    }
                    row = row.checked_add(1).ok_or(Error::InvalidLength)?;
                }
                if count == 0 {
                    return Err(Error::EmptyNonzeroTail);
                }
                write_scalar(scalars, scalar()?, count)
            }
            OP_PROJECT_NONZERO_U64_TAIL_ROWS => {
                let expected = u64::from(tail_count)
                    .checked_mul(8)
                    .and_then(|tail| u64::from(self.data_offset).checked_add(tail))
                    .and_then(|width| usize::try_from(width).ok())
                    .ok_or(Error::DataLengthMismatch)?;
                if account.data().len() != expected {
                    return Err(Error::DataLengthMismatch);
                }
                let stride = usize::from(self.support_row_scalar_stride()?);
                let row_start = usize::from(self.register);
                let expected_rows = scalars
                    .len()
                    .checked_sub(row_start)
                    .filter(|remaining| *remaining != 0 && *remaining % stride == 0)
                    .map(|remaining| remaining / stride)
                    .ok_or(Error::InvalidCoordinate)?;
                let mut support_row = 0_usize;
                let mut outcome = 0_u32;
                while outcome < tail_count {
                    let offset = u64::from(outcome)
                        .checked_mul(8)
                        .and_then(|tail| u64::from(self.data_offset).checked_add(tail))
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or(Error::DataOutOfBounds)?;
                    let end = offset.checked_add(8).ok_or(Error::DataOutOfBounds)?;
                    let coefficient = u64::from_le_bytes(
                        account
                            .data()
                            .get(offset..end)
                            .ok_or(Error::DataOutOfBounds)?
                            .try_into()
                            .map_err(|_| Error::DataOutOfBounds)?,
                    );
                    if coefficient != 0 {
                        if support_row >= expected_rows {
                            return Err(Error::SupportRowCountMismatch);
                        }
                        let destination = support_row
                            .checked_mul(stride)
                            .and_then(|relative| row_start.checked_add(relative))
                            .ok_or(Error::InvalidCoordinate)?;
                        write_scalar(scalars, destination, u64::from(outcome))?;
                        write_scalar(
                            scalars,
                            destination.checked_add(1).ok_or(Error::InvalidCoordinate)?,
                            coefficient,
                        )?;
                        support_row = support_row.checked_add(1).ok_or(Error::InvalidLength)?;
                    }
                    outcome = outcome.checked_add(1).ok_or(Error::InvalidLength)?;
                }
                if support_row == 0 {
                    return Err(Error::EmptyNonzeroTail);
                }
                if support_row != expected_rows {
                    return Err(Error::SupportRowCountMismatch);
                }
                write_scalar(
                    scalars,
                    usize::from(self.support_count_destination()?),
                    u64::try_from(support_row).map_err(|_| Error::InvalidLength)?,
                )
            }
            OP_PROJECT_DATA_IDENTITY => {
                let value = projected_data_field(account.data(), rule, self.data_offset, 32)?
                    .map(|bytes| bytes.try_into().map_err(|_| Error::DataOutOfBounds))
                    .transpose()?
                    .unwrap_or([0; 32]);
                write_identity(identities, identity()?, value)
            }
            OP_PROJECT_DATA_U64_AFFINE => {
                let offset = affine_data_offset(
                    self.data_offset,
                    self.data_stride,
                    item.ok_or(Error::InvalidCoordinate)?,
                    8,
                )?;
                let value = projected_data_field(account.data(), rule, offset, 8)?
                    .map(|bytes| {
                        bytes
                            .try_into()
                            .map(u64::from_le_bytes)
                            .map_err(|_| Error::DataOutOfBounds)
                    })
                    .transpose()?
                    .unwrap_or(0);
                write_scalar(scalars, scalar()?, value)
            }
            OP_PROJECT_DATA_IDENTITY_AFFINE => {
                let offset = affine_data_offset(
                    self.data_offset,
                    self.data_stride,
                    item.ok_or(Error::InvalidCoordinate)?,
                    32,
                )?;
                let value = projected_data_field(account.data(), rule, offset, 32)?
                    .map(|bytes| bytes.try_into().map_err(|_| Error::DataOutOfBounds))
                    .transpose()?
                    .unwrap_or([0; 32]);
                write_identity(identities, identity()?, value)
            }
            OP_SELECT_DATA_WINDOW => {
                let (offset, width) = selected_window_range(profile, self, tail_count, scalars)?;
                let width = usize::try_from(width).map_err(|_| Error::DataOutOfBounds)?;
                data_field(account.data(), offset, width).map(|_| ())
            }
            OP_PROJECT_DATA_U64_SELECTED | OP_PROJECT_DATA_U64_SELECTED_AFFINE => {
                let offset = selected_data_offset(profile, self, item, tail_count, scalars, 8)?;
                let bytes = data_field(account.data(), offset, 8)?;
                write_scalar(
                    scalars,
                    scalar()?,
                    u64::from_le_bytes(bytes.try_into().map_err(|_| Error::DataOutOfBounds)?),
                )
            }
            OP_PROJECT_DATA_IDENTITY_SELECTED | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE => {
                let offset = selected_data_offset(profile, self, item, tail_count, scalars, 32)?;
                let bytes = data_field(account.data(), offset, 32)?;
                write_identity(
                    identities,
                    identity()?,
                    bytes.try_into().map_err(|_| Error::DataOutOfBounds)?,
                )
            }
            _ => Err(Error::UnknownOperation),
        }
    }
}

fn selected_window_range(
    profile: AccountProfileV2<'_>,
    window: Operation,
    tail_count: u32,
    scalars: &[u64],
) -> Result<(u32, u32)> {
    let selector = u32::try_from(
        *scalars
            .get(usize::from(window.register))
            .ok_or(Error::InvalidCoordinate)?,
    )
    .map_err(|_| Error::InvalidCoordinate)?;
    let item_stride = profile.selected_item_stride(window.account)?;
    let row_width = tail_count
        .checked_mul(item_stride)
        .and_then(|tail| window.data_stride.checked_add(tail))
        .ok_or(Error::DataOutOfBounds)?;
    let start = selector
        .checked_mul(row_width)
        .and_then(|selected| window.data_offset.checked_add(selected))
        .ok_or(Error::DataOutOfBounds)?;
    start.checked_add(row_width).ok_or(Error::DataOutOfBounds)?;
    Ok((start, row_width))
}

fn selected_data_offset(
    profile: AccountProfileV2<'_>,
    operation: Operation,
    item: Option<u32>,
    tail_count: u32,
    scalars: &[u64],
    width: u32,
) -> Result<u32> {
    let (_, window) = profile.selected_window(operation.account)?;
    let (row_start, _) = selected_window_range(profile, window, tail_count, scalars)?;
    let local = if operation.is_selected_affine_projection() {
        operation
            .data_stride
            .checked_mul(item.ok_or(Error::InvalidCoordinate)?)
            .and_then(|offset| operation.data_offset.checked_add(offset))
            .ok_or(Error::DataOutOfBounds)?
    } else {
        operation.data_offset
    };
    row_start
        .checked_add(local)
        .and_then(|offset| offset.checked_add(width).map(|_| offset))
        .ok_or(Error::DataOutOfBounds)
}

fn validate_rule(
    rule: AccountRuleV2,
    item: bool,
    index: u16,
    fixed_count: u16,
    artifact_profile: u16,
) -> Result<()> {
    if rule.privileges & !0x07 != 0 {
        return Err(Error::InvalidPrivileges);
    }
    if rule.effect_permissions
        & !(EFFECT_PERMISSION_DEBIT_LAMPORTS
            | EFFECT_PERMISSION_CREDIT_LAMPORTS
            | EFFECT_PERMISSION_WRITE_DATA)
        != 0
        || (rule.effect_permissions != 0
            && rule.privileges & 0x02 == 0
            && !(matches!(
                artifact_profile,
                DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
            ) && !item
                && rule.alias_kind == AliasKindV2::SelfCoordinate
                && rule.alias_index == 0))
    {
        return Err(Error::InvalidEffectPermissions);
    }
    if rule.prestate == AccountPrestateV2::LifecycleBound
        && (rule.alias_kind != AliasKindV2::SelfCoordinate
            || rule.alias_index != 0
            || rule.privileges != 0x02
            || rule.data_length == 0
            || (rule.data_item_stride != 0
                && artifact_profile != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE)
            || rule.effect_permissions
                & (EFFECT_PERMISSION_CREDIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA)
                != (EFFECT_PERMISSION_CREDIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA))
    {
        return Err(Error::InvalidLifecyclePrestate);
    }
    if rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableData
        && (item
            || rule.alias_kind != AliasKindV2::SelfCoordinate
            || rule.alias_index != 0
            || rule.privileges != 0
            || rule.effect_permissions != 0
            || rule.data_length == 0
            || rule.data_item_stride != 0)
    {
        return Err(Error::InvalidVariableDataPrestate);
    }
    if rule.prestate == AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
        && (item
            || rule.alias_kind != AliasKindV2::Fixed
            || rule.alias_index >= index
            || rule.privileges != 0
            || rule.effect_permissions != 0
            || rule.data_length != 0
            || rule.data_item_stride != 0)
    {
        return Err(Error::InvalidVariableDataPrestate);
    }
    if rule.prestate == AccountPrestateV2::AuthenticatedRouteAlias
        && (item
            || rule.alias_kind != AliasKindV2::Fixed
            || rule.alias_index >= index
            || rule.effect_permissions != 0
            || rule.data_length != 0
            || rule.data_item_stride != 0)
    {
        return Err(Error::InvalidRouteAlias);
    }
    if rule.prestate == AccountPrestateV2::AuthenticatedOpaqueReadonlyData
        && (rule.alias_kind != AliasKindV2::SelfCoordinate
            || rule.alias_index != 0
            || rule.effect_permissions != 0
            || rule.data_length != 0
            || rule.data_item_stride != 0)
    {
        return Err(Error::InvalidOpaqueDataPrestate);
    }
    match rule.alias_kind {
        AliasKindV2::SelfCoordinate if rule.alias_index == 0 => Ok(()),
        AliasKindV2::Fixed
            if ((!item && rule.alias_index < index)
                || (item && rule.alias_index < fixed_count)) =>
        {
            Ok(())
        }
        AliasKindV2::SameItem if item && rule.alias_index < index => Ok(()),
        _ => Err(Error::InvalidAlias),
    }
}

fn decode_rule(bytes: &[u8], offset: usize, artifact_profile: u16) -> Result<AccountRuleV2> {
    let alias_kind = match byte(bytes, add(offset, 2)?)? {
        0 => AliasKindV2::SelfCoordinate,
        1 => AliasKindV2::Fixed,
        2 => AliasKindV2::SameItem,
        _ => return Err(Error::InvalidAlias),
    };
    let prestate_tag = byte(bytes, add(offset, 3)?)?;
    let prestate = if artifact_profile == LIFECYCLE_PRESTATE_ARTIFACT_PROFILE {
        match prestate_tag {
            0 => AccountPrestateV2::Exact,
            1 => AccountPrestateV2::LifecycleBound,
            _ => return Err(Error::InvalidLifecyclePrestate),
        }
    } else if matches!(
        artifact_profile,
        ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
            | TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
            | ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
            | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
            | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
            | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
            | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
            | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
    ) {
        match prestate_tag {
            0 => AccountPrestateV2::Exact,
            1 => AccountPrestateV2::LifecycleBound,
            2 => AccountPrestateV2::AdapterAuthenticatedVariableData,
            3 if matches!(
                artifact_profile,
                ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
                    | NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
                    | AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                    | NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
            ) =>
            {
                AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
            }
            4 if matches!(
                artifact_profile,
                AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
                    | DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
                    | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
            ) =>
            {
                AccountPrestateV2::AuthenticatedRouteAlias
            }
            5 if matches!(
                artifact_profile,
                DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE | FIXED_DATA_PREDICATE_ARTIFACT_PROFILE
            ) =>
            {
                AccountPrestateV2::AuthenticatedOpaqueReadonlyData
            }
            _ => return Err(Error::InvalidVariableDataPrestate),
        }
    } else if prestate_tag == 0 {
        AccountPrestateV2::Exact
    } else {
        return Err(Error::NonCanonicalReserved);
    };
    if read_u16(bytes, add(offset, 6)?)? != 0 {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(AccountRuleV2 {
        privileges: byte(bytes, offset)?,
        effect_permissions: byte(bytes, add(offset, 1)?)?,
        alias_kind,
        prestate,
        alias_index: read_u16(bytes, add(offset, 4)?)?,
        data_length: read_u32(bytes, add(offset, 8)?)?,
        data_item_stride: read_u32(bytes, add(offset, 12)?)?,
    })
}

fn exact_rule_data_length(rule: AccountRuleV2, tail_count: u32) -> Result<usize> {
    let width = u64::from(rule.data_item_stride)
        .checked_mul(u64::from(tail_count))
        .and_then(|tail| u64::from(rule.data_length).checked_add(tail))
        .ok_or(Error::DataLengthMismatch)?;
    usize::try_from(width).map_err(|_| Error::DataLengthMismatch)
}

fn physical_data_geometry(
    rule: AccountRuleV2,
    tail_count: u32,
) -> Result<PhysicalAccountDataGeometryV2> {
    let bytes = exact_rule_data_length(rule, tail_count)?;
    match rule.prestate {
        AccountPrestateV2::Exact => Ok(PhysicalAccountDataGeometryV2::Exact { bytes }),
        AccountPrestateV2::LifecycleBound => {
            Ok(PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: bytes })
        }
        AccountPrestateV2::AdapterAuthenticatedVariableData => Ok(
            PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable {
                minimum_bytes: bytes,
            },
        ),
        AccountPrestateV2::AuthenticatedOpaqueReadonlyData => {
            Ok(PhysicalAccountDataGeometryV2::Opaque)
        }
        AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
        | AccountPrestateV2::AuthenticatedRouteAlias => Err(Error::InvalidAlias),
    }
}

fn projected_data_field(
    data: &[u8],
    rule: AccountRuleV2,
    offset: u32,
    width: usize,
) -> Result<Option<&[u8]>> {
    if rule.prestate == AccountPrestateV2::LifecycleBound && data.is_empty() {
        let offset = usize::try_from(offset).map_err(|_| Error::DataOutOfBounds)?;
        if offset
            .checked_add(width)
            .is_none_or(|end| end > usize::try_from(rule.data_length).unwrap_or(0))
        {
            Err(Error::DataOutOfBounds)
        } else {
            Ok(None)
        }
    } else {
        data_field(data, offset, width).map(Some)
    }
}

fn expanded_rule(profile: AccountProfileV2<'_>, coordinate: usize) -> Result<AccountRuleV2> {
    let fixed = usize::from(profile.fixed_accounts);
    if coordinate < fixed {
        profile.rule(
            false,
            u16::try_from(coordinate).map_err(|_| Error::InvalidCoordinate)?,
        )
    } else {
        let stride = usize::from(profile.item_account_stride);
        if stride == 0 {
            return Err(Error::InvalidCoordinate);
        }
        let local = coordinate
            .checked_sub(fixed)
            .ok_or(Error::InvalidCoordinate)?
            % stride;
        profile.rule(
            true,
            u16::try_from(local).map_err(|_| Error::InvalidCoordinate)?,
        )
    }
}

#[derive(Clone, Copy)]
struct DynamicRuleLocationV2 {
    rule: AccountRuleV2,
    item_start: Option<usize>,
}

fn expanded_rule_with_dynamic_spans(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    coordinate: usize,
) -> Result<AccountRuleV2> {
    Ok(dynamic_rule_location(profile, tail_count, span_counts, coordinate)?.rule)
}

fn require_dynamic_span_counts(profile: AccountProfileV2<'_>, span_counts: &[u32]) -> Result<()> {
    if !profile.uses_dynamic_fixed_spans()
        || span_counts.len() != usize::from(profile.dynamic_fixed_span_count)
    {
        return Err(Error::InvalidDynamicSpan);
    }
    let mut index = 0_u16;
    while index < profile.dynamic_fixed_span_count {
        profile.dynamic_fixed_span(index)?.validate_count(
            *span_counts
                .get(usize::from(index))
                .ok_or(Error::InvalidDynamicSpan)?,
        )?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

fn dynamic_rule_location(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    coordinate: usize,
) -> Result<DynamicRuleLocationV2> {
    require_dynamic_span_counts(profile, span_counts)?;
    if coordinate >= dynamic_account_width(profile, tail_count, span_counts)? {
        return Err(Error::InvalidCoordinate);
    }
    let mut base_cursor = 0_usize;
    let mut runtime_cursor = 0_usize;
    let mut index = 0_u16;
    while index < profile.dynamic_fixed_span_count {
        let span = profile.dynamic_fixed_span(index)?;
        let insertion = usize::from(span.insertion_coordinate);
        let fixed_width = insertion
            .checked_sub(base_cursor)
            .ok_or(Error::InvalidDynamicSpan)?;
        let fixed_end = runtime_cursor
            .checked_add(fixed_width)
            .ok_or(Error::InvalidDynamicSpan)?;
        if coordinate < fixed_end {
            let base = base_cursor
                .checked_add(
                    coordinate
                        .checked_sub(runtime_cursor)
                        .ok_or(Error::InvalidCoordinate)?,
                )
                .ok_or(Error::InvalidCoordinate)?;
            return Ok(DynamicRuleLocationV2 {
                rule: profile.rule(
                    false,
                    u16::try_from(base).map_err(|_| Error::InvalidCoordinate)?,
                )?,
                item_start: None,
            });
        }
        runtime_cursor = fixed_end;
        base_cursor = insertion;
        let count = *span_counts
            .get(usize::from(index))
            .ok_or(Error::InvalidDynamicSpan)?;
        let stride = usize::from(span.rule_stride);
        let span_width = usize::try_from(count).map_err(|_| Error::InvalidDynamicSpan)?;
        let span_end = runtime_cursor
            .checked_add(span_width)
            .ok_or(Error::InvalidDynamicSpan)?;
        if coordinate < span_end {
            let relative = coordinate
                .checked_sub(runtime_cursor)
                .ok_or(Error::InvalidCoordinate)?;
            let item_start = runtime_cursor
                .checked_add(
                    (relative / stride)
                        .checked_mul(stride)
                        .ok_or(Error::InvalidCoordinate)?,
                )
                .ok_or(Error::InvalidCoordinate)?;
            let local = u16::try_from(relative % stride).map_err(|_| Error::InvalidCoordinate)?;
            let template = span
                .rule_start
                .checked_add(local)
                .ok_or(Error::InvalidCoordinate)?;
            return Ok(DynamicRuleLocationV2 {
                rule: profile.rule(true, template)?,
                item_start: Some(item_start),
            });
        }
        runtime_cursor = span_end;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    let base = base_cursor
        .checked_add(
            coordinate
                .checked_sub(runtime_cursor)
                .ok_or(Error::InvalidCoordinate)?,
        )
        .ok_or(Error::InvalidCoordinate)?;
    Ok(DynamicRuleLocationV2 {
        rule: profile.rule(
            false,
            u16::try_from(base).map_err(|_| Error::InvalidCoordinate)?,
        )?,
        item_start: None,
    })
}

fn dynamic_runtime_coordinate_for_base(
    profile: AccountProfileV2<'_>,
    span_counts: &[u32],
    base_coordinate: u16,
) -> Result<usize> {
    require_dynamic_span_counts(profile, span_counts)?;
    if base_coordinate >= profile.fixed_accounts {
        return Err(Error::InvalidCoordinate);
    }
    let mut runtime = usize::from(base_coordinate);
    let mut index = 0_u16;
    while index < profile.dynamic_fixed_span_count {
        let span = profile.dynamic_fixed_span(index)?;
        if span.insertion_coordinate <= base_coordinate {
            let count = *span_counts
                .get(usize::from(index))
                .ok_or(Error::InvalidDynamicSpan)?;
            runtime = runtime
                .checked_add(usize::try_from(count).map_err(|_| Error::InvalidDynamicSpan)?)
                .ok_or(Error::InvalidDynamicSpan)?;
        }
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(runtime)
}

fn dynamic_representative(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    coordinate: usize,
) -> Result<usize> {
    let location = dynamic_rule_location(profile, tail_count, span_counts, coordinate)?;
    match location.rule.alias_kind {
        AliasKindV2::SelfCoordinate => Ok(coordinate),
        AliasKindV2::Fixed => {
            dynamic_runtime_coordinate_for_base(profile, span_counts, location.rule.alias_index)
        }
        AliasKindV2::SameItem => {
            let item_start = location.item_start.ok_or(Error::InvalidAlias)?;
            item_start
                .checked_add(usize::from(location.rule.alias_index))
                .ok_or(Error::InvalidCoordinate)
        }
    }
}

fn representative_from_rule(
    rule: AccountRuleV2,
    coordinate: usize,
    item_start: usize,
) -> Result<usize> {
    match rule.alias_kind {
        AliasKindV2::SelfCoordinate => Ok(coordinate),
        AliasKindV2::Fixed => Ok(usize::from(rule.alias_index)),
        AliasKindV2::SameItem => item_start
            .checked_add(usize::from(rule.alias_index))
            .ok_or(Error::InvalidCoordinate),
    }
}

fn account_width(profile: AccountProfileV2<'_>, count: u32) -> Result<usize> {
    if profile.dynamic_fixed_span_count != 0 {
        return Err(Error::InvalidDynamicSpan);
    }
    affine_width(profile.fixed_accounts, profile.item_account_stride, count)
}

fn dynamic_account_width(
    profile: AccountProfileV2<'_>,
    _tail_count: u32,
    span_counts: &[u32],
) -> Result<usize> {
    require_dynamic_span_counts(profile, span_counts)?;
    let mut width = usize::from(profile.fixed_accounts);
    let mut index = 0_u16;
    while index < profile.dynamic_fixed_span_count {
        let count = *span_counts
            .get(usize::from(index))
            .ok_or(Error::InvalidDynamicSpan)?;
        width = width
            .checked_add(usize::try_from(count).map_err(|_| Error::InvalidDynamicSpan)?)
            .ok_or(Error::InvalidDynamicSpan)?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(width)
}

fn affine_width(common: u16, stride: u16, count: u32) -> Result<usize> {
    let width = u64::from(stride)
        .checked_mul(u64::from(count))
        .and_then(|tail| u64::from(common).checked_add(tail))
        .ok_or(Error::InvalidLength)?;
    usize::try_from(width).map_err(|_| Error::InvalidLength)
}

fn item_account_index(profile: AccountProfileV2<'_>, item: u32, local: u16) -> Result<usize> {
    register_index(
        profile.fixed_accounts,
        profile.item_account_stride,
        true,
        Some(item),
        local,
    )
}

fn item_register_index(common: u16, stride: u16, item: u32, local: u16) -> Result<usize> {
    register_index(common, stride, true, Some(item), local)
}

fn register_index(
    common: u16,
    stride: u16,
    item_space: bool,
    item: Option<u32>,
    local: u16,
) -> Result<usize> {
    if !item_space {
        return if local < common {
            Ok(usize::from(local))
        } else {
            Err(Error::InvalidCoordinate)
        };
    }
    if local >= stride {
        return Err(Error::InvalidCoordinate);
    }
    let item = item.ok_or(Error::InvalidCoordinate)?;
    let index = u64::from(item)
        .checked_mul(u64::from(stride))
        .and_then(|offset| offset.checked_add(u64::from(common)))
        .and_then(|offset| offset.checked_add(u64::from(local)))
        .ok_or(Error::InvalidCoordinate)?;
    usize::try_from(index).map_err(|_| Error::InvalidCoordinate)
}

fn data_field(data: &[u8], offset: u32, width: usize) -> Result<&[u8]> {
    let start = usize::try_from(offset).map_err(|_| Error::DataOutOfBounds)?;
    let end = start.checked_add(width).ok_or(Error::DataOutOfBounds)?;
    data.get(start..end).ok_or(Error::DataOutOfBounds)
}

fn affine_data_offset(base: u32, stride: u32, item: u32, width: u32) -> Result<u32> {
    if stride < width {
        return Err(Error::NonCanonicalOperation);
    }
    base.checked_add(item.checked_mul(stride).ok_or(Error::DataOutOfBounds)?)
        .and_then(|start| start.checked_add(width).map(|_| start))
        .ok_or(Error::DataOutOfBounds)
}

fn write_scalar(values: &mut [u64], index: usize, value: u64) -> Result<()> {
    *values.get_mut(index).ok_or(Error::InvalidCoordinate)? = value;
    Ok(())
}

fn write_identity(values: &mut [[u8; 32]], index: usize, value: [u8; 32]) -> Result<()> {
    *values.get_mut(index).ok_or(Error::InvalidCoordinate)? = value;
    Ok(())
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::IdentityMismatch)
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = offset.checked_add(2).ok_or(Error::InvalidLength)?;
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::InvalidLength)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).ok_or(Error::InvalidLength)?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::InvalidLength)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;
    use std::vec::Vec;

    use super::*;

    fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
        let end = offset.checked_add(bytes.len()).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture slice")
            .copy_from_slice(bytes);
    }

    const PRODUCT_COUNT: [u8; 4] = 2_u32.to_le_bytes();

    fn rule(data_length: u32) -> [u8; RULE_BYTES] {
        let mut output = [0_u8; RULE_BYTES];
        put(&mut output, 8, &data_length.to_le_bytes());
        output
    }

    fn affine_rule(data_length: u32, data_item_stride: u32) -> [u8; RULE_BYTES] {
        let mut output = rule(data_length);
        put(&mut output, 12, &data_item_stride.to_le_bytes());
        output
    }

    fn operation(
        opcode: u8,
        account_item: bool,
        account: u16,
        register_item: bool,
        register: u16,
    ) -> [u8; OPERATION_BYTES] {
        let mut output = [0_u8; OPERATION_BYTES];
        *output.get_mut(0).expect("opcode") = opcode;
        *output.get_mut(1).expect("account space") = u8::from(account_item);
        put(&mut output, 2, &account.to_le_bytes());
        *output.get_mut(4).expect("register space") = u8::from(register_item);
        put(&mut output, 6, &register.to_le_bytes());
        output
    }

    fn affine_operation(
        opcode: u8,
        register: u16,
        data_offset: u32,
        data_stride: u32,
    ) -> [u8; OPERATION_BYTES] {
        let mut output = operation(opcode, false, 0, true, register);
        put(&mut output, 8, &data_offset.to_le_bytes());
        put(&mut output, 12, &data_stride.to_le_bytes());
        output
    }

    #[allow(clippy::too_many_arguments)]
    fn data_operation(
        opcode: u8,
        account: u16,
        register_item: bool,
        register: u16,
        data_offset: u32,
        data_stride: u32,
    ) -> [u8; OPERATION_BYTES] {
        let mut output = operation(opcode, false, account, register_item, register);
        put(&mut output, 8, &data_offset.to_le_bytes());
        put(&mut output, 12, &data_stride.to_le_bytes());
        output
    }

    fn profile_bytes() -> Vec<u8> {
        let rules = [rule(4), rule(0), rule(0)];
        let operations = [
            operation(OP_REQUIRE_KEY, false, 0, false, 0),
            operation(OP_PROJECT_TAIL_COUNT_U32, false, 0, false, 0),
            operation(OP_PROJECT_KEY, true, 0, true, 0),
            operation(OP_PROJECT_LAMPORTS, true, 1, true, 1),
        ];
        let mut output =
            vec![
                0_u8;
                HEADER_BYTES + rules.len() * RULE_BYTES + operations.len() * OPERATION_BYTES
            ];
        put(&mut output, 0, &MAGIC);
        for (offset, value) in [
            (8, VERSION),
            (10, ARTIFACT_PROFILE),
            (12, 1),
            (14, 2),
            (16, 2),
            (18, 2),
            (20, 1),
            (22, 2),
            (24, 1),
            (26, 1),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        for (index, value) in rules.iter().enumerate() {
            put(&mut output, HEADER_BYTES + index * RULE_BYTES, value);
        }
        let operations_start = HEADER_BYTES + rules.len() * RULE_BYTES;
        for (index, value) in operations.iter().enumerate() {
            put(
                &mut output,
                operations_start + index * OPERATION_BYTES,
                value,
            );
        }
        output
    }

    fn affine_profile_bytes() -> Vec<u8> {
        let rules = [rule(84)];
        let operations = [
            operation(OP_REQUIRE_KEY, false, 0, false, 0),
            operation(OP_PROJECT_TAIL_COUNT_U32, false, 0, false, 0),
            affine_operation(OP_PROJECT_DATA_U64_AFFINE, 1, 4, 40),
            affine_operation(OP_PROJECT_DATA_IDENTITY_AFFINE, 0, 12, 40),
        ];
        let mut output =
            vec![
                0_u8;
                HEADER_BYTES + rules.len() * RULE_BYTES + operations.len() * OPERATION_BYTES
            ];
        put(&mut output, 0, &MAGIC);
        for (offset, value) in [
            (8, VERSION),
            (10, ARTIFACT_PROFILE),
            (12, 1),
            (14, 0),
            (16, 2),
            (18, 2),
            (20, 1),
            (22, 2),
            (24, 1),
            (26, 1),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        put(&mut output, HEADER_BYTES, &rules[0]);
        let operations_start = HEADER_BYTES + RULE_BYTES;
        for (index, value) in operations.iter().enumerate() {
            put(
                &mut output,
                operations_start + index * OPERATION_BYTES,
                value,
            );
        }
        output
    }

    fn affine_data(count: u32) -> [u8; 84] {
        let mut data = [0_u8; 84];
        put(&mut data, 0, &count.to_le_bytes());
        put(&mut data, 4, &11_u64.to_le_bytes());
        put(&mut data, 12, &[0x31; 32]);
        put(&mut data, 44, &22_u64.to_le_bytes());
        put(&mut data, 52, &[0x32; 32]);
        data
    }

    fn selected_window_profile_bytes() -> Vec<u8> {
        let rules = [rule(4), rule(4), affine_rule(104, 16)];
        let operations = [
            operation(OP_REQUIRE_KEY, false, 0, false, 0),
            operation(OP_PROJECT_TAIL_COUNT_U32, false, 0, false, 0),
            data_operation(OP_PROJECT_DATA_U32, 1, false, 1, 0, 0),
            data_operation(OP_SELECT_DATA_WINDOW, 2, false, 1, 8, 48),
            data_operation(OP_PROJECT_DATA_IDENTITY_SELECTED, 2, false, 1, 0, 0),
            data_operation(OP_PROJECT_DATA_U64_SELECTED, 2, false, 2, 32, 0),
            data_operation(OP_PROJECT_DATA_U64_SELECTED_AFFINE, 2, true, 1, 48, 8),
        ];
        let mut output =
            vec![
                0_u8;
                HEADER_BYTES + rules.len() * RULE_BYTES + operations.len() * OPERATION_BYTES
            ];
        put(&mut output, 0, &MAGIC);
        for (offset, value) in [
            (8, VERSION),
            (10, SELECTED_WINDOW_ARTIFACT_PROFILE),
            (12, 3),
            (14, 0),
            (16, 6),
            (18, 1),
            (20, 3),
            (22, 2),
            (24, 2),
            (26, 0),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        for (index, value) in rules.iter().enumerate() {
            put(&mut output, HEADER_BYTES + index * RULE_BYTES, value);
        }
        let operations_start = HEADER_BYTES + rules.len() * RULE_BYTES;
        for (index, value) in operations.iter().enumerate() {
            put(
                &mut output,
                operations_start + index * OPERATION_BYTES,
                value,
            );
        }
        output
    }

    fn selected_window_data() -> [u8; 136] {
        let mut data = [0_u8; 136];
        put(&mut data, 8, &[0x41; 32]);
        put(&mut data, 40, &66_u64.to_le_bytes());
        put(&mut data, 56, &1_u64.to_le_bytes());
        put(&mut data, 64, &2_u64.to_le_bytes());
        put(&mut data, 72, &[0x42; 32]);
        put(&mut data, 104, &77_u64.to_le_bytes());
        put(&mut data, 120, &11_u64.to_le_bytes());
        put(&mut data, 128, &22_u64.to_le_bytes());
        data
    }

    fn typed_u16_profile_bytes(artifact_profile: u16) -> Vec<u8> {
        let mut encoded_operation = [0_u8; OPERATION_BYTES];
        encode_project_data_u16_operation_v2(&mut encoded_operation, 0, 0, 1)
            .expect("encode u16 projection");
        let mut output = vec![0_u8; HEADER_BYTES + RULE_BYTES + OPERATION_BYTES];
        put(&mut output, 0, &MAGIC);
        for (offset, value) in [
            (8, VERSION),
            (10, artifact_profile),
            (12, 1),
            (14, 0),
            (16, 1),
            (18, 0),
            (20, 1),
            (22, 0),
            (24, 0),
            (26, 0),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        put(&mut output, HEADER_BYTES, &rule(4));
        put(&mut output, HEADER_BYTES + RULE_BYTES, &encoded_operation);
        output
    }

    fn fixed_tail_count_profile_bytes(destinations: &[u16]) -> Vec<u8> {
        let mut output =
            vec![0_u8; HEADER_BYTES + RULE_BYTES + destinations.len() * OPERATION_BYTES];
        put(&mut output, 0, &MAGIC);
        for (offset, value) in [
            (8, VERSION),
            (10, ARTIFACT_PROFILE),
            (12, 1),
            (14, 0),
            (
                16,
                u16::try_from(destinations.len()).expect("operation count"),
            ),
            (18, 0),
            (20, 2),
            (22, 0),
            (24, 0),
            (26, 0),
        ] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        put(&mut output, HEADER_BYTES, &rule(4));
        for (index, destination) in destinations.iter().copied().enumerate() {
            put(
                &mut output,
                HEADER_BYTES + RULE_BYTES + index * OPERATION_BYTES,
                &operation(OP_PROJECT_TAIL_COUNT_U32, false, 0, false, destination),
            );
        }
        output
    }

    fn observations(duplicate_across_items: bool) -> Vec<AccountObservationV1<'static>> {
        let second = if duplicate_across_items {
            [0x21; 32]
        } else {
            [0x31; 32]
        };
        vec![
            AccountObservationV1::new([0x11; 32], [1; 32], 1, &PRODUCT_COUNT, false, false, false),
            AccountObservationV1::new([0x21; 32], [1; 32], 2, &[], false, false, false),
            AccountObservationV1::new([0x22; 32], [1; 32], 3, &[], false, false, false),
            AccountObservationV1::new(second, [1; 32], 4, &[], false, false, false),
            AccountObservationV1::new([0x32; 32], [1; 32], 5, &[], false, false, false),
        ]
    }

    #[test]
    fn authenticated_tail_projects_indices_keys_and_lamports() {
        let bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        let accounts = observations(false);
        let input_scalars = [0_u64; 5];
        let input_identities = [[0x11_u8; 32], [0; 32], [0; 32]];
        let mut scratch_scalars = [0_u64; 5];
        let mut scratch_identities = [[0_u8; 32]; 3];
        let mut output_scalars = [9_u64; 5];
        let mut output_identities = [[9_u8; 32]; 3];
        project_atomic(
            profile,
            2,
            &accounts,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("projection");
        assert_eq!(output_scalars, [2, 0, 3, 1, 5]);
        assert_eq!(output_identities, [[0x11; 32], [0x21; 32], [0x31; 32]]);

        let mut prefix_scratch_scalars = [0_u64; 1];
        let mut prefix_scratch_identities = [[0_u8; 32]; 1];
        let mut prefix_output_scalars = [9_u64; 1];
        let mut prefix_output_identities = [[9_u8; 32]; 1];
        assert_eq!(
            project_tail_count_atomic(
                profile,
                accounts.get(..1).expect("fixed prefix"),
                ProjectionRegistersV2 {
                    input_scalars: &[0],
                    input_identities: &[[0x11; 32]],
                    scratch_scalars: &mut prefix_scratch_scalars,
                    scratch_identities: &mut prefix_scratch_identities,
                    output_scalars: &mut prefix_output_scalars,
                    output_identities: &mut prefix_output_identities,
                },
            ),
            Ok(2)
        );
    }

    #[test]
    fn typed_u16_projection_zero_extends_and_is_profile_gated_atomically() {
        let bytes = typed_u16_profile_bytes(TYPED_SCALAR_ARTIFACT_PROFILE);
        let profile = AccountProfileV2::decode(&bytes).expect("typed profile");
        let data = [0x55, 0x34, 0x12, 0xaa];
        let accounts = [AccountObservationV1::new(
            [1; 32], [2; 32], 0, &data, false, false, false,
        )];
        let mut scratch_scalars = [0_u64];
        let mut output_scalars = [9_u64];
        project_atomic(
            profile,
            0,
            &accounts,
            ProjectionRegistersV2 {
                input_scalars: &[0],
                input_identities: &[],
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut [],
                output_scalars: &mut output_scalars,
                output_identities: &mut [],
            },
        )
        .expect("typed projection");
        assert_eq!(output_scalars, [0x1234]);

        assert_eq!(
            AccountProfileV2::decode(&typed_u16_profile_bytes(SELECTED_WINDOW_ARTIFACT_PROFILE)),
            Err(Error::NonCanonicalOperation)
        );

        let mut short = [7_u8; OPERATION_BYTES - 1];
        let before = short;
        assert_eq!(
            encode_project_data_u16_operation_v2(&mut short, 0, 0, 0),
            Err(Error::InvalidLength)
        );
        assert_eq!(short, before);

        let mut overflow = [7_u8; OPERATION_BYTES];
        let before = overflow;
        assert_eq!(
            encode_project_data_u16_operation_v2(&mut overflow, 0, 0, u32::MAX),
            Err(Error::InvalidLength)
        );
        assert_eq!(overflow, before);
    }

    #[test]
    fn fixed_geometry_may_project_product_tail_count_without_fake_item_state() {
        let bytes = fixed_tail_count_profile_bytes(&[0]);
        let profile = AccountProfileV2::decode(&bytes).expect("fixed sparse profile");
        assert_eq!(profile.item_account_stride(), 0);
        assert_eq!(profile.item_scalar_stride(), 0);
        assert_eq!(profile.item_identity_stride(), 0);
        assert_eq!(
            profile
                .tail_count_projection()
                .expect("tail projection")
                .expect("present")
                .register(),
            0
        );

        let product_count = 513_u32.to_le_bytes();
        let accounts = [AccountObservationV1::new(
            [1; 32],
            [2; 32],
            0,
            &product_count,
            false,
            false,
            false,
        )];
        let mut scratch_scalars = [0_u64; 2];
        let mut output_scalars = [9_u64; 2];
        assert_eq!(
            project_tail_count_atomic(
                profile,
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &[0; 2],
                    input_identities: &[],
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut [],
                    output_scalars: &mut output_scalars,
                    output_identities: &mut [],
                },
            ),
            Ok(513)
        );
        assert_eq!(output_scalars, [513, 0]);
    }

    #[test]
    fn tail_count_remains_unique_fixed_account_common_scalar_and_affine_required() {
        assert_eq!(
            AccountProfileV2::decode(&fixed_tail_count_profile_bytes(&[0, 1])),
            Err(Error::DuplicateProjection)
        );

        let mut item_space = profile_bytes();
        let operations_start = HEADER_BYTES + 3 * RULE_BYTES;
        *item_space
            .get_mut(operations_start + OPERATION_BYTES + 1)
            .expect("tail account space") = 1;
        assert_eq!(
            AccountProfileV2::decode(&item_space),
            Err(Error::NonCanonicalOperation)
        );

        let mut item_register = profile_bytes();
        *item_register
            .get_mut(operations_start + OPERATION_BYTES + 4)
            .expect("tail register space") = 1;
        assert_eq!(
            AccountProfileV2::decode(&item_register),
            Err(Error::NonCanonicalOperation)
        );

        let mut missing_affine_projection = profile_bytes();
        *missing_affine_projection
            .get_mut(operations_start + OPERATION_BYTES)
            .expect("tail opcode") = OP_PROJECT_DATA_U32;
        assert_eq!(
            AccountProfileV2::decode(&missing_affine_projection),
            Err(Error::NonCanonicalOperation)
        );
    }

    #[test]
    fn cross_item_alias_and_width_refuse_without_output_commit() {
        let bytes = profile_bytes();
        let profile = AccountProfileV2::decode(&bytes).expect("profile");
        for accounts in [observations(true), observations(false)] {
            let input_scalars = [0_u64; 5];
            let input_identities = [[0x11_u8; 32], [0; 32], [0; 32]];
            let mut scratch_scalars = [0_u64; 5];
            let mut scratch_identities = [[0_u8; 32]; 3];
            let mut output_scalars = [9_u64; 5];
            let mut output_identities = [[9_u8; 32]; 3];
            let before_scalars = output_scalars;
            let before_identities = output_identities;
            let used =
                if accounts.get(1).expect("item").key() == accounts.get(3).expect("item").key() {
                    accounts.as_slice()
                } else {
                    accounts.get(..4).expect("short accounts")
                };
            assert!(
                project_atomic(
                    profile,
                    2,
                    used,
                    ProjectionRegistersV2 {
                        input_scalars: &input_scalars,
                        input_identities: &input_identities,
                        scratch_scalars: &mut scratch_scalars,
                        scratch_identities: &mut scratch_identities,
                        output_scalars: &mut output_scalars,
                        output_identities: &mut output_identities,
                    }
                )
                .is_err()
            );
            assert_eq!(output_scalars, before_scalars);
            assert_eq!(output_identities, before_identities);
        }
    }

    #[test]
    fn hostile_header_rule_and_operation_refuse() {
        let canonical = profile_bytes();
        for (offset, expected) in [
            (0_usize, Error::InvalidMagic),
            (8, Error::UnsupportedProfile),
            (28, Error::NonCanonicalReserved),
            (HEADER_BYTES + 3, Error::NonCanonicalReserved),
            (
                HEADER_BYTES + 3 * RULE_BYTES + 5,
                Error::NonCanonicalOperation,
            ),
        ] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert_eq!(AccountProfileV2::decode(&hostile), Err(expected));
        }
    }

    #[test]
    fn compact_fixed_account_projects_runtime_affine_tail_atomically() {
        let bytes = affine_profile_bytes();
        let profile = AccountProfileV2::decode(&bytes).expect("affine profile");
        let data = affine_data(2);
        let accounts = [AccountObservationV1::new(
            [0x11; 32], [1; 32], 1, &data, false, false, false,
        )];
        let input_scalars = [0_u64; 5];
        let input_identities = [[0x11_u8; 32], [0; 32], [0; 32]];
        let mut scratch_scalars = [0_u64; 5];
        let mut scratch_identities = [[0_u8; 32]; 3];
        let mut output_scalars = [9_u64; 5];
        let mut output_identities = [[9_u8; 32]; 3];
        project_atomic(
            profile,
            2,
            &accounts,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("affine projection");
        assert_eq!(output_scalars, [2, 0, 11, 1, 22]);
        assert_eq!(output_identities, [[0x11; 32], [0x31; 32], [0x32; 32]]);

        let hostile_data = affine_data(3);
        let hostile_accounts = [AccountObservationV1::new(
            [0x11; 32],
            [1; 32],
            1,
            &hostile_data,
            false,
            false,
            false,
        )];
        let mut hostile_scratch_scalars = [0_u64; 7];
        let mut hostile_scratch_identities = [[0_u8; 32]; 4];
        let mut hostile_output_scalars = [9_u64; 7];
        let mut hostile_output_identities = [[9_u8; 32]; 4];
        let before_scalars = hostile_output_scalars;
        let before_identities = hostile_output_identities;
        assert_eq!(
            project_atomic(
                profile,
                3,
                &hostile_accounts,
                ProjectionRegistersV2 {
                    input_scalars: &[0; 7],
                    input_identities: &[[0x11; 32], [0; 32], [0; 32], [0; 32]],
                    scratch_scalars: &mut hostile_scratch_scalars,
                    scratch_identities: &mut hostile_scratch_identities,
                    output_scalars: &mut hostile_output_scalars,
                    output_identities: &mut hostile_output_identities,
                },
            ),
            Err(Error::DataOutOfBounds)
        );
        assert_eq!(hostile_output_scalars, before_scalars);
        assert_eq!(hostile_output_identities, before_identities);

        let operations_start = HEADER_BYTES + RULE_BYTES;
        for stride in [0_u32, 7] {
            let mut hostile = bytes.clone();
            put(
                &mut hostile,
                operations_start + 2 * OPERATION_BYTES + 12,
                &stride.to_le_bytes(),
            );
            assert_eq!(
                AccountProfileV2::decode(&hostile),
                Err(Error::NonCanonicalOperation)
            );
        }
    }

    #[test]
    fn authenticated_selector_projects_one_runtime_width_row() {
        let bytes = selected_window_profile_bytes();
        let profile = AccountProfileV2::decode(&bytes).expect("selected profile");
        assert_eq!(profile.artifact_profile(), SELECTED_WINDOW_ARTIFACT_PROFILE);
        let selector = 1_u32.to_le_bytes();
        let page = selected_window_data();
        let accounts = [
            AccountObservationV1::new([0x11; 32], [1; 32], 1, &PRODUCT_COUNT, false, false, false),
            AccountObservationV1::new([0x12; 32], [1; 32], 1, &selector, false, false, false),
            AccountObservationV1::new([0x13; 32], [1; 32], 1, &page, false, false, false),
        ];
        let input_scalars = [0_u64; 7];
        let input_identities = [[0x11_u8; 32], [0; 32]];
        let mut scratch_scalars = [0_u64; 7];
        let mut scratch_identities = [[0_u8; 32]; 2];
        let mut output_scalars = [9_u64; 7];
        let mut output_identities = [[9_u8; 32]; 2];
        project_atomic(
            profile,
            2,
            &accounts,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("selected projection");
        assert_eq!(output_scalars, [2, 1, 77, 0, 11, 1, 22]);
        assert_eq!(output_identities, [[0x11; 32], [0x42; 32]]);
    }

    #[test]
    fn selected_window_refuses_unbound_geometry_and_selector_atomically() {
        let canonical = selected_window_profile_bytes();
        let rules_start = HEADER_BYTES;
        let operations_start = HEADER_BYTES + 3 * RULE_BYTES;
        for (offset, value) in [
            (rules_start + 2 * RULE_BYTES + 12, 15_u32),
            (operations_start + 3 * OPERATION_BYTES + 12, 47),
            (operations_start + 6 * OPERATION_BYTES + 12, 7),
        ] {
            let mut hostile = canonical.clone();
            put(&mut hostile, offset, &value.to_le_bytes());
            assert_eq!(
                AccountProfileV2::decode(&hostile),
                Err(Error::NonCanonicalOperation)
            );
        }

        let mut legacy_with_affine_length = canonical.clone();
        put(
            &mut legacy_with_affine_length,
            10,
            &ARTIFACT_PROFILE.to_le_bytes(),
        );
        assert_eq!(
            AccountProfileV2::decode(&legacy_with_affine_length),
            Err(Error::NonCanonicalReserved)
        );

        let profile = AccountProfileV2::decode(&canonical).expect("profile");
        let hostile_selector = 2_u32.to_le_bytes();
        let page = selected_window_data();
        let accounts = [
            AccountObservationV1::new([0x11; 32], [1; 32], 1, &PRODUCT_COUNT, false, false, false),
            AccountObservationV1::new(
                [0x12; 32],
                [1; 32],
                1,
                &hostile_selector,
                false,
                false,
                false,
            ),
            AccountObservationV1::new([0x13; 32], [1; 32], 1, &page, false, false, false),
        ];
        let mut scratch_scalars = [0_u64; 7];
        let mut scratch_identities = [[0_u8; 32]; 2];
        let mut output_scalars = [9_u64; 7];
        let mut output_identities = [[9_u8; 32]; 2];
        let before_scalars = output_scalars;
        let before_identities = output_identities;
        assert_eq!(
            project_atomic(
                profile,
                2,
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &[0; 7],
                    input_identities: &[[0x11; 32], [0; 32]],
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            ),
            Err(Error::DataOutOfBounds)
        );
        assert_eq!(output_scalars, before_scalars);
        assert_eq!(output_identities, before_identities);
    }
}
