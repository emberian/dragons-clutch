//! Sole current Product V3/V5 retirement owner.
//!
//! This module is a successor, not a bridge, for historical RootV2/LinkV2 and
//! FundingV4 retirement.  Every callable transition below hostile-reopens the
//! exact current RootV3/LinkV3 accounts, consumes one concrete move-only family
//! terminal, persists the unique successor, and hostile-reopens the postimage.
//! The final whole-Series composer will consume these postwrites by value
//! before closing FundingV5 and handing its own move-only receipt directly to
//! General action47 in the same instruction.

use super::{AuthenticatedRegistryCapabilityV5, AuthenticatedSeriesFundingAccountV5};

use super::super::failure_market_family_terminal_v2::{
    authenticate_failure_market_source_failure_lifecycle_terminal_v3,
    AuthenticatedFailureMarketPhysicalTerminalV3,
    AuthenticatedFailureMarketPhysicalTerminalAuthorityV3,
    AuthenticatedFailureMarketFamilyTerminalReceiptV3,
    AuthenticatedFailureMarketFamilyTerminalOwnerV2, FailureMarketFamilyTerminalConsumerFactsV3,
    FailureMarketPhysicalTerminalConsumerFactsV3,
};
use super::super::fractional_product_consumer::consume_fractional_terminal_v2;
use super::super::fractional_redemption::AuthenticatedFractionalFamilyPhysicalTerminalV2;
use super::super::general_treasury_position_terminal_v5::
    AuthenticatedProductPositionPhysicalTerminalV5;
use super::super::dealer_facility::AuthenticatedDealerFamilyTerminalReceiptV1;
use super::super::direct_market_v2::{
    AuthenticatedDirectFamilyTerminalV3, AuthenticatedProductDirectFamilyPreterminalV3,
};
use super::super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
};
use super::super::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV7, AuthenticatedSeriesSourceArtifactsV6,
};
use super::super::product_series::physical_v5::{
    retire_current_series_physical_v5, AuthenticatedSeriesPhysicalRetirementV5,
};
use super::super::structured_custody::AuthenticatedStructuredWrapperFamilyTerminalV3;
use super::super::product_series_current::AuthenticatedProductFractionalFamilyTerminalV2;
use super::super::source_funding_custody_retirement_v1::{
    authenticate_source_family_terminal_authority_v3,
    consume_source_family_terminal_into_product_v3, retire_source_funding_custody_v3,
    AuthenticatedSourceMarketSharedCoreTerminalV3,
    AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1,
    SourceFamilyTerminalProjectionV3, SourceMarketSharedCoreTerminalFactsV3,
};
use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, AuthenticatedSeriesFundingAuthorityV5,
    ComponentDebitV1, ContentId, MarketFamilyAggregatorV1,
    MarketFamilyV1, MarketInstanceV2Id, MarketLifecyclePhaseV3,
    MarketSharedCoreTerminalProjectionV3, MarketSharedCoreV3,
    SeriesLinkObligationDispositionV3, SeriesLinkObligationTerminalProjectionV3,
    SeriesLinkObligationStatusV3, SeriesLinkObligationV3, SeriesMarketLinkPhaseV3,
    SeriesMarketLinkV3Id,
    SeriesFundingAbortBindingV5, SeriesFundingComponentV2,
    SeriesFundingCompletionBindingV5, SeriesFundingPhaseV5, SeriesFundingQuoteV6,
    SeriesFundingReservationBindingV5, SeriesFundingStateV5,
    SeriesFundingTerminalProjectionV5, SeriesAttachmentPlanV6, SeriesPlanV5,
    SeriesPlanV5Id, SeriesFundingTermsV2Id, CompiledProductSeriesBundleV7Id,
    SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use clutch_source_plane_v3_runtime::{
    AuthenticatedSourceRouteV1, SourceWorkScheduleBindingV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_FAILURE_CORE_TERMINAL_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-failure-core-terminal-postwrite/v5\0";
const PRODUCT_FAILURE_PHYSICAL_TERMINAL_LATCH_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-failure-physical-terminal-latch/v5\0";
const PRODUCT_DIRECT_FAMILY_PRETERMINAL_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-direct-family-preterminal/v5\0";
const PRODUCT_DIRECT_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-direct-family-terminal-postwrite/v5\0";
const PRODUCT_STRUCTURED_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-structured-family-terminal-postwrite/v5\0";
const PRODUCT_FRACTIONAL_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-fractional-family-terminal-postwrite/v5\0";
const PRODUCT_LIQUIDITY_OBLIGATION_ABSENCE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-liquidity-obligation-absence/v5\0";
const PRODUCT_LIQUIDITY_OBLIGATION_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-liquidity-obligation-postwrite/v5\0";
const PRODUCT_SERIES_FUNDING_TERMINAL_AUTHORITY_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-funding-terminal-authority/v5\0";
const PRODUCT_SERIES_LIFECYCLE_TERMINAL_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-lifecycle-terminal/v5\0";
const PRODUCT_SOURCE_SHARED_CORE_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-source-shared-core-postwrite/v5\0";
const PRODUCT_MARKET_BEGIN_RETIREMENT_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-market-begin-retirement/v5\0";
const PRODUCT_SERIES_PHYSICAL_RETIREMENT_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-physical-retirement-postwrite/v5\0";
const PRODUCT_MARKET_TERMINAL_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-market-terminal-postwrite/v5\0";

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

fn account_id(account: Pubkey) -> ContentId {
    ContentId::from_bytes(account.to_bytes())
}

/// Decode only enough untrusted LinkV3 bytes to supply the typed PDA inputs to
/// the full hostile authenticator below.  No authority is minted from this
/// observation and every field is exact-compared by that authenticator.
fn observe_link_coordinate_v3(
    account: &AccountInfo<'_>,
) -> Outcome<(SeriesPlanV5Id, u32)> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesMarketLinkAccountV3::decode(&data)?;
    Ok((value.state.binding_ref().series_plan_id, value.state.binding_ref().ordinal))
}

/// Product is the sole semantic owner of an attachment obligation which was
/// never founded.  The durable LinkV3 status and its immutable binding are the
/// absence proof; no caller-provided receipt or family DTO participates.
fn liquidity_absence_projection_v5(
    program_id: &Pubkey,
    root_binding_id: ContentId,
    link: &clutch_product_series::SeriesMarketLinkV3,
) -> Outcome<SeriesLinkObligationTerminalProjectionV3> {
    let status = link.obligation_status(SeriesLinkObligationV3::Liquidity);
    require(
        matches!(
            status,
            SeriesLinkObligationStatusV3::CapabilityDisabled
                | SeriesLinkObligationStatusV3::EnabledNeverFounded
        ) && link
            .obligation_admission_receipt_id(SeriesLinkObligationV3::Liquidity)
            .is_zero(),
        ClutchError::MismatchedState,
    )?;
    let binding = link.binding_ref();
    let semantic_id = link
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let owner_receipt_id = hashv(&[
        PRODUCT_LIQUIDITY_OBLIGATION_ABSENCE_DOMAIN_V5,
        program_id.as_ref(),
        &root_binding_id.bytes(),
        &semantic_id.bytes(),
        &binding.compiler_bundle_id.bytes(),
        &binding.attachment_plan_id.bytes(),
        &binding.capability_profile_id.bytes(),
        &binding.obligation_configuration_id.bytes(),
        &[status.wire_byte()],
    ]);
    require_live(owner_receipt_id)?;
    Ok(SeriesLinkObligationTerminalProjectionV3 {
        link_semantic_id: semantic_id,
        obligation: SeriesLinkObligationV3::Liquidity,
        disposition: SeriesLinkObligationDispositionV3::Absent,
        link_transition_sequence: link
            .transition_sequence()
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        owner_terminal_receipt_id: owner_receipt_id,
    })
}

pub(crate) fn write_market_lifecycle_root_v3(
    account: &AccountInfo<'_>,
    before: &MarketLifecycleRootAccountV3,
    after: &clutch_product_series::MarketLifecycleRootV3,
) -> Outcome<()> {
    require(
        account.is_writable && !account.is_signer && !account.executable,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV3::encode_parts(
        after,
        before.rent_principal_lamports,
        before.stored_bump,
        &mut data,
    )?;
    Ok(())
}

fn write_series_market_link_v3(
    account: &AccountInfo<'_>,
    before: &SeriesMarketLinkAccountV3,
    after: &clutch_product_series::SeriesMarketLinkV3,
) -> Outcome<()> {
    require(
        account.is_writable && !account.is_signer && !account.executable,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV3::encode_parts(after, before.stored_bump, &mut data)?;
    Ok(())
}

/// Product's move-only preterminal consumed inside Direct's physical action13
/// close.  Direct remains the semantic owner of its full current authority
/// ID; Product proves the exact RootV3 family prestate and canonical Direct
/// root coordinate without duplicating the Direct binding body.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductDirectFamilyPreterminalV5 {
    id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    series_link_account: Pubkey,
    series_link_binding_id: ContentId,
    direct_root_account_id: ContentId,
    family_prestate_id: ContentId,
    family_terminal_sequence: u32,
}

impl AuthenticatedProductDirectFamilyPreterminalV3
    for AuthenticatedProductDirectFamilyPreterminalV5
{
    fn product_family_prestate_id(&self) -> Outcome<ContentId> {
        Ok(self.family_prestate_id)
    }

    fn family_terminal_sequence(&self) -> Outcome<u32> {
        Ok(self.family_terminal_sequence)
    }

    fn authenticate_direct_family_preterminal_v3(
        &self,
        market_instance_id: ContentId,
        generation: u64,
        product_root_account: ContentId,
        product_market_binding_id: ContentId,
        current_product_authority_id: ContentId,
        series_link_account: ContentId,
        series_link_binding_id: ContentId,
        direct_root_account: ContentId,
        product_family_prestate_id: ContentId,
        family_terminal_sequence: u32,
    ) -> Outcome<()> {
        require(
            !self.id.is_zero()
                && market_instance_id == self.market_instance_id.content_id()
                && generation == self.generation
                && product_root_account == account_id(self.root_account)
                && product_market_binding_id == self.root_binding_id
                && !current_product_authority_id.is_zero()
                && current_product_authority_id != product_root_account
                && current_product_authority_id != product_market_binding_id
                && series_link_account == account_id(self.series_link_account)
                && series_link_binding_id == self.series_link_binding_id
                && current_product_authority_id != direct_root_account
                && current_product_authority_id != product_family_prestate_id
                && direct_root_account == self.direct_root_account_id
                && product_family_prestate_id == self.family_prestate_id
                && family_terminal_sequence == self.family_terminal_sequence,
            ClutchError::MismatchedState,
        )
    }
}

/// Hostile-reopen the current Product RootV3 before Direct closes any local
/// archive.  The resulting value can only be consumed by Direct's typed
/// action13 primitive; it exposes no generic Product writer.
#[inline(never)]
pub(crate) fn authenticate_product_direct_family_preterminal_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductDirectFamilyPreterminalV5> {
    require(root_account.key != link_account.key, ClutchError::AccountAlias)?;
    let mut observed = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    {
        let data = root_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        MarketLifecycleRootAccountV3::decode_into(&data, &mut observed)?;
    }
    let market_instance_id = observed.state.binding_ref().market_instance_id;
    let generation = observed.state.binding_ref().generation;
    let mut value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        generation,
        true,
        &mut value,
    )?;
    let family = root.state().product_families().family(MarketFamilyV1::Direct);
    let family_prestate_id = root
        .state()
        .product_families()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let direct_root_account_id = root
        .state()
        .product_families()
        .binding()
        .family_root_id(MarketFamilyV1::Direct);
    let root_binding_id = root.binding_id();
    let (series_plan_id, ordinal) = observe_link_coordinate_v3(link_account)?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        generation,
        *root_account.key,
        false,
        &mut link_value,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Active
            && link.state().phase() == SeriesMarketLinkPhaseV3::Active
            && link.binding().market_binding_id == root_binding_id
            && family.counts().live != 0
            && family.counts().terminal < family.counts().admitted
            && !direct_root_account_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_DIRECT_FAMILY_PRETERMINAL_DOMAIN_V5,
        program_id.as_ref(),
        root_account.key.as_ref(),
        &root_binding_id.bytes(),
        &root.data_id().bytes(),
        &root.authentication_id().bytes(),
        &root.semantic_id().bytes(),
        link_account.key.as_ref(),
        &link.binding_id().bytes(),
        &link.data_id().bytes(),
        &link.authentication_id().bytes(),
        &link.semantic_id().bytes(),
        &link.state().transition_sequence().to_le_bytes(),
        &family_prestate_id.bytes(),
        &direct_root_account_id.bytes(),
        &family.counts().terminal.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductDirectFamilyPreterminalV5 {
        id,
        market_instance_id,
        generation,
        root_account: *root_account.key,
        root_binding_id,
        root_authentication_id: root.authentication_id(),
        root_semantic_id: root.semantic_id(),
        series_link_account: *link_account.key,
        series_link_binding_id: link.binding_id(),
        direct_root_account_id,
        family_prestate_id,
        family_terminal_sequence: family.counts().terminal,
    })
}

