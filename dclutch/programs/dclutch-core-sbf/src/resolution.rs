//! Resolution-role child composition through the canonical Core authority.

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingLedgerDerivationV2,
    CapabilityManifestV1, ContentId as CapabilityContentId, FUNDING_LEDGER_HEADER_BYTES_V2,
    FUNDING_LEDGER_SLOT_BYTES_V2, FundingLedgerStatusV2, FundingLedgerV2,
};
use dclutch_market_core_codec::{
    Action, CAPABILITY_FUNDING_HEADER_BYTES_V2, CapabilityFundingHeaderV2, ChildEffectObservation,
    CoreEffectAckV1, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState, MarketAdmissionV1, Phase,
    Product, Readiness, Request, Role, TerminalReceipt, admit_terminal, verify_readiness,
};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_resolution_codec::{
    FUNDING_ACTIVATION_RECEIPT_BYTES_V1, FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
    FundingActivationReceiptV1, FundingActivationRequestV1, RESOLUTION_CERTIFICATE_BYTES_V2,
    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, RESOLUTION_CONTROLLER_RELEASE_ID_V7,
    RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2, RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V2,
    ResolutionCertificateKindV2, ResolutionCertificateV2, ResolutionCoreActionV1,
    ResolutionCoreReceiptKindV1, ResolutionRoleRequestV2, funding_lifecycle_account_digest_v1,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, RECOVERY_POLICY_BYTES_V2, RECOVERY_POLICY_SCHEMA_ID_V2,
    RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_MATERIAL_V3_BYTES,
    SOURCE_RESOLUTION_STATE_BYTES_V2, SourceMaterialV3, SourceResolutionPhaseV1,
    SourceResolutionStateV2,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError,
    fixed_role::{
        FixedRoleAccountsV1, authenticate_fixed_role, authenticate_fixed_role_ack,
        invoke_fixed_role, nonzero_identity, persist_state, require_market_unchanged,
    },
    frame::require_distinct,
    product_runtime_v2::{authenticate_selected_runtime_v2, project_core_product_v2},
    records::authenticate_finalized_record,
};

/// Market prestates in which a Market may still create and activate its
/// Resolution Fund.
///
/// Two prestates are admissible and they are the same fact reached by the two
/// founding routes.
///
/// `Founding + Prepaid` is the readiness ladder: Found, then `CreateFund`,
/// then `VerifyFundReady` (which is what moves readiness to `Ready`), then a
/// separate `OpenMarket`.
///
/// `Open + Consumed` is the atomic founding. Its commit-last stage runs
/// `open_series_market`, which transitions `Founding + Prepaid` straight to
/// `Open + Consumed` in one step, so a Market founded that way never passes
/// through the ladder — and before this admission existed it had no route to
/// its own Resolution Fund at all, which made every atomically founded Market
/// permanently unresolvable.
///
/// Admitting the second prestate defers only Source-state creation, not
/// authority. The capability manifest is an immutable seed of the Market
/// address, and the Resolution subset ledger already exists before Market
/// Found. `CreateFund` writes only the prepaid Source destination and
/// authenticates the ledger byte-for-byte. Nothing here is a caller choice.
pub const CREATE_FUND_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 = MarketAdmissionV1::prestates(&[
    (Phase::Founding, Readiness::Prepaid),
    (Phase::Open, Readiness::Consumed),
]);

/// Market prestates in which the Resolution Fund's readiness may be verified.
///
/// Three prestates, and two of them have no transition left to make:
/// `commit_verified_readiness` is what decides that, and it is a guard rather
/// than a refusal because re-presenting an activation this Market already
/// accepted is not an error. Only `Founding + Prepaid` still moves.
pub const VERIFY_FUND_READY_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::prestates(&[
        (Phase::Founding, Readiness::Prepaid),
        (Phase::Founding, Readiness::Ready),
        (Phase::Open, Readiness::Consumed),
    ]);

/// Market prestates in which a terminal Product result may be admitted.
///
/// `Open` is the ordinary case. `Terminal` is admitted so that a re-presented
/// admission of the receipt this Market already holds is idempotent rather
/// than a refusal; the receipt itself is compared byte-for-byte downstream.
pub const ADMIT_TERMINAL_ADMISSIBLE_PRESTATES_V1: MarketAdmissionV1 =
    MarketAdmissionV1::prestates(&[
        (Phase::Open, Readiness::Consumed),
        (Phase::Terminal, Readiness::Consumed),
    ]);

/// Exact Resolution role request after the 280-byte Core envelope.
pub const RESOLUTION_ROLE_REQUEST_BYTES_V1: usize =
    CAPABILITY_FUNDING_HEADER_BYTES_V2 + RESOLUTION_CORE_ROLE_REQUEST_BYTES_V2;
/// Exact top-level Core data width for a Resolution child effect.
pub const RESOLUTION_CORE_INSTRUCTION_BYTES_V1: usize = dclutch_market_core_codec::REQUEST_BYTES
    + dclutch_market_core_codec::CORE_EFFECT_ENVELOPE_BYTES_V1
    + RESOLUTION_ROLE_REQUEST_BYTES_V1;

/// Exact outer account count for Source/Funding creation.
pub const RESOLUTION_CREATE_OUTER_ACCOUNT_COUNT_V1: usize = 18;
/// Exact outer account count for Source/Funding readiness.
pub const RESOLUTION_VERIFY_OUTER_ACCOUNT_COUNT_V1: usize = 20;
/// Exact outer account count for terminal admission, including the three
/// Product Runtime V2 finalized record pairs reauthenticated by both Resolution
/// and Core.
///
/// There is no matching child count: `AdmitTerminal` makes no child invocation.
/// Core authenticates the Resolution-owned certificate itself.
pub const RESOLUTION_ADMIT_OUTER_ACCOUNT_COUNT_V1: usize = 22;

const SOURCE_MATERIAL: usize = 8;
const SOURCE_MATERIAL_STAGING: usize = 9;
const CAPABILITY_MANIFEST: usize = 10;
const CAPABILITY_MANIFEST_STAGING: usize = 11;
const SOURCE_STATE: usize = 12;
const FUNDING_LEDGER: usize = 13;

const CREATE_RENT: usize = 14;
const CREATE_SYSTEM: usize = 15;
const CREATE_RECOVERY_POLICY: usize = 16;
const CREATE_RECOVERY_POLICY_STAGING: usize = 17;
const VERIFY_BENEFICIARY: usize = 14;
const VERIFY_CLOCK: usize = 15;
const VERIFY_RENT: usize = 16;
const VERIFY_ACTIVATION_RECEIPT: usize = 17;
const VERIFY_RECOVERY_POLICY: usize = 18;
const VERIFY_RECOVERY_POLICY_STAGING: usize = 19;
const ADMIT_CERTIFICATE: usize = 14;
const ADMIT_RENT: usize = 15;
const ADMIT_PRODUCT: usize = 16;
const ADMIT_PRODUCT_STAGING: usize = 17;
const ADMIT_RESULT_DOMAIN: usize = 18;
const ADMIT_RESULT_DOMAIN_STAGING: usize = 19;
const ADMIT_PORTFOLIO: usize = 20;
const ADMIT_PORTFOLIO_STAGING: usize = 21;

