//! Physical founder for the current permanent Series lifecycle replay.
//!
//! The replay is created only from the move-only FundingV5 physical founder,
//! current RegistryCapabilityV5, and the hostile-reopened initial FundingV5
//! account. Its permanently retained rent is paid in full by the exact payer
//! already authenticated by physical capitalization. Predictable-address
//! prefunding is swept only to the immutable FundingTerms lamport sink.

use super::physical_v5::AuthenticatedSeriesPhysicalFounderV5;
use super::replay_v3::{
    authenticate_series_lifecycle_replay_v3, AuthenticatedSeriesLifecycleReplayV3,
};
use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{read_rent, SYSTEM_PROGRAM_ID};
use crate::instructions::product_series_current::{
    AuthenticatedRegistryCapabilityV5, AuthenticatedSeriesFundingAccountV5,
};
use crate::seeds;
use clutch_product_series::{
    CompiledProductSeriesBundleV7Id, ContentId, RegistryCapabilityProfileV4Id,
    RegistryProgramReleaseV2Id, SeriesAttachmentPlanV6Id, SeriesFundingPhaseV5,
    SeriesFundingQuoteV6Id, SeriesFundingTermsV2Id, SeriesLifecycleReplayBindingV3,
    SeriesLifecycleReplayPhaseV3, SeriesLifecycleReplayV3,
};
use clutch_solana_layout::product_series::{
    SeriesLifecycleReplayAccountV3, SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_SERIES_LIFECYCLE_REPLAY_FOUNDER_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-lifecycle-replay-founder/v5\0";

/// Move-only postwrite for the exact newly created permanent `0xb8/v3` owner.
///
/// The physical, Registry, and Funding authorities remain owned by this value
/// until the current action15 founder consumes them into its next boundary.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesLifecycleReplayFounderV5 {
    id: ContentId,
    physical: AuthenticatedSeriesPhysicalFounderV5,
    registry: AuthenticatedRegistryCapabilityV5,
    funding: AuthenticatedSeriesFundingAccountV5,
    replay: AuthenticatedSeriesLifecycleReplayV3,
    payer: Pubkey,
    neutral_lamport_sink: Pubkey,
    rent_principal_lamports: u64,
    prefund_donation_lamports: u64,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    sink_lamports_before: u64,
    sink_lamports_after: u64,
}

impl AuthenticatedSeriesLifecycleReplayFounderV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn physical(&self) -> &AuthenticatedSeriesPhysicalFounderV5 {
        &self.physical
    }
    pub(crate) const fn registry(&self) -> &AuthenticatedRegistryCapabilityV5 {
        &self.registry
    }
    pub(crate) const fn funding(&self) -> &AuthenticatedSeriesFundingAccountV5 {
        &self.funding
    }
    pub(crate) const fn replay(&self) -> &AuthenticatedSeriesLifecycleReplayV3 {
        &self.replay
    }
    pub(crate) const fn payer(&self) -> Pubkey { self.payer }
    pub(crate) const fn neutral_lamport_sink(&self) -> Pubkey {
        self.neutral_lamport_sink
    }
    pub(crate) const fn rent_principal_lamports(&self) -> u64 {
        self.rent_principal_lamports
    }
    pub(crate) const fn prefund_donation_lamports(&self) -> u64 {
        self.prefund_donation_lamports
    }

    pub(crate) fn into_current_founder_parts(
        self,
    ) -> (
        AuthenticatedSeriesPhysicalFounderV5,
        AuthenticatedRegistryCapabilityV5,
        AuthenticatedSeriesFundingAccountV5,
        AuthenticatedSeriesLifecycleReplayV3,
    ) {
        (self.physical, self.registry, self.funding, self.replay)
    }
}

