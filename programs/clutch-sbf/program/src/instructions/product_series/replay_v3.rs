//! Hostile current replay authentication and terminal postwrite for Series V5.
//!
//! The permanent `0xb8/v3` account is the sole counted owner of admissions,
//! lapses, and physically retired links.  FundingV5 physical retirement may
//! consume only the move-only terminal receipt produced here after the exact
//! terminal successor has been written and hostile-reopened.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_series_current::retirement_v5::
    AuthenticatedProductSeriesLifecycleTerminalV5;
use crate::instructions::product_market_lifecycle_v3_current::{
    AuthenticatedMarketLifecycleRootV3, AuthenticatedSeriesMarketLinkV3,
};
use crate::instructions::source_funding_custody_retirement_v1::
    AuthenticatedSourceMarketSharedCoreTerminalV3;
use crate::seeds;
use clutch_product_series::{
    ContentId, FixedCodec, RegistryCapabilityProfileV4Id, RegistryProgramReleaseV2Id,
    MarketLifecyclePhaseV3, SeriesLifecycleLinkRetirementProjectionV3,
    SeriesLifecycleReplayBindingV3Id, SeriesLifecycleReplayPhaseV3,
    SeriesLifecycleReplayV3, SeriesLifecycleReplayV3Id, SeriesMarketLinkPhaseV3,
    SeriesLifecycleTerminalEvidenceV3, SeriesLifecycleTerminalProjectionV3,
    SeriesPlanV5Id,
};
use clutch_solana_layout::product_series::{
    SeriesLifecycleReplayAccountV3, SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const SERIES_LIFECYCLE_REPLAY_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-replay-authentication/v3\0";
const SERIES_LIFECYCLE_REPLAY_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/series-lifecycle-replay-postwrite/v3\0";
const PRODUCT_SERIES_REPLAY_TERMINAL_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-replay-terminal/v5\0";
const SERIES_LIFECYCLE_LINK_RETIREMENT_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/series-lifecycle-link-retirement-postwrite/v3\0";
const PRODUCT_SERIES_LINK_RETIREMENT_FACTS_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-link-retirement-facts/v5\0";

/// Exact hostile authentication of the current permanent `0xb8/v3` owner.
///
/// This is deliberately move-only. A decoded replay body or a caller-provided
/// replay ID cannot be detached and reused as transition authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSeriesLifecycleReplayV3 {
    account: Pubkey,
    value: SeriesLifecycleReplayAccountV3,
    observed_lamports: u64,
    writable: bool,
    data_id: ContentId,
    state_id: SeriesLifecycleReplayV3Id,
    binding_id: SeriesLifecycleReplayBindingV3Id,
    authentication_id: ContentId,
}

