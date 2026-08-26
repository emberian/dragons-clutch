//! Chain-derived AccountProfile projection into the onchain Fractional Hot contract.
//!
//! This module owns no execution or wire authority. It converts existing
//! nonforgeable Fractional Token, Claims, and Rent plans into the exact borrowed
//! candidate inputs independently revalidated by the onchain-safe contract.

use dclutch_fractional_claim_contract::{
    FractionalExposureActionV2, FractionalExposureRequestV2, FractionalHotAccountRefV2,
    FractionalHotClaimsEffectV2, FractionalHotRentCloseV2, FractionalHotTokenEffectV2,
    FractionalHotTokenKindV2,
};
use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;
use dclutch_fractional_claims_kernel::PreparedFractionalExposureSignedDeltaV2;

use crate::{
    Error, FractionalExposureRentClosePlanV2, FractionalExposureRetirementPlanV2,
    FractionalExposureTerminalCandidateV2, FractionalExposureTokenEffectV2,
    FractionalExposureTokenPlanV2, Result,
};

/// Authenticated ordered AccountProfile expansion used only as an operator projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotProfileV2<'a> {
    ordered_keys: &'a [[u8; 32]],
}

impl<'a> FractionalHotProfileV2<'a> {
    /// Admit one bounded profile expansion.
    pub fn new(ordered_keys: &'a [[u8; 32]]) -> Result<Self> {
        if ordered_keys.is_empty() || ordered_keys.len() > usize::from(u16::MAX) + 1 {
            return Err(Error::AccountFrame);
        }
        Ok(Self { ordered_keys })
    }

    /// Resolve one expected identity at one exact profile coordinate.
    pub fn account(
        self,
        coordinate: u16,
        expected_key: [u8; 32],
    ) -> Result<FractionalHotAccountRefV2> {
        if self.ordered_keys.get(usize::from(coordinate)) != Some(&expected_key) {
            return Err(Error::AccountFrame);
        }
        FractionalHotAccountRefV2::new(coordinate, expected_key).map_err(|_| Error::AccountFrame)
    }

    /// Exact number of expanded profile coordinates.
    pub const fn len(self) -> usize {
        self.ordered_keys.len()
    }

    /// Whether the profile has no coordinates.
    pub const fn is_empty(self) -> bool {
        self.ordered_keys.is_empty()
    }
}

/// Exact profile coordinates for one non-retirement Token effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotTokenCoordinatesV2 {
    /// Terms-selected Token program.
    pub token_program: u16,
    /// Terms-selected shard Mint.
    pub mint: u16,
    /// Request-selected source, absent exactly when inactive.
    pub source: Option<u16>,
    /// Request-selected destination, absent exactly when inactive.
    pub destination: Option<u16>,
    /// Root or actor authority selected by the action.
    pub authority: u16,
}

/// Exact shared and K-ordered coordinates for zero-supply retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotRetirementCoordinatesV2<'a> {
    /// Terms-selected Token program.
    pub token_program: u16,
    /// Fractional root controlling every shard Mint.
    pub root: u16,
    /// Root-bound lifecycle RentCredit receiving Mint lamports.
    pub rent_credit: u16,
    /// One exact Mint coordinate for every terms-ordered representation coordinate.
    pub mint_coordinates: &'a [u16],
}

/// Exact selected child-program coordinate and contiguous route base.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalHotChildCoordinatesV2 {
    /// Registry-selected child program coordinate.
    pub program: u16,
    /// First coordinate of the canonical child frame.
    pub route_base: u16,
}

