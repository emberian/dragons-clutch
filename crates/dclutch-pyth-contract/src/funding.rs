//! Pyth specialization of the canonical typed capability-funding ledger.
//!
//! Resolution funding is native lamports only. Rent, provider reimbursement,
//! and bounty are never interpreted as Realm collateral or added across units.

pub use dclutch_capability_contract::FundingStateV1;
use dclutch_capability_contract::{
    CapabilityManifestV1, ContentId, FUNDING_STATE_BYTES, FundingAssetClassV1,
    FundingCustodyObservationV1, FundingStatus, RequiredFoundingEntryV1,
};

use crate::{Error, Result};

/// Exact physical byte width of a Pyth Resolution Fund.
pub const FUNDING_BYTES: usize = FUNDING_STATE_BYTES;

/// Construct and activate required-at-founding native Pyth funding.
pub fn construct_required_resolution_funding(
    manifest_content_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    selected: RequiredFoundingEntryV1,
    exact_fund_rent: u64,
    current_slot: u64,
) -> Result<FundingStateV1> {
    let canonical = manifest
        .required_founding_entry_for_config(selected.entry().config_id())
        .map_err(capability_error)?;
    if canonical != selected {
        return Err(Error::FundingSelectionMismatch);
    }
    let quote = selected
        .validate_one_shot_resolution_fund_quote(exact_fund_rent)
        .map_err(capability_error)?;
    require_native_quote(quote)?;
    let initial_lamports = exact_fund_rent
        .checked_add(quote.native_lamports_total())
        .ok_or(Error::ArithmeticOverflow)?;
    let initial = FundingCustodyObservationV1::native_only(initial_lamports, exact_fund_rent)
        .map_err(capability_error)?;
    let mut funding = FundingStateV1::new(manifest_content_id, manifest, selected.index(), initial)
        .map_err(capability_error)?;
    let debit = funding
        .activate(manifest_content_id, manifest, initial, current_slot)
        .map_err(capability_error)?;
    if debit.rent_lamports() != exact_fund_rent || debit.creation_lamports() != 0 {
        return Err(Error::InvalidResolutionFundShape);
    }
    let active_lamports = exact_fund_rent
        .checked_add(funding.remaining().native_lamports_total())
        .ok_or(Error::ArithmeticOverflow)?;
    validate_required_resolution_funding(
        funding,
        manifest_content_id,
        manifest,
        selected,
        exact_fund_rent,
        FundingCustodyObservationV1::native_only(active_lamports, exact_fund_rent)
            .map_err(capability_error)?,
    )?;
    Ok(funding)
}

/// Validate one active Pyth fund against an exact typed custody observation.
///
/// The observation must be native-only. Its state-account Rent reserve is
/// separate from its held native principal, so this API accepts neither a flat
/// `u64` present-principal assertion nor a Realm-collateral vault.
pub fn validate_required_resolution_funding(
    funding: FundingStateV1,
    manifest_content_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    selected: RequiredFoundingEntryV1,
    exact_fund_rent: u64,
    custody: FundingCustodyObservationV1,
) -> Result<()> {
    validate_local_shape(funding)?;
    if custody.exact_state_rent_lamports() != exact_fund_rent
        || custody.realm_collateral().is_some()
    {
        return Err(Error::InvalidResolutionFundShape);
    }
    let canonical = manifest
        .required_founding_entry_for_config(selected.entry().config_id())
        .map_err(capability_error)?;
    if canonical != selected || selected.index() != funding.entry_index() {
        return Err(Error::FundingSelectionMismatch);
    }
    let quote = selected
        .validate_one_shot_resolution_fund_quote(exact_fund_rent)
        .map_err(capability_error)?;
    require_native_quote(quote)?;
    funding
        .validate_against(manifest_content_id, manifest, custody)
        .map_err(capability_error)?;
    let remaining = funding.remaining();
    let released = funding.released();
    if remaining.provider() != quote.amounts().provider()
        || remaining.bounty() != quote.amounts().bounty()
        || released.rent() != quote.amounts().rent()
    {
        return Err(Error::InvalidResolutionFundShape);
    }
    Ok(())
}

/// Return state Rent plus still-held native provider and bounty principal.
pub fn required_resolution_minimum_balance(funding: FundingStateV1) -> Result<u64> {
    validate_local_shape(funding)?;
    funding
        .released()
        .rent()
        .amount()
        .checked_add(funding.remaining().native_lamports_total())
        .ok_or(Error::ArithmeticOverflow)
}

fn require_native_quote(quote: dclutch_capability_contract::FundingQuoteV1) -> Result<()> {
    let amounts = quote.amounts();
    if quote.realm_collateral().is_some()
        || amounts.rent().asset_class() != FundingAssetClassV1::NativeLamports
        || amounts.creation().asset_class() != FundingAssetClassV1::NotApplicable
        || amounts.work().asset_class() != FundingAssetClassV1::NotApplicable
        || amounts.liquidity().asset_class() != FundingAssetClassV1::NotApplicable
        || amounts.service().asset_class() != FundingAssetClassV1::NotApplicable
        || !matches!(
            amounts.provider().asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
        )
        || amounts.bounty().asset_class() != FundingAssetClassV1::NativeLamports
    {
        return Err(Error::InvalidResolutionFundShape);
    }
    Ok(())
}