impl AuthenticatedSeriesLifecycleReplayV3 {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn value(&self) -> &SeriesLifecycleReplayAccountV3 { &self.value }
    pub(crate) const fn state(&self) -> &SeriesLifecycleReplayV3 { &self.value.state }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn is_writable(&self) -> bool { self.writable }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn state_id(&self) -> SeriesLifecycleReplayV3Id { self.state_id }
    pub(crate) const fn binding_id(&self) -> SeriesLifecycleReplayBindingV3Id {
        self.binding_id
    }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Hostile-authenticate exactly one current permanent replay PDA.
pub(crate) fn authenticate_series_lifecycle_replay_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series_plan_id: SeriesPlanV5Id,
    require_writable: bool,
) -> Outcome<AuthenticatedSeriesLifecycleReplayV3> {
    require(
        !account.is_signer
            && !account.executable
            && account.is_writable == require_writable
            && account.owner == program_id
            && account.data_len() == SERIES_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V3,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesLifecycleReplayAccountV3::decode(&data)?;
    let binding = value.state.binding();
    require(
        binding.series_plan_id == expected_series_plan_id
            && binding.lifecycle_replay_account_id
                == ContentId::from_bytes(account.key.to_bytes()),
        ClutchError::MismatchedState,
    )?;
    let (expected, bump) = seeds::product_series_lifecycle_replay_pda(
        program_id,
        &expected_series_plan_id.bytes(),
    );
    expect_pda(account.key, (expected, bump), Some(value.stored_bump))?;
    let data_id = hash_data(&data);
    drop(data);
    let state_id = value
        .state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_lamports = account.lamports();
    require(
        observed_lamports >= value.permanent_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let authentication_id = hashv(&[
        SERIES_LIFECYCLE_REPLAY_AUTHENTICATION_DOMAIN_V3,
        account.key.as_ref(),
        program_id.as_ref(),
        &data_id.bytes(),
        &state_id.bytes(),
        &binding_id.bytes(),
        &value.permanent_rent_principal_lamports.to_le_bytes(),
        &observed_lamports.to_le_bytes(),
        &[value.stored_bump],
    ]);
    require_live(authentication_id)?;
    Ok(AuthenticatedSeriesLifecycleReplayV3 {
        account: *account.key,
        value,
        observed_lamports,
        writable: account.is_writable,
        data_id,
        state_id,
        binding_id,
        authentication_id,
    })
}

/// Private raw replay writer. Only concrete current lifecycle compositors in
/// this module may turn hostile-authenticated bytes into a successor.
fn write_series_lifecycle_replay_v3(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesLifecycleReplayV3,
    successor: SeriesLifecycleReplayV3,
) -> Outcome<AuthenticatedSeriesLifecycleReplayV3> {
    require(
        account.is_writable
            && *account.key == authenticated.account()
            && account.owner == program_id
            && successor.binding() == authenticated.state().binding(),
        ClutchError::MismatchedState,
    )?;
    let live = authenticate_series_lifecycle_replay_v3(
        program_id,
        account,
        authenticated.state().binding().series_plan_id,
        true,
    )?;
    require(live == authenticated, ClutchError::MismatchedState)?;
    let successor_account = SeriesLifecycleReplayAccountV3 {
        state: successor,
        permanent_rent_principal_lamports: authenticated
            .value()
            .permanent_rent_principal_lamports,
        stored_bump: authenticated.value().stored_bump,
    };
    let observed_lamports = authenticated.observed_lamports();
    let data_before_id = authenticated.data_id();
    let authentication_before_id = authenticated.authentication_id();
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        successor_account.encode(&mut data)?;
    }
    let rebound = authenticate_series_lifecycle_replay_v3(
        program_id,
        account,
        successor_account.state.binding().series_plan_id,
        true,
    )?;
    require(
        rebound.value() == &successor_account
            && rebound.observed_lamports() == observed_lamports
            && rebound.data_id() != data_before_id
            && rebound.authentication_id() != authentication_before_id,
        ClutchError::MismatchedState,
    )?;
    Ok(rebound)
}

/// Move-only replay postwrite for the one Source-owned physical LinkV3
/// retirement. It is retained by Product until the exhaustive terminal write.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesLifecycleLinkRetirementV3 {
    id: ContentId,
    replay: AuthenticatedSeriesLifecycleReplayV3,
    state_before_id: SeriesLifecycleReplayV3Id,
    data_before_id: ContentId,
    authentication_before_id: ContentId,
    projection: SeriesLifecycleLinkRetirementProjectionV3,
    projection_id: ContentId,
}

impl AuthenticatedSeriesLifecycleLinkRetirementV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn replay(&self) -> &AuthenticatedSeriesLifecycleReplayV3 {
        &self.replay
    }
    pub(crate) const fn projection(&self) -> SeriesLifecycleLinkRetirementProjectionV3 {
        self.projection
    }
    pub(crate) const fn projection_id(&self) -> ContentId { self.projection_id }
}

