#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-memory interpreter for content-selected account projections.
//!
//! This crate receives plain account observations from an outer runtime
//! adapter. It does not inspect Solana accounts, hash content, derive PDAs, or
//! authenticate Registry records. The adapter must authenticate the complete
//! profile bytes to `authenticated_profile_id`; the caller's already admitted
//! capability descriptor supplies `selected_profile_id`. Decoding requires the
//! two nonzero identities to be exact.

use core::convert::{TryFrom, TryInto};

use dclutch_effect_kernel::v2::AccountPermission;
use dclutch_transition_vm::v2::{RegisterInput, RegisterOutput};

/// Data-defined Trading-owned PDA derivation and state lifecycle policy.
pub mod lifecycle_v3;
/// Runtime-tail account and register projection profiles.
pub mod v2;

#[rustfmt::skip]
#[allow(missing_docs)]
mod generated;

pub use generated::{
    ACCOUNT_PROFILE_ARTIFACT_PROFILE_V1, ACCOUNT_PROFILE_HEADER_BYTES_V1, ACCOUNT_PROFILE_MAGIC_V1,
    ACCOUNT_PROFILE_MAX_BYTES_V1, ACCOUNT_PROFILE_OPERATION_BYTES_V1,
    ACCOUNT_PROFILE_RULE_BYTES_V1, ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1,
    ACCOUNT_PROFILE_SCHEMA_RELEASE_PREIMAGE_V1, ACCOUNT_PROFILE_SCHEMA_VERSION_V1,
    EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
    EFFECT_PERMISSION_WRITE_DATA, GENERAL_ACTIVATION_ACCOUNT_PROFILE_ID_V1,
};

use generated::{
    ACCOUNT_OPERATION_ACCOUNT_OFFSET, ACCOUNT_OPERATION_DATA_OFFSET,
    ACCOUNT_OPERATION_OPCODE_OFFSET, ACCOUNT_OPERATION_REGISTER_OFFSET,
    ACCOUNT_OPERATION_RESERVED_BYTE_OFFSET, ACCOUNT_OPERATION_RESERVED_OFFSET,
    ACCOUNT_OPERATION_RESERVED_SHORT_OFFSET, ACCOUNT_PROFILE_ACCOUNT_COUNT_OFFSET,
    ACCOUNT_PROFILE_ARTIFACT_OFFSET, ACCOUNT_PROFILE_IDENTITY_COUNT_OFFSET,
    ACCOUNT_PROFILE_MAGIC_OFFSET, ACCOUNT_PROFILE_OPERATION_COUNT_OFFSET,
    ACCOUNT_PROFILE_RESERVED_OFFSET, ACCOUNT_PROFILE_SCALAR_COUNT_OFFSET,
    ACCOUNT_PROFILE_VERSION_OFFSET, ACCOUNT_RULE_ALIAS_OF_OFFSET, ACCOUNT_RULE_DATA_LENGTH_OFFSET,
    ACCOUNT_RULE_EFFECT_PERMISSIONS_OFFSET, ACCOUNT_RULE_PRIVILEGES_OFFSET,
    ACCOUNT_RULE_RESERVED_OFFSET, OP_PROJECT_DATA_IDENTITY, OP_PROJECT_DATA_U64, OP_PROJECT_KEY,
    OP_PROJECT_LAMPORTS, OP_PROJECT_OWNER, OP_REQUIRE_KEY_EQ_IDENTITY,
    OP_REQUIRE_OWNER_EQ_IDENTITY,
};

/// Stable hostile-decode, account-admission, or projection refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Profile bytes did not have their exact count-derived width.
    InvalidLength,
    /// Magic did not identify an account profile.
    InvalidMagic,
    /// The schema version is not implemented.
    UnsupportedSchema,
    /// The artifact profile is not implemented.
    UnsupportedArtifactProfile,
    /// A reserved byte or unused operation field was nonzero.
    NonCanonicalReservedBytes,
    /// The profile contained no account, no operation, or no register bank.
    EmptyProfile,
    /// The selected or authenticated content identity was zero.
    ZeroProfileIdentity,
    /// The authenticated profile content was not the descriptor-selected content.
    ProfileIdentityMismatch,
    /// An account privilege bit outside signer/writable/executable was set.
    InvalidPrivileges,
    /// An effect-permission bit outside debit/credit/data-write was set.
    InvalidEffectPermissions,
    /// An effect permission selected a runtime-readonly account.
    EffectRequiresWritable,
    /// Debit or data-write authority lacked an authenticated owner relation.
    EffectOwnerUnanchored,
    /// The alias representative was forward, out of range, or noncanonical.
    InvalidAlias,
    /// The profile contained an unknown operation tag.
    UnknownOperation,
    /// An operation used a nonzero field outside its selected shape.
    NonCanonicalOperation,
    /// An operation selected an account outside the declared suffix.
    InvalidAccountIndex,
    /// An operation selected a register outside the declared bank.
    InvalidRegister,
    /// Two projections targeted the same output register.
    DuplicateProjection,
    /// A projection attempted to overwrite an identity used as authenticated authority.
    AuthorityOverwrite,
    /// A distinct alias representative had no key/owner relation to authenticated input.
    UnanchoredAccount,
    /// The supplied suffix had another account count.
    AccountCountMismatch,
    /// Input, scratch, or output banks had another exact runtime width.
    RegisterWidthMismatch,
    /// An account did not have the profile's exact signer/writable/executable tuple.
    PrivilegeMismatch,
    /// Account keys did not realize the profile's exact canonical alias partition.
    AliasMismatch,
    /// Two aliases exposed inconsistent owner, lamports, data, or privileges.
    InconsistentAliasObservation,
    /// An account data slice did not have its exact declared width.
    DataLengthMismatch,
    /// An account key differed from an authenticated input identity.
    KeyMismatch,
    /// An account owner differed from an authenticated input identity.
    OwnerMismatch,
    /// A data projection exceeded its account's exact declared data width.
    DataFieldOutOfBounds,
}

