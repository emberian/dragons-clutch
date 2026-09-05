//! Trading-authorized precommit refund for one unallocated Series permit.
//!
//! Ordinary permissionless expiry authenticates an already-committed Expired
//! replay state. This narrow sibling exists for the atomic Series controller:
//! it authenticates the Prepared prestate, reconstructs the exact Expire
//! request and replay candidates from finalized Series records, and requires a
//! release-pinned Trading caller PDA over that canonical request. Core may then
//! refund the still-unallocated permit before Trading persists the candidates;
//! any later Trading failure rolls the CPI and refund back with the transaction.

use dclutch_market::SeriesUnallocatedPermitExpiryRequestV1;
use dclutch_market::capability_program::{CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::series::{
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, admit_occurrence_bytes, admit_ticket,
    plan::{ReplayCandidateV3, SeriesReplayActionV3, evaluate_replay_v3},
    replay::{
        SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesStateV3, TicketPhaseV3,
        TicketStateSeedsV3, TicketStateV3,
    },
    ticket_admission_v1::SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1,
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, hash::hash, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;

use crate::{
    CoreSbfError,
    frame::require_distinct,
    infrastructure::authenticate_profile,
    release::{authenticate_role, identity},
    series_permit_expiry::{
        ExpiryAccounts, SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1,
        authenticate_record_derived_unallocated_permit, finalized_series_record,
        refund_record_derived, require_expired,
    },
};

/// Exact account count: the ordinary 25-account frame plus caller authority.
pub const SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1: usize =
    SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1 + 1;

/// Authenticate a unique Expire candidate, refund, and leave replay unwritten.
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SeriesUnallocatedPermitExpiryRequestV1,
    request_bytes: &[u8],
    proof_bytes: &[u8],
) -> Result<(), ProgramError> {
    if accounts.len() != SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1 {
        return Err(CoreSbfError::AccountFrame.into());
    }
    require_distinct(accounts)?;
    let base = accounts
        .get(..SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1)
        .ok_or(CoreSbfError::AccountFrame)?;
    let frame = ExpiryAccounts::parse(base)?;
    let caller = accounts
        .get(SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1)
        .ok_or(CoreSbfError::AccountFrame)?;
    if !caller.is_signer
        || caller.is_writable
        || caller.executable
        || caller.owner != &system_program::ID
        || !caller.data_is_empty()
    {
        return Err(CoreSbfError::AccountFrame.into());
    }

    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Creation)?;
    let profile = authenticate_profile(
        program_id,
        frame.infrastructure_profile,
        frame.registry_artifact_raw,
        frame.registry_artifact_staging,
        frame.registry_program,
        frame.registry_programdata,
        frame.rent_artifact_raw,
        frame.rent_artifact_staging,
        frame.rent_program,
        frame.rent_programdata,
    )?;
    let template_bytes = finalized_series_record(
        &frame,
        frame.template_raw,
        frame.template_staging,
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    )?;
    let template_record = hash(&template_bytes).to_bytes();
    let occurrence_bytes = finalized_series_record(
        &frame,
        frame.occurrence_raw,
        frame.occurrence_staging,
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    )?;
    let ticket_bytes = finalized_series_record(
        &frame,
        frame.ticket_raw,
        frame.ticket_staging,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    )?;
    let admitted = admit_occurrence_bytes(&template_bytes, &occurrence_bytes, proof_bytes)
        .map_err(|_| CoreSbfError::Reference)?;
    let admitted_ticket = admit_ticket(&ticket_bytes).map_err(|_| CoreSbfError::Reference)?;
    admitted
        .require_ticket(admitted_ticket.ticket())
        .map_err(|_| CoreSbfError::Reference)?;
    authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.trading_program,
        frame.trading_programdata,
        identity(profile.registry().program().to_bytes())?,
        admitted.template().release_set().to_bytes(),
        dclutch_market::Role::Trading,
    )?;

    let (series_prestate, ticket_prestate, controller_market) = authenticate_prestate(
        &frame,
        admitted,
        admitted_ticket,
        request.expected_series_revision(),
        request.expected_ticket_revision(),
        template_record,
    )?;
    let (root_candidate, ticket_candidate) = recompute_candidates(
        admitted,
        admitted_ticket,
        series_prestate,
        ticket_prestate,
        request.expected_series_revision(),
        request.expected_ticket_revision(),
    )?;
    authenticate_caller(
        &frame,
        caller,
        admitted,
        admitted_ticket,
        controller_market,
        request_bytes,
    )?;

    let retry_through = admitted
        .template()
        .retry_through(admitted.occurrence().occurrence())
        .map_err(|_| CoreSbfError::Reference)?;
    let clock = Clock::from_account_info(frame.clock).map_err(|_| CoreSbfError::Creation)?;
    require_expired(clock.slot, retry_through, retry_through)?;
    let occurrence = admitted.occurrence();
    let template = admitted.template();
    let ticket_context = admitted_ticket.content_id().to_bytes();
    let generation = u64::from(occurrence.occurrence())
        .checked_add(1)
        .ok_or(CoreSbfError::Arithmetic)?;
    let bump = authenticate_record_derived_unallocated_permit(
        program_id,
        &frame,
        template.release_set().to_bytes(),
        occurrence.market().to_bytes(),
        generation,
        ticket_context,
        admitted_ticket.ticket().refund_owner().to_bytes(),
    )?;

    // Both candidates were completely reconstructed and hostile-decoded before
    // the first lamport mutation. Core intentionally returns no data; Trading
    // commits these same bytes only after observing that empty return channel.
    let _authenticated_candidates = (root_candidate, ticket_candidate);
    refund_record_derived(
        &frame,
        template.release_set().to_bytes(),
        occurrence.market().to_bytes(),
        ticket_context,
        bump,
        &rent,
    )?;
    Ok(())
}