const RESOLUTION_FUNDING_LEDGER_BYTES: usize =
    FUNDING_LEDGER_HEADER_BYTES_V2 + 3 * FUNDING_LEDGER_SLOT_BYTES_V2;

/// The three Resolution actions Core still COMPOSES.
///
/// `ResolutionCoreActionV1` is the WIRE enum and it keeps all four
/// discriminants, because Resolution still owns `CloseFund` and decodes it on
/// its own direct route (`process_direct_funding_close_v1`). Core does not:
/// `a34ff595` moved the close out of Core, and `process` refuses that action at
/// decode, before an account is parsed.
///
/// What that refusal could not do by itself is stop Core's own code from NAMING
/// the action. Twelve arms across nine helpers went on matching `CloseFund` and
/// returning frame widths, account indices, expected-writable pairs and
/// revision rules for a route Core refuses — held there by Rust totality rather
/// than by anything anyone had decided. Each was dead code that READ like
/// specification, and the risk is not that it executes: it is that the next
/// reader takes it for the design, or a later edit makes one of them reachable
/// again by widening the decode guard alone.
///
/// So the refusal is enforced ONCE, at the decode site, by producing this type;
/// past that point Core cannot name an action it refuses, because there is no
/// variant for it. The wire enum is untouched, so nothing about what Resolution
/// accepts changes, and the census keeps reading `#CloseFund` off the guard
/// that still names it.
///
/// `pub(crate)` only because `recovery_walk_has_a_live_route` is, and the weld
/// test re-executes its premise beside it. Nothing outside this crate can name
/// it, which is the whole point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComposedResolutionActionV1 {
    CreateFund,
    VerifyFundReady,
    AdmitTerminal,
}

/// Execute one exact Resolution child effect and commit any Core transition last.
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
) -> Result<(), ProgramError> {
    if role_request.len() != RESOLUTION_ROLE_REQUEST_BYTES_V1 {
        return Err(CoreSbfError::Instruction.into());
    }
    let funding_header = CapabilityFundingHeaderV2::decode(
        role_request
            .get(..CAPABILITY_FUNDING_HEADER_BYTES_V2)
            .ok_or(CoreSbfError::Instruction)?,
    )
    .map_err(|_| CoreSbfError::Instruction)?;
    if funding_header.physical_count() != 1 || funding_header.logical_count() != 3 {
        return Err(CoreSbfError::Instruction.into());
    }
    let resolution_request = ResolutionRoleRequestV2::decode(
        role_request
            .get(CAPABILITY_FUNDING_HEADER_BYTES_V2..)
            .ok_or(CoreSbfError::Instruction)?,
    )
    .map_err(|_| CoreSbfError::Instruction)?;
    // The one place the wire enum is read, and the one place the refusal lives.
    // Everything below this line is typed in `ComposedResolutionActionV1` and
    // therefore CANNOT name `CloseFund`.
    let action = match resolution_request.action {
        // V7 closes directly in Resolution (`process_direct_funding_close_v1`).
        // Retaining the composed Core CPI would preserve the exact
        // duplicate-authentication route that exceeds the transaction compute
        // ceiling, so Core is not this route's owner any more. Refused here, at
        // decode, before a single account is parsed or a signature checked: the
        // instruction cannot succeed, and spending the authentication on it
        // first would only make the refusal expensive.
        ResolutionCoreActionV1::CloseFund => {
            return Err(CoreSbfError::UnsupportedAction.into());
        }
        // Every action Core still composes continues into the shared
        // authentication below and is dispatched after it.
        ResolutionCoreActionV1::CreateFund => ComposedResolutionActionV1::CreateFund,
        ResolutionCoreActionV1::VerifyFundReady => ComposedResolutionActionV1::VerifyFundReady,
        ResolutionCoreActionV1::AdmitTerminal => ComposedResolutionActionV1::AdmitTerminal,
    };
    if funding_header.selected_mask()
        != resolution_request
            .funding_entry_mask()
            .map_err(|_| CoreSbfError::Funding)?
    {
        return Err(CoreSbfError::Instruction.into());
    }
    authenticate_action(request, envelope, resolution_request, action)?;
    validate_outer_frame(program_id, accounts, action)?;
    let frame = FixedRoleAccountsV1::parse(program_id, accounts)?;
    let authenticated = authenticate_fixed_role(
        program_id,
        &frame,
        request,
        envelope,
        role_request,
        Role::Resolution,
    )?;
    authenticate_request_coordinates(
        &frame,
        *authenticated.state,
        envelope,
        resolution_request,
        action,
    )?;
    if action != ComposedResolutionActionV1::AdmitTerminal {
        authenticate_recovery_policy(&frame, accounts, resolution_request, action)?;
    }
    match action {
        ComposedResolutionActionV1::VerifyFundReady => {
            authenticate_activation_accept(
                &frame,
                accounts,
                *authenticated.state,
                resolution_request,
                *authenticated.target_admission,
            )?;
            require_market_unchanged(&frame, authenticated.state_bytes.as_ref())?;
            commit_verified_readiness(
                &frame,
                request,
                *authenticated.state,
                *authenticated.core_admission,
            )?;
            Ok(())
        }
        ComposedResolutionActionV1::AdmitTerminal => {
            // The provider transaction already persisted a Resolution-owned
            // ResolutionCertificateV2. Core independently authenticates that
            // durable poststate above/below and accepts it without asking
            // Resolution to repeat the same release, product, Source, ledger,
            // and certificate work in a child invocation.
            let projection = authenticate_admit_projection(
                &frame,
                accounts,
                *authenticated.state,
                resolution_request,
            )?;
            require_market_unchanged(&frame, authenticated.state_bytes.as_ref())?;
            if let Some(existing) = authenticated.state.terminal_receipt {
                if existing != projection.receipt.receipt_id {
                    return Err(CoreSbfError::Transition.into());
                }
                return Ok(());
            }
            let mut candidate = *authenticated.state;
            admit_terminal(
                request,
                &mut candidate,
                *authenticated.target_admission,
                projection.product,
                true,
                projection.receipt,
            )
            .map_err(|_| CoreSbfError::Transition)?;
            persist_state(frame.market(), candidate)?;
            Ok(())
        }
        ComposedResolutionActionV1::CreateFund => {
            invoke_fixed_role(
                program_id,
                &frame,
                envelope,
                envelope_bytes,
                role_request,
                // The child frame is the outer frame, whichever of its two
                // admissible widths `validate_outer_frame` accepted.
                accounts.len(),
            )?;
            let acknowledgement =
                authenticate_fixed_role_ack(&frame, envelope, envelope_bytes, role_request)?;
            require_market_unchanged(&frame, authenticated.state_bytes.as_ref())?;
            authenticate_poststate(
                &frame,
                accounts,
                *authenticated.state,
                resolution_request,
                acknowledgement,
                action,
            )?;
            Ok(())
        }
    }
}

