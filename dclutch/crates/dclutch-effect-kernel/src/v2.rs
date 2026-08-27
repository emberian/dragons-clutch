//! Runtime-width physical effects behind an authenticated account profile.
//!
//! This module does not authenticate accounts or apply SVM mutations. It
//! hostile-decodes a descriptor-selected effect program, checks every account
//! and register coordinate, and projects its complete lamport post-state into
//! caller-owned memory atomically. Data writes are exposed as resolved values
//! only after the whole projection accepts. The composing adapter remains
//! responsible for proving that permissions came from the authenticated
//! AccountProfile and for committing the resolved effects in program order.

use core::convert::TryInto;

/// Typed, allocation-free encoder for this generation's artifacts.
pub mod encode;

/// Canonical runtime-width effect-program magic.
pub const MAGIC: [u8; 4] = *b"DCE2";
/// Finalized-record schema label for runtime-width effect programs.
pub const SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/effect-program-v2";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE`].
pub const SCHEMA_RELEASE_ID: [u8; 32] = [
    0x02, 0x42, 0x7a, 0x3e, 0x7e, 0x1f, 0x2e, 0x5b, 0xdb, 0x74, 0x2d, 0x48, 0x1d, 0x80, 0xf8, 0xe5,
    0x4a, 0x6b, 0x3a, 0x28, 0xcd, 0x05, 0xef, 0x17, 0x03, 0xdb, 0xdf, 0x32, 0xf1, 0x41, 0xfc, 0x1f,
];
/// Canonical runtime-width effect-program version.
pub const VERSION: u8 = 2;
/// Exact effect-program header width.
pub const HEADER_BYTES: usize = 16;
/// Exact fixed instruction width.
pub const INSTRUCTION_BYTES: usize = 16;

const FLAGS_OFFSET: usize = 5;
const INSTRUCTION_COUNT_OFFSET: usize = 6;
const ACCOUNT_COUNT_OFFSET: usize = 8;
const SCALAR_COUNT_OFFSET: usize = 10;
const IDENTITY_COUNT_OFFSET: usize = 12;
const REQUEST_BYTES_OFFSET: usize = 14;

const OPCODE_OFFSET: usize = 0;
const AUXILIARY_OFFSET: usize = 1;
const ACCOUNT_A_OFFSET: usize = 2;
const ACCOUNT_B_OFFSET: usize = 4;
const REGISTER_OFFSET: usize = 6;
const DATA_OFFSET: usize = 8;
const EXTRA_OFFSET: usize = 12;

const OP_TRANSFER_LAMPORTS: u8 = 0;
const OP_WRITE_SCALAR: u8 = 1;
const OP_WRITE_IDENTITY: u8 = 2;
const OP_REQUIRE_LAMPORTS_EQ: u8 = 3;
const OP_WRITE_REQUEST_SCALAR: u8 = 4;
const OP_WRITE_REQUEST_IDENTITY: u8 = 5;
const OP_INVOKE_ROLE: u8 = 6;
const OP_INVOKE_ROLE_IF_NONZERO: u8 = 7;

/// Stable hostile-decode or projection refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The selected or authenticated effect-program identity was all zero.
    ZeroProgramIdentity,
    /// The authenticated effect-program content was not descriptor-selected.
    ProgramIdentityMismatch,
    /// Bytes did not have the exact count-derived width.
    InvalidLength,
    /// Magic selected another program family.
    InvalidMagic,
    /// The encoded version is not implemented.
    UnsupportedVersion,
    /// Header flags or reserved bytes were nonzero.
    NonCanonicalHeader,
    /// The program contained no instruction or no account coordinate.
    EmptyProgram,
    /// An opcode is outside the bounded V2 vocabulary.
    UnknownOpcode,
    /// An instruction carried nonzero inactive or reserved fields.
    NonCanonicalInstruction,
    /// An instruction addressed an absent or aliased account coordinate.
    InvalidAccount,
    /// The supplied authenticated alias partition was not canonical.
    InvalidAlias,
    /// An instruction addressed an absent scalar or identity coordinate.
    InvalidRegister,
    /// Caller-owned account, register, scratch, or output widths differed.
    WidthMismatch,
    /// This effect program requires the typed role-request projection API.
    RequestBufferRequired,
    /// The authenticated AccountProfile did not grant the required mutation.
    PermissionDenied,
    /// A projected fixed-width data write exceeded the authenticated account.
    DataOutOfBounds,
    /// A lamport debit exceeded the projected source balance.
    InsufficientLamports,
    /// Checked balance or address arithmetic overflowed.
    ArithmeticOverflow,
    /// A post-state equality required by the effect program was false.
    CheckFailed,
    /// Two data writes overlap and would make order part of semantics.
    OverlappingWrites,
}

/// Result alias for runtime-width effect operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Mutation authority derived from an already authenticated AccountProfile.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AccountPermission {
    may_debit_lamports: bool,
    may_credit_lamports: bool,
    may_write_data: bool,
}

impl AccountPermission {
    /// Construct exact per-account effect permissions.
    pub const fn new(
        may_debit_lamports: bool,
        may_credit_lamports: bool,
        may_write_data: bool,
    ) -> Self {
        Self {
            may_debit_lamports,
            may_credit_lamports,
            may_write_data,
        }
    }

    /// Read-only account with no admitted physical mutation.
    pub const fn read_only() -> Self {
        Self::new(false, false, false)
    }

    /// Program-owned mutable account that may conserve lamports and write data.
    pub const fn program_owned_mutable() -> Self {
        Self::new(true, true, true)
    }

    /// External destination that may receive lamports but expose no other effect.
    pub const fn lamport_receiver() -> Self {
        Self::new(false, true, false)
    }

    /// Whether this authenticated account may lose lamports.
    pub const fn may_debit_lamports(self) -> bool {
        self.may_debit_lamports
    }

    /// Whether this authenticated account may gain lamports.
    pub const fn may_credit_lamports(self) -> bool {
        self.may_credit_lamports
    }

    /// Whether this authenticated account may receive a data write.
    pub const fn may_write_data(self) -> bool {
        self.may_write_data
    }
}

/// Immutable physical account facts used during effect projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountInput {
    /// Observed lamport balance before any projected effect.
    pub lamports: u64,
    /// Exact authenticated account-data length.
    pub data_len: usize,
}

/// Hostile-decoded borrowed runtime-width effect program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV2<'a> {
    instruction_count: u16,
    account_count: u16,
    scalar_count: u16,
    identity_count: u16,
    request_bytes: u16,
    bytes: &'a [u8],
}

impl<'a> ProgramV2<'a> {
    /// Decode one effect program after joining descriptor selection to an
    /// adapter-authenticated finalized-record content identity.
    ///
    /// The outer adapter remains responsible for authenticating the record
    /// bytes and deriving `authenticated_program_id`. Keeping the equality
    /// check here prevents a caller from treating a schema release identity as
    /// the authority for arbitrary effect bytes under that schema.
    pub fn decode_selected(
        selected_program_id: [u8; 32],
        authenticated_program_id: [u8; 32],
        bytes: &'a [u8],
    ) -> Result<Self> {
        if selected_program_id == [0; 32] || authenticated_program_id == [0; 32] {
            return Err(Error::ZeroProgramIdentity);
        }
        if selected_program_id != authenticated_program_id {
            return Err(Error::ProgramIdentityMismatch);
        }
        Self::decode(bytes)
    }

