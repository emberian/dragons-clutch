//! Safe, allocation-free AccountProfile V2 artifact encoder.
//!
//! Public typed inputs retain account-space, register-space, alias, privilege,
//! permission, and operation-tag authority in this semantic-owner crate. The
//! encoder hostile-decodes the complete scratch candidate before copying it to
//! output.

use super::{
    ARTIFACT_PROFILE, AccountPrestateV2, AccountProfileV2, Error, HEADER_BYTES,
    LIFECYCLE_PRESTATE_ARTIFACT_PROFILE, MAGIC, OP_PROJECT_DATA_IDENTITY,
    OP_PROJECT_DATA_IDENTITY_AFFINE, OP_PROJECT_DATA_IDENTITY_SELECTED,
    OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE, OP_PROJECT_DATA_U16, OP_PROJECT_DATA_U32,
    OP_PROJECT_DATA_U64, OP_PROJECT_DATA_U64_AFFINE, OP_PROJECT_DATA_U64_SELECTED,
    OP_PROJECT_DATA_U64_SELECTED_AFFINE, OP_PROJECT_KEY, OP_PROJECT_LAMPORTS, OP_PROJECT_OWNER,
    OP_PROJECT_TAIL_COUNT_U32, OP_REQUIRE_KEY, OP_REQUIRE_OWNER, OP_SELECT_DATA_WINDOW,
    OPERATION_BYTES, RULE_BYTES, SELECTED_WINDOW_ARTIFACT_PROFILE,
    TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE, TRUSTED_ENVIRONMENT_CURRENT_SLOT,
    TRUSTED_ENVIRONMENT_KIND_OFFSET, TRUSTED_ENVIRONMENT_SCALAR_OFFSET,
    TYPED_SCALAR_ARTIFACT_PROFILE, TrustedEnvironmentV2, VERSION,
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
}

impl AccountProfileArtifactV2 {
    const fn value(self) -> u16 {
        match self {
            Self::RuntimeTail => ARTIFACT_PROFILE,
            Self::SelectedWindow => SELECTED_WINDOW_ARTIFACT_PROFILE,
            Self::TypedScalar => TYPED_SCALAR_ARTIFACT_PROFILE,
            Self::TrustedEnvironment => TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE,
            Self::LifecyclePrestate => LIFECYCLE_PRESTATE_ARTIFACT_PROFILE,
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
        RuleInputsV2::WithPrestate(fixed_rules),
        RuleInputsV2::WithPrestate(item_rules),
        fixed_operations,
        item_operations,
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

impl RuleInputsV2<'_> {
    fn len(self) -> usize {
        match self {
            Self::Exact(values) => values.len(),
            Self::WithPrestate(values) => values.len(),
        }
    }

    fn get(self, index: usize) -> Result<(AccountRuleInputV2, AccountPrestateV2), Error> {
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

#[allow(clippy::too_many_arguments)]
fn encode_account_profile_atomic(
    artifact: AccountProfileArtifactV2,
    trusted_environment: TrustedEnvironmentV2,
    fixed_rules: RuleInputsV2<'_>,
    item_rules: RuleInputsV2<'_>,
    fixed_operations: &[AccountOperationInputV2],
    item_operations: &[AccountOperationInputV2],
    registers: RegisterGeometryV2,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    if trusted_environment.current_slot_destination().is_some()
        && !matches!(
            artifact,
            AccountProfileArtifactV2::TrustedEnvironment
                | AccountProfileArtifactV2::LifecyclePrestate
        )
    {
        return Err(Error::InvalidTrustedEnvironment);
    }
    let fixed_account_count = u16::try_from(fixed_rules.len()).map_err(|_| Error::InvalidLength)?;
    let item_account_stride = u16::try_from(item_rules.len()).map_err(|_| Error::InvalidLength)?;
    let fixed_operation_count =
        u16::try_from(fixed_operations.len()).map_err(|_| Error::InvalidLength)?;
    let item_operation_count =
        u16::try_from(item_operations.len()).map_err(|_| Error::InvalidLength)?;
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
        .and_then(|body| HEADER_BYTES.checked_add(body))
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
    let mut cursor = HEADER_BYTES;
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
    use crate::v2::{ProjectionRegistersV2, TRUSTED_ENVIRONMENT_RESERVED_OFFSET, project_atomic};

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
}
