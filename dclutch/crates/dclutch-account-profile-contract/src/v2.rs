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
        if read_u16(bytes, 8)? != VERSION || read_u16(bytes, 10)? != ARTIFACT_PROFILE {
            return Err(Error::UnsupportedProfile);
        }
        if read_u32(bytes, 28)? != 0 {
            return Err(Error::NonCanonicalReserved);
        }
        let value = Self {
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
        Ok(value)
    }

    /// Fixed-prefix account count.
    pub const fn fixed_account_count(self) -> u16 {
        self.fixed_accounts
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
            validate_rule(rule, false, fixed, self.fixed_accounts)?;
            self.require_owner_anchor(false, fixed, rule)?;
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_account_stride {
            let rule = self.rule(true, item)?;
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
            operation.validate(self, false)?;
            self.require_unique_projection(false, fixed, operation)?;
            fixed = fixed.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let mut item = 0_u16;
        while item < self.item_operations {
            let operation = self.operation(true, item)?;
            operation.validate(self, true)?;
            self.require_unique_projection(true, item, operation)?;
            item = item.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
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
        if account.data().len()
            != usize::try_from(rule.data_length).map_err(|_| Error::DataLengthMismatch)?
        {
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
}

impl Operation {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        let account_space = byte(bytes, add(offset, 1)?)?;
        let register_space = byte(bytes, add(offset, 4)?)?;
        if account_space > 1
            || register_space > 1
            || byte(bytes, add(offset, 5)?)? != 0
            || read_u32(bytes, add(offset, 12)?)? != 0
        {
            return Err(Error::NonCanonicalOperation);
        }
        Ok(Self {
            opcode: byte(bytes, offset)?,
            account_item: account_space == 1,
            account: read_u16(bytes, add(offset, 2)?)?,
            register_item: register_space == 1,
            register: read_u16(bytes, add(offset, 6)?)?,
            data_offset: read_u32(bytes, add(offset, 8)?)?,
        })
    }

    fn validate(self, profile: AccountProfileV2<'_>, item_body: bool) -> Result<()> {
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
                if self.data_offset != 0 {
                    return Err(Error::NonCanonicalOperation);
                }
            }
            OP_PROJECT_DATA_U64 | OP_PROJECT_DATA_IDENTITY => {}
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
        )
    }

    fn projection_target(self) -> Result<Option<(bool, bool, u16)>> {
        if !self.is_projection() {
            return Ok(None);
        }
        let identity = matches!(
            self.opcode,
            OP_PROJECT_KEY | OP_PROJECT_OWNER | OP_PROJECT_DATA_IDENTITY
        );
        Ok(Some((identity, self.register_item, self.register)))
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        self,
        profile: AccountProfileV2<'_>,
        item: Option<u32>,
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
            OP_PROJECT_DATA_IDENTITY => {
                let bytes = data_field(account.data(), self.data_offset, 32)?;
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
    if byte(bytes, add(offset, 3)?)? != 0
        || read_u16(bytes, add(offset, 6)?)? != 0
        || read_u32(bytes, add(offset, 12)?)? != 0
    {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(AccountRuleV2 {
        privileges: byte(bytes, offset)?,
        effect_permissions: byte(bytes, add(offset, 1)?)?,
        alias_kind,
        alias_index: read_u16(bytes, add(offset, 4)?)?,
        data_length: read_u32(bytes, add(offset, 8)?)?,
    })
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

    fn rule() -> [u8; RULE_BYTES] {
        [0_u8; RULE_BYTES]
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

    fn profile_bytes() -> Vec<u8> {
        let rules = [rule(), rule(), rule()];
        let operations = [
            operation(OP_REQUIRE_KEY, false, 0, false, 0),
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
            (16, 1),
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

    fn observations(duplicate_across_items: bool) -> Vec<AccountObservationV1<'static>> {
        let second = if duplicate_across_items {
            [0x21; 32]
        } else {
            [0x31; 32]
        };
        vec![
            AccountObservationV1::new([0x11; 32], [1; 32], 1, &[], false, false, false),
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
        assert_eq!(output_scalars, [0, 0, 3, 1, 5]);
        assert_eq!(output_identities, [[0x11; 32], [0x21; 32], [0x31; 32]]);
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
}
