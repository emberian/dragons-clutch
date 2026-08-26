//! Solana/Core projection for the SDK-free Series V2 kernel.
//!
//! This is a deliberately small runtime boundary: immutable decoding,
//! content IDs, proof verification, schedule, funding-list identity, and the
//! future-Market seed projection remain owned by `dclutch-series-v2-kernel`.

use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Identity as CoreIdentity, SeriesCoreActionV1, SeriesCoreRequestV1,
};
use dclutch_series_v2_kernel::{
    AccountKeyV2, AdmittedOccurrenceV2, OccurrenceV2, SeriesV2Error, TicketV2,
    funding_list_id as kernel_funding_list_id, future_market_projection,
};
use solana_program::pubkey::Pubkey;

const PHYSICAL_MAXIMUM_FUNDING_STATES_V2: usize = 16;

/// Require the committed future Market to equal its PDA under current Core.
pub fn require_market_pda(
    admitted: AdmittedOccurrenceV2,
    core_program: &Pubkey,
    registry_program: &Pubkey,
) -> Result<(), SeriesV2Error> {
    if *core_program == Pubkey::default() || *registry_program == Pubkey::default() {
        return Err(SeriesV2Error::Market);
    }
    let registry = account_key(*registry_program).map_err(|_| SeriesV2Error::Market)?;
    let projection = future_market_projection(admitted, registry)?;
    let expected = Pubkey::find_program_address(&projection.seeds().as_slices(), core_program).0;
    projection.require_address(account_key(expected)?)
}

/// Hash the exact ordered FundingState accounts at the current physical frame.
///
/// Sixteen is a provisional SBF account-frame profile, not a Series ontology
/// bound. The shared kernel accepts every nonempty alias-free list fitting
/// `u16`; a future physical profile can lift this adapter constant alone.
pub fn funding_list_id(funding_states: &[Pubkey]) -> Result<ContentId, SeriesV2Error> {
    if funding_states.is_empty() || funding_states.len() > PHYSICAL_MAXIMUM_FUNDING_STATES_V2 {
        return Err(SeriesV2Error::Funding);
    }
    let placeholder = AccountKeyV2::new([1; 32]).map_err(|_| SeriesV2Error::Funding)?;
    let mut converted = [placeholder; PHYSICAL_MAXIMUM_FUNDING_STATES_V2];
    for (index, key) in funding_states.iter().copied().enumerate() {
        *converted.get_mut(index).ok_or(SeriesV2Error::Funding)? =
            account_key(key).map_err(|_| SeriesV2Error::Funding)?;
    }
    kernel_funding_list_id(
        converted
            .get(..funding_states.len())
            .ok_or(SeriesV2Error::Funding)?,
    )
}

/// Require actual ordered FundingState accounts to match an occurrence.
pub fn require_funding_list(
    occurrence: OccurrenceV2,
    funding_states: &[Pubkey],
) -> Result<(), SeriesV2Error> {
    if funding_list_id(funding_states)? != occurrence.funding_list() {
        return Err(SeriesV2Error::Funding);
    }
    Ok(())
}

/// Project one admitted Consume onto the canonical Series-to-Core request.
#[allow(clippy::too_many_arguments)]
pub fn core_request(
    admitted: AdmittedOccurrenceV2,
    action: SeriesCoreActionV1,
    ticket: TicketV2,
    ticket_account: Pubkey,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
) -> Result<SeriesCoreRequestV1, SeriesV2Error> {
    if action != SeriesCoreActionV1::Consume || ticket_account == Pubkey::default() {
        return Err(SeriesV2Error::Action);
    }
    admitted.require_ticket(ticket)?;
    let template = admitted.template();
    let occurrence = admitted.occurrence();
    let funds = occurrence.funds();
    SeriesCoreRequestV1::occurrence(
        action,
        core_content_identity(template.release_set())?,
        core_content_identity(admitted.template_id())?,
        core_account_identity(account_key(ticket_account)?)?,
        core_account_identity(occurrence.market())?,
        core_content_identity(template.realm())?,
        core_content_identity(occurrence.product())?,
        core_account_identity(ticket.refund_owner())?,
        core_account_identity(ticket.founder())?,
        occurrence.occurrence(),
        expected_series_revision,
        expected_ticket_revision,
        funds.market_rent(),
        funds.capability_native(),
        funds.founding_work(),
        funds.hoard_principal(),
    )
    .map_err(|_| SeriesV2Error::Commitment)
}

/// Convert one SDK-free account identity at the explicit adapter boundary.
pub(crate) fn pubkey(value: AccountKeyV2) -> Pubkey {
    Pubkey::new_from_array(value.to_bytes())
}

fn account_key(value: Pubkey) -> Result<AccountKeyV2, SeriesV2Error> {
    AccountKeyV2::new(value.to_bytes())
}

pub(crate) fn core_identity(value: ContentId) -> Result<CoreIdentity, SeriesV2Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV2Error::Identity)
}

#[cfg(test)]
pub(crate) fn core_pubkey_identity(value: Pubkey) -> Result<CoreIdentity, SeriesV2Error> {
    core_account_identity(account_key(value)?)
}

fn core_content_identity(value: ContentId) -> Result<CoreIdentity, SeriesV2Error> {
    core_identity(value)
}

fn core_account_identity(value: AccountKeyV2) -> Result<CoreIdentity, SeriesV2Error> {
    CoreIdentity::new(value.to_bytes()).map_err(|_| SeriesV2Error::Identity)
}