    /// Decode, canonicalize, and cross-check one exact effect program.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if byte(bytes, 4)? != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if byte(bytes, FLAGS_OFFSET)? != 0 {
            return Err(Error::NonCanonicalHeader);
        }
        let instruction_count = read_u16(bytes, INSTRUCTION_COUNT_OFFSET)?;
        let account_count = read_u16(bytes, ACCOUNT_COUNT_OFFSET)?;
        let scalar_count = read_u16(bytes, SCALAR_COUNT_OFFSET)?;
        let identity_count = read_u16(bytes, IDENTITY_COUNT_OFFSET)?;
        let request_bytes = read_u16(bytes, REQUEST_BYTES_OFFSET)?;
        if instruction_count == 0 || account_count == 0 {
            return Err(Error::EmptyProgram);
        }
        let expected = usize::from(instruction_count)
            .checked_mul(INSTRUCTION_BYTES)
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let program = Self {
            instruction_count,
            account_count,
            scalar_count,
            identity_count,
            request_bytes,
            bytes,
        };
        let mut index = 0_u16;
        while index < instruction_count {
            program.instruction(index)?.validate(program)?;
            program.require_nonoverlap(index)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(program)
    }

    /// Exact number of effects.
    pub const fn instruction_count(self) -> u16 {
        self.instruction_count
    }

    /// Exact account-vector width selected by the profile.
    pub const fn account_count(self) -> u16 {
        self.account_count
    }

    /// Exact scalar-bank width consumed from TransitionVM V2 output.
    pub const fn scalar_count(self) -> u16 {
        self.scalar_count
    }

    /// Exact identity-bank width consumed from TransitionVM V2 output.
    pub const fn identity_count(self) -> u16 {
        self.identity_count
    }

    /// Exact caller-owned byte buffer used to assemble typed role requests.
    pub const fn request_bytes(self) -> u16 {
        self.request_bytes
    }

    /// Borrow the complete canonical bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Resolve one accepted instruction against exact output registers.
    pub fn resolved_effect(
        self,
        index: u16,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedEffect> {
        self.require_register_widths(scalars, identities)?;
        self.instruction(index)?.resolve(scalars, identities)
    }

    fn instruction(self, index: u16) -> Result<Instruction> {
        if index >= self.instruction_count {
            return Err(Error::InvalidLength);
        }
        let offset = usize::from(index)
            .checked_mul(INSTRUCTION_BYTES)
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        Instruction::decode(self.bytes, offset)
    }

    fn require_register_widths(self, scalars: &[u64], identities: &[[u8; 32]]) -> Result<()> {
        if scalars.len() == usize::from(self.scalar_count)
            && identities.len() == usize::from(self.identity_count)
        {
            Ok(())
        } else {
            Err(Error::WidthMismatch)
        }
    }

    fn require_nonoverlap(self, right_index: u16) -> Result<()> {
        let right = self.instruction(right_index)?;
        let Some((right_account, right_start, right_width)) = right.write_range() else {
            return Ok(());
        };
        let right_end = right_start
            .checked_add(right_width)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut left_index = 0_u16;
        while left_index < right_index {
            let left = self.instruction(left_index)?;
            if let Some((left_account, left_start, left_width)) = left.write_range() {
                let left_end = left_start
                    .checked_add(left_width)
                    .ok_or(Error::ArithmeticOverflow)?;
                if left_account == right_account && left_start < right_end && right_start < left_end
                {
                    return Err(Error::OverlappingWrites);
                }
            }
            left_index = left_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        let Some((right_start, right_width)) = right.request_write_range() else {
            return Ok(());
        };
        let right_end = right_start
            .checked_add(right_width)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut left_index = 0_u16;
        while left_index < right_index {
            let left = self.instruction(left_index)?;
            if let Some((left_start, left_width)) = left.request_write_range() {
                let left_end = left_start
                    .checked_add(left_width)
                    .ok_or(Error::ArithmeticOverflow)?;
                if left_start < right_end && right_start < left_end {
                    return Err(Error::OverlappingWrites);
                }
            }
            left_index = left_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(())
    }
}

/// One of the fixed state-owning roles that Trading may invoke.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixedRole {
    /// Canonical Market Core orchestration role.
    Core,
    /// Canonical Claims economic owner.
    Claims,
    /// Canonical Resolution/provider owner.
    Resolution,
    /// Canonical collateral Custody owner.
    Custody,
}

impl FixedRole {
    fn decode(value: u16) -> Result<Self> {
        match value {
            0 => Ok(Self::Core),
            1 => Ok(Self::Claims),
            3 => Ok(Self::Resolution),
            4 => Ok(Self::Custody),
            _ => Err(Error::NonCanonicalInstruction),
        }
    }
}

/// One fully register-resolved physical effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedEffect {
    /// Conserve lamports by debiting and crediting two authorized accounts.
    TransferLamports {
        /// Source account coordinate.
        source: u16,
        /// Destination account coordinate.
        destination: u16,
        /// Exact amount from one TransitionVM scalar.
        amount: u64,
    },
    /// Write one exact little-endian scalar into program-owned data.
    WriteScalar {
        /// Writable program-owned account coordinate.
        account: u16,
        /// Exact byte offset.
        offset: u32,
        /// Scalar value.
        value: u64,
    },
    /// Write one exact identity into program-owned data.
    WriteIdentity {
        /// Writable program-owned account coordinate.
        account: u16,
        /// Exact byte offset.
        offset: u32,
        /// Identity value.
        value: [u8; 32],
    },
    /// Require one intermediate/final lamport balance exactly.
    RequireLamportsEq {
        /// Account coordinate.
        account: u16,
        /// Required balance.
        value: u64,
    },
    /// Write one scalar into the caller-owned role-request buffer.
    WriteRequestScalar {
        /// Exact byte offset in the complete request buffer.
        offset: u32,
        /// Scalar value.
        value: u64,
    },
    /// Write one identity into the caller-owned role-request buffer.
    WriteRequestIdentity {
        /// Exact byte offset in the complete request buffer.
        offset: u32,
        /// Identity value.
        value: [u8; 32],
    },
    /// Invoke one fixed role with a bounded request and account subframe.
    InvokeRole {
        /// Exact state-owning role; Trading itself is never a child target.
        role: FixedRole,
        /// First authenticated account coordinate supplied to the child.
        account_start: u16,
        /// Exact child account count.
        account_count: u16,
        /// Byte offset of this child's request within the complete buffer.
        request_offset: u32,
        /// Exact request byte length.
        request_len: u32,
        /// Whether the adapter must emit this invocation.
        enabled: bool,
    },
}