/// Result alias for account-profile operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Plain account facts supplied by a separately named runtime adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountObservationV1<'a> {
    key: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data: &'a [u8],
    signer: bool,
    writable: bool,
    executable: bool,
}

impl<'a> AccountObservationV1<'a> {
    /// Construct one complete observation. Validation belongs to the selected profile.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        key: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data: &'a [u8],
        signer: bool,
        writable: bool,
        executable: bool,
    ) -> Self {
        Self {
            key,
            owner,
            lamports,
            data,
            signer,
            writable,
            executable,
        }
    }

    /// Account public key.
    #[must_use]
    pub const fn key(self) -> [u8; 32] {
        self.key
    }

    /// Account owner program identity.
    #[must_use]
    pub const fn owner(self) -> [u8; 32] {
        self.owner
    }

    /// Observed native lamports.
    #[must_use]
    pub const fn lamports(self) -> u64 {
        self.lamports
    }

    /// Exact account data bytes.
    #[must_use]
    pub const fn data(self) -> &'a [u8] {
        self.data
    }

    fn privileges(self) -> u8 {
        u8::from(self.signer) | (u8::from(self.writable) << 1) | (u8::from(self.executable) << 2)
    }
}

/// Canonical alias/privilege/data-width rule for one suffix account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountRuleV1 {
    privileges: u8,
    effect_permissions: u8,
    alias_of: u16,
    data_length: u32,
}

impl AccountRuleV1 {
    /// Exact signer/writable/executable bits in positions 0/1/2.
    #[must_use]
    pub const fn privileges(self) -> u8 {
        self.privileges
    }

    /// Effect-kernel debit/credit/data-write bits in positions 0/1/2.
    #[must_use]
    pub const fn effect_permissions(self) -> u8 {
        self.effect_permissions
    }

    /// Exact effect-kernel authority derived only from authenticated profile bytes.
    #[must_use]
    pub const fn effect_permission(self) -> AccountPermission {
        AccountPermission::new(
            self.effect_permissions & EFFECT_PERMISSION_DEBIT_LAMPORTS != 0,
            self.effect_permissions & EFFECT_PERMISSION_CREDIT_LAMPORTS != 0,
            self.effect_permissions & EFFECT_PERMISSION_WRITE_DATA != 0,
        )
    }

    /// Canonical representative index; a distinct account names itself.
    #[must_use]
    pub const fn alias_of(self) -> u16 {
        self.alias_of
    }

    /// Exact required data length.
    #[must_use]
    pub const fn data_length(self) -> u32 {
        self.data_length
    }
}

const EFFECT_PERMISSION_MASK: u8 = EFFECT_PERMISSION_DEBIT_LAMPORTS
    | EFFECT_PERMISSION_CREDIT_LAMPORTS
    | EFFECT_PERMISSION_WRITE_DATA;

/// One relation or projection selected by an account-profile operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationKindV1 {
    /// Require account key equal an immutable input identity register.
    RequireKeyEqIdentity,
    /// Require account owner equal an immutable input identity register.
    RequireOwnerEqIdentity,
    /// Project account key into an identity register.
    ProjectKey,
    /// Project account owner into an identity register.
    ProjectOwner,
    /// Project account lamports into a scalar register.
    ProjectLamports,
    /// Project an exact little-endian data `u64` into a scalar register.
    ProjectDataU64,
    /// Project an exact 32-byte data field into an identity register.
    ProjectDataIdentity,
}

/// Hostile-decoded fixed-width operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationV1 {
    kind: OperationKindV1,
    account: u16,
    register: u16,
    data_offset: u32,
}

impl OperationV1 {
    /// Selected operation.
    #[must_use]
    pub const fn kind(self) -> OperationKindV1 {
        self.kind
    }

    /// Suffix account index.
    #[must_use]
    pub const fn account(self) -> u16 {
        self.account
    }

    /// Scalar or identity register according to the operation.
    #[must_use]
    pub const fn register(self) -> u16 {
        self.register
    }

    /// Byte offset for data projections; zero for other operations.
    #[must_use]
    pub const fn data_offset(self) -> u32 {
        self.data_offset
    }

    const fn is_requirement(self) -> bool {
        matches!(
            self.kind,
            OperationKindV1::RequireKeyEqIdentity | OperationKindV1::RequireOwnerEqIdentity
        )
    }

