//! Pyth specialization of the canonical capability funding ledger.

pub use dclutch_capability_contract::FundingStateV1;
use dclutch_capability_contract::{
    CapabilityManifestV1, ContentId, FUNDING_STATE_BYTES, FundingStatus, RequiredFoundingEntryV1,
};

use crate::{Error, Result};

/// Exact physical byte width of a Pyth Resolution Fund.
///
/// The physical account is exactly [`FundingStateV1`]; there is no outer Pyth
/// header or duplicate Market, generation, refund, provider, or bounty fact.
pub const FUNDING_BYTES: usize = FUNDING_STATE_BYTES;

/// Construct canonical required-at-founding funding for one Pyth resolution child.
///
/// The adapter authenticates `manifest_content_id` as the content hash of
/// `manifest`, derives `selected` uniquely from the Market's immutable
/// resolution-policy identity, computes `exact_fund_rent` for
/// [`FUNDING_BYTES`], and creates/funds the physical account atomically. This
/// function models an exactly prepaid Pending state followed immediately by
/// required-at-founding activation. Rent is released into physical account
/// rent; provider reimbursement and positive bounty remain the only held
/// non-rent principal.
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
    let mut funding = FundingStateV1::new(
        manifest_content_id,
        manifest,
        selected.index(),
        quote.total_principal(),
    )
    .map_err(capability_error)?;
    let debit = funding
        .activate(
            manifest_content_id,
            manifest,
            quote.total_principal(),
            current_slot,
        )
        .map_err(capability_error)?;
    if debit.rent_principal() != exact_fund_rent || debit.creation_principal() != 0 {
        return Err(Error::InvalidResolutionFundShape);
    }
    validate_required_resolution_funding(
        funding,
        manifest_content_id,
        manifest,
        selected,
        exact_fund_rent,
        funding.remaining().total_principal(),
    )?;
    Ok(funding)
}

/// Validate one raw canonical funding state as the current Pyth Fund profile.
///
/// The adapter supplies the authenticated manifest identity and bytes, the
/// unique `selected` entry derived from the Market resolution-policy identity,
/// freshly calculated rent for [`FUNDING_BYTES`], and physically observed held
/// non-rent principal. Market occurrence, generation, PDA derivation, account
/// owner, and refund authority are authenticated outside this function and are
/// deliberately not persisted again in the Fund.
pub fn validate_required_resolution_funding(
    funding: FundingStateV1,
    manifest_content_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    selected: RequiredFoundingEntryV1,
    exact_fund_rent: u64,
    observed_non_rent_principal: u64,
) -> Result<()> {
    validate_local_shape(funding)?;
    let canonical = manifest
        .required_founding_entry_for_config(selected.entry().config_id())
        .map_err(capability_error)?;
    if canonical != selected || selected.index() != funding.entry_index() {
        return Err(Error::FundingSelectionMismatch);
    }
    let quote = selected
        .validate_one_shot_resolution_fund_quote(exact_fund_rent)
        .map_err(capability_error)?;
    funding
        .validate_against(manifest_content_id, manifest, observed_non_rent_principal)
        .map_err(capability_error)?;
    let remaining = funding.remaining();
    let released = funding.released();
    if remaining.provider_principal() != quote.provider_principal()
        || remaining.bounty_principal() != quote.bounty_principal()
        || released.rent_principal() != exact_fund_rent
    {
        return Err(Error::InvalidResolutionFundShape);
    }
    Ok(())
}

/// Return raw Fund rent plus its exact still-held provider and bounty principal.
///
/// Call [`validate_required_resolution_funding`] first. The adapter refuses an
/// account below this minimum and routes any excess using the authenticated
/// Market root's refund identity, not data duplicated in the Fund.
pub fn required_resolution_minimum_balance(funding: FundingStateV1) -> Result<u64> {
    validate_local_shape(funding)?;
    funding
        .released()
        .rent_principal()
        .checked_add(funding.remaining().total_principal())
        .ok_or(Error::ArithmeticOverflow)
}

