//! Canonical prepaid Pyth-resolution funding layout.

use dclutch_capability_contract::{
    CapabilityManifestV1, ContentId, FUNDING_STATE_BYTES, FundingAmountsV1, FundingStateV1,
    FundingStatus, RequiredFoundingEntryV1,
};

use crate::{Error, Result, array, nonzero, zero};

/// Exact byte width of [`ResolutionFundV1`].
pub const FUNDING_BYTES: usize = 88 + FUNDING_STATE_BYTES;
/// Funding-account magic.
pub const FUNDING_MAGIC: [u8; 8] = *b"DCLTFND1";
/// Implemented funding schema.
pub const FUNDING_SCHEMA_VERSION: u16 = 1;

const FUNDING_STATE_OFFSET: usize = 88;

/// Immutable identity plus the canonical activated capability-funding ledger.
///
/// Provider reimbursement and success bounty are not duplicated here. They
/// remain compartments of `funding_state`, whose manifest entry is their sole
/// immutable authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionFundV1 {
    market: [u8; 32],
    generation: u64,
    sponsor_refund: [u8; 32],
    funding_state: FundingStateV1,
}

impl ResolutionFundV1 {
    /// Construct the one-shot Fund from an authenticated manifest selection.
    ///
    /// The adapter authenticates `manifest_content_id` as the content hash of
    /// `manifest`, computes `exact_fund_rent` from [`FUNDING_BYTES`] and the
    /// current Rent sysvar, and creates/funds the physical account atomically.
    /// This method first models exactly prepaid pending funding, then models
    /// required-at-founding activation. Rent is released into account rent;
    /// provider reimbursement and positive bounty remain the only held
    /// non-rent principal.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        market: [u8; 32],
        generation: u64,
        sponsor_refund: [u8; 32],
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        selected: RequiredFoundingEntryV1,
        exact_fund_rent: u64,
        current_slot: u64,
    ) -> Result<Self> {
        if !nonzero(&market) || !nonzero(&sponsor_refund) {
            return Err(Error::ZeroIdentifier);
        }
        let canonical = manifest
            .required_founding_entry_for_config(selected.entry().config_id())
            .map_err(capability_error)?;
        if canonical != selected {
            return Err(Error::FundingSelectionMismatch);
        }
        let quote = selected
            .validate_one_shot_resolution_fund_quote(exact_fund_rent)
            .map_err(capability_error)?;
        let mut funding_state = FundingStateV1::new(
            manifest_content_id,
            manifest,
            selected.index(),
            quote.total_principal(),
        )
        .map_err(capability_error)?;
        let debit = funding_state
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
        let result = Self {
            market,
            generation,
            sponsor_refund,
            funding_state,
        };
        result.validate_against(
            manifest_content_id,
            manifest,
            exact_fund_rent,
            funding_state.remaining().total_principal(),
        )?;
        Ok(result)
    }

    /// Decode the exact canonical funding layout.
    ///
    /// Decoding validates canonical bytes and the specialized local shape.
    /// Call [`Self::validate_against`] before use to authenticate the manifest
    /// binding and exact immutable quote.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FUNDING_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != FUNDING_MAGIC {
            return Err(Error::InvalidMagic);
        }
        if u16::from_le_bytes(array::<2>(bytes, 8)?) != FUNDING_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema);
        }
        if !zero(bytes.get(10..16).ok_or(Error::InvalidLength)?) {
            return Err(Error::NonCanonicalReservedBytes);
        }
        let funding_bytes = bytes
            .get(FUNDING_STATE_OFFSET..FUNDING_BYTES)
            .ok_or(Error::InvalidLength)?;
        let result = Self {
            market: array(bytes, 16)?,
            generation: u64::from_le_bytes(array(bytes, 48)?),
            sponsor_refund: array(bytes, 56)?,
            funding_state: FundingStateV1::decode(funding_bytes).map_err(capability_error)?,
        };
        if !nonzero(&result.market) || !nonzero(&result.sponsor_refund) {
            return Err(Error::ZeroIdentifier);
        }
        result.validate_local_shape()?;
        Ok(result)
    }

    /// Encode this value into its exact canonical fixed-width bytes.
    pub fn to_bytes(self) -> [u8; FUNDING_BYTES] {
        let mut out = [0; FUNDING_BYTES];
        out[..8].copy_from_slice(&FUNDING_MAGIC);
        out[8..10].copy_from_slice(&FUNDING_SCHEMA_VERSION.to_le_bytes());
        out[16..48].copy_from_slice(&self.market);
        out[48..56].copy_from_slice(&self.generation.to_le_bytes());
        out[56..88].copy_from_slice(&self.sponsor_refund);
        out[FUNDING_STATE_OFFSET..FUNDING_BYTES].copy_from_slice(&self.funding_state.to_bytes());
        out
    }

    /// Encode into an exact-width caller buffer without changing it on refusal.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != FUNDING_BYTES {
            return Err(Error::OutputLength);
        }
        output.copy_from_slice(&self.to_bytes());
        Ok(())
    }

    /// Return the Market identifier.
    pub const fn market(&self) -> &[u8; 32] {
        &self.market
    }
    /// Return the immutable Market generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Return the immutable recipient of any sponsor refund excess.
    pub const fn sponsor_refund(&self) -> &[u8; 32] {
        &self.sponsor_refund
    }
    /// Return the sole canonical funding ledger embedded by value.
    pub const fn funding_state(&self) -> FundingStateV1 {
        self.funding_state
    }
    /// Return exact non-rent principal still held for provider and bounty.
    pub const fn remaining(&self) -> FundingAmountsV1 {
        self.funding_state.remaining()
    }

    /// Validate manifest authority, unique selection, exact rent, specialized
    /// compartments, conservation, activation, and observed held principal.
    pub fn validate_against(
        &self,
        manifest_content_id: ContentId,
        manifest: CapabilityManifestV1<'_>,
        exact_fund_rent: u64,
        observed_non_rent_principal: u64,
    ) -> Result<()> {
        self.validate_local_shape()?;
        let entry = manifest
            .entry(self.funding_state.entry_index())
            .map_err(capability_error)?;
        let selected = manifest
            .required_founding_entry_for_config(entry.config_id())
            .map_err(capability_error)?;
        if selected.index() != self.funding_state.entry_index() || selected.entry() != entry {
            return Err(Error::FundingSelectionMismatch);
        }
        let quote = selected
            .validate_one_shot_resolution_fund_quote(exact_fund_rent)
            .map_err(capability_error)?;
        self.funding_state
            .validate_against(manifest_content_id, manifest, observed_non_rent_principal)
            .map_err(capability_error)?;
        let remaining = self.funding_state.remaining();
        let released = self.funding_state.released();
        if remaining.provider_principal() != quote.provider_principal()
            || remaining.bounty_principal() != quote.bounty_principal()
            || released.rent_principal() != exact_fund_rent
        {
            return Err(Error::InvalidResolutionFundShape);
        }
        Ok(())
    }

    /// Return the exact physical minimum committed by the activated funding
    /// state: released account rent plus still-held non-rent principal.
    pub fn minimum_balance(&self) -> Result<u64> {
        self.funding_state
            .released()
            .rent_principal()
            .checked_add(self.funding_state.remaining().total_principal())
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Classify a balance without inventing a second funding quote. Any
    /// physical excess remains bound to the immutable sponsor-refund recipient.
    pub fn classify_balance(&self, actual: u64) -> Result<BalanceClassification> {
        let minimum = self.minimum_balance()?;
        let sponsor_refund_excess = actual.checked_sub(minimum).ok_or(Error::Underfunded)?;
        Ok(BalanceClassification {
            sponsor_refund_recipient: self.sponsor_refund,
            minimum,
            sponsor_refund_excess,
        })
    }

    fn validate_local_shape(&self) -> Result<()> {
        let remaining = self.funding_state.remaining();
        let released = self.funding_state.released();
        if self.funding_state.status() != FundingStatus::Active
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
}

fn capability_error(error: dclutch_capability_contract::Error) -> Error {
    Error::InvalidCapabilityFunding { error }
}

/// Exact funding classification, including the only recipient of excess.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BalanceClassification {
    sponsor_refund_recipient: [u8; 32],
    minimum: u64,
    sponsor_refund_excess: u64,
}

