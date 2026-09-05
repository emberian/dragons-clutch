//! The pre-Market Series permit expiry, the one Hot path that runs before a
//! Market exists: its selector, its account map and its replay overlap rules.

use super::*;

extern crate alloc;

use dclutch_core_contract::ContentId;
use dclutch_market::capability_program::v4::{
    CapabilityProgramV4, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
};
use dclutch_market::execution_strategy::v2::EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2;
use dclutch_trading::series::{
    generated::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    replay::SERIES_STATE_BYTES_V3,
    request::{SeriesActionRequestV3, SeriesActionV3},
};
use dclutch_vm::account_profile::v3::SCHEMA_RELEASE_ID_V3 as ACCOUNT_PROFILE_SCHEMA_ID_V3;
use dclutch_vm::effect::v5::SCHEMA_RELEASE_ID_V5 as EFFECT_SCHEMA_ID_V5;
use dclutch_vm::request_profile::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1;
use dclutch_vm::v3::SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID_V3;
use solana_program::hash::hash;

// The kernel, not `crate::series`: this route is compiled into links that do
// not select the `series-family` feature, and `pub mod series` is gated on it.
use dclutch_trading::series::{
    SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3, SERIES_ROOT_SCHEMA_PREIMAGE_V3,
    SERIES_SUCCESSOR_KIND_PREIMAGE_V3, SERIES_TICKET_DERIVATION_PREIMAGE_V3,
};

pub(super) const SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1: usize = 81;
pub(super) const SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1: usize = 5;
pub(super) const SERIES_EXPIRE_CORE_ROUTE_START_V1: usize = 55;
pub(super) const SERIES_EXPIRE_CORE_ROUTE_COUNT_V1: usize = 26;
/// Vacant future Market carried by projected-Custody Abort, never the fixed
/// live controller Market authenticated by ordinary Hot.
pub(super) const SERIES_EXPIRE_FUTURE_MARKET_ACCOUNT_V1: usize = 54;
pub(super) const SERIES_EXPIRE_PERMIT_ACCOUNT_V1: usize = SERIES_EXPIRE_CORE_ROUTE_START_V1;
pub(super) const SERIES_EXPIRE_RENT_CREDIT_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 1;
pub(super) const SERIES_EXPIRE_RENT_PROGRAM_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 2;
pub(super) const SERIES_EXPIRE_ROOT_REPLAY_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 14;
pub(super) const SERIES_EXPIRE_TICKET_REPLAY_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 15;
pub(super) const SERIES_EXPIRE_TEMPLATE_RAW_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 16;
pub(super) const SERIES_EXPIRE_TEMPLATE_STAGING_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 17;
pub(super) const SERIES_EXPIRE_OCCURRENCE_RAW_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 18;
pub(super) const SERIES_EXPIRE_OCCURRENCE_STAGING_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 19;
pub(super) const SERIES_EXPIRE_TICKET_RAW_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 20;
pub(super) const SERIES_EXPIRE_TICKET_STAGING_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 21;
pub(super) const SERIES_EXPIRE_SYSTEM_PROGRAM_ACCOUNT_V1: usize =
    SERIES_EXPIRE_CORE_ROUTE_START_V1 + 24;
const SERIES_EXPIRE_CALLER_ACCOUNT_V1: usize = SERIES_EXPIRE_CORE_ROUTE_START_V1 + 25;

const _: () = {
    assert!(SERIES_EXPIRE_CORE_ROUTE_START_V1 + SERIES_EXPIRE_CORE_ROUTE_COUNT_V1 == 81);
    assert!(SERIES_EXPIRE_CALLER_ACCOUNT_V1 + 1 == SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1);
    assert!(SERIES_EXPIRE_ROOT_REPLAY_ACCOUNT_V1 == 69);
    assert!(SERIES_EXPIRE_TICKET_REPLAY_ACCOUNT_V1 == 70);
    assert!(SERIES_EXPIRE_FUTURE_MARKET_ACCOUNT_V1 + 1 == SERIES_EXPIRE_CORE_ROUTE_START_V1);
    assert!(SERIES_EXPIRE_OCCURRENCE_RAW_ACCOUNT_V1 == 73);
    assert!(SERIES_EXPIRE_TICKET_RAW_ACCOUNT_V1 == 75);
};

/// A request which has not yet earned exceptional pre-Market behavior.
///
/// `None` is intentionally the only negative result: classifier failure is not
/// a protocol refusal. The caller must continue through ordinary Hot so short,
/// malformed, or merely Series-looking bytes retain its historical outcome.
pub(super) fn classify_selected_series_expiry_v1(
    family_request: &[u8],
    selected_action: u32,
    selected_config: ContentId,
    descriptor: CapabilityProgramV4,
) -> Option<SeriesActionRequestV3<'_>> {
    let request = SeriesActionRequestV3::decode(family_request).ok()?;
    if request.action() != SeriesActionV3::Expire
        || selected_action != SeriesActionV3::Expire as u32
        || request.template() != selected_config
        || !is_exact_series_expiry_descriptor_v1(descriptor)
    {
        return None;
    }
    Some(request)
}

