//! Sole current Product owner of Failure session pins on `0xad/v3`.
//!
//! These transitions decode and hostile-reopen only RootV3/LinkV3.  The
//! similarly named historical helpers in `product_series_current` remain
//! RootV2/LinkV2 and are not accepted here.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_product_series::{
    ContentId, MarketInstanceV2Id, MarketLifecyclePhaseV3, SeriesMarketLinkPhaseV3,
    SeriesMarketLinkV3, SeriesMarketLinkV3Id, SeriesPlanV5Id, SourceOccurrenceV1Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
    AuthenticatedMarketLifecycleRootV3, AuthenticatedSeriesMarketLinkV3,
};

const SERIES_FAILURE_BEGIN_AUTHENTICATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/series-failure-begin-authentication/v3\0";
const SERIES_FAILURE_RELEASE_PREAUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-failure-release-preauthentication/v4\0";
const SERIES_FAILURE_RELEASE_AUTHENTICATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-failure-release-authentication/v4\0";

/// Exhaustive reason one current pinned Failure session may release LinkV3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureSessionReleaseDispositionV4 {
    Resolved,
    Exhausted,
    SourceAbsent,
    SourceRefused,
}

impl FailureSessionReleaseDispositionV4 {
    pub(crate) const fn wire_byte(self) -> u8 {
        match self {
            Self::Resolved => 1,
            Self::Exhausted => 2,
            Self::SourceAbsent => 3,
            Self::SourceRefused => 4,
        }
    }

    const fn requires_writable_root(self) -> bool {
        matches!(self, Self::Resolved)
    }
}