/// Count one physically retired canonical LinkV3 from Source's sole move-only
/// shared-core owner and the exact hostile Root/Link postimages.
pub(crate) fn record_current_series_link_retirement_v3(
    program_id: &Pubkey,
    replay_account: &AccountInfo<'_>,
    source_terminal: &AuthenticatedSourceMarketSharedCoreTerminalV3,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
) -> Outcome<AuthenticatedSeriesLifecycleLinkRetirementV3> {
    let facts = source_terminal.facts();
    let series_plan_id = link.binding().series_plan_id;
    let replay = authenticate_series_lifecycle_replay_v3(
        program_id,
        replay_account,
        series_plan_id,
        true,
    )?;
    let binding = replay.state().binding();
    require(
        replay.state().phase() == SeriesLifecycleReplayPhaseV3::Open
            && binding.series_plan_id == series_plan_id
            && binding.funding_account_id == ContentId::from_bytes(facts.funding_account.bytes())
            && root.state().phase() == MarketLifecyclePhaseV3::Active
            && root.account().to_bytes() == facts.root_account.bytes()
            && root.binding_id() == facts.root_binding_id
            && root.data_id() == facts.root_data_after_id
            && root.authentication_id() == facts.root_authentication_after_id
            && root.semantic_id() == facts.root_semantic_after_id
            && root.state().transition_sequence() == facts.root_transition_sequence_after
            && link.state().phase() == SeriesMarketLinkPhaseV3::Retired
            && link.account().to_bytes() == facts.link_account.bytes()
            && link.binding_id() == facts.link_binding_id
            && link.data_id() == facts.link_data_after_id
            && link.authentication_id() == facts.link_authentication_after_id
            && ContentId::from_bytes(link.semantic_id().bytes()) == facts.link_semantic_after_id
            && link.state().transition_sequence() == facts.link_transition_sequence_after
            && link.binding().market_instance_id.bytes() == facts.market_instance_id.bytes()
            && link.binding().generation == facts.generation,
        ClutchError::MismatchedState,
    )?;
    let state_before_id = replay.state_id();
    let data_before_id = replay.data_id();
    let authentication_before_id = replay.authentication_id();
    let product_retirement_facts_id = hashv(&[
        PRODUCT_SERIES_LINK_RETIREMENT_FACTS_DOMAIN_V5,
        program_id.as_ref(),
        &source_terminal.id().bytes(),
        root.account().as_ref(),
        &root.data_id().bytes(),
        &root.authentication_id().bytes(),
        &root.semantic_id().bytes(),
        &root.state().transition_sequence().to_le_bytes(),
        link.account().as_ref(),
        &link.data_id().bytes(),
        &link.authentication_id().bytes(),
        &link.semantic_id().bytes(),
        &link.state().transition_sequence().to_le_bytes(),
        &facts.link_retirement_projection_id.bytes(),
    ]);
    require_live(product_retirement_facts_id)?;
    let projection = SeriesLifecycleLinkRetirementProjectionV3 {
        binding_id: replay.binding_id(),
        series_plan_id,
        ordinal: link.binding().ordinal,
        link_account_id: ContentId::from_bytes(link.account().to_bytes()),
        market_root_account_id: ContentId::from_bytes(root.account().to_bytes()),
        market_instance_id: link.binding().market_instance_id,
        product_retirement_facts_id,
        link_retirement_projection_id: facts.link_retirement_projection_id,
        market_admission_receipt_id: link.state().market_admission_receipt_id(),
        generation: facts.generation,
    };
    let projection_id = projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor = replay
        .state()
        .record_link_retirement(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_series_lifecycle_replay_v3(
        program_id,
        replay_account,
        replay,
        successor,
    )?;
    let id = hashv(&[
        SERIES_LIFECYCLE_LINK_RETIREMENT_POSTWRITE_DOMAIN_V3,
        program_id.as_ref(),
        replay_account.key.as_ref(),
        &source_terminal.id().bytes(),
        &state_before_id.bytes(),
        &rebound.state_id().bytes(),
        &data_before_id.bytes(),
        &rebound.data_id().bytes(),
        &authentication_before_id.bytes(),
        &rebound.authentication_id().bytes(),
        &projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedSeriesLifecycleLinkRetirementV3 {
        id,
        replay: rebound,
        state_before_id,
        data_before_id,
        authentication_before_id,
        projection,
        projection_id,
    })
}

/// Exact hostile Terminal replay postwrite. It is retained inside the sole
/// Product physical-retirement input and is never exposed as a generic writer.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesLifecycleReplayTerminalV3 {
    id: ContentId,
    replay: AuthenticatedSeriesLifecycleReplayV3,
    state_before_id: SeriesLifecycleReplayV3Id,
    data_before_id: ContentId,
    authentication_before_id: ContentId,
    projection: SeriesLifecycleTerminalProjectionV3,
    projection_id: ContentId,
}

impl AuthenticatedSeriesLifecycleReplayTerminalV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn replay(&self) -> &AuthenticatedSeriesLifecycleReplayV3 {
        &self.replay
    }
    pub(crate) const fn state_before_id(&self) -> SeriesLifecycleReplayV3Id {
        self.state_before_id
    }
    pub(crate) const fn data_before_id(&self) -> ContentId { self.data_before_id }
    pub(crate) const fn authentication_before_id(&self) -> ContentId {
        self.authentication_before_id
    }
    pub(crate) const fn projection(&self) -> SeriesLifecycleTerminalProjectionV3 {
        self.projection
    }
    pub(crate) const fn projection_id(&self) -> ContentId { self.projection_id }
}

/// Sole move-only input accepted by physical FundingV5 retirement.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesReplayTerminalV5 {
    id: ContentId,
    lifecycle: AuthenticatedProductSeriesLifecycleTerminalV5,
    replay: AuthenticatedSeriesLifecycleReplayTerminalV3,
}

impl AuthenticatedProductSeriesReplayTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn lifecycle(&self) -> &AuthenticatedProductSeriesLifecycleTerminalV5 {
        &self.lifecycle
    }
    pub(crate) const fn replay(&self) -> &AuthenticatedSeriesLifecycleReplayTerminalV3 {
        &self.replay
    }

    pub(super) fn into_physical_parts(
        self,
    ) -> (
        AuthenticatedProductSeriesLifecycleTerminalV5,
        AuthenticatedSeriesLifecycleReplayTerminalV3,
    ) {
        (self.lifecycle, self.replay)
    }
}