fn is_exact_series_expiry_descriptor_v1(descriptor: CapabilityProgramV4) -> bool {
    descriptor.kind().to_bytes() == hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes()
        && descriptor.config_schema().to_bytes() == SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3
        && descriptor.request_schema().to_bytes()
            == hash(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).to_bytes()
        && descriptor.root_schema().to_bytes() == hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes()
        && descriptor.derivation_policy().to_bytes()
            == hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes()
        && descriptor.account_profile().schema().to_bytes() == ACCOUNT_PROFILE_SCHEMA_ID_V3
        && descriptor.request_profile().schema().to_bytes() == REQUEST_PROFILE_SCHEMA_ID_V1
        && descriptor.lifecycle().schema().to_bytes() == SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5
        && descriptor.strategy().schema().to_bytes() == EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2
        && descriptor.transition().schema().to_bytes() == TRANSITION_SCHEMA_ID_V3
        && descriptor.effect().schema().to_bytes() == EFFECT_SCHEMA_ID_V5
        && usize::try_from(descriptor.root_state_bytes()).ok() == Some(SERIES_STATE_BYTES_V3)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use dclutch_market::capability_program::v4::{ArtifactReferenceV4, CapabilityArtifactsV4};
    use dclutch_trading::series::request::encode_series_action_header_v3;

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero content identity")
    }

    fn reference(schema: [u8; 32], program: u8) -> ArtifactReferenceV4 {
        ArtifactReferenceV4::new(ContentId::new(schema).expect("nonzero schema"), id(program))
    }

    fn exact_descriptor() -> CapabilityProgramV4 {
        CapabilityProgramV4::new(
            ContentId::new(hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes())
                .expect("Series kind"),
            ContentId::new(SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3).expect("Template schema"),
            ContentId::new(hash(SERIES_ACTION_HEADER_SCHEMA_PREIMAGE_V3).to_bytes())
                .expect("request schema"),
            ContentId::new(hash(SERIES_ROOT_SCHEMA_PREIMAGE_V3).to_bytes()).expect("root schema"),
            ContentId::new(hash(SERIES_TICKET_DERIVATION_PREIMAGE_V3).to_bytes())
                .expect("Ticket derivation"),
            id(0x20),
            CapabilityArtifactsV4 {
                account_profile: reference(ACCOUNT_PROFILE_SCHEMA_ID_V3, 0x21),
                request_profile: reference(REQUEST_PROFILE_SCHEMA_ID_V1, 0x22),
                lifecycle: reference(SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, 0x23),
                strategy: reference(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, 0x24),
                transition: reference(TRANSITION_SCHEMA_ID_V3, 0x25),
                effect: reference(EFFECT_SCHEMA_ID_V5, 0x26),
            },
            u32::try_from(SERIES_STATE_BYTES_V3).expect("Series state width"),
        )
        .expect("exact descriptor")
    }

    fn request(action: SeriesActionV3, template: ContentId) -> Vec<u8> {
        let header = encode_series_action_header_v3(
            action,
            template,
            Some(id(0x31)),
            Some(id(0x32)),
            7,
            3,
            1,
        )
        .expect("family header");
        let mut output = header.to_vec();
        output.extend_from_slice(&[0x44; 32]);
        output
    }

    #[test]
    fn exact_hostile_decode_and_descriptor_are_both_required() {
        let template = id(0x30);
        let family = request(SeriesActionV3::Expire, template);
        let selected = classify_selected_series_expiry_v1(
            &family,
            SeriesActionV3::Expire as u32,
            template,
            exact_descriptor(),
        )
        .expect("exact pre-Market Series expiry");
        assert_eq!(selected.bytes(), family);
        assert_eq!(selected.proof_bytes(), [0x44; 32]);
    }

    #[test]
    fn malformed_and_series_lookalike_requests_do_not_select() {
        let template = id(0x30);
        let exact = request(SeriesActionV3::Expire, template);
        assert!(
            classify_selected_series_expiry_v1(
                exact.get(..exact.len() - 1).expect("short request"),
                SeriesActionV3::Expire as u32,
                template,
                exact_descriptor(),
            )
            .is_none()
        );

        let consume = request(SeriesActionV3::Consume, template);
        assert!(
            classify_selected_series_expiry_v1(
                &consume,
                SeriesActionV3::Consume as u32,
                template,
                exact_descriptor(),
            )
            .is_none()
        );

        assert!(
            classify_selected_series_expiry_v1(
                &exact,
                SeriesActionV3::Expire as u32,
                id(0x77),
                exact_descriptor(),
            )
            .is_none()
        );
    }

    #[test]
    fn schema_substitution_does_not_earn_the_exception() {
        let template = id(0x30);
        let family = request(SeriesActionV3::Expire, template);
        let mut descriptor = exact_descriptor();
        let mut bytes = descriptor.encode();
        // Hostile-decode a different but individually valid account-profile
        // schema. It must not be enough that the descriptor is still V4.
        let replacement = [0x7a; 32];
        let offset = dclutch_market::capability_program::v4::CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET;
        bytes[offset..offset + 32].copy_from_slice(&replacement);
        descriptor = CapabilityProgramV4::decode(&bytes).expect("valid substituted descriptor");
        assert!(
            classify_selected_series_expiry_v1(
                &family,
                SeriesActionV3::Expire as u32,
                template,
                descriptor,
            )
            .is_none()
        );
    }
}

/// Authenticated byte-and-lamport facts which the final Series Core child may
/// observe but may not alter before Trading's commit-last replay writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SeriesExpiryReplayPrestateV1 {
    root_key: [u8; 32],
    root_data_digest: [u8; 32],
    root_lamports: u64,
    ticket_key: [u8; 32],
    ticket_data_digest: [u8; 32],
    ticket_lamports: u64,
}