/// Project the complete lamport result and validate every data effect atomically.
///
/// `output_lamports` is unchanged on every refusal. `scratch_lamports` may
/// contain a rejected candidate. No account data is mutated here.
pub fn project_atomic(
    program: ProgramV2<'_>,
    scalars: &[u64],
    identities: &[[u8; 32]],
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
    scratch_lamports: &mut [u64],
    output_lamports: &mut [u64],
) -> Result<()> {
    program.require_register_widths(scalars, identities)?;
    let account_count = usize::from(program.account_count);
    if accounts.len() != account_count
        || permissions.len() != account_count
        || scratch_lamports.len() != account_count
        || output_lamports.len() != account_count
    {
        return Err(Error::WidthMismatch);
    }
    for (destination, account) in scratch_lamports.iter_mut().zip(accounts) {
        *destination = account.lamports;
    }
    let mut index = 0_u16;
    while index < program.instruction_count {
        let effect = program.resolved_effect(index, scalars, identities)?;
        project_effect(effect, accounts, permissions, scratch_lamports)?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    output_lamports.copy_from_slice(scratch_lamports);
    Ok(())
}

/// Project lamports, data-write bounds, and all typed fixed-role requests.
///
/// Both output slices remain unchanged on refusal. Scratch slices may contain
/// a rejected candidate. The composing Trading adapter must reauthenticate
/// the selected fixed role, invoke it with the exact resolved account subframe
/// and request slice, check return-data producer/receipt/postconditions, and
/// only then apply local writes and acknowledge Core.
#[allow(clippy::too_many_arguments)]
pub fn project_with_requests_atomic(
    program: ProgramV2<'_>,
    scalars: &[u64],
    identities: &[[u8; 32]],
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
    scratch_lamports: &mut [u64],
    output_lamports: &mut [u64],
    scratch_request: &mut [u8],
    output_request: &mut [u8],
) -> Result<()> {
    program.require_register_widths(scalars, identities)?;
    let account_count = usize::from(program.account_count);
    let request_bytes = usize::from(program.request_bytes);
    if accounts.len() != account_count
        || permissions.len() != account_count
        || scratch_lamports.len() != account_count
        || output_lamports.len() != account_count
        || scratch_request.len() != request_bytes
        || output_request.len() != request_bytes
    {
        return Err(Error::WidthMismatch);
    }
    for (destination, account) in scratch_lamports.iter_mut().zip(accounts) {
        *destination = account.lamports;
    }
    scratch_request.fill(0);
    let mut index = 0_u16;
    while index < program.instruction_count {
        let effect = program.resolved_effect(index, scalars, identities)?;
        project_effect_with_request(
            effect,
            accounts,
            permissions,
            scratch_lamports,
            scratch_request,
        )?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    output_lamports.copy_from_slice(scratch_lamports);
    output_request.copy_from_slice(scratch_request);
    Ok(())
}

/// Project effects using an authenticated physical-account alias partition.
///
/// `aliases[index]` is the canonical representative coordinate selected by
/// the authenticated AccountProfile. Representatives name themselves and an
/// alias may only point backward to a representative. Aliases must expose the
/// same immutable account facts and permissions. Local lamport checks and
/// writes are evaluated against the representative, so two coordinates for
/// one physical account cannot manufacture a transfer or bypass overlapping-
/// write checks. Child invocation frames retain their exact coordinates.
///
/// Both output slices remain unchanged on refusal. Scratch slices may contain
/// a rejected candidate.
#[allow(clippy::too_many_arguments)]
pub fn project_with_aliases_and_requests_atomic(
    program: ProgramV2<'_>,
    scalars: &[u64],
    identities: &[[u8; 32]],
    aliases: &[u16],
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
    scratch_lamports: &mut [u64],
    output_lamports: &mut [u64],
    scratch_request: &mut [u8],
    output_request: &mut [u8],
) -> Result<()> {
    program.require_register_widths(scalars, identities)?;
    let account_count = usize::from(program.account_count);
    let request_bytes = usize::from(program.request_bytes);
    if aliases.len() != account_count
        || accounts.len() != account_count
        || permissions.len() != account_count
        || scratch_lamports.len() != account_count
        || output_lamports.len() != account_count
        || scratch_request.len() != request_bytes
        || output_request.len() != request_bytes
    {
        return Err(Error::WidthMismatch);
    }
    validate_alias_partition(aliases, accounts, permissions)?;
    require_alias_write_nonoverlap(program, scalars, identities, aliases)?;
    for (destination, account) in scratch_lamports.iter_mut().zip(accounts) {
        *destination = account.lamports;
    }
    scratch_request.fill(0);
    let mut index = 0_u16;
    while index < program.instruction_count {
        let effect = program.resolved_effect(index, scalars, identities)?;
        project_effect_with_request_and_aliases(
            effect,
            aliases,
            accounts,
            permissions,
            scratch_lamports,
            scratch_request,
        )?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    for (index, output) in output_lamports.iter_mut().enumerate() {
        *output = *scratch_lamports
            .get(alias_index(aliases, index)?)
            .ok_or(Error::InvalidAlias)?;
    }
    output_request.copy_from_slice(scratch_request);
    Ok(())
}

fn validate_alias_partition(
    aliases: &[u16],
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
) -> Result<()> {
    for index in 0..aliases.len() {
        let representative = alias_index(aliases, index)?;
        if alias_index(aliases, representative)? != representative {
            return Err(Error::InvalidAlias);
        }
        if index != representative
            && (accounts.get(index) != accounts.get(representative)
                || permissions.get(index) != permissions.get(representative))
        {
            return Err(Error::InvalidAlias);
        }
    }
    Ok(())
}

fn alias_index(aliases: &[u16], index: usize) -> Result<usize> {
    let representative = usize::from(*aliases.get(index).ok_or(Error::InvalidAlias)?);
    if representative <= index && representative < aliases.len() {
        Ok(representative)
    } else {
        Err(Error::InvalidAlias)
    }
}

fn require_alias_write_nonoverlap(
    program: ProgramV2<'_>,
    scalars: &[u64],
    identities: &[[u8; 32]],
    aliases: &[u16],
) -> Result<()> {
    let mut right_index = 0_u16;
    while right_index < program.instruction_count {
        let right = program.resolved_effect(right_index, scalars, identities)?;
        let Some((right_account, right_start, right_width)) = resolved_write_range(right) else {
            right_index = right_index.checked_add(1).ok_or(Error::InvalidLength)?;
            continue;
        };
        let right_account = alias_index(aliases, usize::from(right_account))?;
        let right_end = right_start
            .checked_add(right_width)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut left_index = 0_u16;
        while left_index < right_index {
            let left = program.resolved_effect(left_index, scalars, identities)?;
            if let Some((left_account, left_start, left_width)) = resolved_write_range(left) {
                let left_account = alias_index(aliases, usize::from(left_account))?;
                let left_end = left_start
                    .checked_add(left_width)
                    .ok_or(Error::ArithmeticOverflow)?;
                if left_account == right_account && left_start < right_end && right_start < left_end
                {
                    return Err(Error::OverlappingWrites);
                }
            }
            left_index = left_index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        right_index = right_index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

fn resolved_write_range(effect: ResolvedEffect) -> Option<(u16, usize, usize)> {
    match effect {
        ResolvedEffect::WriteScalar {
            account, offset, ..
        } => Some((account, usize::try_from(offset).ok()?, 8)),
        ResolvedEffect::WriteIdentity {
            account, offset, ..
        } => Some((account, usize::try_from(offset).ok()?, 32)),
        _ => None,
    }
}

fn project_effect_with_request_and_aliases(
    effect: ResolvedEffect,
    aliases: &[u16],
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
    lamports: &mut [u64],
    request: &mut [u8],
) -> Result<()> {
    match effect {
        ResolvedEffect::TransferLamports {
            source,
            destination,
            amount,
        } => {
            let source = alias_index(aliases, usize::from(source))?;
            let destination = alias_index(aliases, usize::from(destination))?;
            if source == destination {
                return Err(Error::InvalidAccount);
            }
            project_effect(
                ResolvedEffect::TransferLamports {
                    source: u16::try_from(source).map_err(|_| Error::InvalidAlias)?,
                    destination: u16::try_from(destination).map_err(|_| Error::InvalidAlias)?,
                    amount,
                },
                accounts,
                permissions,
                lamports,
            )
        }
        ResolvedEffect::WriteScalar {
            account,
            offset,
            value,
        } => project_effect(
            ResolvedEffect::WriteScalar {
                account: u16::try_from(alias_index(aliases, usize::from(account))?)
                    .map_err(|_| Error::InvalidAlias)?,
                offset,
                value,
            },
            accounts,
            permissions,
            lamports,
        ),
        ResolvedEffect::WriteIdentity {
            account,
            offset,
            value,
        } => project_effect(
            ResolvedEffect::WriteIdentity {
                account: u16::try_from(alias_index(aliases, usize::from(account))?)
                    .map_err(|_| Error::InvalidAlias)?,
                offset,
                value,
            },
            accounts,
            permissions,
            lamports,
        ),
        ResolvedEffect::RequireLamportsEq { account, value } => project_effect(
            ResolvedEffect::RequireLamportsEq {
                account: u16::try_from(alias_index(aliases, usize::from(account))?)
                    .map_err(|_| Error::InvalidAlias)?,
                value,
            },
            accounts,
            permissions,
            lamports,
        ),
        request_effect => {
            project_effect_with_request(request_effect, accounts, permissions, lamports, request)
        }
    }
}

fn project_effect(
    effect: ResolvedEffect,
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
    lamports: &mut [u64],
) -> Result<()> {
    match effect {
        ResolvedEffect::TransferLamports {
            source,
            destination,
            amount,
        } => {
            let source_index = usize::from(source);
            let destination_index = usize::from(destination);
            let source_permission = permissions.get(source_index).ok_or(Error::InvalidAccount)?;
            let destination_permission = permissions
                .get(destination_index)
                .ok_or(Error::InvalidAccount)?;
            if !source_permission.may_debit_lamports || !destination_permission.may_credit_lamports
            {
                return Err(Error::PermissionDenied);
            }
            let source_after = lamports
                .get(source_index)
                .copied()
                .ok_or(Error::InvalidAccount)?
                .checked_sub(amount)
                .ok_or(Error::InsufficientLamports)?;
            let destination_after = lamports
                .get(destination_index)
                .copied()
                .ok_or(Error::InvalidAccount)?
                .checked_add(amount)
                .ok_or(Error::ArithmeticOverflow)?;
            *lamports
                .get_mut(source_index)
                .ok_or(Error::InvalidAccount)? = source_after;
            *lamports
                .get_mut(destination_index)
                .ok_or(Error::InvalidAccount)? = destination_after;
        }
        ResolvedEffect::WriteScalar {
            account, offset, ..
        } => validate_write(account, offset, 8, accounts, permissions)?,
        ResolvedEffect::WriteIdentity {
            account, offset, ..
        } => validate_write(account, offset, 32, accounts, permissions)?,
        ResolvedEffect::RequireLamportsEq { account, value } => {
            if lamports
                .get(usize::from(account))
                .copied()
                .ok_or(Error::InvalidAccount)?
                != value
            {
                return Err(Error::CheckFailed);
            }
        }
        ResolvedEffect::WriteRequestScalar { .. }
        | ResolvedEffect::WriteRequestIdentity { .. }
        | ResolvedEffect::InvokeRole { .. } => return Err(Error::RequestBufferRequired),
    }
    Ok(())
}

fn project_effect_with_request(
    effect: ResolvedEffect,
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
    lamports: &mut [u64],
    request: &mut [u8],
) -> Result<()> {
    match effect {
        ResolvedEffect::WriteRequestScalar { offset, value } => {
            write_request(request, offset, &value.to_le_bytes())
        }
        ResolvedEffect::WriteRequestIdentity { offset, value } => {
            write_request(request, offset, &value)
        }
        ResolvedEffect::InvokeRole {
            account_start,
            account_count,
            request_offset,
            request_len,
            ..
        } => {
            checked_range(
                usize::from(account_start),
                usize::from(account_count),
                accounts.len(),
            )?;
            let request_start =
                usize::try_from(request_offset).map_err(|_| Error::DataOutOfBounds)?;
            let request_width = usize::try_from(request_len).map_err(|_| Error::DataOutOfBounds)?;
            checked_range(request_start, request_width, request.len())?;
            Ok(())
        }
        local => project_effect(local, accounts, permissions, lamports),
    }
}

fn write_request(request: &mut [u8], offset: u32, value: &[u8]) -> Result<()> {
    let start = usize::try_from(offset).map_err(|_| Error::DataOutOfBounds)?;
    let end = checked_range(start, value.len(), request.len())?;
    request
        .get_mut(start..end)
        .ok_or(Error::DataOutOfBounds)?
        .copy_from_slice(value);
    Ok(())
}

fn checked_range(start: usize, width: usize, limit: usize) -> Result<usize> {
    let end = start.checked_add(width).ok_or(Error::DataOutOfBounds)?;
    if width == 0 || end > limit {
        Err(Error::DataOutOfBounds)
    } else {
        Ok(end)
    }
}

fn validate_write(
    account: u16,
    offset: u32,
    width: usize,
    accounts: &[AccountInput],
    permissions: &[AccountPermission],
) -> Result<()> {
    let index = usize::from(account);
    if !permissions
        .get(index)
        .ok_or(Error::InvalidAccount)?
        .may_write_data
    {
        return Err(Error::PermissionDenied);
    }
    let start = usize::try_from(offset).map_err(|_| Error::DataOutOfBounds)?;
    let end = start.checked_add(width).ok_or(Error::DataOutOfBounds)?;
    if end > accounts.get(index).ok_or(Error::InvalidAccount)?.data_len {
        return Err(Error::DataOutOfBounds);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Instruction {
    opcode: u8,
    auxiliary: u8,
    account_a: u16,
    account_b: u16,
    register: u16,
    data_offset: u32,
    extra: u32,
}

impl Instruction {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        Ok(Self {
            opcode: byte(bytes, add(offset, OPCODE_OFFSET)?)?,
            auxiliary: byte(bytes, add(offset, AUXILIARY_OFFSET)?)?,
            account_a: read_u16(bytes, add(offset, ACCOUNT_A_OFFSET)?)?,
            account_b: read_u16(bytes, add(offset, ACCOUNT_B_OFFSET)?)?,
            register: read_u16(bytes, add(offset, REGISTER_OFFSET)?)?,
            data_offset: read_u32(bytes, add(offset, DATA_OFFSET)?)?,
            extra: read_u32(bytes, add(offset, EXTRA_OFFSET)?)?,
        })
    }

    fn validate(self, program: ProgramV2<'_>) -> Result<()> {
        match self.opcode {
            OP_TRANSFER_LAMPORTS => {
                require_account(self.account_a, program.account_count)?;
                require_account(self.account_b, program.account_count)?;
                require_scalar(self.register, program.scalar_count)?;
                if self.auxiliary != 0 || self.data_offset != 0 || self.extra != 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
                if self.account_a == self.account_b {
                    return Err(Error::InvalidAccount);
                }
            }
            OP_WRITE_SCALAR => {
                require_account(self.account_a, program.account_count)?;
                require_scalar(self.register, program.scalar_count)?;
                if self.auxiliary != 0 || self.account_b != 0 || self.extra != 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
            }
            OP_WRITE_IDENTITY => {
                require_account(self.account_a, program.account_count)?;
                require_identity(self.register, program.identity_count)?;
                if self.auxiliary != 0 || self.account_b != 0 || self.extra != 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
            }
            OP_REQUIRE_LAMPORTS_EQ => {
                require_account(self.account_a, program.account_count)?;
                require_scalar(self.register, program.scalar_count)?;
                if self.auxiliary != 0
                    || self.account_b != 0
                    || self.data_offset != 0
                    || self.extra != 0
                {
                    return Err(Error::NonCanonicalInstruction);
                }
            }
            OP_WRITE_REQUEST_SCALAR => {
                require_scalar(self.register, program.scalar_count)?;
                if self.auxiliary != 0
                    || self.account_a != 0
                    || self.account_b != 0
                    || self.extra != 0
                {
                    return Err(Error::NonCanonicalInstruction);
                }
                require_buffer_range(self.data_offset, 8, program.request_bytes)?;
            }
            OP_WRITE_REQUEST_IDENTITY => {
                require_identity(self.register, program.identity_count)?;
                if self.auxiliary != 0
                    || self.account_a != 0
                    || self.account_b != 0
                    || self.extra != 0
                {
                    return Err(Error::NonCanonicalInstruction);
                }
                require_buffer_range(self.data_offset, 32, program.request_bytes)?;
            }
            OP_INVOKE_ROLE => {
                FixedRole::decode(self.account_a)?;
                if self.auxiliary == 0 || self.register != 0 || self.extra == 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
                require_account_range(
                    self.account_b,
                    u16::from(self.auxiliary),
                    program.account_count,
                )?;
                require_buffer_range(self.data_offset, self.extra, program.request_bytes)?;
            }
            OP_INVOKE_ROLE_IF_NONZERO => {
                FixedRole::decode(self.account_a)?;
                require_scalar(self.register, program.scalar_count)?;
                if self.auxiliary == 0 || self.extra == 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
                require_account_range(
                    self.account_b,
                    u16::from(self.auxiliary),
                    program.account_count,
                )?;
                require_buffer_range(self.data_offset, self.extra, program.request_bytes)?;
            }
            _ => return Err(Error::UnknownOpcode),
        }
        Ok(())
    }

    fn resolve(self, scalars: &[u64], identities: &[[u8; 32]]) -> Result<ResolvedEffect> {
        match self.opcode {
            OP_TRANSFER_LAMPORTS => Ok(ResolvedEffect::TransferLamports {
                source: self.account_a,
                destination: self.account_b,
                amount: scalar(scalars, self.register)?,
            }),
            OP_WRITE_SCALAR => Ok(ResolvedEffect::WriteScalar {
                account: self.account_a,
                offset: self.data_offset,
                value: scalar(scalars, self.register)?,
            }),
            OP_WRITE_IDENTITY => Ok(ResolvedEffect::WriteIdentity {
                account: self.account_a,
                offset: self.data_offset,
                value: identity(identities, self.register)?,
            }),
            OP_REQUIRE_LAMPORTS_EQ => Ok(ResolvedEffect::RequireLamportsEq {
                account: self.account_a,
                value: scalar(scalars, self.register)?,
            }),
            OP_WRITE_REQUEST_SCALAR => Ok(ResolvedEffect::WriteRequestScalar {
                offset: self.data_offset,
                value: scalar(scalars, self.register)?,
            }),
            OP_WRITE_REQUEST_IDENTITY => Ok(ResolvedEffect::WriteRequestIdentity {
                offset: self.data_offset,
                value: identity(identities, self.register)?,
            }),
            OP_INVOKE_ROLE => Ok(ResolvedEffect::InvokeRole {
                role: FixedRole::decode(self.account_a)?,
                account_start: self.account_b,
                account_count: u16::from(self.auxiliary),
                request_offset: self.data_offset,
                request_len: self.extra,
                enabled: true,
            }),
            OP_INVOKE_ROLE_IF_NONZERO => Ok(ResolvedEffect::InvokeRole {
                role: FixedRole::decode(self.account_a)?,
                account_start: self.account_b,
                account_count: u16::from(self.auxiliary),
                request_offset: self.data_offset,
                request_len: self.extra,
                enabled: scalar(scalars, self.register)? != 0,
            }),
            _ => Err(Error::UnknownOpcode),
        }
    }

    fn write_range(self) -> Option<(u16, usize, usize)> {
        let width = match self.opcode {
            OP_WRITE_SCALAR => 8,
            OP_WRITE_IDENTITY => 32,
            _ => return None,
        };
        Some((
            self.account_a,
            usize::try_from(self.data_offset).ok()?,
            width,
        ))
    }

    fn request_write_range(self) -> Option<(usize, usize)> {
        let width = match self.opcode {
            OP_WRITE_REQUEST_SCALAR => 8,
            OP_WRITE_REQUEST_IDENTITY => 32,
            _ => return None,
        };
        Some((usize::try_from(self.data_offset).ok()?, width))
    }
}

fn require_account_range(start: u16, count: u16, limit: u16) -> Result<()> {
    let end = start.checked_add(count).ok_or(Error::InvalidAccount)?;
    if count != 0 && end <= limit {
        Ok(())
    } else {
        Err(Error::InvalidAccount)
    }
}

fn require_buffer_range(start: u32, width: u32, limit: u16) -> Result<()> {
    let end = start.checked_add(width).ok_or(Error::DataOutOfBounds)?;
    if width != 0 && end <= u32::from(limit) {
        Ok(())
    } else {
        Err(Error::DataOutOfBounds)
    }
}

fn require_account(index: u16, count: u16) -> Result<()> {
    if index < count {
        Ok(())
    } else {
        Err(Error::InvalidAccount)
    }
}

fn require_scalar(index: u16, count: u16) -> Result<()> {
    if index < count {
        Ok(())
    } else {
        Err(Error::InvalidRegister)
    }
}

fn require_identity(index: u16, count: u16) -> Result<()> {
    if index < count {
        Ok(())
    } else {
        Err(Error::InvalidRegister)
    }
}

fn scalar(values: &[u64], index: u16) -> Result<u64> {
    values
        .get(usize::from(index))
        .copied()
        .ok_or(Error::InvalidRegister)
}

fn identity(values: &[[u8; 32]], index: u16) -> Result<[u8; 32]> {
    values
        .get(usize::from(index))
        .copied()
        .ok_or(Error::InvalidRegister)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = add(offset, 2)?;
    let value: &[u8; 2] = bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u16::from_le_bytes(*value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = add(offset, 4)?;
    let value: &[u8; 4] = bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u32::from_le_bytes(*value))
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use super::encode::{
        EffectGeometryV2, EffectInstructionV2, RoleFrameV2, effect_program_v2_bytes,
        encode_effect_program_v2_atomic,
    };
    use super::*;

    /// Exact bytes `fixture_program` produced before it had a public encoder.
    ///
    /// This is the byte-identity pin for the fixture replacement: it was
    /// captured from the hand-written builder this module used to carry, and
    /// the encoder has to reproduce it exactly. Unlike the transition VM and
    /// AccountProfile generations there is no Lean-emitted V2 effect artifact
    /// to compare against, so the prior wire bytes are the oracle.
    const FIXTURE_PROGRAM_BYTES_V2: [u8; 96] = [
        0x44, 0x43, 0x45, 0x32, 0x02, 0x00, 0x05, 0x00, 0x03, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    /// One canonical program, built only by the crate's public encoder.
    ///
    /// This is the single fixture authority in the module. Nothing here writes
    /// a header field or an operand slot by hand any more, so a fixture cannot
    /// disagree with the artifact a real author would emit.
    fn program(
        accounts: u16,
        scalars: u16,
        identities: u16,
        instructions: &[EffectInstructionV2],
    ) -> vec::Vec<u8> {
        request_program(accounts, scalars, identities, 0, instructions)
    }

    /// One canonical program that also declares a request buffer.
    fn request_program(
        accounts: u16,
        scalars: u16,
        identities: u16,
        request_bytes: u16,
        instructions: &[EffectInstructionV2],
    ) -> vec::Vec<u8> {
        let width = effect_program_v2_bytes(instructions.len()).expect("fixture width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_effect_program_v2_atomic(
            EffectGeometryV2 {
                accounts,
                scalars,
                identities,
                request_bytes,
            },
            instructions,
            &mut scratch,
            &mut output,
        )
        .expect("fixture program");
        output
    }

    /// Overwrite one byte of a canonical artifact.
    ///
    /// Every hostile case below is exactly one byte away from something the
    /// encoder accepted, which is a stronger statement than a fixture that was
    /// never canonical to begin with.
    fn patched(canonical: &[u8], offset: usize, value: u8) -> vec::Vec<u8> {
        let mut hostile = canonical.to_vec();
        *hostile.get_mut(offset).expect("patch offset") = value;
        hostile
    }

    fn fixture_program() -> vec::Vec<u8> {
        program(
            3,
            4,
            1,
            &[
                EffectInstructionV2::transfer_lamports(0, 1, 0),
                EffectInstructionV2::write_u64(0, 8, 1),
                EffectInstructionV2::write_identity(0, 32, 0),
                EffectInstructionV2::require_lamports_eq(0, 2),
                EffectInstructionV2::require_lamports_eq(1, 3),
            ],
        )
    }

    /// The public encoder reproduces the module's prior fixture byte for byte.
    #[test]
    fn the_public_encoder_reproduces_the_prior_fixture_bytes() {
        assert_eq!(fixture_program().as_slice(), FIXTURE_PROGRAM_BYTES_V2);
    }

    /// The encoder is total: it refuses whatever the decoder refuses, and a
    /// refusal never leaves partial bytes in `output`.
    #[test]
    fn the_public_encoder_refuses_what_the_decoder_refuses() {
        let attempt = |geometry: EffectGeometryV2,
                       instructions: &[EffectInstructionV2],
                       width: usize| {
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0xcd_u8; width];
            let result =
                encode_effect_program_v2_atomic(geometry, instructions, &mut scratch, &mut output);
            assert!(
                output.iter().all(|byte| *byte == 0xcd),
                "a refused encode left bytes in output"
            );
            result
        };
        let geometry = |accounts, scalars, identities, request_bytes| EffectGeometryV2 {
            accounts,
            scalars,
            identities,
            request_bytes,
        };
        let one = effect_program_v2_bytes(1).expect("width");

        // An account coordinate outside the declared vector.
        assert_eq!(
            attempt(
                geometry(1, 1, 0, 0),
                &[EffectInstructionV2::require_lamports_eq(1, 0)],
                one,
            ),
            Err(Error::InvalidAccount)
        );
        // A transfer whose source and destination are the same coordinate.
        assert_eq!(
            attempt(
                geometry(2, 1, 0, 0),
                &[EffectInstructionV2::transfer_lamports(0, 0, 0)],
                one,
            ),
            Err(Error::InvalidAccount)
        );
        // A scalar register outside the declared bank.
        assert_eq!(
            attempt(
                geometry(1, 1, 0, 0),
                &[EffectInstructionV2::require_lamports_eq(0, 1)],
                one,
            ),
            Err(Error::InvalidRegister)
        );
        // A request write that runs off the end of the declared buffer.
        assert_eq!(
            attempt(
                geometry(1, 1, 0, 4),
                &[EffectInstructionV2::write_request_u64(0, 0)],
                one,
            ),
            Err(Error::DataOutOfBounds)
        );
        // Two data writes to the same account that overlap.
        assert_eq!(
            attempt(
                geometry(1, 1, 1, 0),
                &[
                    EffectInstructionV2::write_u64(0, 8, 0),
                    EffectInstructionV2::write_identity(0, 0, 0),
                ],
                effect_program_v2_bytes(2).expect("width"),
            ),
            Err(Error::OverlappingWrites)
        );
        // No instruction, and no account coordinate.
        assert_eq!(
            attempt(
                geometry(1, 1, 0, 0),
                &[],
                effect_program_v2_bytes(0).expect("width"),
            ),
            Err(Error::EmptyProgram)
        );
        assert_eq!(
            attempt(
                geometry(0, 1, 0, 0),
                &[EffectInstructionV2::require_lamports_eq(0, 0)],
                one,
            ),
            Err(Error::EmptyProgram)
        );
        // A child frame or child request of width zero.
        assert_eq!(
            attempt(
                geometry(2, 1, 0, 8),
                &[EffectInstructionV2::invoke_role(
                    FixedRole::Claims,
                    RoleFrameV2 {
                        account_start: 0,
                        account_count: 0,
                        request_offset: 0,
                        request_len: 8,
                    },
                )],
                one,
            ),
            Err(Error::NonCanonicalInstruction)
        );
        // Buffers that are not the exact encoded width.
        assert_eq!(
            attempt(
                geometry(1, 1, 0, 0),
                &[EffectInstructionV2::require_lamports_eq(0, 0)],
                one + 1,
            ),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn complete_projection_is_atomic_and_resolvable() {
        let bytes = fixture_program();
        let program = ProgramV2::decode(&bytes).expect("effect program");
        let scalars = [30, 77, 70, 35];
        let identities = [[9; 32]];
        let accounts = [
            AccountInput {
                lamports: 100,
                data_len: 80,
            },
            AccountInput {
                lamports: 5,
                data_len: 0,
            },
            AccountInput {
                lamports: 11,
                data_len: 0,
            },
        ];
        let permissions = [
            AccountPermission::program_owned_mutable(),
            AccountPermission::lamport_receiver(),
            AccountPermission::read_only(),
        ];
        let mut scratch = [0; 3];
        let mut output = [0xa5; 3];
        project_atomic(
            program,
            &scalars,
            &identities,
            &accounts,
            &permissions,
            &mut scratch,
            &mut output,
        )
        .expect("projection");
        assert_eq!(output, [70, 35, 11]);
        assert_eq!(
            program.resolved_effect(2, &scalars, &identities),
            Ok(ResolvedEffect::WriteIdentity {
                account: 0,
                offset: 32,
                value: [9; 32],
            })
        );
    }

    #[test]
    fn late_refusal_preserves_output() {
        let bytes = fixture_program();
        let program = ProgramV2::decode(&bytes).expect("effect program");
        let accounts = [
            AccountInput {
                lamports: 100,
                data_len: 80,
            },
            AccountInput {
                lamports: 5,
                data_len: 0,
            },
            AccountInput {
                lamports: 11,
                data_len: 0,
            },
        ];
        let permissions = [
            AccountPermission::program_owned_mutable(),
            AccountPermission::lamport_receiver(),
            AccountPermission::read_only(),
        ];
        let mut scratch = [0; 3];
        let mut output = [0xa5; 3];
        assert_eq!(
            project_atomic(
                program,
                &[30, 77, 70, 36],
                &[[9; 32]],
                &accounts,
                &permissions,
                &mut scratch,
                &mut output,
            ),
            Err(Error::CheckFailed)
        );
        assert_eq!(output, [0xa5; 3]);
        assert_eq!(scratch, [70, 35, 11]);
    }

    #[test]
    fn permissions_and_bounds_fail_closed() {
        let bytes = fixture_program();
        let program = ProgramV2::decode(&bytes).expect("effect program");
        let accounts = [
            AccountInput {
                lamports: 100,
                data_len: 63,
            },
            AccountInput {
                lamports: 5,
                data_len: 0,
            },
            AccountInput {
                lamports: 11,
                data_len: 0,
            },
        ];
        let mut scratch = [0; 3];
        let mut output = [0xa5; 3];
        assert_eq!(
            project_atomic(
                program,
                &[30, 77, 70, 35],
                &[[9; 32]],
                &accounts,
                &[
                    AccountPermission::program_owned_mutable(),
                    AccountPermission::lamport_receiver(),
                    AccountPermission::read_only(),
                ],
                &mut scratch,
                &mut output,
            ),
            Err(Error::DataOutOfBounds)
        );
        assert_eq!(output, [0xa5; 3]);

        let mut denied = accounts;
        denied[0].data_len = 80;
        assert_eq!(
            project_atomic(
                program,
                &[30, 77, 70, 35],
                &[[9; 32]],
                &denied,
                &[
                    AccountPermission::read_only(),
                    AccountPermission::lamport_receiver(),
                    AccountPermission::read_only(),
                ],
                &mut scratch,
                &mut output,
            ),
            Err(Error::PermissionDenied)
        );
        assert_eq!(output, [0xa5; 3]);
    }

    #[test]
    fn hostile_programs_refuse_without_alternate_decoders() {
        let canonical = fixture_program();
        for length in 0..canonical.len() {
            assert_eq!(
                ProgramV2::decode(canonical.get(..length).expect("fixture prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut trailing = canonical.clone();
        trailing.push(0);
        assert_eq!(ProgramV2::decode(&trailing), Err(Error::InvalidLength));

        let mut reserved = canonical.clone();
        *reserved
            .get_mut(HEADER_BYTES + 1)
            .expect("fixture reserved byte") = 1;
        assert_eq!(
            ProgramV2::decode(&reserved),
            Err(Error::NonCanonicalInstruction)
        );

        // Two writes to one account, moved into each other by one patched byte.
        let overlapping = patched(
            &program(
                1,
                1,
                1,
                &[
                    EffectInstructionV2::write_u64(0, 32, 0),
                    EffectInstructionV2::write_identity(0, 0, 0),
                ],
            ),
            HEADER_BYTES + DATA_OFFSET,
            8,
        );
        assert_eq!(
            ProgramV2::decode(&overlapping),
            Err(Error::OverlappingWrites)
        );
    }

    #[test]
    fn descriptor_selected_effect_identity_is_exact() {
        let bytes = fixture_program();
        let exact = [9; 32];
        assert_eq!(
            ProgramV2::decode_selected(exact, exact, &bytes)
                .expect("exact authenticated effect program")
                .bytes(),
            bytes
        );
        assert_eq!(
            ProgramV2::decode_selected([0; 32], exact, &bytes),
            Err(Error::ZeroProgramIdentity)
        );
        assert_eq!(
            ProgramV2::decode_selected(exact, [8; 32], &bytes),
            Err(Error::ProgramIdentityMismatch)
        );
    }

    #[test]
    fn authenticated_aliases_share_one_physical_lamport_state() {
        let bytes = program(
            3,
            3,
            0,
            &[
                EffectInstructionV2::transfer_lamports(0, 2, 0),
                EffectInstructionV2::require_lamports_eq(1, 1),
                EffectInstructionV2::require_lamports_eq(2, 2),
            ],
        );
        let program = ProgramV2::decode(&bytes).expect("alias-aware effect program");
        let accounts = [
            AccountInput {
                lamports: 100,
                data_len: 0,
            },
            AccountInput {
                lamports: 100,
                data_len: 0,
            },
            AccountInput {
                lamports: 0,
                data_len: 0,
            },
        ];
        let permissions = [
            AccountPermission::program_owned_mutable(),
            AccountPermission::program_owned_mutable(),
            AccountPermission::lamport_receiver(),
        ];
        let mut scratch_lamports = [0_u64; 3];
        let mut output_lamports = [0xa5_u64; 3];
        let mut scratch_request = [];
        let mut output_request = [];
        project_with_aliases_and_requests_atomic(
            program,
            &[30, 70, 30],
            &[],
            &[0, 0, 2],
            &accounts,
            &permissions,
            &mut scratch_lamports,
            &mut output_lamports,
            &mut scratch_request,
            &mut output_request,
        )
        .expect("one physical debit through its representative");
        assert_eq!(output_lamports, [70, 70, 30]);
    }

    #[test]
    fn alias_self_transfer_and_cross_coordinate_overlap_refuse_atomically() {
        let transfer_bytes = program(2, 1, 0, &[EffectInstructionV2::transfer_lamports(0, 1, 0)]);
        let transfer = ProgramV2::decode(&transfer_bytes).expect("coordinate-distinct transfer");
        let alias_accounts = [
            AccountInput {
                lamports: 100,
                data_len: 64,
            },
            AccountInput {
                lamports: 100,
                data_len: 64,
            },
        ];
        let alias_permissions = [AccountPermission::program_owned_mutable(); 2];
        let mut scratch_lamports = [0_u64; 2];
        let mut output_lamports = [0xa5_u64; 2];
        let mut scratch_request = [];
        let mut output_request = [];
        assert_eq!(
            project_with_aliases_and_requests_atomic(
                transfer,
                &[1],
                &[],
                &[0, 0],
                &alias_accounts,
                &alias_permissions,
                &mut scratch_lamports,
                &mut output_lamports,
                &mut scratch_request,
                &mut output_request,
            ),
            Err(Error::InvalidAccount)
        );
        assert_eq!(output_lamports, [0xa5; 2]);

        let write_bytes = program(
            2,
            1,
            1,
            &[
                EffectInstructionV2::write_u64(0, 0, 0),
                EffectInstructionV2::write_identity(1, 0, 0),
            ],
        );
        let writes = ProgramV2::decode(&write_bytes)
            .expect("coordinate-distinct writes are syntactically valid");
        assert_eq!(
            project_with_aliases_and_requests_atomic(
                writes,
                &[1],
                &[[2; 32]],
                &[0, 0],
                &alias_accounts,
                &alias_permissions,
                &mut scratch_lamports,
                &mut output_lamports,
                &mut scratch_request,
                &mut output_request,
            ),
            Err(Error::OverlappingWrites)
        );
        assert_eq!(output_lamports, [0xa5; 2]);

        assert_eq!(
            project_with_aliases_and_requests_atomic(
                transfer,
                &[1],
                &[],
                &[1, 1],
                &alias_accounts,
                &alias_permissions,
                &mut scratch_lamports,
                &mut output_lamports,
                &mut scratch_request,
                &mut output_request,
            ),
            Err(Error::InvalidAlias)
        );
        assert_eq!(output_lamports, [0xa5; 2]);
    }

    #[test]
    fn register_and_account_widths_are_runtime_values() {
        let bytes = program(
            300,
            258,
            257,
            &[
                EffectInstructionV2::transfer_lamports(299, 298, 257),
                EffectInstructionV2::write_identity(297, 0, 256),
            ],
        );
        let decoded = ProgramV2::decode(&bytes).expect("wide effect program");
        assert_eq!(decoded.account_count(), 300);
        assert_eq!(decoded.scalar_count(), 258);
        assert_eq!(decoded.identity_count(), 257);
    }

    #[test]
    fn fixed_role_requests_are_projected_and_conditionally_emitted() {
        let bytes = request_program(
            4,
            2,
            1,
            48,
            &[
                EffectInstructionV2::write_request_u64(0, 0),
                EffectInstructionV2::write_request_identity(16, 0),
                EffectInstructionV2::invoke_role_if_nonzero(
                    FixedRole::Claims,
                    RoleFrameV2 {
                        account_start: 1,
                        account_count: 2,
                        request_offset: 0,
                        request_len: 48,
                    },
                    1,
                ),
            ],
        );
        let program = ProgramV2::decode(&bytes).expect("typed role effect program");
        let accounts = [AccountInput {
            lamports: 1,
            data_len: 0,
        }; 4];
        let permissions = [AccountPermission::read_only(); 4];
        let mut scratch_lamports = [0; 4];
        let mut output_lamports = [9; 4];
        let mut scratch_request = [0xa5; 48];
        let mut output_request = [0xa5; 48];
        project_with_requests_atomic(
            program,
            &[42, 0],
            &[[7; 32]],
            &accounts,
            &permissions,
            &mut scratch_lamports,
            &mut output_lamports,
            &mut scratch_request,
            &mut output_request,
        )
        .expect("disabled request is still canonically projected");
        assert_eq!(output_lamports, [1; 4]);
        assert_eq!(
            output_request.get(..8),
            Some(42_u64.to_le_bytes().as_slice())
        );
        assert_eq!(output_request.get(8..16), Some([0; 8].as_slice()));
        assert_eq!(output_request.get(16..48), Some([7; 32].as_slice()));
        assert_eq!(
            program.resolved_effect(2, &[42, 0], &[[7; 32]]),
            Ok(ResolvedEffect::InvokeRole {
                role: FixedRole::Claims,
                account_start: 1,
                account_count: 2,
                request_offset: 0,
                request_len: 48,
                enabled: false,
            })
        );
        assert_eq!(
            program.resolved_effect(2, &[42, 1], &[[7; 32]]),
            Ok(ResolvedEffect::InvokeRole {
                role: FixedRole::Claims,
                account_start: 1,
                account_count: 2,
                request_offset: 0,
                request_len: 48,
                enabled: true,
            })
        );

        let mut ordinary_output = [9; 4];
        assert_eq!(
            project_atomic(
                program,
                &[42, 1],
                &[[7; 32]],
                &accounts,
                &permissions,
                &mut scratch_lamports,
                &mut ordinary_output,
            ),
            Err(Error::RequestBufferRequired)
        );
        assert_eq!(ordinary_output, [9; 4]);
    }

    #[test]
    fn request_routes_refuse_trading_targets_and_bad_ranges() {
        // Trading is not a fixed role, so `FixedRole` cannot name it and the
        // encoder cannot emit it. One patched byte retargets a canonical
        // Claims route at role coordinate two.
        let trading = patched(
            &request_program(
                2,
                1,
                0,
                8,
                &[EffectInstructionV2::invoke_role(
                    FixedRole::Claims,
                    RoleFrameV2 {
                        account_start: 0,
                        account_count: 1,
                        request_offset: 0,
                        request_len: 8,
                    },
                )],
            ),
            HEADER_BYTES + ACCOUNT_A_OFFSET,
            2,
        );
        assert_eq!(
            ProgramV2::decode(&trading),
            Err(Error::NonCanonicalInstruction)
        );

        // A child frame that runs off the end of the account vector, made so by
        // narrowing the declared account count under a canonical route.
        let outside = patched(
            &request_program(
                3,
                1,
                0,
                12,
                &[EffectInstructionV2::invoke_role(
                    FixedRole::Claims,
                    RoleFrameV2 {
                        account_start: 1,
                        account_count: 2,
                        request_offset: 4,
                        request_len: 8,
                    },
                )],
            ),
            ACCOUNT_COUNT_OFFSET,
            2,
        );
        assert_eq!(ProgramV2::decode(&outside), Err(Error::InvalidAccount));
    }
}
