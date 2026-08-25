//! Exact hostile-decodable wires for the covered Dealer lifecycle.
//!
//! Account identities, balances, immutable ladder bytes, funding amounts, and
//! Clock observations are never instruction data. They are authenticated from
//! the exact physical frame. The only wire facts are compact child IDs,
//! generation/replay guards, requested quantities, and user-protective limits.

use core::convert::TryInto;

use crate::{
    AddLiquidityRequest, Error as DealerError, LiquidityAmounts, RemoveLiquidityRequest,
    TradeRequest, TradeSide,
};

/// Shared hostile instruction header width.
pub const DEALER_INSTRUCTION_HEADER_BYTES: usize = 16;
/// Canonical Dealer instruction-family magic.
pub const DEALER_INSTRUCTION_MAGIC: [u8; 8] = *b"DCLTDIX1";
/// Implemented instruction schema.
pub const DEALER_INSTRUCTION_SCHEMA_VERSION: u16 = 1;
/// Exact Activate/Open wire width.
pub const ACTIVATE_POOL_BYTES: usize = 80;
/// Exact LP-position creation wire width.
pub const CREATE_LP_POSITION_BYTES: usize = 56;
/// Exact trade wire width.
pub const TRADE_BYTES: usize = 56;
/// Exact slot-gated reset wire width.
pub const RESET_LADDER_BYTES: usize = 24;
/// Exact LP-position close wire width.
pub const CLOSE_LP_POSITION_BYTES: usize = 32;
/// Exact Pool retirement wire width.
pub const RETIRE_POOL_BYTES: usize = 32;

const SCHEMA_OFFSET: usize = 8;
const ACTION_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 11;
const HEADER_RESERVED_OFFSET: usize = 12;
const HEADER_RESERVED_BYTES: usize = 4;

const OPEN_GENERATION_OFFSET: usize = 16;
const OPEN_CHILD_COUNT_OFFSET: usize = 24;
const OPEN_LP_ID_OFFSET: usize = 32;
const OPEN_CLAIM_QUANTITY_OFFSET: usize = 64;
const OPEN_INITIAL_SHARES_OFFSET: usize = 72;

const POSITION_POOL_SEQUENCE_OFFSET: usize = 16;
const POSITION_LP_ID_OFFSET: usize = 24;

const CHANGE_POOL_SEQUENCE_OFFSET: usize = 16;
const CHANGE_POSITION_SEQUENCE_OFFSET: usize = 24;
const CHANGE_SHARES_OFFSET: usize = 32;
const CHANGE_PRINCIPAL_LIMIT_OFFSET: usize = 40;
const CHANGE_FEE_LIMIT_OFFSET: usize = 48;
const CHANGE_CLAIMS_LIMIT_OFFSET: usize = 56;

const TRADE_RESET_OFFSET: usize = 16;
const TRADE_SEQUENCE_OFFSET: usize = 24;
const TRADE_SIDE_OFFSET: usize = 32;
const TRADE_CLAIM_OFFSET: usize = 33;
const TRADE_RESERVED_OFFSET: usize = 34;
const TRADE_RESERVED_BYTES: usize = 6;
const TRADE_QUANTITY_OFFSET: usize = 40;
const TRADE_LIMIT_OFFSET: usize = 48;

const RESET_SEQUENCE_OFFSET: usize = 16;

const CLOSE_POOL_SEQUENCE_OFFSET: usize = 16;
const CLOSE_POSITION_SEQUENCE_OFFSET: usize = 24;

const RETIRE_POOL_SEQUENCE_OFFSET: usize = 16;
const RETIRE_CHILD_COUNT_OFFSET: usize = 24;

/// Refusal from the exact Dealer wire parser or constructor projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionError {
    /// Input did not have the exact selected-profile width.
    InvalidLength,
    /// The magic did not select this instruction family.
    InvalidMagic,
    /// The schema is not implemented.
    UnsupportedSchema,
    /// The action discriminator is unknown.
    UnknownAction,
    /// Flags or reserved bytes were not canonical zeroes.
    NonCanonicalReservedBytes,
    /// A compact LP identity was the zero sentinel.
    ZeroCompactId,
    /// A selected claim index cannot be represented by the exact-N profile.
    ClaimIndexOutOfRange,
    /// Checked profile-width arithmetic overflowed.
    ArithmeticOverflow,
    /// The underlying liquidity request refused a quantity or limit vector.
    Dealer(DealerError),
}

