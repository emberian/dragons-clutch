//! Sole current FundingV5 reservation postwrite.
//!
//! This module owns only the Active-to-Pending boundary needed before Source
//! capitalization. The reservation binding is accepted only from a private
//! current Product preauthorization owner, the receipt ID is derived here, and
//! the returned value owns the hostile-reopened `0x80/v5` poststate. Source
//! must consume that value by ownership; no bare binding or receipt ID is an
//! executable authority.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_series_current::{
    authenticate_series_funding_account_v5, AuthenticatedSeriesFundingAccountV5,
};
use clutch_product_series::{
    AuthenticatedSeriesFundingAuthorityV5, ComponentDebitV1, ContentId,
    SeriesAttachmentPlanV6, SeriesFundingAbortBindingV5, SeriesFundingComponentV2,
    SeriesFundingCompletionBindingV5, SeriesFundingPhaseV5, SeriesFundingQuoteV6,
    SeriesFundingReservationBindingV5, SeriesFundingReservationBindingV5Id,
    SeriesFundingStateV5, SeriesFundingStateV5Id, SeriesFundingTermsV2Id, SeriesPlanV5,
    SeriesPlanV5Id, CompiledProductSeriesBundleV7Id, SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::SeriesFundingAccountV5;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_SERIES_FUNDING_RESERVATION_AUTHORITY_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-funding-reservation-authority/v5\0";
const PRODUCT_SERIES_FUNDING_RESERVATION_POSTWRITE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/product-series-funding-reservation-postwrite/v5\0";

/// Default-refusing current Product owner of an exact acyclic reservation.
///
/// A production implementation must be a move-only preauthorization derived
/// from RegistryCapabilityV5, BundleV7/QuoteV6/AttachmentV6, authenticated
/// Clock eligibility, deterministic Source coordinates, and future RootV3 and
/// LinkV3 coordinates. Caller payloads must not implement this boundary.
pub(crate) trait AuthenticatedProductSeriesFundingReservationOwnerV5 {
    fn owner_authentication_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_series_funding_reservation_v5(
        &self,
        _program_id: &Pubkey,
        _funding_account: Pubkey,
        _funding_data_before_id: ContentId,
        _funding_authentication_before_id: ContentId,
        _funding_state_before_id: SeriesFundingStateV5Id,
        _funding_transition_sequence_before: u64,
        _series_plan_id: SeriesPlanV5Id,
        _funding_terms_id: SeriesFundingTermsV2Id,
        _compiler_bundle_id: CompiledProductSeriesBundleV7Id,
        _funding_quote_id: ContentId,
        _attachment_plan_id: ContentId,
        _reservation_binding_id: SeriesFundingReservationBindingV5Id,
        _reservation_receipt_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Move-only hostile Pending receipt consumed by current Source
/// capitalization. It retains the full typed reservation and poststate.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductSeriesFundingReservationV5 {
    id: ContentId,
    owner_authentication_id: ContentId,
    binding: Box<SeriesFundingReservationBindingV5>,
    binding_id: SeriesFundingReservationBindingV5Id,
    reservation_receipt_id: ContentId,
    funding_account: Pubkey,
    funding_state_before_id: SeriesFundingStateV5Id,
    funding_data_before_id: ContentId,
    funding_authentication_before_id: ContentId,
    pending: AuthenticatedSeriesFundingAccountV5,
}

impl AuthenticatedProductSeriesFundingReservationV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn owner_authentication_id(&self) -> ContentId {
        self.owner_authentication_id
    }
    pub(crate) const fn binding(&self) -> &SeriesFundingReservationBindingV5 {
        &self.binding
    }
    pub(crate) const fn binding_id(&self) -> SeriesFundingReservationBindingV5Id {
        self.binding_id
    }
    pub(crate) const fn reservation_receipt_id(&self) -> ContentId {
        self.reservation_receipt_id
    }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.funding_account }
    pub(crate) const fn funding_state_before_id(&self) -> SeriesFundingStateV5Id {
        self.funding_state_before_id
    }
    pub(crate) const fn funding_data_before_id(&self) -> ContentId {
        self.funding_data_before_id
    }
    pub(crate) const fn funding_authentication_before_id(&self) -> ContentId {
        self.funding_authentication_before_id
    }
    pub(crate) const fn pending(&self) -> &AuthenticatedSeriesFundingAccountV5 {
        &self.pending
    }
}

struct ExactFundingReservationAuthorityV5 {
    funding_state_before_id: SeriesFundingStateV5Id,
    binding_id: SeriesFundingReservationBindingV5Id,
    reservation_receipt_id: ContentId,
}

