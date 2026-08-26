//! Safe, allocation-free AccountProfile V2 artifact encoder.
//!
//! Public typed inputs retain account-space, register-space, alias, privilege,
//! permission, and operation-tag authority in this semantic-owner crate. The
//! encoder hostile-decodes the complete scratch candidate before copying it to
//! output.

use super::generated_profile14::{
    FIXED_DATA_PREDICATE_ARTIFACT_PROFILE, FIXED_DATA_PREDICATE_BYTES,
    FIXED_DATA_PREDICATE_COUNT_OFFSET, FIXED_DATA_PREDICATE_HEADER_BYTES,
    FIXED_DATA_PREDICATE_REQUIRE_U8, FIXED_DATA_PREDICATE_REQUIRE_U16,
    FIXED_DATA_PREDICATE_REQUIRE_U32, FIXED_DATA_PREDICATE_REQUIRE_U64,
    FIXED_DATA_PREDICATE_REQUIRE_ZERO_RANGE,
};
use super::{
    ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE,
    ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE, ARTIFACT_PROFILE,
    AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE, AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES,
    AccountPrestateV2, AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
    DYNAMIC_FIXED_SPAN_COUNT_OFFSET, DYNAMIC_FIXED_SPAN_ENTRY_BYTES,
    DYNAMIC_FIXED_SPAN_ENTRY_COUNT_SCALAR_OFFSET, DYNAMIC_FIXED_SPAN_ENTRY_INSERTION_OFFSET,
    DYNAMIC_FIXED_SPAN_ENTRY_MAX_OFFSET, DYNAMIC_FIXED_SPAN_ENTRY_MIN_OFFSET,
    DYNAMIC_FIXED_SPAN_ENTRY_RULE_START_OFFSET, DYNAMIC_FIXED_SPAN_ENTRY_RULE_STRIDE_OFFSET,
    DYNAMIC_FIXED_SPAN_ENTRY_STEP_OFFSET, DYNAMIC_FIXED_SPAN_HEADER_BYTES, Error, HEADER_BYTES,
    LIFECYCLE_PRESTATE_ARTIFACT_PROFILE, MAGIC, NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE,
    NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE, OP_PROJECT_DATA_IDENTITY,
    OP_PROJECT_DATA_IDENTITY_AFFINE, OP_PROJECT_DATA_IDENTITY_SELECTED,
    OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE, OP_PROJECT_DATA_U8, OP_PROJECT_DATA_U16,
    OP_PROJECT_DATA_U32, OP_PROJECT_DATA_U64, OP_PROJECT_DATA_U64_AFFINE,
    OP_PROJECT_DATA_U64_SELECTED, OP_PROJECT_DATA_U64_SELECTED_AFFINE, OP_PROJECT_KEY,
    OP_PROJECT_LAMPORTS, OP_PROJECT_NONZERO_U64_TAIL_COUNT, OP_PROJECT_NONZERO_U64_TAIL_ROWS,
    OP_PROJECT_OWNER, OP_PROJECT_TAIL_COUNT_U32, OP_REQUIRE_KEY, OP_REQUIRE_OWNER,
    OP_SELECT_DATA_WINDOW, OPERATION_BYTES, RULE_BYTES, SELECTED_WINDOW_ARTIFACT_PROFILE,
    TRUSTED_BUILTIN_IDENTITY_OFFSET, TRUSTED_BUILTIN_KIND_OFFSET, TRUSTED_BUILTIN_SYSTEM_PROGRAM,
    TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE, TRUSTED_ENVIRONMENT_CURRENT_SLOT,
    TRUSTED_ENVIRONMENT_KIND_OFFSET, TRUSTED_ENVIRONMENT_SCALAR_OFFSET,
    TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE, TRUSTED_EXECUTING_PROGRAM_CURRENT,
    TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES, TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET,
    TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET, TYPED_SCALAR_ARTIFACT_PROFILE, TrustedBuiltinIdentityV2,
    TrustedEnvironmentV2, TrustedIdentityEnvironmentV2, VERSION,
};
use crate::{
    EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
    EFFECT_PERMISSION_WRITE_DATA,
};

/// Supported hostile-decoded AccountProfile V2 artifact semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountProfileArtifactV2 {
    /// Original runtime-tail profile.
    RuntimeTail,
    /// Runtime-tail profile with authenticated selected data windows.
    SelectedWindow,
    /// Selected-window profile with checked narrow integer projection.
    TypedScalar,
    /// Typed-scalar profile with an optional trusted runtime environment scalar.
    TrustedEnvironment,
    /// Trusted-environment profile with lifecycle-bound alternative prestates.
    LifecyclePrestate,
    /// Lifecycle-prestate profile with explicitly adapter-authenticated,
    /// variable-width readonly fixed-prefix records.
    AdapterAuthenticatedVariableData,
    /// Variable-data successor with optional trusted executing-program identity.
    TrustedExecutingProgram,
    /// Trusted-program successor with non-owning aliases of authenticated
    /// variable-data representatives.
    AdapterAuthenticatedVariableDataAlias,
    /// Variable-alias successor deriving a positive support width from an
    /// authenticated immutable `u64` tail.
    NonzeroU64TailCount,
    /// Physical-representative route aliases plus optional trusted builtin.
    AuthenticatedRouteAlias,
    /// Ordered sparse rows derived from one authenticated immutable `u64` tail.
    NonzeroU64TailRows,
    /// One checked account span inserted into a fixed logical sequence.
    DynamicFixedSpan,
    /// Dynamic-span/route-alias successor with canonical fixed-data predicates.
    FixedDataPredicate,
}

impl AccountProfileArtifactV2 {
    const fn value(self) -> u16 {
        match self {
            Self::RuntimeTail => ARTIFACT_PROFILE,
            Self::SelectedWindow => SELECTED_WINDOW_ARTIFACT_PROFILE,
            Self::TypedScalar => TYPED_SCALAR_ARTIFACT_PROFILE,
            Self::TrustedEnvironment => TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE,
            Self::LifecyclePrestate => LIFECYCLE_PRESTATE_ARTIFACT_PROFILE,
            Self::AdapterAuthenticatedVariableData => {
                ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
            }
            Self::TrustedExecutingProgram => TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE,
            Self::AdapterAuthenticatedVariableDataAlias => {
                ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
            }
            Self::NonzeroU64TailCount => NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE,
            Self::AuthenticatedRouteAlias => AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE,
            Self::NonzeroU64TailRows => NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE,
            Self::DynamicFixedSpan => DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
            Self::FixedDataPredicate => FIXED_DATA_PREDICATE_ARTIFACT_PROFILE,
        }
    }

    const fn header_bytes(self) -> usize {
        match self {
            Self::TrustedExecutingProgram
            | Self::AdapterAuthenticatedVariableDataAlias
            | Self::NonzeroU64TailCount
            | Self::NonzeroU64TailRows => TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES,
            Self::AuthenticatedRouteAlias => AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES,
            Self::DynamicFixedSpan => DYNAMIC_FIXED_SPAN_HEADER_BYTES,
            Self::FixedDataPredicate => FIXED_DATA_PREDICATE_HEADER_BYTES,
            _ => HEADER_BYTES,
        }
    }
}

/// Descriptor-owned dynamic account-span geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DynamicFixedSpanInputV2 {
    /// Base fixed-rule coordinate at which repeated span rules are inserted.
    pub insertion_coordinate: u16,
    /// Common scalar containing the authenticated runtime span count.
    pub count_scalar: u16,
    /// First span-rule template owned by this entry.
    pub rule_start: u16,
    /// Number of account rules repeated for each selected item.
    pub rule_stride: u16,
    /// Inclusive minimum admitted count.
    pub minimum: u32,
    /// Inclusive maximum admitted count.
    pub maximum: u32,
    /// Positive congruence step within the inclusive range.
    pub step: u32,
}

/// One canonical Profile 14 fixed-data prestate predicate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixedDataPredicateInputV2 {
    /// Require one exact byte.
    RequireDataU8 {
        /// Fixed-rule account coordinate.
        account: u16,
        /// Exact account-data offset.
        data_offset: u32,
        /// Required byte.
        value: u8,
    },
    /// Require one exact little-endian `u16`.
    RequireDataU16 {
        /// Fixed-rule account coordinate.
        account: u16,
        /// Exact account-data offset.
        data_offset: u32,
        /// Required integer.
        value: u16,
    },
    /// Require one exact little-endian `u32`.
    RequireDataU32 {
        /// Fixed-rule account coordinate.
        account: u16,
        /// Exact account-data offset.
        data_offset: u32,
        /// Required integer.
        value: u32,
    },
    /// Require one exact little-endian `u64`.
    RequireDataU64 {
        /// Fixed-rule account coordinate.
        account: u16,
        /// Exact account-data offset.
        data_offset: u32,
        /// Required integer.
        value: u64,
    },
    /// Require one nonempty range to contain only zero bytes.
    RequireZeroRange {
        /// Fixed-rule account coordinate.
        account: u16,
        /// Exact account-data offset.
        data_offset: u32,
        /// Positive range width.
        length: u32,
    },
}

impl FixedDataPredicateInputV2 {
    const fn parts(self) -> (u8, u16, u32, [u8; 8]) {
        match self {
            Self::RequireDataU8 {
                account,
                data_offset,
                value,
            } => {
                let mut payload = [0_u8; 8];
                payload[0] = value;
                (
                    FIXED_DATA_PREDICATE_REQUIRE_U8,
                    account,
                    data_offset,
                    payload,
                )
            }
            Self::RequireDataU16 {
                account,
                data_offset,
                value,
            } => {
                let mut payload = [0_u8; 8];
                let bytes = value.to_le_bytes();
                payload[0] = bytes[0];
                payload[1] = bytes[1];
                (
                    FIXED_DATA_PREDICATE_REQUIRE_U16,
                    account,
                    data_offset,
                    payload,
                )
            }
            Self::RequireDataU32 {
                account,
                data_offset,
                value,
            } => {
                let mut payload = [0_u8; 8];
                let bytes = value.to_le_bytes();
                payload[0] = bytes[0];
                payload[1] = bytes[1];
                payload[2] = bytes[2];
                payload[3] = bytes[3];
                (
                    FIXED_DATA_PREDICATE_REQUIRE_U32,
                    account,
                    data_offset,
                    payload,
                )
            }
            Self::RequireDataU64 {
                account,
                data_offset,
                value,
            } => (
                FIXED_DATA_PREDICATE_REQUIRE_U64,
                account,
                data_offset,
                value.to_le_bytes(),
            ),
            Self::RequireZeroRange {
                account,
                data_offset,
                length,
            } => {
                let mut payload = [0_u8; 8];
                let bytes = length.to_le_bytes();
                payload[0] = bytes[0];
                payload[1] = bytes[1];
                payload[2] = bytes[2];
                payload[3] = bytes[3];
                (
                    FIXED_DATA_PREDICATE_REQUIRE_ZERO_RANGE,
                    account,
                    data_offset,
                    payload,
                )
            }
        }
    }
}

/// Caller-declared register geometry owned by the encoded profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterGeometryV2 {
    /// Fixed/common scalar-bank width.
    pub common_scalars: u16,
    /// Per-Product-item scalar-bank stride.
    pub item_scalar_stride: u16,
    /// Fixed/common identity-bank width.
    pub common_identities: u16,
    /// Per-Product-item identity-bank stride.
    pub item_identity_stride: u16,
}

/// Exact signer/writable/executable privilege tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountPrivilegesV2 {
    signer: bool,
    writable: bool,
    executable: bool,
}

impl AccountPrivilegesV2 {
    /// Construct one exact privilege tuple.
    pub const fn new(signer: bool, writable: bool, executable: bool) -> Self {
        Self {
            signer,
            writable,
            executable,
        }
    }

    const fn bits(self) -> u8 {
        (if self.signer { 1 } else { 0 })
            | (if self.writable { 2 } else { 0 })
            | (if self.executable { 4 } else { 0 })
    }
}

/// Exact effect authority granted to one account coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountEffectPermissionsV2 {
    debit_lamports: bool,
    credit_lamports: bool,
    write_data: bool,
}

impl AccountEffectPermissionsV2 {
    /// Construct one exact generic effect-permission tuple.
    pub const fn new(debit_lamports: bool, credit_lamports: bool, write_data: bool) -> Self {
        Self {
            debit_lamports,
            credit_lamports,
            write_data,
        }
    }

    const fn bits(self) -> u8 {
        (if self.debit_lamports {
            EFFECT_PERMISSION_DEBIT_LAMPORTS
        } else {
            0
        }) | (if self.credit_lamports {
            EFFECT_PERMISSION_CREDIT_LAMPORTS
        } else {
            0
        }) | (if self.write_data {
            EFFECT_PERMISSION_WRITE_DATA
        } else {
            0
        })
    }
}

/// Canonical alias relation for one account rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountAliasInputV2 {
    /// This coordinate is its own representative.
    SelfCoordinate,
    /// Alias one earlier fixed-prefix coordinate.
    Fixed(u16),
    /// Alias one earlier coordinate in the same Product item.
    SameItem(u16),
}

/// One fixed-prefix or repeated-item account rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRuleInputV2 {
    /// Exact runtime privileges.
    pub privileges: AccountPrivilegesV2,
    /// Exact generic effect authority.
    pub effect_permissions: AccountEffectPermissionsV2,
    /// Canonical alias relation.
    pub alias: AccountAliasInputV2,
    /// Fixed exact data width.
    pub data_length: u32,
    /// Additional exact data bytes per authenticated Product item.
    pub data_item_stride: u32,
}

/// One account rule plus its exact lifecycle-prestate semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRuleWithPrestateInputV2 {
    /// Existing exact privilege, effect, alias, and live-width rule.
    pub rule: AccountRuleInputV2,
    /// Exact-only or lifecycle-bound vacant/live prestate.
    pub prestate: AccountPrestateV2,
}

/// Fixed-prefix or repeated-item account coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCoordinateV2 {
    item: bool,
    index: u16,
}

impl AccountCoordinateV2 {
    /// Address one fixed-prefix account.
    pub const fn fixed(index: u16) -> Self {
        Self { item: false, index }
    }

    /// Address one account inside every Product-item subframe.
    pub const fn item(index: u16) -> Self {
        Self { item: true, index }
    }
}

/// Common or repeated-item scalar-register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarCoordinateV2 {
    item: bool,
    index: u16,
}

impl ScalarCoordinateV2 {
    /// Address one common scalar.
    pub const fn common(index: u16) -> Self {
        Self { item: false, index }
    }

    /// Address one scalar inside every Product-item bank.
    pub const fn item(index: u16) -> Self {
        Self { item: true, index }
    }
}

/// Common or repeated-item identity-register coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityCoordinateV2 {
    item: bool,
    index: u16,
}

impl IdentityCoordinateV2 {
    /// Address one common identity.
    pub const fn common(index: u16) -> Self {
        Self { item: false, index }
    }

    /// Address one identity inside every Product-item bank.
    pub const fn item(index: u16) -> Self {
        Self { item: true, index }
    }
}