/// Lower one already checked Token plan into the onchain candidate effect.
pub fn lower_fractional_hot_token_effect_v2(
    profile: FractionalHotProfileV2<'_>,
    terms: FractionalExposureTermsV2<'_>,
    request: FractionalExposureRequestV2,
    root: FractionalHotAccountRefV2,
    plan: &FractionalExposureTokenPlanV2,
    coordinates: FractionalHotTokenCoordinatesV2,
) -> Result<FractionalHotTokenEffectV2> {
    let request = request.bind_terms(terms).map_err(|_| Error::Token)?;
    let input = request.input();
    let (kind, expected_authority) = match (request.action(), plan.effect()) {
        (FractionalExposureActionV2::Wrap, FractionalExposureTokenEffectV2::Mint(_)) => {
            (FractionalHotTokenKindV2::Mint, root.key())
        }
        (FractionalExposureActionV2::Transfer, FractionalExposureTokenEffectV2::Transfer(_)) => {
            (FractionalHotTokenKindV2::Transfer, input.owner)
        }
        (
            FractionalExposureActionV2::WholeUnwrap
            | FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn,
            FractionalExposureTokenEffectV2::Burn(_),
        ) => (FractionalHotTokenKindV2::Burn, root.key()),
        _ => return Err(Error::Token),
    };
    let mint = terms
        .shard_mint(input.representation_coordinate)
        .map_err(|_| Error::Token)?;
    let source = checked_optional_account(profile, coordinates.source, input.source_token_account)?;
    let destination = checked_optional_account(
        profile,
        coordinates.destination,
        input.destination_token_account,
    )?;
    Ok(FractionalHotTokenEffectV2 {
        kind,
        representation_coordinate: input.representation_coordinate,
        token_program: profile.account(coordinates.token_program, terms.token_program())?,
        mint: profile.account(coordinates.mint, mint)?,
        source,
        destination,
        authority: profile.account(coordinates.authority, expected_authority)?,
        amount: plan.consumed_shards(),
        pre_supply: plan.pre_supply(),
        post_supply: plan.post_supply(),
        pre_source: plan.pre_source(),
        post_source: plan.post_source(),
        pre_destination: plan.pre_destination(),
        post_destination: plan.post_destination(),
    })
}

/// Lower K ordered zero-supply Mint closures into caller-owned fixed storage.
pub fn lower_fractional_hot_retirement_effects_v2(
    profile: FractionalHotProfileV2<'_>,
    terms: FractionalExposureTermsV2<'_>,
    request: FractionalExposureRequestV2,
    retirement: &FractionalExposureRetirementPlanV2,
    coordinates: FractionalHotRetirementCoordinatesV2<'_>,
    output: &mut [FractionalHotTokenEffectV2],
) -> Result<()> {
    let request = request.bind_terms(terms).map_err(|_| Error::Rent)?;
    let width = usize::try_from(terms.representation_width()).map_err(|_| Error::Rent)?;
    if request.action() != FractionalExposureActionV2::ZeroSupplyRetire
        || retirement.market() != terms.market()
        || retirement.release_set() != terms.release_set()
        || retirement.post_revision()
            != request
                .input()
                .expected_revision
                .checked_add(1)
                .ok_or(Error::Rent)?
        || retirement.instructions().len() != width
        || coordinates.mint_coordinates.len() != width
        || output.len() != width
    {
        return Err(Error::Rent);
    }
    let token_program = profile.account(coordinates.token_program, terms.token_program())?;
    // Retirement is permissionless, so the request owner is canonically absent;
    // the actual root comes from the nonforgeable retirement instructions.
    if request.input().owner == [0; 32] {
        return lower_retirement_with_instruction_root(
            profile,
            terms,
            retirement,
            coordinates,
            token_program,
            output,
        );
    }
    Err(Error::Rent)
}