/// Commit the readiness transition `VerifyFundReady` just authenticated, if the
/// Market still has one to make.
///
/// This guard is what makes the action idempotent, and it is a guard rather
/// than a refusal on purpose. `authenticate_request_coordinates` admits three
/// prestates for `VerifyFundReady`, and only `Founding + Prepaid` has a
/// transition left: the other two describe a Market whose activation was
/// already accepted once. Re-presenting the same authenticated activation is
/// not an error, so it commits nothing and returns.
#[inline(never)]
fn commit_verified_readiness(
    frame: &FixedRoleAccountsV1<'_, '_>,
    request: Request,
    state: CoreState,
    core_admission: dclutch_market_core_codec::Admission,
) -> Result<(), ProgramError> {
    if state.phase != Phase::Founding || state.readiness != Readiness::Prepaid {
        return Ok(());
    }
    let mut candidate = state;
    verify_readiness(
        request,
        &mut candidate,
        core_admission,
        true,
        complete_child_effect(),
    )
    .map_err(|_| CoreSbfError::Transition)?;
    persist_state(frame.market(), candidate)?;
    Ok(())
}

#[inline(never)]
fn authenticate_action(
    request: Request,
    envelope: CoreEffectEnvelopeV1,
    resolution: ResolutionRoleRequestV2,
    action: ComposedResolutionActionV1,
) -> Result<(), CoreSbfError> {
    let (top_level, effect) = match action {
        ComposedResolutionActionV1::CreateFund => {
            (Action::VerifyReadiness, CoreEffectActionV1::CreateFund)
        }
        ComposedResolutionActionV1::VerifyFundReady => {
            (Action::VerifyReadiness, CoreEffectActionV1::VerifyFundReady)
        }
        ComposedResolutionActionV1::AdmitTerminal => {
            (Action::AdmitTerminal, CoreEffectActionV1::AdmitTerminal)
        }
    };
    if request.action != top_level
        || envelope.action() != effect
        || envelope.target_role() != Role::Resolution
        || envelope.context().to_bytes() != resolution.source_state
    {
        return Err(CoreSbfError::Instruction);
    }
    let revisions_match = match action {
        ComposedResolutionActionV1::CreateFund | ComposedResolutionActionV1::VerifyFundReady => {
            envelope.expected_resource_a_revision() == 0
                && envelope.expected_resource_b_revision() == 0
        }
        ComposedResolutionActionV1::AdmitTerminal => {
            envelope.expected_resource_a_revision() == resolution.receipt_sequence
                && envelope.expected_resource_b_revision() == 1
        }
    };
    if !revisions_match {
        return Err(CoreSbfError::Instruction);
    }
    Ok(())
}

#[inline(never)]
fn authenticate_request_coordinates(
    frame: &FixedRoleAccountsV1<'_, '_>,
    state: CoreState,
    envelope: CoreEffectEnvelopeV1,
    request: ResolutionRoleRequestV2,
    action: ComposedResolutionActionV1,
) -> Result<(), CoreSbfError> {
    if request.source_state
        != account(frame.child_accounts(14)?, SOURCE_STATE)?
            .key
            .to_bytes()
        || request.funding_ledger
            != account(frame.child_accounts(14)?, FUNDING_LEDGER)?
                .key
                .to_bytes()
        || request.source_material != state.identity.resolution_policy.to_bytes()
        || request.capability_manifest != state.identity.capability_manifest.to_bytes()
        || envelope.release_set() != state.identity.selected_release_set
    {
        return Err(CoreSbfError::Reference);
    }
    let beneficiary_matches = match action {
        ComposedResolutionActionV1::CreateFund => {
            request.beneficiary == state.rent_beneficiary.to_bytes()
        }
        ComposedResolutionActionV1::VerifyFundReady => {
            request.beneficiary == state.rent_beneficiary.to_bytes()
                && request.beneficiary
                    == account(
                        frame.child_accounts(VERIFY_BENEFICIARY + 1)?,
                        VERIFY_BENEFICIARY,
                    )?
                    .key
                    .to_bytes()
        }
        ComposedResolutionActionV1::AdmitTerminal => request.beneficiary == [0; 32],
    };
    if !beneficiary_matches {
        return Err(CoreSbfError::Reference);
    }
    let valid_phase = match action {
        // A Market that has already minted a terminal receipt is refused from
        // both directions. `Terminal`, `Retiring` and `Retired` are excluded by
        // the declared prestates, and the receipt is also checked directly so
        // that a phase added later cannot inherit this admission by accident.
        ComposedResolutionActionV1::CreateFund => {
            state.terminal_receipt.is_none()
                && CREATE_FUND_ADMISSIBLE_PRESTATES_V1.admits(state.phase, state.readiness)
        }
        ComposedResolutionActionV1::VerifyFundReady => {
            VERIFY_FUND_READY_ADMISSIBLE_PRESTATES_V1.admits(state.phase, state.readiness)
        }
        ComposedResolutionActionV1::AdmitTerminal => {
            ADMIT_TERMINAL_ADMISSIBLE_PRESTATES_V1.admits(state.phase, state.readiness)
        }
    };
    if !valid_phase {
        return Err(CoreSbfError::Transition);
    }
    Ok(())
}