/// One account relation or register projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountOperationInputV2 {
    /// Require account key equal an immutable input identity.
    RequireKey {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Immutable identity coordinate.
        expected: IdentityCoordinateV2,
    },
    /// Require account owner equal an immutable input identity.
    RequireOwner {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Immutable identity coordinate.
        expected: IdentityCoordinateV2,
    },
    /// Project account key.
    ProjectKey {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination identity coordinate.
        destination: IdentityCoordinateV2,
    },
    /// Project account owner.
    ProjectOwner {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination identity coordinate.
        destination: IdentityCoordinateV2,
    },
    /// Project native lamports.
    ProjectLamports {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination scalar coordinate.
        destination: ScalarCoordinateV2,
    },
    /// Project little-endian `u64` account data.
    ProjectDataU64 {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination scalar coordinate.
        destination: ScalarCoordinateV2,
        /// Exact account-data offset.
        data_offset: u32,
    },
    /// Project little-endian `u32` account data.
    ProjectDataU32 {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination scalar coordinate.
        destination: ScalarCoordinateV2,
        /// Exact account-data offset.
        data_offset: u32,
    },
    /// Project little-endian `u16` account data.
    ProjectDataU16 {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination scalar coordinate.
        destination: ScalarCoordinateV2,
        /// Exact account-data offset.
        data_offset: u32,
    },
    /// Project one `u8` account-data field.
    ProjectDataU8 {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination scalar coordinate.
        destination: ScalarCoordinateV2,
        /// Exact account-data offset.
        data_offset: u32,
    },
    /// Count nonzero rows in one exact Product-width `u64` descriptor tail.
    ProjectNonzeroU64TailCount {
        /// Fixed adapter-authenticated descriptor account.
        account: AccountCoordinateV2,
        /// Common scalar receiving the positive support width.
        destination: ScalarCoordinateV2,
        /// Exact byte offset at which the `u64` tail begins.
        tail_offset: u32,
    },
    /// Derive exact ordered nonzero `u64` support rows into a flat common bank.
    ProjectNonzeroU64TailRows {
        /// Fixed adapter-authenticated descriptor account.
        account: AccountCoordinateV2,
        /// Protected common scalar receiving exact positive K.
        count_destination: ScalarCoordinateV2,
        /// First common scalar receiving row-zero outcome.
        rows_destination: ScalarCoordinateV2,
        /// Exact byte offset at which Product-N coefficients begin.
        tail_offset: u32,
        /// Exact common-scalar stride between row starts; outcome and
        /// coefficient occupy offsets zero and one.
        row_scalar_stride: u16,
    },
    /// Select the sole Product-owned runtime-tail count.
    ProjectTailCountU32 {
        /// Authenticated Product account coordinate.
        account: AccountCoordinateV2,
        /// Destination scalar coordinate.
        destination: ScalarCoordinateV2,
        /// Exact Product-data offset.
        data_offset: u32,
    },
    /// Project one 32-byte account-data identity.
    ProjectDataIdentity {
        /// Account coordinate.
        account: AccountCoordinateV2,
        /// Destination identity coordinate.
        destination: IdentityCoordinateV2,
        /// Exact account-data offset.
        data_offset: u32,
    },
    /// Project one affine per-item `u64` from fixed account data.
    ProjectDataU64Affine {
        /// Fixed account coordinate.
        account: AccountCoordinateV2,
        /// Per-item destination scalar coordinate.
        destination: ScalarCoordinateV2,
        /// First-item account-data offset.
        data_offset: u32,
        /// Exact per-item account-data stride.
        data_stride: u32,
    },
    /// Project one affine per-item identity from fixed account data.
    ProjectDataIdentityAffine {
        /// Fixed account coordinate.
        account: AccountCoordinateV2,
        /// Per-item destination identity coordinate.
        destination: IdentityCoordinateV2,
        /// First-item account-data offset.
        data_offset: u32,
        /// Exact per-item account-data stride.
        data_stride: u32,
    },
    /// Select one authenticated fixed-data row using a projected scalar.
    SelectDataWindow {
        /// Fixed account coordinate.
        account: AccountCoordinateV2,
        /// Common selector scalar.
        selector: ScalarCoordinateV2,
        /// First row account-data offset.
        data_offset: u32,
        /// Fixed bytes before the row's affine tail.
        fixed_row_bytes: u32,
    },
    /// Project one `u64` from the selected fixed-data row.
    ProjectDataU64Selected {
        /// Fixed account coordinate.
        account: AccountCoordinateV2,
        /// Common destination scalar.
        destination: ScalarCoordinateV2,
        /// Offset within the selected row.
        data_offset: u32,
    },
    /// Project one identity from the selected fixed-data row.
    ProjectDataIdentitySelected {
        /// Fixed account coordinate.
        account: AccountCoordinateV2,
        /// Common destination identity.
        destination: IdentityCoordinateV2,
        /// Offset within the selected row.
        data_offset: u32,
    },
    /// Project one per-item `u64` from the selected row's affine tail.
    ProjectDataU64SelectedAffine {
        /// Fixed account coordinate.
        account: AccountCoordinateV2,
        /// Per-item destination scalar.
        destination: ScalarCoordinateV2,
        /// First-item offset relative to the selected row.
        data_offset: u32,
        /// Exact per-item stride.
        data_stride: u32,
    },
    /// Project one per-item identity from the selected row's affine tail.
    ProjectDataIdentitySelectedAffine {
        /// Fixed account coordinate.
        account: AccountCoordinateV2,
        /// Per-item destination identity.
        destination: IdentityCoordinateV2,
        /// First-item offset relative to the selected row.
        data_offset: u32,
        /// Exact per-item stride.
        data_stride: u32,
    },
}

/// Encode one complete AccountProfile V2 atomically.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_v2_atomic(
    artifact: AccountProfileArtifactV2,
    fixed_rules: &[AccountRuleInputV2],
    item_rules: &[AccountRuleInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_with_environment_v2_atomic(
        artifact,
        TrustedEnvironmentV2::None,
        fixed_rules,
        item_rules,
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode one complete AccountProfile V2 with a typed trusted environment.
///
/// A current-slot declaration is accepted only by
/// [`AccountProfileArtifactV2::TrustedEnvironment`]. The complete candidate is
/// hostile-decoded before `output` changes, including environment-coordinate
/// bounds and projection-collision checks.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_environment_v2_atomic(
    artifact: AccountProfileArtifactV2,
    trusted_environment: TrustedEnvironmentV2,
    fixed_rules: &[AccountRuleInputV2],
    item_rules: &[AccountRuleInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        artifact,
        trusted_environment,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::None,
        RuleInputsV2::Exact(fixed_rules),
        RuleInputsV2::Exact(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 6 with explicitly typed lifecycle-bound account rules.
///
/// The complete candidate is hostile-decoded before `output` changes. At
/// least one rule must be lifecycle-bound; old exact profiles therefore cannot
/// be relabeled as this successor artifact.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_lifecycle_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    item_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        AccountProfileArtifactV2::LifecyclePrestate,
        trusted_environment,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::None,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 7 with lifecycle-bound and adapter-authenticated
/// variable-width prestates.
///
/// At least one variable-width rule is required. The hostile decoder enforces
/// that every such rule is a self-representative readonly fixed-prefix record
/// with a nonzero checked prefix and no effect permissions.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_adapter_authenticated_variable_data_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    item_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        AccountProfileArtifactV2::AdapterAuthenticatedVariableData,
        trusted_environment,
        TrustedIdentityEnvironmentV2::None,
        TrustedBuiltinIdentityV2::None,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 8 with trusted runtime scalar and role-identity declarations.
///
/// The identity declaration names only a common register. The authenticated
/// outer seeds that register with its Registry-selected current program before
/// account projection. Profile 8 retains lifecycle-bound and explicitly
/// adapter-authenticated variable-data prestates from profile 7.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_trusted_executing_program_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    item_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        AccountProfileArtifactV2::TrustedExecutingProgram,
        trusted_environment,
        trusted_identity_environment,
        TrustedBuiltinIdentityV2::None,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 9 with non-owning route aliases of adapter-authenticated
/// variable-data representatives.
///
/// Every alias must be a readonly fixed-prefix coordinate that points backward
/// to a self-representative variable-data rule. Its encoded width is zero
/// because runtime width and authentication are inherited from that
/// representative. The complete candidate is hostile-decoded before `output`
/// changes.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_adapter_authenticated_variable_data_alias_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    item_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        AccountProfileArtifactV2::AdapterAuthenticatedVariableDataAlias,
        trusted_environment,
        trusted_identity_environment,
        TrustedBuiltinIdentityV2::None,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 10 with one exact nonzero-`u64` tail-count projection.
///
/// Runtime projection receives Product-authenticated `tail_count`, requires
/// exact `tail_offset + tail_count * 8` data length, and refuses an all-zero
/// tail. The complete candidate is hostile-decoded before `output` changes.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_nonzero_u64_tail_count_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    item_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        AccountProfileArtifactV2::NonzeroU64TailCount,
        trusted_environment,
        trusted_identity_environment,
        TrustedBuiltinIdentityV2::None,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 12 with one exact ordered nonzero-`u64` tail-row projection.
///
/// Runtime projection receives Product-authenticated N, requires exact
/// `tail_offset + N * 8` bytes, derives positive K, requires K to exactly fill
/// the artifact-owned flat row bank, and writes each row's ordered outcome and
/// nonzero coefficient atomically.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    item_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        AccountProfileArtifactV2::NonzeroU64TailRows,
        trusted_environment,
        trusted_identity_environment,
        TrustedBuiltinIdentityV2::None,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 11 with authenticated physical route representatives.
///
/// Later fixed aliases declare only route-local privilege subsets and inherit
/// key, owner, lamports, data, variable-data authentication, and effect
/// authority from their earlier representative. The optional trusted builtin
/// names a common identity destination that the adapter seeds from its
/// canonical System Program constant.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_authenticated_route_alias_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    item_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic(
        AccountProfileArtifactV2::AuthenticatedRouteAlias,
        trusted_environment,
        trusted_identity_environment,
        trusted_builtin_identity,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode profile 13 with zero or more checked dynamic account spans inserted
/// into a fixed logical sequence.
///
/// Each span count is read only from its declared common scalar and must remain
/// within the exact inclusive range. An empty span table is the canonical
/// fixed-topology form and requires an empty span-rule template. Fixed rules
/// and operations retain base coordinates; the decoder shifts every suffix
/// coordinate and alias target at runtime. Opaque-data rules authenticate
/// account facts without granting local data projection or effect authority.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_dynamic_fixed_span_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    dynamic_spans: &[DynamicFixedSpanInputV2],
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    span_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic_with_span(
        AccountProfileArtifactV2::DynamicFixedSpan,
        trusted_environment,
        trusted_identity_environment,
        trusted_builtin_identity,
        Some(dynamic_spans),
        None,
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(span_rules),
        fixed_operations,
        &[],
        registers,
        scratch,
        output,
    )
}

/// Encode profile 13 while projecting each fixed rule on demand.
///
/// This is byte-for-byte equivalent to
/// [`encode_account_profile_with_dynamic_fixed_span_v2_atomic`], but it does
/// not require callers with a large fixed account vector to materialize a
/// parallel rule array. The projector is invoked exactly once for every
/// coordinate in ascending order. Its error, hostile decoding failure, or any
/// geometry refusal leaves `output` unchanged.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic<F>(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    dynamic_spans: &[DynamicFixedSpanInputV2],
    fixed_rule_count: u16,
    fixed_rule: F,
    span_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error>
where
    F: FnMut(u16) -> Result<AccountRuleWithPrestateInputV2, Error>,
{
    let mut fixed_rule = fixed_rule;
    encode_account_profile_with_dynamic_fixed_span_v2_borrowed_generated_atomic(
        trusted_environment,
        trusted_identity_environment,
        trusted_builtin_identity,
        dynamic_spans,
        fixed_rule_count,
        &mut fixed_rule,
        span_rules,
        fixed_operations,
        registers,
        scratch,
        output,
    )
}

/// Encode Profile 13 from a borrowed generated-rule projector.
///
/// This has identical failure-atomic semantics and bytes to
/// [`encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic`].
/// The explicit borrowed callback is a physical refinement for constrained
/// targets: it prevents a large family projector from being monomorphized into
/// the semantic-owner encoder's hostile-decode frame.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn encode_account_profile_with_dynamic_fixed_span_v2_borrowed_generated_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    dynamic_spans: &[DynamicFixedSpanInputV2],
    fixed_rule_count: u16,
    fixed_rule: &mut dyn FnMut(u16) -> Result<AccountRuleWithPrestateInputV2, Error>,
    span_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic_with_span(
        AccountProfileArtifactV2::DynamicFixedSpan,
        trusted_environment,
        trusted_identity_environment,
        trusted_builtin_identity,
        Some(dynamic_spans),
        None,
        GeneratedRuleInputsV2 {
            len: usize::from(fixed_rule_count),
            project: fixed_rule,
        },
        RuleInputsV2::WithPrestate(span_rules),
        fixed_operations,
        &[],
        registers,
        scratch,
        output,
    )
}

/// Encode Profile 14 with Profile 13 dynamic-span/route-alias semantics plus
/// a canonical table of fixed-data prestate predicates.
///
/// Predicate ordering and target eligibility are hostile-decoded before
/// `output` changes. Predicates may address only exact or lifecycle-live,
/// fixed, self-representative rules with fixed data width.
#[allow(clippy::too_many_arguments)]
pub fn encode_account_profile_with_fixed_data_predicates_v2_atomic(
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    dynamic_spans: &[DynamicFixedSpanInputV2],
    predicates: &[FixedDataPredicateInputV2],
    fixed_rules: &[AccountRuleWithPrestateInputV2],
    span_rules: &[AccountRuleWithPrestateInputV2],
    fixed_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic_with_span(
        AccountProfileArtifactV2::FixedDataPredicate,
        trusted_environment,
        trusted_identity_environment,
        trusted_builtin_identity,
        Some(dynamic_spans),
        Some(predicates),
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(span_rules),
        fixed_operations,
        &[],
        registers,
        scratch,
        output,
    )
}

#[derive(Clone, Copy)]
enum RuleInputsV2<'a> {
    Exact(&'a [AccountRuleInputV2]),
    WithPrestate(&'a [AccountRuleWithPrestateInputV2]),
}

trait RuleSourceV2 {
    fn len(&self) -> usize;

    fn get(&mut self, index: usize) -> Result<(AccountRuleInputV2, AccountPrestateV2), Error>;
}

impl RuleSourceV2 for RuleInputsV2<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Exact(values) => values.len(),
            Self::WithPrestate(values) => values.len(),
        }
    }

    fn get(&mut self, index: usize) -> Result<(AccountRuleInputV2, AccountPrestateV2), Error> {
        match self {
            Self::Exact(values) => values
                .get(index)
                .copied()
                .map(|rule| (rule, AccountPrestateV2::Exact)),
            Self::WithPrestate(values) => values
                .get(index)
                .copied()
                .map(|value| (value.rule, value.prestate)),
        }
        .ok_or(Error::InvalidLength)
    }
}

struct GeneratedRuleInputsV2<F> {
    len: usize,
    project: F,
}

