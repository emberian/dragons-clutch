//! Funding-owned fixed-account lifecycle declarations over canonical V4 effects.
//!
//! This successor does not execute Solana account operations and carries no
//! family discriminator. It hostile-decodes the exact funding actions, their
//! bounded PDA seed sources, and an embedded byte-exact V4 effect program. The
//! outer runtime must join every action to AccountProfile V3 before performing
//! create or close, then preserve the ordinary V4 projection semantics.

use dclutch_capability_seal_contract::{SealedArtifactV1, SealedRoleV1};

use super::v4::{ErrorV4, ProgramV4};

/// Distinct successor magic.
pub const MAGIC_V5: [u8; 4] = *b"DCE6";
/// Successor wire version.
pub const VERSION_V5: u8 = 6;
/// Finalized-record schema label.
pub const SCHEMA_RELEASE_PREIMAGE_V5: &[u8] =
    b"dclutch/schema/effect-program-v6-funding-account-lifecycle-v1";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE_V5`].
pub const SCHEMA_RELEASE_ID_V5: [u8; 32] = [
    0x68, 0xdf, 0x4b, 0x64, 0xc6, 0xa7, 0x1c, 0xc4, 0x40, 0xc5, 0x12, 0xeb, 0xf0, 0x87, 0x24, 0x5c,
    0xb9, 0xb3, 0x18, 0x82, 0x21, 0xf3, 0xee, 0x86, 0x97, 0xc1, 0x45, 0x8b, 0x27, 0x26, 0x06, 0x84,
];
/// Exact successor header width.
pub const HEADER_BYTES_V5: usize = 32;
/// Exact width of one funding action.
pub const FUNDING_ACTION_BYTES_V5: usize = 24;
/// Exact width of one funding seed declaration.
pub const FUNDING_SEED_BYTES_V5: usize = 40;
/// Runtime bound on one program's funding actions.
pub const MAX_FUNDING_ACTIONS_V5: u16 = 16;
/// Runtime bound on all funding PDA seeds.
pub const MAX_FUNDING_SEEDS_V5: u16 = 64;
/// Runtime bound on one funding PDA's seed count.
pub const MAX_ACTION_SEEDS_V5: u8 = 16;

const OPCODE_CREATE: u8 = 0;
const OPCODE_CLOSE: u8 = 1;
const SEED_LITERAL: u8 = 0;
const SEED_COMMON_SCALAR: u8 = 1;
const SEED_COMMON_IDENTITY: u8 = 2;
const SEED_CANONICAL_BUMP: u8 = 3;
const UNUSED_COORDINATE: u16 = u16::MAX;

/// Stable hostile-decode or source-resolution refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorV5 {
    /// Width, magic, version, or reserved bytes differed.
    Wire,
    /// Embedded exact V4 program refused.
    BaseProgram,
    /// Funding actions were unordered, aliased, or malformed.
    ActionTable,
    /// Funding seeds were malformed, unowned, or noncanonical.
    SeedTable,
    /// A selected scalar or identity register was unavailable or too wide.
    RegisterSelection,
    /// Checked integer arithmetic overflowed.
    Arithmetic,
}

impl From<ErrorV4> for ErrorV5 {
    fn from(_: ErrorV4) -> Self {
        Self::BaseProgram
    }
}

/// Result alias for funding-lifecycle successor programs.
pub type ResultV5<T> = core::result::Result<T, ErrorV5>;

/// Funding-owned physical account operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FundingOperationV5 {
    /// Top up, allocate, and assign one vacant System account before children.
    Create = OPCODE_CREATE,
    /// Drain, truncate, and assign one live Trading-owned account after children.
    Close = OPCODE_CLOSE,
}

impl FundingOperationV5 {
    fn decode(value: u8) -> ResultV5<Self> {
        match value {
            OPCODE_CREATE => Ok(Self::Create),
            OPCODE_CLOSE => Ok(Self::Close),
            _ => Err(ErrorV5::ActionTable),
        }
    }
}

/// One funding-owned fixed-account lifecycle declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FundingActionV5 {
    operation: FundingOperationV5,
    seed_count: u8,
    state: u16,
    counterparty: u16,
    refund_destination: u16,
    system_program: u16,
    lamports_scalar: u16,
    refund_owner_identity: u16,
    seed_start: u16,
    live_bytes: u32,
}

impl FundingActionV5 {
    /// Declare one funding-owned creation.
    #[allow(clippy::too_many_arguments)]
    pub const fn create(
        state: u16,
        payer: u16,
        surplus_refund: u16,
        system_program: u16,
        target_lamports_scalar: u16,
        refund_owner_identity: u16,
        live_bytes: u32,
        seed_start: u16,
        seed_count: u8,
    ) -> Self {
        Self {
            operation: FundingOperationV5::Create,
            seed_count,
            state,
            counterparty: payer,
            refund_destination: surplus_refund,
            system_program,
            lamports_scalar: target_lamports_scalar,
            refund_owner_identity,
            seed_start,
            live_bytes,
        }
    }

    /// Declare one funding-owned closure.
    pub const fn close(
        state: u16,
        rent_credit: u16,
        observed_lamports_scalar: u16,
        refund_owner_identity: u16,
    ) -> Self {
        Self {
            operation: FundingOperationV5::Close,
            seed_count: 0,
            state,
            counterparty: rent_credit,
            refund_destination: UNUSED_COORDINATE,
            system_program: UNUSED_COORDINATE,
            lamports_scalar: observed_lamports_scalar,
            refund_owner_identity,
            seed_start: 0,
            live_bytes: 0,
        }
    }

    /// Selected funding operation.
    pub const fn operation(self) -> FundingOperationV5 {
        self.operation
    }

    /// Fixed state coordinate. Coordinate zero is never admitted.
    pub const fn state(self) -> u16 {
        self.state
    }

    /// Create payer or Close RentCredit coordinate.
    pub const fn counterparty(self) -> u16 {
        self.counterparty
    }

    /// Create payer coordinate, absent for Close.
    pub const fn payer(self) -> Option<u16> {
        match self.operation {
            FundingOperationV5::Create => Some(self.counterparty),
            FundingOperationV5::Close => None,
        }
    }

    /// Close RentCredit coordinate, absent for Create.
    pub const fn rent_credit(self) -> Option<u16> {
        match self.operation {
            FundingOperationV5::Create => None,
            FundingOperationV5::Close => Some(self.counterparty),
        }
    }

    /// Create-only surplus-refund wallet coordinate.
    pub const fn refund_destination(self) -> Option<u16> {
        match self.operation {
            FundingOperationV5::Create => Some(self.refund_destination),
            FundingOperationV5::Close => None,
        }
    }

    /// Create-only System Program coordinate.
    pub const fn system_program(self) -> Option<u16> {
        match self.operation {
            FundingOperationV5::Create => Some(self.system_program),
            FundingOperationV5::Close => None,
        }
    }

    /// Target lamports for Create or observed full balance for Close.
    pub const fn lamports_scalar(self) -> u16 {
        self.lamports_scalar
    }

    /// Immutable refund-owner identity source.
    pub const fn refund_owner_identity(self) -> u16 {
        self.refund_owner_identity
    }

    /// Exact live data width for Create; zero for Close.
    pub const fn live_bytes(self) -> u32 {
        self.live_bytes
    }

    /// First seed declaration for Create.
    pub const fn seed_start(self) -> u16 {
        self.seed_start
    }

    /// Exact Create seed count; zero for Close.
    pub const fn seed_count(self) -> u8 {
        self.seed_count
    }

    fn decode(bytes: &[u8], offset: usize) -> ResultV5<Self> {
        if slice(bytes, add(offset, 20)?, 4)?
            .iter()
            .any(|value| *value != 0)
        {
            return Err(ErrorV5::Wire);
        }
        Ok(Self {
            operation: FundingOperationV5::decode(read_u8(bytes, offset)?)?,
            seed_count: read_u8(bytes, add(offset, 1)?)?,
            state: read_u16(bytes, add(offset, 2)?)?,
            counterparty: read_u16(bytes, add(offset, 4)?)?,
            refund_destination: read_u16(bytes, add(offset, 6)?)?,
            system_program: read_u16(bytes, add(offset, 8)?)?,
            lamports_scalar: read_u16(bytes, add(offset, 10)?)?,
            refund_owner_identity: read_u16(bytes, add(offset, 12)?)?,
            seed_start: read_u16(bytes, add(offset, 14)?)?,
            live_bytes: read_u32(bytes, add(offset, 16)?)?,
        })
    }
}

/// One bounded PDA seed source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundingSeedV5 {
    /// Exact nonempty artifact bytes.
    Literal {
        /// Padded bytes; only `len` leading bytes are semantic.
        bytes: [u8; 32],
        /// Exact nonzero literal width.
        len: u8,
    },
    /// Little-endian low bytes of one authenticated common scalar.
    CommonScalar {
        /// Common scalar index.
        index: u16,
        /// Exact width: one, two, four, or eight bytes.
        width: u8,
    },
    /// One authenticated common identity.
    CommonIdentity {
        /// Common identity index.
        index: u16,
    },
    /// Adapter-derived canonical PDA bump, necessarily the final seed.
    CanonicalBump,
}

impl FundingSeedV5 {
    /// Construct one exact literal without allocation.
    pub fn literal(value: &[u8]) -> ResultV5<Self> {
        if value.is_empty() || value.len() > 32 {
            return Err(ErrorV5::SeedTable);
        }
        let mut bytes = [0_u8; 32];
        bytes
            .get_mut(..value.len())
            .ok_or(ErrorV5::SeedTable)?
            .copy_from_slice(value);
        Ok(Self::Literal {
            bytes,
            len: u8::try_from(value.len()).map_err(|_| ErrorV5::Arithmetic)?,
        })
    }

    /// Construct one scalar seed after validating its bounded width.
    pub fn common_scalar(index: u16, width: u8) -> ResultV5<Self> {
        if !matches!(width, 1 | 2 | 4 | 8) {
            return Err(ErrorV5::SeedTable);
        }
        Ok(Self::CommonScalar { index, width })
    }

    fn decode(bytes: &[u8], offset: usize) -> ResultV5<Self> {
        let kind = read_u8(bytes, offset)?;
        let width = read_u8(bytes, add(offset, 1)?)?;
        let source = read_u16(bytes, add(offset, 2)?)?;
        let payload: [u8; 32] = slice(bytes, add(offset, 4)?, 32)?
            .try_into()
            .map_err(|_| ErrorV5::Wire)?;
        if slice(bytes, add(offset, 36)?, 4)?
            .iter()
            .any(|value| *value != 0)
        {
            return Err(ErrorV5::Wire);
        }
        match kind {
            SEED_LITERAL
                if source == 0
                    && width != 0
                    && width <= 32
                    && payload[usize::from(width)..]
                        .iter()
                        .all(|value| *value == 0) =>
            {
                Ok(Self::Literal {
                    bytes: payload,
                    len: width,
                })
            }
            SEED_COMMON_SCALAR
                if matches!(width, 1 | 2 | 4 | 8) && payload.iter().all(|value| *value == 0) =>
            {
                Ok(Self::CommonScalar {
                    index: source,
                    width,
                })
            }
            SEED_COMMON_IDENTITY if width == 32 && payload.iter().all(|value| *value == 0) => {
                Ok(Self::CommonIdentity { index: source })
            }
            SEED_CANONICAL_BUMP
                if width == 1 && source == 0 && payload.iter().all(|value| *value == 0) =>
            {
                Ok(Self::CanonicalBump)
            }
            _ => Err(ErrorV5::SeedTable),
        }
    }

    fn encode(self, output: &mut [u8], offset: usize) -> ResultV5<()> {
        match self {
            Self::Literal { bytes, len } => {
                if len == 0 || len > 32 || bytes[usize::from(len)..].iter().any(|value| *value != 0)
                {
                    return Err(ErrorV5::SeedTable);
                }
                put(output, offset, &[SEED_LITERAL, len])?;
                put(output, add(offset, 4)?, &bytes)?;
            }
            Self::CommonScalar { index, width } => {
                if !matches!(width, 1 | 2 | 4 | 8) {
                    return Err(ErrorV5::SeedTable);
                }
                put(output, offset, &[SEED_COMMON_SCALAR, width])?;
                put(output, add(offset, 2)?, &index.to_le_bytes())?;
            }
            Self::CommonIdentity { index } => {
                put(output, offset, &[SEED_COMMON_IDENTITY, 32])?;
                put(output, add(offset, 2)?, &index.to_le_bytes())?;
            }
            Self::CanonicalBump => {
                put(output, offset, &[SEED_CANONICAL_BUMP, 1])?;
            }
        }
        Ok(())
    }
}

/// One resolved non-bump seed value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedFundingSeedV5 {
    bytes: [u8; 32],
    len: u8,
}

impl ResolvedFundingSeedV5 {
    /// Exact resolved bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

/// Result of resolving one funding seed declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FundingSeedInputV5 {
    /// Exact seed bytes ready for PDA derivation.
    Bytes(ResolvedFundingSeedV5),
    /// The runtime must derive and append the canonical bump.
    CanonicalBump,
}

/// Borrowed exact V4 program plus funding-owned lifecycle declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV5<'a> {
    bytes: &'a [u8],
    base: ProgramV4<'a>,
    action_count: u16,
    seed_count: u16,
    seed_start: usize,
}

impl<'a> ProgramV5<'a> {
    /// Hostile-decode the complete successor and embedded V4 program.
    pub fn decode(bytes: &'a [u8]) -> ResultV5<Self> {
        let value = Self::decode_shape(bytes)?;
        value.validate_tables()?;
        Ok(value)
    }

    /// Decode bytes authenticated by the current Trading capability seal.
    pub fn from_sealed(bytes: &'a [u8], sealed: SealedArtifactV1<'_>) -> ResultV5<Self> {
        sealed
            .require(SealedRoleV1::EffectProgram, bytes)
            .map_err(|_| ErrorV5::Wire)?;
        Self::decode(bytes)
    }

    fn decode_shape(bytes: &'a [u8]) -> ResultV5<Self> {
        if bytes.len() < HEADER_BYTES_V5
            || bytes.get(..4) != Some(MAGIC_V5.as_slice())
            || read_u8(bytes, 4)? != VERSION_V5
            || slice(bytes, 5, 1)?[0] != 0
            || read_u16(bytes, 10)? != 0
            || slice(bytes, 16, 16)?.iter().any(|value| *value != 0)
        {
            return Err(ErrorV5::Wire);
        }
        let action_count = read_u16(bytes, 6)?;
        let seed_count = read_u16(bytes, 8)?;
        if action_count > MAX_FUNDING_ACTIONS_V5
            || seed_count > MAX_FUNDING_SEEDS_V5
            || (action_count == 0 && seed_count != 0)
        {
            return Err(ErrorV5::ActionTable);
        }
        let base_bytes = usize::try_from(read_u32(bytes, 12)?).map_err(|_| ErrorV5::Wire)?;
        let seed_start = HEADER_BYTES_V5
            .checked_add(
                usize::from(action_count)
                    .checked_mul(FUNDING_ACTION_BYTES_V5)
                    .ok_or(ErrorV5::Arithmetic)?,
            )
            .ok_or(ErrorV5::Arithmetic)?;
        let base_start = seed_start
            .checked_add(
                usize::from(seed_count)
                    .checked_mul(FUNDING_SEED_BYTES_V5)
                    .ok_or(ErrorV5::Arithmetic)?,
            )
            .ok_or(ErrorV5::Arithmetic)?;
        let base_end = base_start
            .checked_add(base_bytes)
            .ok_or(ErrorV5::Arithmetic)?;
        if base_bytes == 0 || base_end != bytes.len() {
            return Err(ErrorV5::Wire);
        }
        let base = ProgramV4::decode(slice(bytes, base_start, base_bytes)?)?;
        Ok(Self {
            bytes,
            base,
            action_count,
            seed_count,
            seed_start,
        })
    }

    /// Complete canonical successor bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Embedded byte-exact V4 effect program.
    pub const fn base(self) -> ProgramV4<'a> {
        self.base
    }

    /// Exact funding action count.
    pub const fn funding_action_count(self) -> u16 {
        self.action_count
    }

    /// Exact funding seed count.
    pub const fn funding_seed_count(self) -> u16 {
        self.seed_count
    }

    /// Decode one canonical funding action.
    pub fn funding_action(self, index: u16) -> ResultV5<FundingActionV5> {
        if index >= self.action_count {
            return Err(ErrorV5::ActionTable);
        }
        let offset = HEADER_BYTES_V5
            .checked_add(
                usize::from(index)
                    .checked_mul(FUNDING_ACTION_BYTES_V5)
                    .ok_or(ErrorV5::Arithmetic)?,
            )
            .ok_or(ErrorV5::Arithmetic)?;
        FundingActionV5::decode(self.bytes, offset)
    }

    /// Decode one canonical funding seed.
    pub fn funding_seed(self, index: u16) -> ResultV5<FundingSeedV5> {
        if index >= self.seed_count {
            return Err(ErrorV5::SeedTable);
        }
        let offset = self
            .seed_start
            .checked_add(
                usize::from(index)
                    .checked_mul(FUNDING_SEED_BYTES_V5)
                    .ok_or(ErrorV5::Arithmetic)?,
            )
            .ok_or(ErrorV5::Arithmetic)?;
        FundingSeedV5::decode(self.bytes, offset)
    }

    /// Resolve one action-local seed from exact authenticated common registers.
    pub fn resolve_funding_seed(
        self,
        action_index: u16,
        ordinal: u8,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> ResultV5<FundingSeedInputV5> {
        let action = self.funding_action(action_index)?;
        if action.operation != FundingOperationV5::Create || ordinal >= action.seed_count {
            return Err(ErrorV5::SeedTable);
        }
        if self
            .base
            .base()
            .scalar_count(tail_count)
            .map_err(|_| ErrorV5::RegisterSelection)?
            != scalars.len()
            || self
                .base
                .base()
                .identity_count(tail_count)
                .map_err(|_| ErrorV5::RegisterSelection)?
                != identities.len()
        {
            return Err(ErrorV5::RegisterSelection);
        }
        let seed = self.funding_seed(
            action
                .seed_start
                .checked_add(u16::from(ordinal))
                .ok_or(ErrorV5::Arithmetic)?,
        )?;
        let mut resolved = ResolvedFundingSeedV5 {
            bytes: [0_u8; 32],
            len: 0,
        };
        match seed {
            FundingSeedV5::Literal { bytes, len } => {
                resolved.bytes = bytes;
                resolved.len = len;
            }
            FundingSeedV5::CommonScalar { index, width } => {
                let value = *scalars
                    .get(usize::from(index))
                    .ok_or(ErrorV5::RegisterSelection)?;
                let bytes = value.to_le_bytes();
                let width = usize::from(width);
                if bytes[width..].iter().any(|value| *value != 0) {
                    return Err(ErrorV5::RegisterSelection);
                }
                resolved.bytes[..width].copy_from_slice(&bytes[..width]);
                resolved.len = u8::try_from(width).map_err(|_| ErrorV5::Arithmetic)?;
            }
            FundingSeedV5::CommonIdentity { index } => {
                resolved.bytes = *identities
                    .get(usize::from(index))
                    .ok_or(ErrorV5::RegisterSelection)?;
                resolved.len = 32;
            }
            FundingSeedV5::CanonicalBump => return Ok(FundingSeedInputV5::CanonicalBump),
        }
        Ok(FundingSeedInputV5::Bytes(resolved))
    }

    fn validate_tables(self) -> ResultV5<()> {
        let common_scalars = self.base.base().common_scalar_count();
        let common_identities = self.base.base().common_identity_count();
        let mut prior_state = None;
        let mut seed_cursor = 0_u16;
        let mut index = 0_u16;
        while index < self.action_count {
            let action = self.funding_action(index)?;
            if action.state == 0
                || action.state == UNUSED_COORDINATE
                || action.counterparty == 0
                || action.counterparty == UNUSED_COORDINATE
                || action.state == action.counterparty
                || prior_state.is_some_and(|prior| prior >= action.state)
                || action.lamports_scalar >= common_scalars
                || action.refund_owner_identity >= common_identities
            {
                return Err(ErrorV5::ActionTable);
            }
            match action.operation {
                FundingOperationV5::Create => {
                    if action.seed_count == 0
                        || action.seed_count > MAX_ACTION_SEEDS_V5
                        || action.seed_start != seed_cursor
                        || action.live_bytes == 0
                        || action.refund_destination == UNUSED_COORDINATE
                        || action.system_program == UNUSED_COORDINATE
                        || action.refund_destination == 0
                        || action.system_program == 0
                        || action.state == action.refund_destination
                        || action.state == action.system_program
                        || action.counterparty == action.refund_destination
                        || action.counterparty == action.system_program
                        || action.refund_destination == action.system_program
                    {
                        return Err(ErrorV5::ActionTable);
                    }
                    let end = action
                        .seed_start
                        .checked_add(u16::from(action.seed_count))
                        .ok_or(ErrorV5::Arithmetic)?;
                    if end > self.seed_count {
                        return Err(ErrorV5::SeedTable);
                    }
                    let mut ordinal = 0_u8;
                    while ordinal < action.seed_count {
                        let seed = self.funding_seed(
                            action
                                .seed_start
                                .checked_add(u16::from(ordinal))
                                .ok_or(ErrorV5::Arithmetic)?,
                        )?;
                        if matches!(seed, FundingSeedV5::CanonicalBump)
                            != (ordinal.checked_add(1) == Some(action.seed_count))
                        {
                            return Err(ErrorV5::SeedTable);
                        }
                        match seed {
                            FundingSeedV5::CommonScalar { index, .. }
                                if index >= common_scalars =>
                            {
                                return Err(ErrorV5::SeedTable);
                            }
                            FundingSeedV5::CommonIdentity { index }
                                if index >= common_identities =>
                            {
                                return Err(ErrorV5::SeedTable);
                            }
                            _ => {}
                        }
                        ordinal = ordinal.checked_add(1).ok_or(ErrorV5::Arithmetic)?;
                    }
                    seed_cursor = end;
                }
                FundingOperationV5::Close => {
                    if action.seed_count != 0
                        || action.seed_start != 0
                        || action.refund_destination != UNUSED_COORDINATE
                        || action.system_program != UNUSED_COORDINATE
                        || action.live_bytes != 0
                    {
                        return Err(ErrorV5::ActionTable);
                    }
                }
            }
            prior_state = Some(action.state);
            index = index.checked_add(1).ok_or(ErrorV5::Arithmetic)?;
        }
        if seed_cursor != self.seed_count {
            return Err(ErrorV5::SeedTable);
        }
        Ok(())
    }
}

/// Encode one funding-lifecycle successor atomically around exact V4 bytes.
pub fn encode_program_v5_atomic(
    base_program: &[u8],
    actions: &[FundingActionV5],
    seeds: &[FundingSeedV5],
    scratch: &mut [u8],
    output: &mut [u8],
) -> ResultV5<()> {
    ProgramV4::decode(base_program)?;
    let expected = HEADER_BYTES_V5
        .checked_add(
            actions
                .len()
                .checked_mul(FUNDING_ACTION_BYTES_V5)
                .ok_or(ErrorV5::Arithmetic)?,
        )
        .and_then(|value| {
            seeds
                .len()
                .checked_mul(FUNDING_SEED_BYTES_V5)
                .and_then(|width| value.checked_add(width))
        })
        .and_then(|value| value.checked_add(base_program.len()))
        .ok_or(ErrorV5::Arithmetic)?;
    if scratch.len() != expected || output.len() != expected {
        return Err(ErrorV5::Wire);
    }
    scratch.fill(0);
    put(scratch, 0, &MAGIC_V5)?;
    put(scratch, 4, &[VERSION_V5])?;
    put(
        scratch,
        6,
        &u16::try_from(actions.len())
            .map_err(|_| ErrorV5::Arithmetic)?
            .to_le_bytes(),
    )?;
    put(
        scratch,
        8,
        &u16::try_from(seeds.len())
            .map_err(|_| ErrorV5::Arithmetic)?
            .to_le_bytes(),
    )?;
    put(
        scratch,
        12,
        &u32::try_from(base_program.len())
            .map_err(|_| ErrorV5::Arithmetic)?
            .to_le_bytes(),
    )?;
    for (index, action) in actions.iter().copied().enumerate() {
        let offset = HEADER_BYTES_V5
            .checked_add(
                index
                    .checked_mul(FUNDING_ACTION_BYTES_V5)
                    .ok_or(ErrorV5::Arithmetic)?,
            )
            .ok_or(ErrorV5::Arithmetic)?;
        put(
            scratch,
            offset,
            &[action.operation as u8, action.seed_count],
        )?;
        for (field_offset, value) in [
            (2, action.state),
            (4, action.counterparty),
            (6, action.refund_destination),
            (8, action.system_program),
            (10, action.lamports_scalar),
            (12, action.refund_owner_identity),
            (14, action.seed_start),
        ] {
            put(scratch, add(offset, field_offset)?, &value.to_le_bytes())?;
        }
        put(scratch, add(offset, 16)?, &action.live_bytes.to_le_bytes())?;
    }
    let seed_start = HEADER_BYTES_V5
        .checked_add(
            actions
                .len()
                .checked_mul(FUNDING_ACTION_BYTES_V5)
                .ok_or(ErrorV5::Arithmetic)?,
        )
        .ok_or(ErrorV5::Arithmetic)?;
    for (index, seed) in seeds.iter().copied().enumerate() {
        let offset = seed_start
            .checked_add(
                index
                    .checked_mul(FUNDING_SEED_BYTES_V5)
                    .ok_or(ErrorV5::Arithmetic)?,
            )
            .ok_or(ErrorV5::Arithmetic)?;
        seed.encode(scratch, offset)?;
    }
    let base_start = seed_start
        .checked_add(
            seeds
                .len()
                .checked_mul(FUNDING_SEED_BYTES_V5)
                .ok_or(ErrorV5::Arithmetic)?,
        )
        .ok_or(ErrorV5::Arithmetic)?;
    put(scratch, base_start, base_program)?;
    ProgramV5::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn read_u8(bytes: &[u8], offset: usize) -> ResultV5<u8> {
    bytes.get(offset).copied().ok_or(ErrorV5::Wire)
}

fn read_u16(bytes: &[u8], offset: usize) -> ResultV5<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| ErrorV5::Wire)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> ResultV5<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| ErrorV5::Wire)?,
    ))
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> ResultV5<&[u8]> {
    let end = offset.checked_add(width).ok_or(ErrorV5::Arithmetic)?;
    bytes.get(offset..end).ok_or(ErrorV5::Wire)
}

fn add(left: usize, right: usize) -> ResultV5<usize> {
    left.checked_add(right).ok_or(ErrorV5::Arithmetic)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> ResultV5<()> {
    let end = offset.checked_add(value.len()).ok_or(ErrorV5::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(ErrorV5::Wire)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::{vec, vec::Vec};

    use super::*;
    use crate::{
        v2::FixedRole,
        v3::{
            HEADER_BYTES as HEADER_BYTES_V3, ROUTE_BYTES, RouteKindV3,
            encode::{EffectGeometryV3, RouteInputV3, encode_effect_program_v3_atomic},
        },
        v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, encode_program_v4_atomic},
    };

    fn base_program() -> Vec<u8> {
        let routes = [RouteInputV3 {
            role: FixedRole::Core,
            kind: RouteKindV3::Once,
            enable_common_scalar: None,
            witness_range_common_scalar: None,
            receipt_dependency: None,
            fixed_account_start: 0,
            fixed_account_count: 5,
            item_account_start: 0,
            item_account_count: 0,
            fixed_request: &[],
            item_request: &[],
        }];
        let mut base_scratch = vec![0_u8; HEADER_BYTES_V3 + ROUTE_BYTES];
        let mut base = vec![0_u8; base_scratch.len()];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 5,
                item_account_stride: 0,
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 2,
                item_identity_stride: 0,
            },
            &routes,
            &[],
            &[],
            &mut base_scratch,
            &mut base,
        )
        .expect("V3 base");
        let mut scratch = vec![0_u8; HEADER_BYTES_V4 + base.len()];
        let mut output = vec![0_u8; scratch.len()];
        encode_program_v4_atomic(
            &base,
            BorrowedRangePolicyV4::DisjointExactCoverage,
            1,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("V4 base");
        output
    }

    fn exact_program() -> Vec<u8> {
        let base = base_program();
        let seeds = [
            FundingSeedV5::literal(b"series-ticket").expect("literal"),
            FundingSeedV5::common_scalar(0, 8).expect("scalar"),
            FundingSeedV5::CommonIdentity { index: 1 },
            FundingSeedV5::CanonicalBump,
        ];
        let actions = [
            FundingActionV5::create(1, 2, 3, 4, 0, 0, 64, 0, 4),
            FundingActionV5::close(5, 6, 1, 0),
        ];
        let width = HEADER_BYTES_V5
            + actions.len() * FUNDING_ACTION_BYTES_V5
            + seeds.len() * FUNDING_SEED_BYTES_V5
            + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_program_v5_atomic(&base, &actions, &seeds, &mut scratch, &mut output)
            .expect("V5 program");
        output
    }

    #[test]
    fn exact_create_close_and_seed_sources_round_trip() {
        let bytes = exact_program();
        let program = ProgramV5::decode(&bytes).expect("V5 decode");
        assert_eq!(program.funding_action_count(), 2);
        assert_eq!(program.funding_seed_count(), 4);
        assert_eq!(
            program.funding_action(0).map(FundingActionV5::live_bytes),
            Ok(64)
        );
        assert_eq!(
            program.funding_action(1).map(FundingActionV5::live_bytes),
            Ok(0)
        );
        let scalars = [7_u64, 81];
        let identities = [[0x11; 32], [0x22; 32]];
        assert_eq!(
            program
                .resolve_funding_seed(0, 0, 0, &scalars, &identities)
                .map(|value| match value {
                    FundingSeedInputV5::Bytes(value) => value.as_slice() == b"series-ticket",
                    FundingSeedInputV5::CanonicalBump => false,
                }),
            Ok(true)
        );
        assert_eq!(
            program.resolve_funding_seed(0, 3, 0, &scalars, &identities),
            Ok(FundingSeedInputV5::CanonicalBump)
        );
        assert_eq!(program.base().bytes(), base_program());
    }

    #[test]
    fn schema_substitution_and_noncanonical_tables_refuse() {
        let exact = exact_program();
        for (offset, value, expected) in [
            (0, 0xff, ErrorV5::Wire),
            (5, 1, ErrorV5::Wire),
            (HEADER_BYTES_V5 + 2, 0, ErrorV5::ActionTable),
            (
                HEADER_BYTES_V5 + FUNDING_ACTION_BYTES_V5 + 14,
                1,
                ErrorV5::ActionTable,
            ),
            (
                HEADER_BYTES_V5 + 2 * FUNDING_ACTION_BYTES_V5 + 1,
                0,
                ErrorV5::SeedTable,
            ),
        ] {
            let mut hostile = exact.clone();
            hostile[offset] = value;
            assert_eq!(ProgramV5::decode(&hostile), Err(expected));
        }
    }

    #[test]
    fn encoder_is_atomic_on_alias_gap_and_register_hostiles() {
        let base = base_program();
        let seeds = [FundingSeedV5::CanonicalBump];
        let hostiles = [
            FundingActionV5::create(0, 2, 3, 4, 0, 0, 64, 0, 1),
            FundingActionV5::create(1, 1, 3, 4, 0, 0, 64, 0, 1),
            FundingActionV5::create(1, 2, 3, 4, 2, 0, 64, 0, 1),
            FundingActionV5::create(1, 2, 3, 4, 0, 0, 64, 1, 1),
        ];
        for hostile in hostiles {
            let width =
                HEADER_BYTES_V5 + FUNDING_ACTION_BYTES_V5 + FUNDING_SEED_BYTES_V5 + base.len();
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0x55_u8; width];
            assert!(
                encode_program_v5_atomic(&base, &[hostile], &seeds, &mut scratch, &mut output)
                    .is_err()
            );
            assert!(output.iter().all(|value| *value == 0x55));
        }
    }

    #[test]
    fn scalar_seed_refuses_truncation_and_wrong_bank_width() {
        let bytes = exact_program();
        let program = ProgramV5::decode(&bytes).expect("V5 decode");
        let identities = [[0x11; 32], [0x22; 32]];
        assert_eq!(
            program.resolve_funding_seed(0, 1, 0, &[1], &identities),
            Err(ErrorV5::RegisterSelection)
        );
        let narrow = FundingSeedV5::common_scalar(0, 1).expect("narrow scalar");
        let base = base_program();
        let actions = [FundingActionV5::create(1, 2, 3, 4, 0, 0, 64, 0, 2)];
        let seeds = [narrow, FundingSeedV5::CanonicalBump];
        let width =
            HEADER_BYTES_V5 + FUNDING_ACTION_BYTES_V5 + 2 * FUNDING_SEED_BYTES_V5 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_program_v5_atomic(&base, &actions, &seeds, &mut scratch, &mut output)
            .expect("narrow seed program");
        let program = ProgramV5::decode(&output).expect("decode");
        assert_eq!(
            program.resolve_funding_seed(0, 0, 0, &[256, 0], &identities),
            Err(ErrorV5::RegisterSelection)
        );
    }

    #[test]
    fn exact_empty_successor_round_trips_without_phantom_actions() {
        let base = base_program();
        let width = HEADER_BYTES_V5 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_program_v5_atomic(&base, &[], &[], &mut scratch, &mut output)
            .expect("canonical empty V5 successor");
        let decoded = ProgramV5::decode(&output).expect("empty successor decodes");
        assert_eq!(decoded.funding_action_count(), 0);
        assert_eq!(decoded.funding_seed_count(), 0);
        assert_eq!(decoded.base().bytes(), base);
        assert_eq!(decoded.funding_action(0), Err(ErrorV5::ActionTable));
        assert_eq!(decoded.funding_seed(0), Err(ErrorV5::SeedTable));
    }

    #[test]
    fn close_only_successor_requires_no_phantom_seed() {
        let base = base_program();
        let actions = [FundingActionV5::close(1, 2, 0, 0)];
        let width = HEADER_BYTES_V5 + FUNDING_ACTION_BYTES_V5 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_program_v5_atomic(&base, &actions, &[], &mut scratch, &mut output)
            .expect("close-only V5 successor");
        let decoded = ProgramV5::decode(&output).expect("close-only successor decodes");
        assert_eq!(decoded.funding_action_count(), 1);
        assert_eq!(decoded.funding_seed_count(), 0);
        assert_eq!(
            decoded.funding_action(0).map(FundingActionV5::operation),
            Ok(FundingOperationV5::Close)
        );
    }

    #[test]
    fn noncanonical_empty_and_seedless_create_tables_refuse() {
        let base = base_program();
        let empty_width = HEADER_BYTES_V5 + base.len();
        let mut scratch = vec![0_u8; empty_width];
        let mut exact = vec![0_u8; empty_width];
        encode_program_v5_atomic(&base, &[], &[], &mut scratch, &mut exact)
            .expect("canonical empty V5 successor");

        let mut reserved = exact.clone();
        reserved[5] = 1;
        assert_eq!(ProgramV5::decode(&reserved), Err(ErrorV5::Wire));

        let mut trailing = exact;
        trailing.push(0);
        assert_eq!(ProgramV5::decode(&trailing), Err(ErrorV5::Wire));

        let seedless_create = [FundingActionV5::create(1, 2, 3, 4, 0, 0, 64, 0, 0)];
        let width = HEADER_BYTES_V5 + FUNDING_ACTION_BYTES_V5 + base.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width];
        assert_eq!(
            encode_program_v5_atomic(&base, &seedless_create, &[], &mut scratch, &mut output,),
            Err(ErrorV5::ActionTable)
        );
        assert!(output.iter().all(|value| *value == 0x55));

        let mut orphan_seed = vec![0_u8; HEADER_BYTES_V5 + FUNDING_SEED_BYTES_V5 + base.len()];
        orphan_seed[..HEADER_BYTES_V5].copy_from_slice(&scratch[..HEADER_BYTES_V5]);
        orphan_seed[..4].copy_from_slice(&MAGIC_V5);
        orphan_seed[4] = VERSION_V5;
        orphan_seed[6..8].copy_from_slice(&0_u16.to_le_bytes());
        orphan_seed[8..10].copy_from_slice(&1_u16.to_le_bytes());
        orphan_seed[12..16]
            .copy_from_slice(&u32::try_from(base.len()).expect("base width").to_le_bytes());
        FundingSeedV5::CanonicalBump
            .encode(&mut orphan_seed, HEADER_BYTES_V5)
            .expect("orphan seed shape");
        let base_start = HEADER_BYTES_V5 + FUNDING_SEED_BYTES_V5;
        orphan_seed[base_start..].copy_from_slice(&base);
        assert_eq!(ProgramV5::decode(&orphan_seed), Err(ErrorV5::ActionTable));
    }
}