/// Result alias for Dealer instruction processing.
pub type Result<T> = core::result::Result<T, InstructionError>;

/// Canonical minimal Dealer lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DealerActionV1 {
    /// Activate selected capability funding and open Pool custody.
    ActivatePool = 1,
    /// Create one reusable zero-share LP position.
    CreateLpPosition = 2,
    /// Mint shares for a bounded proportional deposit.
    AddLiquidity = 3,
    /// Burn shares for a bounded proportional withdrawal.
    RemoveLiquidity = 4,
    /// Execute one immediate covered trade.
    Trade = 5,
    /// Reopen the identical ladder after its Clock boundary.
    ResetLadder = 6,
    /// Close one empty LP position to its exact RentCredit.
    CloseLpPosition = 7,
    /// Retire a quiescent Pool and immutable config.
    RetirePool = 8,
}

impl DealerActionV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::ActivatePool),
            2 => Ok(Self::CreateLpPosition),
            3 => Ok(Self::AddLiquidity),
            4 => Ok(Self::RemoveLiquidity),
            5 => Ok(Self::Trade),
            6 => Ok(Self::ResetLadder),
            7 => Ok(Self::CloseLpPosition),
            8 => Ok(Self::RetirePool),
            _ => Err(InstructionError::UnknownAction),
        }
    }
}

/// Activate/Open replay and quantity facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatePoolV1 {
    generation: u64,
    expected_market_child_count: u64,
    initial_lp_id: [u8; 32],
    initial_claim_quantity: u64,
    initial_shares: u64,
}

impl ActivatePoolV1 {
    /// Construct canonical Activate/Open facts.
    pub fn new(
        generation: u64,
        expected_market_child_count: u64,
        initial_lp_id: [u8; 32],
        initial_claim_quantity: u64,
        initial_shares: u64,
    ) -> Result<Self> {
        require_id(initial_lp_id)?;
        if initial_claim_quantity == 0 || initial_shares == 0 {
            return Err(InstructionError::Dealer(DealerError::InvalidQuantity));
        }
        Ok(Self {
            generation,
            expected_market_child_count,
            initial_lp_id,
            initial_claim_quantity,
            initial_shares,
        })
    }

    /// Return immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Return exact Market direct-child replay guard.
    pub const fn expected_market_child_count(self) -> u64 {
        self.expected_market_child_count
    }
    /// Return compact initial LP-position derivation identity.
    pub const fn initial_lp_id(self) -> [u8; 32] {
        self.initial_lp_id
    }
    /// Return exact complete-set claim quantity transferred from Position.
    pub const fn initial_claim_quantity(self) -> u64 {
        self.initial_claim_quantity
    }
    /// Return exact initial LP shares.
    pub const fn initial_shares(self) -> u64 {
        self.initial_shares
    }
}

/// LP-position creation replay and compact identity facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateLpPositionV1 {
    expected_pool_sequence: u64,
    lp_id: [u8; 32],
}

impl CreateLpPositionV1 {
    /// Construct a canonical position-creation request.
    pub fn new(expected_pool_sequence: u64, lp_id: [u8; 32]) -> Result<Self> {
        require_id(lp_id)?;
        Ok(Self {
            expected_pool_sequence,
            lp_id,
        })
    }
    /// Return Pool replay guard.
    pub const fn expected_pool_sequence(self) -> u64 {
        self.expected_pool_sequence
    }
    /// Return compact LP-position derivation identity.
    pub const fn lp_id(self) -> [u8; 32] {
        self.lp_id
    }
}

/// Add-liquidity request for the LP account named by the physical frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddLiquidityV1<const N: usize> {
    request: AddLiquidityRequest<N>,
}

impl<const N: usize> AddLiquidityV1<N> {
    /// Construct from replay, share, and maximum-deposit facts.
    pub fn new(
        expected_pool_sequence: u64,
        expected_position_sequence: u64,
        shares_to_mint: u64,
        maximum_deposit: LiquidityAmounts<N>,
    ) -> Result<Self> {
        Ok(Self {
            request: AddLiquidityRequest::new(
                expected_pool_sequence,
                expected_position_sequence,
                shares_to_mint,
                maximum_deposit,
            )
            .map_err(InstructionError::Dealer)?,
        })
    }
    /// Return the bounded kernel request.
    pub const fn request(self) -> AddLiquidityRequest<N> {
        self.request
    }
}

