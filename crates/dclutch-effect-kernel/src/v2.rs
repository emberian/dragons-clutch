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

/// Canonical runtime-width effect-program magic.
pub const MAGIC: [u8; 4] = *b"DCE2";
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
const HEADER_RESERVED_OFFSET: usize = 14;

const OPCODE_OFFSET: usize = 0;
const INSTRUCTION_RESERVED_BYTE_OFFSET: usize = 1;
const ACCOUNT_A_OFFSET: usize = 2;
const ACCOUNT_B_OFFSET: usize = 4;
const REGISTER_OFFSET: usize = 6;
const DATA_OFFSET: usize = 8;
const INSTRUCTION_RESERVED_OFFSET: usize = 12;

const OP_TRANSFER_LAMPORTS: u8 = 0;
const OP_WRITE_SCALAR: u8 = 1;
const OP_WRITE_IDENTITY: u8 = 2;
const OP_REQUIRE_LAMPORTS_EQ: u8 = 3;

/// Stable hostile-decode or projection refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
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
    /// An instruction addressed an absent scalar or identity coordinate.
    InvalidRegister,
    /// Caller-owned account, register, scratch, or output widths differed.
    WidthMismatch,
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
    bytes: &'a [u8],
}

impl<'a> ProgramV2<'a> {
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
        if byte(bytes, FLAGS_OFFSET)? != 0
            || bytes
                .get(HEADER_RESERVED_OFFSET..HEADER_BYTES)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|value| *value != 0)
        {
            return Err(Error::NonCanonicalHeader);
        }
        let instruction_count = read_u16(bytes, INSTRUCTION_COUNT_OFFSET)?;
        let account_count = read_u16(bytes, ACCOUNT_COUNT_OFFSET)?;
        let scalar_count = read_u16(bytes, SCALAR_COUNT_OFFSET)?;
        let identity_count = read_u16(bytes, IDENTITY_COUNT_OFFSET)?;
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
        Ok(())
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
    }
    Ok(())
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
    account_a: u16,
    account_b: u16,
    register: u16,
    data_offset: u32,
}

impl Instruction {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        if byte(bytes, add(offset, INSTRUCTION_RESERVED_BYTE_OFFSET)?)? != 0
            || bytes
                .get(add(offset, INSTRUCTION_RESERVED_OFFSET)?..add(offset, INSTRUCTION_BYTES)?)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|value| *value != 0)
        {
            return Err(Error::NonCanonicalInstruction);
        }
        Ok(Self {
            opcode: byte(bytes, add(offset, OPCODE_OFFSET)?)?,
            account_a: read_u16(bytes, add(offset, ACCOUNT_A_OFFSET)?)?,
            account_b: read_u16(bytes, add(offset, ACCOUNT_B_OFFSET)?)?,
            register: read_u16(bytes, add(offset, REGISTER_OFFSET)?)?,
            data_offset: read_u32(bytes, add(offset, DATA_OFFSET)?)?,
        })
    }

    fn validate(self, program: ProgramV2<'_>) -> Result<()> {
        require_account(self.account_a, program.account_count)?;
        match self.opcode {
            OP_TRANSFER_LAMPORTS => {
                require_account(self.account_b, program.account_count)?;
                require_scalar(self.register, program.scalar_count)?;
                if self.account_a == self.account_b || self.data_offset != 0 {
                    return Err(Error::InvalidAccount);
                }
            }
            OP_WRITE_SCALAR => {
                require_scalar(self.register, program.scalar_count)?;
                if self.account_b != 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
            }
            OP_WRITE_IDENTITY => {
                require_identity(self.register, program.identity_count)?;
                if self.account_b != 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
            }
            OP_REQUIRE_LAMPORTS_EQ => {
                require_scalar(self.register, program.scalar_count)?;
                if self.account_b != 0 || self.data_offset != 0 {
                    return Err(Error::NonCanonicalInstruction);
                }
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

    use super::*;

    fn instruction(
        opcode: u8,
        account_a: u16,
        account_b: u16,
        register: u16,
        offset: u32,
    ) -> [u8; INSTRUCTION_BYTES] {
        let mut bytes = [0_u8; INSTRUCTION_BYTES];
        bytes[0] = opcode;
        bytes[2..4].copy_from_slice(&account_a.to_le_bytes());
        bytes[4..6].copy_from_slice(&account_b.to_le_bytes());
        bytes[6..8].copy_from_slice(&register.to_le_bytes());
        bytes[8..12].copy_from_slice(&offset.to_le_bytes());
        bytes
    }

    fn program(
        account_count: u16,
        scalar_count: u16,
        identity_count: u16,
        instructions: &[[u8; INSTRUCTION_BYTES]],
    ) -> vec::Vec<u8> {
        let mut bytes = vec![0_u8; HEADER_BYTES + instructions.len() * INSTRUCTION_BYTES];
        bytes
            .get_mut(..4)
            .expect("fixture magic destination")
            .copy_from_slice(&MAGIC);
        *bytes.get_mut(4).expect("fixture version destination") = VERSION;
        bytes
            .get_mut(6..8)
            .expect("fixture count destination")
            .copy_from_slice(
                &u16::try_from(instructions.len())
                    .expect("fixture instruction count")
                    .to_le_bytes(),
            );
        bytes
            .get_mut(8..10)
            .expect("fixture account count destination")
            .copy_from_slice(&account_count.to_le_bytes());
        bytes
            .get_mut(10..12)
            .expect("fixture scalar count destination")
            .copy_from_slice(&scalar_count.to_le_bytes());
        bytes
            .get_mut(12..14)
            .expect("fixture identity count destination")
            .copy_from_slice(&identity_count.to_le_bytes());
        for (index, value) in instructions.iter().enumerate() {
            let start = HEADER_BYTES + index * INSTRUCTION_BYTES;
            bytes
                .get_mut(start..start + INSTRUCTION_BYTES)
                .expect("fixture instruction destination")
                .copy_from_slice(value);
        }
        bytes
    }

    fn fixture_program() -> vec::Vec<u8> {
        program(
            3,
            4,
            1,
            &[
                instruction(OP_TRANSFER_LAMPORTS, 0, 1, 0, 0),
                instruction(OP_WRITE_SCALAR, 0, 0, 1, 8),
                instruction(OP_WRITE_IDENTITY, 0, 0, 0, 32),
                instruction(OP_REQUIRE_LAMPORTS_EQ, 0, 0, 2, 0),
                instruction(OP_REQUIRE_LAMPORTS_EQ, 1, 0, 3, 0),
            ],
        )
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

        let overlapping = program(
            1,
            1,
            1,
            &[
                instruction(OP_WRITE_SCALAR, 0, 0, 0, 8),
                instruction(OP_WRITE_IDENTITY, 0, 0, 0, 0),
            ],
        );
        assert_eq!(
            ProgramV2::decode(&overlapping),
            Err(Error::OverlappingWrites)
        );
    }

    #[test]
    fn register_and_account_widths_are_runtime_values() {
        let bytes = program(
            300,
            258,
            257,
            &[
                instruction(OP_TRANSFER_LAMPORTS, 299, 298, 257, 0),
                instruction(OP_WRITE_IDENTITY, 297, 0, 256, 0),
            ],
        );
        let decoded = ProgramV2::decode(&bytes).expect("wide effect program");
        assert_eq!(decoded.account_count(), 300);
        assert_eq!(decoded.scalar_count(), 258);
        assert_eq!(decoded.identity_count(), 257);
    }
}
