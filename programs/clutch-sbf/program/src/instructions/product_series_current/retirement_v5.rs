//! Sole current Product V3/V5 retirement owner.
//!
//! This module is a successor, not a bridge, for historical RootV2/LinkV2 and
//! FundingV4 retirement.  Every callable transition below hostile-reopens the
//! exact current RootV3/LinkV3 accounts, consumes one concrete move-only family
//! terminal, persists the unique successor, and hostile-reopens the postimage.
//! The final whole-Series composer will consume these postwrites by value
//! before closing FundingV5 and handing its own move-only receipt directly to
//! General action47 in the same instruction.

use super::super::failure_market_family_terminal_v2::{
    authenticate_failure_market_source_failure_lifecycle_terminal_v3,
    AuthenticatedFailureMarketFamilyTerminalReceiptV3,
    AuthenticatedFailureMarketFamilyTerminalOwnerV2, FailureMarketFamilyTerminalConsumerFactsV3,
};
use super::super::dealer_facility::AuthenticatedDealerFamilyTerminalReceiptV1;
use super::super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
};
use super::super::structured_custody::AuthenticatedStructuredWrapperFamilyTerminalV3;
use super::super::source_funding_custody_retirement_v1::{
    authenticate_source_family_terminal_authority_v3,
    consume_source_family_terminal_into_product_v3, retire_source_funding_custody_v3,
    AuthenticatedSourceFundingCustodyLifecycleTerminalAuthorityV1,
    SourceFamilyTerminalProjectionV3,
};
use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, ContentId, MarketFamilyAggregatorV1,
    MarketFamilyV1, MarketInstanceV2Id, MarketLifecyclePhaseV3,
    MarketSharedCoreTerminalProjectionV3, MarketSharedCoreV3,
    SeriesLinkObligationDispositionV3, SeriesLinkObligationTerminalProjectionV3,
    SeriesLinkObligationV3, SeriesMarketLinkPhaseV3, SeriesMarketLinkV3Id,
    SeriesPlanV5Id,
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
const PRODUCT_STRUCTURED_FAMILY_TERMINAL_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-structured-family-terminal-postwrite/v5\0";

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

fn write_market_lifecycle_root_v3(
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

/// Move-only Product postwrite proving that the exact hostile Failure owner
/// was consumed into the current RootV3 Failure shared-core slot.
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
    shared_core_projection_id: ContentId,
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
    pub(crate) shared_core_projection_id: ContentId,
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
    pub(crate) const fn shared_core_projection_id(&self) -> ContentId {
        self.shared_core_projection_id
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
                shared_core_projection_id: self.shared_core_projection_id,
                link_account: self.link_account,
                link_authentication_id: self.link_authentication_id,
                link_data_id: self.link_data_id,
                link_semantic_id: self.link_semantic_id,
            },
            self.failure_owner,
        )
    }
}

/// Consume the durable Failure-family terminal into RootV3.  The writable
/// LinkV3 is hostile-authenticated but not mutated: its exact account identity
/// is required because Failure's persisted Source release binds that Series
/// coordinate and no caller-supplied link ID is accepted.
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
    let sequence_after = root
        .state()
        .transition_sequence()
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let projection = MarketSharedCoreTerminalProjectionV3::new(
        binding,
        MarketSharedCoreV3::Failure,
        facts.owner_account_id,
        facts.owner_release_id,
        facts.owner_terminal_receipt_id,
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
    let link_authentication_id = link.authentication_id();
    let link_data_id = link.data_id();
    let link_semantic_id = link.semantic_id();
    drop(link);
    drop(root);
    write_market_lifecycle_root_v3(root_account, &root_value, &next)?;
    let mut reopened_value = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let reopened = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        facts.market_instance_id,
        facts.generation,
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
        PRODUCT_FAILURE_CORE_TERMINAL_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &failure_id.bytes(),
        root_account.key.as_ref(),
        &root_binding_id.bytes(),
        &root_data_before_id.bytes(),
        &reopened.data_id().bytes(),
        &root_authentication_before_id.bytes(),
        &reopened.authentication_id().bytes(),
        &root_semantic_before_id.bytes(),
        &reopened.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &sequence_after.to_le_bytes(),
        &projection.id().bytes(),
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
        root_data_after_id: reopened.data_id(),
        root_authentication_before_id,
        root_authentication_after_id: reopened.authentication_id(),
        root_semantic_before_id,
        root_semantic_after_id: reopened.semantic_id(),
        root_transition_sequence_before,
        root_transition_sequence_after: sequence_after,
        shared_core_projection_id: projection.id(),
        link_account: *link_account.key,
        link_authentication_id,
        link_data_id,
        link_semantic_id,
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
    let link_next = (*link.state())
        .consume_obligation(dealer_projection)
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
    require(
        reopened_root.state() == &root_next
            && reopened_link.state() == &link_next
            && reopened_link
                .state()
                .obligation_terminal_receipt_id(SeriesLinkObligationV3::Dealer)
                == dealer_projection_id,
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
    })
}

/// Move-only Product receipt left after the final Source custody is physically
/// closed and its exact LinkV3 retirement projection has been consumed by the
/// live RootV3.  Source's projection is facts-only; the physical authority was
/// consumed inside the transition and cannot be replayed.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSourceSeriesRetirementV5 {
    id: ContentId,
    failure: ProductFailureCoreTerminalFactsV5,
    source: SourceFamilyTerminalProjectionV3,
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
    pub(crate) const fn source(&self) -> SourceFamilyTerminalProjectionV3 { self.source }
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
    let source_projection = consume_source_family_terminal_into_product_v3(
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
    let id = hashv(&[
        b"dragons-clutch/sbf/product-source-series-retirement/v5\0",
        program_id.as_ref(),
        &failure.id.bytes(),
        &source_projection.id.bytes(),
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
        source: source_projection,
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