fn authenticate_prestate(
    frame: &ExpiryAccounts<'_, '_>,
    admitted: dclutch_trading::series::AdmittedOccurrenceV3,
    admitted_ticket: dclutch_trading::series::AdmittedTicketV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
    template_record: [u8; 32],
) -> Result<(SeriesStateV3, TicketStateV3, [u8; 32]), CoreSbfError> {
    if frame.root.owner != frame.trading_program.key
        || frame.root.data_len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
        || frame.ticket_state.owner != frame.trading_program.key
        || frame.ticket_state.data_len() != SERIES_TICKET_STATE_BYTES_V3
    {
        return Err(CoreSbfError::Reference);
    }
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(CoreSbfError::Reference)?,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let series = SeriesStateV3::decode(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(CoreSbfError::Reference)?,
        admitted.template().occurrence_count(),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let expected_root =
        Pubkey::find_program_address(&header.seeds().as_slices(), frame.trading_program.key).0;
    // Core deliberately does not reinterpret the controller Market account:
    // Trading has already authenticated that live Market before minting the
    // caller PDA. Core independently binds the same persistent root through
    // its PDA, release, and Template selection, and requires the occurrence's
    // record-derived future Market to be a distinct child coordinate.
    if frame.root.key != &expected_root
        || header.release_set().to_bytes() != admitted.template().release_set().to_bytes()
        || !controller_and_future_markets_are_distinct(
            header.market(),
            admitted.occurrence().market().to_bytes(),
        )
        || header.generation() == 0
        || header.selection().config().to_bytes() != template_record
        || series.next_occurrence() != admitted.occurrence().occurrence()
        || !series.current_ticket_prepared()
        || series.revision() != expected_series_revision
    {
        return Err(CoreSbfError::Reference);
    }

    let ticket_data = frame
        .ticket_state
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket = TicketStateV3::decode(&ticket_data).map_err(|_| CoreSbfError::Reference)?;
    let seeds = TicketStateSeedsV3::new(frame.root.key.to_bytes(), admitted_ticket.content_id());
    let expected_ticket =
        Pubkey::find_program_address(&seeds.as_slices(), frame.trading_program.key).0;
    if frame.ticket_state.key != &expected_ticket
        || ticket.ticket_record_id() != admitted_ticket.content_id()
        || !SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1.admits(ticket.phase())
        || ticket.revision() != expected_ticket_revision
    {
        return Err(CoreSbfError::Reference);
    }
    Ok((series, ticket, header.market()))
}

fn controller_and_future_markets_are_distinct(
    controller_market: [u8; 32],
    future_market: [u8; 32],
) -> bool {
    controller_market != [0; 32] && future_market != [0; 32] && controller_market != future_market
}

fn recompute_candidates(
    admitted: dclutch_trading::series::AdmittedOccurrenceV3,
    admitted_ticket: dclutch_trading::series::AdmittedTicketV3,
    series: SeriesStateV3,
    ticket: TicketStateV3,
    expected_series_revision: u64,
    expected_ticket_revision: u64,
) -> Result<
    (
        [u8; SERIES_STATE_BYTES_V3],
        [u8; SERIES_TICKET_STATE_BYTES_V3],
    ),
    CoreSbfError,
> {
    let series_bytes = series
        .encode(admitted.template().occurrence_count())
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket_bytes = ticket.encode();
    let replay = evaluate_replay_v3(
        SeriesReplayActionV3::Expire {
            ticket_record: admitted_ticket.content_id(),
            expected_ticket_revision,
        },
        admitted.template().occurrence_count(),
        expected_series_revision,
        &series_bytes,
        Some(&ticket_bytes),
    )
    .map_err(|_| CoreSbfError::Reference)?;
    let root_candidate = match replay.series() {
        ReplayCandidateV3::Replace(bytes) => bytes,
        ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => {
            return Err(CoreSbfError::Reference);
        }
    };
    let ticket_candidate = match replay.ticket() {
        ReplayCandidateV3::Replace(bytes) => bytes,
        ReplayCandidateV3::Unchanged | ReplayCandidateV3::Delete => {
            return Err(CoreSbfError::Reference);
        }
    };
    let root_after = SeriesStateV3::decode(&root_candidate, admitted.template().occurrence_count())
        .map_err(|_| CoreSbfError::Reference)?;
    let ticket_after =
        TicketStateV3::decode(&ticket_candidate).map_err(|_| CoreSbfError::Reference)?;
    if root_after.next_occurrence()
        != admitted
            .occurrence()
            .occurrence()
            .checked_add(1)
            .ok_or(CoreSbfError::Arithmetic)?
        || root_after.current_ticket_prepared()
        || ticket_after.phase() != TicketPhaseV3::Expired
        || ticket_after.ticket_record_id() != admitted_ticket.content_id()
    {
        return Err(CoreSbfError::Reference);
    }
    Ok((root_candidate, ticket_candidate))
}

fn authenticate_caller(
    frame: &ExpiryAccounts<'_, '_>,
    caller: &AccountInfo<'_>,
    admitted: dclutch_trading::series::AdmittedOccurrenceV3,
    admitted_ticket: dclutch_trading::series::AdmittedTicketV3,
    controller_market: [u8; 32],
    request_bytes: &[u8],
) -> Result<(), CoreSbfError> {
    let request_digest = hash(request_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        admitted.template().release_set().to_bytes(),
        controller_market,
        ExecutionRoleV1::Trading,
        admitted_ticket.content_id().to_bytes(),
        request_digest,
    )
    .map_err(|_| CoreSbfError::CallerAuthority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), frame.trading_program.key).0;
    if caller.key != &expected {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;
    use dclutch_trading::series::replay::{SeriesPhaseV3, TicketPhaseV3};

    use super::*;

    #[test]
    fn controller_root_market_is_not_the_occurrence_future_market() {
        assert!(controller_and_future_markets_are_distinct(
            [0x41; 32], [0x42; 32],
        ));
        assert!(!controller_and_future_markets_are_distinct(
            [0x41; 32], [0x41; 32],
        ));
        assert!(!controller_and_future_markets_are_distinct(
            [0; 32], [0x42; 32],
        ));
    }

    #[test]
    fn candidate_reconstruction_is_exact_and_substitution_refuses() {
        let ticket_id = ContentId::new([0x71; 32]).expect("ticket id");
        let prepared_root = SeriesStateV3::new(9)
            .prepare_ticket(0)
            .expect("prepared root");
        let prepared_ticket = TicketStateV3::prepared(ticket_id);
        // Exercise the semantic owner directly: the adapter's wrapper above
        // additionally supplies finalized records and their occurrence count.
        let root_bytes = prepared_root.encode(1).expect("root prestate");
        let ticket_bytes = prepared_ticket.encode();
        let replay = evaluate_replay_v3(
            SeriesReplayActionV3::Expire {
                ticket_record: ticket_id,
                expected_ticket_revision: 0,
            },
            1,
            1,
            &root_bytes,
            Some(&ticket_bytes),
        )
        .expect("exact Expire candidate");
        let ReplayCandidateV3::Replace(root_after) = replay.series() else {
            panic!("root replacement")
        };
        let ReplayCandidateV3::Replace(ticket_after) = replay.ticket() else {
            panic!("Ticket replacement")
        };
        let root = SeriesStateV3::decode(&root_after, 1).expect("candidate root");
        let ticket = TicketStateV3::decode(&ticket_after).expect("candidate Ticket");
        assert_eq!(root.phase(), SeriesPhaseV3::Terminal);
        assert_eq!(root.next_occurrence(), 1);
        assert!(!root.current_ticket_prepared());
        assert_eq!(ticket.phase(), TicketPhaseV3::Expired);

        assert!(
            evaluate_replay_v3(
                SeriesReplayActionV3::Expire {
                    ticket_record: ticket_id,
                    expected_ticket_revision: 1,
                },
                1,
                1,
                &root_bytes,
                Some(&ticket_bytes),
            )
            .is_err(),
            "a substituted candidate revision must refuse before mutation",
        );
    }
}