/// Product postwrite which owns Direct's unique physical terminal by value.
/// It is the only current path from action13 into the RootV3 Direct counter.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductDirectFamilyTerminalV5 {
    id: ContentId,
    terminal: AuthenticatedDirectFamilyTerminalV3,
    root_account: Pubkey,
    root_data_before_id: ContentId,
    root_data_after_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    series_link_account: Pubkey,
    series_link_data_id: ContentId,
    series_link_authentication_id: ContentId,
    series_link_semantic_id: SeriesMarketLinkV3Id,
    series_link_transition_sequence: u64,
}

impl AuthenticatedProductDirectFamilyTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn terminal_id(&self) -> ContentId { self.terminal.id() }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
}

/// Consume Direct's already-physical terminal exactly once into RootV3.
#[inline(never)]
pub(crate) fn consume_direct_family_terminal_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    terminal: AuthenticatedDirectFamilyTerminalV3,
) -> Outcome<AuthenticatedProductDirectFamilyTerminalV5> {
    let market_instance_id = MarketInstanceV2Id::from_bytes(terminal.market_instance_id().bytes());
    require(
        root_account.key != link_account.key
            && terminal.product_root_account() == account_id(*root_account.key)
            && terminal.series_link_account() == account_id(*link_account.key),
        ClutchError::MismatchedState,
    )?;
    let mut value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        terminal.generation(),
        true,
        &mut value,
    )?;
    let family_prestate_id = root
        .state()
        .product_families()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let family = root.state().product_families().family(MarketFamilyV1::Direct);
    let (series_plan_id, ordinal) = observe_link_coordinate_v3(link_account)?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        terminal.generation(),
        *root_account.key,
        false,
        &mut link_value,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Active
            && root.binding_id() == terminal.product_market_binding_id()
            && link.state().phase() == SeriesMarketLinkPhaseV3::Active
            && link.binding_id() == terminal.series_link_binding_id()
            && link.binding().market_binding_id == root.binding_id()
            && family_prestate_id == terminal.product_family_prestate_id()
            && family.counts().terminal == terminal.family_terminal_sequence()
            && root
                .state()
                .product_families()
                .binding()
                .family_root_id(MarketFamilyV1::Direct)
                == terminal.direct_root_account()
            && !terminal.current_product_authority_id().is_zero(),
        ClutchError::MismatchedState,
    )?;
    let authority = ExactFamilyTerminalAuthorityV5 {
        market_instance_id,
        generation: terminal.generation(),
        family: MarketFamilyV1::Direct,
        family_root_id: terminal.direct_root_account(),
        terminal_sequence: terminal.family_terminal_sequence(),
        terminal_receipt_id: terminal.id(),
    };
    let next = (*root.state())
        .terminalize_product_family_child(
            &authority,
            MarketFamilyV1::Direct,
            terminal.family_terminal_sequence(),
            terminal.id(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data_before_id = root.data_id();
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    let series_link_data_id = link.data_id();
    let series_link_authentication_id = link.authentication_id();
    let series_link_semantic_id = link.semantic_id();
    let series_link_transition_sequence = link.state().transition_sequence();
    drop(link);
    drop(root);
    write_market_lifecycle_root_v3(root_account, &value, &next)?;
    let mut reopened_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        terminal.generation(),
        true,
        &mut reopened_value,
    )?;
    require(reopened.state() == &next, ClutchError::MismatchedState)?;
    let id = hashv(&[
        PRODUCT_DIRECT_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &terminal.id().bytes(),
        &terminal.current_product_authority_id().bytes(),
        root_account.key.as_ref(),
        &root_data_before_id.bytes(),
        &reopened.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &reopened.state().transition_sequence().to_le_bytes(),
        link_account.key.as_ref(),
        &series_link_data_id.bytes(),
        &series_link_authentication_id.bytes(),
        &series_link_semantic_id.bytes(),
        &series_link_transition_sequence.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductDirectFamilyTerminalV5 {
        id,
        terminal,
        root_account: *root_account.key,
        root_data_before_id,
        root_data_after_id: reopened.data_id(),
        root_authentication_before_id,
        root_authentication_after_id: reopened.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: reopened.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: reopened.state().transition_sequence(),
        series_link_account: *link_account.key,
        series_link_data_id,
        series_link_authentication_id,
        series_link_semantic_id,
        series_link_transition_sequence,
    })
}

/// Move-only Product postwrite proving that Fractional's exact a4/a5 physical
/// close was consumed into the current RootV3.  The retained V2 value is the
/// current Fractional-to-Product receipt version; it is not a Product RootV2
/// bridge and cannot be reconstructed from its public IDs.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductFractionalFamilyTerminalV5 {
    id: ContentId,
    terminal: AuthenticatedProductFractionalFamilyTerminalV2,
    physical_terminal_id: ContentId,
    policy_account: Pubkey,
    ledger_account: Pubkey,
    refund_owner: Pubkey,
    neutral_sink: Pubkey,
    policy_lamports_before: u64,
    ledger_lamports_before: u64,
    refund_lamports_before: u64,
    refund_lamports_after: u64,
    neutral_sink_lamports_before: u64,
    neutral_sink_lamports_after: u64,
    link_account: Pubkey,
    link_authentication_id: ContentId,
}

impl AuthenticatedProductFractionalFamilyTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn physical_terminal_id(&self) -> ContentId {
        self.physical_terminal_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.terminal.root_account() }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.terminal.root_authentication_after()
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.terminal.root_semantic_after()
    }
    pub(crate) const fn terminal_receipt_id(&self) -> ContentId {
        self.terminal.terminal_receipt_id()
    }
    pub(crate) const fn policy_terminal_state_id(&self) -> ContentId {
        self.terminal.policy_terminal_state_id()
    }
    pub(crate) const fn ledger_terminal_state_id(&self) -> ContentId {
        self.terminal.ledger_terminal_state_id()
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
}

/// Consume Fractional's sole physical terminal by value and immediately latch
/// its two terminal states into RootV3.  LinkV3 is hostile-authenticated as the
/// exact writable founder link needed by the later same-instruction Series
/// retirement; no Fractional obligation is invented on LinkV3.
#[inline(never)]
pub(crate) fn consume_fractional_family_physical_terminal_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    terminal: AuthenticatedFractionalFamilyPhysicalTerminalV2,
    schedule: &clutch_product_series::MarketFoundationScheduleV4,
    graph: &clutch_product_series::MarketFoundationAccountGraphV4,
) -> Outcome<AuthenticatedProductFractionalFamilyTerminalV5> {
    require(root_account.key != link_account.key, ClutchError::AccountAlias)?;
    let family = terminal.family_terminal();
    let physical_terminal_id = terminal.id();
    let policy_account = terminal.policy_account();
    let ledger_account = terminal.ledger_account();
    let refund_owner = terminal.refund_owner();
    let neutral_sink = terminal.neutral_sink();
    let policy_lamports_before = terminal.policy_lamports_before();
    let ledger_lamports_before = terminal.ledger_lamports_before();
    let refund_lamports_before = terminal.refund_lamports_before();
    let refund_lamports_after = terminal.refund_lamports_after();
    let neutral_sink_lamports_before = terminal.neutral_sink_lamports_before();
    let neutral_sink_lamports_after = terminal.neutral_sink_lamports_after();
    require_live(physical_terminal_id)?;
    let (series_plan_id, ordinal) = observe_link_coordinate_v3(link_account)?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        MarketInstanceV2Id::from_bytes(family.market_instance_id().bytes()),
        family.domain_generation(),
        *root_account.key,
        true,
        &mut link_value,
    )?;
    let link_authentication_id = link.authentication_id();
    let mut root_before = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut root_successor = Box::new(clutch_product_series::MarketLifecycleRootV3::decode_buffer());
    let mut root_after = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let accepted = consume_fractional_terminal_v2(
        program_id,
        root_account,
        terminal,
        &link,
        schedule,
        graph,
        &mut root_before,
        &mut root_successor,
        &mut root_after,
    )?;
    require(
        accepted.terminal_receipt_id() == physical_terminal_id
            && accepted.root_account() == *root_account.key,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_FRACTIONAL_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &physical_terminal_id.bytes(),
        &accepted.id().bytes(),
        root_account.key.as_ref(),
        &accepted.root_authentication_before().bytes(),
        &accepted.root_authentication_after().bytes(),
        &accepted.root_semantic_before().bytes(),
        &accepted.root_semantic_after().bytes(),
        link_account.key.as_ref(),
        &link_authentication_id.bytes(),
        policy_account.as_ref(),
        &policy_lamports_before.to_le_bytes(),
        ledger_account.as_ref(),
        &ledger_lamports_before.to_le_bytes(),
        refund_owner.as_ref(),
        &refund_lamports_before.to_le_bytes(),
        &refund_lamports_after.to_le_bytes(),
        neutral_sink.as_ref(),
        &neutral_sink_lamports_before.to_le_bytes(),
        &neutral_sink_lamports_after.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductFractionalFamilyTerminalV5 {
        id,
        terminal: accepted,
        physical_terminal_id,
        policy_account,
        ledger_account,
        refund_owner,
        neutral_sink,
        policy_lamports_before,
        ledger_lamports_before,
        refund_lamports_before,
        refund_lamports_after,
        neutral_sink_lamports_before,
        neutral_sink_lamports_after,
        link_account: *link_account.key,
        link_authentication_id,
    })
}

/// Move-only Product preauthorization proving that the exact hostile Failure
/// owner and Active RootV3/LinkV3 tuple may enter Source custody retirement.
/// It deliberately does not latch the Failure shared-core slot: the later
/// physical Failure close owns that Retiring-only transition.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductFailureCoreTerminalV5 {
    id: ContentId,
    failure_owner: AuthenticatedFailureMarketFamilyTerminalOwnerV2,
    failure_facts: FailureMarketFamilyTerminalConsumerFactsV3,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_data_before_id: ContentId,
    root_data_after_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    source_retirement_preauthorization_id: ContentId,
    link_account: Pubkey,
    link_authentication_id: ContentId,
    link_data_id: ContentId,
    link_semantic_id: SeriesMarketLinkV3Id,
}

/// Copy-only transcript left after the unique Failure owner moves onward into
/// Source custody retirement.  It proves the Product postwrite but cannot
/// authorize another Failure or Source transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductFailureCoreTerminalFactsV5 {
    pub(crate) id: ContentId,
    pub(crate) failure: FailureMarketFamilyTerminalConsumerFactsV3,
    pub(crate) root_account: Pubkey,
    pub(crate) root_binding_id: ContentId,
    pub(crate) root_data_before_id: ContentId,
    pub(crate) root_data_after_id: ContentId,
    pub(crate) root_authentication_before_id: ContentId,
    pub(crate) root_authentication_after_id: ContentId,
    pub(crate) root_semantic_before_id: ContentId,
    pub(crate) root_semantic_after_id: ContentId,
    pub(crate) root_transition_sequence_before: u64,
    pub(crate) root_transition_sequence_after: u64,
    pub(crate) source_retirement_preauthorization_id: ContentId,
    pub(crate) link_account: Pubkey,
    pub(crate) link_authentication_id: ContentId,
    pub(crate) link_data_id: ContentId,
    pub(crate) link_semantic_id: SeriesMarketLinkV3Id,
}

impl AuthenticatedProductFailureCoreTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn source_retirement_preauthorization_id(&self) -> ContentId {
        self.source_retirement_preauthorization_id
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_id(&self) -> ContentId {
        self.link_authentication_id
    }
    pub(crate) const fn link_data_id(&self) -> ContentId { self.link_data_id }
    pub(crate) const fn link_semantic_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_id
    }
    pub(crate) const fn failure_facts(&self) -> FailureMarketFamilyTerminalConsumerFactsV3 {
        self.failure_facts
    }

    /// Move the unique Failure owner onward only after the RootV3 postwrite
    /// has been hostile-reopened.  The Copy facts remain the exact Product
    /// transcript input; no second Failure capability is minted.
    pub(crate) fn into_source_parts(
        self,
    ) -> (
        ProductFailureCoreTerminalFactsV5,
        AuthenticatedFailureMarketFamilyTerminalOwnerV2,
    ) {
        (
            ProductFailureCoreTerminalFactsV5 {
                id: self.id,
                failure: self.failure_facts,
                root_account: self.root_account,
                root_binding_id: self.root_binding_id,
                root_data_before_id: self.root_data_before_id,
                root_data_after_id: self.root_data_after_id,
                root_authentication_before_id: self.root_authentication_before_id,
                root_authentication_after_id: self.root_authentication_after_id,
                root_semantic_before_id: self.root_semantic_before_id,
                root_semantic_after_id: self.root_semantic_after_id,
                root_transition_sequence_before: self.root_transition_sequence_before,
                root_transition_sequence_after: self.root_transition_sequence_after,
                source_retirement_preauthorization_id:
                    self.source_retirement_preauthorization_id,
                link_account: self.link_account,
                link_authentication_id: self.link_authentication_id,
                link_data_id: self.link_data_id,
                link_semantic_id: self.link_semantic_id,
            },
            self.failure_owner,
        )
    }
}

/// Bind the durable Failure-family terminal to the live Product/Source tuple.
/// RootV3 and LinkV3 are hostile-authenticated but not mutated: Source consumes
/// this move-only authority first, and only the later physical Failure receipt
/// can latch the Retiring RootV3 shared-core slot.
#[inline(never)]
pub(crate) fn consume_failure_family_terminal_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    failure: AuthenticatedFailureMarketFamilyTerminalReceiptV3,
) -> Outcome<AuthenticatedProductFailureCoreTerminalV5> {
    require(root_account.key != link_account.key, ClutchError::AccountAlias)?;
    let failure_id = failure.id();
    let (facts, failure_owner) = failure.into_product_v3_parts();
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        facts.market_instance_id,
        facts.generation,
        true,
        &mut root_value,
    )?;
    let binding = *root.binding();
    let root_binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Active
            && root.state().failure_terminal_receipt_id().is_zero()
            && facts.source_product_link_account_id == account_id(*link_account.key),
        ClutchError::MismatchedState,
    )?;
    let (series_plan_id, ordinal) = observe_link_coordinate_v3(link_account)?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        facts.market_instance_id,
        facts.generation,
        *root_account.key,
        true,
        &mut link_value,
    )?;
    require(
        link.state().phase() == SeriesMarketLinkPhaseV3::Active
            && link.binding().market_binding_id == root_binding_id,
        ClutchError::MismatchedState,
    )?;
    let root_data_before_id = root.data_id();
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    let link_authentication_id = link.authentication_id();
    let link_data_id = link.data_id();
    let link_semantic_id = link.semantic_id();
    let source_retirement_preauthorization_id = hashv(&[
        PRODUCT_FAILURE_CORE_TERMINAL_POSTWRITE_DOMAIN_V5,
        b"source-retirement-preauthorization",
        program_id.as_ref(),
        &failure_id.bytes(),
        root_account.key.as_ref(),
        &root_binding_id.bytes(),
        &root_data_before_id.bytes(),
        &root_authentication_before_id.bytes(),
        &root_semantic_before_id.bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        link_account.key.as_ref(),
        &link_authentication_id.bytes(),
        &link_data_id.bytes(),
        &link_semantic_id.bytes(),
    ]);
    require_live(source_retirement_preauthorization_id)?;
    drop(link);
    drop(root);
    let id = hashv(&[
        PRODUCT_FAILURE_CORE_TERMINAL_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &failure_id.bytes(),
        root_account.key.as_ref(),
        &root_binding_id.bytes(),
        &root_data_before_id.bytes(),
        &root_data_before_id.bytes(),
        &root_authentication_before_id.bytes(),
        &root_authentication_before_id.bytes(),
        &root_semantic_before_id.bytes(),
        &root_semantic_before_id.bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &source_retirement_preauthorization_id.bytes(),
        link_account.key.as_ref(),
        &link_authentication_id.bytes(),
        &link_data_id.bytes(),
        &link_semantic_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductFailureCoreTerminalV5 {
        id,
        failure_owner,
        failure_facts: facts,
        root_account: *root_account.key,
        root_binding_id,
        root_data_before_id,
        root_data_after_id: root_data_before_id,
        root_authentication_before_id,
        root_authentication_after_id: root_authentication_before_id,
        root_semantic_before_id,
        root_semantic_after_id: root_semantic_before_id,
        root_transition_sequence_before,
        root_transition_sequence_after: root_transition_sequence_before,
        source_retirement_preauthorization_id,
        link_account: *link_account.key,
        link_authentication_id,
        link_data_id,
        link_semantic_id,
    })
}

/// Move-only RootV3 postwrite proving that Failure's four deletable accounts
/// were physically closed before the Retiring shared-core slot was latched.
/// RootV3 stores the projection identity, never the physical receipt ID.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductFailurePhysicalTerminalV5 {
    id: ContentId,
    physical: FailureMarketPhysicalTerminalConsumerFactsV3,
    shared_core_projection_id: ContentId,
    root_account: Pubkey,
    root_data_before_id: ContentId,
    root_data_after_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
}

impl AuthenticatedProductFailurePhysicalTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn physical_id(&self) -> ContentId { self.physical.id }
    pub(crate) const fn shared_core_projection_id(&self) -> ContentId {
        self.shared_core_projection_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
}

/// Consume Failure's final physical receipt and latch exactly one shared-core
/// projection into the already-Retiring Product RootV3.
#[inline(never)]
pub(crate) fn consume_failure_market_physical_terminal_v5<'root, A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    terminal: A,
) -> Outcome<AuthenticatedProductFailurePhysicalTerminalV5>
where
    A: AuthenticatedFailureMarketPhysicalTerminalAuthorityV3<'root>,
{
    let (root, physical) =
        terminal.into_authenticated_failure_market_physical_terminal_v3()?;
    let failure = physical.failure_terminal_facts;
    require(
        root.account() == *root_account.key
            && root.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV3::Retiring
            && root.state().failure_terminal_receipt_id().is_zero()
            && root.data_id() == physical.market_root_data_before_id
            && root.semantic_id() == physical.market_root_semantic_before_id
            && root.binding_id() == physical.market_root_binding_id
            && root.authentication_id() == physical.market_root_authentication_before_id
            && root.state().transition_sequence() == physical.market_root_transition_sequence
            && physical.refunded_principal_lamports != 0
            && physical.rent_refund_balance_after_lamports
                == physical
                    .rent_refund_balance_before_lamports
                    .checked_add(physical.refunded_principal_lamports)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && physical.neutral_sink_balance_after_lamports
                == physical
                    .neutral_sink_balance_before_lamports
                    .checked_add(physical.neutralized_donation_lamports)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    let sequence_after = root
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let projection = MarketSharedCoreTerminalProjectionV3::new(
        *root.binding(),
        MarketSharedCoreV3::Failure,
        failure.owner_account_id,
        failure.owner_release_id,
        physical.id,
        sequence_after,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let next = (*root.state())
        .consume_shared_core_terminal(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data_before_id = root.data_id();
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    write_market_lifecycle_root_v3(root_account, root.value(), &next)?;
    drop(root);
    let mut reopened_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        failure.market_instance_id,
        failure.generation,
        true,
        &mut reopened_value,
    )?;
    require(
        reopened.state() == &next
            && reopened.state().failure_terminal_receipt_id() == projection.id()
            && reopened.state().transition_sequence() == sequence_after,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_FAILURE_PHYSICAL_TERMINAL_LATCH_DOMAIN_V5,
        program_id.as_ref(),
        &physical.id.bytes(),
        &physical.family_seal_id.bytes(),
        &physical.interval_close_authorization_id.bytes(),
        &projection.id().bytes(),
        root_account.key.as_ref(),
        &root_data_before_id.bytes(),
        &reopened.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &sequence_after.to_le_bytes(),
        physical.rent_refund_owner.as_ref(),
        &physical.rent_refund_balance_before_lamports.to_le_bytes(),
        &physical.rent_refund_balance_after_lamports.to_le_bytes(),
        physical.neutral_sink.as_ref(),
        &physical.neutral_sink_balance_before_lamports.to_le_bytes(),
        &physical.neutral_sink_balance_after_lamports.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductFailurePhysicalTerminalV5 {
        id,
        physical,
        shared_core_projection_id: projection.id(),
        root_account: *root_account.key,
        root_data_before_id,
        root_data_after_id: reopened.data_id(),
        root_authentication_before_id,
        root_authentication_after_id: reopened.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: reopened.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: sequence_after,
    })
}

struct ExactFamilyTerminalAuthorityV5 {
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    family: MarketFamilyV1,
    family_root_id: ContentId,
    terminal_sequence: u32,
    terminal_receipt_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactFamilyTerminalAuthorityV5 {
    fn authenticate_terminal(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if family != self.family
            || current.binding().market_instance_id != self.market_instance_id
            || current.binding().generation != self.generation
            || family_root_id != self.family_root_id
            || family_terminal_sequence != self.terminal_sequence
            || terminal_receipt_id != self.terminal_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Move-only postwrite proving that Structured's physical terminal was
/// consumed once into both its Market family count and the exact per-Series
/// Structured/Wrapper obligation pair.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductStructuredFamilyTerminalV5 {
    id: ContentId,
    terminal: AuthenticatedStructuredWrapperFamilyTerminalV3,
    root_account: Pubkey,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    link_account: Pubkey,
    link_authentication_before_id: ContentId,
    link_authentication_after_id: ContentId,
    link_data_before_id: ContentId,
    link_data_after_id: ContentId,
    link_semantic_before_id: SeriesMarketLinkV3Id,
    link_semantic_after_id: SeriesMarketLinkV3Id,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
    structured_obligation_projection_id: ContentId,
    wrapper_obligation_projection_id: ContentId,
}

impl AuthenticatedProductStructuredFamilyTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_after_id(&self) -> ContentId {
        self.link_authentication_after_id
    }
    pub(crate) const fn link_data_after_id(&self) -> ContentId { self.link_data_after_id }
    pub(crate) const fn link_semantic_after_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after_id
    }
    pub(crate) const fn link_transition_sequence_after(&self) -> u64 {
        self.link_transition_sequence_after
    }
    pub(crate) const fn structured_obligation_projection_id(&self) -> ContentId {
        self.structured_obligation_projection_id
    }
    pub(crate) const fn wrapper_obligation_projection_id(&self) -> ContentId {
        self.wrapper_obligation_projection_id
    }
    pub(crate) const fn terminal_id(&self) -> ContentId { self.terminal.id() }
}

/// Consume Structured's already-physical terminal into the current RootV3 and
/// LinkV3.  The receipt's retained preterminal Product tuple must equal the
/// hostile live accounts before either successor is written.
#[inline(never)]
pub(crate) fn consume_structured_family_terminal_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    terminal: AuthenticatedStructuredWrapperFamilyTerminalV3,
) -> Outcome<AuthenticatedProductStructuredFamilyTerminalV5> {
    require(
        root_account.key != link_account.key
            && terminal.product_root_account() == *root_account.key
            && terminal.product_link_account() == *link_account.key,
        ClutchError::AccountAlias,
    )?;
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        terminal.market_instance_id(),
        terminal.generation(),
        true,
        &mut root_value,
    )?;
    let root_binding_id = root
        .binding()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        terminal.series_plan_id(),
        terminal.ordinal(),
        terminal.market_instance_id(),
        terminal.generation(),
        *root_account.key,
        true,
        &mut link_value,
    )?;
    require(
        root_binding_id == terminal.product_market_binding_id()
            && link.binding_id() == terminal.product_link_binding_id()
            && link.authentication_id() == terminal.product_link_authentication_id()
            && link.data_id() == terminal.product_link_data_id()
            && link.semantic_id().content_id() == terminal.product_link_semantic_id()
            && link.state().transition_sequence()
                == terminal.product_link_transition_sequence()
            && link.state().phase() == SeriesMarketLinkPhaseV3::Active,
        ClutchError::MismatchedState,
    )?;

    let family = root.state().product_families().family(MarketFamilyV1::Structured);
    let terminal_sequence = family.counts().terminal;
    let authority = ExactFamilyTerminalAuthorityV5 {
        market_instance_id: terminal.market_instance_id(),
        generation: terminal.generation(),
        family: MarketFamilyV1::Structured,
        family_root_id: root
            .state()
            .product_families()
            .binding()
            .family_root_id(MarketFamilyV1::Structured),
        terminal_sequence,
        terminal_receipt_id: terminal.id(),
    };
    let root_next = (*root.state())
        .terminalize_product_family_child(
            &authority,
            MarketFamilyV1::Structured,
            terminal_sequence,
            terminal.id(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let structured_projection = SeriesLinkObligationTerminalProjectionV3 {
        link_semantic_id: link.semantic_id(),
        obligation: SeriesLinkObligationV3::Structured,
        disposition: SeriesLinkObligationDispositionV3::Terminal,
        link_transition_sequence: link
            .state()
            .transition_sequence()
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        owner_terminal_receipt_id: terminal.aggregate_terminal_receipt_id(),
    };
    let link_after_structured = (*link.state())
        .consume_obligation(structured_projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let wrapper_projection = SeriesLinkObligationTerminalProjectionV3 {
        link_semantic_id: link_after_structured
            .semantic_id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        obligation: SeriesLinkObligationV3::Wrapper,
        disposition: SeriesLinkObligationDispositionV3::Terminal,
        link_transition_sequence: link_after_structured
            .transition_sequence()
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        owner_terminal_receipt_id: terminal.descriptor_terminal_receipt_id(),
    };
    let link_next = link_after_structured
        .consume_obligation(wrapper_projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    let link_authentication_before_id = link.authentication_id();
    let link_data_before_id = link.data_id();
    let link_semantic_before_id = link.semantic_id();
    let link_transition_sequence_before = link.state().transition_sequence();
    drop(link);
    drop(root);
    write_market_lifecycle_root_v3(root_account, &root_value, &root_next)?;
    write_series_market_link_v3(link_account, &link_value, &link_next)?;

    let mut reopened_root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened_root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        terminal.market_instance_id(),
        terminal.generation(),
        true,
        &mut reopened_root_value,
    )?;
    let mut reopened_link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let reopened_link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        terminal.series_plan_id(),
        terminal.ordinal(),
        terminal.market_instance_id(),
        terminal.generation(),
        *root_account.key,
        true,
        &mut reopened_link_value,
    )?;
    let structured_projection_id = structured_projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let wrapper_projection_id = wrapper_projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        reopened_root.state() == &root_next
            && reopened_link.state() == &link_next
            && reopened_link
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV3::Structured)
                == structured_projection_id
            && reopened_link
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV3::Wrapper)
                == wrapper_projection_id,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_STRUCTURED_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &terminal.id().bytes(),
        root_account.key.as_ref(),
        &root_authentication_before_id.bytes(),
        &reopened_root.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened_root.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &reopened_root.state().transition_sequence().to_le_bytes(),
        link_account.key.as_ref(),
        &link_authentication_before_id.bytes(),
        &reopened_link.authentication_id().bytes(),
        &link_data_before_id.bytes(),
        &reopened_link.data_id().bytes(),
        &link_semantic_before_id.bytes(),
        &reopened_link.semantic_id().bytes(),
        &link_transition_sequence_before.to_le_bytes(),
        &reopened_link.state().transition_sequence().to_le_bytes(),
        &structured_projection_id.bytes(),
        &wrapper_projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductStructuredFamilyTerminalV5 {
        id,
        terminal,
        root_account: *root_account.key,
        root_authentication_before_id,
        root_authentication_after_id: reopened_root.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: reopened_root.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: reopened_root.state().transition_sequence(),
        link_account: *link_account.key,
        link_authentication_before_id,
        link_authentication_after_id: reopened_link.authentication_id(),
        link_data_before_id,
        link_data_after_id: reopened_link.data_id(),
        link_semantic_before_id,
        link_semantic_after_id: reopened_link.semantic_id(),
        link_transition_sequence_before,
        link_transition_sequence_after: reopened_link.state().transition_sequence(),
        structured_obligation_projection_id: structured_projection_id,
        wrapper_obligation_projection_id: wrapper_projection_id,
    })
}