impl SeriesExpiryReplayPrestateV1 {
    pub(super) fn authenticated(
        root: &AccountInfo<'_>,
        authenticated_root_digest: [u8; 32],
        ticket: &AccountInfo<'_>,
        authenticated_ticket_digest: [u8; 32],
    ) -> Result<Self, ProgramError> {
        let root_digest =
            hash(&root.try_borrow_data().map_err(|_| TradingSbfError::Root)?).to_bytes();
        let ticket_digest = hash(
            &ticket
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?,
        )
        .to_bytes();
        if root_digest != authenticated_root_digest || ticket_digest != authenticated_ticket_digest
        {
            return Err(TradingSbfError::Content.into());
        }
        Ok(Self {
            root_key: root.key.to_bytes(),
            root_data_digest: root_digest,
            root_lamports: root.lamports(),
            ticket_key: ticket.key.to_bytes(),
            ticket_data_digest: ticket_digest,
            ticket_lamports: ticket.lamports(),
        })
    }
}

/// Authenticate the one exact Series Expire execution whose Market does not
/// exist yet.
///
/// The negative result is deliberately non-refusing. Until the authenticated
/// ProgramSet and sealed descriptor select the exact Series Expire shape, the
/// ordinary live-Market path remains the authority for the invocation. Once
/// that selection is made, every later failure is a refusal: a selected
/// pre-Market action may not fall back to pretending its vacant account is a
/// live `CoreState`.
#[inline(never)]
pub(super) fn try_authenticate_series_expiry_premarket_v1<'accounts, 'info>(
    program_id: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    family_request: &[u8],
    invocation: AuthenticatedHotInvocationV3,
    frame: &HotFrameV3<'accounts, 'info>,
    root: &AuthenticatedRootV3,
    product_runtime_v3: &AuthenticatedProductRuntimeV3<'accounts, 'info>,
) -> Result<Option<[u8; 32]>, ProgramError> {
    if !matches!(
        SeriesActionRequestV3::decode(family_request),
        Ok(request) if request.action() == SeriesActionV3::Expire
    ) {
        return Ok(None);
    }
    // All errors before the exact sealed descriptor is classified are a
    // negative classification, not a new public refusal path. Ordinary Hot
    // repeats these checks in its historical order in the common tail.
    let Some((descriptor, selected_program, selected_action)) =
        authenticate_series_expiry_selection_v1(program_id, family_request, frame, root)
            .ok()
            .flatten()
    else {
        return Ok(None);
    };

    authenticate_selected_series_expiry_premarket_v1(
        program_id,
        accounts,
        family_request,
        invocation,
        frame,
        root,
        product_runtime_v3,
        descriptor,
        selected_program,
        selected_action,
    )
    .map(Some)
}