fn validate_local_shape(funding: FundingStateV1) -> Result<()> {
    let remaining = funding.remaining();
    let released = funding.released();
    if funding.status() != FundingStatus::Active
        || remaining.rent().amount() != 0
        || remaining.creation().amount() != 0
        || remaining.work().amount() != 0
        || remaining.liquidity().amount() != 0
        || remaining.service().amount() != 0
        || released.creation().amount() != 0
        || released.work().amount() != 0
        || released.provider().amount() != 0
        || released.bounty().amount() != 0
        || released.liquidity().amount() != 0
        || released.service().amount() != 0
        || released.rent().asset_class() != FundingAssetClassV1::NativeLamports
        || !matches!(
            remaining.provider().asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
        )
        || remaining.bounty().asset_class() != FundingAssetClassV1::NativeLamports
        || remaining.bounty().amount() == 0
    {
        return Err(Error::InvalidResolutionFundShape);
    }
    Ok(())
}

fn capability_error(error: dclutch_capability_contract::Error) -> Error {
    Error::InvalidCapabilityFunding { error }
}

#[cfg(test)]
mod tests {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
        CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    use super::*;

    const MANIFEST_BYTES: usize = MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("content ID")
    }
    fn native(value: u64) -> CompartmentFundingV1 {
        CompartmentFundingV1::native_lamports(value).expect("native")
    }
    fn na() -> CompartmentFundingV1 {
        CompartmentFundingV1::not_applicable()
    }
    fn quote(rent: u64, provider: u64, bounty: u64) -> FundingQuoteV1 {
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                native(rent),
                na(),
                na(),
                if provider == 0 {
                    na()
                } else {
                    native(provider)
                },
                native(bounty),
                na(),
                na(),
            )
            .expect("amounts"),
            None,
        )
        .expect("quote")
    }
    fn manifest(
        storage: &mut [u8; MANIFEST_BYTES],
        quote: FundingQuoteV1,
    ) -> CapabilityManifestV1<'_> {
        let entry = CapabilityEntryV1::new(
            id(11),
            id(21),
            id(31),
            id(23),
            id(24),
            id(25),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("entry");
        CapabilityManifestV1::encode_into(&[entry], storage).expect("manifest")
    }
    fn custody(rent: u64, provider: u64, bounty: u64) -> FundingCustodyObservationV1 {
        FundingCustodyObservationV1::native_only(rent + provider + bounty, rent).expect("custody")
    }

    #[test]
    fn construction_is_typed_native_and_close_amount_is_exact() {
        let mut storage = [0; MANIFEST_BYTES];
        let manifest = manifest(&mut storage, quote(100, 7, 11));
        let selected = manifest
            .required_founding_entry_for_config(id(31))
            .expect("selected");
        let funding = construct_required_resolution_funding(id(99), manifest, selected, 100, 44)
            .expect("funding");
        assert_eq!(FUNDING_BYTES, FUNDING_STATE_BYTES);
        assert_eq!(funding.remaining().provider().amount(), 7);
        assert_eq!(funding.remaining().bounty().amount(), 11);
        assert_eq!(funding.released().rent().amount(), 100);
        assert_eq!(required_resolution_minimum_balance(funding), Ok(118));
        assert_eq!(
            validate_required_resolution_funding(
                funding,
                id(99),
                manifest,
                selected,
                100,
                custody(100, 7, 11)
            ),
            Ok(())
        );
    }

    #[test]
    fn partial_native_custody_refuses_without_a_distribution_plan() {
        let mut storage = [0; MANIFEST_BYTES];
        let manifest = manifest(&mut storage, quote(100, 7, 11));
        let selected = manifest
            .required_founding_entry_for_config(id(31))
            .expect("selected");
        let funding = construct_required_resolution_funding(id(99), manifest, selected, 100, 44)
            .expect("funding");
        assert!(
            validate_required_resolution_funding(
                funding,
                id(99),
                manifest,
                selected,
                100,
                FundingCustodyObservationV1::native_only(117, 100).expect("short"),
            )
            .is_err()
        );
    }

    #[test]
    fn non_native_quote_is_refused_by_the_specialization() {
        let amounts = FundingAmountsV1::new(
            native(100),
            na(),
            na(),
            CompartmentFundingV1::realm_collateral(7).expect("realm"),
            native(11),
            na(),
            na(),
        )
        .expect("amounts");
        let binding = dclutch_capability_contract::RealmCollateralBindingV1::new(
            id(1),
            id(2),
            [3; 32],
            [4; 32],
            [5; 32],
        )
        .expect("binding");
        let quote = FundingQuoteV1::new(amounts, Some(binding)).expect("typed quote");
        let mut storage = [0; MANIFEST_BYTES];
        let manifest = manifest(&mut storage, quote);
        let selected = manifest
            .required_founding_entry_for_config(id(31))
            .expect("selected");
        assert_eq!(
            construct_required_resolution_funding(id(99), manifest, selected, 100, 44),
            Err(Error::InvalidCapabilityFunding {
                error: dclutch_capability_contract::Error::ExtraneousResolutionFundPrincipal,
            })
        );
    }
}
