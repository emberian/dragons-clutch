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
    AuthenticatedFailureMarketFamilyTerminalReceiptV3,
    FailureMarketFamilyTerminalConsumerFactsV3,
};
use super::super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
};
use super::super::structured_custody::AuthenticatedStructuredWrapperFamilyTerminalV3;
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
    failure: AuthenticatedFailureMarketFamilyTerminalReceiptV3,
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
        self.failure.facts()
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
    let facts = failure.facts();
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
        matches!(root.state().phase(), MarketLifecyclePhaseV3::Active | MarketLifecyclePhaseV3::Retiring)
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
        &failure.id().bytes(),
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
        failure,
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

struct ExactStructuredFamilyTerminalAuthorityV5 {
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    family_root_id: ContentId,
    terminal_sequence: u32,
    terminal_receipt_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactStructuredFamilyTerminalAuthorityV5 {
    fn authenticate_terminal(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if family != MarketFamilyV1::Structured
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
    let authority = ExactStructuredFamilyTerminalAuthorityV5 {
        market_instance_id: terminal.market_instance_id(),
        generation: terminal.generation(),
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
