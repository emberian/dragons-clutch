//! The shared LiabilityBasisV2 state vocabulary.
//!
//! This module is now exactly what outlived the `DCLLBX02` route: the re-export
//! surface eight sibling modules and roughly twenty crates reach the LBV2
//! aggregate/Position vocabulary through (`MarketViewV2`, `PositionViewV2`,
//! `LIABILITY_BASIS_MARKET_SEED_V2`, the two header widths, the two input
//! types), and the three encode/read helpers that speak it. It owns no route
//! and dispatches nothing.
//!
//! Its refusal enum outlived the route too, and for four months longer than it
//! should have: eleven `#[repr(u32)]` discriminants, ten of which nothing in
//! the tree could raise, all eleven published by every generated mirror. The
//! banishment below was finished at the Rust boundary and at the browser, and
//! missed the taxonomy in between. The ten are withdrawn as of 2026-09-01; see
//! [`LiabilityBasisSbfErrorV2::ClaimsState`] for why removal, not annotation,
//! is the honest disposition here.
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
/// Stable LiabilityBasisV2 SBF refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LiabilityBasisSbfErrorV2 {
    /// Claims aggregate or Position bytes/PDA/revision refused.
    ///
    /// The only refusal left in this family, and the only one there was ever
    /// anything to raise. Ten siblings occupied `0x5101..=0x510A` until
    /// 2026-09-01 -- `Instruction`, `Accounts`, `FinalizedRecord`,
    /// `ProductLink`, `Release`, `Candidate`, `CustodyRequest`, `CustodyCpi`,
    /// `Postcondition`, `Commit` -- the refusal taxonomy of the `DCLLBX02`
    /// route. That route was deleted (`docs/ASPIRATION_LEDGER.md` D-6,
    /// "ANSWERED AND EXECUTED: deleted", dead on both ends), and the taxonomy
    /// was left behind: still `#[repr(u32)]`, so the census still enumerated it
    /// and every generated mirror still published ten codes the protocol could
    /// not return, under a page that promises "every error code the protocol
    /// can return".
    ///
    /// They are removed rather than annotated because the question that decides
    /// it is answerable and was answered: the route is GONE, not reserved. Nor
    /// can any historical log resolve against them -- `DCLLBX02` had no
    /// producer in the tree and no `DCLTLNK2` record was ever finalized on
    /// chain, so not one of the ten was ever raised anywhere. Withdrawn, never
    /// reused: `0x5101..=0x510A` stay spent inside Claims' sub-band.
    ///
    /// This one survives because the module's three read/encode helpers still
    /// raise it, and roughly twenty crates reach LBV2 state through them.
    ClaimsState = 0x5100,
}

impl LiabilityBasisSbfErrorV2 {
    /// Every refusal this request family can raise, in discriminant order.
    ///
    /// This is what the sub-band assertions below read. It is kept honest by
    /// [`LiabilityBasisSbfErrorV2::ordinal`], whose match is exhaustive: a variant added to the
    /// enum does not compile until its author writes an arm here, and the only arm that satisfies
    /// the assertions is its own index in this array.
    pub const ALL: [Self; 1] = [Self::ClaimsState];

    /// This refusal's position in [`LiabilityBasisSbfErrorV2::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a second variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::ClaimsState => 0,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing about
// the variants after it and goes stale silently every single time the family
// grows -- the failure is not that the name is wrong, it is that nothing can
// notice. Claims' own top-level band proved it the expensive way: its bound went
// on naming `ReleaseSuperseded` after a later variant landed, so for as long as
// that stood, the newest refusal in the program was checked by nothing.
//
// So the sub-band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    const SUB_BAND: u32 = dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x100;
    assert!(
        LiabilityBasisSbfErrorV2::ALL[0] as u32 == SUB_BAND,
        "LiabilityBasisSbfErrorV2 must start at its registered sub-band offset"
    );
    let mut index = 0;
    while index < LiabilityBasisSbfErrorV2::ALL.len() {
        let variant = LiabilityBasisSbfErrorV2::ALL[index];
        assert!(
            variant.ordinal() == index,
            "LiabilityBasisSbfErrorV2::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32 == SUB_BAND + index as u32,
            "LiabilityBasisSbfErrorV2 discriminants are not the contiguous run from the sub-band offset that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "LiabilityBasisSbfErrorV2 must not run past its registered refusal band"
        );
        index += 1;
    }
};

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
