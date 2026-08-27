//! Safe, allocation-free `EffectProgram` V2 artifact encoder.
//!
//! Typed constructors retain opcode, auxiliary-field and operand-position
//! authority in the effect kernel. Without them an artifact author has to
//! restate `OPCODE_OFFSET`, `REGISTER_OFFSET` and the eight `OP_*` numbers, and
//! the wire format acquires a second authority that drifts from this one.
//!
//! The encoder builds into caller scratch, hostile-decodes the complete
//! candidate with [`ProgramV2::decode`] -- which is the same walk the adapter
//! performs, including the overlapping-write check across every prior
//! instruction -- and copies to `output` only after it accepts. A refusal
//! leaves `output` byte-for-byte unchanged.

use super::{
    ACCOUNT_A_OFFSET, ACCOUNT_B_OFFSET, ACCOUNT_COUNT_OFFSET, AUXILIARY_OFFSET, DATA_OFFSET,
    EXTRA_OFFSET, Error, FixedRole, HEADER_BYTES, IDENTITY_COUNT_OFFSET, INSTRUCTION_BYTES,
    INSTRUCTION_COUNT_OFFSET, MAGIC, OP_INVOKE_ROLE, OP_INVOKE_ROLE_IF_NONZERO,
    OP_REQUIRE_LAMPORTS_EQ, OP_TRANSFER_LAMPORTS, OP_WRITE_IDENTITY, OP_WRITE_REQUEST_IDENTITY,
    OP_WRITE_REQUEST_SCALAR, OP_WRITE_SCALAR, OPCODE_OFFSET, ProgramV2, REGISTER_OFFSET,
    REQUEST_BYTES_OFFSET, SCALAR_COUNT_OFFSET, VERSION,
};

/// Exact account and register geometry declared by the encoded program.
///
/// `accounts` is the authenticated `AccountProfile` suffix width the adapter
/// supplies; `scalars` and `identities` are the `TransitionVM` V2 output banks
/// the program consumes; `request_bytes` is the caller-owned buffer the
/// request writes and child invocations address. In an activation that last
/// field is the family root tail's exact width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectGeometryV2 {
    /// Exact account-vector width selected by the profile.
    pub accounts: u16,
    /// Exact scalar-bank width consumed from `TransitionVM` V2 output.
    pub scalars: u16,
    /// Exact identity-bank width consumed from `TransitionVM` V2 output.
    pub identities: u16,
    /// Exact caller-owned request-buffer width; zero declares no buffer.
    pub request_bytes: u16,
}

/// One typed V2 effect operation.
///
/// Every constructor names its operands by role and holds the fields the
/// decoder requires to be canonically zero at zero, so an author cannot emit a
/// noncanonical instruction by forgetting a slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectInstructionV2 {
    opcode: u8,
    auxiliary: u8,
    account_a: u16,
    account_b: u16,
    register: u16,
    data_offset: u32,
    extra: u32,
}

impl EffectInstructionV2 {
    const fn new(
        opcode: u8,
        auxiliary: u8,
        account_a: u16,
        account_b: u16,
        register: u16,
        data_offset: u32,
        extra: u32,
    ) -> Self {
        Self {
            opcode,
            auxiliary,
            account_a,
            account_b,
            register,
            data_offset,
            extra,
        }
    }

    /// Move an exact scalar's worth of lamports between two account coordinates.
    #[must_use]
    pub const fn transfer_lamports(source: u16, destination: u16, amount: u16) -> Self {
        Self::new(OP_TRANSFER_LAMPORTS, 0, source, destination, amount, 0, 0)
    }

    /// Require one account's projected lamport balance to equal a scalar.
    #[must_use]
    pub const fn require_lamports_eq(account: u16, value: u16) -> Self {
        Self::new(OP_REQUIRE_LAMPORTS_EQ, 0, account, 0, value, 0, 0)
    }

    /// Write one scalar as little-endian `u64` account data.
    #[must_use]
    pub const fn write_u64(account: u16, offset: u32, value: u16) -> Self {
        Self::new(OP_WRITE_SCALAR, 0, account, 0, value, offset, 0)
    }

    /// Write one identity register as thirty-two bytes of account data.
    #[must_use]
    pub const fn write_identity(account: u16, offset: u32, value: u16) -> Self {
        Self::new(OP_WRITE_IDENTITY, 0, account, 0, value, offset, 0)
    }

    /// Write one scalar as little-endian `u64` into the request buffer.
    #[must_use]
    pub const fn write_request_u64(offset: u32, value: u16) -> Self {
        Self::new(OP_WRITE_REQUEST_SCALAR, 0, 0, 0, value, offset, 0)
    }

    /// Write one identity register as thirty-two request-buffer bytes.
    #[must_use]
    pub const fn write_request_identity(offset: u32, value: u16) -> Self {
        Self::new(OP_WRITE_REQUEST_IDENTITY, 0, 0, 0, value, offset, 0)
    }