fn validate_local_shape(funding: FundingStateV1) -> Result<()> {
    let remaining = funding.remaining();
    let released = funding.released();
    if funding.status() != FundingStatus::Active
        || remaining.rent_principal() != 0
        || remaining.creation_principal() != 0
        || remaining.work_principal() != 0
        || remaining.bounty_principal() == 0
        || remaining.liquidity_principal() != 0
        || remaining.service_principal() != 0
        || released.creation_principal() != 0
        || released.work_principal() != 0
        || released.provider_principal() != 0
        || released.bounty_principal() != 0
        || released.liquidity_principal() != 0
        || released.service_principal() != 0
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
        Error as CapabilityError, FundingCompartment, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    use super::*;

    const MANIFEST_BYTES_1: usize = MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES;
    const MANIFEST_BYTES_2: usize = MANIFEST_HEADER_BYTES + 2 * CAPABILITY_ENTRY_BYTES;
    const STATE_STATUS: usize = 10;
    const STATE_ENTRY_INDEX: usize = 48;
    const STATE_REMAINING_PROVIDER: usize = 64 + 24;
    const STATE_REMAINING_TOTAL: usize = 64 + 56;
    const STATE_RELEASED_RENT: usize = 128;
    const STATE_RELEASED_TOTAL: usize = 128 + 56;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero content id")
    }

    #[allow(clippy::too_many_arguments)]
    fn quote(
        rent: u64,
        creation: u64,
        work: u64,
        provider: u64,
        bounty: u64,
        liquidity: u64,
        service: u64,
    ) -> FundingQuoteV1 {
        FundingQuoteV1::new(rent, creation, work, provider, bounty, liquidity, service)
            .expect("checked quote")
    }

    fn entry(kind: u8, config: u8, funding_quote: FundingQuoteV1) -> CapabilityEntryV1 {
        CapabilityEntryV1::new(
            id(kind),
            id(21),
            id(config),
            id(23),
            id(24),
            id(25),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            funding_quote,
        )
        .expect("valid entry")
    }

    fn funding<'a>(
        storage: &'a mut [u8; MANIFEST_BYTES_1],
        rent: u64,
    ) -> (CapabilityManifestV1<'a>, FundingStateV1) {
        let entries = [entry(11, 31, quote(rent, 0, 0, 7, 11, 0, 0))];
        let manifest = CapabilityManifestV1::encode_into(&entries, storage).expect("manifest");
        let selected = manifest
            .required_founding_entry_for_config(id(31))
            .expect("unique founding entry");
        let result = construct_required_resolution_funding(id(99), manifest, selected, rent, 44)
            .expect("resolution funding");
        (manifest, result)
    }

    fn selected(manifest: CapabilityManifestV1<'_>) -> RequiredFoundingEntryV1 {
        manifest
            .required_founding_entry_for_config(id(31))
            .expect("unique founding entry")
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        let destination = bytes.get_mut(offset..offset + 8).expect("test offset");
        destination.copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn construction_activates_the_raw_canonical_ledger() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = funding(&mut storage, 100);
        assert_eq!(FUNDING_BYTES, FUNDING_STATE_BYTES);
        assert_eq!(FUNDING_BYTES, 192);
        assert_eq!(funding.status(), FundingStatus::Active);
        assert_eq!(funding.activation_slot(), 44);
        assert_eq!(funding.manifest_content_id(), id(99));
        assert_eq!(funding.entry_index(), 0);
        assert_eq!(funding.remaining().provider_principal(), 7);
        assert_eq!(funding.remaining().bounty_principal(), 11);
        assert_eq!(funding.remaining().total_principal(), 18);
        assert_eq!(funding.released().rent_principal(), 100);
        assert_eq!(required_resolution_minimum_balance(funding), Ok(118));
        assert_eq!(
            validate_required_resolution_funding(
                funding,
                id(99),
                manifest,
                selected(manifest),
                100,
                18,
            ),
            Ok(())
        );
        let bytes = funding.to_bytes();
        assert_eq!(bytes.len(), FUNDING_BYTES);
        assert_eq!(FundingStateV1::decode(&bytes), Ok(funding));
    }

    #[test]
    fn raw_physical_bytes_have_one_exact_hostile_decoder() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (_, funding) = funding(&mut storage, 100);
        let bytes = funding.to_bytes();
        for length in 0..FUNDING_BYTES {
            assert_eq!(
                FundingStateV1::decode(bytes.get(..length).expect("prefix")),
                Err(CapabilityError::InvalidLength)
            );
        }
        let mut trailing = [0u8; FUNDING_BYTES + 1];
        trailing[..FUNDING_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            FundingStateV1::decode(&trailing),
            Err(CapabilityError::InvalidLength)
        );

        let mut changed = bytes;
        changed[0] = 0;
        assert_eq!(
            FundingStateV1::decode(&changed),
            Err(CapabilityError::InvalidMagic)
        );
        let mut changed = bytes;
        changed[8] = 2;
        assert_eq!(
            FundingStateV1::decode(&changed),
            Err(CapabilityError::UnsupportedSchema)
        );
        let mut changed = bytes;
        changed[11] = 1;
        assert_eq!(
            FundingStateV1::decode(&changed),
            Err(CapabilityError::NonCanonicalReservedBytes)
        );
    }

    #[test]
    fn wrong_manifest_index_status_and_conservation_refuse() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = funding(&mut storage, 100);
        assert_eq!(
            validate_required_resolution_funding(
                funding,
                id(98),
                manifest,
                selected(manifest),
                100,
                18,
            ),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::FundingBindingMismatch,
            })
        );

        let wrong_entries = [entry(11, 31, quote(100, 0, 0, 8, 11, 0, 0))];
        let mut wrong_storage = [0u8; MANIFEST_BYTES_1];
        let wrong_manifest = CapabilityManifestV1::encode_into(&wrong_entries, &mut wrong_storage)
            .expect("wrong manifest");
        assert_eq!(
            validate_required_resolution_funding(
                funding,
                id(99),
                wrong_manifest,
                selected(manifest),
                100,
                18,
            ),
            Err(Error::FundingSelectionMismatch)
        );

        let mut changed = funding.to_bytes();
        changed[STATE_ENTRY_INDEX..STATE_ENTRY_INDEX + 2].copy_from_slice(&1u16.to_le_bytes());
        let wrong_index = FundingStateV1::decode(&changed).expect("structural state");
        assert_eq!(
            validate_required_resolution_funding(
                wrong_index,
                id(99),
                manifest,
                selected(manifest),
                100,
                18,
            ),
            Err(Error::FundingSelectionMismatch)
        );

        let mut changed = funding.to_bytes();
        changed[STATE_STATUS] = 0;
        assert_eq!(
            FundingStateV1::decode(&changed),
            Err(CapabilityError::InvalidFundingStatus)
        );

        let mut changed = funding.to_bytes();
        put_u64(&mut changed, STATE_REMAINING_PROVIDER, 8);
        put_u64(&mut changed, STATE_REMAINING_TOTAL, 19);
        let unconserved = FundingStateV1::decode(&changed).expect("structural state");
        assert_eq!(
            validate_required_resolution_funding(
                unconserved,
                id(99),
                manifest,
                selected(manifest),
                100,
                19,
            ),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::FundingConservationMismatch,
            })
        );
        assert_eq!(
            validate_required_resolution_funding(
                funding,
                id(99),
                manifest,
                selected(manifest),
                100,
                19,
            ),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::PresentPrincipalMismatch,
            })
        );
    }

    #[test]
    fn rent_and_extra_compartments_are_manifest_refusals() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = funding(&mut storage, 100);
        assert_eq!(
            validate_required_resolution_funding(
                funding,
                id(99),
                manifest,
                selected(manifest),
                99,
                18,
            ),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::ResolutionFundRentMismatch,
            })
        );

        for extra in [
            quote(100, 1, 0, 7, 11, 0, 0),
            quote(100, 0, 1, 7, 11, 0, 0),
            quote(100, 0, 0, 7, 11, 1, 0),
            quote(100, 0, 0, 7, 11, 0, 1),
        ] {
            let entries = [entry(11, 31, extra)];
            let mut extra_storage = [0u8; MANIFEST_BYTES_1];
            let extra_manifest = CapabilityManifestV1::encode_into(&entries, &mut extra_storage)
                .expect("extra manifest");
            let selected = extra_manifest
                .required_founding_entry_for_config(id(31))
                .expect("selected");
            assert_eq!(
                construct_required_resolution_funding(id(99), extra_manifest, selected, 100, 44,),
                Err(Error::InvalidCapabilityFunding {
                    error: CapabilityError::ExtraneousResolutionFundPrincipal,
                })
            );
        }

        let entries = [entry(11, 31, quote(100, 0, 0, 7, 0, 0, 0))];
        let mut zero_bounty_storage = [0u8; MANIFEST_BYTES_1];
        let zero_bounty_manifest =
            CapabilityManifestV1::encode_into(&entries, &mut zero_bounty_storage)
                .expect("zero bounty manifest");
        let selected = zero_bounty_manifest
            .required_founding_entry_for_config(id(31))
            .expect("selected");
        assert_eq!(
            construct_required_resolution_funding(id(99), zero_bounty_manifest, selected, 100, 44,),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::MissingResolutionFundBounty,
            })
        );
    }

    #[test]
    fn selection_from_another_manifest_cannot_bypass_uniqueness() {
        let first_entries = [entry(11, 31, quote(100, 0, 0, 7, 11, 0, 0))];
        let mut first_storage = [0u8; MANIFEST_BYTES_1];
        let first = CapabilityManifestV1::encode_into(&first_entries, &mut first_storage)
            .expect("first manifest");
        let selected = first
            .required_founding_entry_for_config(id(31))
            .expect("first selection");

        let ambiguous_entries = [
            entry(11, 31, quote(100, 0, 0, 7, 11, 0, 0)),
            entry(12, 31, quote(100, 0, 0, 7, 11, 0, 0)),
        ];
        let mut ambiguous_storage = [0u8; MANIFEST_BYTES_2];
        let ambiguous =
            CapabilityManifestV1::encode_into(&ambiguous_entries, &mut ambiguous_storage)
                .expect("ambiguous manifest");
        assert_eq!(
            construct_required_resolution_funding(id(99), ambiguous, selected, 100, 44),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::RequiredFoundingConfigAmbiguous,
            })
        );
    }

    #[test]
    fn one_shot_release_uses_only_the_returned_canonical_ledger() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = funding(&mut storage, 100);
        let mut state = funding;
        assert_eq!(
            state.release(id(99), manifest, 18, FundingCompartment::Provider, 7,),
            Ok(())
        );
        assert_eq!(state.remaining().total_principal(), 11);
        assert_eq!(
            state.release(id(99), manifest, 11, FundingCompartment::Bounty, 11,),
            Ok(())
        );
        assert_eq!(state.validate_against(id(99), manifest, 0), Ok(()));
        assert_eq!(state.remaining().total_principal(), 0);
        assert_eq!(state.released().rent_principal(), 100);
        assert_eq!(state.released().provider_principal(), 7);
        assert_eq!(state.released().bounty_principal(), 11);
        assert_eq!(
            state.release(id(99), manifest, 0, FundingCompartment::Bounty, 1,),
            Err(CapabilityError::InsufficientCompartmentPrincipal)
        );
    }

    #[test]
    fn arithmetic_boundaries_refuse() {
        assert_eq!(
            FundingQuoteV1::new(u64::MAX, 0, 0, 0, 1, 0, 0),
            Err(CapabilityError::ArithmeticOverflow)
        );

        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (_, funding) = funding(&mut storage, 100);
        let mut changed = funding.to_bytes();
        put_u64(&mut changed, STATE_RELEASED_RENT, u64::MAX);
        put_u64(&mut changed, STATE_RELEASED_TOTAL, u64::MAX);
        let overflowing = FundingStateV1::decode(&changed).expect("structural state");
        assert_eq!(
            required_resolution_minimum_balance(overflowing),
            Err(Error::ArithmeticOverflow)
        );
    }
}