/// Move-only Product-owned proof that an immutable absent liquidity
/// obligation was consumed once.  It is separate from Dealer's physical
/// terminal: no Dealer receipt exists when the capability was disabled or the
/// attachment was never founded.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductLiquidityObligationAbsenceV5 {
    id: ContentId,
    registry_capability_id: ContentId,
    root_account: Pubkey,
    root_authentication_id: ContentId,
    link_account: Pubkey,
    link_authentication_before_id: ContentId,
    link_authentication_after_id: ContentId,
    link_semantic_before_id: SeriesMarketLinkV3Id,
    link_semantic_after_id: SeriesMarketLinkV3Id,
    projection_id: ContentId,
}

impl AuthenticatedProductLiquidityObligationAbsenceV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_after_id(&self) -> ContentId {
        self.link_authentication_after_id
    }
    pub(crate) const fn link_semantic_after_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after_id
    }
    pub(crate) const fn projection_id(&self) -> ContentId { self.projection_id }
}

/// Consume canonical absence for Liquidity directly from the hostile current
/// Registry/Bundle/RootV3/LinkV3 graph.  A Live obligation is categorically
/// refused and must instead consume Dealer's move-only physical value receipt.
#[inline(never)]
pub(crate) fn consume_absent_liquidity_obligation_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
) -> Outcome<AuthenticatedProductLiquidityObligationAbsenceV5> {
    require(root_account.key != link_account.key, ClutchError::AccountAlias)?;
    artifacts.validate_registry_projection(&registry.projection())?;
    let mut observed_link = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    {
        let data = link_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV3::decode_into(&data, &mut observed_link)?;
    }
    let observed_binding = *observed_link.state.binding_ref();
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        observed_binding.market_instance_id,
        observed_binding.generation,
        true,
        &mut root_value,
    )?;
    let root_binding_id = root.binding_id();
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        observed_binding.series_plan_id,
        observed_binding.ordinal,
        observed_binding.market_instance_id,
        observed_binding.generation,
        *root_account.key,
        true,
        &mut link_value,
    )?;
    let series_plan_id = artifacts
        .series()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let quote_id = artifacts
        .quote()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = artifacts
        .attachment()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        registry.activation_consumed()
            && registry.series_plan_id() == series_plan_id
            && registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && bundle.bundle().series_plan_id == series_plan_id
            && bundle.bundle().funding_terms_id == funding_terms_id
            && bundle.bundle().funding_quote_id == quote_id
            && bundle.bundle().attachment_plan_id == attachment_id
            && root.state().phase() == MarketLifecyclePhaseV3::Active
            && root.binding().product_template_id
                == bundle.bundle().product_template_id.content_id()
            && root.binding().market_genesis_profile_id
                == bundle.bundle().market_genesis_profile_id.content_id()
            && root.binding().capability_profile_id == registry.capability_profile_id()
            && link.state().phase() == SeriesMarketLinkPhaseV3::Active
            && link.binding().market_binding_id == root_binding_id
            && link.binding().series_plan_id == series_plan_id
            && link.binding().funding_terms_id == funding_terms_id
            && link.binding().funding_quote_id == quote_id
            && link.binding().attachment_plan_id == attachment_id
            && link.binding().compiler_bundle_id == bundle.bundle_id()
            && link.binding().capability_profile_id == registry.capability_profile_id(),
        ClutchError::MismatchedState,
    )?;
    let projection = liquidity_absence_projection_v5(program_id, root_binding_id, link.state())?;
    let next = (*link.state())
        .consume_obligation(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_authentication_id = root.authentication_id();
    let link_authentication_before_id = link.authentication_id();
    let link_semantic_before_id = link.semantic_id();
    drop(link);
    drop(root);
    write_series_market_link_v3(link_account, &link_value, &next)?;
    let mut reopened_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let reopened = authenticate_series_market_link_v3(
        program_id,
        link_account,
        observed_binding.series_plan_id,
        observed_binding.ordinal,
        observed_binding.market_instance_id,
        observed_binding.generation,
        *root_account.key,
        true,
        &mut reopened_value,
    )?;
    let projection_id = projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        reopened.state() == &next
            && reopened
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV3::Liquidity)
                == projection_id,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_LIQUIDITY_OBLIGATION_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &registry.id().bytes(),
        root_account.key.as_ref(),
        &root_authentication_id.bytes(),
        link_account.key.as_ref(),
        &link_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &link_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductLiquidityObligationAbsenceV5 {
        id,
        registry_capability_id: registry.id(),
        root_account: *root_account.key,
        root_authentication_id,
        link_account: *link_account.key,
        link_authentication_before_id,
        link_authentication_after_id: reopened.authentication_id(),
        link_semantic_before_id,
        link_semantic_after_id: reopened.semantic_id(),
        projection_id,
    })
}

/// Move-only postwrite proving that Dealer's complete physical terminal was
/// consumed into both the Dealer family counter and this Series' Dealer
/// obligation.  The Dealer receipt remains owned here and cannot authorize a
/// second RootV3 or LinkV3 transition.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductDealerFamilyTerminalV5 {
    id: ContentId,
    terminal: AuthenticatedDealerFamilyTerminalReceiptV1,
    root_account: Pubkey,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    link_account: Pubkey,
    link_authentication_before_id: ContentId,
    link_authentication_after_id: ContentId,
    link_data_before_id: ContentId,
    link_data_after_id: ContentId,
    link_semantic_before_id: SeriesMarketLinkV3Id,
    link_semantic_after_id: SeriesMarketLinkV3Id,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
    dealer_obligation_projection_id: ContentId,
    liquidity_obligation_projection_id: ContentId,
}

impl AuthenticatedProductDealerFamilyTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn terminal_id(&self) -> ContentId { self.terminal.id() }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_after_id(&self) -> ContentId {
        self.link_authentication_after_id
    }
    pub(crate) const fn link_data_after_id(&self) -> ContentId { self.link_data_after_id }
    pub(crate) const fn link_semantic_after_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after_id
    }
    pub(crate) const fn link_transition_sequence_after(&self) -> u64 {
        self.link_transition_sequence_after
    }
    pub(crate) const fn dealer_obligation_projection_id(&self) -> ContentId {
        self.dealer_obligation_projection_id
    }
    pub(crate) const fn liquidity_obligation_projection_id(&self) -> ContentId {
        self.liquidity_obligation_projection_id
    }
}