    const fn is_identity_projection(self) -> bool {
        matches!(
            self.kind,
            OperationKindV1::ProjectKey
                | OperationKindV1::ProjectOwner
                | OperationKindV1::ProjectDataIdentity
        )
    }

    const fn is_scalar_projection(self) -> bool {
        matches!(
            self.kind,
            OperationKindV1::ProjectLamports | OperationKindV1::ProjectDataU64
        )
    }
}

/// Borrowed, content-selected, exact runtime-width account profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountProfileV1<'a> {
    profile_id: [u8; 32],
    account_count: u16,
    operation_count: u16,
    scalar_count: u16,
    identity_count: u16,
    bytes: &'a [u8],
}

impl<'a> AccountProfileV1<'a> {
    /// Hostile-decode one profile after joining descriptor selection to an
    /// adapter-authenticated finalized-record content identity.
    ///
    /// # Errors
    ///
    /// Refuses mismatched content identities, malformed or noncanonical wire
    /// bytes, unsafe account rules, and invalid relation/projection programs.
    pub fn decode_selected(
        selected_profile_id: [u8; 32],
        authenticated_profile_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        if selected_profile_id == [0; 32] || authenticated_profile_id == [0; 32] {
            return Err(Error::ZeroProfileIdentity);
        }
        if selected_profile_id != authenticated_profile_id {
            return Err(Error::ProfileIdentityMismatch);
        }
        if bytes.len() < ACCOUNT_PROFILE_HEADER_BYTES_V1
            || bytes.len() > ACCOUNT_PROFILE_MAX_BYTES_V1
        {
            return Err(Error::InvalidLength);
        }
        let magic_end = ACCOUNT_PROFILE_MAGIC_OFFSET
            .checked_add(ACCOUNT_PROFILE_MAGIC_V1.len())
            .ok_or(Error::InvalidLength)?;
        if bytes.get(ACCOUNT_PROFILE_MAGIC_OFFSET..magic_end) != Some(&ACCOUNT_PROFILE_MAGIC_V1) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, ACCOUNT_PROFILE_VERSION_OFFSET)? != ACCOUNT_PROFILE_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        if read_u16(bytes, ACCOUNT_PROFILE_ARTIFACT_OFFSET)? != ACCOUNT_PROFILE_ARTIFACT_PROFILE_V1
        {
            return Err(Error::UnsupportedArtifactProfile);
        }
        require_zero(bytes, ACCOUNT_PROFILE_RESERVED_OFFSET, 12)?;
        let account_count = read_u16(bytes, ACCOUNT_PROFILE_ACCOUNT_COUNT_OFFSET)?;
        let operation_count = read_u16(bytes, ACCOUNT_PROFILE_OPERATION_COUNT_OFFSET)?;
        let scalar_count = read_u16(bytes, ACCOUNT_PROFILE_SCALAR_COUNT_OFFSET)?;
        let identity_count = read_u16(bytes, ACCOUNT_PROFILE_IDENTITY_COUNT_OFFSET)?;
        if account_count == 0 || operation_count == 0 || (scalar_count == 0 && identity_count == 0)
        {
            return Err(Error::EmptyProfile);
        }
        let expected = usize::from(account_count)
            .checked_mul(ACCOUNT_PROFILE_RULE_BYTES_V1)
            .and_then(|rules| ACCOUNT_PROFILE_HEADER_BYTES_V1.checked_add(rules))
            .and_then(|prefix| {
                usize::from(operation_count)
                    .checked_mul(ACCOUNT_PROFILE_OPERATION_BYTES_V1)
                    .and_then(|operations| prefix.checked_add(operations))
            })
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let profile = Self {
            profile_id: selected_profile_id,
            account_count,
            operation_count,
            scalar_count,
            identity_count,
            bytes,
        };
        profile.validate_structure()?;
        Ok(profile)
    }

    /// Exact selected and authenticated content identity.
    #[must_use]
    pub const fn profile_id(self) -> [u8; 32] {
        self.profile_id
    }

    /// Exact suffix account count.
    #[must_use]
    pub const fn account_count(self) -> u16 {
        self.account_count
    }

    /// Exact relation/projection count.
    #[must_use]
    pub const fn operation_count(self) -> u16 {
        self.operation_count
    }

    /// Exact `TransitionVM` V2 scalar-bank width.
    #[must_use]
    pub const fn scalar_count(self) -> u16 {
        self.scalar_count
    }

    /// Exact `TransitionVM` V2 identity-bank width.
    #[must_use]
    pub const fn identity_count(self) -> u16 {
        self.identity_count
    }

    /// Complete canonical profile bytes.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Decode one account rule by suffix index.
    ///
    /// # Errors
    ///
    /// Refuses an absent index or a malformed rule in the authenticated wire.
    pub fn rule(self, index: u16) -> Result<AccountRuleV1> {
        if index >= self.account_count {
            return Err(Error::InvalidAccountIndex);
        }
        let offset = ACCOUNT_PROFILE_HEADER_BYTES_V1
            .checked_add(
                usize::from(index)
                    .checked_mul(ACCOUNT_PROFILE_RULE_BYTES_V1)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        require_zero(self.bytes, add(offset, ACCOUNT_RULE_RESERVED_OFFSET)?, 8)?;
        let privileges = byte(self.bytes, add(offset, ACCOUNT_RULE_PRIVILEGES_OFFSET)?)?;
        if privileges & !0x07 != 0 {
            return Err(Error::InvalidPrivileges);
        }
        let effect_permissions = byte(
            self.bytes,
            add(offset, ACCOUNT_RULE_EFFECT_PERMISSIONS_OFFSET)?,
        )?;
        if effect_permissions & !EFFECT_PERMISSION_MASK != 0 {
            return Err(Error::InvalidEffectPermissions);
        }
        if effect_permissions != 0 && privileges & 0x02 == 0 {
            return Err(Error::EffectRequiresWritable);
        }
        Ok(AccountRuleV1 {
            privileges,
            effect_permissions,
            alias_of: read_u16(self.bytes, add(offset, ACCOUNT_RULE_ALIAS_OF_OFFSET)?)?,
            data_length: read_u32(self.bytes, add(offset, ACCOUNT_RULE_DATA_LENGTH_OFFSET)?)?,
        })
    }

    /// Decode one relation/projection by operation index.
    ///
    /// # Errors
    ///
    /// Refuses an absent index, unknown operation, or noncanonical fields.
    pub fn operation(self, index: u16) -> Result<OperationV1> {
        if index >= self.operation_count {
            return Err(Error::InvalidLength);
        }
        let rules_bytes = usize::from(self.account_count)
            .checked_mul(ACCOUNT_PROFILE_RULE_BYTES_V1)
            .ok_or(Error::InvalidLength)?;
        let offset = ACCOUNT_PROFILE_HEADER_BYTES_V1
            .checked_add(rules_bytes)
            .and_then(|prefix| {
                usize::from(index)
                    .checked_mul(ACCOUNT_PROFILE_OPERATION_BYTES_V1)
                    .and_then(|body| prefix.checked_add(body))
            })
            .ok_or(Error::InvalidLength)?;
        if byte(
            self.bytes,
            add(offset, ACCOUNT_OPERATION_RESERVED_BYTE_OFFSET)?,
        )? != 0
            || read_u16(
                self.bytes,
                add(offset, ACCOUNT_OPERATION_RESERVED_SHORT_OFFSET)?,
            )? != 0
            || read_u32(self.bytes, add(offset, ACCOUNT_OPERATION_RESERVED_OFFSET)?)? != 0
        {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let kind = match byte(self.bytes, add(offset, ACCOUNT_OPERATION_OPCODE_OFFSET)?)? {
            OP_REQUIRE_KEY_EQ_IDENTITY => OperationKindV1::RequireKeyEqIdentity,
            OP_REQUIRE_OWNER_EQ_IDENTITY => OperationKindV1::RequireOwnerEqIdentity,
            OP_PROJECT_KEY => OperationKindV1::ProjectKey,
            OP_PROJECT_OWNER => OperationKindV1::ProjectOwner,
            OP_PROJECT_LAMPORTS => OperationKindV1::ProjectLamports,
            OP_PROJECT_DATA_U64 => OperationKindV1::ProjectDataU64,
            OP_PROJECT_DATA_IDENTITY => OperationKindV1::ProjectDataIdentity,
            _ => return Err(Error::UnknownOperation),
        };
        let operation = OperationV1 {
            kind,
            account: read_u16(self.bytes, add(offset, ACCOUNT_OPERATION_ACCOUNT_OFFSET)?)?,
            register: read_u16(self.bytes, add(offset, ACCOUNT_OPERATION_REGISTER_OFFSET)?)?,
            data_offset: read_u32(self.bytes, add(offset, ACCOUNT_OPERATION_DATA_OFFSET)?)?,
        };
        if !matches!(
            kind,
            OperationKindV1::ProjectDataU64 | OperationKindV1::ProjectDataIdentity
        ) && operation.data_offset != 0
        {
            return Err(Error::NonCanonicalOperation);
        }
        Ok(operation)
    }

    fn validate_structure(self) -> Result<()> {
        let mut account_index = 0_u16;
        while account_index < self.account_count {
            let rule = self.rule(account_index)?;
            if rule.alias_of > account_index {
                return Err(Error::InvalidAlias);
            }
            if rule.alias_of < account_index && self.rule(rule.alias_of)?.alias_of != rule.alias_of
            {
                return Err(Error::InvalidAlias);
            }
            if rule.alias_of < account_index {
                let representative = self.rule(rule.alias_of)?;
                if rule.privileges != representative.privileges
                    || rule.effect_permissions != representative.effect_permissions
                    || rule.data_length != representative.data_length
                {
                    return Err(Error::InvalidAlias);
                }
            }
            account_index = account_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }

        let mut projection_count = 0_u16;
        let mut operation_index = 0_u16;
        while operation_index < self.operation_count {
            let operation = self.operation(operation_index)?;
            if operation.account >= self.account_count {
                return Err(Error::InvalidAccountIndex);
            }
            let identity_operation =
                operation.is_requirement() || operation.is_identity_projection();
            if (identity_operation && operation.register >= self.identity_count)
                || (operation.is_scalar_projection() && operation.register >= self.scalar_count)
            {
                return Err(Error::InvalidRegister);
            }
            let field_width = match operation.kind {
                OperationKindV1::ProjectDataU64 => 8_u32,
                OperationKindV1::ProjectDataIdentity => 32_u32,
                _ => 0,
            };
            if field_width != 0 {
                let end = operation
                    .data_offset
                    .checked_add(field_width)
                    .ok_or(Error::DataFieldOutOfBounds)?;
                if end > self.rule(operation.account)?.data_length {
                    return Err(Error::DataFieldOutOfBounds);
                }
            }
            if operation.is_identity_projection() || operation.is_scalar_projection() {
                projection_count = projection_count
                    .checked_add(1)
                    .ok_or(Error::InvalidLength)?;
                self.require_unique_projection(operation_index, operation)?;
            }
            if operation.is_identity_projection() {
                self.require_not_authority_register(operation.register)?;
            }
            operation_index = operation_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        if projection_count == 0 {
            return Err(Error::EmptyProfile);
        }
        self.require_anchored_representatives()?;
        self.require_effect_authority()
    }

    fn require_unique_projection(self, index: u16, operation: OperationV1) -> Result<()> {
        let mut prior_index = 0_u16;
        while prior_index < index {
            let prior = self.operation(prior_index)?;
            if ((operation.is_identity_projection() && prior.is_identity_projection())
                || (operation.is_scalar_projection() && prior.is_scalar_projection()))
                && operation.register == prior.register
            {
                return Err(Error::DuplicateProjection);
            }
            prior_index = prior_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }

    fn require_not_authority_register(self, destination: u16) -> Result<()> {
        let mut index = 0_u16;
        while index < self.operation_count {
            let operation = self.operation(index)?;
            if operation.is_requirement() && operation.register == destination {
                return Err(Error::AuthorityOverwrite);
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }

    fn require_anchored_representatives(self) -> Result<()> {
        let mut account = 0_u16;
        while account < self.account_count {
            if self.rule(account)?.alias_of == account {
                let mut anchored = false;
                let mut operation_index = 0_u16;
                while operation_index < self.operation_count {
                    let operation = self.operation(operation_index)?;
                    if operation.account == account && operation.is_requirement() {
                        anchored = true;
                    }
                    operation_index = operation_index.checked_add(1).ok_or(Error::InvalidLength)?;
                }
                if !anchored {
                    return Err(Error::UnanchoredAccount);
                }
            }
            account = account.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }

    fn require_effect_authority(self) -> Result<()> {
        let mut account = 0_u16;
        while account < self.account_count {
            let rule = self.rule(account)?;
            let requires_owner = rule.effect_permissions
                & (EFFECT_PERMISSION_DEBIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA)
                != 0;
            if requires_owner {
                let mut owner_anchored = false;
                let mut operation_index = 0_u16;
                while operation_index < self.operation_count {
                    let operation = self.operation(operation_index)?;
                    if operation.account == rule.alias_of
                        && operation.kind == OperationKindV1::RequireOwnerEqIdentity
                    {
                        owner_anchored = true;
                    }
                    operation_index = operation_index.checked_add(1).ok_or(Error::InvalidLength)?;
                }
                if !owner_anchored {
                    return Err(Error::EffectOwnerUnanchored);
                }
            }
            account = account.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }
}

/// Derive the exact effect-kernel permission slice from authenticated profile rules.
///
/// The output length must equal the profile's suffix width. Length refusal is
/// checked before the first write, so `output` remains unchanged on error.
///
/// # Errors
///
/// Refuses a permission bank whose width differs from the profile account count.
pub fn derive_effect_permissions(
    profile: AccountProfileV1<'_>,
    output: &mut [AccountPermission],
) -> Result<()> {
    if output.len() != usize::from(profile.account_count) {
        return Err(Error::AccountCountMismatch);
    }
    for (index, permission) in output.iter_mut().enumerate() {
        let rule = profile.rule(u16::try_from(index).map_err(|_| Error::InvalidAccountIndex)?)?;
        *permission = rule.effect_permission();
    }
    Ok(())
}

/// Caller-owned runtime-width banks used for commit-last projection.
pub struct ProjectionRegistersV2<'a> {
    input: RegisterInput<'a>,
    scratch: RegisterOutput<'a>,
    output: RegisterOutput<'a>,
}

impl<'a> ProjectionRegistersV2<'a> {
    /// Construct one immutable input, mutable scratch, and mutable output frame.
    #[must_use]
    pub const fn new(
        input: RegisterInput<'a>,
        scratch: RegisterOutput<'a>,
        output: RegisterOutput<'a>,
    ) -> Self {
        Self {
            input,
            scratch,
            output,
        }
    }
}

/// Validate the exact suffix and atomically project it into `TransitionVM` V2 banks.
///
/// Every relation reads the immutable input bank. Profile bytes cannot carry
/// expected identity literals, and identity projection cannot overwrite a
/// register used by a relation. Output is copied only after every account,
/// relation, and data-field check succeeds.
///
/// # Errors
///
/// Refuses width, privilege, alias, data, key, owner, or projection mismatch.
pub fn project_atomic(
    profile: AccountProfileV1<'_>,
    accounts: &[AccountObservationV1<'_>],
    registers: ProjectionRegistersV2<'_>,
) -> Result<()> {
    let ProjectionRegistersV2 {
        input,
        scratch,
        output,
    } = registers;
    require_widths(profile, input.scalars.len(), input.identities.len())?;
    require_widths(profile, scratch.scalars.len(), scratch.identities.len())?;
    require_widths(profile, output.scalars.len(), output.identities.len())?;
    validate_accounts(profile, accounts, input)?;

    scratch.scalars.copy_from_slice(input.scalars);
    scratch.identities.copy_from_slice(input.identities);
    apply_projections(profile, accounts, scratch.scalars, scratch.identities)?;
    output.scalars.copy_from_slice(scratch.scalars);
    output.identities.copy_from_slice(scratch.identities);
    Ok(())
}

fn require_widths(profile: AccountProfileV1<'_>, scalars: usize, identities: usize) -> Result<()> {
    if scalars == usize::from(profile.scalar_count)
        && identities == usize::from(profile.identity_count)
    {
        Ok(())
    } else {
        Err(Error::RegisterWidthMismatch)
    }
}

fn validate_accounts(
    profile: AccountProfileV1<'_>,
    accounts: &[AccountObservationV1<'_>],
    input: RegisterInput<'_>,
) -> Result<()> {
    if accounts.len() != usize::from(profile.account_count) {
        return Err(Error::AccountCountMismatch);
    }
    for (index, account) in accounts.iter().copied().enumerate() {
        let rule = profile.rule(u16::try_from(index).map_err(|_| Error::InvalidAccountIndex)?)?;
        if account.privileges() != rule.privileges {
            return Err(Error::PrivilegeMismatch);
        }
        if account.data.len()
            != usize::try_from(rule.data_length).map_err(|_| Error::DataLengthMismatch)?
        {
            return Err(Error::DataLengthMismatch);
        }
    }
    validate_aliases(profile, accounts)?;

    let mut operation_index = 0_u16;
    while operation_index < profile.operation_count {
        let operation = profile.operation(operation_index)?;
        let account = accounts
            .get(usize::from(operation.account))
            .copied()
            .ok_or(Error::InvalidAccountIndex)?;
        let expected = input
            .identities
            .get(usize::from(operation.register))
            .copied();
        match operation.kind {
            OperationKindV1::RequireKeyEqIdentity if Some(account.key) != expected => {
                return Err(Error::KeyMismatch);
            }
            OperationKindV1::RequireOwnerEqIdentity if Some(account.owner) != expected => {
                return Err(Error::OwnerMismatch);
            }
            _ => {}
        }
        operation_index = operation_index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

fn validate_aliases(
    profile: AccountProfileV1<'_>,
    accounts: &[AccountObservationV1<'_>],
) -> Result<()> {
    for (left_index, left) in accounts.iter().copied().enumerate() {
        let left_rule =
            profile.rule(u16::try_from(left_index).map_err(|_| Error::InvalidAccountIndex)?)?;
        for (right_index, right) in accounts.iter().copied().enumerate().skip(left_index + 1) {
            let right_rule = profile
                .rule(u16::try_from(right_index).map_err(|_| Error::InvalidAccountIndex)?)?;
            let should_alias = left_rule.alias_of == right_rule.alias_of;
            if (left.key == right.key) != should_alias {
                return Err(Error::AliasMismatch);
            }
            if should_alias
                && (left.owner != right.owner
                    || left.lamports != right.lamports
                    || left.data != right.data
                    || left.privileges() != right.privileges())
            {
                return Err(Error::InconsistentAliasObservation);
            }
        }
    }
    Ok(())
}

fn apply_projections(
    profile: AccountProfileV1<'_>,
    accounts: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<()> {
    let mut operation_index = 0_u16;
    while operation_index < profile.operation_count {
        let operation = profile.operation(operation_index)?;
        let account = accounts
            .get(usize::from(operation.account))
            .copied()
            .ok_or(Error::InvalidAccountIndex)?;
        match operation.kind {
            OperationKindV1::RequireKeyEqIdentity | OperationKindV1::RequireOwnerEqIdentity => {}
            OperationKindV1::ProjectKey => {
                write_identity(identities, operation.register, account.key)?;
            }
            OperationKindV1::ProjectOwner => {
                write_identity(identities, operation.register, account.owner)?;
            }
            OperationKindV1::ProjectLamports => {
                write_scalar(scalars, operation.register, account.lamports)?;
            }
            OperationKindV1::ProjectDataU64 => {
                write_scalar(
                    scalars,
                    operation.register,
                    u64::from_le_bytes(read_data::<8>(account.data, operation.data_offset)?),
                )?;
            }
            OperationKindV1::ProjectDataIdentity => {
                write_identity(
                    identities,
                    operation.register,
                    read_data::<32>(account.data, operation.data_offset)?,
                )?;
            }
        }
        operation_index = operation_index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

fn read_data<const N: usize>(data: &[u8], offset: u32) -> Result<[u8; N]> {
    let start = usize::try_from(offset).map_err(|_| Error::DataFieldOutOfBounds)?;
    let end = start.checked_add(N).ok_or(Error::DataFieldOutOfBounds)?;
    data.get(start..end)
        .ok_or(Error::DataFieldOutOfBounds)?
        .try_into()
        .map_err(|_| Error::DataFieldOutOfBounds)
}

fn write_scalar(values: &mut [u64], index: u16, value: u64) -> Result<()> {
    *values
        .get_mut(usize::from(index))
        .ok_or(Error::InvalidRegister)? = value;
    Ok(())
}

fn write_identity(values: &mut [[u8; 32]], index: u16, value: [u8; 32]) -> Result<()> {
    *values
        .get_mut(usize::from(index))
        .ok_or(Error::InvalidRegister)? = value;
    Ok(())
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read::<2>(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read::<4>(bytes, offset)?))
}

fn read<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::*;
    use generated::{AGREEMENT_PROFILE_V1, ALIAS_AGREEMENT_PROFILE_V1, REFUSAL_CORPUS_V1};

    const PROFILE_ID: [u8; 32] = [0xa5; 32];

    fn profile() -> AccountProfileV1<'static> {
        AccountProfileV1::decode_selected(PROFILE_ID, PROFILE_ID, &AGREEMENT_PROFILE_V1)
            .expect("Lean agreement profile")
    }

    fn account_data() -> (vec::Vec<u8>, vec::Vec<u8>, vec::Vec<u8>) {
        let descriptor = vec![0x55; 64];
        let mut config = vec![0_u8; 232];
        config
            .get_mut(16..48)
            .expect("capability identity")
            .copy_from_slice(&[0x41; 32]);
        config
            .get_mut(80..112)
            .expect("surplus beneficiary")
            .copy_from_slice(&[0x42; 32]);
        config
            .get_mut(112..120)
            .expect("candidate capacity")
            .copy_from_slice(&7_u64.to_le_bytes());
        config
            .get_mut(120..128)
            .expect("page capacity")
            .copy_from_slice(&100_u64.to_le_bytes());
        let clock = vec![0x09, 0, 0, 0, 0, 0, 0, 0]
            .into_iter()
            .chain([0_u8; 32])
            .collect();
        (descriptor, config, clock)
    }

    #[test]
    fn generated_agreement_projects_activation_resources_atomically() {
        let profile = profile();
        assert_eq!(profile.account_count(), 4);
        assert_eq!(profile.scalar_count(), 20);
        assert_eq!(profile.identity_count(), 16);
        let (descriptor, config, clock) = account_data();
        let accounts = [
            AccountObservationV1::new([0x31; 32], [0x81; 32], 11, &descriptor, false, false, false),
            AccountObservationV1::new([0x32; 32], [0x81; 32], 12, &config, false, false, false),
            AccountObservationV1::new([0x33; 32], [0x82; 32], 13, &[], false, true, false),
            AccountObservationV1::new([0x34; 32], [0x83; 32], 14, &clock, false, false, false),
        ];
        let mut input_scalars = [0_u64; 20];
        input_scalars[1] = 7;
        let mut input_identities = [[0_u8; 32]; 16];
        input_identities[6] = [0x33; 32];
        input_identities[7] = [0x82; 32];
        input_identities[8] = [0x81; 32];
        input_identities[9] = [0x31; 32];
        input_identities[10] = [0x32; 32];
        input_identities[11] = [0x34; 32];
        let mut scratch_scalars = [0_u64; 20];
        let mut scratch_identities = [[0_u8; 32]; 16];
        let mut output_scalars = [0xff_u64; 20];
        let mut output_identities = [[0xff_u8; 32]; 16];
        project_atomic(
            profile,
            &accounts,
            ProjectionRegistersV2::new(
                RegisterInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
                },
                RegisterOutput {
                    scalars: &mut scratch_scalars,
                    identities: &mut scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut output_scalars,
                    identities: &mut output_identities,
                },
            ),
        )
        .expect("profile accepts exact resources");
        assert_eq!(output_scalars[14], 13);
        assert_eq!(output_scalars[15], 7);
        assert_eq!(output_scalars[16], 100);
        assert_eq!(output_scalars[17], 9);
        assert_eq!(output_identities[12], [0x33; 32]);
        assert_eq!(output_identities[13], [0x82; 32]);
        assert_eq!(output_identities[14], [0x41; 32]);
        assert_eq!(output_identities[15], [0x42; 32]);

        let mut permissions = [AccountPermission::read_only(); 4];
        derive_effect_permissions(profile, &mut permissions).expect("profile owns permissions");
        assert_eq!(permissions[0], AccountPermission::read_only());
        assert_eq!(permissions[1], AccountPermission::read_only());
        assert_eq!(permissions[2], AccountPermission::new(true, false, false));
        assert_eq!(permissions[3], AccountPermission::read_only());
    }

    #[test]
    fn generated_refusal_corpus_matches_hostile_decoder() {
        for hostile in &REFUSAL_CORPUS_V1 {
            assert!(AccountProfileV1::decode_selected(PROFILE_ID, PROFILE_ID, hostile).is_err());
        }
        assert_eq!(
            AccountProfileV1::decode_selected([0; 32], PROFILE_ID, &AGREEMENT_PROFILE_V1),
            Err(Error::ZeroProfileIdentity)
        );
        assert_eq!(
            AccountProfileV1::decode_selected([1; 32], [2; 32], &AGREEMENT_PROFILE_V1),
            Err(Error::ProfileIdentityMismatch)
        );
    }

    #[test]
    fn generated_alias_agreement_requires_one_exact_observation() {
        let alias_profile =
            AccountProfileV1::decode_selected(PROFILE_ID, PROFILE_ID, &ALIAS_AGREEMENT_PROFILE_V1)
                .expect("Lean alias agreement profile");
        let data = 17_u64.to_le_bytes();
        let account =
            AccountObservationV1::new([0x51; 32], [0x52; 32], 53, &data, false, false, false);
        let canonical = [account, account];
        let input_scalars = [0_u64; 1];
        let input_identities = [[0x51_u8; 32]; 1];
        let mut scratch_scalars = [0_u64; 1];
        let mut scratch_identities = [[0_u8; 32]; 1];
        let mut output_scalars = [0xd1_u64; 1];
        let mut output_identities = [[0xd2_u8; 32]; 1];
        project_atomic(
            alias_profile,
            &canonical,
            ProjectionRegistersV2::new(
                RegisterInput {
                    scalars: &input_scalars,
                    identities: &input_identities,
                },
                RegisterOutput {
                    scalars: &mut scratch_scalars,
                    identities: &mut scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut output_scalars,
                    identities: &mut output_identities,
                },
            ),
        )
        .expect("exact alias partition");
        assert_eq!(output_scalars, [53]);

        let mut inconsistent = canonical;
        inconsistent[1].lamports = 54;
        let sentinel_scalars = [0xd3_u64; 1];
        let sentinel_identities = [[0xd4_u8; 32]; 1];
        output_scalars = sentinel_scalars;
        output_identities = sentinel_identities;
        assert_eq!(
            project_atomic(
                alias_profile,
                &inconsistent,
                ProjectionRegistersV2::new(
                    RegisterInput {
                        scalars: &input_scalars,
                        identities: &input_identities,
                    },
                    RegisterOutput {
                        scalars: &mut scratch_scalars,
                        identities: &mut scratch_identities,
                    },
                    RegisterOutput {
                        scalars: &mut output_scalars,
                        identities: &mut output_identities,
                    },
                ),
            ),
            Err(Error::InconsistentAliasObservation)
        );
        assert_eq!(output_scalars, sentinel_scalars);
        assert_eq!(output_identities, sentinel_identities);
    }

    #[test]
    fn every_runtime_refusal_preserves_candidate_output() {
        let profile = profile();
        let (descriptor, config, clock) = account_data();
        let canonical_accounts = [
            AccountObservationV1::new([0x31; 32], [0x81; 32], 11, &descriptor, false, false, false),
            AccountObservationV1::new([0x32; 32], [0x81; 32], 12, &config, false, false, false),
            AccountObservationV1::new([0x33; 32], [0x82; 32], 13, &[], false, true, false),
            AccountObservationV1::new([0x34; 32], [0x83; 32], 14, &clock, false, false, false),
        ];
        let input_scalars = [0_u64; 20];
        let mut input_identities = [[0_u8; 32]; 16];
        input_identities[6] = [0x33; 32];
        input_identities[7] = [0x82; 32];
        input_identities[8] = [0x81; 32];
        input_identities[9] = [0x31; 32];
        input_identities[10] = [0x32; 32];
        input_identities[11] = [0x34; 32];
        let sentinel_scalars = [0xd1_u64; 20];
        let sentinel_identities = [[0xd2_u8; 32]; 16];

        for fault in 0..5 {
            let mut accounts = canonical_accounts;
            match fault {
                0 => accounts[0].owner = [0xee; 32],
                1 => accounts[1].key = accounts[0].key,
                2 => accounts[2].writable = false,
                3 => accounts[2].key = [0xef; 32],
                _ => accounts[3].data = &[],
            }
            let mut scratch_scalars = [0_u64; 20];
            let mut scratch_identities = [[0_u8; 32]; 16];
            let mut output_scalars = sentinel_scalars;
            let mut output_identities = sentinel_identities;
            assert!(
                project_atomic(
                    profile,
                    &accounts,
                    ProjectionRegistersV2::new(
                        RegisterInput {
                            scalars: &input_scalars,
                            identities: &input_identities,
                        },
                        RegisterOutput {
                            scalars: &mut scratch_scalars,
                            identities: &mut scratch_identities,
                        },
                        RegisterOutput {
                            scalars: &mut output_scalars,
                            identities: &mut output_identities,
                        },
                    ),
                )
                .is_err()
            );
            assert_eq!(output_scalars, sentinel_scalars);
            assert_eq!(output_identities, sentinel_identities);
        }
    }
}