/// Authenticate the immutable V7 activation receipt and its exact live Active ledger.
#[inline(never)]
fn authenticate_activation_accept(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV2,
    target_admission: dclutch_market_core_codec::Admission,
) -> Result<(), CoreSbfError> {
    let rent = read_rent(account(accounts, VERIFY_RENT)?)?;
    authenticate_live_poststate(
        frame,
        accounts,
        state,
        request,
        ComposedResolutionActionV1::VerifyFundReady,
        &rent,
    )?;
    let receipt_account = account(accounts, VERIFY_ACTIVATION_RECEIPT)?;
    let generation_seed = state.identity.generation.to_le_bytes();
    let expected_receipt = Pubkey::find_program_address(
        &[
            FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            frame.market().key.as_ref(),
            &generation_seed,
        ],
        frame.target_program().key,
    )
    .0;
    if receipt_account.key != &expected_receipt
        || receipt_account.owner != frame.target_program().key
        || receipt_account.data_len() != FUNDING_ACTIVATION_RECEIPT_BYTES_V1
        || !rent.is_exempt(
            receipt_account.lamports(),
            FUNDING_ACTIVATION_RECEIPT_BYTES_V1,
        )
        || target_admission
            .receipt
            .observed
            .semantic_release
            .to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V7
    {
        return Err(CoreSbfError::Reference);
    }
    let receipt_data = receipt_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let receipt =
        FundingActivationReceiptV1::decode(&receipt_data).map_err(|_| CoreSbfError::Reference)?;
    let market_data = frame
        .market()
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Market)?;
    let source_data = account(accounts, SOURCE_STATE)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let ledger_account = account(accounts, FUNDING_LEDGER)?;
    let ledger_data = ledger_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Funding)?;
    let manifest_data = account(accounts, CAPABILITY_MANIFEST)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
    let manifest_id =
        CapabilityContentId::new(request.capability_manifest).map_err(|_| CoreSbfError::Funding)?;
    let active = FundingLedgerV2::decode(&ledger_data)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .map_err(|_| CoreSbfError::Funding)?;
    let remaining_native_principal_lamports = active
        .remaining_native_lamports_total()
        .map_err(|_| CoreSbfError::Funding)?;
    let ledger_rent_lamports = rent.minimum_balance(RESOLUTION_FUNDING_LEDGER_BYTES);
    // The receipt binds the exact Prepaid Market observed by Resolution. Once
    // Core commits Ready, a repeated Accept must reauthenticate that immutable
    // predecessor instead of treating the current Ready bytes as a new
    // activation request. This makes crash recovery suffix-only and does not
    // weaken CreateFund or direct activation admission.
    let market_state_digest = activation_receipt_market_digest(state, &market_data)?;
    let source_state_digest = hash(&source_data).to_bytes();
    let active_ledger_digest = funding_lifecycle_account_digest_v1(
        ledger_account.owner.to_bytes(),
        ledger_account.key.to_bytes(),
        ledger_account.lamports(),
        &ledger_data,
    );
    let activation_request = FundingActivationRequestV1 {
        release_set: state.identity.selected_release_set.to_bytes(),
        market: frame.market().key.to_bytes(),
        generation: state.identity.generation,
        role: request,
        expected_market_state_digest: market_state_digest,
        expected_source_state_digest: source_state_digest,
        expected_pending_ledger_digest: receipt.pending_ledger_digest,
        receipt: receipt_account.key.to_bytes(),
    };
    if receipt.request_digest
        != activation_request
            .digest()
            .map_err(|_| CoreSbfError::Reference)?
        || receipt.release_set != state.identity.selected_release_set.to_bytes()
        || receipt.resolution_release != RESOLUTION_CONTROLLER_RELEASE_ID_V7
        || receipt.market != frame.market().key.to_bytes()
        || receipt.generation != state.identity.generation
        || receipt.role != request
        || receipt.market_state_digest != market_state_digest
        || receipt.source_state_digest != source_state_digest
        || receipt.active_ledger_digest != active_ledger_digest
        || receipt.ledger_rent_lamports != ledger_rent_lamports
        || receipt.remaining_native_principal_lamports != remaining_native_principal_lamports
        || receipt.post_ledger_lamports != ledger_account.lamports()
        || receipt.producer != frame.target_program().key.to_bytes()
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn activation_receipt_market_digest(
    state: CoreState,
    current_bytes: &[u8],
) -> Result<[u8; 32], CoreSbfError> {
    if state.phase == Phase::Founding && state.readiness == Readiness::Ready {
        let mut predecessor = state;
        predecessor.readiness = Readiness::Prepaid;
        return Ok(hash(&predecessor.encode().map_err(|_| CoreSbfError::Transition)?).to_bytes());
    }
    Ok(hash(current_bytes).to_bytes())
}