/// Finish a fully selected Series Expire without keeping selection, sealed
/// artifact, record, and replay values in one SBPF frame.
///
/// This is only a mechanical frame boundary. Each subordinate stage consumes
/// the original authenticated objects or immutable record bytes; it does not
/// introduce another protocol representation.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_selected_series_expiry_premarket_v1<'accounts, 'info>(
    program_id: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    family_request: &[u8],
    invocation: AuthenticatedHotInvocationV3,
    frame: &HotFrameV3<'accounts, 'info>,
    root: &AuthenticatedRootV3,
    product_runtime_v3: &AuthenticatedProductRuntimeV3<'accounts, 'info>,
    descriptor: CapabilityProgramV4,
    selected_program: ContentId,
    selected_action: u32,
) -> Result<[u8; 32], ProgramError> {
    let (runtime_accounts, core_template) = authenticate_series_expiry_execution_artifacts_v1(
        program_id,
        accounts,
        invocation,
        frame,
        root,
        descriptor,
        selected_program,
        selected_action,
    )?;
    authenticate_series_expiry_records_and_projection_v1(
        program_id,
        family_request,
        frame,
        &runtime_accounts,
        &core_template,
        product_runtime_v3,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_series_expiry_execution_artifacts_v1<'accounts, 'info>(
    program_id: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    invocation: AuthenticatedHotInvocationV3,
    frame: &HotFrameV3<'accounts, 'info>,
    root: &AuthenticatedRootV3,
    descriptor: CapabilityProgramV4,
    selected_program: ContentId,
    selected_action: u32,
) -> Result<(Vec<&'accounts AccountInfo<'info>>, Vec<u8>), ProgramError> {
    if frame.uses_sealed_execution_aliases() {
        return Err(TradingSbfError::Content.into());
    }
    let context = root.context;
    let seal_data = frame
        .capability_seal
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let seal = authenticate_capability_seal_v3(
        program_id,
        *frame,
        PROGRAM_SCHEMA_ID_V4,
        selected_program.to_bytes(),
        selected_action,
        root.trading_semantic_release,
        &seal_data,
    )?;

    let account_profile_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::AccountProfile,
        frame.account_profile_raw,
        frame.account_profile_staging,
        descriptor.account_profile().schema().to_bytes(),
        descriptor.account_profile().program().to_bytes(),
    )?;
    let account_profile_token = sealed_token(
        seal,
        SealedRoleV1::AccountProfile,
        descriptor.account_profile().schema().to_bytes(),
        descriptor.account_profile().program().to_bytes(),
        &account_profile_data,
    )?;
    let funding_profile =
        AccountProfileV3::from_sealed(&account_profile_data, account_profile_token)
            .map_err(|_| TradingSbfError::Content)?;
    let account_profile = funding_profile.base();
    if funding_profile.funding_bound_count() != 0 {
        return Err(TradingSbfError::Content.into());
    }

    let effect_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::EffectProgram,
        frame.effect_raw,
        frame.effect_staging,
        descriptor.effect().schema().to_bytes(),
        descriptor.effect().program().to_bytes(),
    )?;
    let effect_token = sealed_token(
        seal,
        SealedRoleV1::EffectProgram,
        descriptor.effect().schema().to_bytes(),
        descriptor.effect().program().to_bytes(),
        &effect_data,
    )?;
    let funding_effect = EffectProgramV5::from_sealed(&effect_data, effect_token)
        .map_err(|_| TradingSbfError::Content)?;
    if funding_effect.funding_action_count() != 0 || funding_effect.funding_seed_count() != 0 {
        return Err(TradingSbfError::Content.into());
    }
    let effect = funding_effect.base().base();

    let (strategy, strategy_extras_end) = authenticate_strategy_from_sealed_boxed_v3(
        frame,
        accounts,
        context,
        selected_program,
        &descriptor,
        invocation.strategy_extras_start,
    )?;
    if strategy.strategy().disposition() != StrategyDispositionV2::Interpreted {
        return Err(TradingSbfError::UnsupportedContent.into());
    }

    // Expire's selected Profile13 has no dynamic insertions and no item
    // accounts. A zero tail therefore resolves the exact same fixed logical
    // vector as the later authenticated Product outcome count; the common tail
    // repeats the expansion and the full geometry agreement before mutation.
    let runtime_accounts = expand_runtime_accounts_v3(
        account_profile,
        0,
        &[],
        [
            frame.root,
            frame.config_raw,
            frame.product_raw,
            frame.portfolio_raw,
            frame.linked_basis_raw,
        ],
        accounts
            .get(strategy_extras_end..)
            .ok_or(TradingSbfError::Content)?,
    )?;
    if runtime_accounts.len() != series_expiry::SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1
        || account_profile.fixed_account_count()
            != u16::try_from(series_expiry::SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1)
                .map_err(|_| TradingSbfError::Content)?
        || effect.route_count() != 5
    {
        return Err(TradingSbfError::Content.into());
    }

    let core_route = effect.route(4).map_err(|_| TradingSbfError::Content)?;
    let (core_template, core_item) = effect
        .route_template(4)
        .map_err(|_| TradingSbfError::Content)?;
    if core_route.role() != FixedRole::Core
        || core_route.kind() != RouteKindV3::Once
        || usize::from(core_route.fixed_account_start())
            != series_expiry::SERIES_EXPIRE_CORE_ROUTE_START_V1
        || usize::from(core_route.fixed_account_count())
            != series_expiry::SERIES_EXPIRE_CORE_ROUTE_COUNT_V1
        || core_route.item_account_count() != 0
        || core_route.item_request_bytes() != 0
        || !core_item.is_empty()
        || core_template.len() != SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1
    {
        return Err(TradingSbfError::Content.into());
    }

    Ok((runtime_accounts, core_template.to_vec()))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_series_expiry_records_and_projection_v1<'accounts, 'info>(
    program_id: &Pubkey,
    family_request: &[u8],
    frame: &HotFrameV3<'accounts, 'info>,
    runtime_accounts: &[&'accounts AccountInfo<'info>],
    core_template: &[u8],
    product_runtime_v3: &AuthenticatedProductRuntimeV3<'accounts, 'info>,
) -> Result<[u8; 32], ProgramError> {
    let template_raw = *runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_TEMPLATE_RAW_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let template_staging = *runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_TEMPLATE_STAGING_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let occurrence_raw = *runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_OCCURRENCE_RAW_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let occurrence_staging = *runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_OCCURRENCE_STAGING_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let ticket_raw = *runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_TICKET_RAW_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let ticket_staging = *runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_TICKET_STAGING_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let template_bytes = borrow_series_finalized_record_v1(
        *frame,
        template_raw,
        template_staging,
        SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
    )?;
    let occurrence_bytes = borrow_series_finalized_record_v1(
        *frame,
        occurrence_raw,
        occurrence_staging,
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    )?;
    let ticket_bytes = borrow_series_finalized_record_v1(
        *frame,
        ticket_raw,
        ticket_staging,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    )?;
    let future = derive_series_expiry_future_projection_from_records_v1(
        frame,
        family_request,
        &template_bytes,
        &occurrence_bytes,
        product_runtime_v3,
    )?;
    let expected_future_market =
        Pubkey::find_program_address(&future.seeds().as_slices(), frame.core_program.key).0;
    let future_market = *runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_FUTURE_MARKET_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    require_series_expiry_future_market_vacancy_v1(
        future_market,
        expected_future_market,
        frame.market.key,
    )?;
    authenticate_series_expiry_replay_from_records_v1(
        program_id,
        frame,
        runtime_accounts,
        family_request,
        &template_bytes,
        &occurrence_bytes,
        &ticket_bytes,
    )?;
    authenticate_series_expiry_core_template_v1(core_template)?;
    let rent_credit = authenticate_series_expiry_vacant_permit_request_v1(
        program_id,
        frame,
        runtime_accounts,
        family_request,
        &template_bytes,
        &occurrence_bytes,
        &ticket_bytes,
    )?;
    drop(template_bytes);
    drop(occurrence_bytes);
    drop(ticket_bytes);

    Ok(rent_credit)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn derive_series_expiry_future_projection_from_records_v1(
    frame: &HotFrameV3<'_, '_>,
    family_request: &[u8],
    template_bytes: &[u8],
    occurrence_bytes: &[u8],
    product_runtime_v3: &AuthenticatedProductRuntimeV3<'_, '_>,
) -> Result<dclutch_trading::series::FutureMarketProjectionV3, ProgramError> {
    let family =
        SeriesActionRequestV3::decode(family_request).map_err(|_| TradingSbfError::Content)?;
    let occurrence = admit_occurrence_bytes(template_bytes, occurrence_bytes, family.proof_bytes())
        .map_err(|_| TradingSbfError::Content)?;
    if family.action() != SeriesActionV3::Expire
        || family.template() != occurrence.template_id()
        || family.occurrence() != Some(occurrence.occurrence_id())
    {
        return Err(TradingSbfError::Content.into());
    }
    let projected_product = AuthenticatedProductProjectionV2::new(
        ContentId::new(
            product_runtime_v3
                .runtime
                .product_record
                .content_digest
                .to_bytes(),
        )
        .map_err(|_| TradingSbfError::Content)?,
        ContentId::new(product_runtime_v3.runtime.product_id.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        ContentId::new(
            product_runtime_v3
                .runtime
                .result_domain_record
                .content_digest
                .to_bytes(),
        )
        .map_err(|_| TradingSbfError::Content)?,
    );
    let future = future_market_projection(
        occurrence,
        projected_product,
        AccountKeyV3::new(frame.registry.key.to_bytes()).map_err(|_| TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let expected_market =
        Pubkey::find_program_address(&future.seeds().as_slices(), frame.core_program.key).0;
    future
        .require_address(
            AccountKeyV3::new(expected_market.to_bytes()).map_err(|_| TradingSbfError::Content)?,
        )
        .map_err(|_| TradingSbfError::Content)?;
    Ok(future)
}

#[inline(never)]
fn authenticate_series_expiry_selection_v1(
    program_id: &Pubkey,
    family_request: &[u8],
    frame: &HotFrameV3<'_, '_>,
    root: &AuthenticatedRootV3,
) -> Result<Option<(CapabilityProgramV4, ContentId, u32)>, ProgramError> {
    let context = root.context;
    let bumps = context.record_bumps();
    let manifest_data = borrow_finalized_record_at(
        *frame,
        frame.manifest_raw,
        frame.manifest_staging,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        context.selection().manifest().to_bytes(),
        bumps.manifest_raw(),
        bumps.manifest_staging(),
    )?;
    let entry = authenticate_manifest_entry_boxed_v3(&manifest_data, &context)?;
    let release = context.selection().capability_release().to_bytes();
    let program_set_data = borrow_finalized_record_at(
        *frame,
        frame.program_set_raw,
        frame.program_set_staging,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        release,
        context.selection().capability_release_raw_bump(),
        context.selection().capability_release_staging_bump(),
    )?;
    let set = CapabilityProgramSetV2::decode_selected(release, release, &program_set_data)
        .map_err(|_| TradingSbfError::Content)?;
    let selected = set
        .select_entry(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let reference = selected.descriptor();
    if reference.schema().to_bytes() != PROGRAM_SCHEMA_ID_V4 {
        return Ok(None);
    }
    let selected_program = reference.program();
    let selected_action = selected.selector();
    let seal_data = frame
        .capability_seal
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let seal = authenticate_capability_seal_v3(
        program_id,
        *frame,
        reference.schema().to_bytes(),
        selected_program.to_bytes(),
        selected_action,
        root.trading_semantic_release,
        &seal_data,
    )?;
    let descriptor_data = borrow_sealed_record(
        *frame,
        seal,
        SealedRoleV1::Descriptor,
        frame.descriptor_raw,
        frame.descriptor_staging,
        reference.schema().to_bytes(),
        selected_program.to_bytes(),
    )?;
    if descriptor_data.len() != CAPABILITY_PROGRAM_V4_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let descriptor =
        CapabilityProgramV4::decode(&descriptor_data).map_err(|_| TradingSbfError::Content)?;
    authenticate_descriptor_root_selection(&descriptor, &context, &entry)?;
    if series_expiry::classify_selected_series_expiry_v1(
        family_request,
        selected_action,
        context.selection().config(),
        descriptor,
    )
    .is_none()
    {
        return Ok(None);
    }
    Ok(Some((descriptor, selected_program, selected_action)))
}

fn borrow_series_finalized_record_v1<'a, 'info>(
    frame: HotFrameV3<'_, 'info>,
    raw: &'a AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    schema: [u8; 32],
) -> Result<core::cell::Ref<'a, [u8]>, ProgramError> {
    let digest = {
        let data = raw
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Content)?;
        hash(&data).to_bytes()
    };
    borrow_finalized_record(frame, raw, staging, schema, digest)
}

pub(super) fn require_series_expiry_future_market_vacancy_v1(
    market: &AccountInfo<'_>,
    expected_market: Pubkey,
    controller_market: &Pubkey,
) -> Result<(), ProgramError> {
    if market.key != &expected_market
        || market.key == controller_market
        || market.owner != &system_program::ID
        || !market.data_is_empty()
        || market.is_signer
        || market.is_writable
        || market.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

/// The vacant permit an expiry REFUNDS, which is not the permit a founding
/// creates.
///
/// Core never allocated this slot; the founding prepaid it in an earlier
/// transaction, at that transaction's rate, and this route hands the lamports
/// back. A floor at the rate of the moment therefore refuses a slot the
/// founding really did prepay the instant the cluster charges more than it did
/// then -- and the refund is stranded forever, because nothing tops up a permit
/// nobody owns. The seeds are the authority for WHICH slot this is;
/// `funded_rent_persists_v1` is the authority for whether there is anything
/// left in it.
pub(super) fn require_series_expiry_vacant_permit_v1(
    permit: &AccountInfo<'_>,
    expected_permit: Pubkey,
) -> Result<(), ProgramError> {
    if permit.key != &expected_permit
        || permit.owner != &system_program::ID
        || !permit.data_is_empty()
        || !permit.is_writable
        || permit.is_signer
        || permit.executable
        || !funded_rent_persists_v1(permit.lamports())
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_series_expiry_replay_from_records_v1(
    program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    runtime: &[&AccountInfo<'_>],
    family_request: &[u8],
    template_bytes: &[u8],
    occurrence_bytes: &[u8],
    ticket_bytes: &[u8],
) -> Result<(), ProgramError> {
    let admitted = admit_series_action_v3(
        family_request,
        template_bytes,
        Some(occurrence_bytes),
        Some(ticket_bytes),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let family = admitted.request();
    let occurrence = admitted
        .required_occurrence()
        .map_err(|_| TradingSbfError::Content)?;
    let ticket = admitted
        .required_ticket()
        .map_err(|_| TradingSbfError::Content)?;
    let replay_root = *runtime.first().ok_or(TradingSbfError::Content)?;
    let replay_ticket = *runtime
        .get(series_expiry::SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    if replay_root.key != frame.root.key
        || replay_root.owner != program_id
        || replay_root.data_len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3
        || replay_ticket.owner != program_id
        || replay_ticket.data_len() != SERIES_TICKET_STATE_BYTES_V3
        || runtime
            .get(series_expiry::SERIES_EXPIRE_ROOT_REPLAY_ACCOUNT_V1)
            .is_none_or(|account| account.key != replay_root.key)
        || runtime
            .get(series_expiry::SERIES_EXPIRE_TICKET_REPLAY_ACCOUNT_V1)
            .is_none_or(|account| account.key != replay_ticket.key)
    {
        return Err(TradingSbfError::Root.into());
    }
    let root_data = replay_root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    let series = SeriesStateV3::decode(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(TradingSbfError::Root)?,
        occurrence.template().occurrence_count(),
    )
    .map_err(|_| TradingSbfError::Root)?;
    let series_bytes = series
        .encode(occurrence.template().occurrence_count())
        .map_err(|_| TradingSbfError::Root)?;
    drop(root_data);
    let ticket_data = replay_ticket
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    let ticket_state = TicketStateV3::decode(&ticket_data).map_err(|_| TradingSbfError::Root)?;
    let ticket_bytes = ticket_state.encode();
    drop(ticket_data);
    let ticket_seeds = TicketStateSeedsV3::new(replay_root.key.to_bytes(), ticket.content_id());
    if Pubkey::find_program_address(&ticket_seeds.as_slices(), program_id).0 != *replay_ticket.key
        || series.next_occurrence() != occurrence.occurrence().occurrence()
        || !series.current_ticket_prepared()
        || series.revision() != family.expected_series_revision()
        || ticket_state.ticket_record_id() != ticket.content_id()
        || !SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1.admits(ticket_state.phase())
        || ticket_state.revision() != family.expected_ticket_revision()
    {
        return Err(TradingSbfError::Root.into());
    }
    let replay = evaluate_replay_v3(
        SeriesReplayActionV3::Expire {
            ticket_record: ticket.content_id(),
            expected_ticket_revision: family.expected_ticket_revision(),
        },
        occurrence.template().occurrence_count(),
        family.expected_series_revision(),
        &series_bytes,
        Some(&ticket_bytes),
    )
    .map_err(|_| TradingSbfError::Root)?;
    if !matches!(replay.series(), ReplayCandidateV3::Replace(_))
        || !matches!(replay.ticket(), ReplayCandidateV3::Replace(_))
    {
        return Err(TradingSbfError::Root.into());
    }
    Ok(())
}

/// Authenticate the Expire artifact's Core route template as the canonical
/// transient transport -- and NOT as a statement about any live revision.
///
/// TWO FACTS MEET AT THIS TEMPLATE AND ONLY ONE OF THEM IS THE ARTIFACT'S.
///
/// The first is *which revisions the Core CPI will assert*, and the family
/// request owns it. `SeriesActionRequestV3` carries them; the Expire
/// RequestProfile projects them into common scalars 8 and 9
/// (`series::expire_funding_artifacts_v5`'s `emit_request_profile`); the
/// Transition VM checks those against the account-projected observed revisions
/// 10 and 11; and the Effect VM writes them into route 4's fixed request at
/// `SERIES_UNALLOCATED_PERMIT_EXPIRY_EXPECTED_{SERIES,TICKET}_REVISION_OFFSET_V1`
/// immediately before the Core CPI. The projected bytes are then compared to
/// the family request AGAIN at `core_composition_v3::authenticate_core_request`,
/// and to the live root and Ticket accounts a third time inside
/// `core-sbf`'s `series_permit_expiry_precommit_v1::authenticate_prestate`.
/// Nothing here is owed a fourth author.
///
/// The second is *what the sealed release compiled*, and this function owns it.
/// `encode_request_bank` emits `SeriesUnallocatedPermitExpiryRequestV1::new(0,
/// 0)`: the hashed artifact carries the transient transport with ZERO
/// PLACEHOLDERS, because a revision is per-request runtime state and a release
/// artifact is per-release-set. The codec says the same thing in its own words
/// -- "only the two replay revisions originate in the family request and
/// therefore cross this wire".
///
/// What is left to check is the artifact's own promise, and it is fail-closed:
/// an artifact that dropped the two patch operations would send zeros to Core,
/// which compares them against a live root that is not at revision zero.
#[inline(never)]
fn authenticate_series_expiry_core_template_v1(core_template: &[u8]) -> Result<(), ProgramError> {
    let template = SeriesUnallocatedPermitExpiryRequestV1::decode(core_template)
        .map_err(|_| TradingSbfError::SeriesExpireCoreTemplate)?;
    if template.expected_series_revision() != 0 || template.expected_ticket_revision() != 0 {
        return Err(TradingSbfError::SeriesExpireCoreTemplate.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_series_expiry_vacant_permit_request_v1(
    _program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    runtime: &[&AccountInfo<'_>],
    family_request: &[u8],
    template_bytes: &[u8],
    occurrence_bytes: &[u8],
    ticket_bytes: &[u8],
) -> Result<[u8; 32], ProgramError> {
    let admitted = admit_series_action_v3(
        family_request,
        template_bytes,
        Some(occurrence_bytes),
        Some(ticket_bytes),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let occurrence = admitted
        .required_occurrence()
        .map_err(|_| TradingSbfError::Content)?;
    let ticket = admitted
        .required_ticket()
        .map_err(|_| TradingSbfError::Content)?;
    let release_set = occurrence.template().release_set().to_bytes();
    let future_market = occurrence.occurrence().market().to_bytes();
    let generation = u64::from(occurrence.occurrence().occurrence())
        .checked_add(1)
        .ok_or(TradingSbfError::Content)?;
    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        CoreIdentity::new(release_set).map_err(|_| TradingSbfError::Content)?,
        CoreIdentity::new(future_market).map_err(|_| TradingSbfError::Content)?,
        CoreIdentity::new(ticket.content_id().to_bytes()).map_err(|_| TradingSbfError::Content)?,
    );
    let expected_permit =
        Pubkey::find_program_address(&permit_seeds.as_slices(), frame.core_program.key).0;
    let permit = *runtime
        .get(series_expiry::SERIES_EXPIRE_PERMIT_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    require_series_expiry_vacant_permit_v1(permit, expected_permit)?;

    let rent_credit = *runtime
        .get(series_expiry::SERIES_EXPIRE_RENT_CREDIT_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let rent_program = *runtime
        .get(series_expiry::SERIES_EXPIRE_RENT_PROGRAM_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let credit_data = rent_credit
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let credit =
        LifecycleRentCreditV2::decode(&credit_data).map_err(|_| TradingSbfError::Content)?;
    if credit.refund_wallet().to_bytes() != ticket.ticket().refund_owner().to_bytes()
        || credit.market().to_bytes() != future_market
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != generation
    {
        return Err(TradingSbfError::Content.into());
    }
    let credit_seeds = credit.pda_seeds();
    let credit_bump = [credit_seeds.bump()];
    let credit_market = credit_seeds.market().to_bytes();
    let credit_generation = credit_seeds.generation();
    let expected_credit = Pubkey::create_program_address(
        &[
            credit_seeds.domain(),
            &credit_market,
            &credit_generation,
            credit_bump.as_slice(),
        ],
        rent_program.key,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if rent_credit.key != &expected_credit
        || rent_credit.owner != rent_program.key
        || rent_credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !funded_rent_persists_v1(rent_credit.lamports())
        || !rent_program.executable
        || rent_program.is_signer
        || rent_program.is_writable
    {
        return Err(TradingSbfError::Content.into());
    }
    drop(credit_data);
    let system = runtime
        .get(series_expiry::SERIES_EXPIRE_SYSTEM_PROGRAM_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    if system.key != &system_program::ID
        || !system.executable
        || system.is_signer
        || system.is_writable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(rent_credit.key.to_bytes())
}

/// Select the one Series child which must observe both replay prestates before
/// Trading commits their independently planned Expire candidates.
#[allow(clippy::too_many_arguments)]
pub(super) fn series_expiry_local_replay_overlap_v1(
    effect: SelectedEffectProgramV4<'_>,
    route_index: u16,
    invocation_index: u32,
    invocation: dclutch_vm::effect::v3::ResolvedInvocationV3,
    tail_count: u32,
    scalars: &[u64],
    request_bank: &[u8],
    family_request: &[u8],
    aliases: &[usize],
    participation: &[CoordinateParticipationV3],
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, '_>,
    authenticated_series_expiry_replay: bool,
    authenticated_series_expiry_rent_credit: [u8; 32],
    parent: CoreCompositionParentV3,
) -> Result<AllowedLocalOverlapV3, ProgramError> {
    use series_expiry::{
        SERIES_EXPIRE_CORE_ROUTE_COUNT_V1, SERIES_EXPIRE_CORE_ROUTE_START_V1,
        SERIES_EXPIRE_RENT_CREDIT_ACCOUNT_V1, SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1,
    };

    if !authenticated_series_expiry_replay
        || usize::from(invocation.fixed_account_start) != SERIES_EXPIRE_CORE_ROUTE_START_V1
        || usize::from(invocation.fixed_account_count) != SERIES_EXPIRE_CORE_ROUTE_COUNT_V1
        || !is_series_permit_expiry_precommit_observation_v1(
            effect.base(),
            route_index,
            invocation_index,
            invocation,
            request_bank,
            family_request,
            parent,
        )?
    {
        return Ok(AllowedLocalOverlapV3::None);
    }
    let ranges = BorrowedRouteRangesV4::new(
        effect.successor,
        route_index,
        tail_count,
        scalars,
        family_request,
    );
    let family = match SeriesActionRequestV3::decode(family_request) {
        Ok(request) if request.action() == SeriesActionV3::Expire => request,
        Ok(_) | Err(_) => return Ok(AllowedLocalOverlapV3::None),
    };
    // The route's borrowed bytes must be EXACTLY the family's proof, and the
    // empty proof is a real case rather than a missing one. `proof_height` is
    // zero for a Template with one occurrence, a `BorrowedRangeV4` is
    // canonically nonempty, so route 4 declares NO range there -- and this
    // read must say "zero ranges borrow zero proof bytes" instead of silently
    // returning `None`. It is the second author of the same fact as
    // `series::expire_funding_artifacts_v5::series_expire_borrowed_range_count_v5`,
    // and withdrawing the range without rewriting this conjunct would have
    // made the overlap disappear with no refusal and no word in the log.
    let borrowed_proof_matches = match ranges.count()? {
        0 => family.proof_bytes().is_empty(),
        1 => ranges.range(0)? == family.proof_bytes(),
        _ => false,
    };
    if !borrowed_proof_matches {
        return Ok(AllowedLocalOverlapV3::None);
    }
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let transport = request_bank
        .get(invocation.request_offset..request_end)
        .and_then(|request| request.get(..SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1))
        .and_then(|request| SeriesUnallocatedPermitExpiryRequestV1::decode(request).ok());
    if transport.is_none_or(|request| {
        request.expected_series_revision() != family.expected_series_revision()
            || request.expected_ticket_revision() != family.expected_ticket_revision()
    }) || effect_accounts
        .view(SERIES_EXPIRE_RENT_CREDIT_ACCOUNT_V1)?
        .key
        .to_bytes()
        != authenticated_series_expiry_rent_credit
    {
        return Ok(AllowedLocalOverlapV3::None);
    }

    const CORE_ROOT_LOCAL_V1: usize = 14;
    const CORE_TICKET_LOCAL_V1: usize = 15;
    let logical_root = SERIES_EXPIRE_CORE_ROUTE_START_V1 + CORE_ROOT_LOCAL_V1;
    let logical_ticket = SERIES_EXPIRE_CORE_ROUTE_START_V1 + CORE_TICKET_LOCAL_V1;
    if aliases.get(logical_root).copied() != Some(0)
        || aliases.get(logical_ticket).copied() != Some(SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1)
        || !participation
            .first()
            .copied()
            .unwrap_or_default()
            .locally_mutated()
        || !participation
            .get(SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1)
            .copied()
            .unwrap_or_default()
            .locally_mutated()
    {
        return Ok(AllowedLocalOverlapV3::None);
    }
    let root = effect_accounts.view(logical_root)?;
    let ticket = effect_accounts.view(logical_ticket)?;
    if root.is_signer
        || root.is_writable
        || root.executable
        || ticket.is_signer
        || ticket.is_writable
        || ticket.executable
    {
        return Ok(AllowedLocalOverlapV3::None);
    }
    let mut coordinate = SERIES_EXPIRE_CORE_ROUTE_START_V1;
    let end = coordinate
        .checked_add(SERIES_EXPIRE_CORE_ROUTE_COUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    while coordinate < end {
        let representative = aliases
            .get(coordinate)
            .copied()
            .ok_or(TradingSbfError::Content)?;
        if (representative == 0 && coordinate != logical_root)
            || (representative == SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1
                && coordinate != logical_ticket)
        {
            return Ok(AllowedLocalOverlapV3::None);
        }
        coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(AllowedLocalOverlapV3::SeriesExpiryReplay {
        root: 0,
        ticket: SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1,
    })
}

/// Refuse any Core-side mutation of the two replay prestates observed by the
/// exact Series precommit child. Trading is their sole writer and commits both
/// only after this proof; transaction atomicity then rolls the CPI back if the
/// later local commit cannot complete.
pub(super) fn verify_series_expiry_replay_unchanged_after_children_v1(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    let Some(expected) = prepared.series_expiry_replay_prestate else {
        return Ok(());
    };
    let root = prepared
        .runtime_accounts
        .first()
        .copied()
        .ok_or(TradingSbfError::Commit)?;
    let ticket = prepared
        .runtime_accounts
        .get(series_expiry::SERIES_EXPIRE_TICKET_STATE_ACCOUNT_V1)
        .copied()
        .ok_or(TradingSbfError::Commit)?;
    require_series_expiry_replay_prestate_v1(root, ticket, expected)
}

pub(super) fn require_series_expiry_replay_prestate_v1(
    root: &AccountInfo<'_>,
    ticket: &AccountInfo<'_>,
    expected: SeriesExpiryReplayPrestateV1,
) -> Result<(), ProgramError> {
    if root.key.to_bytes() != expected.root_key
        || root.lamports() != expected.root_lamports
        || ticket.key.to_bytes() != expected.ticket_key
        || ticket.lamports() != expected.ticket_lamports
    {
        return Err(TradingSbfError::Commit.into());
    }
    let root_digest = hash(
        &root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?,
    )
    .to_bytes();
    let ticket_digest = hash(
        &ticket
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?,
    )
    .to_bytes();
    if root_digest != expected.root_data_digest || ticket_digest != expected.ticket_data_digest {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}