/// Create and hostile-reopen the sole current permanent Series replay.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn initialize_current_series_lifecycle_replay_v3<'a>(
    program_id: &Pubkey,
    physical: AuthenticatedSeriesPhysicalFounderV5,
    registry: AuthenticatedRegistryCapabilityV5,
    funding: AuthenticatedSeriesFundingAccountV5,
    payer: &AccountInfo<'a>,
    replay_account: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> Outcome<AuthenticatedSeriesLifecycleReplayFounderV5> {
    let capitalization = physical.capitalization();
    let funding_state = funding.state();
    let series_plan_id = physical.series_plan_id();
    let (expected_replay, stored_bump) = seeds::product_series_lifecycle_replay_pda(
        program_id,
        &series_plan_id.bytes(),
    );
    let rent = read_rent(rent_sysvar)?;
    let rent_principal_lamports =
        rent.minimum_balance(SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V3)?;
    require(
        registry.activation_consumed()
            && registry.id() == physical.registry_capability_after_id()
            && registry.series_plan_id() == series_plan_id
            && registry.funding_terms_id().content_id() == capitalization.funding_terms_id()
            && registry.compiler_bundle_id().content_id() == capitalization.compiler_bundle_id()
            && registry.series_registry_account() == capitalization.registry_account()
            && registry.registry_release_id() == physical.registry_release_id()
            && registry.capability_profile_id() == physical.capability_profile_id()
            && funding.is_writable()
            && funding.account() == capitalization.funding_account()
            && funding.data_id() == capitalization.funding_data_id()
            && funding.authentication_id() == capitalization.funding_authentication_id()
            && funding_state.series_plan_id == series_plan_id
            && funding_state.funding_terms_id == registry.funding_terms_id()
            && funding_state.compiler_bundle_id == registry.compiler_bundle_id()
            && funding_state.funding_quote_id.content_id() == capitalization.funding_quote_id()
            && funding_state.attachment_plan_id.content_id()
                == capitalization.attachment_plan_id()
            && funding_state.phase == SeriesFundingPhaseV5::Active
            && funding_state.next_ordinal == 0
            && funding_state.lapsed_count == 0
            && funding_state.transition_sequence == 0
            && *payer.key == capitalization.payer()
            && payer.key.to_bytes() == capitalization.lamport_principal_refund().to_bytes()
            && *neutral_lamport_sink.key == capitalization.neutral_lamport_sink()
            && *replay_account.key == expected_replay
            && replay_account.key != payer.key
            && replay_account.key != neutral_lamport_sink.key
            && payer.key != neutral_lamport_sink.key
            && *system_program.key == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )?;
    super::require_system_lamport_destination(
        neutral_lamport_sink,
        ContentId::from_bytes(capitalization.neutral_lamport_sink().to_bytes()),
    )?;

    let binding = SeriesLifecycleReplayBindingV3 {
        series_plan_id,
        funding_terms_id: SeriesFundingTermsV2Id::from_bytes(
            capitalization.funding_terms_id().bytes(),
        ),
        funding_quote_id: SeriesFundingQuoteV6Id::from_bytes(
            capitalization.funding_quote_id().bytes(),
        ),
        attachment_plan_id: SeriesAttachmentPlanV6Id::from_bytes(
            capitalization.attachment_plan_id().bytes(),
        ),
        compiler_bundle_id: CompiledProductSeriesBundleV7Id::from_bytes(
            capitalization.compiler_bundle_id().bytes(),
        ),
        registry_release_id: RegistryProgramReleaseV2Id::from_bytes(
            physical.registry_release_id().bytes(),
        ),
        capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes(
            physical.capability_profile_id().bytes(),
        ),
        registry_account_id: ContentId::from_bytes(capitalization.registry_account().to_bytes()),
        funding_account_id: ContentId::from_bytes(capitalization.funding_account().to_bytes()),
        lifecycle_replay_account_id: ContentId::from_bytes(replay_account.key.to_bytes()),
        permanent_rent_funder: ContentId::from_bytes(payer.key.to_bytes()),
        neutral_lamport_sink: ContentId::from_bytes(neutral_lamport_sink.key.to_bytes()),
        instance_count: funding_state.instance_count,
    };
    let state = SeriesLifecycleReplayV3::initialize(binding)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let prefund_donation_lamports = replay_account.lamports();
    let payer_lamports_before = payer.lamports();
    let sink_lamports_before = neutral_lamport_sink.lamports();
    let series_seed = series_plan_id.bytes();
    let bump_seed = [stored_bump];
    super::create_series_program_account(
        program_id,
        payer,
        replay_account,
        neutral_lamport_sink,
        system_program,
        &rent,
        SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V3,
        rent_principal_lamports,
        &[
            seeds::SEED_PRODUCT_SERIES_LIFECYCLE_REPLAY,
            &series_seed,
            &bump_seed,
        ],
    )?;
    let encoded = SeriesLifecycleReplayAccountV3 {
        state,
        permanent_rent_principal_lamports: rent_principal_lamports,
        stored_bump,
    };
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(data.iter().all(|byte| *byte == 0), ClutchError::AlreadyInitialized)?;
        encoded.encode(&mut data)?;
    }
    let replay = authenticate_series_lifecycle_replay_v3(
        program_id,
        replay_account,
        series_plan_id,
        true,
    )?;
    let payer_lamports_after = payer.lamports();
    let sink_lamports_after = neutral_lamport_sink.lamports();
    require(
        replay.value() == &encoded
            && replay.state().phase() == SeriesLifecycleReplayPhaseV3::Open
            && replay.state().transition_sequence() == 0
            && replay.state().processed_ordinals() == 0
            && replay.observed_lamports() == rent_principal_lamports
            && payer_lamports_after
                == payer_lamports_before
                    .checked_sub(rent_principal_lamports)
                    .ok_or(ClutchError::Arithmetic)?
            && sink_lamports_after
                == sink_lamports_before
                    .checked_add(prefund_donation_lamports)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_SERIES_LIFECYCLE_REPLAY_FOUNDER_DOMAIN_V5,
            program_id.as_ref(),
            &physical.id().bytes(),
            &registry.id().bytes(),
            funding.account().as_ref(),
            &funding.authentication_id().bytes(),
            replay_account.key.as_ref(),
            &replay.binding_id().bytes(),
            &replay.state_id().bytes(),
            &replay.data_id().bytes(),
            &replay.authentication_id().bytes(),
            payer.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            rent_sysvar.key.as_ref(),
            &rent.lamports_per_byte_year.to_le_bytes(),
            &rent.exemption_threshold.to_bits().to_le_bytes(),
            &rent_principal_lamports.to_le_bytes(),
            &prefund_donation_lamports.to_le_bytes(),
            &payer_lamports_before.to_le_bytes(),
            &payer_lamports_after.to_le_bytes(),
            &sink_lamports_before.to_le_bytes(),
            &sink_lamports_after.to_le_bytes(),
            &[stored_bump],
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSeriesLifecycleReplayFounderV5 {
        id,
        physical,
        registry,
        funding,
        replay,
        payer: *payer.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        rent_principal_lamports,
        prefund_donation_lamports,
        payer_lamports_before,
        payer_lamports_after,
        sink_lamports_before,
        sink_lamports_after,
    })
}
