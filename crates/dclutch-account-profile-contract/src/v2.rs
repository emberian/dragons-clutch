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
/// Exact V2 header width.
pub const HEADER_BYTES: usize = 32;
/// Exact account-rule width.
pub const RULE_BYTES: usize = 16;
/// Exact projection-operation width.
pub const OPERATION_BYTES: usize = 16;

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
}

/// Result alias for runtime-tail profiles.
pub type Result<T> = core::result::Result<T, Error>;

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

/// Hostile-decoded account rule template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRuleV2 {
    privileges: u8,
    effect_permissions: u8,
    alias_kind: AliasKindV2,
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
                ARTIFACT_PROFILE | SELECTED_WINDOW_ARTIFACT_PROFILE
            )
        {
            return Err(Error::UnsupportedProfile);
        }
        if read_u32(bytes, 28)? != 0 {
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
            bytes,
        };
        if value.fixed_accounts == 0
            || (value.common_scalars == 0
                && value.item_scalar_stride == 0
                && value.common_identities == 0
                && value.item_identity_stride == 0)
            || (value.item_account_stride != 0 && value.item_scalar_stride == 0)
        {
            return Err(Error::EmptyProfile);
        }
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
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        value.validate_rules()?;
        value.validate_operations()?;
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

    /// Borrow complete canonical profile bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
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
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        decode_rule(self.bytes, offset)
    }

    /// Resolve one expanded physical account to its representative coordinate.
    pub fn representative(self, tail_count: u32, coordinate: usize) -> Result<usize> {
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
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        Operation::decode(self.bytes, offset)
    }

    fn validate_rules(self) -> Result<()> {
        let mut fixed = 0_u16;
        while fixed < self.fixed_accounts {
            let rule = self.rule(false, fixed)?;
            if self.artifact_profile == ARTIFACT_PROFILE && rule.data_item_stride != 0 {
                return Err(Error::NonCanonicalReserved);
            }
            validate_rule(rule, false, fixed, self.fixed_accounts)?;
            self.require_owner_anchor(false, fixed, rule)?;
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_account_stride {
            let rule = self.rule(true, item)?;
            if self.artifact_profile == ARTIFACT_PROFILE && rule.data_item_stride != 0 {
                return Err(Error::NonCanonicalReserved);
            }
            validate_rule(rule, true, item, self.fixed_accounts)?;
            self.require_owner_anchor(true, item, rule)?;
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }

    fn require_owner_anchor(self, item: bool, account: u16, rule: AccountRuleV2) -> Result<()> {
        if rule.effect_permissions
            & (EFFECT_PERMISSION_DEBIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA)
            == 0
        {
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
            self.require_unique_projection(false, fixed, operation)?;
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_operations {
            let operation = self.operation(true, item)?;
            operation.validate(self, true, item)?;
            self.require_unique_projection(true, item, operation)?;
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
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
        let affine = self.item_account_stride != 0
            || self.item_operations != 0
            || self.item_scalar_stride != 0
            || self.item_identity_stride != 0
            || self.has_affine_data_length()?;
        if self.tail_count_projection()?.is_some() == affine {
            Ok(())
        } else {
            Err(Error::NonCanonicalOperation)
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
        let Some(target) = operation.projection_target()? else {
            return Ok(());
        };
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
            if self.operation(item, prior)?.projection_target()? == Some(target) {
                return Err(Error::DuplicateProjection);
            }
            prior = prior.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }
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
        let fixed = usize::from(profile.fixed_accounts);
        let rule = if coordinate < fixed {
            profile.rule(
                false,
                u16::try_from(coordinate).map_err(|_| Error::InvalidCoordinate)?,
            )?
        } else {
            let stride = usize::from(profile.item_account_stride);
            let local = coordinate
                .checked_sub(fixed)
                .ok_or(Error::InvalidCoordinate)?
                % stride;
            profile.rule(
                true,
                u16::try_from(local).map_err(|_| Error::InvalidCoordinate)?,
            )?
        };
        *permission = rule.permission();
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
        if account.privileges() != rule.privileges {
            return Err(Error::PrivilegeMismatch);
        }
        if account.data().len() != exact_rule_data_length(rule, tail_count)? {
            return Err(Error::DataLengthMismatch);
        }
        let representative = profile.representative(tail_count, coordinate)?;
        let canonical = accounts
            .get(representative)
            .copied()
            .ok_or(Error::InvalidCoordinate)?;
        if account.key() != canonical.key()
            || account.owner() != canonical.owner()
            || account.lamports() != canonical.lamports()
            || account.data() != canonical.data()
            || account.privileges() != canonical.privileges()
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

fn inject_indices(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    scalars: &mut [u64],
) -> Result<()> {
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
            | OP_PROJECT_TAIL_COUNT_U32 => {
                if self.data_stride != 0 {
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
                if profile.artifact_profile != SELECTED_WINDOW_ARTIFACT_PROFILE
                    || item_body
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
                if profile.artifact_profile != SELECTED_WINDOW_ARTIFACT_PROFILE
                    || item_body
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
                | OP_PROJECT_TAIL_COUNT_U32
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

    fn is_selected_affine_projection(self) -> bool {
        matches!(
            self.opcode,
            OP_PROJECT_DATA_U64_SELECTED_AFFINE | OP_PROJECT_DATA_IDENTITY_SELECTED_AFFINE
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        self,
        profile: AccountProfileV2<'_>,
        item: Option<u32>,
        tail_count: u32,
        accounts: &[AccountObservationV1<'_>],
        input_identities: &[[u8; 32]],
        scalars: &mut [u64],
        identities: &mut [[u8; 32]],
    ) -> Result<()> {
        let account_index = if self.account_item {
            item_account_index(profile, item.ok_or(Error::InvalidCoordinate)?, self.account)?
        } else {
            usize::from(self.account)
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
                let bytes = data_field(account.data(), self.data_offset, 8)?;
                write_scalar(
                    scalars,
                    scalar()?,
                    u64::from_le_bytes(bytes.try_into().map_err(|_| Error::DataOutOfBounds)?),
                )
            }
            OP_PROJECT_DATA_U32 | OP_PROJECT_TAIL_COUNT_U32 => {
                let bytes = data_field(account.data(), self.data_offset, 4)?;
                write_scalar(
                    scalars,
                    scalar()?,
                    u64::from(u32::from_le_bytes(
                        bytes.try_into().map_err(|_| Error::DataOutOfBounds)?,
                    )),
                )
            }
            OP_PROJECT_DATA_IDENTITY => {
                let bytes = data_field(account.data(), self.data_offset, 32)?;
                write_identity(
                    identities,
                    identity()?,
                    bytes.try_into().map_err(|_| Error::DataOutOfBounds)?,
                )
            }
            OP_PROJECT_DATA_U64_AFFINE => {
                let offset = affine_data_offset(
                    self.data_offset,
                    self.data_stride,
                    item.ok_or(Error::InvalidCoordinate)?,
                    8,
                )?;
                let bytes = data_field(account.data(), offset, 8)?;
                write_scalar(
                    scalars,
                    scalar()?,
                    u64::from_le_bytes(bytes.try_into().map_err(|_| Error::DataOutOfBounds)?),
                )
            }
            OP_PROJECT_DATA_IDENTITY_AFFINE => {
                let offset = affine_data_offset(
                    self.data_offset,
                    self.data_stride,
                    item.ok_or(Error::InvalidCoordinate)?,
                    32,
                )?;
                let bytes = data_field(account.data(), offset, 32)?;
                write_identity(
                    identities,
                    identity()?,
                    bytes.try_into().map_err(|_| Error::DataOutOfBounds)?,
                )
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

fn validate_rule(rule: AccountRuleV2, item: bool, index: u16, fixed_count: u16) -> Result<()> {
    if rule.privileges & !0x07 != 0 {
        return Err(Error::InvalidPrivileges);
    }
    if rule.effect_permissions
        & !(EFFECT_PERMISSION_DEBIT_LAMPORTS
            | EFFECT_PERMISSION_CREDIT_LAMPORTS
            | EFFECT_PERMISSION_WRITE_DATA)
        != 0
        || (rule.effect_permissions != 0 && rule.privileges & 0x02 == 0)
    {
        return Err(Error::InvalidEffectPermissions);
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

fn decode_rule(bytes: &[u8], offset: usize) -> Result<AccountRuleV2> {
    let alias_kind = match byte(bytes, add(offset, 2)?)? {
        0 => AliasKindV2::SelfCoordinate,
        1 => AliasKindV2::Fixed,
        2 => AliasKindV2::SameItem,
        _ => return Err(Error::InvalidAlias),
    };
    if byte(bytes, add(offset, 3)?)? != 0 || read_u16(bytes, add(offset, 6)?)? != 0 {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(AccountRuleV2 {
        privileges: byte(bytes, offset)?,
        effect_permissions: byte(bytes, add(offset, 1)?)?,
        alias_kind,
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
    affine_width(profile.fixed_accounts, profile.item_account_stride, count)
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
            assert!(project_atomic(
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
            .is_err());
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