impl AuthenticatedSeriesFundingAuthorityV5 for ExactFundingReservationAuthorityV5 {
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
        state: &SeriesFundingStateV5,
        binding: &SeriesFundingReservationBindingV5,
        reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if state.id()? != self.funding_state_before_id
            || binding.id()? != self.binding_id
            || reservation_receipt_id != self.reservation_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
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
        _state: &SeriesFundingStateV5,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

/// Reserve exact current principal, persist Pending FundingV5, and
/// hostile-reopen the poststate. No component vault movement occurs here:
/// reservation is the semantic one-shot consumed by the subsequent Source
/// physical capitalization transaction within the same outer instruction.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn reserve_current_product_series_funding_v5<O>(
    program_id: &Pubkey,
    owner: O,
    funding_before: AuthenticatedSeriesFundingAccountV5,
    funding_account: &AccountInfo<'_>,
    series: &SeriesPlanV5,
    quote: &SeriesFundingQuoteV6,
    attachment: &SeriesAttachmentPlanV6,
    binding: Box<SeriesFundingReservationBindingV5>,
) -> Outcome<AuthenticatedProductSeriesFundingReservationV5>
where
    O: AuthenticatedProductSeriesFundingReservationOwnerV5,
{
    let state_before = funding_before.state();
    let state_before_id = state_before
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let series_plan_id = series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let quote_id = quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = attachment
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        funding_before.is_writable()
            && funding_before.account() == *funding_account.key
            && funding_account.owner == program_id
            && funding_account.is_writable
            && !funding_account.is_signer
            && !funding_account.executable
            && funding_before.value().state.phase == SeriesFundingPhaseV5::Active
            && binding.funding_account_id.bytes() == funding_account.key.to_bytes()
            && binding.funding_account_authentication_before_id
                == funding_before.authentication_id()
            && binding.funding_state_before_id == state_before_id
            && binding.funding_transition_sequence_before == state_before.transition_sequence
            && binding.series_plan_id == series_plan_id
            && binding.series_plan_id == state_before.series_plan_id
            && binding.funding_terms_id == state_before.funding_terms_id
            && binding.funding_quote_id == quote_id
            && binding.funding_quote_id == state_before.funding_quote_id
            && binding.attachment_plan_id == attachment_id
            && binding.attachment_plan_id == state_before.attachment_plan_id
            && binding.compiler_bundle_id == state_before.compiler_bundle_id,
        ClutchError::MismatchedState,
    )?;
    {
        let data = funding_account
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(
            funding_account.lamports() == funding_before.observed_lamports()
                && hash_data(&data) == funding_before.data_id(),
            ClutchError::MismatchedState,
        )?;
    }
    let owner_authentication_id = owner.owner_authentication_id()?;
    require_live(owner_authentication_id)?;
    let reservation_receipt_id = hashv(&[
        PRODUCT_SERIES_FUNDING_RESERVATION_AUTHORITY_DOMAIN_V5,
        program_id.as_ref(),
        funding_account.key.as_ref(),
        &funding_before.data_id().bytes(),
        &funding_before.authentication_id().bytes(),
        &state_before_id.bytes(),
        &binding_id.bytes(),
        &owner_authentication_id.bytes(),
        &state_before.transition_sequence.to_le_bytes(),
    ]);
    require_live(reservation_receipt_id)?;
    owner.authenticate_product_series_funding_reservation_v5(
        program_id,
        *funding_account.key,
        funding_before.data_id(),
        funding_before.authentication_id(),
        state_before_id,
        state_before.transition_sequence,
        series_plan_id,
        binding.funding_terms_id,
        binding.compiler_bundle_id,
        quote_id.content_id(),
        attachment_id.content_id(),
        binding_id,
        reservation_receipt_id,
    )?;
    let authority = ExactFundingReservationAuthorityV5 {
        funding_state_before_id: state_before_id,
        binding_id,
        reservation_receipt_id,
    };
    let mut successor = *state_before;
    let ordinal = successor
        .reserve_created(
            &authority,
            series,
            quote,
            attachment,
            &binding,
            reservation_receipt_id,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        ordinal == binding.ordinal
            && successor.phase == SeriesFundingPhaseV5::Pending
            && successor.pending_pre_source_reservation_binding_id
                == binding_id.content_id()
            && successor.pending_reservation_receipt_id == reservation_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let successor_account = SeriesFundingAccountV5 {
        state: successor,
        rent_principal_lamports: funding_before.value().rent_principal_lamports,
        collateral_vault_rent_principal_lamports: funding_before
            .value()
            .collateral_vault_rent_principal_lamports,
        stored_bump: funding_before.value().stored_bump,
    };
    {
        let mut data = funding_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        successor_account.encode(&mut data)?;
    }
    let pending = authenticate_series_funding_account_v5(
        program_id,
        funding_account,
        series_plan_id,
        true,
    )?;
    require(
        pending.value() == &successor_account
            && pending.observed_lamports() == funding_before.observed_lamports()
            && pending.data_id() != funding_before.data_id()
            && pending.authentication_id() != funding_before.authentication_id()
            && pending.state().phase == SeriesFundingPhaseV5::Pending,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_SERIES_FUNDING_RESERVATION_POSTWRITE_DOMAIN_V5,
        program_id.as_ref(),
        &owner_authentication_id.bytes(),
        &binding_id.bytes(),
        &reservation_receipt_id.bytes(),
        funding_account.key.as_ref(),
        &state_before_id.bytes(),
        &pending
            .state()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
        &funding_before.data_id().bytes(),
        &pending.data_id().bytes(),
        &funding_before.authentication_id().bytes(),
        &pending.authentication_id().bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductSeriesFundingReservationV5 {
        id,
        owner_authentication_id,
        binding,
        binding_id,
        reservation_receipt_id,
        funding_account: *funding_account.key,
        funding_state_before_id: state_before_id,
        funding_data_before_id: funding_before.data_id(),
        funding_authentication_before_id: funding_before.authentication_id(),
        pending,
    })
}

fn hash_data(data: &[u8]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(&[data]).to_bytes())
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
