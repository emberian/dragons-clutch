//! Projection of checked effect plans into the onchain Structured V2 candidate.
//!
//! This module owns no execution or wire authority.  It converts an already
//! checked [`StructuredTokenEffectPlanV2`] sequence into the exact borrowed
//! candidate inputs that the onchain-safe contract independently revalidates.

use dclutch_structured_v2_contract::{
    StructuredHotAccountRefV2, StructuredHotRentCloseV2, StructuredHotTokenEffectV2,
};

use crate::{Error, Result, StructuredActionPlanV2, StructuredTokenEffectPlanV2};

/// Authenticated ordered AccountProfile expansion used only as a projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredHotProfileV2<'a> {
    ordered_keys: &'a [[u8; 32]],
}

impl<'a> StructuredHotProfileV2<'a> {
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
    ) -> Result<StructuredHotAccountRefV2> {
        if self.ordered_keys.get(usize::from(coordinate)) != Some(&expected_key) {
            return Err(Error::AccountFrame);
        }
        StructuredHotAccountRefV2::new(coordinate, expected_key).map_err(|_| Error::AccountFrame)
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

/// Exact profile coordinates for one Token effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredHotEffectCoordinatesV2 {
    /// Terms-selected Token program.
    pub token_program: u16,
    /// Receipt Mint or shard Mint.
    pub mint: u16,
    /// Source account, absent exactly when the plan has none.
    pub source: Option<u16>,
    /// Destination account, absent exactly when the plan has none.
    pub destination: Option<u16>,
    /// Root or actor authority selected by the action.
    pub authority: u16,
}

/// Lower one already checked action plan into the onchain candidate effects.
///
/// `coordinates` and `output` are caller-owned fixed storage whose lengths must
/// equal the plan's effect count, so the projection cannot silently drop or
/// duplicate an effect.
pub fn lower_structured_hot_effects_v2(
    profile: StructuredHotProfileV2<'_>,
    plan: &StructuredActionPlanV2,
    coordinates: &[StructuredHotEffectCoordinatesV2],
    output: &mut [StructuredHotTokenEffectV2],
) -> Result<()> {
    if coordinates.len() != plan.effects.len() || output.len() != plan.effects.len() {
        return Err(Error::AccountFrame);
    }
    for (index, effect) in plan.effects.iter().enumerate() {
        let selected = coordinates.get(index).copied().ok_or(Error::AccountFrame)?;
        *output.get_mut(index).ok_or(Error::AccountFrame)? =
            lower_one_effect(profile, *effect, selected)?;
    }
    Ok(())
}

/// Project the canonical lifecycle close into the retirement candidate.
pub fn lower_structured_hot_rent_close_v2(
    profile: StructuredHotProfileV2<'_>,
    selected_rent_program: [u8; 32],
    rent_credit: [u8; 32],
    post_resource_digest: [u8; 32],
    rent_program_coordinate: u16,
    rent_credit_coordinate: u16,
    route_base: u16,
) -> Result<StructuredHotRentCloseV2> {
    if post_resource_digest == [0; 32] {
        return Err(Error::Rent);
    }
    Ok(StructuredHotRentCloseV2 {
        rent_program: profile.account(rent_program_coordinate, selected_rent_program)?,
        rent_credit: profile.account(rent_credit_coordinate, rent_credit)?,
        route_base,
        post_resource_digest,
    })
}

fn lower_one_effect(
    profile: StructuredHotProfileV2<'_>,
    effect: StructuredTokenEffectPlanV2,
    coordinates: StructuredHotEffectCoordinatesV2,
) -> Result<StructuredHotTokenEffectV2> {
    Ok(StructuredHotTokenEffectV2 {
        kind: effect.kind,
        representation_coordinate: effect.representation_coordinate,
        token_program: profile.account(coordinates.token_program, effect.token_program)?,
        mint: profile.account(coordinates.mint, effect.mint)?,
        source: optional_account(profile, coordinates.source, effect.source)?,
        destination: optional_account(profile, coordinates.destination, effect.destination)?,
        authority: profile.account(coordinates.authority, effect.authority)?,
        amount: effect.amount,
        pre_supply: effect.pre_supply,
        post_supply: effect.post_supply,
        pre_source: effect.pre_source,
        post_source: effect.post_source,
        pre_destination: effect.pre_destination,
        post_destination: effect.post_destination,
    })
}

fn optional_account(
    profile: StructuredHotProfileV2<'_>,
    coordinate: Option<u16>,
    expected_key: Option<[u8; 32]>,
) -> Result<Option<StructuredHotAccountRefV2>> {
    match (coordinate, expected_key) {
        (None, None) => Ok(None),
        (Some(coordinate), Some(key)) => profile.account(coordinate, key).map(Some),
        _ => Err(Error::AccountFrame),
    }
}