/// Consume Dealer's same-instruction physical terminal into current Product
/// state.  Every immutable Product coordinate retained by Dealer is compared
/// to the hostile RootV3/LinkV3 graph before either postwrite occurs.
#[inline(never)]
pub(crate) fn consume_dealer_family_terminal_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    terminal: AuthenticatedDealerFamilyTerminalReceiptV1,
) -> Outcome<AuthenticatedProductDealerFamilyTerminalV5> {
    require(
        root_account.key != link_account.key
            && terminal.product_market_root_account() == *root_account.key
            && terminal.series_market_link_account() == *link_account.key,
        ClutchError::AccountAlias,
    )?;
    let market_instance_id = MarketInstanceV2Id::from_bytes(terminal.market_instance_id().bytes());
    let series_plan_id = SeriesPlanV5Id::from_bytes(terminal.series_plan_id().bytes());
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        terminal.product_generation(),
        true,
        &mut root_value,
    )?;
    let root_binding_id = root
        .binding()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        terminal.series_ordinal(),
        market_instance_id,
        terminal.product_generation(),
        *root_account.key,
        true,
        &mut link_value,
    )?;
    let link_binding = link.binding();
    require(
        root_binding_id == terminal.product_market_binding_id()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.compiler_bundle_id.content_id() == terminal.compiler_bundle_id()
            && link_binding.attachment_plan_id.content_id() == terminal.attachment_plan_id()
            && root.state().resolution_semantic_id() == terminal.resolution_semantic_id()
            && root.state().resolution_data_id() == terminal.resolution_data_id()
            && link.state().phase() == SeriesMarketLinkPhaseV3::Active,
        ClutchError::MismatchedState,
    )?;

    let family_slot = root.state().product_families().family(MarketFamilyV1::Dealer);
    let terminal_sequence = family_slot.counts().terminal;
    let authority = ExactFamilyTerminalAuthorityV5 {
        market_instance_id,
        generation: terminal.product_generation(),
        family: MarketFamilyV1::Dealer,
        family_root_id: root
            .state()
            .product_families()
            .binding()
            .family_root_id(MarketFamilyV1::Dealer),
        terminal_sequence,
        terminal_receipt_id: terminal.id(),
    };
    let root_next = (*root.state())
        .terminalize_product_family_child(
            &authority,
            MarketFamilyV1::Dealer,
            terminal_sequence,
            terminal.id(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let dealer_projection = SeriesLinkObligationTerminalProjectionV3 {
        link_semantic_id: link.semantic_id(),
        obligation: SeriesLinkObligationV3::Dealer,
        disposition: SeriesLinkObligationDispositionV3::Terminal,
        link_transition_sequence: link
            .state()
            .transition_sequence()
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        owner_terminal_receipt_id: terminal.dealer_obligation_close_receipt_id(),
    };
    let link_after_dealer = (*link.state())
        .consume_obligation(dealer_projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let liquidity_projection = match link_after_dealer
        .obligation_status(SeriesLinkObligationV3::Liquidity)
    {
        SeriesLinkObligationStatusV3::Live => {
            let owner_terminal_receipt_id = terminal.value_terminal_receipt_id();
            require_live(owner_terminal_receipt_id)?;
            SeriesLinkObligationTerminalProjectionV3 {
                link_semantic_id: link_after_dealer
                    .semantic_id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
                obligation: SeriesLinkObligationV3::Liquidity,
                disposition: SeriesLinkObligationDispositionV3::Terminal,
                link_transition_sequence: link_after_dealer
                    .transition_sequence()
                    .checked_add(1)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
                owner_terminal_receipt_id,
            }
        }
        SeriesLinkObligationStatusV3::CapabilityDisabled
        | SeriesLinkObligationStatusV3::EnabledNeverFounded => {
            liquidity_absence_projection_v5(program_id, root_binding_id, &link_after_dealer)?
        }
        SeriesLinkObligationStatusV3::Terminal => {
            return Err(Refusal::Adapter(ClutchError::Replay));
        }
    };
    let link_next = link_after_dealer
        .consume_obligation(liquidity_projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    let link_authentication_before_id = link.authentication_id();
    let link_data_before_id = link.data_id();
    let link_semantic_before_id = link.semantic_id();
    let link_transition_sequence_before = link.state().transition_sequence();
    drop(link);
    drop(root);
    write_market_lifecycle_root_v3(root_account, &root_value, &root_next)?;
    write_series_market_link_v3(link_account, &link_value, &link_next)?;

    let mut reopened_root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened_root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        terminal.product_generation(),
        true,
        &mut reopened_root_value,
    )?;
    let mut reopened_link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let reopened_link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        terminal.series_ordinal(),
        market_instance_id,
        terminal.product_generation(),
        *root_account.key,
        true,
        &mut reopened_link_value,
    )?;
    let dealer_projection_id = dealer_projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let liquidity_projection_id = liquidity_projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        reopened_root.state() == &root_next
            && reopened_link.state() == &link_next
            && reopened_link
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV3::Dealer)
                == dealer_projection_id
            && reopened_link
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV3::Liquidity)
                == liquidity_projection_id,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        b"dragons-clutch/sbf/product-dealer-family-terminal-postwrite/v5\0",
        program_id.as_ref(),
        &terminal.id().bytes(),
        root_account.key.as_ref(),
        &root_authentication_before_id.bytes(),
        &reopened_root.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened_root.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &reopened_root.state().transition_sequence().to_le_bytes(),
        link_account.key.as_ref(),
        &link_authentication_before_id.bytes(),
        &reopened_link.authentication_id().bytes(),
        &link_data_before_id.bytes(),
        &reopened_link.data_id().bytes(),
        &link_semantic_before_id.bytes(),
        &reopened_link.semantic_id().bytes(),
        &link_transition_sequence_before.to_le_bytes(),
        &reopened_link.state().transition_sequence().to_le_bytes(),
        &dealer_projection_id.bytes(),
        &liquidity_projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductDealerFamilyTerminalV5 {
        id,
        terminal,
        root_account: *root_account.key,
        root_authentication_before_id,
        root_authentication_after_id: reopened_root.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: reopened_root.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: reopened_root.state().transition_sequence(),
        link_account: *link_account.key,
        link_authentication_before_id,
        link_authentication_after_id: reopened_link.authentication_id(),
        link_data_before_id,
        link_data_after_id: reopened_link.data_id(),
        link_semantic_before_id,
        link_semantic_after_id: reopened_link.semantic_id(),
        link_transition_sequence_before,
        link_transition_sequence_after: reopened_link.state().transition_sequence(),
        dealer_obligation_projection_id: dealer_projection_id,
        liquidity_obligation_projection_id: liquidity_projection_id,
    })
}

/// Move-only Product receipt left after the final Source custody is physically
/// closed and its exact LinkV3 retirement projection has been consumed by the
/// live RootV3.  It retains Source's sole move-only Market shared-core owner;
/// the later Product RootV3 retirement step must consume that owner by value.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSourceSeriesRetirementV5 {
    id: ContentId,
    failure: ProductFailureCoreTerminalFactsV5,
    source_market_terminal: Option<AuthenticatedSourceMarketSharedCoreTerminalV3>,
    source_market_terminal_id: ContentId,
    source_market_terminal_facts: SourceMarketSharedCoreTerminalFactsV3,
    source_projection: Option<SourceFamilyTerminalProjectionV3>,
    source_shared_core_projection_id: ContentId,
    root_account: Pubkey,
    root_data_before_id: ContentId,
    root_data_after_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    link_account: Pubkey,
    link_data_before_id: ContentId,
    link_data_after_id: ContentId,
    link_authentication_before_id: ContentId,
    link_authentication_after_id: ContentId,
    link_semantic_before_id: SeriesMarketLinkV3Id,
    link_semantic_after_id: SeriesMarketLinkV3Id,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
}

impl AuthenticatedProductSourceSeriesRetirementV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn failure(&self) -> ProductFailureCoreTerminalFactsV5 {
        self.failure
    }
    pub(crate) const fn source_market_terminal_id(&self) -> ContentId {
        self.source_market_terminal_id
    }
    pub(crate) const fn source_market_terminal_facts(
        &self,
    ) -> SourceMarketSharedCoreTerminalFactsV3 {
        self.source_market_terminal_facts
    }
    pub(crate) fn source_projection(
        &self,
    ) -> Outcome<SourceFamilyTerminalProjectionV3> {
        self.source_projection
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))
    }
    pub(crate) const fn source_shared_core_projection_id(&self) -> ContentId {
        self.source_shared_core_projection_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_authentication_after_id(&self) -> ContentId {
        self.link_authentication_after_id
    }
    pub(crate) const fn link_semantic_after_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after_id
    }
    pub(crate) const fn link_transition_sequence_after(&self) -> u64 {
        self.link_transition_sequence_after
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn retire_source_and_count_series_link_v5<A>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    funding_account: &AccountInfo<'_>,
    failure: ProductFailureCoreTerminalFactsV5,
    source_authority: A,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductSourceSeriesRetirementV5>
where
    A: AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1,
{
    require(
        failure.root_account == *root_account.key
            && failure.link_account == *link_account.key
            && root_account.key != link_account.key
            && root_account.key != funding_account.key
            && link_account.key != funding_account.key,
        ClutchError::AccountAlias,
    )?;
    let market_instance_id = failure.failure.market_instance_id;
    let generation = failure.failure.generation;
    let (series_plan_id, ordinal) = observe_link_coordinate_v3(link_account)?;
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        generation,
        true,
        &mut root_value,
    )?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        generation,
        *root_account.key,
        true,
        &mut link_value,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Active
            && root.binding_id() == failure.root_binding_id
            && root.data_id() == failure.root_data_after_id
            && root.authentication_id() == failure.root_authentication_after_id
            && root.semantic_id() == failure.root_semantic_after_id
            && root.state().transition_sequence() == failure.root_transition_sequence_after
            && link.authentication_id() == failure.link_authentication_id
            && link.data_id() == failure.link_data_id
            && link.semantic_id() == failure.link_semantic_id
            && link.state().phase() == SeriesMarketLinkPhaseV3::Active,
        ClutchError::MismatchedState,
    )?;
    let mut retiring_link = clutch_product_series::SeriesMarketLinkV3::decode_buffer();
    link.state()
        .begin_retirement_into(&mut retiring_link)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(link);
    drop(root);
    write_series_market_link_v3(link_account, &link_value, &retiring_link)?;

    let mut retiring_root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let retiring_root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        generation,
        true,
        &mut retiring_root_value,
    )?;
    let mut retiring_link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let retiring_link_auth = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        generation,
        *root_account.key,
        true,
        &mut retiring_link_value,
    )?;
    require(
        retiring_link_auth.state() == &retiring_link
            && retiring_link_auth.state().phase() == SeriesMarketLinkPhaseV3::Retiring,
        ClutchError::MismatchedState,
    )?;
    let custody = super::super::source_plane_v3_actions::authenticate_source_funding_custody_v1(
        program_id,
        route,
        schedule,
        custody_account,
    )?;
    let lifecycle = authenticate_source_family_terminal_authority_v3(
        source_authority,
        route,
        schedule,
        &retiring_link_auth,
        custody,
    )?;
    let funding = super::authenticate_series_funding_account_v5(
        program_id,
        funding_account,
        series_plan_id,
        false,
    )?;
    let source = retire_source_funding_custody_v3(
        program_id,
        route,
        schedule,
        lifecycle,
        &retiring_link_auth,
        &funding,
        custody_account,
        principal_refund,
        neutral_sink,
        system_program,
    )?;
    require(
        source.facts().funding_account.bytes() == funding.account().to_bytes()
            && source.facts().funding_state_id.bytes()
                == funding
                    .state()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .bytes()
            && source.facts().funding_account_data_id == funding.data_id()
            && source.facts().funding_account_authentication_id == funding.authentication_id(),
        ClutchError::MismatchedState,
    )?;
    let root_data_before_id = retiring_root.data_id();
    let root_authentication_before_id = retiring_root.authentication_id();
    let root_semantic_before_id = retiring_root.semantic_id();
    let root_transition_sequence_before = retiring_root.state().transition_sequence();
    let link_data_before_id = retiring_link_auth.data_id();
    let link_authentication_before_id = retiring_link_auth.authentication_id();
    let link_semantic_before_id = retiring_link_auth.semantic_id();
    let link_transition_sequence_before = retiring_link_auth.state().transition_sequence();
    let mut root_successor = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut link_successor = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let mut root_reopen = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut link_reopen = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let source_market_terminal = consume_source_family_terminal_into_product_v3(
        program_id,
        root_account,
        link_account,
        retiring_root,
        retiring_link_auth,
        source,
        &mut root_successor,
        &mut link_successor,
        &mut root_reopen,
        &mut link_reopen,
    )?;
    let final_root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        generation,
        true,
        &mut root_reopen,
    )?;
    let final_link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        generation,
        *root_account.key,
        true,
        &mut link_reopen,
    )?;
    require(
        final_root.state().retired_series_links()
            == root_successor.state.retired_series_links()
            && final_link.state().phase() == SeriesMarketLinkPhaseV3::Retired
            && final_link.state() == &link_successor.state,
        ClutchError::MismatchedState,
    )?;
    let source_market_terminal_id = source_market_terminal.id();
    let source_market_terminal_facts = source_market_terminal.facts();
    let id = hashv(&[
        b"dragons-clutch/sbf/product-source-series-retirement/v5\0",
        program_id.as_ref(),
        &failure.id.bytes(),
        &source_market_terminal_id.bytes(),
        root_account.key.as_ref(),
        &root_data_before_id.bytes(),
        &final_root.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &final_root.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &final_root.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &final_root.state().transition_sequence().to_le_bytes(),
        link_account.key.as_ref(),
        &link_data_before_id.bytes(),
        &final_link.data_id().bytes(),
        &link_authentication_before_id.bytes(),
        &final_link.authentication_id().bytes(),
        &link_semantic_before_id.bytes(),
        &final_link.semantic_id().bytes(),
        &link_transition_sequence_before.to_le_bytes(),
        &final_link.state().transition_sequence().to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSourceSeriesRetirementV5 {
        id,
        failure,
        source_market_terminal: Some(source_market_terminal),
        source_market_terminal_id,
        source_market_terminal_facts,
        source_projection: None,
        source_shared_core_projection_id: ContentId::ZERO,
        root_account: *root_account.key,
        root_data_before_id,
        root_data_after_id: final_root.data_id(),
        root_authentication_before_id,
        root_authentication_after_id: final_root.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: final_root.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: final_root.state().transition_sequence(),
        link_account: *link_account.key,
        link_data_before_id,
        link_data_after_id: final_link.data_id(),
        link_authentication_before_id,
        link_authentication_after_id: final_link.authentication_id(),
        link_semantic_before_id,
        link_semantic_after_id: final_link.semantic_id(),
        link_transition_sequence_before,
        link_transition_sequence_after: final_link.state().transition_sequence(),
    })
}