impl<F> RuleSourceV2 for GeneratedRuleInputsV2<F>
where
    F: FnMut(u16) -> Result<AccountRuleWithPrestateInputV2, Error>,
{
    fn len(&self) -> usize {
        self.len
    }

    fn get(&mut self, index: usize) -> Result<(AccountRuleInputV2, AccountPrestateV2), Error> {
        if index >= self.len {
            return Err(Error::InvalidLength);
        }
        let value = (self.project)(u16::try_from(index).map_err(|_| Error::InvalidLength)?)?;
        Ok((value.rule, value.prestate))
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_account_profile_atomic(
    artifact: AccountProfileArtifactV2,
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    fixed_rules: RuleInputsV2<'_>,
    item_rules: RuleInputsV2<'_>,
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    encode_account_profile_atomic_with_span(
        artifact,
        trusted_environment,
        trusted_identity_environment,
        trusted_builtin_identity,
        None,
        None,
        fixed_rules,
        item_rules,
        fixed_operations,
        item_operations,
        registers,
        scratch,
        output,
    )
}

#[allow(clippy::too_many_arguments)]
fn encode_account_profile_atomic_with_span<F, I>(
    artifact: AccountProfileArtifactV2,
    trusted_environment: TrustedEnvironmentV2,
    trusted_identity_environment: TrustedIdentityEnvironmentV2,
    trusted_builtin_identity: TrustedBuiltinIdentityV2,
    dynamic_fixed_spans: Option<&[DynamicFixedSpanInputV2]>,
    fixed_data_predicates: Option<&[FixedDataPredicateInputV2]>,
    mut fixed_rules: F,
    mut item_rules: I,
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error>
where
    F: RuleSourceV2,
    I: RuleSourceV2,
{
    if trusted_environment.current_slot_destination().is_some()
        && !matches!(
            artifact,
            AccountProfileArtifactV2::TrustedEnvironment
                | AccountProfileArtifactV2::LifecyclePrestate
                | AccountProfileArtifactV2::AdapterAuthenticatedVariableData
                | AccountProfileArtifactV2::TrustedExecutingProgram
                | AccountProfileArtifactV2::AdapterAuthenticatedVariableDataAlias
                | AccountProfileArtifactV2::NonzeroU64TailCount
                | AccountProfileArtifactV2::AuthenticatedRouteAlias
                | AccountProfileArtifactV2::NonzeroU64TailRows
                | AccountProfileArtifactV2::DynamicFixedSpan
                | AccountProfileArtifactV2::FixedDataPredicate
        )
    {
        return Err(Error::InvalidTrustedEnvironment);
    }
    if trusted_identity_environment
        .current_executing_program_destination()
        .is_some()
        && !matches!(
            artifact,
            AccountProfileArtifactV2::TrustedExecutingProgram
                | AccountProfileArtifactV2::AdapterAuthenticatedVariableDataAlias
                | AccountProfileArtifactV2::NonzeroU64TailCount
                | AccountProfileArtifactV2::AuthenticatedRouteAlias
                | AccountProfileArtifactV2::NonzeroU64TailRows
                | AccountProfileArtifactV2::DynamicFixedSpan
                | AccountProfileArtifactV2::FixedDataPredicate
        )
    {
        return Err(Error::InvalidTrustedExecutingProgram);
    }
    if trusted_builtin_identity
        .system_program_destination()
        .is_some()
        && !matches!(
            artifact,
            AccountProfileArtifactV2::AuthenticatedRouteAlias
                | AccountProfileArtifactV2::DynamicFixedSpan
                | AccountProfileArtifactV2::FixedDataPredicate
        )
    {
        return Err(Error::InvalidTrustedBuiltin);
    }
    let fixed_account_count = u16::try_from(fixed_rules.len()).map_err(|_| Error::InvalidLength)?;
    let item_account_stride = u16::try_from(item_rules.len()).map_err(|_| Error::InvalidLength)?;
    let fixed_operation_count =
        u16::try_from(fixed_operations.len()).map_err(|_| Error::InvalidLength)?;
    let item_operation_count =
        u16::try_from(item_operations.len()).map_err(|_| Error::InvalidLength)?;
    let dynamic_table_bytes = match dynamic_fixed_spans {
        Some(spans) => spans
            .len()
            .checked_mul(DYNAMIC_FIXED_SPAN_ENTRY_BYTES)
            .ok_or(Error::InvalidLength)?,
        None => 0,
    };
    let predicates = fixed_data_predicates.unwrap_or(&[]);
    if (artifact == AccountProfileArtifactV2::FixedDataPredicate) != fixed_data_predicates.is_some()
    {
        return Err(Error::InvalidFixedDataPredicate);
    }
    let predicate_table_bytes = predicates
        .len()
        .checked_mul(FIXED_DATA_PREDICATE_BYTES)
        .ok_or(Error::InvalidLength)?;
    let expected = fixed_rules
        .len()
        .checked_add(item_rules.len())
        .and_then(|count| count.checked_mul(RULE_BYTES))
        .and_then(|rules| {
            fixed_operations
                .len()
                .checked_add(item_operations.len())
                .and_then(|count| count.checked_mul(OPERATION_BYTES))
                .and_then(|operations| rules.checked_add(operations))
        })
        .and_then(|body| {
            artifact
                .header_bytes()
                .checked_add(dynamic_table_bytes)
                .and_then(|header| header.checked_add(predicate_table_bytes))
                .and_then(|header| header.checked_add(body))
        })
        .ok_or(Error::InvalidLength)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(scratch, 0, &MAGIC)?;
    for (offset, value) in [
        (8, VERSION),
        (10, artifact.value()),
        (12, fixed_account_count),
        (14, item_account_stride),
        (16, fixed_operation_count),
        (18, item_operation_count),
        (20, registers.common_scalars),
        (22, registers.item_scalar_stride),
        (24, registers.common_identities),
        (26, registers.item_identity_stride),
    ] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    if let TrustedEnvironmentV2::CurrentSlot { destination } = trusted_environment {
        write(
            scratch,
            TRUSTED_ENVIRONMENT_SCALAR_OFFSET,
            &destination.to_le_bytes(),
        )?;
        write_byte(
            scratch,
            TRUSTED_ENVIRONMENT_KIND_OFFSET,
            TRUSTED_ENVIRONMENT_CURRENT_SLOT,
        )?;
    }
    if let TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination } =
        trusted_identity_environment
    {
        write(
            scratch,
            TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET,
            &destination.to_le_bytes(),
        )?;
        write_byte(
            scratch,
            TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET,
            TRUSTED_EXECUTING_PROGRAM_CURRENT,
        )?;
    }
    if let TrustedBuiltinIdentityV2::SystemProgram { destination } = trusted_builtin_identity {
        write(
            scratch,
            TRUSTED_BUILTIN_IDENTITY_OFFSET,
            &destination.to_le_bytes(),
        )?;
        write_byte(
            scratch,
            TRUSTED_BUILTIN_KIND_OFFSET,
            TRUSTED_BUILTIN_SYSTEM_PROGRAM,
        )?;
    }
    if let Some(spans) = dynamic_fixed_spans {
        write(
            scratch,
            DYNAMIC_FIXED_SPAN_COUNT_OFFSET,
            &u16::try_from(spans.len())
                .map_err(|_| Error::InvalidLength)?
                .to_le_bytes(),
        )?;
        let mut index = 0_usize;
        while index < spans.len() {
            let span = *spans.get(index).ok_or(Error::InvalidLength)?;
            let offset = index
                .checked_mul(DYNAMIC_FIXED_SPAN_ENTRY_BYTES)
                .and_then(|body| DYNAMIC_FIXED_SPAN_HEADER_BYTES.checked_add(body))
                .ok_or(Error::InvalidLength)?;
            for (relative, value) in [
                (
                    DYNAMIC_FIXED_SPAN_ENTRY_INSERTION_OFFSET,
                    span.insertion_coordinate,
                ),
                (
                    DYNAMIC_FIXED_SPAN_ENTRY_COUNT_SCALAR_OFFSET,
                    span.count_scalar,
                ),
                (DYNAMIC_FIXED_SPAN_ENTRY_RULE_START_OFFSET, span.rule_start),
                (
                    DYNAMIC_FIXED_SPAN_ENTRY_RULE_STRIDE_OFFSET,
                    span.rule_stride,
                ),
            ] {
                write(scratch, add(offset, relative)?, &value.to_le_bytes())?;
            }
            write(
                scratch,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_MIN_OFFSET)?,
                &span.minimum.to_le_bytes(),
            )?;
            write(
                scratch,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_MAX_OFFSET)?,
                &span.maximum.to_le_bytes(),
            )?;
            write(
                scratch,
                add(offset, DYNAMIC_FIXED_SPAN_ENTRY_STEP_OFFSET)?,
                &span.step.to_le_bytes(),
            )?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
    }
    let mut cursor = artifact
        .header_bytes()
        .checked_add(dynamic_table_bytes)
        .ok_or(Error::InvalidLength)?;
    if fixed_data_predicates.is_some() {
        write(
            scratch,
            FIXED_DATA_PREDICATE_COUNT_OFFSET,
            &u16::try_from(predicates.len())
                .map_err(|_| Error::InvalidLength)?
                .to_le_bytes(),
        )?;
        let mut index = 0_usize;
        while index < predicates.len() {
            let (opcode, account, data_offset, payload) = predicates
                .get(index)
                .copied()
                .ok_or(Error::InvalidLength)?
                .parts();
            write_byte(scratch, cursor, opcode)?;
            write(scratch, add(cursor, 2)?, &account.to_le_bytes())?;
            write(scratch, add(cursor, 4)?, &data_offset.to_le_bytes())?;
            write(scratch, add(cursor, 8)?, &payload)?;
            cursor = add(cursor, FIXED_DATA_PREDICATE_BYTES)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
    }
    let mut rule_index = 0_usize;
    while rule_index < fixed_rules.len() {
        let (rule, prestate) = fixed_rules.get(rule_index)?;
        encode_rule(rule, prestate, scratch, cursor)?;
        cursor = add(cursor, RULE_BYTES)?;
        rule_index = rule_index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    rule_index = 0;
    while rule_index < item_rules.len() {
        let (rule, prestate) = item_rules.get(rule_index)?;
        encode_rule(rule, prestate, scratch, cursor)?;
        cursor = add(cursor, RULE_BYTES)?;
        rule_index = rule_index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    for operation in fixed_operations.iter().chain(item_operations) {
        encode_operation(*operation, scratch, cursor)?;
        cursor = add(cursor, OPERATION_BYTES)?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    AccountProfileV2::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn encode_rule(
    rule: AccountRuleInputV2,
    prestate: AccountPrestateV2,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    write_byte(output, offset, rule.privileges.bits())?;
    write_byte(output, add(offset, 1)?, rule.effect_permissions.bits())?;
    let (alias_kind, alias_index) = match rule.alias {
        AccountAliasInputV2::SelfCoordinate => (0, 0),
        AccountAliasInputV2::Fixed(index) => (1, index),
        AccountAliasInputV2::SameItem(index) => (2, index),
    };
    write_byte(output, add(offset, 2)?, alias_kind)?;
    write_byte(
        output,
        add(offset, 3)?,
        match prestate {
            AccountPrestateV2::Exact => 0,
            AccountPrestateV2::LifecycleBound => 1,
            AccountPrestateV2::AdapterAuthenticatedVariableData => 2,
            AccountPrestateV2::AdapterAuthenticatedVariableDataAlias => 3,
            AccountPrestateV2::AuthenticatedRouteAlias => 4,
            AccountPrestateV2::AuthenticatedOpaqueReadonlyData => 5,
        },
    )?;
    write(output, add(offset, 4)?, &alias_index.to_le_bytes())?;
    write(output, add(offset, 8)?, &rule.data_length.to_le_bytes())?;
    write(
        output,
        add(offset, 12)?,
        &rule.data_item_stride.to_le_bytes(),
    )
}

#[derive(Clone, Copy)]
struct EncodedOperationV2 {
    opcode: u8,
    account: AccountCoordinateV2,
    register_item: bool,
    register: u16,
    data_offset: u32,
    data_stride: u32,
}

fn encode_operation(
    operation: AccountOperationInputV2,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    let value = operation.encode();
    write_byte(output, offset, value.opcode)?;
    write_byte(output, add(offset, 1)?, u8::from(value.account.item))?;
    write(output, add(offset, 2)?, &value.account.index.to_le_bytes())?;
    write_byte(output, add(offset, 4)?, u8::from(value.register_item))?;
    write(output, add(offset, 6)?, &value.register.to_le_bytes())?;
    write(output, add(offset, 8)?, &value.data_offset.to_le_bytes())?;
    write(output, add(offset, 12)?, &value.data_stride.to_le_bytes())
}

impl AccountOperationInputV2 {
    fn encode(self) -> EncodedOperationV2 {
        match self {
            Self::RequireKey { account, expected } => {
                identity(OP_REQUIRE_KEY, account, expected, 0, 0)
            }
            Self::RequireOwner { account, expected } => {
                identity(OP_REQUIRE_OWNER, account, expected, 0, 0)
            }
            Self::ProjectKey {
                account,
                destination,
            } => identity(OP_PROJECT_KEY, account, destination, 0, 0),
            Self::ProjectOwner {
                account,
                destination,
            } => identity(OP_PROJECT_OWNER, account, destination, 0, 0),
            Self::ProjectLamports {
                account,
                destination,
            } => scalar(OP_PROJECT_LAMPORTS, account, destination, 0, 0),
            Self::ProjectDataU64 {
                account,
                destination,
                data_offset,
            } => scalar(OP_PROJECT_DATA_U64, account, destination, data_offset, 0),
            Self::ProjectDataU32 {
                account,
                destination,
                data_offset,
            } => scalar(OP_PROJECT_DATA_U32, account, destination, data_offset, 0),
            Self::ProjectDataU16 {
                account,
                destination,
                data_offset,
            } => scalar(OP_PROJECT_DATA_U16, account, destination, data_offset, 0),
            Self::ProjectDataU8 {
                account,
                destination,
                data_offset,
            } => scalar(OP_PROJECT_DATA_U8, account, destination, data_offset, 0),
            Self::ProjectNonzeroU64TailCount {
                account,
                destination,
                tail_offset,
            } => scalar(
                OP_PROJECT_NONZERO_U64_TAIL_COUNT,
                account,
                destination,
                tail_offset,
                8,
            ),
            Self::ProjectNonzeroU64TailRows {
                account,
                count_destination,
                rows_destination,
                tail_offset,
                row_scalar_stride,
            } => {
                let packed = if count_destination.item {
                    0
                } else {
                    u32::from(count_destination.index) << 16 | u32::from(row_scalar_stride)
                };
                EncodedOperationV2 {
                    opcode: OP_PROJECT_NONZERO_U64_TAIL_ROWS,
                    account,
                    register_item: rows_destination.item,
                    register: rows_destination.index,
                    data_offset: tail_offset,
                    data_stride: packed,
                }
            }
            Self::ProjectTailCountU32 {
                account,
                destination,
                data_offset,
            } => scalar(
                OP_PROJECT_TAIL_COUNT_U32,
                account,
                destination,
                data_offset,
                0,
            ),
            Self::ProjectDataIdentity {
                account,
                destination,
                data_offset,
            } => identity(
                OP_PROJECT_DATA_IDENTITY,
                account,
                destination,
                data_offset,
                0,
            ),
            Self::ProjectDataU64Affine {
                account,
                destination,
                data_offset,
                data_stride,
            } => scalar(
                OP_PROJECT_DATA_U64_AFFINE,
                account,
                destination,
                data_offset,
                data_stride,
            ),
            Self::ProjectDataIdentityAffine {
                account,
                destination,
                data_offset,
                data_stride,
            } => identity(
                OP_PROJECT_DATA_IDENTITY_AFFINE,
                account,
                destination,
                data_offset,
                data_stride,
            ),
            Self::SelectDataWindow {
                account,
                selector,
                data_offset,
                fixed_row_bytes,
            } => scalar(
                OP_SELECT_DATA_WINDOW,
                account,
                selector,
                data_offset,
                fixed_row_bytes,
            ),
            Self::ProjectDataU64Selected {
                account,
                destination,
                data_offset,
            } => scalar(
                OP_PROJECT_DATA_U64_SELECTED,
                account,
                destination,
                data_offset,
                0,
            ),
            Self::ProjectDataIdentitySelected {
                account,
                destination,
                data_offset,
            } => identity(
                OP_PROJECT_DATA_IDENTITY_SELECTED,
                account,
                destination,
                data_offset,
                0,
            ),
            Self::ProjectDataU64SelectedAffine {
                account,
                destination,
                data_offset,
                data_stride,
            } => scalar(
                OP_PROJECT_DATA_U64_SELECTED_AFFINE,
                account,
                destination,
                data_offset,
                data_stride,
            ),
            Self::ProjectDataIdentitySelectedAffine {
                account,
                destination,
                data_offset,
                data_stride,
            } => identity(
                OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE,
                account,
                destination,
                data_offset,
                data_stride,
            ),
        }
    }
}

const fn scalar(
    opcode: u8,
    account: AccountCoordinateV2,
    register: ScalarCoordinateV2,
    data_offset: u32,
    data_stride: u32,
) -> EncodedOperationV2 {
    EncodedOperationV2 {
        opcode,
        account,
        register_item: register.item,
        register: register.index,
        data_offset,
        data_stride,
    }
}

const fn identity(
    opcode: u8,
    account: AccountCoordinateV2,
    register: IdentityCoordinateV2,
    data_offset: u32,
    data_stride: u32,
) -> EncodedOperationV2 {
    EncodedOperationV2 {
        opcode,
        account,
        register_item: register.item,
        register: register.index,
        data_offset,
        data_stride,
    }
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = add(offset, value.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::AccountObservationV1;
    use crate::v2::{
        PhysicalAccountDataGeometryV2, ProjectionRegistersV2, TRUSTED_ENVIRONMENT_RESERVED_OFFSET,
        derive_effect_permissions_with_dynamic_spans, project_atomic,
        project_dynamic_fixed_spans_atomic,
    };

    const READONLY: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, false);
    const WRITABLE: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, true, false);
    const NO_EFFECTS: AccountEffectPermissionsV2 =
        AccountEffectPermissionsV2::new(false, false, false);

    #[test]
    fn typed_encoder_round_trips_and_preserves_output_on_hostile_profile() {
        let rules = [
            AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 4,
                data_item_stride: 0,
            },
            AccountRuleInputV2 {
                privileges: WRITABLE,
                effect_permissions: AccountEffectPermissionsV2::new(false, false, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
        ];
        let operations = [
            AccountOperationInputV2::RequireKey {
                account: AccountCoordinateV2::fixed(0),
                expected: IdentityCoordinateV2::common(0),
            },
            AccountOperationInputV2::RequireOwner {
                account: AccountCoordinateV2::fixed(1),
                expected: IdentityCoordinateV2::common(1),
            },
            AccountOperationInputV2::ProjectDataU16 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(0),
                data_offset: 1,
            },
            AccountOperationInputV2::ProjectKey {
                account: AccountCoordinateV2::fixed(1),
                destination: IdentityCoordinateV2::common(2),
            },
        ];
        let width = HEADER_BYTES + rules.len() * RULE_BYTES + operations.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![9_u8; width];
        encode_account_profile_v2_atomic(
            AccountProfileArtifactV2::TypedScalar,
            &rules,
            &[],
            &operations,
            &[],
            RegisterGeometryV2 {
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 3,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut output,
        )
        .expect("encode");
        let profile = AccountProfileV2::decode(&output).expect("hostile decode encoded profile");
        assert_eq!(profile.fixed_account_count(), 2);
        assert_eq!(profile.artifact_profile(), TYPED_SCALAR_ARTIFACT_PROFILE);

        let hostile = [
            operations[0],
            operations[1],
            operations[2],
            AccountOperationInputV2::ProjectLamports {
                account: AccountCoordinateV2::fixed(1),
                destination: ScalarCoordinateV2::common(0),
            },
        ];
        let mut hostile_scratch = std::vec![0_u8; width];
        let mut hostile_output = std::vec![7_u8; width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_v2_atomic(
                AccountProfileArtifactV2::TypedScalar,
                &rules,
                &[],
                &hostile,
                &[],
                RegisterGeometryV2 {
                    common_scalars: 1,
                    item_scalar_stride: 0,
                    common_identities: 3,
                    item_identity_stride: 0,
                },
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::DuplicateProjection)
        );
        assert_eq!(hostile_output, before);
    }

    #[test]
    fn trusted_current_slot_round_trips_and_cannot_be_projected_over() {
        let rules = [AccountRuleInputV2 {
            privileges: READONLY,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 4,
            data_item_stride: 0,
        }];
        let projected = [AccountOperationInputV2::ProjectDataU16 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
            data_offset: 1,
        }];
        let registers = RegisterGeometryV2 {
            common_scalars: 64,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let width = HEADER_BYTES + RULE_BYTES + OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![9_u8; width];
        encode_account_profile_with_environment_v2_atomic(
            AccountProfileArtifactV2::TrustedEnvironment,
            TrustedEnvironmentV2::CurrentSlot { destination: 1 },
            &rules,
            &[],
            &projected,
            &[],
            registers,
            &mut scratch,
            &mut output,
        )
        .expect("trusted environment profile");
        assert_eq!(
            output.get(TRUSTED_ENVIRONMENT_SCALAR_OFFSET..TRUSTED_ENVIRONMENT_KIND_OFFSET),
            Some(1_u16.to_le_bytes().as_slice())
        );
        assert_eq!(
            output.get(TRUSTED_ENVIRONMENT_KIND_OFFSET),
            Some(&TRUSTED_ENVIRONMENT_CURRENT_SLOT)
        );
        assert_eq!(output.get(TRUSTED_ENVIRONMENT_RESERVED_OFFSET), Some(&0));
        let profile = AccountProfileV2::decode(&output).expect("decode trusted environment");
        assert_eq!(
            profile.trusted_environment(),
            TrustedEnvironmentV2::CurrentSlot { destination: 1 }
        );
        assert_eq!(profile.trusted_current_slot_scalar(), Some(1));
        assert_eq!(
            profile.writes_register(crate::v2::ProjectionTargetV2 {
                kind: crate::v2::ProjectionRegisterKindV2::Scalar,
                space: crate::v2::ProjectionRegisterSpaceV2::Common,
                index: 0,
            }),
            Ok(true)
        );
        assert_eq!(
            profile.writes_register(crate::v2::ProjectionTargetV2 {
                kind: crate::v2::ProjectionRegisterKindV2::Scalar,
                space: crate::v2::ProjectionRegisterSpaceV2::Common,
                index: 1,
            }),
            Ok(true)
        );
        assert_eq!(
            profile.writes_register(crate::v2::ProjectionTargetV2 {
                kind: crate::v2::ProjectionRegisterKindV2::Identity,
                space: crate::v2::ProjectionRegisterSpaceV2::Common,
                index: 0,
            }),
            Ok(false)
        );

        let data = [0x55, 0x34, 0x12, 0xaa];
        let accounts = [AccountObservationV1::new(
            [1; 32], [2; 32], 0, &data, false, false, false,
        )];
        let mut input_scalars = [0_u64; 64];
        *input_scalars.get_mut(1).expect("slot scalar") = 77_777;
        let mut scratch_scalars = [0_u64; 64];
        let mut output_scalars = [9_u64; 64];
        project_atomic(
            profile,
            0,
            &accounts,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &[],
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut [],
                output_scalars: &mut output_scalars,
                output_identities: &mut [],
            },
        )
        .expect("preserve trusted slot");
        assert_eq!(output_scalars.first(), Some(&0x1234));
        assert_eq!(output_scalars.get(1), Some(&77_777));

        let collision = [AccountOperationInputV2::ProjectDataU16 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(1),
            data_offset: 1,
        }];
        let mut collision_output = std::vec![7_u8; width];
        let before = collision_output.clone();
        assert_eq!(
            encode_account_profile_with_environment_v2_atomic(
                AccountProfileArtifactV2::TrustedEnvironment,
                TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                &rules,
                &[],
                &collision,
                &[],
                registers,
                &mut scratch,
                &mut collision_output,
            ),
            Err(Error::TrustedEnvironmentOverwrite)
        );
        assert_eq!(collision_output, before);
    }

    #[test]
    fn lifecycle_prestate_projects_vacant_zero_or_exact_live_data_atomically() {
        let rules = [AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: WRITABLE,
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 152,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        }];
        let operations = [
            AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(0),
                data_offset: 8,
            },
            AccountOperationInputV2::ProjectDataIdentity {
                account: AccountCoordinateV2::fixed(0),
                destination: IdentityCoordinateV2::common(0),
                data_offset: 32,
            },
        ];
        let width = HEADER_BYTES + RULE_BYTES + operations.len() * OPERATION_BYTES;
        let mut encode_scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_lifecycle_v2_atomic(
            TrustedEnvironmentV2::CurrentSlot { destination: 1 },
            &rules,
            &[],
            &operations,
            &[],
            RegisterGeometryV2 {
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut encode_scratch,
            &mut encoded,
        )
        .expect("lifecycle profile");
        let profile = AccountProfileV2::decode(&encoded).expect("decode lifecycle profile");
        assert_eq!(
            profile.artifact_profile(),
            LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
        );
        assert_eq!(
            profile.rule(false, 0).expect("rule").prestate(),
            AccountPrestateV2::LifecycleBound
        );

        let project =
            |data: &[u8], output_scalars: &mut [u64; 2], output_ids: &mut [[u8; 32]; 1]| {
                let accounts = [AccountObservationV1::new(
                    [1; 32], [2; 32], 7, data, false, true, false,
                )];
                let input_scalars = [9_u64, 77_777];
                let mut scalar_scratch = [0_u64; 2];
                let mut identity_scratch = [[0_u8; 32]; 1];
                project_atomic(
                    profile,
                    0,
                    &accounts,
                    ProjectionRegistersV2 {
                        input_scalars: &input_scalars,
                        input_identities: &[[8; 32]],
                        scratch_scalars: &mut scalar_scratch,
                        scratch_identities: &mut identity_scratch,
                        output_scalars,
                        output_identities: output_ids,
                    },
                )
            };
        let mut vacant_scalars = [99_u64; 2];
        let mut vacant_ids = [[9_u8; 32]; 1];
        project(&[], &mut vacant_scalars, &mut vacant_ids).expect("vacant zero prestate");
        assert_eq!(vacant_scalars, [0, 77_777]);
        assert_eq!(vacant_ids, [[0; 32]]);

        let mut live = [0_u8; 152];
        live.get_mut(8..16)
            .expect("nonce")
            .copy_from_slice(&41_u64.to_le_bytes());
        live.get_mut(32..64)
            .expect("maker")
            .copy_from_slice(&[6; 32]);
        let mut live_scalars = [99_u64; 2];
        let mut live_ids = [[9_u8; 32]; 1];
        project(&live, &mut live_scalars, &mut live_ids).expect("live exact prestate");
        assert_eq!(live_scalars, [41, 77_777]);
        assert_eq!(live_ids, [[6; 32]]);

        let mut partial_scalars = [99_u64; 2];
        let before_scalars = partial_scalars;
        let mut partial_ids = [[9_u8; 32]; 1];
        let before_ids = partial_ids;
        assert_eq!(
            project(&live[..151], &mut partial_scalars, &mut partial_ids),
            Err(Error::DataLengthMismatch)
        );
        assert_eq!(partial_scalars, before_scalars);
        assert_eq!(partial_ids, before_ids);

        let hostile_operations = [AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(0),
            expected: IdentityCoordinateV2::common(0),
        }];
        let hostile_width = HEADER_BYTES + RULE_BYTES + OPERATION_BYTES;
        let mut hostile_scratch = std::vec![0_u8; hostile_width];
        let mut hostile_output = std::vec![0x55_u8; hostile_width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_lifecycle_v2_atomic(
                TrustedEnvironmentV2::None,
                &rules,
                &[],
                &hostile_operations,
                &[],
                RegisterGeometryV2 {
                    common_scalars: 1,
                    item_scalar_stride: 0,
                    common_identities: 1,
                    item_identity_stride: 0,
                },
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidLifecyclePrestate)
        );
        assert_eq!(hostile_output, before);
    }

    #[test]
    fn adapter_authenticated_variable_data_is_explicit_bounded_and_readonly() {
        let variable_rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 16,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AdapterAuthenticatedVariableData,
        };
        let lifecycle_rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: WRITABLE,
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 152,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        };
        let rules = [variable_rule, lifecycle_rule];
        let operations = [
            AccountOperationInputV2::ProjectDataU32 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(0),
                data_offset: 260,
            },
            AccountOperationInputV2::ProjectDataU64 {
                account: AccountCoordinateV2::fixed(1),
                destination: ScalarCoordinateV2::common(2),
                data_offset: 8,
            },
        ];
        let width = HEADER_BYTES + rules.len() * RULE_BYTES + operations.len() * OPERATION_BYTES;
        let mut encode_scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        let registers = RegisterGeometryV2 {
            common_scalars: 3,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        encode_account_profile_with_adapter_authenticated_variable_data_v2_atomic(
            TrustedEnvironmentV2::CurrentSlot { destination: 1 },
            &rules,
            &[],
            &operations,
            &[],
            registers,
            &mut encode_scratch,
            &mut encoded,
        )
        .expect("variable-data successor profile");
        let profile = AccountProfileV2::decode(&encoded).expect("decode successor profile");
        assert_eq!(
            profile.artifact_profile(),
            ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE
        );
        assert_eq!(
            profile.rule(false, 0).expect("variable rule").prestate(),
            AccountPrestateV2::AdapterAuthenticatedVariableData
        );

        let mut variable_data = [0_u8; 264];
        variable_data
            .get_mut(260..264)
            .expect("projected field")
            .copy_from_slice(&0x4433_2211_u32.to_le_bytes());
        let live_lifecycle = [0_u8; 152];
        let project = |variable: AccountObservationV1<'_>,
                       output: &mut [u64; 3]|
         -> crate::v2::Result<()> {
            let accounts = [
                variable,
                AccountObservationV1::new([3; 32], [4; 32], 9, &live_lifecycle, false, true, false),
            ];
            let input = [0_u64, 77_777, 0];
            let mut scratch = [0_u64; 3];
            project_atomic(
                profile,
                0,
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &input,
                    input_identities: &[],
                    scratch_scalars: &mut scratch,
                    scratch_identities: &mut [],
                    output_scalars: output,
                    output_identities: &mut [],
                },
            )
        };

        let trusted = AccountObservationV1::new_adapter_authenticated_variable_data(
            [1; 32],
            [2; 32],
            7,
            &variable_data,
            false,
            false,
            false,
        );
        assert!(trusted.adapter_authenticated_variable_data());
        let mut projected = [9_u64; 3];
        project(trusted, &mut projected).expect("project actual variable width");
        assert_eq!(projected, [0x4433_2211, 77_777, 0]);

        let ordinary =
            AccountObservationV1::new([1; 32], [2; 32], 7, &variable_data, false, false, false);
        assert!(!ordinary.adapter_authenticated_variable_data());
        let mut refused = [9_u64; 3];
        let before = refused;
        assert_eq!(
            project(ordinary, &mut refused),
            Err(Error::InvalidVariableDataPrestate)
        );
        assert_eq!(refused, before);

        let short = AccountObservationV1::new_adapter_authenticated_variable_data(
            [1; 32],
            [2; 32],
            7,
            &variable_data[..260],
            false,
            false,
            false,
        );
        assert_eq!(project(short, &mut refused), Err(Error::DataOutOfBounds));
        assert_eq!(refused, before);

        let exact_rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                data_length: 264,
                ..variable_rule.rule
            },
            prestate: AccountPrestateV2::Exact,
        };
        let mut exact_scratch = std::vec![0_u8; width];
        let mut exact_output = std::vec![0x55_u8; width];
        let exact_before = exact_output.clone();
        assert_eq!(
            encode_account_profile_with_adapter_authenticated_variable_data_v2_atomic(
                TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                &[exact_rule, lifecycle_rule],
                &[],
                &operations,
                &[],
                registers,
                &mut exact_scratch,
                &mut exact_output,
            ),
            Err(Error::InvalidVariableDataPrestate)
        );
        assert_eq!(exact_output, exact_before);

        let mut wrong_profile = encoded.clone();
        wrong_profile
            .get_mut(10..12)
            .expect("artifact profile")
            .copy_from_slice(&LIFECYCLE_PRESTATE_ARTIFACT_PROFILE.to_le_bytes());
        assert_eq!(
            AccountProfileV2::decode(&wrong_profile),
            Err(Error::InvalidLifecyclePrestate)
        );

        let exact_rules = [AccountRuleInputV2 {
            data_length: 264,
            ..variable_rule.rule
        }];
        let exact_width = HEADER_BYTES + RULE_BYTES + OPERATION_BYTES;
        let mut exact_scratch = std::vec![0_u8; exact_width];
        let mut exact_encoded = std::vec![0_u8; exact_width];
        encode_account_profile_with_environment_v2_atomic(
            AccountProfileArtifactV2::TrustedEnvironment,
            TrustedEnvironmentV2::None,
            &exact_rules,
            &[],
            &operations[..1],
            &[],
            RegisterGeometryV2 {
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &mut exact_scratch,
            &mut exact_encoded,
        )
        .expect("exact profile");
        let exact_profile = AccountProfileV2::decode(&exact_encoded).expect("exact decode");
        let exact_accounts = [trusted];
        let input_scalars = [0_u64];
        let mut scratch_scalars = [0_u64];
        let mut output_scalars = [9_u64];
        assert_eq!(
            project_atomic(
                exact_profile,
                0,
                &exact_accounts,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &[],
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut [],
                    output_scalars: &mut output_scalars,
                    output_identities: &mut [],
                },
            ),
            Err(Error::InvalidVariableDataPrestate)
        );
        assert_eq!(output_scalars, [9]);

        for (hostile, expected) in [
            (
                AccountRuleWithPrestateInputV2 {
                    rule: AccountRuleInputV2 {
                        privileges: WRITABLE,
                        ..variable_rule.rule
                    },
                    ..variable_rule
                },
                Error::InvalidVariableDataPrestate,
            ),
            (
                AccountRuleWithPrestateInputV2 {
                    rule: AccountRuleInputV2 {
                        effect_permissions: AccountEffectPermissionsV2::new(false, true, false),
                        ..variable_rule.rule
                    },
                    ..variable_rule
                },
                Error::InvalidEffectPermissions,
            ),
            (
                AccountRuleWithPrestateInputV2 {
                    rule: AccountRuleInputV2 {
                        data_length: 0,
                        ..variable_rule.rule
                    },
                    ..variable_rule
                },
                Error::InvalidVariableDataPrestate,
            ),
            (
                AccountRuleWithPrestateInputV2 {
                    rule: AccountRuleInputV2 {
                        data_item_stride: 1,
                        ..variable_rule.rule
                    },
                    ..variable_rule
                },
                Error::InvalidVariableDataPrestate,
            ),
        ] {
            let mut hostile_scratch = std::vec![0_u8; width];
            let mut hostile_output = std::vec![0x55_u8; width];
            let hostile_before = hostile_output.clone();
            assert_eq!(
                encode_account_profile_with_adapter_authenticated_variable_data_v2_atomic(
                    TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                    &[hostile, lifecycle_rule],
                    &[],
                    &operations,
                    &[],
                    registers,
                    &mut hostile_scratch,
                    &mut hostile_output,
                ),
                Err(expected)
            );
            assert_eq!(hostile_output, hostile_before);
        }
    }

    #[test]
    fn trusted_environment_hostile_headers_and_bounds_refuse_atomically() {
        let rules = [AccountRuleInputV2 {
            privileges: READONLY,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 0,
            data_item_stride: 0,
        }];
        let registers = RegisterGeometryV2 {
            common_scalars: 2,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let width = HEADER_BYTES + RULE_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_account_profile_with_environment_v2_atomic(
            AccountProfileArtifactV2::TrustedEnvironment,
            TrustedEnvironmentV2::CurrentSlot { destination: 1 },
            &rules,
            &[],
            &[],
            &[],
            registers,
            &mut scratch,
            &mut output,
        )
        .expect("canonical environment");

        for (offset, value, expected) in [
            (
                TRUSTED_ENVIRONMENT_SCALAR_OFFSET,
                2,
                Error::InvalidTrustedEnvironment,
            ),
            (
                TRUSTED_ENVIRONMENT_KIND_OFFSET,
                2,
                Error::InvalidTrustedEnvironment,
            ),
            (
                TRUSTED_ENVIRONMENT_RESERVED_OFFSET,
                1,
                Error::InvalidTrustedEnvironment,
            ),
        ] {
            let mut hostile = output.clone();
            *hostile.get_mut(offset).expect("hostile header") = value;
            assert_eq!(AccountProfileV2::decode(&hostile), Err(expected));
        }
        let mut noncanonical_none = output.clone();
        *noncanonical_none
            .get_mut(TRUSTED_ENVIRONMENT_KIND_OFFSET)
            .expect("kind") = 0;
        assert_eq!(
            AccountProfileV2::decode(&noncanonical_none),
            Err(Error::InvalidTrustedEnvironment)
        );

        let mut old_profile = output.clone();
        old_profile
            .get_mut(10..12)
            .expect("artifact profile")
            .copy_from_slice(&TYPED_SCALAR_ARTIFACT_PROFILE.to_le_bytes());
        assert_eq!(
            AccountProfileV2::decode(&old_profile),
            Err(Error::NonCanonicalReserved)
        );

        let mut refused_output = std::vec![7_u8; width];
        let before = refused_output.clone();
        assert_eq!(
            encode_account_profile_with_environment_v2_atomic(
                AccountProfileArtifactV2::TrustedEnvironment,
                TrustedEnvironmentV2::CurrentSlot { destination: 2 },
                &rules,
                &[],
                &[],
                &[],
                registers,
                &mut scratch,
                &mut refused_output,
            ),
            Err(Error::InvalidTrustedEnvironment)
        );
        assert_eq!(refused_output, before);

        assert_eq!(
            encode_account_profile_with_environment_v2_atomic(
                AccountProfileArtifactV2::TypedScalar,
                TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                &rules,
                &[],
                &[],
                &[],
                registers,
                &mut scratch,
                &mut refused_output,
            ),
            Err(Error::InvalidTrustedEnvironment)
        );
        assert_eq!(refused_output, before);
    }

    #[test]
    fn trusted_executing_program_is_profile8_only_bounded_and_overwrite_safe() {
        let rules = [AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 4,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AdapterAuthenticatedVariableData,
        }];
        let operations = [
            AccountOperationInputV2::ProjectDataU32 {
                account: AccountCoordinateV2::fixed(0),
                destination: ScalarCoordinateV2::common(0),
                data_offset: 0,
            },
            AccountOperationInputV2::ProjectKey {
                account: AccountCoordinateV2::fixed(0),
                destination: IdentityCoordinateV2::common(1),
            },
        ];
        let registers = RegisterGeometryV2 {
            common_scalars: 2,
            item_scalar_stride: 0,
            common_identities: 3,
            item_identity_stride: 0,
        };
        let width = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES
            + RULE_BYTES
            + operations.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_trusted_executing_program_v2_atomic(
            TrustedEnvironmentV2::CurrentSlot { destination: 1 },
            TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 2 },
            &rules,
            &[],
            &operations,
            &[],
            registers,
            &mut scratch,
            &mut encoded,
        )
        .expect("profile8");
        let profile = AccountProfileV2::decode(&encoded).expect("decode profile8");
        assert_eq!(
            profile.artifact_profile(),
            TRUSTED_EXECUTING_PROGRAM_ARTIFACT_PROFILE
        );
        assert_eq!(profile.trusted_current_slot_scalar(), Some(1));
        assert_eq!(
            profile.trusted_current_executing_program_identity(),
            Some(2)
        );
        assert_eq!(
            profile.trusted_identity_environment(),
            TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 2 }
        );
        assert!(
            profile
                .writes_register(crate::v2::ProjectionTargetV2 {
                    kind: crate::v2::ProjectionRegisterKindV2::Identity,
                    space: crate::v2::ProjectionRegisterSpaceV2::Common,
                    index: 2,
                })
                .expect("inspect trusted writer")
        );

        let data = 0x4433_2211_u32.to_le_bytes();
        let account = AccountObservationV1::new_adapter_authenticated_variable_data(
            [4; 32], [5; 32], 9, &data, false, false, false,
        );
        let input_scalars = [0_u64, 77];
        let input_identities = [[0; 32], [0; 32], [8; 32]];
        let mut scratch_scalars = [0_u64; 2];
        let mut scratch_identities = [[0_u8; 32]; 3];
        let mut output_scalars = [9_u64; 2];
        let mut output_identities = [[9_u8; 32]; 3];
        project_atomic(
            profile,
            0,
            &[account],
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("project profile8");
        assert_eq!(output_scalars, [0x4433_2211, 77]);
        assert_eq!(output_identities, [[0; 32], [4; 32], [8; 32]]);

        for (offset, value, expected) in [
            (
                TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET,
                3,
                Error::InvalidTrustedExecutingProgram,
            ),
            (
                TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET,
                2,
                Error::InvalidTrustedExecutingProgram,
            ),
            (
                crate::v2::TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET,
                1,
                Error::InvalidTrustedExecutingProgram,
            ),
        ] {
            let mut hostile = encoded.clone();
            *hostile.get_mut(offset).expect("hostile profile8 header") = value;
            assert_eq!(AccountProfileV2::decode(&hostile), Err(expected));
        }
        let truncated = encoded
            .get(..encoded.len().saturating_sub(1))
            .expect("truncated profile8");
        assert_eq!(
            AccountProfileV2::decode(truncated),
            Err(Error::InvalidLength)
        );
        let mut relabeled = encoded.clone();
        relabeled
            .get_mut(10..12)
            .expect("profile")
            .copy_from_slice(&ADAPTER_AUTHENTICATED_VARIABLE_DATA_ARTIFACT_PROFILE.to_le_bytes());
        assert_eq!(
            AccountProfileV2::decode(&relabeled),
            Err(Error::InvalidLength)
        );

        let colliding_operations = [AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(0),
            destination: IdentityCoordinateV2::common(2),
        }];
        let colliding_width = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES + RULE_BYTES + OPERATION_BYTES;
        let mut colliding_scratch = std::vec![0_u8; colliding_width];
        let mut refused = std::vec![0x55_u8; colliding_width];
        let before = refused.clone();
        assert_eq!(
            encode_account_profile_with_trusted_executing_program_v2_atomic(
                TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 2 },
                &rules,
                &[],
                &colliding_operations,
                &[],
                registers,
                &mut colliding_scratch,
                &mut refused,
            ),
            Err(Error::TrustedExecutingProgramOverwrite)
        );
        assert_eq!(refused, before);

        let mut out_of_bounds_scratch = std::vec![0_u8; width];
        let mut out_of_bounds = std::vec![0x66_u8; width];
        let out_of_bounds_before = out_of_bounds.clone();
        assert_eq!(
            encode_account_profile_with_trusted_executing_program_v2_atomic(
                TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 3 },
                &rules,
                &[],
                &operations,
                &[],
                registers,
                &mut out_of_bounds_scratch,
                &mut out_of_bounds,
            ),
            Err(Error::InvalidTrustedExecutingProgram)
        );
        assert_eq!(out_of_bounds, out_of_bounds_before);
    }

    #[test]
    fn variable_data_route_alias_inherits_one_authenticated_representative() {
        let empty = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let variable = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                data_length: 16,
                ..empty.rule
            },
            prestate: AccountPrestateV2::AdapterAuthenticatedVariableData,
        };
        let alias = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                alias: AccountAliasInputV2::Fixed(4),
                ..empty.rule
            },
            prestate: AccountPrestateV2::AdapterAuthenticatedVariableDataAlias,
        };
        let mut rules = [empty; 15];
        *rules.get_mut(0).expect("bump rule") = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                data_length: 12,
                ..empty.rule
            },
            prestate: AccountPrestateV2::Exact,
        };
        *rules.get_mut(4).expect("variable representative") = variable;
        *rules.get_mut(14).expect("route alias") = alias;
        let operations = [AccountOperationInputV2::ProjectDataU8 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
            data_offset: 11,
        }];
        let registers = RegisterGeometryV2 {
            common_scalars: 2,
            item_scalar_stride: 0,
            common_identities: 2,
            item_identity_stride: 0,
        };
        let width = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES
            + rules.len() * RULE_BYTES
            + operations.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_adapter_authenticated_variable_data_alias_v2_atomic(
            TrustedEnvironmentV2::CurrentSlot { destination: 1 },
            TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
            &rules,
            &[],
            &operations,
            &[],
            registers,
            &mut scratch,
            &mut encoded,
        )
        .expect("profile9");
        let profile = AccountProfileV2::decode(&encoded).expect("decode profile9");
        assert_eq!(
            profile.artifact_profile(),
            ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE
        );
        assert_eq!(
            profile.rule(false, 14).expect("alias rule").prestate(),
            AccountPrestateV2::AdapterAuthenticatedVariableDataAlias
        );
        assert_eq!(profile.representative(0, 14), Ok(4));

        let mut state = [0_u8; 12];
        *state.get_mut(11).expect("bump") = 0xa5;
        let basis = [0x44_u8; 64];
        let mut accounts = std::vec::Vec::with_capacity(15);
        for coordinate in 0_u8..15 {
            let observation = match coordinate {
                0 => AccountObservationV1::new([200; 32], [2; 32], 7, &state, false, false, false),
                4 => AccountObservationV1::new_adapter_authenticated_variable_data(
                    [4; 32], [5; 32], 9, &basis, false, false, false,
                ),
                14 => AccountObservationV1::new([4; 32], [5; 32], 9, &basis, false, false, false),
                value => {
                    AccountObservationV1::new([value; 32], [2; 32], 0, &[], false, false, false)
                }
            };
            accounts.push(observation);
        }
        let project = |observations: &[AccountObservationV1<'_>],
                       output_scalars: &mut [u64; 2],
                       output_identities: &mut [[u8; 32]; 2]| {
            let input_scalars = [0_u64, 77];
            let input_identities = [[0_u8; 32], [8_u8; 32]];
            let mut scalar_scratch = [0_u64; 2];
            let mut identity_scratch = [[0_u8; 32]; 2];
            project_atomic(
                profile,
                0,
                observations,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scalar_scratch,
                    scratch_identities: &mut identity_scratch,
                    output_scalars,
                    output_identities,
                },
            )
        };
        let mut output_scalars = [9_u64; 2];
        let mut output_identities = [[9_u8; 32]; 2];
        project(&accounts, &mut output_scalars, &mut output_identities)
            .expect("inherited authenticated alias");
        assert_eq!(output_scalars, [0xa5, 77]);
        assert_eq!(output_identities, [[0; 32], [8; 32]]);

        let assert_atomic_refusal = |hostile: &[AccountObservationV1<'_>], expected: Error| {
            let mut refused_scalars = [19_u64; 2];
            let before_scalars = refused_scalars;
            let mut refused_identities = [[19_u8; 32]; 2];
            let before_identities = refused_identities;
            assert_eq!(
                project(hostile, &mut refused_scalars, &mut refused_identities),
                Err(expected)
            );
            assert_eq!(refused_scalars, before_scalars);
            assert_eq!(refused_identities, before_identities);
        };

        let mut asserted_alias = accounts.clone();
        *asserted_alias.get_mut(14).expect("alias") =
            AccountObservationV1::new_adapter_authenticated_variable_data(
                [4; 32], [5; 32], 9, &basis, false, false, false,
            );
        assert_atomic_refusal(&asserted_alias, Error::InvalidVariableDataPrestate);

        let mut substituted_alias = accounts.clone();
        *substituted_alias.get_mut(14).expect("alias") =
            AccountObservationV1::new([6; 32], [5; 32], 9, &basis, false, false, false);
        assert_atomic_refusal(&substituted_alias, Error::AliasMismatch);

        let mut wrong_owner = accounts.clone();
        *wrong_owner.get_mut(14).expect("alias") =
            AccountObservationV1::new([4; 32], [6; 32], 9, &basis, false, false, false);
        assert_atomic_refusal(&wrong_owner, Error::AliasMismatch);

        let short_basis = [0x44_u8; 63];
        let mut partial_body = accounts.clone();
        *partial_body.get_mut(14).expect("alias") =
            AccountObservationV1::new([4; 32], [5; 32], 9, &short_basis, false, false, false);
        assert_atomic_refusal(&partial_body, Error::AliasMismatch);

        let mut unauthenticated_representative = accounts.clone();
        *unauthenticated_representative
            .get_mut(4)
            .expect("representative") =
            AccountObservationV1::new([4; 32], [5; 32], 9, &basis, false, false, false);
        assert_atomic_refusal(
            &unauthenticated_representative,
            Error::InvalidVariableDataPrestate,
        );

        let alias_offset = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES + 14 * RULE_BYTES;
        for hostile in [(3_usize, 2_u8), (8, 1), (12, 1)] {
            let mut bytes = encoded.clone();
            *bytes
                .get_mut(alias_offset + hostile.0)
                .expect("hostile alias rule") = hostile.1;
            assert_eq!(
                AccountProfileV2::decode(&bytes),
                Err(Error::InvalidVariableDataPrestate)
            );
        }
        let mut wrong_representative = encoded.clone();
        wrong_representative
            .get_mut(alias_offset + 4..alias_offset + 6)
            .expect("alias representative")
            .copy_from_slice(&0_u16.to_le_bytes());
        assert_eq!(
            AccountProfileV2::decode(&wrong_representative),
            Err(Error::InvalidVariableDataPrestate)
        );

        let alias_operation = [AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(14),
            destination: IdentityCoordinateV2::common(0),
        }];
        let hostile_width =
            TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES + rules.len() * RULE_BYTES + OPERATION_BYTES;
        let mut hostile_scratch = std::vec![0_u8; hostile_width];
        let mut hostile_output = std::vec![0x55_u8; hostile_width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_adapter_authenticated_variable_data_alias_v2_atomic(
                TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
                &rules,
                &[],
                &alias_operation,
                &[],
                registers,
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidVariableDataPrestate)
        );
        assert_eq!(hostile_output, before);

        let mut u8_bytes = [0_u8; OPERATION_BYTES];
        crate::v2::encode_project_data_u8_operation_v2(&mut u8_bytes, 0, 0, 11)
            .expect("typed u8 operation");
        assert_eq!(u8_bytes.first(), Some(&OP_PROJECT_DATA_U8));
        assert_eq!(
            crate::v2::encode_project_data_u8_operation_v2(&mut u8_bytes[..15], 0, 0, 11),
            Err(Error::InvalidLength)
        );
        assert_eq!(
            crate::v2::encode_project_data_u8_operation_v2(&mut u8_bytes, 0, 0, u32::MAX,),
            Err(Error::InvalidLength)
        );

        let out_of_bounds = [AccountOperationInputV2::ProjectDataU8 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
            data_offset: 12,
        }];
        let mut out_of_bounds_scratch = std::vec![0_u8; width];
        let mut out_of_bounds_bytes = std::vec![0_u8; width];
        encode_account_profile_with_adapter_authenticated_variable_data_alias_v2_atomic(
            TrustedEnvironmentV2::CurrentSlot { destination: 1 },
            TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
            &rules,
            &[],
            &out_of_bounds,
            &[],
            registers,
            &mut out_of_bounds_scratch,
            &mut out_of_bounds_bytes,
        )
        .expect("bounded u8 profile");
        let out_of_bounds_profile =
            AccountProfileV2::decode(&out_of_bounds_bytes).expect("decode bounded profile");
        let input_scalars = [0_u64, 77];
        let input_identities = [[0_u8; 32], [8_u8; 32]];
        let mut scalar_scratch = [0_u64; 2];
        let mut identity_scratch = [[0_u8; 32]; 2];
        let mut refused_scalars = [31_u64; 2];
        let before_scalars = refused_scalars;
        let mut refused_identities = [[31_u8; 32]; 2];
        let before_identities = refused_identities;
        assert_eq!(
            project_atomic(
                out_of_bounds_profile,
                0,
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scalar_scratch,
                    scratch_identities: &mut identity_scratch,
                    output_scalars: &mut refused_scalars,
                    output_identities: &mut refused_identities,
                },
            ),
            Err(Error::DataOutOfBounds)
        );
        assert_eq!(refused_scalars, before_scalars);
        assert_eq!(refused_identities, before_identities);

        let u8_operation = *operations.first().expect("u8 operation");
        let duplicate_u8 = [u8_operation, u8_operation];
        let duplicate_width = width + OPERATION_BYTES;
        let mut duplicate_scratch = std::vec![0_u8; duplicate_width];
        let mut duplicate_output = std::vec![0x55_u8; duplicate_width];
        let before = duplicate_output.clone();
        assert_eq!(
            encode_account_profile_with_adapter_authenticated_variable_data_alias_v2_atomic(
                TrustedEnvironmentV2::CurrentSlot { destination: 1 },
                TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
                &rules,
                &[],
                &duplicate_u8,
                &[],
                registers,
                &mut duplicate_scratch,
                &mut duplicate_output,
            ),
            Err(Error::DuplicateProjection)
        );
        assert_eq!(duplicate_output, before);
    }

    #[test]
    fn nonzero_u64_tail_count_is_exact_positive_and_failure_atomic() {
        const TAIL_OFFSET: u32 = 16;
        const TAIL_COUNT: u32 = 4;

        let readonly = AccountRuleInputV2 {
            privileges: READONLY,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 4,
            data_item_stride: 0,
        };
        let rules = [
            AccountRuleWithPrestateInputV2 {
                rule: readonly,
                prestate: AccountPrestateV2::Exact,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    data_length: TAIL_OFFSET,
                    ..readonly
                },
                prestate: AccountPrestateV2::AdapterAuthenticatedVariableData,
            },
        ];
        let tail_projection = AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
            data_offset: 0,
        };
        let count_projection = AccountOperationInputV2::ProjectNonzeroU64TailCount {
            account: AccountCoordinateV2::fixed(1),
            destination: ScalarCoordinateV2::common(7),
            tail_offset: TAIL_OFFSET,
        };
        let operations = [tail_projection, count_projection];
        let registers = RegisterGeometryV2 {
            common_scalars: 8,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let width = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES
            + rules.len() * RULE_BYTES
            + operations.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_nonzero_u64_tail_count_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            &rules,
            &[],
            &operations,
            &[],
            registers,
            &mut scratch,
            &mut encoded,
        )
        .expect("profile10");
        let profile = AccountProfileV2::decode(&encoded).expect("decode profile10");
        assert_eq!(
            profile.artifact_profile(),
            NONZERO_U64_TAIL_COUNT_ARTIFACT_PROFILE
        );
        let projection = profile
            .nonzero_u64_tail_count_projection()
            .expect("inspect projection")
            .expect("count projection");
        assert_eq!(projection.account(), 1);
        assert_eq!(projection.destination(), 7);
        assert_eq!(projection.tail_offset(), TAIL_OFFSET);

        let product = TAIL_COUNT.to_le_bytes();
        let mut descriptor = std::vec![0_u8; usize::try_from(TAIL_OFFSET).expect("offset")];
        for value in [0_u64, 7, 0, 9] {
            descriptor.extend_from_slice(&value.to_le_bytes());
        }
        let project = |descriptor_data: &[u8], output: &mut [u64; 8]| {
            let observations = [
                AccountObservationV1::new([1; 32], [2; 32], 1, &product, false, false, false),
                AccountObservationV1::new_adapter_authenticated_variable_data(
                    [3; 32],
                    [4; 32],
                    1,
                    descriptor_data,
                    false,
                    false,
                    false,
                ),
            ];
            let input_scalars = [0_u64; 8];
            let input_identities: [[u8; 32]; 0] = [];
            let mut scalar_scratch = [0_u64; 8];
            let mut identity_scratch: [[u8; 32]; 0] = [];
            let mut output_identities: [[u8; 32]; 0] = [];
            project_atomic(
                profile,
                TAIL_COUNT,
                &observations,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scalar_scratch,
                    scratch_identities: &mut identity_scratch,
                    output_scalars: output,
                    output_identities: &mut output_identities,
                },
            )
        };
        let mut output = [99_u64; 8];
        project(&descriptor, &mut output).expect("derive sparse support");
        assert_eq!(output[0], u64::from(TAIL_COUNT));
        assert_eq!(output[7], 2);

        let short = descriptor
            .get(..descriptor.len() - 1)
            .expect("short")
            .to_vec();
        let mut long = descriptor.clone();
        long.push(0);
        for hostile in [&short[..], &long[..]] {
            let mut refused = [77_u64; 8];
            let before = refused;
            assert_eq!(
                project(hostile, &mut refused),
                Err(Error::DataLengthMismatch)
            );
            assert_eq!(refused, before);
        }
        let all_zero = std::vec![
            0_u8;
            usize::try_from(TAIL_OFFSET).expect("offset")
                + usize::try_from(TAIL_COUNT).expect("count") * 8
        ];
        let mut refused = [55_u64; 8];
        let before = refused;
        assert_eq!(
            project(&all_zero, &mut refused),
            Err(Error::EmptyNonzeroTail)
        );
        assert_eq!(refused, before);

        let zero_product = 0_u32.to_le_bytes();
        let zero_descriptor = std::vec![0_u8; usize::try_from(TAIL_OFFSET).expect("offset")];
        let zero_observations = [
            AccountObservationV1::new([1; 32], [2; 32], 1, &zero_product, false, false, false),
            AccountObservationV1::new_adapter_authenticated_variable_data(
                [3; 32],
                [4; 32],
                1,
                &zero_descriptor,
                false,
                false,
                false,
            ),
        ];
        let input_scalars = [0_u64; 8];
        let input_identities: [[u8; 32]; 0] = [];
        let mut scalar_scratch = [0_u64; 8];
        let mut identity_scratch: [[u8; 32]; 0] = [];
        let mut zero_output = [44_u64; 8];
        let zero_before = zero_output;
        let mut output_identities: [[u8; 32]; 0] = [];
        assert_eq!(
            project_atomic(
                profile,
                0,
                &zero_observations,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scalar_scratch,
                    scratch_identities: &mut identity_scratch,
                    output_scalars: &mut zero_output,
                    output_identities: &mut output_identities,
                },
            ),
            Err(Error::EmptyNonzeroTail)
        );
        assert_eq!(zero_output, zero_before);

        let operation_offset = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES + rules.len() * RULE_BYTES;
        let count_operation_offset = operation_offset + OPERATION_BYTES;
        let mut wrong_stride = encoded.clone();
        wrong_stride
            .get_mut(count_operation_offset + 12..count_operation_offset + 16)
            .expect("count stride")
            .copy_from_slice(&7_u32.to_le_bytes());
        assert_eq!(
            AccountProfileV2::decode(&wrong_stride),
            Err(Error::NonCanonicalOperation)
        );
        let mut wrong_profile = encoded.clone();
        wrong_profile
            .get_mut(10..12)
            .expect("artifact profile")
            .copy_from_slice(
                &ADAPTER_AUTHENTICATED_VARIABLE_DATA_ALIAS_ARTIFACT_PROFILE.to_le_bytes(),
            );
        assert_eq!(
            AccountProfileV2::decode(&wrong_profile),
            Err(Error::InvalidVariableDataPrestate)
        );

        let mut wrong_rules = rules;
        wrong_rules[1].prestate = AccountPrestateV2::Exact;
        let mut wrong_scratch = std::vec![0_u8; width];
        let mut wrong_output = std::vec![0x55_u8; width];
        let before = wrong_output.clone();
        assert_eq!(
            encode_account_profile_with_nonzero_u64_tail_count_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                &wrong_rules,
                &[],
                &operations,
                &[],
                registers,
                &mut wrong_scratch,
                &mut wrong_output,
            ),
            Err(Error::InvalidVariableDataPrestate)
        );
        assert_eq!(wrong_output, before);

        for hostile_operations in [
            [count_projection, count_projection],
            [tail_projection, tail_projection],
        ] {
            let mut hostile_scratch = std::vec![0_u8; width];
            let mut hostile_output = std::vec![0x66_u8; width];
            let before = hostile_output.clone();
            assert_eq!(
                encode_account_profile_with_nonzero_u64_tail_count_v2_atomic(
                    TrustedEnvironmentV2::None,
                    TrustedIdentityEnvironmentV2::None,
                    &rules,
                    &[],
                    &hostile_operations,
                    &[],
                    registers,
                    &mut hostile_scratch,
                    &mut hostile_output,
                ),
                Err(Error::DuplicateProjection)
            );
            assert_eq!(hostile_output, before);
        }

        for missing in [[tail_projection], [count_projection]] {
            let missing_width =
                TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES + rules.len() * RULE_BYTES + OPERATION_BYTES;
            let mut missing_scratch = std::vec![0_u8; missing_width];
            let mut missing_output = std::vec![0x77_u8; missing_width];
            let before = missing_output.clone();
            assert_eq!(
                encode_account_profile_with_nonzero_u64_tail_count_v2_atomic(
                    TrustedEnvironmentV2::None,
                    TrustedIdentityEnvironmentV2::None,
                    &rules,
                    &[],
                    &missing,
                    &[],
                    registers,
                    &mut missing_scratch,
                    &mut missing_output,
                ),
                Err(Error::NonCanonicalOperation)
            );
            assert_eq!(missing_output, before);
        }
    }

    #[test]
    fn nonzero_u64_tail_rows_are_ordered_exact_and_failure_atomic() {
        const TAIL_OFFSET: u32 = 16;
        const TAIL_COUNT: u32 = 5;

        let readonly = AccountRuleInputV2 {
            privileges: READONLY,
            effect_permissions: NO_EFFECTS,
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 4,
            data_item_stride: 0,
        };
        let rules = [
            AccountRuleWithPrestateInputV2 {
                rule: readonly,
                prestate: AccountPrestateV2::Exact,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    data_length: TAIL_OFFSET,
                    ..readonly
                },
                prestate: AccountPrestateV2::AdapterAuthenticatedVariableData,
            },
        ];
        let tail_projection = AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(1),
            data_offset: 0,
        };
        let rows_projection = AccountOperationInputV2::ProjectNonzeroU64TailRows {
            account: AccountCoordinateV2::fixed(1),
            count_destination: ScalarCoordinateV2::common(0),
            rows_destination: ScalarCoordinateV2::common(2),
            tail_offset: TAIL_OFFSET,
            row_scalar_stride: 2,
        };
        let operations = [tail_projection, rows_projection];
        let registers = RegisterGeometryV2 {
            common_scalars: 8,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let width = TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES
            + rules.len() * RULE_BYTES
            + operations.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            &rules,
            &[],
            &operations,
            &[],
            registers,
            &mut scratch,
            &mut encoded,
        )
        .expect("profile12");
        let profile = AccountProfileV2::decode(&encoded).expect("decode profile12");
        assert_eq!(
            profile.artifact_profile(),
            NONZERO_U64_TAIL_ROWS_ARTIFACT_PROFILE
        );
        let projection = profile
            .nonzero_u64_tail_rows_projection()
            .expect("inspect projection")
            .expect("row projection");
        assert_eq!(projection.account(), 1);
        assert_eq!(projection.count_destination(), 0);
        assert_eq!(projection.rows_destination(), 2);
        assert_eq!(projection.tail_offset(), TAIL_OFFSET);
        assert_eq!(projection.row_scalar_stride(), 2);

        let product = TAIL_COUNT.to_le_bytes();
        let descriptor = descriptor_with_coefficients(TAIL_OFFSET, &[0, 7, 5, 0, 9]);
        let project =
            |selected_profile: AccountProfileV2<'_>, descriptor_data: &[u8], output: &mut [u64]| {
                let observations = [
                    AccountObservationV1::new([1; 32], [2; 32], 1, &product, false, false, false),
                    AccountObservationV1::new_adapter_authenticated_variable_data(
                        [3; 32],
                        [4; 32],
                        1,
                        descriptor_data,
                        false,
                        false,
                        false,
                    ),
                ];
                let input_scalars = std::vec![0_u64; output.len()];
                let input_identities: [[u8; 32]; 0] = [];
                let mut scalar_scratch = std::vec![0_u64; output.len()];
                let mut identity_scratch: [[u8; 32]; 0] = [];
                let mut output_identities: [[u8; 32]; 0] = [];
                project_atomic(
                    selected_profile,
                    TAIL_COUNT,
                    &observations,
                    ProjectionRegistersV2 {
                        input_scalars: &input_scalars,
                        input_identities: &input_identities,
                        scratch_scalars: &mut scalar_scratch,
                        scratch_identities: &mut identity_scratch,
                        output_scalars: output,
                        output_identities: &mut output_identities,
                    },
                )
            };
        let mut output = [99_u64; 8];
        project(profile, &descriptor, &mut output).expect("derive exact ordered rows");
        assert_eq!(output, [3, 5, 1, 7, 2, 5, 4, 9]);

        for index in 0_u16..8 {
            assert_eq!(
                profile.writes_register(crate::v2::ProjectionTargetV2 {
                    kind: crate::v2::ProjectionRegisterKindV2::Scalar,
                    space: crate::v2::ProjectionRegisterSpaceV2::Common,
                    index,
                }),
                Ok(true)
            );
        }

        let short = descriptor
            .get(..descriptor.len() - 1)
            .expect("short")
            .to_vec();
        let mut long = descriptor.clone();
        long.push(0);
        for hostile in [&short[..], &long[..]] {
            let mut refused = [77_u64; 8];
            let before = refused;
            assert_eq!(
                project(profile, hostile, &mut refused),
                Err(Error::DataLengthMismatch)
            );
            assert_eq!(refused, before);
        }

        let all_zero = descriptor_with_coefficients(TAIL_OFFSET, &[0; 5]);
        let mut refused = [55_u64; 8];
        let before = refused;
        assert_eq!(
            project(profile, &all_zero, &mut refused),
            Err(Error::EmptyNonzeroTail)
        );
        assert_eq!(refused, before);

        for common_scalars in [6_u16, 10] {
            let hostile_registers = RegisterGeometryV2 {
                common_scalars,
                ..registers
            };
            let mut hostile_scratch = std::vec![0_u8; width];
            let mut hostile_encoded = std::vec![0_u8; width];
            encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                &rules,
                &[],
                &operations,
                &[],
                hostile_registers,
                &mut hostile_scratch,
                &mut hostile_encoded,
            )
            .expect("hostile K geometry still decodes");
            let hostile_profile =
                AccountProfileV2::decode(&hostile_encoded).expect("hostile profile");
            let mut hostile_output = std::vec![88_u64; usize::from(common_scalars)];
            let before = hostile_output.clone();
            assert_eq!(
                project(hostile_profile, &descriptor, &mut hostile_output),
                Err(Error::SupportRowCountMismatch)
            );
            assert_eq!(hostile_output, before);
        }

        for hostile_operations in [
            [rows_projection, rows_projection],
            [tail_projection, tail_projection],
            [
                rows_projection,
                AccountOperationInputV2::ProjectDataU64 {
                    account: AccountCoordinateV2::fixed(0),
                    destination: ScalarCoordinateV2::common(3),
                    data_offset: 0,
                },
            ],
        ] {
            let mut hostile_scratch = std::vec![0_u8; width];
            let mut hostile_output = std::vec![0x66_u8; width];
            let before = hostile_output.clone();
            assert_eq!(
                encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
                    TrustedEnvironmentV2::None,
                    TrustedIdentityEnvironmentV2::None,
                    &rules,
                    &[],
                    &hostile_operations,
                    &[],
                    registers,
                    &mut hostile_scratch,
                    &mut hostile_output,
                ),
                Err(Error::DuplicateProjection)
            );
            assert_eq!(hostile_output, before);
        }

        for missing in [[tail_projection], [rows_projection]] {
            let missing_width =
                TRUSTED_EXECUTING_PROGRAM_HEADER_BYTES + rules.len() * RULE_BYTES + OPERATION_BYTES;
            let mut missing_scratch = std::vec![0_u8; missing_width];
            let mut missing_output = std::vec![0x77_u8; missing_width];
            let before = missing_output.clone();
            assert_eq!(
                encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
                    TrustedEnvironmentV2::None,
                    TrustedIdentityEnvironmentV2::None,
                    &rules,
                    &[],
                    &missing,
                    &[],
                    registers,
                    &mut missing_scratch,
                    &mut missing_output,
                ),
                Err(Error::NonCanonicalOperation)
            );
            assert_eq!(missing_output, before);
        }

        for hostile_rows in [
            AccountOperationInputV2::ProjectNonzeroU64TailRows {
                account: AccountCoordinateV2::fixed(1),
                count_destination: ScalarCoordinateV2::common(2),
                rows_destination: ScalarCoordinateV2::common(2),
                tail_offset: TAIL_OFFSET,
                row_scalar_stride: 2,
            },
            AccountOperationInputV2::ProjectNonzeroU64TailRows {
                account: AccountCoordinateV2::fixed(1),
                count_destination: ScalarCoordinateV2::common(0),
                rows_destination: ScalarCoordinateV2::common(2),
                tail_offset: TAIL_OFFSET,
                row_scalar_stride: 1,
            },
            AccountOperationInputV2::ProjectNonzeroU64TailRows {
                account: AccountCoordinateV2::fixed(1),
                count_destination: ScalarCoordinateV2::common(0),
                rows_destination: ScalarCoordinateV2::item(0),
                tail_offset: TAIL_OFFSET,
                row_scalar_stride: 2,
            },
            AccountOperationInputV2::ProjectNonzeroU64TailRows {
                account: AccountCoordinateV2::fixed(1),
                count_destination: ScalarCoordinateV2::item(0),
                rows_destination: ScalarCoordinateV2::common(2),
                tail_offset: TAIL_OFFSET,
                row_scalar_stride: 2,
            },
            AccountOperationInputV2::ProjectNonzeroU64TailRows {
                account: AccountCoordinateV2::fixed(1),
                count_destination: ScalarCoordinateV2::common(0),
                rows_destination: ScalarCoordinateV2::common(2),
                tail_offset: TAIL_OFFSET + 1,
                row_scalar_stride: 2,
            },
        ] {
            let hostile_operations = [tail_projection, hostile_rows];
            let mut hostile_scratch = std::vec![0_u8; width];
            let mut hostile_output = std::vec![0x33_u8; width];
            let before = hostile_output.clone();
            assert_eq!(
                encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
                    TrustedEnvironmentV2::None,
                    TrustedIdentityEnvironmentV2::None,
                    &rules,
                    &[],
                    &hostile_operations,
                    &[],
                    registers,
                    &mut hostile_scratch,
                    &mut hostile_output,
                ),
                Err(Error::NonCanonicalOperation)
            );
            assert_eq!(hostile_output, before);
        }

        let item_geometry = RegisterGeometryV2 {
            item_scalar_stride: 1,
            ..registers
        };
        let mut hostile_scratch = std::vec![0_u8; width];
        let mut hostile_output = std::vec![0x22_u8; width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                &rules,
                &[],
                &operations,
                &[],
                item_geometry,
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::NonCanonicalOperation)
        );
        assert_eq!(hostile_output, before);

        let mut wrong_rules = rules;
        wrong_rules[1].prestate = AccountPrestateV2::Exact;
        let mut wrong_scratch = std::vec![0_u8; width];
        let mut wrong_output = std::vec![0x11_u8; width];
        let before = wrong_output.clone();
        assert_eq!(
            encode_account_profile_with_nonzero_u64_tail_rows_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                &wrong_rules,
                &[],
                &operations,
                &[],
                registers,
                &mut wrong_scratch,
                &mut wrong_output,
            ),
            Err(Error::InvalidVariableDataPrestate)
        );
        assert_eq!(wrong_output, before);
    }

    fn descriptor_with_coefficients(tail_offset: u32, coefficients: &[u64]) -> std::vec::Vec<u8> {
        let mut descriptor = std::vec![0_u8; usize::try_from(tail_offset).expect("tail offset")];
        for coefficient in coefficients {
            descriptor.extend_from_slice(&coefficient.to_le_bytes());
        }
        descriptor
    }

    #[test]
    fn authenticated_route_aliases_are_zero_privilege_logical_views() {
        let representative = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: WRITABLE,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 4,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let writable_alias = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::Fixed(0),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        };
        let readonly_alias = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                ..writable_alias.rule
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        };
        let system = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: AccountPrivilegesV2::new(false, false, true),
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let rules = [representative, writable_alias, readonly_alias, system];
        let registers = RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 4,
            item_identity_stride: 0,
        };
        let width = AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES + rules.len() * RULE_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_authenticated_route_alias_v2_atomic(
            TrustedEnvironmentV2::CurrentSlot { destination: 0 },
            TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
            TrustedBuiltinIdentityV2::SystemProgram { destination: 2 },
            &rules,
            &[],
            &[],
            &[],
            registers,
            &mut scratch,
            &mut encoded,
        )
        .expect("profile11");
        let profile = AccountProfileV2::decode(&encoded).expect("decode profile11");
        assert_eq!(
            profile.artifact_profile(),
            AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE
        );
        assert_eq!(profile.trusted_system_program_identity(), Some(2));
        assert_eq!(
            profile.writes_register(crate::v2::ProjectionTargetV2 {
                kind: crate::v2::ProjectionRegisterKindV2::Identity,
                space: crate::v2::ProjectionRegisterSpaceV2::Common,
                index: 2,
            }),
            Ok(true)
        );
        assert_eq!(profile.logical_account_count(0), Ok(4));
        assert_eq!(profile.physical_account_count(0), Ok(2));
        assert_eq!(profile.physical_account_ordinal(0, 0), Ok(0));
        assert_eq!(profile.physical_account_ordinal(0, 1), Ok(0));
        assert_eq!(profile.physical_account_ordinal(0, 2), Ok(0));
        assert_eq!(profile.physical_account_ordinal(0, 3), Ok(1));
        assert_eq!(profile.physical_representative_coordinate(0, 0), Ok(0));
        assert_eq!(profile.physical_representative_coordinate(0, 1), Ok(3));
        assert_eq!(
            profile.physical_representative_coordinate(0, 2),
            Err(Error::InvalidCoordinate)
        );
        let representative_geometry = profile
            .physical_account_geometry(0, 0)
            .expect("physical representative geometry");
        assert_eq!(representative_geometry.logical_representative(), 0);
        assert!(representative_geometry.privileges().writable());
        assert_eq!(
            representative_geometry.data(),
            PhysicalAccountDataGeometryV2::Exact { bytes: 4 }
        );
        assert_eq!(
            profile
                .physical_account_geometry(0, 1)
                .expect("system geometry")
                .data(),
            PhysicalAccountDataGeometryV2::Exact { bytes: 0 }
        );
        assert_eq!(
            profile.physical_account_geometry(0, 2),
            Err(Error::InvalidCoordinate)
        );
        assert!(
            profile
                .route_privileges(0, 0)
                .expect("representative route")
                .writable()
        );
        assert!(
            !profile
                .route_privileges(0, 1)
                .expect("logical alias")
                .writable()
        );
        assert!(
            !profile
                .route_privileges(0, 2)
                .expect("read route")
                .writable()
        );

        let body = [7_u8; 4];
        let representative_observation =
            AccountObservationV1::new([1; 32], [2; 32], 9, &body, false, true, false);
        let system_observation =
            AccountObservationV1::new([3; 32], [4; 32], 0, &[], false, false, true);
        let accounts = [
            representative_observation,
            representative_observation,
            representative_observation,
            system_observation,
        ];
        let input_scalars = [11_u64];
        let input_identities = [[5_u8; 32], [6; 32], [7; 32], [8; 32]];
        let mut scalar_scratch = [0_u64; 1];
        let mut identity_scratch = [[0_u8; 32]; 4];
        let mut output_scalars = [99_u64; 1];
        let mut output_identities = [[99_u8; 32]; 4];
        project_atomic(
            profile,
            0,
            &accounts,
            ProjectionRegistersV2 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scalar_scratch,
                scratch_identities: &mut identity_scratch,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("union-privileged physical representative");
        assert_eq!(output_scalars, input_scalars);
        assert_eq!(output_identities, input_identities);

        let assert_refuses = |hostile: &[AccountObservationV1<'_>], expected: Error| {
            let mut scalar_scratch = [0_u64; 1];
            let mut identity_scratch = [[0_u8; 32]; 4];
            let mut output_scalars = [99_u64; 1];
            let before_scalars = output_scalars;
            let mut output_identities = [[99_u8; 32]; 4];
            let before_identities = output_identities;
            assert_eq!(
                project_atomic(
                    profile,
                    0,
                    hostile,
                    ProjectionRegistersV2 {
                        input_scalars: &input_scalars,
                        input_identities: &input_identities,
                        scratch_scalars: &mut scalar_scratch,
                        scratch_identities: &mut identity_scratch,
                        output_scalars: &mut output_scalars,
                        output_identities: &mut output_identities,
                    },
                ),
                Err(expected)
            );
            assert_eq!(output_scalars, before_scalars);
            assert_eq!(output_identities, before_identities);
        };

        let readonly_physical =
            AccountObservationV1::new([1; 32], [2; 32], 9, &body, false, false, false);
        assert_refuses(
            &[
                readonly_physical,
                readonly_physical,
                readonly_physical,
                system_observation,
            ],
            Error::PrivilegeMismatch,
        );
        let substituted = AccountObservationV1::new([9; 32], [2; 32], 9, &body, false, true, false);
        assert_refuses(
            &[
                representative_observation,
                substituted,
                representative_observation,
                system_observation,
            ],
            Error::AliasMismatch,
        );
        let wrong_owner = AccountObservationV1::new([1; 32], [9; 32], 9, &body, false, true, false);
        assert_refuses(
            &[
                representative_observation,
                representative_observation,
                wrong_owner,
                system_observation,
            ],
            Error::AliasMismatch,
        );
        let other_body = [8_u8; 4];
        let wrong_data =
            AccountObservationV1::new([1; 32], [2; 32], 9, &other_body, false, true, false);
        assert_refuses(
            &[
                representative_observation,
                representative_observation,
                wrong_data,
                system_observation,
            ],
            Error::AliasMismatch,
        );
        let wrong_lamports =
            AccountObservationV1::new([1; 32], [2; 32], 10, &body, false, true, false);
        assert_refuses(
            &[
                representative_observation,
                wrong_lamports,
                representative_observation,
                system_observation,
            ],
            Error::AliasMismatch,
        );

        for privileges in [
            AccountPrivilegesV2::new(true, false, false),
            AccountPrivilegesV2::new(false, true, false),
            AccountPrivilegesV2::new(false, false, true),
        ] {
            let mut privileged_alias_rules = rules;
            privileged_alias_rules[1].rule.privileges = privileges;
            let mut hostile_scratch = std::vec![0_u8; width];
            let mut hostile_output = std::vec![0x55_u8; width];
            let before = hostile_output.clone();
            assert_eq!(
                encode_account_profile_with_authenticated_route_alias_v2_atomic(
                    TrustedEnvironmentV2::CurrentSlot { destination: 0 },
                    TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
                    TrustedBuiltinIdentityV2::SystemProgram { destination: 2 },
                    &privileged_alias_rules,
                    &[],
                    &[],
                    &[],
                    registers,
                    &mut hostile_scratch,
                    &mut hostile_output,
                ),
                Err(Error::InvalidRouteAlias)
            );
            assert_eq!(hostile_output, before);
        }

        let alias_offset = AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES + RULE_BYTES;
        for hostile_index in [1_u16, 2, 7] {
            let mut forward_alias = encoded.clone();
            forward_alias
                .get_mut(alias_offset + 4..alias_offset + 6)
                .expect("alias index")
                .copy_from_slice(&hostile_index.to_le_bytes());
            assert_eq!(
                AccountProfileV2::decode(&forward_alias),
                Err(Error::InvalidRouteAlias)
            );
        }

        let overwrite = [AccountOperationInputV2::ProjectKey {
            account: AccountCoordinateV2::fixed(3),
            destination: IdentityCoordinateV2::common(2),
        }];
        let overwrite_width = width + OPERATION_BYTES;
        let mut overwrite_scratch = std::vec![0_u8; overwrite_width];
        let mut overwrite_output = std::vec![0x77_u8; overwrite_width];
        let before = overwrite_output.clone();
        assert_eq!(
            encode_account_profile_with_authenticated_route_alias_v2_atomic(
                TrustedEnvironmentV2::CurrentSlot { destination: 0 },
                TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
                TrustedBuiltinIdentityV2::SystemProgram { destination: 2 },
                &rules,
                &[],
                &overwrite,
                &[],
                registers,
                &mut overwrite_scratch,
                &mut overwrite_output,
            ),
            Err(Error::TrustedBuiltinOverwrite)
        );
        assert_eq!(overwrite_output, before);

        for trusted_builtin in [
            TrustedBuiltinIdentityV2::SystemProgram { destination: 1 },
            TrustedBuiltinIdentityV2::SystemProgram { destination: 4 },
        ] {
            let mut hostile_scratch = std::vec![0_u8; width];
            let mut hostile_output = std::vec![0x88_u8; width];
            let before = hostile_output.clone();
            assert_eq!(
                encode_account_profile_with_authenticated_route_alias_v2_atomic(
                    TrustedEnvironmentV2::CurrentSlot { destination: 0 },
                    TrustedIdentityEnvironmentV2::CurrentExecutingProgram { destination: 1 },
                    trusted_builtin,
                    &rules,
                    &[],
                    &[],
                    &[],
                    registers,
                    &mut hostile_scratch,
                    &mut hostile_output,
                ),
                Err(Error::InvalidTrustedBuiltin)
            );
            assert_eq!(hostile_output, before);
        }
    }

    #[test]
    fn dynamic_spans_shift_aliases_protect_widths_and_admit_opaque_data() {
        let writable_data = AccountEffectPermissionsV2::new(false, false, true);
        let exact = |data_length| AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let fixed_rules = [
            exact(0),
            exact(0),
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: READONLY,
                    effect_permissions: writable_data,
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 4,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::Exact,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: READONLY,
                    effect_permissions: NO_EFFECTS,
                    alias: AccountAliasInputV2::Fixed(2),
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::AuthenticatedRouteAlias,
            },
        ];
        let opaque = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        };
        let operations = [AccountOperationInputV2::RequireOwner {
            account: AccountCoordinateV2::fixed(2),
            expected: IdentityCoordinateV2::common(0),
        }];
        let spans = [DynamicFixedSpanInputV2 {
            insertion_coordinate: 2,
            count_scalar: 0,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: 3,
            step: 1,
        }];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + (fixed_rules.len() + 1) * RULE_BYTES
            + operations.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &spans,
            &fixed_rules,
            &[opaque],
            &operations,
            RegisterGeometryV2 {
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut encoded,
        )
        .expect("profile13");
        let profile = AccountProfileV2::decode(&encoded).expect("decode profile13");
        assert_eq!(profile.dynamic_fixed_span_count(), 1);
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(9, &[2]),
            Ok(6)
        );
        assert_eq!(profile.representative_with_dynamic_spans(9, &[2], 5), Ok(4));
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(9, &[2]),
            Ok(5)
        );
        assert_eq!(
            profile.physical_account_ordinal_with_dynamic_spans(9, &[2], 5),
            Ok(4)
        );
        let opaque_geometry = profile
            .physical_account_geometry_with_dynamic_spans(9, &[2], 3)
            .expect("opaque physical geometry");
        assert_eq!(opaque_geometry.logical_representative(), 3);
        assert_eq!(
            opaque_geometry.data(),
            PhysicalAccountDataGeometryV2::Opaque
        );
        let ticket_geometry = profile
            .physical_account_geometry_with_dynamic_spans(9, &[2], 4)
            .expect("outer-writable physical geometry");
        assert_eq!(ticket_geometry.logical_representative(), 4);
        assert!(ticket_geometry.privileges().writable());
        assert_eq!(
            ticket_geometry.data(),
            PhysicalAccountDataGeometryV2::Exact { bytes: 4 }
        );
        assert!(
            !profile
                .route_privileges_with_dynamic_spans(9, &[2], 4)
                .expect("ticket route")
                .writable()
        );

        let owner = [0x44; 32];
        let ticket_key = [0x55; 32];
        let ticket_data = [1, 2, 3, 4];
        let opaque_data = [8, 9, 10];
        let accounts = [
            AccountObservationV1::new([1; 32], owner, 1, &[], false, false, false),
            AccountObservationV1::new([2; 32], owner, 2, &[], false, false, false),
            AccountObservationV1::new([3; 32], owner, 3, &[], false, false, false),
            AccountObservationV1::new([4; 32], owner, 4, &opaque_data, false, false, false),
            AccountObservationV1::new(ticket_key, owner, 5, &ticket_data, false, true, false),
            AccountObservationV1::new(ticket_key, owner, 5, &ticket_data, false, true, false),
        ];
        let input_scalars = [2_u64];
        let input_identities = [owner];
        let mut scratch_scalars = [0_u64];
        let mut output_scalars = [0_u64];
        let mut scratch_identities = [[0_u8; 32]];
        let mut output_identities = [[0_u8; 32]];
        project_dynamic_fixed_spans_atomic(
            profile,
            9,
            &[2],
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
        .expect("dynamic projection");
        assert_eq!(output_scalars, input_scalars);
        assert_eq!(output_identities, input_identities);
        let mut permissions = [dclutch_effect_kernel::v2::AccountPermission::read_only(); 6];
        derive_effect_permissions_with_dynamic_spans(profile, 9, &[2], &mut permissions)
            .expect("permissions");
        assert!(permissions[4].may_write_data());
        assert!(permissions[5].may_write_data());

        let mut widths = [0xaaaa_aaaa_u32];
        assert_eq!(
            profile.dynamic_span_widths_from_scalars(&[4], &mut widths),
            Err(Error::InvalidDynamicSpan)
        );
        assert_eq!(widths, [0xaaaa_aaaa]);
        let mut hostile_output_scalars = [77_u64];
        let mut hostile_output_identities = [[0x77_u8; 32]];
        assert_eq!(
            project_dynamic_fixed_spans_atomic(
                profile,
                9,
                &[1],
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut hostile_output_scalars,
                    output_identities: &mut hostile_output_identities,
                },
            ),
            Err(Error::WidthMismatch)
        );
        assert_eq!(hostile_output_scalars, [77]);
        assert_eq!(hostile_output_identities, [[0x77; 32]]);
    }

    #[test]
    fn dynamic_spans_admit_exact_runtime_width_lifecycle_state_atomically() {
        let lifecycle = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: WRITABLE,
                effect_permissions: AccountEffectPermissionsV2::new(false, true, true),
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 16,
                data_item_stride: 8,
            },
            prestate: AccountPrestateV2::LifecycleBound,
        };
        let opaque = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        };
        let count_source = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 4,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let operations = [AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
            data_offset: 0,
        }];
        let spans = [DynamicFixedSpanInputV2 {
            insertion_coordinate: 2,
            count_scalar: 1,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: 1,
            step: 1,
        }];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + 3 * RULE_BYTES
            + OPERATION_BYTES;
        let mut encode_scratch = std::vec![0_u8; width];
        let mut encoded = std::vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &spans,
            &[count_source, lifecycle],
            &[opaque],
            &operations,
            RegisterGeometryV2 {
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &mut encode_scratch,
            &mut encoded,
        )
        .expect("runtime-width lifecycle Profile13");
        let profile = AccountProfileV2::decode(&encoded).expect("decode Profile13");
        assert_eq!(
            profile
                .physical_account_geometry_with_dynamic_spans(3, &[1], 1)
                .expect("runtime-width lifecycle geometry")
                .data(),
            PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: 40 }
        );

        let owner = [0x22; 32];
        let opaque_data = [0x55];
        let count_data = 3_u32.to_le_bytes();
        let project = |state_data: &[u8], output: &mut [u64; 2]| {
            let accounts = [
                AccountObservationV1::new([3; 32], owner, 1, &count_data, false, false, false),
                AccountObservationV1::new([1; 32], owner, 9, state_data, false, true, false),
                AccountObservationV1::new([2; 32], owner, 1, &opaque_data, false, false, false),
            ];
            let input = [7_u64, 1];
            let mut scratch_scalars = [0_u64; 2];
            project_dynamic_fixed_spans_atomic(
                profile,
                3,
                &[1],
                &accounts,
                ProjectionRegistersV2 {
                    input_scalars: &input,
                    input_identities: &[],
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut [],
                    output_scalars: output,
                    output_identities: &mut [],
                },
            )
        };

        let mut vacant_output = [9_u64; 2];
        project(&[], &mut vacant_output).expect("vacant lifecycle prestate");
        assert_eq!(vacant_output, [3, 1]);

        let mut live = [0_u8; 40];
        live.get_mut(8..16)
            .expect("projected field")
            .copy_from_slice(&41_u64.to_le_bytes());
        let mut live_output = [9_u64; 2];
        project(&live, &mut live_output).expect("exact runtime-width live prestate");
        assert_eq!(live_output, [3, 1]);

        for hostile in [&live[..39], &live[..]] {
            let oversized = if hostile.len() == live.len() {
                Some(std::vec![0_u8; 41])
            } else {
                None
            };
            let hostile = oversized.as_deref().unwrap_or(hostile);
            let mut output = [99_u64; 2];
            let before = output;
            assert_eq!(
                project(hostile, &mut output),
                Err(Error::DataLengthMismatch)
            );
            assert_eq!(output, before);
        }

        assert_eq!(
            profile.physical_account_geometry_with_dynamic_spans(u32::MAX, &[u32::MAX], 1),
            Err(Error::InvalidDynamicSpan)
        );

        let legacy_width = HEADER_BYTES + RULE_BYTES;
        let mut legacy_scratch = std::vec![0_u8; legacy_width];
        let mut legacy_output = std::vec![0x99_u8; legacy_width];
        let legacy_before = legacy_output.clone();
        assert_eq!(
            encode_account_profile_with_lifecycle_v2_atomic(
                TrustedEnvironmentV2::None,
                &[lifecycle],
                &[],
                &[],
                &[],
                RegisterGeometryV2 {
                    common_scalars: 1,
                    item_scalar_stride: 0,
                    common_identities: 0,
                    item_identity_stride: 0,
                },
                &mut legacy_scratch,
                &mut legacy_output,
            ),
            Err(Error::InvalidLifecyclePrestate)
        );
        assert_eq!(legacy_output, legacy_before);
    }

    #[test]
    fn generated_dynamic_rules_are_exact_and_failure_atomic() {
        let exact = |data_length| AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let fixed = [exact(3), exact(5), exact(7)];
        let span_rules = [AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        }];
        let spans = [DynamicFixedSpanInputV2 {
            insertion_coordinate: 2,
            count_scalar: 0,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: 4,
            step: 1,
        }];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + (fixed.len() + span_rules.len()) * RULE_BYTES;
        let registers = RegisterGeometryV2 {
            common_scalars: 1,
            item_scalar_stride: 0,
            common_identities: 0,
            item_identity_stride: 0,
        };
        let mut slice_scratch = std::vec![0_u8; width];
        let mut slice_output = std::vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &spans,
            &fixed,
            &span_rules,
            &[],
            registers,
            &mut slice_scratch,
            &mut slice_output,
        )
        .expect("slice profile13");

        let mut next_coordinate = 0_u16;
        let mut generated_scratch = std::vec![0_u8; width];
        let mut generated_output = std::vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &spans,
            u16::try_from(fixed.len()).expect("bounded fixture"),
            |coordinate| {
                assert_eq!(coordinate, next_coordinate);
                next_coordinate = next_coordinate.checked_add(1).expect("bounded fixture");
                fixed
                    .get(usize::from(coordinate))
                    .copied()
                    .ok_or(Error::InvalidLength)
            },
            &span_rules,
            &[],
            registers,
            &mut generated_scratch,
            &mut generated_output,
        )
        .expect("generated profile13");
        assert_eq!(next_coordinate, 3);
        assert_eq!(generated_output, slice_output);

        let mut hostile_scratch = std::vec![0_u8; width];
        let mut hostile_output = std::vec![0x5a_u8; width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_dynamic_fixed_span_v2_generated_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                TrustedBuiltinIdentityV2::None,
                &spans,
                u16::try_from(fixed.len()).expect("bounded fixture"),
                |coordinate| {
                    if coordinate == 1 {
                        return Err(Error::InvalidLength);
                    }
                    fixed
                        .get(usize::from(coordinate))
                        .copied()
                        .ok_or(Error::InvalidLength)
                },
                &span_rules,
                &[],
                registers,
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidLength)
        );
        assert_eq!(hostile_output, before);
    }

    #[test]
    fn dynamic_spans_are_orthogonal_to_product_item_register_geometry() {
        let fixed = [AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 4,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        }];
        let span_rules = [AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        }];
        let spans = [DynamicFixedSpanInputV2 {
            insertion_coordinate: 1,
            count_scalar: 0,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: 1,
            step: 1,
        }];
        let operations = [AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(1),
            data_offset: 0,
        }];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + (fixed.len() + span_rules.len()) * RULE_BYTES
            + operations.len() * OPERATION_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &spans,
            &fixed,
            &span_rules,
            &operations,
            RegisterGeometryV2 {
                common_scalars: 2,
                item_scalar_stride: 1,
                common_identities: 1,
                item_identity_stride: 1,
            },
            &mut scratch,
            &mut output,
        )
        .expect("dynamic accounts with Product item registers");
        let profile = AccountProfileV2::decode(&output).expect("Profile13");
        let tail_count_bytes = 2_u32.to_le_bytes();
        let accounts = [
            AccountObservationV1::new([1; 32], [2; 32], 1, &tail_count_bytes, false, false, false),
            AccountObservationV1::new([3; 32], [4; 32], 1, &[], false, false, false),
        ];
        let input_scalars = [1_u64, 0, 99, 99];
        let input_identities = [[5_u8; 32]; 3];
        let mut scratch_scalars = [0_u64; 4];
        let mut output_scalars = [0_u64; 4];
        let mut scratch_identities = [[0_u8; 32]; 3];
        let mut output_identities = [[0_u8; 32]; 3];
        project_dynamic_fixed_spans_atomic(
            profile,
            2,
            &[1],
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
        .expect("orthogonal projection");
        assert_eq!(output_scalars, [1, 2, 0, 1]);
        assert_eq!(output_identities, input_identities);
    }

    #[test]
    fn dynamic_span_table_is_stable_congruent_and_failure_atomic() {
        let fixed = [AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        }; 2];
        let opaque = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        };
        let span_rules = [opaque; 15];
        let spans = [
            DynamicFixedSpanInputV2 {
                insertion_coordinate: 1,
                count_scalar: 0,
                rule_start: 0,
                rule_stride: 14,
                minimum: 0,
                maximum: 14,
                step: 14,
            },
            DynamicFixedSpanInputV2 {
                insertion_coordinate: 1,
                count_scalar: 1,
                rule_start: 14,
                rule_stride: 1,
                minimum: 1,
                maximum: 2,
                step: 1,
            },
        ];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + spans.len() * DYNAMIC_FIXED_SPAN_ENTRY_BYTES
            + (fixed.len() + span_rules.len()) * RULE_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &spans,
            &fixed,
            &span_rules,
            &[],
            RegisterGeometryV2 {
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut output,
        )
        .expect("equal insertions preserve declaration order");
        let profile = AccountProfileV2::decode(&output).expect("decode multi-span");
        let mut selected = [99_u32; 2];
        assert_eq!(
            profile.dynamic_span_widths_from_scalars(&[7, 1], &mut selected),
            Err(Error::InvalidDynamicSpan)
        );
        assert_eq!(selected, [99, 99]);
        profile
            .dynamic_span_widths_from_scalars(&[14, 2], &mut selected)
            .expect("finite congruence");
        assert_eq!(selected, [14, 2]);
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(0, &selected),
            Ok(18)
        );

        let mut unsorted = spans;
        unsorted[0].insertion_coordinate = 2;
        let mut hostile_scratch = std::vec![0_u8; width];
        let mut hostile_output = std::vec![0x66_u8; width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_dynamic_fixed_span_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                TrustedBuiltinIdentityV2::None,
                &unsorted,
                &fixed,
                &span_rules,
                &[],
                RegisterGeometryV2 {
                    common_scalars: 2,
                    item_scalar_stride: 0,
                    common_identities: 1,
                    item_identity_stride: 0,
                },
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidDynamicSpan)
        );
        assert_eq!(hostile_output, before);

        let opaque_fixed = [opaque, fixed[1]];
        let data_projection = [AccountOperationInputV2::ProjectDataU8 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(2),
            data_offset: 0,
        }];
        let operation_width = width + OPERATION_BYTES;
        let mut hostile_scratch = std::vec![0_u8; operation_width];
        let mut hostile_output = std::vec![0x77_u8; operation_width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_dynamic_fixed_span_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                TrustedBuiltinIdentityV2::None,
                &spans,
                &opaque_fixed,
                &span_rules,
                &data_projection,
                RegisterGeometryV2 {
                    common_scalars: 3,
                    item_scalar_stride: 0,
                    common_identities: 1,
                    item_identity_stride: 0,
                },
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidOpaqueDataPrestate)
        );
        assert_eq!(hostile_output, before);

        let count_overwrite = [AccountOperationInputV2::ProjectLamports {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
        }];
        let mut hostile_scratch = std::vec![0_u8; operation_width];
        let mut hostile_output = std::vec![0x88_u8; operation_width];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_dynamic_fixed_span_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                TrustedBuiltinIdentityV2::None,
                &spans,
                &fixed,
                &span_rules,
                &count_overwrite,
                RegisterGeometryV2 {
                    common_scalars: 2,
                    item_scalar_stride: 0,
                    common_identities: 1,
                    item_identity_stride: 0,
                },
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidDynamicSpan)
        );
        assert_eq!(hostile_output, before);
    }

    #[test]
    fn dynamic_span_profile_admits_canonical_fixed_topology() {
        let fixed = [
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: READONLY,
                    effect_permissions: NO_EFFECTS,
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 0,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
            },
            AccountRuleWithPrestateInputV2 {
                rule: AccountRuleInputV2 {
                    privileges: READONLY,
                    effect_permissions: NO_EFFECTS,
                    alias: AccountAliasInputV2::SelfCoordinate,
                    data_length: 32,
                    data_item_stride: 0,
                },
                prestate: AccountPrestateV2::Exact,
            },
        ];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES + fixed.len() * RULE_BYTES;
        let mut scratch = std::vec![0_u8; width];
        let mut output = std::vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::None,
            &[],
            &fixed,
            &[],
            &[],
            RegisterGeometryV2 {
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut output,
        )
        .expect("fixed-topology profile13");
        let profile = AccountProfileV2::decode(&output).expect("decode fixed profile13");
        assert_eq!(profile.dynamic_fixed_span_count(), 0);
        assert_eq!(
            profile.logical_account_count_with_dynamic_spans(99, &[]),
            Ok(2)
        );
        assert_eq!(
            profile.physical_account_count_with_dynamic_spans(99, &[]),
            Ok(2)
        );
        assert_eq!(
            profile
                .physical_account_geometry_with_dynamic_spans(99, &[], 0)
                .expect("opaque geometry")
                .data(),
            PhysicalAccountDataGeometryV2::Opaque
        );

        let mut hostile_scratch = std::vec![0_u8; width + RULE_BYTES];
        let mut hostile_output = std::vec![0x91_u8; width + RULE_BYTES];
        let before = hostile_output.clone();
        assert_eq!(
            encode_account_profile_with_dynamic_fixed_span_v2_atomic(
                TrustedEnvironmentV2::None,
                TrustedIdentityEnvironmentV2::None,
                TrustedBuiltinIdentityV2::None,
                &[],
                &fixed,
                &fixed[..1],
                &[],
                RegisterGeometryV2 {
                    common_scalars: 1,
                    item_scalar_stride: 0,
                    common_identities: 1,
                    item_identity_stride: 0,
                },
                &mut hostile_scratch,
                &mut hostile_output,
            ),
            Err(Error::InvalidDynamicSpan)
        );
        assert_eq!(hostile_output, before);
    }
}