impl BalanceClassification {
    /// Return the immutable sponsor-refund recipient for the classified excess.
    pub const fn sponsor_refund_recipient(&self) -> &[u8; 32] {
        &self.sponsor_refund_recipient
    }
    /// Return the exact required rent, reimbursement, and bounty minimum.
    pub const fn minimum(&self) -> u64 {
        self.minimum
    }
    /// Return the exact excess payable only to the refund recipient.
    pub const fn sponsor_refund_excess(&self) -> u64 {
        self.sponsor_refund_excess
    }
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
    const STATE_STATUS_IN_FUND: usize = FUNDING_STATE_OFFSET + 10;
    const STATE_ENTRY_INDEX_IN_FUND: usize = FUNDING_STATE_OFFSET + 48;
    const STATE_REMAINING_PROVIDER_IN_FUND: usize = FUNDING_STATE_OFFSET + 64 + 24;
    const STATE_REMAINING_TOTAL_IN_FUND: usize = FUNDING_STATE_OFFSET + 64 + 56;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn content_id(value: u8) -> ContentId {
        ContentId::new(id(value)).expect("nonzero content id")
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
            content_id(kind),
            content_id(21),
            content_id(config),
            content_id(23),
            content_id(24),
            content_id(25),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            funding_quote,
        )
        .expect("valid entry")
    }

    fn fund<'a>(
        storage: &'a mut [u8; MANIFEST_BYTES_1],
        rent: u64,
    ) -> (CapabilityManifestV1<'a>, ResolutionFundV1) {
        let entries = [entry(11, 31, quote(rent, 0, 0, 7, 11, 0, 0))];
        let manifest = CapabilityManifestV1::encode_into(&entries, storage).expect("manifest");
        let selected = manifest
            .required_founding_entry_for_config(content_id(31))
            .expect("unique founding entry");
        let result = ResolutionFundV1::new(
            id(1),
            9,
            id(2),
            content_id(99),
            manifest,
            selected,
            rent,
            44,
        )
        .expect("resolution fund");
        (manifest, result)
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        let destination = bytes.get_mut(offset..offset + 8).expect("test offset");
        destination.copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn construction_activates_canonical_funding_and_round_trips() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = fund(&mut storage, 100);
        assert_eq!(FUNDING_BYTES, 280);
        assert_eq!(funding.market(), &id(1));
        assert_eq!(funding.generation(), 9);
        assert_eq!(funding.sponsor_refund(), &id(2));
        assert_eq!(funding.funding_state().status(), FundingStatus::Active);
        assert_eq!(funding.funding_state().activation_slot(), 44);
        assert_eq!(
            funding.funding_state().manifest_content_id(),
            content_id(99)
        );
        assert_eq!(funding.funding_state().entry_index(), 0);
        assert_eq!(funding.remaining().provider_principal(), 7);
        assert_eq!(funding.remaining().bounty_principal(), 11);
        assert_eq!(funding.remaining().total_principal(), 18);
        assert_eq!(funding.funding_state().released().rent_principal(), 100);
        assert_eq!(funding.minimum_balance(), Ok(118));
        assert_eq!(
            funding.validate_against(content_id(99), manifest, 100, 18),
            Ok(())
        );
        assert_eq!(ResolutionFundV1::decode(&funding.to_bytes()), Ok(funding));

        let classified = funding.classify_balance(125).expect("funded");
        assert_eq!(classified.sponsor_refund_recipient(), &id(2));
        assert_eq!(classified.minimum(), 118);
        assert_eq!(classified.sponsor_refund_excess(), 7);
        assert_eq!(funding.classify_balance(117), Err(Error::Underfunded));
    }

    #[test]
    fn one_shot_release_uses_only_the_embedded_canonical_ledger() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = fund(&mut storage, 100);
        let mut state = funding.funding_state();
        assert_eq!(
            state.release(
                content_id(99),
                manifest,
                18,
                FundingCompartment::Provider,
                7,
            ),
            Ok(())
        );
        assert_eq!(state.remaining().total_principal(), 11);
        assert_eq!(
            state.release(content_id(99), manifest, 11, FundingCompartment::Bounty, 11,),
            Ok(())
        );
        assert_eq!(state.validate_against(content_id(99), manifest, 0), Ok(()));
        assert_eq!(state.remaining().total_principal(), 0);
        assert_eq!(state.released().rent_principal(), 100);
        assert_eq!(state.released().provider_principal(), 7);
        assert_eq!(state.released().bounty_principal(), 11);
        assert_eq!(
            state.release(content_id(99), manifest, 0, FundingCompartment::Bounty, 1,),
            Err(CapabilityError::InsufficientCompartmentPrincipal)
        );
    }

    #[test]
    fn hostile_exact_layouts_refuse_without_output_mutation() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (_, funding) = fund(&mut storage, 100);
        let bytes = funding.to_bytes();
        for length in 0..FUNDING_BYTES {
            assert_eq!(
                ResolutionFundV1::decode(bytes.get(..length).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut trailing = [0u8; FUNDING_BYTES + 1];
        trailing[..FUNDING_BYTES].copy_from_slice(&bytes);
        assert_eq!(
            ResolutionFundV1::decode(&trailing),
            Err(Error::InvalidLength)
        );

        let mut changed = bytes;
        changed[0] = 0;
        assert_eq!(ResolutionFundV1::decode(&changed), Err(Error::InvalidMagic));
        let mut changed = bytes;
        changed[8] = 2;
        assert_eq!(
            ResolutionFundV1::decode(&changed),
            Err(Error::UnsupportedSchema)
        );
        let mut changed = bytes;
        changed[10] = 1;
        assert_eq!(
            ResolutionFundV1::decode(&changed),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut output = [0xa5; FUNDING_BYTES - 1];
        assert_eq!(funding.encode(&mut output), Err(Error::OutputLength));
        assert_eq!(output, [0xa5; FUNDING_BYTES - 1]);
    }

    #[test]
    fn wrong_manifest_index_status_and_conservation_refuse() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = fund(&mut storage, 100);
        assert_eq!(
            funding.validate_against(content_id(98), manifest, 100, 18),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::FundingBindingMismatch,
            })
        );

        let wrong_entries = [entry(11, 31, quote(100, 0, 0, 8, 11, 0, 0))];
        let mut wrong_storage = [0u8; MANIFEST_BYTES_1];
        let wrong_manifest = CapabilityManifestV1::encode_into(&wrong_entries, &mut wrong_storage)
            .expect("wrong manifest");
        assert_eq!(
            funding.validate_against(content_id(99), wrong_manifest, 100, 18),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::FundingConservationMismatch,
            })
        );

        let mut changed = funding.to_bytes();
        changed[STATE_ENTRY_INDEX_IN_FUND..STATE_ENTRY_INDEX_IN_FUND + 2]
            .copy_from_slice(&1u16.to_le_bytes());
        let wrong_index = ResolutionFundV1::decode(&changed).expect("structural state");
        assert_eq!(
            wrong_index.validate_against(content_id(99), manifest, 100, 18),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::InvalidDependency,
            })
        );

        let mut changed = funding.to_bytes();
        changed[STATE_STATUS_IN_FUND] = 0;
        assert_eq!(
            ResolutionFundV1::decode(&changed),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::InvalidFundingStatus,
            })
        );

        let mut changed = funding.to_bytes();
        put_u64(&mut changed, STATE_REMAINING_PROVIDER_IN_FUND, 8);
        put_u64(&mut changed, STATE_REMAINING_TOTAL_IN_FUND, 19);
        let unconserved = ResolutionFundV1::decode(&changed).expect("structural state");
        assert_eq!(
            unconserved.validate_against(content_id(99), manifest, 100, 19),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::FundingConservationMismatch,
            })
        );
        assert_eq!(
            funding.validate_against(content_id(99), manifest, 100, 19),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::PresentPrincipalMismatch,
            })
        );
    }

    #[test]
    fn rent_and_extra_compartments_are_manifest_refusals() {
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let (manifest, funding) = fund(&mut storage, 100);
        assert_eq!(
            funding.validate_against(content_id(99), manifest, 99, 18),
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
                .required_founding_entry_for_config(content_id(31))
                .expect("selected");
            assert_eq!(
                ResolutionFundV1::new(
                    id(1),
                    9,
                    id(2),
                    content_id(99),
                    extra_manifest,
                    selected,
                    100,
                    44,
                ),
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
            .required_founding_entry_for_config(content_id(31))
            .expect("selected");
        assert_eq!(
            ResolutionFundV1::new(
                id(1),
                9,
                id(2),
                content_id(99),
                zero_bounty_manifest,
                selected,
                100,
                44,
            ),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::MissingResolutionFundBounty,
            })
        );
    }

    #[test]
    fn a_selection_from_another_manifest_cannot_authorize_funding() {
        let first_entries = [entry(11, 31, quote(100, 0, 0, 7, 11, 0, 0))];
        let mut first_storage = [0u8; MANIFEST_BYTES_1];
        let first = CapabilityManifestV1::encode_into(&first_entries, &mut first_storage)
            .expect("first manifest");
        let selected = first
            .required_founding_entry_for_config(content_id(31))
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
            ResolutionFundV1::new(
                id(1),
                9,
                id(2),
                content_id(99),
                ambiguous,
                selected,
                100,
                44,
            ),
            Err(Error::InvalidCapabilityFunding {
                error: CapabilityError::RequiredFoundingConfigAmbiguous,
            })
        );
    }

    #[test]
    fn identity_and_arithmetic_boundaries_refuse() {
        assert_eq!(
            FundingQuoteV1::new(u64::MAX, 0, 0, 0, 1, 0, 0),
            Err(CapabilityError::ArithmeticOverflow)
        );

        let entries = [entry(11, 31, quote(100, 0, 0, 7, 11, 0, 0))];
        let mut storage = [0u8; MANIFEST_BYTES_1];
        let manifest = CapabilityManifestV1::encode_into(&entries, &mut storage).expect("manifest");
        let selected = manifest
            .required_founding_entry_for_config(content_id(31))
            .expect("selected");
        assert_eq!(
            ResolutionFundV1::new(
                [0; 32],
                9,
                id(2),
                content_id(99),
                manifest,
                selected,
                100,
                44,
            ),
            Err(Error::ZeroIdentifier)
        );
        assert_eq!(
            ResolutionFundV1::new(
                id(1),
                9,
                [0; 32],
                content_id(99),
                manifest,
                selected,
                100,
                44,
            ),
            Err(Error::ZeroIdentifier)
        );
    }
}