/// Successful Source occurrence: the durable Failure owner itself is the sole
/// Source lifecycle terminal authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_successful_source_and_count_series_link_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    funding_account: &AccountInfo<'_>,
    failure: AuthenticatedProductFailureCoreTerminalV5,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductSourceSeriesRetirementV5> {
    let (failure, owner) = failure.into_source_parts();
    retire_source_and_count_series_link_v5(
        program_id,
        root_account,
        link_account,
        funding_account,
        failure,
        owner,
        route,
        schedule,
        custody_account,
        principal_refund,
        neutral_sink,
        system_program,
    )
}

/// SourceAbsent/SourceRefused occurrence: hostile-reopen the persisted Source
/// V3 terminal and inseparably join it to the unique Failure owner before the
/// same physical Source/Link transition.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_failed_source_and_count_series_link_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    funding_account: &AccountInfo<'_>,
    failure: AuthenticatedProductFailureCoreTerminalV5,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    persisted_source_terminal_account: &AccountInfo<'_>,
    custody_account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductSourceSeriesRetirementV5> {
    let (failure, owner) = failure.into_source_parts();
    let source_authority = authenticate_failure_market_source_failure_lifecycle_terminal_v3(
        program_id,
        route,
        persisted_source_terminal_account,
        owner,
    )?;
    retire_source_and_count_series_link_v5(
        program_id,
        root_account,
        link_account,
        funding_account,
        failure,
        source_authority,
        route,
        schedule,
        custody_account,
        principal_refund,
        neutral_sink,
        system_program,
    )
}

/// Exact private authority for sealing the hostile current family aggregator.
/// It is derived only inside the RootV3 postwriter after the final Series link
/// has physically retired; callers cannot provide an aggregate identity.
struct ExactProductMarketBeginRetirementAuthorityV5 {
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    family_prestate_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactProductMarketBeginRetirementAuthorityV5 {
    fn authenticate_begin_retirement(
        &self,
        current: &MarketFamilyAggregatorV1,
    ) -> clutch_product_series::Result<()> {
        let current_id = current.semantic_id()?.content_id();
        if self.market_instance_id != current.binding().market_instance_id
            || self.generation != current.binding().generation
            || self.family_prestate_id != current_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Move-only Product authority proving the exact final-Series postwrite was
/// consumed into the one-way RootV3 Active→Retiring transition.  It owns the
/// unique Source Market terminal until that capability is latched next.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductMarketRetiringV5 {
    id: ContentId,
    source: AuthenticatedProductSourceSeriesRetirementV5,
    family_prestate_id: ContentId,
    root_account: Pubkey,
    root_data_before_id: ContentId,
    root_data_after_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
}

impl AuthenticatedProductMarketRetiringV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn source_series_retirement_id(&self) -> ContentId {
        self.source.id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
}

/// Seal new Series admissions after the final physically retired Source link.
/// All dynamic link counts and the rolling link transcript come from the
/// hostile RootV3 and the retained Source Market terminal; no caller count or
/// identity participates.
#[inline(never)]
pub(crate) fn begin_current_product_market_retirement_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    source: AuthenticatedProductSourceSeriesRetirementV5,
) -> Outcome<AuthenticatedProductMarketRetiringV5> {
    let facts = source.source_market_terminal_facts;
    require(
        source.source_market_terminal.is_some()
            && source.source_projection.is_none()
            && source.source_shared_core_projection_id.is_zero()
            && source.root_account == *root_account.key
            && facts.root_account.bytes() == root_account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let market_instance_id = MarketInstanceV2Id::from_bytes(facts.market_instance_id.bytes());
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        facts.generation,
        true,
        &mut root_value,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Active
            && root.binding_id() == facts.root_binding_id
            && root.state().live_series_links() == 0
            && root.state().admitted_series_links() == facts.admitted_series_links
            && root.state().retired_series_links() == facts.retired_series_links
            && root.state().series_link_transcript_id() == facts.series_link_transcript_id
            && root.state().transition_sequence() >= facts.root_transition_sequence_after
            && root
                .state()
                .shared_core_terminal_receipts()
                .iter()
                .all(|id| id.is_zero()),
        ClutchError::MismatchedState,
    )?;
    let family_prestate_id = root
        .state()
        .product_families()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let authority = ExactProductMarketBeginRetirementAuthorityV5 {
        market_instance_id,
        generation: facts.generation,
        family_prestate_id,
    };
    let mut next = Box::new(clutch_product_series::MarketLifecycleRootV3::decode_buffer());
    root.state()
        .begin_retirement_into(&authority, &mut next)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data_before_id = root.data_id();
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    drop(root);
    write_market_lifecycle_root_v3(root_account, &root_value, &next)?;
    let mut reopened_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        facts.generation,
        true,
        &mut reopened_value,
    )?;
    require(
        reopened.state() == next.as_ref()
            && reopened.state().phase() == MarketLifecyclePhaseV3::Retiring
            && reopened.state().transition_sequence()
                == root_transition_sequence_before
                    .checked_add(1)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_MARKET_BEGIN_RETIREMENT_DOMAIN_V5,
        program_id.as_ref(),
        &source.id.bytes(),
        &source.source_market_terminal_id.bytes(),
        &family_prestate_id.bytes(),
        root_account.key.as_ref(),
        &root_data_before_id.bytes(),
        &reopened.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &reopened.state().transition_sequence().to_le_bytes(),
        &facts.series_link_transcript_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductMarketRetiringV5 {
        id,
        source,
        family_prestate_id,
        root_account: *root_account.key,
        root_data_before_id,
        root_data_after_id: reopened.data_id(),
        root_authentication_before_id,
        root_authentication_after_id: reopened.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: reopened.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: reopened.state().transition_sequence(),
    })
}

/// Consume Source's sole move-only Market terminal into the current RootV3
/// shared-core latch. The Product Source-series receipt is transformed in
/// place: no second Product authority or detachable Source receipt is
/// returned.
///
/// The root may have advanced from Active to Retiring and consumed other
/// terminal owners since Source retired the final link. Its immutable binding,
/// zero-live-link counts, and exact link transcript must still equal Source's
/// postwrite. The final LinkV3 and FundingV5 accounts are hostile-reopened and
/// exact-matched before the Source capability is consumed.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn consume_source_market_shared_core_v5(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    funding_account: &AccountInfo<'_>,
    retiring: AuthenticatedProductMarketRetiringV5,
) -> Outcome<AuthenticatedProductSourceSeriesRetirementV5> {
    let AuthenticatedProductMarketRetiringV5 {
        id: retiring_id,
        source: mut source,
        family_prestate_id: _,
        root_account: retiring_root_account,
        root_data_before_id: _,
        root_data_after_id: retiring_root_data_id,
        root_authentication_before_id: _,
        root_authentication_after_id: retiring_root_authentication_id,
        root_semantic_before_id: _,
        root_semantic_after_id: retiring_root_semantic_id,
        root_transition_sequence_before: _,
        root_transition_sequence_after: retiring_root_sequence,
    } = retiring;
    let terminal = source
        .source_market_terminal
        .take()
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let facts = terminal.facts();
    require(
        source.source_projection.is_none()
            && source.source_shared_core_projection_id.is_zero()
            && !retiring_id.is_zero()
            && retiring_root_account == *root_account.key
            && terminal.id() == source.source_market_terminal_id
            && facts == source.source_market_terminal_facts
            && source.failure.failure.market_instance_id.bytes()
                == facts.market_instance_id.bytes()
            && source.failure.failure.generation == facts.generation
            && source.root_account == *root_account.key
            && facts.root_account.bytes() == root_account.key.to_bytes()
            && facts.link_account.bytes() == link_account.key.to_bytes()
            && source.root_data_after_id == facts.root_data_after_id
            && source.root_authentication_after_id == facts.root_authentication_after_id
            && source.root_semantic_after_id == facts.root_semantic_after_id
            && source.root_transition_sequence_after == facts.root_transition_sequence_after
            && source.link_account == *link_account.key
            && source.link_data_after_id == facts.link_data_after_id
            && source.link_authentication_after_id == facts.link_authentication_after_id
            && source.link_semantic_after_id.bytes() == facts.link_semantic_after_id.bytes()
            && source.link_transition_sequence_after == facts.link_transition_sequence_after
            && root_account.key != link_account.key
            && root_account.key != funding_account.key
            && link_account.key != funding_account.key,
        ClutchError::MismatchedState,
    )?;
    let market_instance_id = MarketInstanceV2Id::from_bytes(facts.market_instance_id.bytes());
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        facts.generation,
        true,
        &mut root_value,
    )?;
    let (series_plan_id, ordinal) = observe_link_coordinate_v3(link_account)?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        facts.generation,
        *root_account.key,
        true,
        &mut link_value,
    )?;
    let funding = super::authenticate_series_funding_account_v5(
        program_id,
        funding_account,
        series_plan_id,
        false,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Retiring
            && root.binding_id() == facts.root_binding_id
            && root.data_id() == retiring_root_data_id
            && root.authentication_id() == retiring_root_authentication_id
            && root.semantic_id() == retiring_root_semantic_id
            && root.state().transition_sequence() == retiring_root_sequence
            && root.state().live_series_links() == 0
            && root.state().admitted_series_links() == facts.admitted_series_links
            && root.state().series_link_transcript_id() == facts.series_link_transcript_id
            && root.state().transition_sequence() > facts.root_transition_sequence_after
            && root
                .state()
                .shared_core_terminal_receipt(MarketSharedCoreV3::Source)
                .is_zero()
            && link.state().phase() == SeriesMarketLinkPhaseV3::Retired
            && link.binding_id() == facts.link_binding_id
            && link.data_id() == facts.link_data_after_id
            && link.authentication_id() == facts.link_authentication_after_id
            && link.semantic_id().bytes() == facts.link_semantic_after_id.bytes()
            && link.state().transition_sequence() == facts.link_transition_sequence_after
            && link.binding().source_occurrence_account_id.bytes()
                == facts.owner_account_id.bytes()
            && link.binding().source_release_id == facts.owner_release_id
            && funding.account().to_bytes() == facts.funding_account.bytes()
            && funding
                .state()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == facts.funding_state_id.bytes()
            && funding.data_id() == facts.funding_account_data_id
            && funding.authentication_id() == facts.funding_account_authentication_id,
        ClutchError::MismatchedState,
    )?;
    let sequence_after = root
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let projection = MarketSharedCoreTerminalProjectionV3::new(
        *root.binding(),
        MarketSharedCoreV3::Source,
        facts.owner_account_id,
        facts.owner_release_id,
        terminal.id(),
        sequence_after,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let next = (*root.state())
        .consume_shared_core_terminal(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data_before_id = root.data_id();
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    drop(link);
    drop(root);
    write_market_lifecycle_root_v3(root_account, &root_value, &next)?;
    let mut reopened_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        facts.generation,
        true,
        &mut reopened_value,
    )?;
    require(
        reopened.state() == &next
            && reopened
                .state()
                .shared_core_terminal_receipt(MarketSharedCoreV3::Source)
                == projection.id()
            && reopened.state().transition_sequence() == sequence_after,
        ClutchError::MismatchedState,
    )?;
    let source_market_terminal_id = terminal.id();
    let family_projection = terminal.into_family_projection();
    let prior_product_id = source.id;
    let id = hashv(&[
        PRODUCT_SOURCE_SHARED_CORE_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &retiring_id.bytes(),
        &prior_product_id.bytes(),
        &source_market_terminal_id.bytes(),
        &family_projection.id.bytes(),
        &projection.id().bytes(),
        root_account.key.as_ref(),
        &root_data_before_id.bytes(),
        &reopened.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &sequence_after.to_le_bytes(),
        link_account.key.as_ref(),
        &facts.link_data_after_id.bytes(),
        &facts.link_authentication_after_id.bytes(),
        &facts.link_semantic_after_id.bytes(),
        funding_account.key.as_ref(),
        &facts.funding_state_id.bytes(),
        &facts.funding_account_data_id.bytes(),
        &facts.funding_account_authentication_id.bytes(),
    ]);
    require_live(id)?;
    source.id = id;
    source.source_projection = Some(family_projection);
    source.source_shared_core_projection_id = projection.id();
    source.root_data_before_id = root_data_before_id;
    source.root_data_after_id = reopened.data_id();
    source.root_authentication_before_id = root_authentication_before_id;
    source.root_authentication_after_id = reopened.authentication_id();
    source.root_semantic_before_id = root_semantic_before_id;
    source.root_semantic_after_id = reopened.semantic_id();
    source.root_transition_sequence_before = root_transition_sequence_before;
    source.root_transition_sequence_after = sequence_after;
    Ok(source)
}