/// Remove-liquidity request for the LP account named by the physical frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoveLiquidityV1<const N: usize> {
    request: RemoveLiquidityRequest<N>,
}

impl<const N: usize> RemoveLiquidityV1<N> {
    /// Construct from replay, share, and minimum-withdrawal facts.
    pub fn new(
        expected_pool_sequence: u64,
        expected_position_sequence: u64,
        shares_to_burn: u64,
        minimum_withdrawal: LiquidityAmounts<N>,
    ) -> Result<Self> {
        Ok(Self {
            request: RemoveLiquidityRequest::new(
                expected_pool_sequence,
                expected_position_sequence,
                shares_to_burn,
                minimum_withdrawal,
            )
            .map_err(InstructionError::Dealer)?,
        })
    }
    /// Return the bounded kernel request.
    pub const fn request(self) -> RemoveLiquidityRequest<N> {
        self.request
    }
}

/// Empty-position closure replay facts for the LP account named by the frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseLpPositionV1 {
    expected_pool_sequence: u64,
    expected_position_sequence: u64,
}

impl CloseLpPositionV1 {
    /// Construct a canonical empty-position closure request.
    pub fn new(expected_pool_sequence: u64, expected_position_sequence: u64) -> Result<Self> {
        Ok(Self {
            expected_pool_sequence,
            expected_position_sequence,
        })
    }
    /// Return Pool replay guard.
    pub const fn expected_pool_sequence(self) -> u64 {
        self.expected_pool_sequence
    }
    /// Return position-local replay guard.
    pub const fn expected_position_sequence(self) -> u64 {
        self.expected_position_sequence
    }
}

/// Decoded exact Dealer instruction for one exact-N profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerInstructionV1<const N: usize> {
    /// Activate funding and open the Pool.
    ActivatePool(ActivatePoolV1),
    /// Create a zero-share LP position.
    CreateLpPosition(CreateLpPositionV1),
    /// Add bounded proportional liquidity.
    AddLiquidity(AddLiquidityV1<N>),
    /// Remove bounded proportional liquidity.
    RemoveLiquidity(RemoveLiquidityV1<N>),
    /// Execute one covered trade.
    Trade(TradeRequest),
    /// Reset the identical immutable ladder at a trusted Clock slot.
    ResetLadder {
        /// Exact Pool replay sequence selected for the reset.
        expected_pool_sequence: u64,
    },
    /// Close one empty LP position.
    CloseLpPosition(CloseLpPositionV1),
    /// Retire Pool using a Market child-count replay guard.
    RetirePool {
        /// Exact Pool replay sequence selected for retirement.
        expected_pool_sequence: u64,
        /// Exact Market direct-child count before retirement.
        expected_market_child_count: u64,
    },
}

