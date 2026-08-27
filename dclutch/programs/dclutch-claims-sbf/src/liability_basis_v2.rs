//! The shared LiabilityBasisV2 state vocabulary.
//!
//! This module is now exactly what outlived the `DCLLBX02` route: the re-export
//! surface eight sibling modules and roughly twenty crates reach the LBV2
//! aggregate/Position vocabulary through (`MarketViewV2`, `PositionViewV2`,
//! `LIABILITY_BASIS_MARKET_SEED_V2`, the two header widths, the two input
//! types), the terminal-coordinate schema identity, and the four encode/read
//! helpers that speak it. It owns no route and dispatches nothing.
//!
//! # What was here, and why it is not
//!
//! `DCLLBX02` was the last Claims path expecting a Core-owned
//! `LinkedBasisRecordV2` (`DCLTLNK2`). The four live basis consumers --
//! `founding_v5`, `affine_batch_v2`, `signed_delta_v3`, `protocol_position_v2`
//! -- converged onto `authenticate_product_basis_v3`, the Registry-owned
//! `ProductBasisV3` record Core authenticates when it commits a founding
//! permit. This route never did, and it was dead on BOTH ends: nothing in the
//! tree built a `DCLLBX02` instruction (no operator, no Trading composer, no
//! frontend, no bootstrap -- only its own ProgramTest), and nothing on chain
//! finalized a `DCLTLNK2` record either, so it could not have been driven even
//! if an instruction had existed. Its one issuance path, Split, composed an
//! External source compartment that Custody refuses by design.
//!
//! Its own deletion note queued it behind "whoever retires the V2
//! liability-basis kernel". That kernel has an active lane and is not being
//! retired, so the queue was an event that would never arrive. Deleted on its
//! own merits instead.

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryFrom;

use dclutch_claims_svm::liability_basis_state_v2::{
    encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
    liability_basis_vector_width_v2, read_claim_v2,
};
use solana_program::program_error::ProgramError;

pub use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
    LiabilityBasisPositionInputV2,
};
pub(crate) use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2 as MarketViewV2, LiabilityBasisPositionViewV2 as PositionViewV2,
};
pub use dclutch_claims_svm::product_basis_terminal_v3::{
    TERMINAL_COORDINATE_BYTES_V2, TERMINAL_COORDINATE_MAGIC_V2,
    TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
};
const ABI_VERSION_V2: u16 = 2;
/// Stable LiabilityBasisV2 SBF refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LiabilityBasisSbfErrorV2 {
    /// Instruction bytes were not the sole canonical V2 action.
    Instruction = 0x5100,
    /// Account count, order, privilege, owner, or alias checks refused.
    Accounts = 0x5101,
    /// Claims aggregate or Position bytes/PDA/revision refused.
    ClaimsState = 0x5102,
    /// A finalized raw-record PDA, staging vacancy, rent, or digest refused.
    FinalizedRecord = 0x5103,
    /// Product instance, basis identity, runtime width, or Core join refused.
    ProductLink = 0x5104,
    /// Registry current-deployment authentication refused.
    Release = 0x5105,
    /// The pure exact liability transition refused.
    Candidate = 0x5106,
    /// Exact Custody request, authority, replay, or vault binding refused.
    CustodyRequest = 0x5107,
    /// Custody CPI failed.
    CustodyCpi = 0x5108,
    /// Custody return data or physical postconditions refused.
    Postcondition = 0x5109,
    /// Complete candidate state could not be committed atomically.
    Commit = 0x510A,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    LiabilityBasisSbfErrorV2::Instruction as u32
        == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x100,
    "LiabilityBasisSbfErrorV2 must start at its registered refusal band base"
);
const _: () = assert!(
    (LiabilityBasisSbfErrorV2::Commit as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "LiabilityBasisSbfErrorV2 must not run past its registered refusal band"
);

impl From<LiabilityBasisSbfErrorV2> for ProgramError {
    fn from(value: LiabilityBasisSbfErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

/// Encode canonical runtime-width aggregate state for initialization tooling.
pub fn encode_liability_basis_market_v2(
    input: LiabilityBasisMarketInputV2,
    supplies: &[u64],
) -> Result<Vec<u8>, LiabilityBasisSbfErrorV2> {
    let claim_count =
        u32::try_from(supplies.len()).map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    let width = vector_width(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, claim_count)?;
    let mut output = alloc::vec![0_u8; width];
    encode_liability_basis_market_into_v2(input, supplies, &mut output)
        .map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    Ok(output)
}

/// Encode canonical runtime-width Position state for initialization tooling.
pub fn encode_liability_basis_position_v2(
    input: LiabilityBasisPositionInputV2,
    balances: &[u64],
) -> Result<Vec<u8>, LiabilityBasisSbfErrorV2> {
    let claim_count =
        u32::try_from(balances.len()).map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    let width = vector_width(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, claim_count)?;
    let mut output = alloc::vec![0_u8; width];
    encode_liability_basis_position_into_v2(input, balances, &mut output)
        .map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    Ok(output)
}

/// Encode one exact rational terminal-coordinate record.
pub fn encode_terminal_coordinate_v2(
    numerator: i64,
    denominator: u32,
) -> Result<[u8; TERMINAL_COORDINATE_BYTES_V2], LiabilityBasisSbfErrorV2> {
    if denominator == 0 {
        return Err(LiabilityBasisSbfErrorV2::Instruction);
    }
    let mut output = [0_u8; TERMINAL_COORDINATE_BYTES_V2];
    put_infallible(&mut output, 0, &TERMINAL_COORDINATE_MAGIC_V2);
    put_infallible(&mut output, 8, &ABI_VERSION_V2.to_le_bytes());
    put_infallible(&mut output, 16, &numerator.to_le_bytes());
    put_infallible(&mut output, 24, &denominator.to_le_bytes());
    Ok(output)
}

pub(crate) fn vector_width(
    header: usize,
    claim_count: u32,
) -> Result<usize, LiabilityBasisSbfErrorV2> {
    liability_basis_vector_width_v2(header, claim_count)
        .map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)
}

pub(crate) fn read_vector(
    bytes: &[u8],
    offset: usize,
    claim_count: u32,
) -> Result<Vec<u64>, LiabilityBasisSbfErrorV2> {
    let count = usize::try_from(claim_count).map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        output.push(
            read_claim_v2(
                bytes,
                offset,
                claim_count,
                u32::try_from(index).map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?,
            )
            .map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?,
        );
    }
    Ok(output)
}

fn put_infallible(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}