struct ExactProductSeriesFundingTerminalAuthorityV5 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    compiler_bundle_id: CompiledProductSeriesBundleV7Id,
    transition_sequence: u64,
}

impl AuthenticatedSeriesFundingAuthorityV5 for ExactProductSeriesFundingTerminalAuthorityV5 {
    fn authenticate_activation(
        &self,
        _series: &SeriesPlanV5,
        _funding_terms_id: SeriesFundingTermsV2Id,
        _compiler_bundle_id: CompiledProductSeriesBundleV7Id,
        _quote: &SeriesFundingQuoteV6,
        _attachment: &SeriesAttachmentPlanV6,
        _principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        _donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn current_bucket(&self, _series: &SeriesPlanV5) -> clutch_product_series::Result<u64> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV5,
        _binding: &SeriesFundingReservationBindingV5,
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        _state: &SeriesFundingStateV5,
        _binding: &SeriesFundingCompletionBindingV5,
        _completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV5,
        _binding: &SeriesFundingAbortBindingV5,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV5,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        state: &SeriesFundingStateV5,
        terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if self.id.is_zero()
            || terminal_receipt_id != self.id
            || state.phase != SeriesFundingPhaseV5::Closed
            || state.series_plan_id != self.series_plan_id
            || state.funding_terms_id != self.funding_terms_id
            || state.compiler_bundle_id != self.compiler_bundle_id
            || state.transition_sequence != self.transition_sequence
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Move-only terminal seal consumed only by current physical FundingV5
/// retirement.  It owns the hostile Registry/artifact/Funding graph and the
/// Source→RootV3/LinkV3 postwrite which authorized the exact projection.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesLifecycleTerminalV5 {
    id: ContentId,
    physical_authority_id: ContentId,
    source: AuthenticatedProductSourceSeriesRetirementV5,
    registry: AuthenticatedRegistryCapabilityV5,
    funding: Box<AuthenticatedSeriesFundingAccountV5>,
    bundle: AuthenticatedCompiledProductSeriesBundleV7,
    artifacts: AuthenticatedSeriesSourceArtifactsV6,
    terminal_projection: SeriesFundingTerminalProjectionV5,
    terminal_projection_id: ContentId,
}

impl AuthenticatedProductSeriesLifecycleTerminalV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn physical_authority_id(&self) -> ContentId {
        self.physical_authority_id
    }
    pub(crate) const fn source(&self) -> &AuthenticatedProductSourceSeriesRetirementV5 {
        &self.source
    }
    pub(crate) const fn registry(&self) -> &AuthenticatedRegistryCapabilityV5 {
        &self.registry
    }
    pub(crate) const fn funding(&self) -> &AuthenticatedSeriesFundingAccountV5 {
        &self.funding
    }
    pub(crate) const fn bundle(&self) -> &AuthenticatedCompiledProductSeriesBundleV7 {
        &self.bundle
    }
    pub(crate) const fn artifacts(&self) -> &AuthenticatedSeriesSourceArtifactsV6 {
        &self.artifacts
    }
    pub(crate) const fn terminal_projection(&self) -> SeriesFundingTerminalProjectionV5 {
        self.terminal_projection
    }
    pub(crate) const fn terminal_projection_id(&self) -> ContentId {
        self.terminal_projection_id
    }

    pub(crate) fn authenticate_physical_preflight_v5(
        &self,
        registry: &AuthenticatedRegistryCapabilityV5,
        funding: &AuthenticatedSeriesFundingAccountV5,
        projection: SeriesFundingTerminalProjectionV5,
    ) -> Outcome<ContentId> {
        let source_projection = self.source.source_projection()?;
        require(
            self.physical_authority_id == projection.terminal_receipt_id
                && self.terminal_projection == projection
                && self.terminal_projection_id
                    == projection
                        .id()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                && self.registry.id() == registry.id()
                && self.registry.series_registry_account()
                    == registry.series_registry_account()
                && self.registry.series_registry_authentication_id()
                    == registry.series_registry_authentication_id()
                && self.funding.as_ref() == funding
                && source_projection.facts.funding_account.bytes()
                    == funding.account().to_bytes()
                && source_projection.facts.funding_account_data_id == funding.data_id()
                && source_projection.facts.funding_account_authentication_id
                    == funding.authentication_id(),
            ClutchError::MismatchedState,
        )?;
        Ok(self.id)
    }
}

/// Seal the exact Closed FundingV5 projection after Source custody and the
/// current LinkV3 have physically terminated.  No Funding bytes are mutated;
/// the returned capability must be consumed by physical retirement in the
/// same instruction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn terminalize_product_series_funding_v5(
    source: AuthenticatedProductSourceSeriesRetirementV5,
    registry: AuthenticatedRegistryCapabilityV5,
    funding: AuthenticatedSeriesFundingAccountV5,
    bundle: AuthenticatedCompiledProductSeriesBundleV7,
    artifacts: AuthenticatedSeriesSourceArtifactsV6,
) -> Outcome<AuthenticatedProductSeriesLifecycleTerminalV5> {
    artifacts.validate_registry_projection(&registry.projection())?;
    let series_plan_id = artifacts
        .series()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let quote_id = artifacts
        .quote()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = artifacts
        .attachment()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_projection = source.source_projection()?;
    require(
        registry.activation_consumed()
            && registry.series_plan_id() == series_plan_id
            && registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && bundle.bundle().series_plan_id == series_plan_id
            && bundle.bundle().funding_terms_id == funding_terms_id
            && bundle.bundle().funding_quote_id == quote_id
            && bundle.bundle().attachment_plan_id == attachment_id
            && funding.state().phase == SeriesFundingPhaseV5::Closed
            && funding.state().series_plan_id == series_plan_id
            && funding.state().funding_terms_id == funding_terms_id
            && funding.state().funding_quote_id == quote_id
            && funding.state().attachment_plan_id == attachment_id
            && funding.state().compiler_bundle_id == bundle.bundle_id()
            && source.source_market_terminal.is_none()
            && !source.source_market_terminal_id.is_zero()
            && !source.source_shared_core_projection_id.is_zero()
            && source_projection.facts.funding_account.bytes() == funding.account().to_bytes()
            && source_projection.facts.funding_state_id.bytes()
                == funding
                    .state()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .bytes()
            && source_projection.facts.funding_account_data_id == funding.data_id()
            && source_projection.facts.funding_account_authentication_id
                == funding.authentication_id(),
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_SERIES_FUNDING_TERMINAL_AUTHORITY_DOMAIN_V5,
        &source.id.bytes(),
        &source.failure.id.bytes(),
        &source_projection.id.bytes(),
        &source.source_market_terminal_id.bytes(),
        &source.source_shared_core_projection_id.bytes(),
        &source.root_authentication_after_id.bytes(),
        &source.root_semantic_after_id.bytes(),
        &source.root_transition_sequence_after.to_le_bytes(),
        &source.link_authentication_after_id.bytes(),
        &source.link_semantic_after_id.bytes(),
        &source.link_transition_sequence_after.to_le_bytes(),
        &registry.id().bytes(),
        &funding.data_id().bytes(),
        &funding.authentication_id().bytes(),
        &bundle.bundle_id().bytes(),
    ]);
    require_live(id)?;
    let authority = ExactProductSeriesFundingTerminalAuthorityV5 {
        id,
        series_plan_id,
        funding_terms_id,
        compiler_bundle_id: bundle.bundle_id(),
        transition_sequence: funding.state().transition_sequence,
    };
    let projection = funding
        .state()
        .close(
            &authority,
            artifacts.series(),
            artifacts.quote(),
            artifacts.attachment(),
            id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let projection_id = projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let seal_id = hashv(&[
        PRODUCT_SERIES_LIFECYCLE_TERMINAL_DOMAIN_V5,
        &id.bytes(),
        &projection_id.bytes(),
        &registry.id().bytes(),
        &funding.data_id().bytes(),
        &funding.authentication_id().bytes(),
    ]);
    require_live(seal_id)?;
    // The projection's private terminal receipt is the physical authority ID;
    // the outer seal is retained separately and cannot be substituted for it.
    Ok(AuthenticatedProductSeriesLifecycleTerminalV5 {
        id: seal_id,
        physical_authority_id: id,
        source,
        registry,
        funding: Box::new(funding),
        bundle,
        artifacts,
        terminal_projection: projection,
        terminal_projection_id: projection_id,
    })
}

/// Transaction-local Product postwrite over the sole physical FundingV5
/// retirement.  It is private so callers cannot detach physical closure from
/// the final RootV3 terminal postwrite below.
#[derive(Debug)]
struct AuthenticatedProductSeriesPhysicalRetirementPostwriteV5 {
    id: ContentId,
    physical: AuthenticatedSeriesPhysicalRetirementV5,
    physical_receipt_transcript_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    root_binding_id: ContentId,
    root_account: Pubkey,
    root_data_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_transition_sequence: u64,
    link_account: Pubkey,
    link_series_plan_id: SeriesPlanV5Id,
    link_ordinal: u32,
    link_data_id: ContentId,
    link_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV3Id,
    link_transition_sequence: u64,
}