impl<const N: usize> DealerInstructionV1<N> {
    /// Hostile-decode one exact selected-profile Dealer instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let action = decode_header(bytes)?;
        require_exact_length::<N>(action, bytes.len())?;
        match action {
            DealerActionV1::ActivatePool => Ok(Self::ActivatePool(ActivatePoolV1::new(
                read_u64(bytes, OPEN_GENERATION_OFFSET)?,
                read_u64(bytes, OPEN_CHILD_COUNT_OFFSET)?,
                read_array(bytes, OPEN_LP_ID_OFFSET)?,
                read_u64(bytes, OPEN_CLAIM_QUANTITY_OFFSET)?,
                read_u64(bytes, OPEN_INITIAL_SHARES_OFFSET)?,
            )?)),
            DealerActionV1::CreateLpPosition => {
                Ok(Self::CreateLpPosition(CreateLpPositionV1::new(
                    read_u64(bytes, POSITION_POOL_SEQUENCE_OFFSET)?,
                    read_array(bytes, POSITION_LP_ID_OFFSET)?,
                )?))
            }
            DealerActionV1::AddLiquidity => Ok(Self::AddLiquidity(AddLiquidityV1::new(
                read_u64(bytes, CHANGE_POOL_SEQUENCE_OFFSET)?,
                read_u64(bytes, CHANGE_POSITION_SEQUENCE_OFFSET)?,
                read_u64(bytes, CHANGE_SHARES_OFFSET)?,
                read_liquidity(bytes)?,
            )?)),
            DealerActionV1::RemoveLiquidity => Ok(Self::RemoveLiquidity(RemoveLiquidityV1::new(
                read_u64(bytes, CHANGE_POOL_SEQUENCE_OFFSET)?,
                read_u64(bytes, CHANGE_POSITION_SEQUENCE_OFFSET)?,
                read_u64(bytes, CHANGE_SHARES_OFFSET)?,
                read_liquidity(bytes)?,
            )?)),
            DealerActionV1::Trade => {
                require_zero(bytes, TRADE_RESERVED_OFFSET, TRADE_RESERVED_BYTES)?;
                let side = match read_byte(bytes, TRADE_SIDE_OFFSET)? {
                    0 => TradeSide::BuyClaimFromPool,
                    1 => TradeSide::SellClaimToPool,
                    _ => return Err(InstructionError::UnknownAction),
                };
                let claim_index = usize::from(read_byte(bytes, TRADE_CLAIM_OFFSET)?);
                if claim_index >= N {
                    return Err(InstructionError::ClaimIndexOutOfRange);
                }
                Ok(Self::Trade(
                    TradeRequest::new(
                        read_u64(bytes, TRADE_RESET_OFFSET)?,
                        read_u64(bytes, TRADE_SEQUENCE_OFFSET)?,
                        side,
                        claim_index,
                        read_u64(bytes, TRADE_QUANTITY_OFFSET)?,
                        read_u64(bytes, TRADE_LIMIT_OFFSET)?,
                    )
                    .map_err(InstructionError::Dealer)?,
                ))
            }
            DealerActionV1::ResetLadder => Ok(Self::ResetLadder {
                expected_pool_sequence: read_u64(bytes, RESET_SEQUENCE_OFFSET)?,
            }),
            DealerActionV1::CloseLpPosition => Ok(Self::CloseLpPosition(CloseLpPositionV1::new(
                read_u64(bytes, CLOSE_POOL_SEQUENCE_OFFSET)?,
                read_u64(bytes, CLOSE_POSITION_SEQUENCE_OFFSET)?,
            )?)),
            DealerActionV1::RetirePool => Ok(Self::RetirePool {
                expected_pool_sequence: read_u64(bytes, RETIRE_POOL_SEQUENCE_OFFSET)?,
                expected_market_child_count: read_u64(bytes, RETIRE_CHILD_COUNT_OFFSET)?,
            }),
        }
    }

    /// Return the canonical action discriminator.
    pub const fn action(self) -> DealerActionV1 {
        match self {
            Self::ActivatePool(_) => DealerActionV1::ActivatePool,
            Self::CreateLpPosition(_) => DealerActionV1::CreateLpPosition,
            Self::AddLiquidity(_) => DealerActionV1::AddLiquidity,
            Self::RemoveLiquidity(_) => DealerActionV1::RemoveLiquidity,
            Self::Trade(_) => DealerActionV1::Trade,
            Self::ResetLadder { .. } => DealerActionV1::ResetLadder,
            Self::CloseLpPosition(_) => DealerActionV1::CloseLpPosition,
            Self::RetirePool { .. } => DealerActionV1::RetirePool,
        }
    }

    /// Return exact selected-profile byte width.
    pub fn encoded_len(self) -> Result<usize> {
        instruction_len::<N>(self.action())
    }

    /// Encode into an exact selected-profile destination.
    pub fn encode_into(self, out: &mut [u8]) -> Result<()> {
        let action = self.action();
        require_exact_length::<N>(action, out.len())?;
        out.fill(0);
        put(out, 0, &DEALER_INSTRUCTION_MAGIC)?;
        put_u16(out, SCHEMA_OFFSET, DEALER_INSTRUCTION_SCHEMA_VERSION)?;
        put_byte(out, ACTION_OFFSET, action as u8)?;
        match self {
            Self::ActivatePool(value) => {
                put_u64(out, OPEN_GENERATION_OFFSET, value.generation)?;
                put_u64(
                    out,
                    OPEN_CHILD_COUNT_OFFSET,
                    value.expected_market_child_count,
                )?;
                put(out, OPEN_LP_ID_OFFSET, &value.initial_lp_id)?;
                put_u64(
                    out,
                    OPEN_CLAIM_QUANTITY_OFFSET,
                    value.initial_claim_quantity,
                )?;
                put_u64(out, OPEN_INITIAL_SHARES_OFFSET, value.initial_shares)?;
            }
            Self::CreateLpPosition(value) => {
                put_u64(
                    out,
                    POSITION_POOL_SEQUENCE_OFFSET,
                    value.expected_pool_sequence,
                )?;
                put(out, POSITION_LP_ID_OFFSET, &value.lp_id)?;
            }
            Self::AddLiquidity(value) => {
                write_change(out, value.request)?;
            }
            Self::RemoveLiquidity(value) => {
                write_remove(out, value.request)?;
            }
            Self::Trade(value) => {
                put_u64(out, TRADE_RESET_OFFSET, value.reset_number())?;
                put_u64(out, TRADE_SEQUENCE_OFFSET, value.expected_sequence())?;
                let side = match value.side() {
                    TradeSide::BuyClaimFromPool => 0,
                    TradeSide::SellClaimToPool => 1,
                };
                put_byte(out, TRADE_SIDE_OFFSET, side)?;
                let claim = u8::try_from(value.claim_index())
                    .map_err(|_| InstructionError::ClaimIndexOutOfRange)?;
                put_byte(out, TRADE_CLAIM_OFFSET, claim)?;
                put_u64(out, TRADE_QUANTITY_OFFSET, value.quantity())?;
                put_u64(out, TRADE_LIMIT_OFFSET, value.collateral_limit())?;
            }
            Self::ResetLadder {
                expected_pool_sequence,
            } => put_u64(out, RESET_SEQUENCE_OFFSET, expected_pool_sequence)?,
            Self::CloseLpPosition(value) => {
                put_u64(
                    out,
                    CLOSE_POOL_SEQUENCE_OFFSET,
                    value.expected_pool_sequence,
                )?;
                put_u64(
                    out,
                    CLOSE_POSITION_SEQUENCE_OFFSET,
                    value.expected_position_sequence,
                )?;
            }
            Self::RetirePool {
                expected_pool_sequence,
                expected_market_child_count,
            } => {
                put_u64(out, RETIRE_POOL_SEQUENCE_OFFSET, expected_pool_sequence)?;
                put_u64(out, RETIRE_CHILD_COUNT_OFFSET, expected_market_child_count)?;
            }
        }
        Ok(())
    }
}

