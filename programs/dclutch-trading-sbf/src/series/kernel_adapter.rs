//! Solana/Core projection for the SDK-free Series V3 kernel.
//!
//! This is a deliberately small runtime boundary: immutable decoding,
//! content IDs, proof verification, schedule, funding-list identity, and the
//! future-Market seed projection remain owned by `dclutch-series-v3-kernel`.

use dclutch_core_contract::ContentId;
#[cfg(test)]
use dclutch_market_core_codec::Identity as CoreIdentity;
use dclutch_market_core_codec::{SeriesCoreActionV1, SeriesCoreRequestV1};
use dclutch_series_v3_kernel::{
    AccountKeyV3, AdmittedOccurrenceV3, AdmittedTicketV3, AuthenticatedProductProjectionV2,
    OccurrenceV3, SeriesV3Error, funding_list_id as kernel_funding_list_id,
    future_market_projection, series_core_consume_request,
};
use solana_program::pubkey::Pubkey;

const PHYSICAL_MAXIMUM_FUNDING_STATES_V3: usize = 16;

/// Require the committed future Market to equal its PDA under current Core.
pub fn require_market_pda(
    admitted: AdmittedOccurrenceV3,
    product: AuthenticatedProductProjectionV2,
    core_program: &Pubkey,
    registry_program: &Pubkey,
) -> Result<(), SeriesV3Error> {
    if *core_program == Pubkey::default() || *registry_program == Pubkey::default() {
        return Err(SeriesV3Error::Market);
    }
    let registry = account_key(*registry_program).map_err(|_| SeriesV3Error::Market)?;
    let projection = future_market_projection(admitted, product, registry)?;
    let expected = Pubkey::find_program_address(&projection.seeds().as_slices(), core_program).0;
    projection.require_address(account_key(expected)?)
}

/// Hash the exact ordered FundingState accounts at the current physical frame.
///
/// Sixteen is a provisional SBF account-frame profile, not a Series ontology
/// bound. The shared kernel accepts every nonempty alias-free list fitting
/// `u16`; a future physical profile can lift this adapter constant alone.
pub fn funding_list_id(funding_states: &[Pubkey]) -> Result<ContentId, SeriesV3Error> {
    if funding_states.is_empty() || funding_states.len() > PHYSICAL_MAXIMUM_FUNDING_STATES_V3 {
        return Err(SeriesV3Error::Funding);
    }
    let placeholder = AccountKeyV3::new([1; 32]).map_err(|_| SeriesV3Error::Funding)?;
    let mut converted = [placeholder; PHYSICAL_MAXIMUM_FUNDING_STATES_V3];
    for (index, key) in funding_states.iter().copied().enumerate() {
        *converted.get_mut(index).ok_or(SeriesV3Error::Funding)? =
            account_key(key).map_err(|_| SeriesV3Error::Funding)?;
    }
    kernel_funding_list_id(
        converted
            .get(..funding_states.len())
            .ok_or(SeriesV3Error::Funding)?,
    )
}

/// Require actual ordered FundingState accounts to match an occurrence.
pub fn require_funding_list(
    occurrence: OccurrenceV3,
    funding_states: &[Pubkey],
) -> Result<(), SeriesV3Error> {
    if funding_list_id(funding_states)? != occurrence.funding_list() {
        return Err(SeriesV3Error::Funding);
    }
    Ok(())
}

/// Project one admitted Consume onto the canonical Series-to-Core request.
#[allow(clippy::too_many_arguments)]
pub fn core_request(
    admitted: AdmittedOccurrenceV3,
    product: AuthenticatedProductProjectionV2,
    action: SeriesCoreActionV1,
    ticket: AdmittedTicketV3,
    ticket_account: Pubkey,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
) -> Result<SeriesCoreRequestV1, SeriesV3Error> {
    if action != SeriesCoreActionV1::Consume || ticket_account == Pubkey::default() {
        return Err(SeriesV3Error::Action);
    }
    series_core_consume_request(
        admitted,
        ticket,
        product,
        account_key(ticket_account)?,
        expected_series_revision,
        expected_ticket_revision,
    )
}

/// Convert one SDK-free account identity at the explicit adapter boundary.
pub(crate) fn pubkey(value: AccountKeyV3) -> Pubkey {
    Pubkey::new_from_array(value.to_bytes())
}

fn account_key(value: Pubkey) -> Result<AccountKeyV3, SeriesV3Error> {
    AccountKeyV3::new(value.to_bytes())
}

#[cfg(test)]
pub(crate) fn core_identity(value: ContentId) -> Result<CoreIdentity, SeriesV3Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV3Error::Identity)
}

#[cfg(test)]
pub(crate) fn core_pubkey_identity(value: Pubkey) -> Result<CoreIdentity, SeriesV3Error> {
    core_account_identity(account_key(value)?)
}

#[cfg(test)]
fn core_account_identity(value: AccountKeyV3) -> Result<CoreIdentity, SeriesV3Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV3Error::Identity)
}