/// Write and hostile-reopen the exhaustive Terminal replay successor before
/// any FundingV5 account or physical custody can be closed.
pub(crate) fn terminalize_current_series_lifecycle_replay_v3(
    program_id: &Pubkey,
    replay_account: &AccountInfo<'_>,
    lifecycle: AuthenticatedProductSeriesLifecycleTerminalV5,
) -> Outcome<AuthenticatedProductSeriesReplayTerminalV5> {
    let (lifecycle, link_retirement) = lifecycle.into_replay_terminal_parts()?;
    let series_plan_id = lifecycle
        .artifacts()
        .series()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = lifecycle
        .artifacts()
        .funding_terms()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = lifecycle
        .artifacts()
        .quote()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_plan_id = lifecycle
        .artifacts()
        .attachment()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay = authenticate_series_lifecycle_replay_v3(
        program_id,
        replay_account,
        series_plan_id,
        true,
    )?;
    require(
        replay == link_retirement.replay
            && !link_retirement.id.is_zero()
            && link_retirement.projection.binding_id == replay.binding_id(),
        ClutchError::MismatchedState,
    )?;
    let binding = replay.state().binding();
    require(
        binding.funding_terms_id == funding_terms_id
            && binding.funding_quote_id == funding_quote_id
            && binding.attachment_plan_id == attachment_plan_id
            && binding.compiler_bundle_id == lifecycle.bundle().bundle_id()
            && binding.registry_release_id
                == RegistryProgramReleaseV2Id::from_bytes(
                    lifecycle.registry().registry_release_id().bytes(),
                )
            && binding.capability_profile_id
                == RegistryCapabilityProfileV4Id::from_bytes(
                    lifecycle.registry().capability_profile_id().bytes(),
                )
            && binding.registry_account_id
                == ContentId::from_bytes(lifecycle.registry().series_registry_account().to_bytes())
            && binding.funding_account_id
                == ContentId::from_bytes(lifecycle.funding().account().to_bytes())
            && binding.lifecycle_replay_account_id
                == ContentId::from_bytes(replay_account.key.to_bytes())
            && replay.binding_id()
                == binding
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    let state_before_id = replay.state_id();
    let data_before_id = replay.data_id();
    let authentication_before_id = replay.authentication_id();
    let evidence = SeriesLifecycleTerminalEvidenceV3 {
        binding_id: replay.binding_id(),
        funding_account_id: binding.funding_account_id,
        funding_state_id: lifecycle
            .funding()
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .content_id(),
        funding_terminal_projection_id: lifecycle.terminal_projection_id(),
        registry_account_id: binding.registry_account_id,
        registry_authentication_id: lifecycle.registry().id(),
        terminal_authority_receipt_id: lifecycle.id(),
    };
    let (successor, projection) = replay
        .state()
        .terminalize(evidence)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projection_id = projection.id().content_id();
    let rebound = write_series_lifecycle_replay_v3(
        program_id,
        replay_account,
        replay,
        successor,
    )?;
    require(
        rebound.state().phase() == clutch_product_series::SeriesLifecycleReplayPhaseV3::Terminal
            && rebound.state().terminal_projection_id() == projection_id
            && rebound.binding_id() == projection.binding_id()
            && rebound.state_id()
                == rebound
                    .state()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    let postwrite_id = hashv(&[
        SERIES_LIFECYCLE_REPLAY_POSTWRITE_DOMAIN_V3,
        program_id.as_ref(),
        replay_account.key.as_ref(),
        &lifecycle.id().bytes(),
        &state_before_id.bytes(),
        &rebound.state_id().bytes(),
        &data_before_id.bytes(),
        &rebound.data_id().bytes(),
        &authentication_before_id.bytes(),
        &rebound.authentication_id().bytes(),
        &projection_id.bytes(),
    ]);
    require_live(postwrite_id)?;
    let replay_terminal = AuthenticatedSeriesLifecycleReplayTerminalV3 {
        id: postwrite_id,
        replay: rebound,
        state_before_id,
        data_before_id,
        authentication_before_id,
        projection,
        projection_id,
    };
    let id = hashv(&[
        PRODUCT_SERIES_REPLAY_TERMINAL_DOMAIN_V5,
        program_id.as_ref(),
        &lifecycle.id().bytes(),
        &replay_terminal.id().bytes(),
        &replay_terminal.replay().authentication_id().bytes(),
        &projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesReplayTerminalV5 {
        id,
        lifecycle,
        replay: replay_terminal,
    })
}

fn hash_data(data: &[u8]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(id != ContentId::ZERO, ClutchError::MismatchedState)
}