    /// Invoke one fixed role over an account subframe and a request slice.
    ///
    /// `account_count` and `request_len` must both be nonzero; the decoder
    /// refuses a zero-width child frame or an empty child request.
    #[must_use]
    pub const fn invoke_role(role: FixedRole, frame: RoleFrameV2) -> Self {
        Self::new(
            OP_INVOKE_ROLE,
            frame.account_count,
            encode_role(role),
            frame.account_start,
            0,
            frame.request_offset,
            frame.request_len,
        )
    }

    /// Invoke one fixed role only when an enabling scalar register is nonzero.
    #[must_use]
    pub const fn invoke_role_if_nonzero(
        role: FixedRole,
        frame: RoleFrameV2,
        enable_scalar: u16,
    ) -> Self {
        Self::new(
            OP_INVOKE_ROLE_IF_NONZERO,
            frame.account_count,
            encode_role(role),
            frame.account_start,
            enable_scalar,
            frame.request_offset,
            frame.request_len,
        )
    }
}

/// The account subframe and request slice one child invocation carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleFrameV2 {
    /// First authenticated account coordinate supplied to the child.
    pub account_start: u16,
    /// Exact child account count; the wire field is one byte wide.
    pub account_count: u8,
    /// Byte offset of this child's request within the complete buffer.
    pub request_offset: u32,
    /// Exact request byte length.
    pub request_len: u32,
}

const fn encode_role(role: FixedRole) -> u16 {
    match role {
        FixedRole::Core => 0,
        FixedRole::Claims => 1,
        FixedRole::Resolution => 3,
        FixedRole::Custody => 4,
    }
}

/// Exact encoded width of a V2 effect program with `instructions` instructions.
///
/// # Errors
///
/// Refuses a width that does not fit `usize`.
pub const fn effect_program_v2_bytes(instructions: usize) -> Result<usize, Error> {
    match instructions.checked_mul(INSTRUCTION_BYTES) {
        Some(body) => match HEADER_BYTES.checked_add(body) {
            Some(total) => Ok(total),
            None => Err(Error::InvalidLength),
        },
        None => Err(Error::InvalidLength),
    }
}

/// Encode one complete `EffectProgram` V2 into caller-owned buffers atomically.
///
/// `scratch` and `output` must both be exactly [`effect_program_v2_bytes`]
/// wide. The candidate is assembled in `scratch`, hostile-decoded in full, and
/// copied to `output` only on success: on any refusal `output` is unchanged.
///
/// # Errors
///
/// Refuses buffer widths that differ from the exact encoded width, and every
/// refusal [`ProgramV2::decode`] itself raises against the candidate --
/// unknown opcodes, absent or aliased account coordinates, out-of-bank
/// registers, request ranges outside the declared buffer, and writes that
/// overlap an earlier write to the same account or request range.
pub fn encode_effect_program_v2_atomic(
    geometry: EffectGeometryV2,
    instructions: &[EffectInstructionV2],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    let instruction_count = u16::try_from(instructions.len()).map_err(|_| Error::InvalidLength)?;
    let expected = effect_program_v2_bytes(instructions.len())?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(scratch, 0, &MAGIC)?;
    write_byte(scratch, 4, VERSION)?;
    for (offset, value) in [
        (INSTRUCTION_COUNT_OFFSET, instruction_count),
        (ACCOUNT_COUNT_OFFSET, geometry.accounts),
        (SCALAR_COUNT_OFFSET, geometry.scalars),
        (IDENTITY_COUNT_OFFSET, geometry.identities),
        (REQUEST_BYTES_OFFSET, geometry.request_bytes),
    ] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    let mut cursor = HEADER_BYTES;
    for instruction in instructions {
        encode_instruction(*instruction, scratch, cursor)?;
        cursor = add(cursor, INSTRUCTION_BYTES)?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    ProgramV2::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn encode_instruction(
    instruction: EffectInstructionV2,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    write_byte(output, add(offset, OPCODE_OFFSET)?, instruction.opcode)?;
    write_byte(
        output,
        add(offset, AUXILIARY_OFFSET)?,
        instruction.auxiliary,
    )?;
    for (local, value) in [
        (ACCOUNT_A_OFFSET, instruction.account_a),
        (ACCOUNT_B_OFFSET, instruction.account_b),
        (REGISTER_OFFSET, instruction.register),
    ] {
        write(output, add(offset, local)?, &value.to_le_bytes())?;
    }
    write(
        output,
        add(offset, DATA_OFFSET)?,
        &instruction.data_offset.to_le_bytes(),
    )?;
    write(
        output,
        add(offset, EXTRA_OFFSET)?,
        &instruction.extra.to_le_bytes(),
    )
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    let end = add(offset, bytes.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}