#[inline(never)]
fn validate_outer_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: ComposedResolutionActionV1,
) -> Result<(), CoreSbfError> {
    let expected = outer_account_count(action);
    // The three fund actions end with the finalized RecoveryPolicyV2 pair. A
    // material that bought no recovery walk has no such record, so its frame
    // is the same frame without those two tail positions; whether the short
    // shape is admissible is decided against the authenticated material in
    // `authenticate_recovery_policy`, not here.
    let admissible_count = accounts.len() == expected
        || (action != ComposedResolutionActionV1::AdmitTerminal
            && accounts.len() == expected.saturating_sub(2));
    if !admissible_count || accounts.iter().any(|value| value.is_signer) {
        return Err(CoreSbfError::AccountFrame);
    }
    let has_policy_positions = accounts.len() == expected;
    require_distinct(accounts)?;
    let common = FixedRoleAccountsV1::parse(program_id, accounts)?;
    if common.market().owner != program_id {
        return Err(CoreSbfError::Market);
    }
    for index in [
        SOURCE_MATERIAL,
        SOURCE_MATERIAL_STAGING,
        CAPABILITY_MANIFEST,
        CAPABILITY_MANIFEST_STAGING,
    ] {
        let value = account(accounts, index)?;
        if value.is_writable || value.executable {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    let expected_writable = match action {
        ComposedResolutionActionV1::CreateFund => [true, false],
        ComposedResolutionActionV1::VerifyFundReady => [false, false],
        ComposedResolutionActionV1::AdmitTerminal => [false, false],
    };
    for (index, writable) in [SOURCE_STATE, FUNDING_LEDGER]
        .into_iter()
        .zip(expected_writable)
    {
        let value = account(accounts, index)?;
        if value.is_writable != writable || value.executable {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    match action {
        ComposedResolutionActionV1::CreateFund => {
            require_sysvar(account(accounts, CREATE_RENT)?, sysvar::rent::ID)?;
            require_program(account(accounts, CREATE_SYSTEM)?, system_program::ID)?;
            if has_policy_positions {
                require_readonly_pair(
                    accounts,
                    CREATE_RECOVERY_POLICY,
                    CREATE_RECOVERY_POLICY_STAGING,
                )?;
            }
        }
        ComposedResolutionActionV1::VerifyFundReady => {
            let beneficiary = account(accounts, VERIFY_BENEFICIARY)?;
            let receipt = account(accounts, VERIFY_ACTIVATION_RECEIPT)?;
            if beneficiary.is_writable
                || beneficiary.executable
                || receipt.is_writable
                || receipt.executable
            {
                return Err(CoreSbfError::AccountFrame);
            }
            require_sysvar(account(accounts, VERIFY_CLOCK)?, sysvar::clock::ID)?;
            require_sysvar(account(accounts, VERIFY_RENT)?, sysvar::rent::ID)?;
            if has_policy_positions {
                require_readonly_pair(
                    accounts,
                    VERIFY_RECOVERY_POLICY,
                    VERIFY_RECOVERY_POLICY_STAGING,
                )?;
            }
        }
        ComposedResolutionActionV1::AdmitTerminal => {
            let certificate = account(accounts, ADMIT_CERTIFICATE)?;
            if certificate.is_writable || certificate.executable {
                return Err(CoreSbfError::AccountFrame);
            }
            require_sysvar(account(accounts, ADMIT_RENT)?, sysvar::rent::ID)?;
            for index in [
                ADMIT_PRODUCT,
                ADMIT_PRODUCT_STAGING,
                ADMIT_RESULT_DOMAIN,
                ADMIT_RESULT_DOMAIN_STAGING,
                ADMIT_PORTFOLIO,
                ADMIT_PORTFOLIO_STAGING,
            ] {
                let value = account(accounts, index)?;
                if value.is_writable || value.executable {
                    return Err(CoreSbfError::AccountFrame);
                }
            }
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_recovery_policy(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    request: ResolutionRoleRequestV2,
    action: ComposedResolutionActionV1,
) -> Result<(), CoreSbfError> {
    let (policy_index, staging_index) = recovery_policy_indices(action)?;
    let rent = read_rent(account(accounts, rent_index(action))?)?;
    let material_account = account(accounts, SOURCE_MATERIAL)?;
    let material_data = material_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if material_data.len() != SOURCE_MATERIAL_V3_BYTES {
        return Err(CoreSbfError::Reference);
    }
    authenticate_finalized_record(
        frame.registry().key,
        material_account,
        account(accounts, SOURCE_MATERIAL_STAGING)?,
        &rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.source_material,
        &material_data,
    )?;
    let material = SourceMaterialV3::decode(&material_data).map_err(|_| CoreSbfError::Reference)?;
    // A material either bought an ordered recovery walk or it did not, and the
    // two shapes authenticate differently on purpose. `Some` is the original
    // path, byte for byte. `None` is the §12.7/§12.8 no-recovery market — the
    // one whose whole failure walk is the funded
    // `Primary -> Exhausted -> FailureCommitted` — and before this arm existed
    // it could not exist at all: this guard demanded a recovery policy the
    // exhaust transition refuses, so no live founding could ever reach the
    // executed walk.
    let policy = match material.recovery_policy() {
        Some(recovery_id) => {
            // Liveness census R2/Q2: the ordered recovery ladder has no live
            // route, so a fund created over this material would have no
            // terminal at all. Refuse to create it. See
            // `recovery_walk_has_a_live_route`.
            if !recovery_walk_has_a_live_route(action) {
                return Err(CoreSbfError::RecoveryWalkUnavailable);
            }
            let policy_account = account(accounts, policy_index)?;
            let policy_data = policy_account
                .try_borrow_data()
                .map_err(|_| CoreSbfError::FinalizedRecord)?;
            if policy_data.len() != RECOVERY_POLICY_BYTES_V2 {
                return Err(CoreSbfError::Reference);
            }
            authenticate_finalized_record(
                frame.registry().key,
                policy_account,
                account(accounts, staging_index)?,
                &rent,
                RECOVERY_POLICY_SCHEMA_ID_V2,
                recovery_id.to_bytes(),
                &policy_data,
            )?;
            let policy =
                RecoveryPolicyV2::decode(&policy_data).map_err(|_| CoreSbfError::Reference)?;
            if policy.attempt_count() != 1 {
                return Err(CoreSbfError::Reference);
            }
            Some((recovery_id, policy))
        }
        None => {
            // No recovery policy record exists, so the frame carries no
            // positions for one: the short frame IS the statement that none
            // exists, and the authenticated material is what makes that
            // statement checkable rather than a caller's choice. The two
            // policy indices are the frame's own tail, so absence is exactly
            // "the frame ends before them".
            if accounts.len() > policy_index {
                return Err(CoreSbfError::Reference);
            }
            let _ = staging_index;
            None
        }
    };
    let manifest_data = account(accounts, CAPABILITY_MANIFEST)?
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_finalized_record(
        frame.registry().key,
        account(accounts, CAPABILITY_MANIFEST)?,
        account(accounts, CAPABILITY_MANIFEST_STAGING)?,
        &rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.capability_manifest,
        &manifest_data,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
    match policy {
        Some((recovery_id, policy)) => {
            let recovery_allocation = policy
                .attempt(0)
                .map_err(|_| CoreSbfError::Reference)?
                .funding_allocation_id()
                .to_bytes();
            for (entry_index, expected_config) in [
                (request.recovery_entry_index, recovery_allocation),
                (request.exhaustion_entry_index, recovery_id.to_bytes()),
                (request.failure_entry_index, request.source_material),
            ] {
                let entry = manifest
                    .entry(entry_index)
                    .map_err(|_| CoreSbfError::Funding)?;
                if entry.config_id().to_bytes() != expected_config
                    || entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7
                {
                    return Err(CoreSbfError::Funding);
                }
            }
        }
        None => authenticate_no_recovery_entries(manifest, request)?,
    }
    Ok(())
}

/// Pin the three funding entries of a material that bought no recovery walk.
///
/// With no recovery policy there is no allocation identity and no policy
/// digest to pin the recovery and exhaustion entries to, so the rule becomes
/// structural: three pairwise-distinct Resolution-controller entries, exactly
/// one of which — the failure entry — is configured by this market's own
/// Source material. The funded deadline walk admits a compartment by that
/// same configuration comparison (`funded::plan_funding_release`), so the two
/// non-material compartments can never stand in for the escrow; they exist,
/// prepaid, until `CloseFund` refunds them with the failure compartment.
#[inline(never)]
fn authenticate_no_recovery_entries(
    manifest: CapabilityManifestV1<'_>,
    request: ResolutionRoleRequestV2,
) -> Result<(), CoreSbfError> {
    if request.recovery_entry_index == request.exhaustion_entry_index
        || request.recovery_entry_index == request.failure_entry_index
        || request.exhaustion_entry_index == request.failure_entry_index
    {
        return Err(CoreSbfError::Funding);
    }
    let mut configs = [[0_u8; 32]; 3];
    for (slot, entry_index) in [
        request.recovery_entry_index,
        request.exhaustion_entry_index,
        request.failure_entry_index,
    ]
    .into_iter()
    .enumerate()
    {
        let entry = manifest
            .entry(entry_index)
            .map_err(|_| CoreSbfError::Funding)?;
        if entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7 {
            return Err(CoreSbfError::Funding);
        }
        if let Some(config) = configs.get_mut(slot) {
            *config = entry.config_id().to_bytes();
        }
    }
    let [recovery_config, exhaustion_config, failure_config] = configs;
    if failure_config != request.source_material
        || recovery_config == request.source_material
        || exhaustion_config == request.source_material
        || recovery_config == exhaustion_config
    {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

/// May this action still touch a material that bought an ordered recovery walk?
///
/// Liveness census R2 / queue Q2, welded on the 16:36 board ruling. A recovery
/// material's failure walk is dead code in both directions:
/// `SourceResolutionStateV2::exhaust_after_primary_deadline` refuses
/// `recovery_policy().is_some()` outright
/// (`crates/dclutch-source-contract/src/source_resolution_v2.rs`,
/// `Error::RecoveryNotExhausted`), and the ladder that was supposed to consume
/// the paid-for legs instead — `funded::process_funded_transition` — has
/// exactly one call site, inside a `#[cfg(any())]` function
/// (`programs/dclutch-resolution-proof-sbf/src/lib.rs`). So at the resolution
/// deadline such a market admits neither the success capture nor the failure
/// walk: it is stuck in `Primary` forever, with every holder's principal in it.
///
/// The weld is exactly one conjunct and it sits on **creation only**.
/// `CreateFund` is what mints the `SourceResolutionStateV2` — the object with
/// no exit — so refusing there is refusing to bring the unresolvable thing into
/// existence, before any position can be sold against it.
/// `VerifyFundReady` stays admissible on purpose: welding it would take a route
/// *away* from a state that already exists, which is the opposite of the
/// census's charter. A weld may not strand what it finds. (`CloseFund` used to
/// be listed below for totality only; it is not an action Core composes, so it
/// is not a variant this function can be asked about any more.)
///
/// This returns `true` again the moment the ladder gets a live route (Q2's
/// build half: resurrect `funded::process_funded_transition` and give
/// `RecoveryAdvanced`/`Exhausted` real routes). Deleting this function is then
/// the whole of the revert.
pub(crate) const fn recovery_walk_has_a_live_route(action: ComposedResolutionActionV1) -> bool {
    match action {
        ComposedResolutionActionV1::CreateFund => false,
        ComposedResolutionActionV1::VerifyFundReady | ComposedResolutionActionV1::AdmitTerminal => {
            true
        }
    }
}

fn recovery_policy_indices(
    action: ComposedResolutionActionV1,
) -> Result<(usize, usize), CoreSbfError> {
    match action {
        ComposedResolutionActionV1::CreateFund => {
            Ok((CREATE_RECOVERY_POLICY, CREATE_RECOVERY_POLICY_STAGING))
        }
        ComposedResolutionActionV1::VerifyFundReady => {
            Ok((VERIFY_RECOVERY_POLICY, VERIFY_RECOVERY_POLICY_STAGING))
        }
        ComposedResolutionActionV1::AdmitTerminal => Err(CoreSbfError::Instruction),
    }
}

fn require_readonly_pair(
    accounts: &[AccountInfo<'_>],
    raw_index: usize,
    staging_index: usize,
) -> Result<(), CoreSbfError> {
    for index in [raw_index, staging_index] {
        let value = account(accounts, index)?;
        if value.is_writable || value.executable {
            return Err(CoreSbfError::AccountFrame);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct AdmitProjection {
    product: Product,
    receipt: TerminalReceipt,
}

#[inline(never)]
fn authenticate_admit_projection(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV2,
) -> Result<AdmitProjection, CoreSbfError> {
    let rent = read_rent(account(accounts, ADMIT_RENT)?)?;
    let registry = frame.registry().key;
    let material_account = account(accounts, SOURCE_MATERIAL)?;
    let material_data = material_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if material_data.len() != SOURCE_MATERIAL_V3_BYTES {
        return Err(CoreSbfError::Reference);
    }
    authenticate_finalized_record(
        registry,
        material_account,
        account(accounts, SOURCE_MATERIAL_STAGING)?,
        &rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        request.source_material,
        &material_data,
    )?;
    let material = SourceMaterialV3::decode(&material_data).map_err(|_| CoreSbfError::Reference)?;

    let runtime = authenticate_selected_runtime_v2(
        registry,
        &rent,
        state.identity.product_record.to_bytes(),
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: account(accounts, ADMIT_PRODUCT)?,
                staging: account(accounts, ADMIT_PRODUCT_STAGING)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: account(accounts, ADMIT_RESULT_DOMAIN)?,
                staging: account(accounts, ADMIT_RESULT_DOMAIN_STAGING)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: account(accounts, ADMIT_PORTFOLIO)?,
                staging: account(accounts, ADMIT_PORTFOLIO_STAGING)?,
            },
        },
    )?;
    let product = project_core_product_v2(runtime)?;
    material
        .authenticate_product_record(
            SourceContentId::new(runtime.product_record.content_digest.to_bytes())
                .map_err(|_| CoreSbfError::Reference)?,
        )
        .map_err(|_| CoreSbfError::Reference)?;
    if runtime.product_record.content_digest.to_bytes() != state.identity.product_record.to_bytes()
        || runtime.product_id.to_bytes() != state.identity.product_id.to_bytes()
    {
        return Err(CoreSbfError::Reference);
    }

    let source_account = account(accounts, SOURCE_STATE)?;
    let source_data = source_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let source =
        SourceResolutionStateV2::decode(&source_data).map_err(|_| CoreSbfError::Reference)?;
    authenticate_source_state(frame.target_program().key, source_account, source)?;
    let expected_receipt_kind = match source.phase() {
        SourceResolutionPhaseV1::Resolved => ResolutionCoreReceiptKindV1::TerminalSuccess,
        SourceResolutionPhaseV1::FailureCommitted => ResolutionCoreReceiptKindV1::TerminalFailure,
        _ => return Err(CoreSbfError::Reference),
    };
    if request.receipt_kind != expected_receipt_kind
        || source.market() != frame.market().key.to_bytes()
        || source.generation() != state.identity.generation
        || source.material_id().to_bytes() != request.source_material
    {
        return Err(CoreSbfError::Reference);
    }
    let decision = source
        .decision(product.outcome_count)
        .map_err(|_| CoreSbfError::Transition)?;
    if decision.terminal_sequence() != request.receipt_sequence {
        return Err(CoreSbfError::Transition);
    }
    let certificate_account = account(accounts, ADMIT_CERTIFICATE)?;
    let certificate_data = certificate_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Reference)?;
    let certificate = authenticate_terminal_certificate(
        frame.target_program().key,
        source_account,
        certificate_account,
        request,
        state,
        decision.selector(),
        product.outcome_count,
        &certificate_data,
        &rent,
    )?;
    Ok(AdmitProjection {
        product,
        receipt: TerminalReceipt {
            receipt_id: nonzero_identity(certificate_account.key.to_bytes())?,
            market_id: state.identity.market_id,
            resolution_policy: state.identity.resolution_policy,
            product_id: state.identity.product_id,
            generation: state.identity.generation,
            selector: certificate.selector,
            authenticated: true,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_terminal_certificate(
    resolution_program: &Pubkey,
    source_account: &AccountInfo<'_>,
    certificate_account: &AccountInfo<'_>,
    request: ResolutionRoleRequestV2,
    state: CoreState,
    selector: u32,
    outcome_count: u32,
    bytes: &[u8],
    rent: &Rent,
) -> Result<ResolutionCertificateV2, CoreSbfError> {
    let (expected_kind, kind_tag) = match request.receipt_kind {
        ResolutionCoreReceiptKindV1::TerminalSuccess => {
            (ResolutionCertificateKindV2::ResolutionSuccess, 1_u8)
        }
        ResolutionCoreReceiptKindV1::TerminalFailure => {
            (ResolutionCertificateKindV2::ResolutionFailure, 4_u8)
        }
        ResolutionCoreReceiptKindV1::None | ResolutionCoreReceiptKindV1::Closure => {
            return Err(CoreSbfError::Reference);
        }
    };
    if certificate_account.key.to_bytes() != request.receipt
        || certificate_account.owner != resolution_program
        || certificate_account.data_len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || !rent.is_exempt(
            certificate_account.lamports(),
            RESOLUTION_CERTIFICATE_BYTES_V2,
        )
    {
        return Err(CoreSbfError::Reference);
    }
    let certificate =
        ResolutionCertificateV2::decode(bytes).map_err(|_| CoreSbfError::Reference)?;
    if certificate.kind != expected_kind
        || certificate.market != state.identity.market_id.to_bytes()
        || certificate.source_material != request.source_material
        || certificate.product_record_digest != state.identity.product_record.to_bytes()
        || certificate.receipt_account != certificate_account.key.to_bytes()
        || certificate.generation != state.identity.generation
        || certificate.selector != selector
    {
        return Err(CoreSbfError::Reference);
    }
    certificate
        .validate_terminal_product(state.identity.product_record.to_bytes(), outcome_count)
        .map_err(|_| CoreSbfError::Reference)?;
    let kind = [kind_tag];
    let sequence = request.receipt_sequence.to_le_bytes();
    let expected = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_account.key.as_ref(),
            &kind,
            &sequence,
        ],
        resolution_program,
    )
    .0;
    if certificate_account.key != &expected {
        return Err(CoreSbfError::Reference);
    }
    Ok(certificate)
}

#[inline(never)]
fn authenticate_poststate(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV2,
    acknowledgement: CoreEffectAckV1,
    action: ComposedResolutionActionV1,
) -> Result<(), CoreSbfError> {
    let rent = read_rent(account(accounts, rent_index(action))?)?;
    let observed_digest = {
        {
            authenticate_live_poststate(frame, accounts, state, request, action, &rent)?;
            let source = account(accounts, SOURCE_STATE)?
                .try_borrow_data()
                .map_err(|_| CoreSbfError::ChildAck)?;
            let funding_ledger = account(accounts, FUNDING_LEDGER)?
                .try_borrow_data()
                .map_err(|_| CoreSbfError::ChildAck)?;
            let certificate = if action == ComposedResolutionActionV1::AdmitTerminal {
                Some(
                    account(accounts, ADMIT_CERTIFICATE)?
                        .try_borrow_data()
                        .map_err(|_| CoreSbfError::ChildAck)?,
                )
            } else {
                None
            };
            resolution_poststate_digest(
                action,
                &source,
                &funding_ledger,
                certificate.as_ref().map(|bytes| bytes.as_ref()),
            )?
        }
    };
    if acknowledgement.post_resource_digest() != observed_digest
        || !ack_revisions_match(acknowledgement, request, action)
    {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn authenticate_live_poststate(
    frame: &FixedRoleAccountsV1<'_, '_>,
    accounts: &[AccountInfo<'_>],
    state: CoreState,
    request: ResolutionRoleRequestV2,
    action: ComposedResolutionActionV1,
    rent: &Rent,
) -> Result<(), CoreSbfError> {
    let source_account = account(accounts, SOURCE_STATE)?;
    if source_account.owner != frame.target_program().key
        || source_account.data_len() != SOURCE_RESOLUTION_STATE_BYTES_V2
    {
        return Err(CoreSbfError::ChildAck);
    }
    let source_data = source_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let source =
        SourceResolutionStateV2::decode(&source_data).map_err(|_| CoreSbfError::ChildAck)?;
    authenticate_source_state(frame.target_program().key, source_account, source)?;
    if source.market() != frame.market().key.to_bytes()
        || source.generation() != state.identity.generation
        || source.material_id().to_bytes() != request.source_material
        || !matches!(
            (action, source.phase()),
            (
                ComposedResolutionActionV1::CreateFund
                    | ComposedResolutionActionV1::VerifyFundReady,
                SourceResolutionPhaseV1::Primary
            ) | (
                ComposedResolutionActionV1::AdmitTerminal,
                SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
            )
        )
    {
        return Err(CoreSbfError::ChildAck);
    }
    let manifest_account = account(accounts, CAPABILITY_MANIFEST)?;
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    authenticate_finalized_record(
        frame.registry().key,
        manifest_account,
        account(accounts, CAPABILITY_MANIFEST_STAGING)?,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.capability_manifest,
        &manifest_data,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
    let manifest_id =
        CapabilityContentId::new(request.capability_manifest).map_err(|_| CoreSbfError::Funding)?;
    let expected_status = if action == ComposedResolutionActionV1::CreateFund {
        FundingLedgerStatusV2::Pending
    } else {
        FundingLedgerStatusV2::Active
    };
    let funding_account = account(accounts, FUNDING_LEDGER)?;
    if funding_account.owner != frame.target_program().key
        || funding_account.data_len() != RESOLUTION_FUNDING_LEDGER_BYTES
    {
        return Err(CoreSbfError::ChildAck);
    }
    let funding_data = funding_account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ChildAck)?;
    let ledger = FundingLedgerV2::decode(&funding_data).map_err(|_| CoreSbfError::Funding)?;
    if ledger.selected_mask()
        != request
            .funding_entry_mask()
            .map_err(|_| CoreSbfError::Funding)?
        || ledger.slot_count() != 3
    {
        return Err(CoreSbfError::Funding);
    }
    let authenticated = ledger
        .authenticate(manifest_id, manifest)
        .map_err(|_| CoreSbfError::Funding)?;
    for entry_index in [
        request.recovery_entry_index,
        request.exhaustion_entry_index,
        request.failure_entry_index,
    ] {
        let slot = authenticated
            .slot(entry_index)
            .map_err(|_| CoreSbfError::Funding)?;
        if slot.status() != expected_status
            || slot.remaining().realm_collateral_total() != 0
            || slot.released().realm_collateral_total() != 0
        {
            return Err(CoreSbfError::ChildAck);
        }
    }
    authenticated
        .validate_native_custody(
            funding_account.lamports(),
            rent.minimum_balance(RESOLUTION_FUNDING_LEDGER_BYTES),
            false,
        )
        .map_err(|_| CoreSbfError::Funding)?;
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        frame.target_program().key.to_bytes(),
        frame.market().key.to_bytes(),
        state.identity.generation,
        manifest_id,
        ledger,
    )
    .map_err(|_| CoreSbfError::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), frame.target_program().key).0
        != *funding_account.key
    {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

fn authenticate_source_state(
    resolution_program: &Pubkey,
    account: &AccountInfo<'_>,
    state: SourceResolutionStateV2,
) -> Result<(), CoreSbfError> {
    let seeds = state.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        resolution_program,
    )
    .map_err(|_| CoreSbfError::Reference)?;
    if account.key != &expected {
        return Err(CoreSbfError::Reference);
    }
    Ok(())
}

fn resolution_poststate_digest(
    action: ComposedResolutionActionV1,
    source_or_closure: &[u8],
    funding_ledger: &[u8],
    certificate: Option<&[u8]>,
) -> Result<dclutch_market_core_codec::Identity, CoreSbfError> {
    // The tag is still the WIRE effect byte, unchanged: the digest is a
    // cross-program agreement with Resolution, not a Core-internal encoding.
    let action_tag = [match action {
        ComposedResolutionActionV1::CreateFund => CoreEffectActionV1::CreateFund as u8,
        ComposedResolutionActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady as u8,
        ComposedResolutionActionV1::AdmitTerminal => CoreEffectActionV1::AdmitTerminal as u8,
    }];
    nonzero_identity(
        hashv(&[
            RESOLUTION_POSTSTATE_DIGEST_DOMAIN_V2,
            &action_tag,
            source_or_closure,
            funding_ledger,
            certificate.unwrap_or(&[]),
        ])
        .to_bytes(),
    )
}

fn ack_revisions_match(
    acknowledgement: CoreEffectAckV1,
    request: ResolutionRoleRequestV2,
    action: ComposedResolutionActionV1,
) -> bool {
    match action {
        ComposedResolutionActionV1::CreateFund => {
            acknowledgement.pre_resource_a_revision() == 0
                && acknowledgement.post_resource_a_revision() == 0
                && acknowledgement.pre_resource_b_revision() == 0
                && acknowledgement.post_resource_b_revision() == 0
        }
        ComposedResolutionActionV1::VerifyFundReady => {
            acknowledgement.pre_resource_a_revision() == 0
                && acknowledgement.post_resource_a_revision() == 0
                && acknowledgement.pre_resource_b_revision() == 0
                && acknowledgement.post_resource_b_revision() == 1
        }
        ComposedResolutionActionV1::AdmitTerminal => {
            acknowledgement.pre_resource_a_revision() == request.receipt_sequence
                && acknowledgement.post_resource_a_revision() == request.receipt_sequence
                && acknowledgement.pre_resource_b_revision() == 1
                && acknowledgement.post_resource_b_revision() == 1
        }
    }
}

const fn complete_child_effect() -> ChildEffectObservation {
    ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

const fn outer_account_count(action: ComposedResolutionActionV1) -> usize {
    match action {
        ComposedResolutionActionV1::CreateFund => RESOLUTION_CREATE_OUTER_ACCOUNT_COUNT_V1,
        ComposedResolutionActionV1::VerifyFundReady => RESOLUTION_VERIFY_OUTER_ACCOUNT_COUNT_V1,
        ComposedResolutionActionV1::AdmitTerminal => RESOLUTION_ADMIT_OUTER_ACCOUNT_COUNT_V1,
    }
}

const fn rent_index(action: ComposedResolutionActionV1) -> usize {
    match action {
        ComposedResolutionActionV1::CreateFund => CREATE_RENT,
        ComposedResolutionActionV1::VerifyFundReady => VERIFY_RENT,
        ComposedResolutionActionV1::AdmitTerminal => ADMIT_RENT,
    }
}

fn read_rent(account: &AccountInfo<'_>) -> Result<Rent, CoreSbfError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID {
        return Err(CoreSbfError::AccountFrame);
    }
    Rent::from_account_info(account).map_err(|_| CoreSbfError::AccountFrame)
}

fn require_sysvar(account: &AccountInfo<'_>, key: Pubkey) -> Result<(), CoreSbfError> {
    if account.key != &key || account.is_writable || account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn require_program(account: &AccountInfo<'_>, key: Pubkey) -> Result<(), CoreSbfError> {
    if account.key != &key || account.is_writable || !account.executable {
        return Err(CoreSbfError::AccountFrame);
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

const _: usize = SOURCE_MATERIAL_V3_BYTES;

#[cfg(test)]
mod tests {
    use dclutch_market_core_codec::Identity;

    use super::*;

    fn identity(value: u8) -> Identity {
        Identity::new([value; 32]).expect("nonzero identity")
    }

    /// One well-formed role request for `action`, in that action's exact shape.
    ///
    /// Well-formed is the whole point: the refusal under test must be earned by
    /// the action itself and not by a malformed request that any action would
    /// have failed on.
    fn role_request(action: ResolutionCoreActionV1) -> [u8; RESOLUTION_ROLE_REQUEST_BYTES_V1] {
        let (receipt_kind, receipt, beneficiary, receipt_sequence) = match action {
            ResolutionCoreActionV1::CreateFund | ResolutionCoreActionV1::VerifyFundReady => {
                (ResolutionCoreReceiptKindV1::None, [0; 32], [6; 32], 0)
            }
            ResolutionCoreActionV1::AdmitTerminal => (
                ResolutionCoreReceiptKindV1::TerminalSuccess,
                [5; 32],
                [0; 32],
                1,
            ),
            ResolutionCoreActionV1::CloseFund => {
                (ResolutionCoreReceiptKindV1::Closure, [5; 32], [6; 32], 2)
            }
        };
        let role = ResolutionRoleRequestV2 {
            action,
            receipt_kind,
            source_state: [1; 32],
            source_material: [2; 32],
            capability_manifest: [3; 32],
            funding_ledger: [4; 32],
            receipt,
            beneficiary,
            recovery_entry_index: 0,
            exhaustion_entry_index: 1,
            failure_entry_index: 2,
            receipt_sequence,
        };
        let body = role.to_bytes().expect("the role request encodes");
        let header = CapabilityFundingHeaderV2::new(
            1,
            3,
            role.funding_entry_mask().expect("three distinct entries"),
        )
        .expect("the funding header encodes")
        .encode();
        let mut bytes = [0_u8; RESOLUTION_ROLE_REQUEST_BYTES_V1];
        bytes[..CAPABILITY_FUNDING_HEADER_BYTES_V2].copy_from_slice(&header);
        bytes[CAPABILITY_FUNDING_HEADER_BYTES_V2..].copy_from_slice(&body);
        bytes
    }

    /// Drive `process` with an empty account frame and return what it refuses.
    ///
    /// Empty is deliberate and is what makes the assertion below meaningful:
    /// every conjunct that reads an account refuses on the frame, so a run that
    /// reports something else reached its subject before touching one.
    fn refusal(action: ResolutionCoreActionV1) -> ProgramError {
        let envelope = CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::CloseFund,
            Role::Resolution,
            identity(7),
            identity(8),
            identity(9),
            identity(10),
            identity(11),
            identity(12),
            identity(13),
            1,
            1,
            1,
            u32::try_from(RESOLUTION_ROLE_REQUEST_BYTES_V1).expect("the width fits"),
        )
        .expect("the envelope encodes");
        process(
            &Pubkey::new_unique(),
            &[],
            Request::administrative(Action::Retire, 1, identity(10)),
            envelope,
            &[],
            &role_request(action),
        )
        .expect_err("no action can succeed against an empty account frame")
    }

    /// V7 moved the Source close out of Core, and the refusal says so by name.
    ///
    /// `CloseFund` decodes cleanly and is still live on the wire — Resolution
    /// dispatches it — so `Instruction`, which accuses the caller's bytes of
    /// being malformed, was the wrong accusation for four months. The code a
    /// reader needs is the one that says Core is not this route's owner.
    #[test]
    fn close_fund_earns_the_unsupported_action_code() {
        assert_eq!(
            refusal(ResolutionCoreActionV1::CloseFund),
            ProgramError::Custom(CoreSbfError::UnsupportedAction as u32)
        );
    }

    /// The positive control for the refusal above.
    ///
    /// Without it `close_fund_earns_the_unsupported_action_code` could pass on
    /// a program that refused every action that way. Each composed action is
    /// driven through the same entry, with the same empty frame, and must reach
    /// the account frame instead — proving the new code is action-specific and
    /// that the CloseFund run really did stop at the decode-time guard.
    #[test]
    fn every_composed_action_outlives_the_decode_time_guard() {
        for action in [
            ResolutionCoreActionV1::CreateFund,
            ResolutionCoreActionV1::VerifyFundReady,
            ResolutionCoreActionV1::AdmitTerminal,
        ] {
            assert_ne!(
                refusal(action),
                ProgramError::Custom(CoreSbfError::UnsupportedAction as u32),
                "a composed action was refused as unsupported"
            );
        }
    }
}