fn physical_retirement_receipt_transcript_v5(
    physical: &AuthenticatedSeriesPhysicalRetirementV5,
) -> Outcome<ContentId> {
    let mut transcript = hashv(&[
        PRODUCT_SERIES_PHYSICAL_RETIREMENT_POSTWRITE_DOMAIN_V5,
        b"physical-receipt-transcript",
        &physical.id().bytes(),
        &physical.funding_close_receipt_id().bytes(),
    ]);
    for receipts in [
        physical.lamport_retirement_receipt_ids().as_slice(),
        physical.collateral_principal_receipt_ids().as_slice(),
        physical.collateral_donation_receipt_ids().as_slice(),
        physical.collateral_close_receipt_ids().as_slice(),
    ] {
        for receipt in receipts {
            require_live(*receipt)?;
            transcript = hashv(&[
                PRODUCT_SERIES_PHYSICAL_RETIREMENT_POSTWRITE_DOMAIN_V5,
                &transcript.bytes(),
                &receipt.bytes(),
            ]);
        }
    }
    require_live(transcript)?;
    Ok(transcript)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn physically_retire_current_product_series_v5<'a>(
    program_id: &Pubkey,
    terminal: AuthenticatedProductSeriesLifecycleTerminalV5,
    position: &AuthenticatedProductPositionPhysicalTerminalV5,
    root_account: &AccountInfo<'a>,
    link_account: &AccountInfo<'a>,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    physical_accounts: &[AccountInfo<'a>],
) -> Outcome<AuthenticatedProductSeriesPhysicalRetirementPostwriteV5> {
    require(
        root_account.key != link_account.key
            && root_account.key != registry_account.key
            && root_account.key != funding_account.key
            && link_account.key != registry_account.key
            && link_account.key != funding_account.key,
        ClutchError::AccountAlias,
    )?;
    for account in physical_accounts {
        require(
            account.key != root_account.key && account.key != link_account.key,
            ClutchError::AccountAlias,
        )?;
    }
    let source = terminal.source();
    let source_facts = source.source_market_terminal_facts();
    let source_projection = source.source_projection()?;
    let lifecycle_terminal_id = terminal.id();
    let terminal_projection = terminal.terminal_projection();
    let terminal_projection_id = terminal.terminal_projection_id();
    let market_instance_id =
        MarketInstanceV2Id::from_bytes(source_facts.market_instance_id.bytes());
    let (series_plan_id, ordinal) = observe_link_coordinate_v3(link_account)?;
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        source_facts.generation,
        true,
        &mut root_value,
    )?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        source_facts.generation,
        *root_account.key,
        true,
        &mut link_value,
    )?;
    let expected_position_sequence = position
        .root_transition_sequence_before()
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Retiring
            && root.binding_id() == source_facts.root_binding_id
            && root.account() == source.root_account()
            && position.market_instance_id() == market_instance_id
            && position.generation() == source_facts.generation
            && position.root_account() == *root_account.key
            && position.root_binding_id() == source_facts.root_binding_id
            && position.root_data_before_id() == source.root_data_after_id
            && position.root_authentication_before_id()
                == source.root_authentication_after_id()
            && position.root_semantic_before_id() == source.root_semantic_after_id()
            && position.root_transition_sequence_before()
                == source.root_transition_sequence_after()
            && position.root_transition_sequence_after() == expected_position_sequence
            && root.data_id() == position.root_data_after_id()
            && root.authentication_id() == position.root_authentication_after_id()
            && root.semantic_id() == position.root_semantic_after_id()
            && root.state().transition_sequence()
                == position.root_transition_sequence_after()
            && root
                .state()
                .shared_core_terminal_receipt(MarketSharedCoreV3::Source)
                == source.source_shared_core_projection_id()
            && root
                .state()
                .shared_core_terminal_receipt(MarketSharedCoreV3::Position)
                == position.shared_core_projection_id()
            && link.account() == source.link_account()
            && link.state().phase() == SeriesMarketLinkPhaseV3::Retired
            && link.data_id() == source.link_data_after_id
            && link.authentication_id() == source.link_authentication_after_id()
            && link.semantic_id() == source.link_semantic_after_id()
            && link.state().transition_sequence() == source.link_transition_sequence_after()
            && source_projection.facts.funding_account.bytes()
                == funding_account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let root_binding_id = root.binding_id();
    let root_data_id = root.data_id();
    let root_authentication_id = root.authentication_id();
    let root_semantic_id = root.semantic_id();
    let root_transition_sequence = root.state().transition_sequence();
    let link_data_id = link.data_id();
    let link_authentication_id = link.authentication_id();
    let link_semantic_id = link.semantic_id();
    let link_transition_sequence = link.state().transition_sequence();
    drop(link);
    drop(root);
    let physical = retire_current_series_physical_v5(
        program_id,
        terminal,
        registry_account,
        funding_account,
        physical_accounts,
    )?;
    require(
        physical.lifecycle_terminal_id() == lifecycle_terminal_id
            && physical.terminal_projection() == terminal_projection
            && physical.terminal_projection_id() == terminal_projection_id
            && physical.registry_account() == *registry_account.key
            && physical.funding_account() == *funding_account.key,
        ClutchError::MismatchedState,
    )?;
    let mut reopened_root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened_root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        market_instance_id,
        source_facts.generation,
        true,
        &mut reopened_root_value,
    )?;
    let mut reopened_link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let reopened_link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        series_plan_id,
        ordinal,
        market_instance_id,
        source_facts.generation,
        *root_account.key,
        true,
        &mut reopened_link_value,
    )?;
    require(
        reopened_root.data_id() == root_data_id
            && reopened_root.authentication_id() == root_authentication_id
            && reopened_root.semantic_id() == root_semantic_id
            && reopened_root.state().transition_sequence() == root_transition_sequence
            && reopened_link.data_id() == link_data_id
            && reopened_link.authentication_id() == link_authentication_id
            && reopened_link.semantic_id() == link_semantic_id
            && reopened_link.state().transition_sequence() == link_transition_sequence,
        ClutchError::MismatchedState,
    )?;
    let physical_receipt_transcript_id =
        physical_retirement_receipt_transcript_v5(&physical)?;
    let id = hashv(&[
        PRODUCT_SERIES_PHYSICAL_RETIREMENT_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &lifecycle_terminal_id.bytes(),
        &terminal_projection_id.bytes(),
        &physical.id().bytes(),
        &physical_receipt_transcript_id.bytes(),
        root_account.key.as_ref(),
        &root_binding_id.bytes(),
        &root_data_id.bytes(),
        &root_authentication_id.bytes(),
        &root_semantic_id.bytes(),
        &root_transition_sequence.to_le_bytes(),
        link_account.key.as_ref(),
        &link_data_id.bytes(),
        &link_authentication_id.bytes(),
        &link_semantic_id.bytes(),
        &link_transition_sequence.to_le_bytes(),
        registry_account.key.as_ref(),
        &physical.registry_data_id().bytes(),
        &physical.registry_authentication_id().bytes(),
        funding_account.key.as_ref(),
        &physical.funding_data_before_id().bytes(),
        &physical.funding_authentication_before_id().bytes(),
        &physical.funding_close_receipt_id().bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesPhysicalRetirementPostwriteV5 {
        id,
        physical,
        physical_receipt_transcript_id,
        market_instance_id,
        generation: source_facts.generation,
        root_binding_id,
        root_account: *root_account.key,
        root_data_id,
        root_authentication_id,
        root_semantic_id,
        root_transition_sequence,
        link_account: *link_account.key,
        link_series_plan_id: series_plan_id,
        link_ordinal: ordinal,
        link_data_id,
        link_authentication_id,
        link_semantic_id,
        link_transition_sequence,
    })
}

/// Sole whole-Series Product terminal authority.  General action47 consumes
/// this value directly; neither the physical Funding receipt nor the terminal
/// RootV3 projection is detachable.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesRetirementV5 {
    id: ContentId,
    position: AuthenticatedProductPositionPhysicalTerminalV5,
    physical: AuthenticatedProductSeriesPhysicalRetirementPostwriteV5,
    failure: AuthenticatedProductFailurePhysicalTerminalV5,
    market_terminal_projection: clutch_product_series::MarketInstanceTerminalProjectionV3,
    market_terminal_projection_id: ContentId,
    root_data_before_id: ContentId,
    root_data_after_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
}

impl AuthenticatedProductSeriesRetirementV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.physical.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.physical.generation }
    pub(crate) const fn root_account(&self) -> Pubkey { self.physical.root_account }
    pub(crate) const fn root_binding_id(&self) -> ContentId {
        self.physical.root_binding_id
    }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.physical.link_account }
    pub(crate) const fn link_series_plan_id(&self) -> SeriesPlanV5Id {
        self.physical.link_series_plan_id
    }
    pub(crate) const fn link_ordinal(&self) -> u32 { self.physical.link_ordinal }
    pub(crate) const fn link_authentication_id(&self) -> ContentId {
        self.physical.link_authentication_id
    }
    pub(crate) const fn link_semantic_id(&self) -> SeriesMarketLinkV3Id {
        self.physical.link_semantic_id
    }
    pub(crate) const fn physical_retirement_id(&self) -> ContentId {
        self.physical.physical.id()
    }
    pub(crate) const fn physical_funding_close_receipt_id(&self) -> ContentId {
        self.physical.physical.funding_close_receipt_id()
    }
    pub(crate) const fn market_terminal_projection_id(&self) -> ContentId {
        self.market_terminal_projection_id
    }
    pub(crate) const fn market_terminal_projection(
        &self,
    ) -> clutch_product_series::MarketInstanceTerminalProjectionV3 {
        self.market_terminal_projection
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn retire_current_product_series_v5<'a, 'failure>(
    program_id: &Pubkey,
    terminal: AuthenticatedProductSeriesLifecycleTerminalV5,
    position: AuthenticatedProductPositionPhysicalTerminalV5,
    failure_terminal: AuthenticatedFailureMarketPhysicalTerminalV3<'failure>,
    root_account: &AccountInfo<'a>,
    link_account: &AccountInfo<'a>,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    physical_accounts: &[AccountInfo<'a>],
) -> Outcome<AuthenticatedProductSeriesRetirementV5> {
    let physical = physically_retire_current_product_series_v5(
        program_id,
        terminal,
        &position,
        root_account,
        link_account,
        registry_account,
        funding_account,
        physical_accounts,
    )?;
    let failure = consume_failure_market_physical_terminal_v5(
        program_id,
        root_account,
        failure_terminal,
    )?;
    let mut root_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        physical.market_instance_id,
        physical.generation,
        true,
        &mut root_value,
    )?;
    let mut link_value = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link = authenticate_series_market_link_v3(
        program_id,
        link_account,
        physical.link_series_plan_id,
        physical.link_ordinal,
        physical.market_instance_id,
        physical.generation,
        *root_account.key,
        true,
        &mut link_value,
    )?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Retiring
            && root.binding_id() == physical.root_binding_id
            && root.account() == failure.root_account()
            && root.authentication_id() == failure.root_authentication_after_id()
            && root.semantic_id() == failure.root_semantic_after_id()
            && root.state().transition_sequence() == failure.root_transition_sequence_after()
            && root
                .state()
                .shared_core_terminal_receipts()
                .iter()
                .all(|receipt| !receipt.is_zero())
            && link.state().phase() == SeriesMarketLinkPhaseV3::Retired
            && link.data_id() == physical.link_data_id
            && link.authentication_id() == physical.link_authentication_id
            && link.semantic_id() == physical.link_semantic_id
            && link.state().transition_sequence() == physical.link_transition_sequence,
        ClutchError::MismatchedState,
    )?;
    let root_data_before_id = root.data_id();
    let root_authentication_before_id = root.authentication_id();
    let root_semantic_before_id = root.semantic_id();
    let root_transition_sequence_before = root.state().transition_sequence();
    let mut root_next = Box::new(clutch_product_series::MarketLifecycleRootV3::decode_buffer());
    let market_terminal_projection = root
        .state()
        .finalize_terminal_into(&mut root_next)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(link);
    drop(root);
    write_market_lifecycle_root_v3(root_account, &root_value, &root_next)?;
    let mut reopened_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        physical.market_instance_id,
        physical.generation,
        true,
        &mut reopened_value,
    )?;
    let market_terminal_projection_id = market_terminal_projection.id();
    require(
        reopened.state() == root_next.as_ref()
            && reopened.state().phase() == MarketLifecyclePhaseV3::Terminal
            && reopened.state().terminal_projection().map_err(|_| {
                Refusal::Adapter(ClutchError::MismatchedState)
            })? == market_terminal_projection
            && market_terminal_projection.root_semantic_id() == reopened.semantic_id()
            && market_terminal_projection.final_transition_sequence()
                == reopened.state().transition_sequence(),
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_MARKET_TERMINAL_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &position.id().bytes(),
        &position.physical_terminal_id().bytes(),
        &position.shared_core_projection_id().bytes(),
        &physical.id.bytes(),
        &physical.physical.id().bytes(),
        &physical.physical_receipt_transcript_id.bytes(),
        &failure.id().bytes(),
        &failure.shared_core_projection_id().bytes(),
        root_account.key.as_ref(),
        &physical.root_binding_id.bytes(),
        &root_data_before_id.bytes(),
        &reopened.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &reopened.state().transition_sequence().to_le_bytes(),
        &market_terminal_projection_id.bytes(),
        link_account.key.as_ref(),
        &physical.link_data_id.bytes(),
        &physical.link_authentication_id.bytes(),
        &physical.link_semantic_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesRetirementV5 {
        id,
        position,
        physical,
        failure,
        market_terminal_projection,
        market_terminal_projection_id,
        root_data_before_id,
        root_data_after_id: reopened.data_id(),
        root_authentication_before_id,
        root_authentication_after_id: reopened.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: reopened.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: reopened.state().transition_sequence(),
    })
}

#[cfg(test)]
mod source_shared_core_adversarial_tests {
    #[test]
    fn product_consumes_source_market_owner_before_funding_terminal() {
        let source = include_str!("retirement_v5.rs");
        let consume = source
            .split("pub(crate) fn consume_source_market_shared_core_v5")
            .nth(1)
            .and_then(|value| value.split("struct ExactProductSeriesFundingTerminalAuthorityV5").next())
            .expect("Product Source shared-core consumer");
        for exact in [
            "MarketLifecyclePhaseV3::Retiring",
            "series_link_transcript_id() == facts.series_link_transcript_id",
            "SeriesMarketLinkPhaseV3::Retired",
            "funding.authentication_id() == facts.funding_account_authentication_id",
            "MarketSharedCoreV3::Source",
            "terminal.into_family_projection()",
            "source_market_terminal\n        .take()",
        ] {
            assert!(consume.contains(exact), "missing Source shared-core join {exact}");
        }
        assert!(!consume.contains("AuthenticatedProductSourceSharedCoreTerminal"));

        let funding = source
            .split("pub(crate) fn terminalize_product_series_funding_v5")
            .nth(1)
            .expect("FundingV5 terminal owner");
        assert!(funding.contains("source.source_market_terminal.is_none()"));
        assert!(funding.contains("!source.source_shared_core_projection_id.is_zero()"));
    }
}