fn lower_retirement_with_instruction_root(
    profile: FractionalHotProfileV2<'_>,
    terms: FractionalExposureTermsV2<'_>,
    retirement: &FractionalExposureRetirementPlanV2,
    coordinates: FractionalHotRetirementCoordinatesV2<'_>,
    token_program: FractionalHotAccountRefV2,
    output: &mut [FractionalHotTokenEffectV2],
) -> Result<()> {
    let first = retirement.instructions().first().ok_or(Error::Rent)?;
    let root_key = instruction_key(first, 2)?;
    let root = profile.account(coordinates.root, root_key)?;
    let rent_credit =
        profile.account(coordinates.rent_credit, retirement.rent_credit().to_bytes())?;
    for (index, slot) in output.iter_mut().enumerate() {
        let coordinate = u32::try_from(index).map_err(|_| Error::Rent)?;
        let mint_key = terms.shard_mint(coordinate).map_err(|_| Error::Rent)?;
        let instruction = retirement.instructions().get(index).ok_or(Error::Rent)?;
        if instruction.program_id.to_bytes() != terms.token_program()
            || instruction_key(instruction, 0)? != mint_key
            || instruction_key(instruction, 1)? != rent_credit.key()
            || instruction_key(instruction, 2)? != root.key()
        {
            return Err(Error::Rent);
        }
        *slot = FractionalHotTokenEffectV2 {
            kind: FractionalHotTokenKindV2::CloseMint,
            representation_coordinate: coordinate,
            token_program,
            mint: profile.account(
                *coordinates.mint_coordinates.get(index).ok_or(Error::Rent)?,
                mint_key,
            )?,
            source: None,
            destination: Some(rent_credit),
            authority: root,
            amount: 0,
            pre_supply: 0,
            post_supply: 0,
            pre_source: 0,
            post_source: 0,
            pre_destination: 0,
            post_destination: 0,
        };
    }
    Ok(())
}

/// Project the checked wrap/unwrap packet into the sole SignedDelta child route.
pub fn lower_fractional_hot_signed_delta_v2<'a>(
    profile: FractionalHotProfileV2<'_>,
    prepared: PreparedFractionalExposureSignedDeltaV2,
    packet: &'a [u8],
    coordinates: FractionalHotChildCoordinatesV2,
) -> Result<FractionalHotClaimsEffectV2<'a>> {
    prepared.table_bytes(packet).map_err(|_| Error::Claims)?;
    Ok(FractionalHotClaimsEffectV2::SignedDelta {
        claims_program: profile.account(coordinates.program, prepared.claims_program())?,
        route_base: coordinates.route_base,
        packet,
    })
}

/// Project the checked terminal plan into the sole generic Claims terminal route.
pub fn lower_fractional_hot_terminal_v2<'a>(
    profile: FractionalHotProfileV2<'_>,
    candidate: &'a FractionalExposureTerminalCandidateV2,
    coordinates: FractionalHotChildCoordinatesV2,
) -> Result<FractionalHotClaimsEffectV2<'a>> {
    let request = candidate.settlement_request_ref();
    Ok(FractionalHotClaimsEffectV2::Terminal {
        claims_program: profile.account(coordinates.program, request.input().claims_program)?,
        route_base: coordinates.route_base,
        request,
    })
}

/// Project the canonical lifecycle close into the retirement candidate.
pub fn lower_fractional_hot_rent_close_v2(
    profile: FractionalHotProfileV2<'_>,
    retirement: &FractionalExposureRetirementPlanV2,
    rent_close: FractionalExposureRentClosePlanV2,
    selected_rent_program: [u8; 32],
    coordinates: FractionalHotChildCoordinatesV2,
    rent_credit_coordinate: u16,
) -> Result<FractionalHotRentCloseV2> {
    if rent_close.plan.post_resource_digest() == [0; 32]
        || rent_close.receipt.input().post_resource_digest != rent_close.plan.post_resource_digest()
    {
        return Err(Error::Rent);
    }
    Ok(FractionalHotRentCloseV2 {
        rent_program: profile.account(coordinates.program, selected_rent_program)?,
        rent_credit: profile
            .account(rent_credit_coordinate, retirement.rent_credit().to_bytes())?,
        route_base: coordinates.route_base,
        post_resource_digest: rent_close.plan.post_resource_digest(),
    })
}

fn checked_optional_account(
    profile: FractionalHotProfileV2<'_>,
    coordinate: Option<u16>,
    expected_key: [u8; 32],
) -> Result<Option<FractionalHotAccountRefV2>> {
    match (coordinate, expected_key == [0; 32]) {
        (None, true) => Ok(None),
        (Some(coordinate), false) => profile.account(coordinate, expected_key).map(Some),
        _ => Err(Error::AccountFrame),
    }
}

fn instruction_key(
    instruction: &solana_program::instruction::Instruction,
    index: usize,
) -> Result<[u8; 32]> {
    instruction
        .accounts
        .get(index)
        .map(|meta| meta.pubkey.to_bytes())
        .ok_or(Error::Rent)
}