/// Default-refusing owner of the physical Failure Begin postwrite.
pub(crate) trait AuthenticatedSeriesFailureSessionBeginV4 {
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_failure_session_begin_v4(
        &self,
        _root_account: Pubkey,
        _root_authentication_id: ContentId,
        _root_semantic_id: ContentId,
        _root_binding_id: ContentId,
        _link_account: Pubkey,
        _link_authentication_id: ContentId,
        _link_semantic_id: SeriesMarketLinkV3Id,
        _link_binding_id: ContentId,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _begin_admission_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Move-only hostile postwrite for one exclusive LinkV3 Failure pin.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesFailureSessionPinV3 {
    id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_binding_id: ContentId,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: SeriesMarketLinkV3Id,
    link_semantic_after: SeriesMarketLinkV3Id,
    link_binding_id: ContentId,
    begin_admission_receipt_id: ContentId,
    session_binding_id: ContentId,
}

impl AuthenticatedSeriesFailureSessionPinV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId { self.root_semantic_id }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_before(&self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(&self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_semantic_before(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_before
    }
    pub(crate) const fn link_semantic_after(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after
    }
    pub(crate) const fn link_binding_id(&self) -> ContentId { self.link_binding_id }
    pub(crate) const fn begin_admission_receipt_id(&self) -> ContentId {
        self.begin_admission_receipt_id
    }
    pub(crate) const fn session_binding_id(&self) -> ContentId { self.session_binding_id }
}

/// Move-only hostile RootV3/LinkV3 prestate retained by one archive/reset.
#[derive(Debug)]
pub(crate) struct AuthenticatedWritableFailureSessionReleaseLinkV4 {
    id: ContentId,
    disposition: FailureSessionReleaseDispositionV4,
    root_account: Pubkey,
    root_owner_program: Pubkey,
    root_observed_lamports: u64,
    root_data_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_binding_id: ContentId,
    link_account: Pubkey,
    link_owner_program: Pubkey,
    link_observed_lamports: u64,
    link_data_id: ContentId,
    link_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV3Id,
    link_binding_id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    source_occurrence_id: SourceOccurrenceV1Id,
    transition_sequence: u64,
    failure_sessions_started: u32,
    failure_session_transcript_id: ContentId,
}

impl AuthenticatedWritableFailureSessionReleaseLinkV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn disposition(&self) -> FailureSessionReleaseDispositionV4 {
        self.disposition
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_id(&self) -> ContentId {
        self.root_authentication_id
    }
    pub(crate) const fn root_semantic_id(&self) -> ContentId { self.root_semantic_id }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_id(&self) -> ContentId {
        self.link_authentication_id
    }
    pub(crate) const fn link_semantic_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_id
    }
    pub(crate) const fn link_binding_id(&self) -> ContentId { self.link_binding_id }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id { self.series_plan_id }
    pub(crate) const fn ordinal(&self) -> u32 { self.ordinal }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn source_occurrence_id(&self) -> SourceOccurrenceV1Id {
        self.source_occurrence_id
    }
    pub(crate) const fn transition_sequence(&self) -> u64 { self.transition_sequence }
    pub(crate) const fn failure_sessions_started(&self) -> u32 {
        self.failure_sessions_started
    }
    pub(crate) const fn session_binding_id(&self) -> ContentId {
        self.failure_session_transcript_id
    }
}

/// Default-refusing exact archive/reset owner consumed before LinkV3 release.
pub(crate) trait AuthenticatedSeriesFailureArchivePostwriteV4 {
    fn archive_postwrite_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn append_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn reset_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn market_instance_id(&self) -> Outcome<MarketInstanceV2Id> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn generation(&self) -> Outcome<u64> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn source_occurrence_id(&self) -> Outcome<SourceOccurrenceV1Id> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn session_binding_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn session_terminal_receipt_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn release_link_preauthorization_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn release_disposition(&self) -> Outcome<FailureSessionReleaseDispositionV4> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_series_failure_archive_release_postwrite_v4(
        &self,
        _archive_postwrite_id: ContentId,
        _append_receipt_id: ContentId,
        _reset_receipt_id: ContentId,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _source_occurrence_id: SourceOccurrenceV1Id,
        _session_binding_id: ContentId,
        _session_terminal_receipt_id: ContentId,
        _disposition: FailureSessionReleaseDispositionV4,
        _release_link_preauthorization_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Move-only postwrite for one exact LinkV3 Failure release.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesFailureSessionReleaseV4 {
    id: ContentId,
    disposition: FailureSessionReleaseDispositionV4,
    link_account: Pubkey,
    link_authentication_before: ContentId,
    link_authentication_after: ContentId,
    link_semantic_before: SeriesMarketLinkV3Id,
    link_semantic_after: SeriesMarketLinkV3Id,
    transition_sequence_before: u64,
    transition_sequence_after: u64,
    failure_session_transcript_before: ContentId,
    failure_session_transcript_after: ContentId,
    session_terminal_receipt_id: ContentId,
    archive_postwrite_id: ContentId,
    append_receipt_id: ContentId,
    reset_receipt_id: ContentId,
    release_link_preauthorization_id: ContentId,
}

impl AuthenticatedSeriesFailureSessionReleaseV4 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn disposition(&self) -> FailureSessionReleaseDispositionV4 {
        self.disposition
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_before(&self) -> ContentId {
        self.link_authentication_before
    }
    pub(crate) const fn link_authentication_after(&self) -> ContentId {
        self.link_authentication_after
    }
    pub(crate) const fn link_semantic_before(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_before
    }
    pub(crate) const fn link_semantic_after(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after
    }
    pub(crate) const fn transition_sequence_before(&self) -> u64 {
        self.transition_sequence_before
    }
    pub(crate) const fn transition_sequence_after(&self) -> u64 {
        self.transition_sequence_after
    }
    pub(crate) const fn failure_session_transcript_before(&self) -> ContentId {
        self.failure_session_transcript_before
    }
    pub(crate) const fn failure_session_transcript_after(&self) -> ContentId {
        self.failure_session_transcript_after
    }
    pub(crate) const fn session_terminal_receipt_id(&self) -> ContentId {
        self.session_terminal_receipt_id
    }
    pub(crate) const fn release_link_preauthorization_id(&self) -> ContentId {
        self.release_link_preauthorization_id
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn pin_series_market_link_failure_v3<'next, A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    cached_root: AuthenticatedMarketLifecycleRootV3<'_>,
    link_account: &AccountInfo<'_>,
    cached_link: AuthenticatedSeriesMarketLinkV3<'_>,
    begin_admission_receipt_id: ContentId,
    authority: &A,
    root_rebound_output: &mut MarketLifecycleRootAccountV3,
    link_rebound_output: &'next mut SeriesMarketLinkAccountV3,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV3<'next>,
    AuthenticatedSeriesFailureSessionPinV3,
)>
where
    A: AuthenticatedSeriesFailureSessionBeginV4 + ?Sized,
{
    require_live(begin_admission_receipt_id)?;
    let root_binding = *cached_root.binding();
    let root_binding_id = cached_root.binding_id();
    let live_root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        root_binding.market_instance_id,
        root_binding.generation,
        false,
        root_rebound_output,
    )?;
    let link_binding = *cached_link.binding();
    require_unresolved_market_resolution_v3(live_root.state())?;
    require(
        !cached_root.is_writable()
            && !live_root.is_writable()
            && live_root.account() == cached_root.account()
            && live_root.value() == cached_root.value()
            && live_root.authentication_id() == cached_root.authentication_id()
            && live_root.semantic_id() == cached_root.semantic_id()
            && live_root.binding_id() == root_binding_id
            && root_account.key != link_account.key
            && live_root.state().phase() == MarketLifecyclePhaseV3::Active
            && cached_link.is_writable()
            && cached_link.state().phase() == SeriesMarketLinkPhaseV3::Active
            && cached_link.state().active_failure_sessions() == 0
            && link_binding.market_root_account_id.bytes() == live_root.account().to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation,
        ClutchError::MismatchedState,
    )?;
    authority.authenticate_series_failure_session_begin_v4(
        live_root.account(),
        live_root.authentication_id(),
        live_root.semantic_id(),
        root_binding_id,
        cached_link.account(),
        cached_link.authentication_id(),
        cached_link.semantic_id(),
        cached_link.binding_id(),
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        link_binding.source_occurrence_id,
        begin_admission_receipt_id,
    )?;
    let semantic_before = cached_link.semantic_id();
    let authentication_before = cached_link.authentication_id();
    let link_binding_id = cached_link.binding_id();
    let successor = cached_link
        .state()
        .pin_failure_session(begin_admission_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_series_market_link_v3(
        program_id,
        link_account,
        cached_link,
        &successor,
        link_rebound_output,
    )?;
    let semantic_after = rebound.semantic_id();
    let authentication_after = rebound.authentication_id();
    let session_binding_id = rebound.state().failure_session_transcript_id();
    require(
        rebound.state().active_failure_sessions() == 1
            && rebound.state().failure_sessions_started()
                == successor.failure_sessions_started()
            && session_binding_id != ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        SERIES_FAILURE_BEGIN_AUTHENTICATION_DOMAIN_V3,
        program_id.as_ref(),
        live_root.account().as_ref(),
        &live_root.authentication_id().bytes(),
        &live_root.semantic_id().bytes(),
        &root_binding_id.bytes(),
        rebound.account().as_ref(),
        &authentication_before.bytes(),
        &authentication_after.bytes(),
        &semantic_before.bytes(),
        &semantic_after.bytes(),
        &link_binding_id.bytes(),
        &begin_admission_receipt_id.bytes(),
        &session_binding_id.bytes(),
    ]);
    require_live(id)?;
    Ok((
        rebound,
        AuthenticatedSeriesFailureSessionPinV3 {
            id,
            root_account: live_root.account(),
            root_authentication_id: live_root.authentication_id(),
            root_semantic_id: live_root.semantic_id(),
            root_binding_id,
            link_account: *link_account.key,
            link_authentication_before: authentication_before,
            link_authentication_after: authentication_after,
            link_semantic_before: semantic_before,
            link_semantic_after: semantic_after,
            link_binding_id,
            begin_admission_receipt_id,
            session_binding_id,
        },
    ))
}

pub(crate) fn authenticate_writable_failure_resolution_link_v4(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV3<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV4> {
    authenticate_writable_failure_session_release_link_v4(
        program_id,
        root_account,
        root,
        link_account,
        FailureSessionReleaseDispositionV4::Resolved,
        root_output,
        link_output,
    )
}

pub(crate) fn authenticate_writable_failure_exhausted_link_v4(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV3<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV4> {
    authenticate_writable_failure_session_release_link_v4(
        program_id,
        root_account,
        root,
        link_account,
        FailureSessionReleaseDispositionV4::Exhausted,
        root_output,
        link_output,
    )
}

pub(crate) fn authenticate_writable_failure_source_absent_link_v4(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV3<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV4> {
    authenticate_writable_failure_session_release_link_v4(
        program_id,
        root_account,
        root,
        link_account,
        FailureSessionReleaseDispositionV4::SourceAbsent,
        root_output,
        link_output,
    )
}

pub(crate) fn authenticate_writable_failure_source_refused_link_v4(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    root: AuthenticatedMarketLifecycleRootV3<'_>,
    link_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV4> {
    authenticate_writable_failure_session_release_link_v4(
        program_id,
        root_account,
        root,
        link_account,
        FailureSessionReleaseDispositionV4::SourceRefused,
        root_output,
        link_output,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_writable_failure_session_release_link_v4(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    cached_root: AuthenticatedMarketLifecycleRootV3<'_>,
    link_account: &AccountInfo<'_>,
    disposition: FailureSessionReleaseDispositionV4,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedWritableFailureSessionReleaseLinkV4> {
    let cached_binding = *cached_root.binding();
    let root_requires_writable = disposition.requires_writable_root();
    let live_root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        cached_binding.market_instance_id,
        cached_binding.generation,
        root_requires_writable,
        root_output,
    )?;
    require_unresolved_market_resolution_v3(live_root.state())?;
    require(
        cached_root.is_writable() == root_requires_writable
            && live_root.account() == cached_root.account()
            && live_root.value() == cached_root.value()
            && live_root.authentication_id() == cached_root.authentication_id()
            && live_root.semantic_id() == cached_root.semantic_id()
            && live_root.state().phase() == MarketLifecyclePhaseV3::Active
            && root_account.key != link_account.key,
        ClutchError::MismatchedState,
    )?;
    let data = link_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV3::decode_into(&data, link_output)?;
    let decoded_binding = *link_output.state.binding_ref();
    drop(data);
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        decoded_binding.series_plan_id,
        decoded_binding.ordinal,
        cached_binding.market_instance_id,
        cached_binding.generation,
        live_root.account(),
        true,
        link_output,
    )?;
    let state = link.state();
    let binding = *state.binding_ref();
    let transcript = state.failure_session_transcript_id();
    require(
        link.is_writable()
            && state.phase() == SeriesMarketLinkPhaseV3::Active
            && state.active_failure_sessions() == 1
            && state.failure_sessions_started() != 0
            && transcript != ContentId::ZERO
            && binding.market_root_account_id.bytes() == live_root.account().to_bytes()
            && binding.market_binding_id == live_root.binding_id()
            && binding.market_instance_id == cached_binding.market_instance_id
            && binding.generation == cached_binding.generation,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        SERIES_FAILURE_RELEASE_PREAUTHENTICATION_DOMAIN_V4,
        &[disposition.wire_byte()],
        program_id.as_ref(),
        live_root.account().as_ref(),
        &live_root.data_id().bytes(),
        &live_root.authentication_id().bytes(),
        &live_root.semantic_id().bytes(),
        &live_root.binding_id().bytes(),
        link.account().as_ref(),
        &link.data_id().bytes(),
        &link.authentication_id().bytes(),
        &link.semantic_id().bytes(),
        &link.binding_id().bytes(),
        &state.transition_sequence().to_le_bytes(),
        &state.failure_sessions_started().to_le_bytes(),
        &transcript.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedWritableFailureSessionReleaseLinkV4 {
        id,
        disposition,
        root_account: live_root.account(),
        root_owner_program: live_root.owner_program(),
        root_observed_lamports: live_root.observed_lamports(),
        root_data_id: live_root.data_id(),
        root_authentication_id: live_root.authentication_id(),
        root_semantic_id: live_root.semantic_id(),
        root_binding_id: live_root.binding_id(),
        link_account: link.account(),
        link_owner_program: link.owner_program(),
        link_observed_lamports: link.observed_lamports(),
        link_data_id: link.data_id(),
        link_authentication_id: link.authentication_id(),
        link_semantic_id: link.semantic_id(),
        link_binding_id: link.binding_id(),
        series_plan_id: binding.series_plan_id,
        ordinal: binding.ordinal,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        source_occurrence_id: binding.source_occurrence_id,
        transition_sequence: state.transition_sequence(),
        failure_sessions_started: state.failure_sessions_started(),
        failure_session_transcript_id: transcript,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn release_series_market_link_failure_v4<'next, A>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedSeriesMarketLinkV3<'_>,
    release_link: AuthenticatedWritableFailureSessionReleaseLinkV4,
    archive: A,
    rebound_output: &'next mut SeriesMarketLinkAccountV3,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV3<'next>,
    AuthenticatedSeriesFailureSessionReleaseV4,
)>
where
    A: AuthenticatedSeriesFailureArchivePostwriteV4,
{
    let binding = *authenticated.binding();
    let semantic_before = authenticated.semantic_id();
    let authentication_before = authenticated.authentication_id();
    let transition_sequence_before = authenticated.state().transition_sequence();
    let transcript_before = authenticated.state().failure_session_transcript_id();
    let sessions_started_before = authenticated.state().failure_sessions_started();
    let archive_postwrite_id = archive.archive_postwrite_id()?;
    let append_receipt_id = archive.append_receipt_id()?;
    let reset_receipt_id = archive.reset_receipt_id()?;
    let market_instance_id = archive.market_instance_id()?;
    let generation = archive.generation()?;
    let source_occurrence_id = archive.source_occurrence_id()?;
    let session_binding_id = archive.session_binding_id()?;
    let session_terminal_receipt_id = archive.session_terminal_receipt_id()?;
    let disposition = archive.release_disposition()?;
    let preauthorization_id = archive.release_link_preauthorization_id()?;
    for id in [
        archive_postwrite_id,
        append_receipt_id,
        reset_receipt_id,
        session_binding_id,
        session_terminal_receipt_id,
        preauthorization_id,
    ] {
        require_live(id)?;
    }
    require(
        authenticated.is_writable()
            && authenticated.state().phase() == SeriesMarketLinkPhaseV3::Active
            && authenticated.state().active_failure_sessions() == 1
            && disposition == release_link.disposition
            && preauthorization_id == release_link.id
            && release_link.link_account == *account.key
            && release_link.link_owner_program == *program_id
            && release_link.link_observed_lamports == authenticated.observed_lamports()
            && release_link.link_data_id == authenticated.data_id()
            && release_link.link_authentication_id == authentication_before
            && release_link.link_semantic_id == semantic_before
            && release_link.link_binding_id == authenticated.binding_id()
            && release_link.root_account
                == Pubkey::new_from_array(binding.market_root_account_id.bytes())
            && release_link.root_owner_program == *program_id
            && release_link.root_authentication_id != ContentId::ZERO
            && release_link.root_semantic_id != ContentId::ZERO
            && release_link.root_binding_id == binding.market_binding_id
            && release_link.series_plan_id == binding.series_plan_id
            && release_link.ordinal == binding.ordinal
            && release_link.market_instance_id == binding.market_instance_id
            && release_link.generation == binding.generation
            && release_link.source_occurrence_id == binding.source_occurrence_id
            && release_link.transition_sequence == transition_sequence_before
            && release_link.failure_sessions_started == sessions_started_before
            && release_link.failure_session_transcript_id == transcript_before
            && session_binding_id == transcript_before
            && market_instance_id == binding.market_instance_id
            && generation == binding.generation
            && source_occurrence_id == binding.source_occurrence_id
            && archive_postwrite_id != append_receipt_id
            && archive_postwrite_id != reset_receipt_id
            && append_receipt_id != reset_receipt_id
            && session_terminal_receipt_id != session_binding_id,
        ClutchError::MismatchedState,
    )?;
    archive.authenticate_series_failure_archive_release_postwrite_v4(
        archive_postwrite_id,
        append_receipt_id,
        reset_receipt_id,
        market_instance_id,
        generation,
        source_occurrence_id,
        session_binding_id,
        session_terminal_receipt_id,
        disposition,
        preauthorization_id,
    )?;
    let successor = authenticated
        .state()
        .release_failure_session(session_terminal_receipt_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_series_market_link_v3(
        program_id,
        account,
        authenticated,
        &successor,
        rebound_output,
    )?;
    let semantic_after = rebound.semantic_id();
    let authentication_after = rebound.authentication_id();
    let sequence_after = rebound.state().transition_sequence();
    let transcript_after = rebound.state().failure_session_transcript_id();
    require(
        rebound.state().phase() == SeriesMarketLinkPhaseV3::Active
            && rebound.state().active_failure_sessions() == 0
            && rebound.state().failure_sessions_started() == sessions_started_before
            && sequence_after
                == transition_sequence_before
                    .checked_add(1)
                    .ok_or(ClutchError::Arithmetic)?
            && transcript_after != transcript_before,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        SERIES_FAILURE_RELEASE_AUTHENTICATION_DOMAIN_V4,
        &[disposition.wire_byte()],
        account.key.as_ref(),
        &authentication_before.bytes(),
        &authentication_after.bytes(),
        &semantic_before.bytes(),
        &semantic_after.bytes(),
        &transition_sequence_before.to_le_bytes(),
        &sequence_after.to_le_bytes(),
        &transcript_before.bytes(),
        &transcript_after.bytes(),
        &session_terminal_receipt_id.bytes(),
        &archive_postwrite_id.bytes(),
        &append_receipt_id.bytes(),
        &reset_receipt_id.bytes(),
        &preauthorization_id.bytes(),
    ]);
    require_live(id)?;
    Ok((
        rebound,
        AuthenticatedSeriesFailureSessionReleaseV4 {
            id,
            disposition,
            link_account: *account.key,
            link_authentication_before: authentication_before,
            link_authentication_after: authentication_after,
            link_semantic_before: semantic_before,
            link_semantic_after: semantic_after,
            transition_sequence_before,
            transition_sequence_after: sequence_after,
            failure_session_transcript_before: transcript_before,
            failure_session_transcript_after: transcript_after,
            session_terminal_receipt_id,
            archive_postwrite_id,
            append_receipt_id,
            reset_receipt_id,
            release_link_preauthorization_id: preauthorization_id,
        },
    ))
}

fn write_series_market_link_v3<'next>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    before: AuthenticatedSeriesMarketLinkV3<'_>,
    after: &SeriesMarketLinkV3,
    rebound_output: &'next mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedSeriesMarketLinkV3<'next>> {
    let binding = *before.binding();
    require(
        before.is_writable()
            && before.owner_program() == *program_id
            && before.account() == *account.key
            && account.is_writable
            && !account.is_signer
            && !account.executable,
        ClutchError::MismatchedState,
    )?;
    let before_authentication_id = before.authentication_id();
    let before_data_id = before.data_id();
    let before_semantic_id = before.semantic_id();
    let observed_lamports = before.observed_lamports();
    let stored_bump = before.value().stored_bump;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV3::encode_parts(after, stored_bump, &mut data)?;
    }
    let rebound = authenticate_series_market_link_v3(
        program_id,
        account,
        binding.series_plan_id,
        binding.ordinal,
        binding.market_instance_id,
        binding.generation,
        Pubkey::new_from_array(binding.market_root_account_id.bytes()),
        true,
        rebound_output,
    )?;
    require(
        rebound.state() == after
            && rebound.binding_id() == before.binding_id()
            && rebound.observed_lamports() == observed_lamports
            && rebound.authentication_id() != before_authentication_id
            && rebound.data_id() != before_data_id
            && rebound.semantic_id() != before_semantic_id,
        ClutchError::MismatchedState,
    )?;
    Ok(rebound)
}

fn require_unresolved_market_resolution_v3(
    root: &clutch_product_series::MarketLifecycleRootV3,
) -> Outcome<()> {
    require(
        root.resolution_semantic_id() == ContentId::ZERO
            && root.resolution_data_id() == ContentId::ZERO
            && root.resolution_activation_receipt_id() == ContentId::ZERO,
        ClutchError::MismatchedState,
    )
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