/// Return exact selected-profile instruction width for an action.
pub fn instruction_len<const N: usize>(action: DealerActionV1) -> Result<usize> {
    match action {
        DealerActionV1::ActivatePool => Ok(ACTIVATE_POOL_BYTES),
        DealerActionV1::CreateLpPosition => Ok(CREATE_LP_POSITION_BYTES),
        DealerActionV1::AddLiquidity | DealerActionV1::RemoveLiquidity => N
            .checked_mul(8)
            .and_then(|value| CHANGE_CLAIMS_LIMIT_OFFSET.checked_add(value))
            .ok_or(InstructionError::ArithmeticOverflow),
        DealerActionV1::Trade => Ok(TRADE_BYTES),
        DealerActionV1::ResetLadder => Ok(RESET_LADDER_BYTES),
        DealerActionV1::CloseLpPosition => Ok(CLOSE_LP_POSITION_BYTES),
        DealerActionV1::RetirePool => Ok(RETIRE_POOL_BYTES),
    }
}

fn decode_header(bytes: &[u8]) -> Result<DealerActionV1> {
    if bytes.len() < DEALER_INSTRUCTION_HEADER_BYTES {
        return Err(InstructionError::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != DEALER_INSTRUCTION_MAGIC {
        return Err(InstructionError::InvalidMagic);
    }
    if read_u16(bytes, SCHEMA_OFFSET)? != DEALER_INSTRUCTION_SCHEMA_VERSION {
        return Err(InstructionError::UnsupportedSchema);
    }
    if read_byte(bytes, FLAGS_OFFSET)? != 0 {
        return Err(InstructionError::NonCanonicalReservedBytes);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
    DealerActionV1::decode(read_byte(bytes, ACTION_OFFSET)?)
}

fn require_exact_length<const N: usize>(action: DealerActionV1, actual: usize) -> Result<()> {
    if instruction_len::<N>(action)? == actual {
        Ok(())
    } else {
        Err(InstructionError::InvalidLength)
    }
}

fn read_liquidity<const N: usize>(bytes: &[u8]) -> Result<LiquidityAmounts<N>> {
    let mut claims = [0u64; N];
    for (index, claim) in claims.iter_mut().enumerate() {
        let offset = index
            .checked_mul(8)
            .and_then(|value| CHANGE_CLAIMS_LIMIT_OFFSET.checked_add(value))
            .ok_or(InstructionError::ArithmeticOverflow)?;
        *claim = read_u64(bytes, offset)?;
    }
    LiquidityAmounts::new(
        read_u64(bytes, CHANGE_PRINCIPAL_LIMIT_OFFSET)?,
        read_u64(bytes, CHANGE_FEE_LIMIT_OFFSET)?,
        claims,
    )
    .map_err(InstructionError::Dealer)
}

fn write_change<const N: usize>(out: &mut [u8], request: AddLiquidityRequest<N>) -> Result<()> {
    // This module owns the wire and therefore projects through a lossless
    // round-trip helper rather than exposing caller-authored balance facts.
    let probe = change_fields_from_add(request);
    write_change_fields(out, probe)
}

fn write_remove<const N: usize>(out: &mut [u8], request: RemoveLiquidityRequest<N>) -> Result<()> {
    let probe = change_fields_from_remove(request);
    write_change_fields(out, probe)
}

#[derive(Clone, Copy)]
struct ChangeFields<const N: usize> {
    pool_sequence: u64,
    position_sequence: u64,
    shares: u64,
    limits: LiquidityAmounts<N>,
}

// Private fields live in the parent module, so constructors deliberately need
// public projections. These helpers use encoded round trips supplied below by
// crate-visible accessors on the request types.
fn change_fields_from_add<const N: usize>(request: AddLiquidityRequest<N>) -> ChangeFields<N> {
    ChangeFields {
        pool_sequence: request.expected_pool_sequence(),
        position_sequence: request.expected_position_sequence(),
        shares: request.shares_to_mint(),
        limits: request.maximum_deposit(),
    }
}

fn change_fields_from_remove<const N: usize>(
    request: RemoveLiquidityRequest<N>,
) -> ChangeFields<N> {
    ChangeFields {
        pool_sequence: request.expected_pool_sequence(),
        position_sequence: request.expected_position_sequence(),
        shares: request.shares_to_burn(),
        limits: request.minimum_withdrawal(),
    }
}

fn write_change_fields<const N: usize>(out: &mut [u8], fields: ChangeFields<N>) -> Result<()> {
    put_u64(out, CHANGE_POOL_SEQUENCE_OFFSET, fields.pool_sequence)?;
    put_u64(
        out,
        CHANGE_POSITION_SEQUENCE_OFFSET,
        fields.position_sequence,
    )?;
    put_u64(out, CHANGE_SHARES_OFFSET, fields.shares)?;
    put_u64(
        out,
        CHANGE_PRINCIPAL_LIMIT_OFFSET,
        fields.limits.principal_collateral(),
    )?;
    put_u64(
        out,
        CHANGE_FEE_LIMIT_OFFSET,
        fields.limits.realized_fee_collateral(),
    )?;
    for (index, claim) in fields.limits.claim_reserves().iter().enumerate() {
        let offset = index
            .checked_mul(8)
            .and_then(|value| CHANGE_CLAIMS_LIMIT_OFFSET.checked_add(value))
            .ok_or(InstructionError::ArithmeticOverflow)?;
        put_u64(out, offset, *claim)?;
    }
    Ok(())
}

fn require_id(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(InstructionError::ZeroCompactId)
    } else {
        Ok(())
    }
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset
        .checked_add(width)
        .ok_or(InstructionError::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(InstructionError::InvalidLength)?
        .iter()
        .any(|value| *value != 0)
    {
        Err(InstructionError::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(InstructionError::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const W: usize>(bytes: &[u8], offset: usize) -> Result<[u8; W]> {
    let end = offset
        .checked_add(W)
        .ok_or(InstructionError::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(InstructionError::InvalidLength)?
        .try_into()
        .map_err(|_| InstructionError::InvalidLength)
}

fn put(out: &mut [u8], offset: usize, bytes: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(bytes.len())
        .ok_or(InstructionError::InvalidLength)?;
    out.get_mut(offset..end)
        .ok_or(InstructionError::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn put_byte(out: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *out.get_mut(offset).ok_or(InstructionError::InvalidLength)? = value;
    Ok(())
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) -> Result<()> {
    put(out, offset, &value.to_le_bytes())
}

fn put_u64(out: &mut [u8], offset: usize, value: u64) -> Result<()> {
    put(out, offset, &value.to_le_bytes())
}
