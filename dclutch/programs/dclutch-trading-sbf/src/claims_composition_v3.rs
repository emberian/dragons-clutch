//! Family-neutral Claims CPI execution for EffectProgram V3 routes.
//!
//! [`ClaimsCompositionV3`] preflights the complete enabled Claims subsequence.
//! The common Trading outer may then call [`execute_claims_route_v3`] in global
//! EffectProgram order, interleaved with other fixed roles. Every invocation
//! uses the release-pinned Trading authority derived from the exact canonical
//! child request and immediately validates the current Claims producer receipt.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use dclutch_capability_program_contract::CapabilityRootSeedsV1;
use dclutch_claims_svm::{
    affine_batch_v2::{AFFINE_BATCH_PLAN_MAGIC_V2, AffineBatchPlanV2, AffineBatchReceiptV2},
    composition_v3::ClaimsCompositionV3,
    founding_v5::{
        CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5, CLAIMS_FOUNDING_REQUEST_MAGIC_V5,
        ClaimsFoundingReceiptV5, ClaimsFoundingRequestV5,
    },
    fractional_claim_check_compaction_receipt_v1::{
        FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1, FractionalClaimCheckCompactionReceiptV1,
    },
    fractional_claim_check_compaction_request_v1::FractionalCompactToClaimCheckRequestV1,
    fractional_claim_check_v1::{
        FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1, FractionalCompactionRoleV1,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_REQUEST_MAGIC_V2, ProtocolPositionActionV2, ProtocolPositionAdmissionV2,
        ProtocolPositionCloseReceiptV2, ProtocolPositionRequestV2,
    },
    series_founding_transport_v1::{
        SERIES_CLAIMS_FOUNDING_TRANSPORT_MAGIC_V1, SeriesClaimsFoundingTransportV1,
    },
    signed_delta_v3::{SIGNED_DELTA_PLAN_MAGIC_V3, SignedDeltaPlanV3, SignedDeltaReceiptV3},
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_MAGIC_V1, SparseNativeTransferPoststateSlicesV1,
        SparseNativeTransferReceiptV1, SparseNativeTransferV1,
        sparse_native_transfer_poststate_digest_v1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ResolvedInvocationV3, RouteKindV3},
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_RECEIPT_BYTES_V3, FRACTIONAL_ATOMIC_ROOT_V3,
    FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2, FRACTIONAL_RETIREMENT_COORDINATE_RECEIPT_BYTES_V3,
    FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3, FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3,
    FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3, FRACTIONAL_TERMINAL_ROOT_V3,
    FractionalAtomicReceiptV3, FractionalExposureActionV2, FractionalExposureRequestV2,
    FractionalRetirementCoordinateReceiptV3, FractionalRetirementRequestV3,
    FractionalTerminalAtomicReceiptV3, decode_fractional_capability_root_v4,
};
use dclutch_rational_representation_v2_contract::{
    CallerRoleV2, RECEIPT_BYTES_V2 as REPRESENTATION_RECEIPT_BYTES_V2,
    REQUEST_MAGIC_V2 as REPRESENTATION_REQUEST_MAGIC_V2, RepresentationReceiptV2,
    RepresentationRequestV2,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_REQUEST_MAGIC_V2, LifecycleReceiptV2, LifecycleRequestV2,
    hot_v3::verify_rational_lifecycle_hot_receipt_v3,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{
    TradingSbfError,
    child_authority_v4::{PreflightedCallerBumpV4, child_caller_authority_v4},
    child_receipt_v3::{
        ReceiptDeliveryV3, deliver_receipt_dependency_v3, receipt_dependency_width_v3,
    },
    hot_v3::{ChildInvocationBuffersV3, DowngradedEffectAccountsV3},
};

/// Exact receipt returned by one canonical Claims route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimsRouteReceiptV3 {
    /// Vacant canonical Position and admission record were admitted.
    Admit(ProtocolPositionAdmissionV2),
    /// Sole affine Claims mutation committed.
    Affine(AffineBatchReceiptV2),
    /// Canonical runtime-width unique signed-delta batch committed.
    SignedDelta(SignedDeltaReceiptV3),
    /// Canonical O(1) native-claim transfer committed.
    SparseNativeTransfer(SparseNativeTransferReceiptV1),
    /// Permit-authorized aggregate, founder Position, and admission were created.
    Founding(Box<ClaimsFoundingReceiptV5>),
    /// One exact Rational receipt/coordinate resource lifecycle committed.
    RationalLifecycle(LifecycleReceiptV2),
    /// One exact Rational representation mutation committed.
    RationalRepresentation(Box<RepresentationReceiptV2>),
    /// One Claims+Token atomic Fractional wrap or whole unwrap committed.
    FractionalAtomic(FractionalAtomicReceiptV3),
    /// One Claims+Custody+Token terminal Fractional mutation committed.
    FractionalTerminalAtomic(FractionalTerminalAtomicReceiptV3),
    /// One zero native Position and terms-selected Mint retired atomically.
    FractionalRetirementCoordinate(FractionalRetirementCoordinateReceiptV3),
    /// One sleeping Fractional reserve compacted into a shard-addressed record.
    FractionalClaimCheckCompaction(FractionalClaimCheckCompactionReceiptV1),
    /// Zero canonical Position and admission record were reclaimed.
    Close(ProtocolPositionCloseReceiptV2),
}

/// Which layer owns the sparse-transfer post-resource/body join.
///
/// Generic Claims routes verify the returned aggregate/source/destination
/// commitment immediately. Direct ordinary already has a stronger typed
/// finalization join after every child CPI: it compares the exact returned
/// receipt inside the child transcript and hashes each actual poststate body
/// against the independently projected candidate. Rehashing all three bodies
/// here as well proves the same conjunction twice on the hottest path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SparsePostResourceVerificationV3 {
    /// Hash the actual child bodies and compare the returned commitment here.
    Immediate,
    /// Defer the body join to Direct's typed finalization verifier.
    DirectFinalization,
}

/// Invoke and verify one preflighted Claims route in global EffectProgram order.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_claims_route_v3<'info>(
    program_id: &Pubkey,
    successor_account_count: usize,
    composition: ClaimsCompositionV3<'_>,
    route_index: u16,
    invocation_index: u32,
    invocation: ResolvedInvocationV3,
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    aliases: &[usize],
    request_bank: &[u8],
    family_request: &[u8],
    prior_receipt: Option<&[u8]>,
    buffers: &mut ChildInvocationBuffersV3<'info>,
    claims_program: &AccountInfo<'info>,
    sparse_post_resource_verification: SparsePostResourceVerificationV3,
    // The caller's mined bump for this route's Trading caller authority. The
    // Claims route is the one child route with no preflight derivation to
    // reuse, so without a hint this is where the search happens. `None` keeps
    // it; `Some` reproduces and refuses at the coordinate-0 equality below.
    hint: PreflightedCallerBumpV4,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    if successor_account_count != effect_accounts.len()
        || !claims_program.executable
        || claims_program.is_writable
        || claims_program.is_signer
    {
        return Err(TradingSbfError::Content.into());
    }
    if invocation_index != 0
        || invocation.role != FixedRole::Claims
        || !composition_owns_route(composition, route_index)
    {
        return Err(TradingSbfError::Content.into());
    }
    let request = invocation_request(invocation, request_bank, family_request)?;
    gather_invocation_accounts(&mut buffers.accounts, invocation, effect_accounts)?;
    // ONE counter, shared with the preflight walk, and ALIAS-AWARE. This used
    // to be a second hand-rolled filter over the gathered infos, and the two
    // copies of the predicate agreed with each other and disagreed with the
    // frame's own AccountProfile: a coordinate declared an
    // `AuthenticatedRouteAlias` counted as a second occurrence of the child
    // program, which is what the Claims representation wire's inactive-slot
    // sentinel always is. See `invocation_accounts_contain_program`.
    if buffers.accounts.is_empty()
        || crate::hot_v3::invocation_accounts_contain_program(
            invocation,
            effect_accounts,
            aliases,
            claims_program.key,
        )? != 1
    {
        return Err(TradingSbfError::Content.into());
    }
    let (authority_seeds, receipt_kind) = route_authority(request, invocation.kind)?;
    let (expected_authority, bump) = child_caller_authority_v4(&authority_seeds, program_id, hint)?;
    if buffers
        .accounts
        .first()
        .is_none_or(|account| account.key != &expected_authority)
    {
        return Err(TradingSbfError::Release.into());
    }

    buffers.fill_metas()?;
    buffers.set_wire(request)?;
    deliver_receipt_dependency_v3(
        invocation,
        &mut buffers.data,
        prior_receipt,
        receipt_kind.delivery(),
    )?;
    if carries_caller_bump_suffix_v4(receipt_kind) {
        buffers
            .data
            .try_reserve(1)
            .map_err(|_| TradingSbfError::Content)?;
        buffers.data.push(bump);
    }
    let fractional_root = fractional_root_signer(
        program_id,
        receipt_kind,
        request,
        &buffers.accounts,
        &mut buffers.metas,
    )?;
    buffers.push_callee(claims_program)?;
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    let caller_signer = [domain, release, market, role, context, digest, &bump_seed];
    if let Some(root) = fractional_root {
        let root_bump = [root.bump];
        let [
            root_domain,
            root_market,
            root_generation,
            root_manifest,
            root_entry,
            root_kind,
            root_release,
            root_config,
        ] = root.seeds.as_slices();
        let root_signer = [
            root_domain,
            root_market,
            root_generation,
            root_manifest,
            root_entry,
            root_kind,
            root_release,
            root_config,
            root_bump.as_slice(),
        ];
        buffers
            .invoke(claims_program.key, &[&caller_signer, &root_signer])
            .map_err(|_| TradingSbfError::Transition)?;
    } else {
        buffers
            .invoke(claims_program.key, &[&caller_signer])
            .map_err(|_| TradingSbfError::Transition)?;
    }
    buffers.capture_return()?;
    if buffers.producer != *claims_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let post_resources = match receipt_kind {
        ReceiptKindV3::SignedDelta => {
            PostResourceEvidenceV3::Single(signed_delta_post_resource_digest(
                &buffers.accounts,
                SignedDeltaPlanV3::decode(request)
                    .map_err(|_| TradingSbfError::Content)?
                    .position_count(),
            )?)
        }
        ReceiptKindV3::SparseNativeTransfer => match sparse_post_resource_verification {
            SparsePostResourceVerificationV3::Immediate => PostResourceEvidenceV3::Single(
                sparse_native_post_resource_digest(&buffers.accounts)?,
            ),
            SparsePostResourceVerificationV3::DirectFinalization => {
                PostResourceEvidenceV3::DeferredDirect
            }
        },
        ReceiptKindV3::Founding | ReceiptKindV3::SeriesFounding => {
            PostResourceEvidenceV3::Founding(founding_post_resource_digests(&buffers.accounts)?)
        }
        ReceiptKindV3::FractionalAtomic => PostResourceEvidenceV3::FractionalRoot(
            buffers
                .accounts
                .get(FRACTIONAL_ATOMIC_ROOT_V3)
                .ok_or(TradingSbfError::Content)?
                .key
                .to_bytes(),
        ),
        ReceiptKindV3::FractionalTerminalAtomic => PostResourceEvidenceV3::FractionalRoot(
            buffers
                .accounts
                .get(FRACTIONAL_TERMINAL_ROOT_V3)
                .ok_or(TradingSbfError::Content)?
                .key
                .to_bytes(),
        ),
        ReceiptKindV3::FractionalRetirementCoordinate => PostResourceEvidenceV3::FractionalRoot(
            buffers
                .accounts
                .get(FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3)
                .ok_or(TradingSbfError::Content)?
                .key
                .to_bytes(),
        ),
        // Stated rather than left to the fallthrough below. The receipt
        // verifier refuses this kind today, so the evidence is unread either
        // way -- which is exactly why writing it now is worth the four lines:
        // when the receipt lands it will be checked against the root, and a
        // `None` inherited from a wildcard would have made that check compare
        // against nothing while still looking verified.
        //
        // The index is the frame declaration's, not a second constant. The
        // three arms above each name their family's own root coordinate; this
        // one asks the enum that owns the compaction frame where its root sits.
        ReceiptKindV3::FractionalClaimCheckCompaction => PostResourceEvidenceV3::FractionalRoot(
            buffers
                .accounts
                .get(
                    FractionalCompactionRoleV1::FractionalCapabilityRoot
                        .index()
                        .ok_or(TradingSbfError::Content)?,
                )
                .ok_or(TradingSbfError::Content)?
                .key
                .to_bytes(),
        ),
        _ => PostResourceEvidenceV3::None,
    };
    verify_route_receipt(
        receipt_kind,
        request,
        &buffers.returned,
        claims_program.key.to_bytes(),
        program_id.to_bytes(),
        post_resources,
    )
}

/// Exact maximum child-wire width this authenticated Claims invocation uses.
///
/// The mutation-free child preflight calls this before the first CPI so the
/// one reusable wire allocation is bought at its final width. Requests with no
/// dependency (including Direct ordinary) need no second request decode. When
/// a dependency exists, the same request-kind owner used by execution decides
/// whether Claims reads it as an exact suffix.
pub(crate) fn claims_child_wire_capacity_v3(
    invocation: ResolvedInvocationV3,
    request_bank: &[u8],
    family_request: &[u8],
) -> Result<usize, ProgramError> {
    let request = invocation_request(invocation, request_bank, family_request)?;
    let receipt_bytes = receipt_dependency_width_v3(invocation);
    let (_, receipt_kind) = route_authority(request, invocation.kind)?;
    let bump_suffix = usize::from(carries_caller_bump_suffix_v4(receipt_kind));
    if receipt_bytes == 0 {
        return request
            .len()
            .checked_add(bump_suffix)
            .ok_or_else(|| TradingSbfError::Content.into());
    }
    let suffix = if receipt_kind.delivery() == ReceiptDeliveryV3::ExactSuffix {
        usize::try_from(receipt_bytes).map_err(|_| TradingSbfError::Content)?
    } else {
        0
    };
    request
        .len()
        .checked_add(suffix)
        .and_then(|width| width.checked_add(bump_suffix))
        .ok_or_else(|| TradingSbfError::Content.into())
}

/// Whether this child route reads a caller-authority bump appended after its
/// request.
///
/// ONE route does, and the restriction is not caution: the Claims dispatcher
/// routes on a magic prefix, so an extra byte reaches every handler, and the
/// five handlers that hash their WHOLE instruction data into the packet digest
/// would refuse it (see [`ReceiptKindV3::delivery`], which records exactly that
/// distinction for the receipt suffix). `sparse_native_transfer_v1` hashes the
/// fixed-width request prefix only, and its
/// `the_caller_authority_digest_covers_the_request_prefix_only` test is what
/// keeps that true.
///
/// A second route joins this list by proving the same property in its own
/// program, with its own named test -- never by being added here.
const fn carries_caller_bump_suffix_v4(kind: ReceiptKindV3) -> bool {
    matches!(kind, ReceiptKindV3::SparseNativeTransfer)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiptKindV3 {
    Admit,
    Affine,
    SignedDelta,
    SparseNativeTransfer,
    Founding,
    SeriesFounding,
    RationalLifecycle,
    RationalRepresentation,
    FractionalAtomic,
    FractionalTerminalAtomic,
    FractionalRetirementCoordinate,
    FractionalClaimCheckCompaction,
    Close,
}

impl ReceiptKindV3 {
    /// What `claims-sbf` does with a producer receipt on THIS request's wire.
    ///
    /// The Claims dispatcher routes on a magic prefix, so a suffix reaches the
    /// handler either way and each handler decides for itself. Three of the
    /// eight read one; the other five hash their WHOLE instruction data into
    /// the packet digest their caller authority is derived from, so bytes
    /// appended after the request do not merely go unread -- they change the
    /// digest and the child refuses. Delivering a receipt there was never a
    /// widening the child tolerated; it was a refusal waiting for a second
    /// child CPI to reach it.
    const fn delivery(self) -> ReceiptDeliveryV3 {
        match self {
            // `protocol_position_v2::split_sparse_receipt` reads an optional
            // trailing `SparseNativeTransferReceiptV1` on Close and refuses any
            // suffix on Admit.
            Self::Close
            // `sparse_native_transfer_v1::split_instruction` reads an optional
            // trailing `ProtocolPositionAdmissionV2`.
            | Self::SparseNativeTransfer
            // `founding_v5::decode_instruction` requires the lock receipt and
            // the projected receipt at an exact total width.
            | Self::Founding
            | Self::SeriesFounding => ReceiptDeliveryV3::ExactSuffix,
            Self::Admit
            | Self::Affine
            | Self::SignedDelta
            | Self::RationalLifecycle
            | Self::RationalRepresentation
            | Self::FractionalAtomic
            | Self::FractionalTerminalAtomic
            | Self::FractionalRetirementCoordinate
            // Same answer as its two Fractional siblings, and for the reason
            // stated above rather than by proximity: the Claims compaction
            // handler will hash its whole instruction data into the packet
            // digest its caller authority is derived from, so a suffix does not
            // merely go unread -- it changes the digest and the child refuses.
            | Self::FractionalClaimCheckCompaction => ReceiptDeliveryV3::VerifiedOnly,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundingPostResourceDigestsV5 {
    aggregate: [u8; 32],
    position: [u8; 32],
    admission: [u8; 32],
    combined: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PostResourceEvidenceV3 {
    None,
    Single([u8; 32]),
    Founding(FoundingPostResourceDigestsV5),
    FractionalRoot([u8; 32]),
    /// The authenticated sparse receipt is joined to actual bodies by the
    /// typed Direct finalization verifier after the complete child walk.
    DeferredDirect,
}

fn composition_owns_route(composition: ClaimsCompositionV3<'_>, route: u16) -> bool {
    composition.admit_route() == Some(route)
        || composition.mutation_route() == route
        || composition.close_route() == Some(route)
}

fn invocation_request<'a>(
    invocation: ResolvedInvocationV3,
    request_bank: &'a [u8],
    family_request: &'a [u8],
) -> Result<&'a [u8], ProgramError> {
    let end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let fixed = request_bank
        .get(invocation.request_offset..end)
        .ok_or(TradingSbfError::Content)?;
    match invocation.borrowed_witness {
        None => Ok(fixed),
        Some(witness) if fixed.is_empty() => {
            let request = witness
                .slice(family_request)
                .map_err(|_| TradingSbfError::Content)?;
            if request.get(..8) == Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice()) {
                let plan =
                    SignedDeltaPlanV3::decode(request).map_err(|_| TradingSbfError::Content)?;
                let parent = family_request
                    .get(..witness.source_offset())
                    .ok_or(TradingSbfError::Content)?;
                if hash(parent).to_bytes() != plan.request_id() {
                    return Err(TradingSbfError::Content.into());
                }
            }
            Ok(request)
        }
        Some(_) => Err(TradingSbfError::Content.into()),
    }
}

/// Gather this invocation's account windows into a caller-owned buffer.
///
/// The buffer is cleared and reserved at the exact width the windows fill, so
/// the appends below never grow it -- and on a second invocation the capacity
/// the first one bought already satisfies the reservation.
fn gather_invocation_accounts<'info>(
    output: &mut Vec<AccountInfo<'info>>,
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
) -> Result<(), ProgramError> {
    accounts.reserve_invocation_frame(output, invocation)?;
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    accounts.extend_window(
        output,
        fixed_start,
        fixed_end
            .checked_sub(fixed_start)
            .ok_or(TradingSbfError::Content)?,
    )?;
    if invocation.kind == RouteKindV3::AffineOnce {
        let count = usize::from(invocation.item_account_count);
        let stride = usize::from(invocation.item_account_stride);
        let mut item = 0_u32;
        while item < invocation.repeated_item_count {
            let index = usize::try_from(item).map_err(|_| TradingSbfError::Content)?;
            let start = invocation
                .item_account_start
                .checked_add(index.checked_mul(stride).ok_or(TradingSbfError::Content)?)
                .ok_or(TradingSbfError::Content)?;
            let end = start.checked_add(count).ok_or(TradingSbfError::Content)?;
            accounts.extend_window(
                output,
                start,
                end.checked_sub(start).ok_or(TradingSbfError::Content)?,
            )?;
            item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
    } else if invocation.item_account_count != 0 || invocation.repeated_item_count != 0 {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn route_authority(
    request: &[u8],
    kind: RouteKindV3,
) -> Result<(CallerAuthoritySeedsV1, ReceiptKindV3), ProgramError> {
    let packet_digest = hash(request).to_bytes();
    if request.get(..8) == Some(PROTOCOL_POSITION_REQUEST_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let position =
            ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(position.release_set).map_err(|_| TradingSbfError::Content)?,
            position.market,
            ExecutionRoleV1::Trading,
            position.position_owner,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        let receipt = match position.action {
            ProtocolPositionActionV2::Admit => ReceiptKindV3::Admit,
            ProtocolPositionActionV2::Close => ReceiptKindV3::Close,
        };
        Ok((seeds, receipt))
    } else if request.get(..8) == Some(AFFINE_BATCH_PLAN_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::AffineOnce {
            return Err(TradingSbfError::Content.into());
        }
        let plan = AffineBatchPlanV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(plan.release_set()).map_err(|_| TradingSbfError::Content)?,
            plan.market(),
            ExecutionRoleV1::Trading,
            plan.request_id(),
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::Affine))
    } else if request.get(..8) == Some(SIGNED_DELTA_PLAN_MAGIC_V3.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let plan = SignedDeltaPlanV3::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(plan.release_set()).map_err(|_| TradingSbfError::Content)?,
            plan.market(),
            ExecutionRoleV1::Trading,
            plan.request_id(),
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::SignedDelta))
    } else if request.get(..8) == Some(SPARSE_NATIVE_TRANSFER_MAGIC_V1.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request =
            SparseNativeTransferV1::decode(request).map_err(|_| TradingSbfError::Content)?;
        let input = request.input();
        if input.caller_role != dclutch_claims_svm::CallerRole::Trading {
            return Err(TradingSbfError::Content.into());
        }
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(input.release_set).map_err(|_| TradingSbfError::Content)?,
            input.market,
            ExecutionRoleV1::Trading,
            input.request_id,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::SparseNativeTransfer))
    } else if request.get(..8) == Some(SERIES_CLAIMS_FOUNDING_TRANSPORT_MAGIC_V1.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let transport = SeriesClaimsFoundingTransportV1::decode(request)
            .map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(transport.release_set()).map_err(|_| TradingSbfError::Content)?,
            transport.market(),
            ExecutionRoleV1::Trading,
            transport.permit(),
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::SeriesFounding))
    } else if request.get(..8) == Some(CLAIMS_FOUNDING_REQUEST_MAGIC_V5.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request =
            ClaimsFoundingRequestV5::decode(request).map_err(|_| TradingSbfError::Content)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(request.release_set()).map_err(|_| TradingSbfError::Content)?,
            request.market(),
            ExecutionRoleV1::Trading,
            request.founding_intent_digest(),
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::Founding))
    } else if request.get(..8) == Some(REPRESENTATION_REQUEST_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request =
            RepresentationRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let header = request.header();
        if header.caller_role != CallerRoleV2::Trading {
            return Err(TradingSbfError::Content.into());
        }
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(header.release_set).map_err(|_| TradingSbfError::Content)?,
            header.market,
            ExecutionRoleV1::Trading,
            header.parent_context,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::RationalRepresentation))
    } else if request.get(..8) == Some(LIFECYCLE_REQUEST_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request = LifecycleRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let header = request.header();
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(header.release_set).map_err(|_| TradingSbfError::Content)?,
            header.market,
            ExecutionRoleV1::Trading,
            header.parent_context,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::RationalLifecycle))
    } else if request.get(..8) == Some(FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request =
            FractionalExposureRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
        let receipt = match request.action() {
            FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap => {
                ReceiptKindV3::FractionalAtomic
            }
            FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn => {
                ReceiptKindV3::FractionalTerminalAtomic
            }
            _ => return Err(TradingSbfError::Content.into()),
        };
        let input = request.input();
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(input.release_set).map_err(|_| TradingSbfError::Content)?,
            input.market,
            ExecutionRoleV1::Trading,
            input.terms,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, receipt))
    } else if request.get(..8) == Some(FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3.as_slice()) {
        if kind != RouteKindV3::Once {
            return Err(TradingSbfError::Content.into());
        }
        let request =
            FractionalRetirementRequestV3::decode(request).map_err(|_| TradingSbfError::Content)?;
        let input = request.input();
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(input.release_set).map_err(|_| TradingSbfError::Content)?,
            input.market,
            ExecutionRoleV1::Trading,
            input.terms,
            packet_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok((seeds, ReceiptKindV3::FractionalRetirementCoordinate))
    } else if request.get(..8) == Some(FRACTIONAL_CLAIM_CHECK_COMPACT_MAGIC_V1.as_slice()) {
        fractional_compaction_authority(request, kind, packet_digest)
    } else {
        Err(TradingSbfError::Content.into())
    }
}

/// The compaction arm, kept out of `route_authority`'s own frame.
///
/// `#[inline(never)]` is load-bearing here, and it was added after measuring
/// rather than on principle. A fractional compaction request is 744 bytes and
/// decodes into a struct embedding the whole `TerminalSettlementRequestV3`, so
/// inlining this arm put that struct on the frame every OTHER Claims route also
/// pays for: `route_authority` measured **3,072 bytes before this arm and 3,712
/// after**, dropping its spare from 1,024 to 384 in a single edit, on a link
/// whose deepest function already sits at 4,032 of 4,096.
///
/// Split out, `route_authority` returns to its old frame and the decode pays for
/// its own. Same discipline as `claim_check_compaction_v1::authenticate_compaction`
/// and `hot_v3::decode_claims_composition_boxed_v3`; worth restating because the
/// cost is invisible without `tools/sbf-frame-sizes.py` -- the plain build
/// reports zero diagnostics at 3,712 exactly as it does at 3,072.
#[inline(never)]
fn fractional_compaction_authority(
    request: &[u8],
    kind: RouteKindV3,
    packet_digest: [u8; 32],
) -> Result<(CallerAuthoritySeedsV1, ReceiptKindV3), ProgramError> {
    if kind != RouteKindV3::Once {
        return Err(TradingSbfError::Content.into());
    }
    let request = FractionalCompactToClaimCheckRequestV1::decode(request)
        .map_err(|_| TradingSbfError::Content)?;
    let input = request.input();
    // The fourth seed is `terms`, as it is for both the other Fractional
    // families -- and here it is the terms the compaction request carries
    // rather than a settlement field, because the terminal header has no terms
    // and the denominator's author is what this route is bound to.
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(input.release_set).map_err(|_| TradingSbfError::Content)?,
        input.market,
        ExecutionRoleV1::Trading,
        request.coordinates().terms,
        packet_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    Ok((seeds, ReceiptKindV3::FractionalClaimCheckCompaction))
}

fn verify_route_receipt(
    kind: ReceiptKindV3,
    request: &[u8],
    receipt: &[u8],
    claims_program: [u8; 32],
    trading_program: [u8; 32],
    expected_post_resources: PostResourceEvidenceV3,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request_digest = hash(request).to_bytes();
    match kind {
        ReceiptKindV3::Admit => verify_admit_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            trading_program,
        ),
        ReceiptKindV3::Affine => {
            verify_affine_receipt(request, receipt, request_digest, claims_program)
        }
        ReceiptKindV3::SignedDelta => verify_signed_delta_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            match expected_post_resources {
                PostResourceEvidenceV3::Single(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::SparseNativeTransfer => verify_sparse_native_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            match expected_post_resources {
                PostResourceEvidenceV3::Single(value) => Some(value),
                _ => None,
            },
            expected_post_resources == PostResourceEvidenceV3::DeferredDirect,
        ),
        ReceiptKindV3::Founding => verify_founding_receipt(
            request,
            receipt,
            request_digest,
            claims_program,
            trading_program,
            match expected_post_resources {
                PostResourceEvidenceV3::Founding(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::SeriesFounding => verify_series_founding_receipt(
            request,
            receipt,
            claims_program,
            trading_program,
            match expected_post_resources {
                PostResourceEvidenceV3::Founding(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::RationalLifecycle => {
            verify_rational_lifecycle_receipt(request, receipt, request_digest)
        }
        ReceiptKindV3::RationalRepresentation => {
            verify_rational_representation_receipt(request, receipt, request_digest, claims_program)
        }
        ReceiptKindV3::FractionalAtomic => verify_fractional_atomic_receipt(
            request,
            receipt,
            request_digest,
            match expected_post_resources {
                PostResourceEvidenceV3::FractionalRoot(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::FractionalTerminalAtomic => verify_fractional_terminal_atomic_receipt(
            request,
            receipt,
            request_digest,
            match expected_post_resources {
                PostResourceEvidenceV3::FractionalRoot(value) => Some(value),
                _ => None,
            },
        ),
        ReceiptKindV3::FractionalRetirementCoordinate => {
            verify_fractional_retirement_coordinate_receipt(
                request,
                receipt,
                request_digest,
                match expected_post_resources {
                    PostResourceEvidenceV3::FractionalRoot(value) => Some(value),
                    _ => None,
                },
            )
        }
        // THE REFUSAL IS GONE, AND IT WENT WHERE IT SAID IT WOULD: "until the
        // Claims route and its receipt land together." They did, in one lane.
        // Every field this verifies is a value the route holds at the moment it
        // returns -- not a field set to what a route was guessed to produce,
        // which is the reason three lanes correctly declined to write this arm
        // while there was no route to bind it to.
        ReceiptKindV3::FractionalClaimCheckCompaction => {
            verify_fractional_claim_check_compaction_receipt(
                request,
                receipt,
                request_digest,
                match expected_post_resources {
                    PostResourceEvidenceV3::FractionalRoot(value) => Some(value),
                    _ => None,
                },
            )
        }
        ReceiptKindV3::Close => {
            verify_close_receipt(request, receipt, request_digest, claims_program)
        }
    }
}

/// Verify one fractional compaction receipt against the request that caused it.
///
/// The last of the four Fractional verifiers, and the only one whose kind used
/// to refuse outright. It is a sibling of the three above in every structural
/// respect -- exact width, decode the request, decode the receipt, bind the
/// receipt to the request digest, then compare the root against the account the
/// parent authenticated in the frame it built.
///
/// The root comparison is what the `PostResourceEvidenceV3::FractionalRoot`
/// arm was written for three lanes early, and its comment predicted this
/// moment: an evidence value inherited from a wildcard would have made this
/// check compare against `None` while still looking verified. It compares
/// against the frame declaration's own coordinate instead.
///
/// **`#[inline(never)]`, for the frame and not for tidiness.** This decodes the
/// same 744-byte wire that moved `route_authority` by 640 bytes when FRACCHECK-3
/// measured it, on a link whose deepest function sits 64 bytes from the 4,096
/// bound. Its three neighbours carry the attribute for the same reason.
#[inline(never)]
fn verify_fractional_claim_check_compaction_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    expected_root: Option<[u8; 32]>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    if receipt.len() != FRACTIONAL_CLAIM_CHECK_COMPACT_RECEIPT_BYTES_V1 {
        return Err(TradingSbfError::Transition.into());
    }
    let request = FractionalCompactToClaimCheckRequestV1::decode(request)
        .map_err(|_| TradingSbfError::Content)?;
    let receipt = FractionalClaimCheckCompactionReceiptV1::decode(receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    receipt
        .verify_for(request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    if expected_root != Some(receipt.root()) {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::FractionalClaimCheckCompaction(
        receipt,
    ))
}

#[inline(never)]
fn verify_fractional_retirement_coordinate_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    expected_root: Option<[u8; 32]>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    if receipt.len() != FRACTIONAL_RETIREMENT_COORDINATE_RECEIPT_BYTES_V3 {
        return Err(TradingSbfError::Transition.into());
    }
    let request =
        FractionalRetirementRequestV3::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = FractionalRetirementCoordinateReceiptV3::decode(receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    receipt
        .verify_for(request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    if expected_root != Some(receipt.root()) {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::FractionalRetirementCoordinate(
        receipt,
    ))
}

#[inline(never)]
fn verify_fractional_atomic_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    expected_root: Option<[u8; 32]>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    if receipt.len() != FRACTIONAL_ATOMIC_RECEIPT_BYTES_V3 {
        return Err(TradingSbfError::Transition.into());
    }
    let request =
        FractionalExposureRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt =
        FractionalAtomicReceiptV3::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .verify_for(request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    if expected_root != Some(receipt.root()) {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::FractionalAtomic(receipt))
}

#[inline(never)]
fn verify_fractional_terminal_atomic_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    expected_root: Option<[u8; 32]>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    if receipt.len() != FRACTIONAL_TERMINAL_ATOMIC_RECEIPT_BYTES_V3 {
        return Err(TradingSbfError::Transition.into());
    }
    let request =
        FractionalExposureRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = FractionalTerminalAtomicReceiptV3::decode(receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    receipt
        .verify_for(request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    if expected_root != Some(receipt.root()) {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::FractionalTerminalAtomic(receipt))
}

#[derive(Clone, Copy)]
struct FractionalRootSignerV3 {
    seeds: CapabilityRootSeedsV1,
    bump: u8,
}

/// What one Fractional kind expects of the capability root in its own frame.
///
/// The four kinds agree about every root fact but one, so the facts they share
/// are checked once below and only the differences are carried here. The
/// difference that matters is [`Self::writable`]: it is stated by the arm that
/// decodes the request, so a kind cannot end up with one arm's revision and
/// another arm's privileges.
#[derive(Clone, Copy)]
struct FractionalRootExpectationV3 {
    release_set: [u8; 32],
    market: [u8; 32],
    terms: [u8; 32],
    revision: u64,
    index: usize,
    /// Whether the frame declares the root writable, and the frame decides.
    ///
    /// The three exposure and retirement kinds require a **writable** root
    /// because their effect program writes the root's revision commit-last: a
    /// read-only root there is a route that cannot finish its own job. A
    /// fractional compaction revises nothing about the root -- it spends the
    /// root's signature on one `SetAuthority` and leaves the record untouched --
    /// so its frame declares `(signer, not writable)` and this arm requires
    /// exactly that. Design §17.8 ruling 1; witness w3 pins the inversion so it
    /// cannot be quietly "fixed" back toward its three neighbours.
    writable: bool,
}

/// Root coordinates of a fractional compaction, decoded off its own frame.
///
/// **`#[inline(never)]` is load-bearing, not tidiness.** The compaction request
/// is 744 bytes and decodes into a struct embedding the whole
/// `TerminalSettlementRequestV3`. Inlined here it would join
/// [`fractional_root_signer`]'s frame, which every Fractional route in this link
/// pays for, on a link whose deepest function sits 64 bytes from the 4,096
/// bound -- and `cargo build-sbf` reports a frame under the bound exactly as it
/// reports one far below it, so the growth would be invisible to the gate CI
/// runs. FRACCHECK-3 watched this happen prospectively on `route_authority`
/// (3,072 -> 3,712 for the same 744 bytes); measure with
/// `tools/sbf-frame-sizes.py`, never trust a zero-diagnostic build.
#[inline(never)]
fn fractional_compaction_root_expectation(
    request: &[u8],
) -> Result<FractionalRootExpectationV3, ProgramError> {
    let request = FractionalCompactToClaimCheckRequestV1::decode(request)
        .map_err(|_| TradingSbfError::Content)?;
    let input = request.input();
    let coordinates = request.coordinates();
    Ok(FractionalRootExpectationV3 {
        release_set: input.release_set,
        market: input.market,
        // The Fractional exposure terms, off the coordinates this wire adds --
        // NOT anything in the terminal header, which carries an exposure id and
        // digest but no terms. This is the same account the exposure arms pin,
        // and it is the denominator's sole author.
        terms: coordinates.terms,
        // The field's only purpose, and the reason it is on the wire at all.
        revision: coordinates.expected_root_revision,
        // The frame declaration is the one author of where the root sits; this
        // asks it rather than writing a second constant down.
        index: FractionalCompactionRoleV1::FractionalCapabilityRoot
            .index()
            .ok_or(TradingSbfError::Content)?,
        writable: false,
    })
}

fn fractional_root_signer(
    program_id: &Pubkey,
    kind: ReceiptKindV3,
    request: &[u8],
    accounts: &[AccountInfo<'_>],
    metas: &mut [solana_program::instruction::AccountMeta],
) -> Result<Option<FractionalRootSignerV3>, ProgramError> {
    if !matches!(
        kind,
        ReceiptKindV3::FractionalAtomic
            | ReceiptKindV3::FractionalTerminalAtomic
            | ReceiptKindV3::FractionalRetirementCoordinate
            | ReceiptKindV3::FractionalClaimCheckCompaction
    ) {
        return Ok(None);
    }
    let expectation = match kind {
        ReceiptKindV3::FractionalAtomic | ReceiptKindV3::FractionalTerminalAtomic => {
            let request = FractionalExposureRequestV2::decode(request)
                .map_err(|_| TradingSbfError::Content)?;
            let input = request.input();
            FractionalRootExpectationV3 {
                release_set: input.release_set,
                market: input.market,
                terms: input.terms,
                revision: input.expected_revision,
                index: if kind == ReceiptKindV3::FractionalAtomic {
                    FRACTIONAL_ATOMIC_ROOT_V3
                } else {
                    FRACTIONAL_TERMINAL_ROOT_V3
                },
                writable: true,
            }
        }
        ReceiptKindV3::FractionalRetirementCoordinate => {
            let request = FractionalRetirementRequestV3::decode(request)
                .map_err(|_| TradingSbfError::Content)?;
            let input = request.input();
            FractionalRootExpectationV3 {
                release_set: input.release_set,
                market: input.market,
                terms: input.terms,
                revision: input.expected_revision,
                index: FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3,
                writable: true,
            }
        }
        // Design §17.8 ruling 1. The root's signature is load-bearing on this
        // route exactly once, and its once is the hand-off: `SetAuthority`
        // (`PermissionedBurn`), root -> escrow, which Token-2022 refuses without
        // the CURRENT authority's signature and which nothing but a Trading
        // `invoke_signed` can produce -- and nothing can produce after the
        // market retires. That is why §17.4 made this route Trading-composed at
        // all. After the hand-off the root signs nothing ever again; redemption
        // is holder plus escrow. The decode is split out because of its frame,
        // not its length; see the helper.
        ReceiptKindV3::FractionalClaimCheckCompaction => {
            fractional_compaction_root_expectation(request)?
        }
        _ => return Err(TradingSbfError::Content.into()),
    };
    let FractionalRootExpectationV3 {
        release_set,
        market,
        terms,
        revision: expected_revision,
        index: root_index,
        writable: root_writable,
    } = expectation;
    let root_account = accounts.get(root_index).ok_or(TradingSbfError::Content)?;
    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let composite =
        decode_fractional_capability_root_v4(&root_data).ok_or(TradingSbfError::Content)?;
    let header = composite.header();
    let state = composite.state();
    let seeds = header.seeds();
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    let selected_config = header.selection().config().to_bytes();
    // THE TWO ARMS ARE NOT THE SAME KIND OF CHECK, and reading them as one has
    // now cost two lanes a hostile hunt. Written down so it costs a third
    // nothing.
    //
    // V1 binds the CALLER. `terms` is an ordinary wire field, and a historical
    // root carries its own terms digest, so `historical_terms == terms` is a
    // real refusal a caller can provoke.
    //
    // V2 CANNOT BE PROVOKED BY THE CALLER, and that is a weaker and more useful
    // statement than "cannot fail" -- which is what I first wrote here and had
    // to withdraw. No REQUEST field reaches either side. Both descend from one
    // value in one instruction: `outer.rs:2233` seeds
    // `ACTIVATION_CONFIG_IDENTITY_V2` from `request.selection().config()`, the
    // Fractional family projects THAT register into the root tail at the
    // selection-config offset (`FRACTIONAL_ACTIVATION_TAIL_FIELDS_V1`, a
    // `SeamIdentity`), and the header this reads persists the same
    // `request.selection().config()`. The `find_program_address` re-derivation
    // above -- from this header's OWN seeds, under this program's ownership --
    // makes that exhaustive over accounts: one reaching this line was created by
    // that seam or was refused three predicates ago.
    //
    // BUT IT IS NOT UNSATISFIABLE, and the difference is the whole reason the
    // conjunct earns its place. The register-to-offset binding is declared by
    // the family's own published effect program, and ON CHAIN nothing checks it:
    // `require_activation_local_effects` (`outer.rs:2087`) restricts effect KINDS
    // and never asks which register feeds which tail offset. The only thing that
    // checks it is `validate_fractional_activation_bundle_v1`
    // (`fractional-claim-operator/src/selected_release_v4.rs:517`), which is
    // operator-side and never runs on chain. So a descriptor whose effect wrote a
    // constant, or a different register, at the selection-config offset would
    // produce a root that decodes cleanly as V2 with these two divergent -- and
    // the seam would create it.
    //
    // This is therefore a live guard against a malformed or hostile DESCRIPTOR,
    // not a tautology and not the `ff8ca269` shape I first took it for. Deleting
    // it would be a weakening. It stays.
    //
    // WHAT IT LOOKS LIKE IT LOST, AND WHY THAT IS NOT A HOLE. V2 does not pin
    // the caller's `terms` -- on this arm `terms` is decoded and never read.
    // That is deliberate and the contract says so: the V2 root stores no terms
    // digest because "storing their digest in the root would recreate the
    // manifest fixed point V2 removes", and the terms "are joined against
    // `selection_config` AT EXECUTION"
    // (`dclutch-fractional-claim-contract/src/root.rs:138-141`). Execution is
    // Claims, and Claims does it on all four kinds that reach here, deriving the
    // config from the AUTHENTICATED terms record rather than from a digest:
    // `fractional_atomic_v3.rs:253` and `:736`, `fractional_retirement_v3.rs:614`,
    // `fractional_claim_check_v1.rs:1268` (the last added by `bf362312`, after a
    // permissionless crank was shown to be able to choose its own denominator).
    //
    // So this seam MUST NOT "restore" the binding. It holds a 32-byte digest and
    // no terms account -- `fractional_selection_config_from_terms_v1` needs the
    // decoded record -- so it could only re-check the fixed point V2 deleted, at
    // the cost of two frame accounts the Claims side explicitly declined
    // (`fractional_atomic_v3.rs:150-154`).
    //
    // The conjunct stays. It is not caller-facing, so a hostile test that feeds
    // this route bad REQUEST bytes will never provoke it and should not try; the
    // test that would is one that activates a root under a descriptor whose
    // effect program mis-declares the selection-config offset, and none exists.
    let state_binding_matches = match (state.terms_v1(), state.selection_config_v2()) {
        (Some(historical_terms), None) => historical_terms == terms && selected_config == terms,
        (None, Some(current_config)) => current_config == selected_config,
        _ => false,
    };
    if root_account.key != &expected
        || root_account.owner != program_id
        || root_account.is_signer
        // The ONE predicate that differs by kind, and it is an equality against
        // what the kind's own frame declares rather than a `!` every kind must
        // share. A compaction whose root arrived writable is refused here, on
        // the same line that refuses an exposure whose root arrived read-only.
        || root_account.is_writable != root_writable
        || root_account.executable
        || header.release_set().to_bytes() != release_set
        || header.market() != market
        || !state_binding_matches
        || state.bump() != bump
        || state.market() != market
        || state.revision() != expected_revision
    {
        return Err(TradingSbfError::Content.into());
    }
    let meta = metas.get_mut(root_index).ok_or(TradingSbfError::Content)?;
    // And the meta the child CPI will carry has to agree with the account the
    // parent authenticated, writability included -- otherwise a compaction
    // could authenticate a read-only root and then hand the child a writable
    // one, which is the whole point of checking the meta separately at all.
    if meta.pubkey != expected || meta.is_writable != root_writable {
        return Err(TradingSbfError::Content.into());
    }
    meta.is_signer = true;
    Ok(Some(FractionalRootSignerV3 { seeds, bump }))
}

#[inline(never)]
fn verify_rational_representation_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    if receipt.len() != REPRESENTATION_RECEIPT_BYTES_V2 {
        return Err(TradingSbfError::Transition.into());
    }
    let request = RepresentationRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = Box::new(
        RepresentationReceiptV2::decode(receipt).map_err(|_| TradingSbfError::Transition)?,
    );
    receipt
        .verify_for(request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    if receipt.representation_program() != claims_program
        || receipt.claims_program() != claims_program
        || receipt.token_program() != request.header().token_program
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::RationalRepresentation(receipt))
}

#[inline(never)]
fn verify_rational_lifecycle_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request = LifecycleRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = verify_rational_lifecycle_hot_receipt_v3(request, request_digest, receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(ClaimsRouteReceiptV3::RationalLifecycle(receipt))
}

#[inline(never)]
fn verify_founding_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    trading_program: [u8; 32],
    expected: Option<FoundingPostResourceDigestsV5>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request = ClaimsFoundingRequestV5::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = Box::new(
        ClaimsFoundingReceiptV5::decode(receipt).map_err(|_| TradingSbfError::Transition)?,
    );
    receipt
        .verify_for(&request, request_digest)
        .map_err(|_| TradingSbfError::Transition)?;
    let expected = expected.ok_or(TradingSbfError::Transition)?;
    if request.claims_program() != claims_program
        || request.trading_program() != trading_program
        || receipt.aggregate_digest() != expected.aggregate
        || receipt.position_digest() != expected.position
        || receipt.admission_digest() != expected.admission
        || receipt.post_resource_digest() != expected.combined
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::Founding(receipt))
}

#[inline(never)]
fn verify_series_founding_receipt(
    request: &[u8],
    receipt: &[u8],
    claims_program: [u8; 32],
    trading_program: [u8; 32],
    expected: Option<FoundingPostResourceDigestsV5>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let permit = decode_series_founding_transport_permit(request)?;
    let receipt = decode_series_founding_receipt_boxed(receipt)?;
    verify_series_founding_receipt_self_hash(&receipt)?;
    verify_series_founding_transport_normalization(request, permit, receipt.request_ref())?;
    let expected = expected.ok_or(TradingSbfError::Transition)?;
    if receipt.request_ref().claims_program() != claims_program
        || receipt.request_ref().trading_program() != trading_program
        || receipt.aggregate_digest() != expected.aggregate
        || receipt.position_digest() != expected.position
        || receipt.admission_digest() != expected.admission
        || receipt.post_resource_digest() != expected.combined
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::Founding(receipt))
}

/// Decode the wide transient transport in its own bounded SBF frame and return
/// only the compact authenticated coordinate needed by later phases.
#[inline(never)]
fn decode_series_founding_transport_permit(request: &[u8]) -> Result<[u8; 32], ProgramError> {
    let transport =
        SeriesClaimsFoundingTransportV1::decode(request).map_err(|_| TradingSbfError::Content)?;
    Ok(transport.permit())
}

/// Decode the wide canonical receipt directly into its heap-owned immutable
/// result so the caller never carries its fixed-layout body on the SBF stack.
#[inline(never)]
fn decode_series_founding_receipt_boxed(
    receipt: &[u8],
) -> Result<Box<ClaimsFoundingReceiptV5>, ProgramError> {
    Ok(Box::new(
        ClaimsFoundingReceiptV5::decode(receipt).map_err(|_| TradingSbfError::Transition)?,
    ))
}

/// Verify the embedded canonical request's exact byte digest in a phase where
/// its one 832-byte encoding is the only wide local value.
#[inline(never)]
fn verify_series_founding_receipt_self_hash(
    receipt: &ClaimsFoundingReceiptV5,
) -> Result<(), ProgramError> {
    let canonical = receipt.request_ref();
    let canonical_bytes = canonical.to_bytes();
    receipt
        .verify_for(canonical, hash(&canonical_bytes).to_bytes())
        .map_err(|_| TradingSbfError::Transition.into())
}

/// Recreate the root-independent transport and compare every byte in a phase
/// disjoint from receipt decode and canonical request hashing.
#[inline(never)]
fn verify_series_founding_transport_normalization(
    request: &[u8],
    permit: [u8; 32],
    canonical: &ClaimsFoundingRequestV5,
) -> Result<(), ProgramError> {
    let normalized = SeriesClaimsFoundingTransportV1::from_canonical_v5(permit, *canonical)
        .map_err(|_| TradingSbfError::Transition)?;
    if normalized.to_bytes().as_slice() != request {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

#[inline(never)]
fn verify_admit_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    trading_program: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request =
        ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = ProtocolPositionAdmissionV2::decode_receipt(receipt)
        .map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_request(request, request_digest, claims_program, trading_program)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(ClaimsRouteReceiptV3::Admit(receipt))
}

#[inline(never)]
fn verify_affine_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let plan = AffineBatchPlanV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = AffineBatchReceiptV2::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_plan(plan)
        .map_err(|_| TradingSbfError::Transition)?;
    if receipt.packet_digest() != request_digest || receipt.claims_program() != claims_program {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::Affine(receipt))
}

#[inline(never)]
fn verify_signed_delta_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    expected_post_resource_digest: Option<[u8; 32]>,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let plan = SignedDeltaPlanV3::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt = SignedDeltaReceiptV3::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_plan(plan)
        .map_err(|_| TradingSbfError::Transition)?;
    let (positions, aggregates, deltas) = plan.table_bytes();
    let table_digest = hashv(&[
        b"dclutch/claims/signed-delta-table/v3",
        positions,
        aggregates,
        deltas,
    ])
    .to_bytes();
    if receipt.packet_digest() != request_digest
        || receipt.table_digest() != table_digest
        || receipt.claims_program() != claims_program
        || Some(receipt.post_resource_digest()) != expected_post_resource_digest
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::SignedDelta(receipt))
}

#[inline(never)]
fn verify_sparse_native_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    expected_post_resource_digest: Option<[u8; 32]>,
    deferred_to_direct: bool,
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request = SparseNativeTransferV1::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt =
        SparseNativeTransferReceiptV1::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_request(request)
        .map_err(|_| TradingSbfError::Transition)?;
    if receipt.packet_digest() != request_digest
        || receipt.claims_program() != claims_program
        || (!deferred_to_direct
            && Some(receipt.post_resource_digest()) != expected_post_resource_digest)
        || (deferred_to_direct && expected_post_resource_digest.is_some())
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(ClaimsRouteReceiptV3::SparseNativeTransfer(receipt))
}

#[inline(never)]
fn verify_close_receipt(
    request: &[u8],
    receipt: &[u8],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
) -> Result<ClaimsRouteReceiptV3, ProgramError> {
    let request =
        ProtocolPositionRequestV2::decode(request).map_err(|_| TradingSbfError::Content)?;
    let receipt =
        ProtocolPositionCloseReceiptV2::decode(receipt).map_err(|_| TradingSbfError::Transition)?;
    receipt
        .validate_request(request, request_digest, claims_program)
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(ClaimsRouteReceiptV3::Close(receipt))
}

pub(crate) fn signed_delta_post_resource_digest(
    child_accounts: &[AccountInfo<'_>],
    position_count: u32,
) -> Result<[u8; 32], ProgramError> {
    let positions = usize::try_from(position_count).map_err(|_| TradingSbfError::Content)?;
    let expected = 21_usize
        .checked_add(positions)
        .ok_or(TradingSbfError::Content)?;
    if child_accounts.len() != expected {
        return Err(TradingSbfError::Content.into());
    }
    // The preimage is HASHED, never kept, so it is never CONCATENATED either.
    // This used to grow one `Vec<u8>` from empty through `extend_from_slice`,
    // which on the SBF bump allocator is the whole doubling ladder plus a live
    // copy of every account body it walked: 985 bytes charged for the rest of
    // the instruction, measured inside the child walk where the heap is
    // scarcest. `hashv` takes the parts, so what is carried is the parts --
    // one borrow guard and one fat pointer per account, reserved exactly.
    let end = 20_usize
        .checked_add(positions)
        .ok_or(TradingSbfError::Content)?;
    let bodies = child_accounts
        .get(20..end)
        .ok_or(TradingSbfError::Content)?;
    let mut guards = Vec::new();
    guards
        .try_reserve_exact(
            bodies
                .len()
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?,
        )
        .map_err(|_| TradingSbfError::Content)?;
    guards.push(
        child_accounts
            .get(1)
            .ok_or(TradingSbfError::Content)?
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?,
    );
    for account in bodies {
        guards.push(
            account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Transition)?,
        );
    }
    let mut parts: Vec<&[u8]> = Vec::new();
    parts
        .try_reserve_exact(
            guards
                .len()
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?,
        )
        .map_err(|_| TradingSbfError::Content)?;
    parts.push(b"dclutch/claims/signed-delta-post-resources/v3");
    for guard in &guards {
        parts.push(guard);
    }
    Ok(hashv(&parts).to_bytes())
}

fn sparse_native_post_resource_digest(
    child_accounts: &[AccountInfo<'_>],
) -> Result<[u8; 32], ProgramError> {
    if child_accounts.len() != 23 {
        return Err(TradingSbfError::Content.into());
    }
    // Three accounts at fixed coordinates and a domain: the parts are known at
    // compile time, so this hashes them where they lie and allocates nothing at
    // all. It used to concatenate all three bodies into a `Vec<u8>` grown from
    // empty -- the doubling ladder plus a live copy, on an allocator that never
    // gives either back. Same fact, same bytes, same order.
    let market = child_accounts
        .get(1)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let position = child_accounts
        .get(20)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let admission = child_accounts
        .get(21)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let empty = &[][..];
    Ok(sparse_native_transfer_poststate_digest_v1(
        SparseNativeTransferPoststateSlicesV1 {
            market: [&market, empty, empty, empty, empty],
            source: [&position, empty, empty, empty, empty],
            destination: [&admission, empty, empty, empty, empty],
        },
    ))
}

fn founding_post_resource_digests(
    child_accounts: &[AccountInfo<'_>],
) -> Result<FoundingPostResourceDigestsV5, ProgramError> {
    // The exact FoundingV5 frame is 32 accounts; the CPI program account is
    // appended once by this adapter for invoke_signed.
    if child_accounts.len() != 33 {
        return Err(TradingSbfError::Content.into());
    }
    let aggregate_data = child_accounts
        .get(2)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let position_data = child_accounts
        .get(3)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let admission_data = child_accounts
        .get(4)
        .ok_or(TradingSbfError::Content)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(FoundingPostResourceDigestsV5 {
        aggregate: hash(&aggregate_data).to_bytes(),
        position: hash(&position_data).to_bytes(),
        admission: hash(&admission_data).to_bytes(),
        combined: hashv(&[
            CLAIMS_FOUNDING_POST_RESOURCE_DIGEST_DOMAIN_V5,
            &aggregate_data,
            &position_data,
            &admission_data,
        ])
        .to_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use dclutch_claims_svm::{
        CallerRole,
        affine_batch_v2::{
            AffineBatchPlanInputV2, AffineBatchPositionV2, AffineBatchRowInputV2, AffineBatchRowV2,
            DeltaDirectionV2, SignedMagnitudeV2, plan_bytes,
        },
        founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5},
        protocol_position_v2::{
            ProtocolPositionAdmissionEvidenceV2, ProtocolPositionCloseEvidenceV2,
            ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        },
        signed_delta_v3::{
            DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
            SignedDeltaPositionV3, SignedDeltaV3, plan_bytes as signed_plan_bytes,
        },
        sparse_native_transfer_v1::{
            SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1, SparseNativeTransferInputV1,
            SparseNativeTransferV1,
        },
    };
    use dclutch_rational_representation_v2_contract::{
        ABSENT_REVISION, ASSET_BYTES_V3, AssetV2, RepresentationActionV2,
        RepresentationRequestHeaderV2,
    };
    use dclutch_rational_representation_v2_request_contract::generated as representation_wire;
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn account_info(signer: bool, writable: bool) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(Vec::<u8>::new().into_boxed_slice());
        AccountInfo::new(key, signer, writable, lamports, data, owner, false)
    }

    fn fractional_root_info(
        request: FractionalExposureRequestV2,
        trading: Pubkey,
    ) -> (AccountInfo<'static>, u8) {
        let input = request.input();
        // The exposure and retirement frames declare a WRITABLE root, because
        // their effect program commits the revision last.
        fractional_root_info_at(
            trading,
            input.release_set,
            input.market,
            input.terms,
            input.expected_revision,
            true,
        )
    }

    /// One capability root, built at exactly the facts a caller states.
    ///
    /// Split out of `fractional_root_info` so the compaction witnesses can name
    /// a revision and a writability of their own without a second copy of the
    /// root's byte layout -- the layout has one author here, and the two callers
    /// differ only in what they ask of it.
    fn fractional_root_info_at(
        trading: Pubkey,
        release_set: [u8; 32],
        market: [u8; 32],
        terms: [u8; 32],
        revision: u64,
        writable: bool,
    ) -> (AccountInfo<'static>, u8) {
        use dclutch_capability_program_contract::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
        use dclutch_fractional_claim_contract::{
            FRACTIONAL_CAPABILITY_ROOT_BYTES_V4, FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4,
            FractionalRootInputV1, FractionalRootV1,
        };
        use dclutch_release_set_contract::CapabilityExecutionSelectionV1;

        let selection = CapabilityExecutionSelectionV1::new(
            0,
            ContentId::new(id(51)).expect("manifest"),
            ContentId::new(id(52)).expect("kind"),
            ContentId::new(id(53)).expect("capability release"),
            ContentId::new(terms).expect("terms config"),
        )
        .expect("selection");
        let header = CapabilityRootHeaderV1::new(
            ContentId::new(release_set).expect("release set"),
            market,
            1,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("header");
        let (root_key, bump) = Pubkey::find_program_address(&header.seeds().as_slices(), &trading);
        let state = FractionalRootV1::new(FractionalRootInputV1 {
            bump,
            terms,
            market,
            rent_beneficiary: id(50),
            revision,
            historical_rent_principal: 1,
        })
        .expect("root state");
        let mut bytes = alloc::vec![0_u8; FRACTIONAL_CAPABILITY_ROOT_BYTES_V4];
        bytes[..FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4].copy_from_slice(&header.to_bytes());
        bytes[FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4..].copy_from_slice(&state.to_bytes());
        let root_key = Box::leak(Box::new(root_key));
        let root_owner = Box::leak(Box::new(trading));
        let root_lamports = Box::leak(Box::new(1_u64));
        let root_data = Box::leak(bytes.into_boxed_slice());
        (
            AccountInfo::new(
                root_key,
                false,
                writable,
                root_lamports,
                root_data,
                root_owner,
                false,
            ),
            bump,
        )
    }

    /// The privilege rule now lives once, in the walk's shared buffer set, and
    /// this is still the frame that states it: coordinate 0 is the
    /// release-pinned caller authority and signs; an ordinary coordinate does
    /// not; a coordinate the frame already declares a signer keeps it.
    #[test]
    fn authority_signing_is_added_and_existing_actor_signer_is_preserved() {
        let mut buffers = ChildInvocationBuffersV3::new();
        buffers.accounts.push(account_info(false, false));
        buffers.accounts.push(account_info(false, true));
        buffers.accounts.push(account_info(false, true));
        buffers.accounts.push(account_info(true, false));
        buffers.fill_metas().expect("metas");

        let signer = |index: usize| buffers.metas.get(index).expect("meta").is_signer;
        assert!(signer(0));
        assert!(!signer(1));
        assert!(!signer(2));
        assert!(signer(3));
        let writable = |index: usize| buffers.metas.get(index).expect("meta").is_writable;
        assert!(!writable(0));
        assert!(writable(1));
        assert!(!writable(3));
    }

    fn position(action: ProtocolPositionActionV2) -> ProtocolPositionRequestV2 {
        ProtocolPositionRequestV2 {
            action,
            owner_kind: ProtocolPositionOwnerKindV2::User,
            presence: match action {
                ProtocolPositionActionV2::Admit => ProtocolPositionPresenceV2::Vacant,
                ProtocolPositionActionV2::Close => ProtocolPositionPresenceV2::Existing,
            },
            release_set: id(1),
            market: id(2),
            position_owner: id(3),
            parent_request_digest: id(4),
            rent_credit: id(5),
            rent_program: id(6),
            generation: 7,
            expected_market_revision: 8,
            expected_position_revision: if action == ProtocolPositionActionV2::Admit {
                0
            } else {
                9
            },
            observed_position_lamports: 12,
            observed_admission_lamports: 13,
            position_rent_principal: 10,
            admission_rent_principal: 11,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        }
        .new()
        .expect("position request")
    }

    fn delta(direction: DeltaDirectionV2, magnitude: u64) -> SignedMagnitudeV2 {
        SignedMagnitudeV2::new(direction, magnitude).expect("delta")
    }

    fn affine_bytes() -> Vec<u8> {
        let positions = [
            AffineBatchPositionV2::new(id(3), 8).expect("source"),
            AffineBatchPositionV2::new(id(9), 0).expect("destination"),
        ];
        let rows = [AffineBatchRowV2::new(
            AffineBatchRowInputV2 {
                source_present: true,
                destination_present: true,
                outcome: 0,
                source_position_index: 0,
                destination_position_index: 1,
                aggregate_delta: delta(DeltaDirectionV2::Neutral, 0),
                source_delta: delta(DeltaDirectionV2::Debit, 5),
                destination_delta: delta(DeltaDirectionV2::Credit, 5),
            },
            2,
            2,
        )
        .expect("row")];
        let mut bytes = alloc::vec![0; plan_bytes(2, 1).expect("width")];
        AffineBatchPlanV2::encode_into(
            AffineBatchPlanInputV2 {
                caller_role: CallerRole::Trading,
                release_set: id(1),
                market: id(2),
                request_id: id(4),
                product_record_digest: id(10),
                semantic_basis_id: id(11),
                linked_basis_record_digest: id(12),
                expected_market_revision: 8,
                outcome_count: 2,
            },
            &positions,
            &rows,
            &mut bytes,
        )
        .expect("affine");
        bytes
    }

    fn signed_bytes() -> Vec<u8> {
        let positions = [
            SignedDeltaPositionV3::new(id(3), 8).expect("source"),
            SignedDeltaPositionV3::new(id(9), 4).expect("destination"),
        ];
        let aggregates = [
            SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("zero"),
            SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).expect("zero"),
        ];
        let rows = [
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 0,
                    outcome: 1,
                    delta: SignedDeltaV3::new(DeltaDirectionV3::Debit, 5).expect("debit"),
                },
                2,
                2,
            )
            .expect("source row"),
            PositionDeltaV3::new(
                PositionDeltaInputV3 {
                    position_index: 1,
                    outcome: 1,
                    delta: SignedDeltaV3::new(DeltaDirectionV3::Credit, 5).expect("credit"),
                },
                2,
                2,
            )
            .expect("destination row"),
        ];
        let mut bytes = alloc::vec![0; signed_plan_bytes(2, 2, 2).expect("width")];
        SignedDeltaPlanV3::encode_into(
            SignedDeltaPlanInputV3 {
                caller_role: CallerRole::Trading,
                release_set: id(1),
                market: id(2),
                request_id: id(4),
                product_record_digest: id(10),
                semantic_basis_id: id(11),
                linked_basis_record_digest: id(12),
                expected_market_revision: 8,
                claim_count: 2,
            },
            &positions,
            &aggregates,
            &rows,
            &mut bytes,
        )
        .expect("signed");
        bytes
    }

    fn sparse() -> SparseNativeTransferV1 {
        SparseNativeTransferV1::new(SparseNativeTransferInputV1 {
            caller_role: CallerRole::Trading,
            release_set: id(1),
            market: id(2),
            request_id: id(4),
            product_record_digest: id(10),
            semantic_basis_id: id(11),
            linked_basis_record_digest: id(12),
            source_owner: id(3),
            destination_owner: id(9),
            expected_market_revision: 8,
            expected_source_revision: 4,
            expected_destination_revision: 6,
            generation: 7,
            outcome: 1,
            claim_count: 2,
            quantity: 5,
        })
        .expect("sparse request")
    }

    fn representation_bytes() -> Vec<u8> {
        let mut asset = [0_u8; ASSET_BYTES_V3];
        AssetV2 {
            shard_mint: id(50),
            actor_shard_account: id(51),
            structured_custody_account: id(52),
            claims_custody_owner: id(53),
            coefficient: 10,
            expected_shard_supply: 20,
            expected_actor_shards: 10,
            expected_structured_shards: 10,
        }
        .encode_into(&mut asset)
        .expect("asset");
        let request = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::Denominate,
                caller_role: CallerRoleV2::Trading,
                release_set: id(1),
                market: id(2),
                graph_id: id(54),
                descriptor_id: id(55),
                parent_context: id(4),
                actor: id(56),
                receipt_mint: id(57),
                receipt_account: [0; 32],
                representation_authority: id(58),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: [0; 32],
                collateral_recipient: [0; 32],
                expected_representation_revision: 3,
                expected_claims_market_revision: 8,
                expected_actor_position_revision: 4,
                expected_custody_position_revision: 5,
                expected_custody_replay_revision: ABSENT_REVISION,
                generation: 7,
                quantity: 1,
                denominator: 10,
                expected_receipt_supply: 0,
                outcome_count: 2,
                selected_outcome: 1,
                asset_count: 1,
            },
            &asset,
        )
        .expect("representation");
        // The header width is a function of the action's class in v3.
        let mut bytes = alloc::vec![0_u8; request.wire_len().expect("class width")];
        request.encode_into(&mut bytes).expect("request bytes");
        bytes
    }

    fn fractional_bytes() -> Vec<u8> {
        FractionalExposureRequestV2::new(
            FractionalExposureActionV2::Wrap,
            dclutch_fractional_claim_contract::FractionalExposureRequestInputV2 {
                release_set: id(1),
                market: id(2),
                product_record: id(30),
                result_domain: id(31),
                terms: id(32),
                token_behavior: id(33),
                exposure: id(34),
                owner: id(35),
                source_token_account: [0; 32],
                destination_token_account: id(36),
                terminal_digest: [0; 32],
                expected_revision: 4,
                quantity: 2,
                representation_coordinate: 1,
            },
        )
        .expect("fractional request")
        .to_bytes()
        .expect("fractional bytes")
        .to_vec()
    }

    fn fractional_terminal_bytes(action: FractionalExposureActionV2) -> Vec<u8> {
        FractionalExposureRequestV2::new(
            action,
            dclutch_fractional_claim_contract::FractionalExposureRequestInputV2 {
                release_set: id(1),
                market: id(2),
                product_record: id(30),
                result_domain: id(31),
                terms: id(32),
                token_behavior: id(33),
                exposure: id(34),
                owner: id(35),
                source_token_account: id(36),
                destination_token_account: [0; 32],
                terminal_digest: id(37),
                expected_revision: 4,
                quantity: 2,
                representation_coordinate: 1,
            },
        )
        .expect("fractional terminal request")
        .to_bytes()
        .expect("fractional terminal bytes")
        .to_vec()
    }

    fn fractional_retirement_bytes(root: [u8; 32]) -> Vec<u8> {
        FractionalRetirementRequestV3::new(
            dclutch_fractional_claim_contract::FractionalRetirementActionV3::RetireCoordinate,
            dclutch_fractional_claim_contract::FractionalRetirementRequestInputV3 {
                release_set: id(1),
                market: id(2),
                terms: id(32),
                token_program: TOKEN_2022_PROGRAM_ID,
                token_behavior: id(33),
                exposure: id(34),
                root,
                rent_credit: id(50),
                expected_revision: 4,
                representation_coordinate: 1,
            },
        )
        .expect("fractional retirement request")
        .to_bytes()
        .expect("fractional retirement bytes")
        .to_vec()
    }

    fn representation_receipt_bytes(request_bytes: &[u8], claims: [u8; 32]) -> Vec<u8> {
        let request = RepresentationRequestV2::decode(request_bytes).expect("request");
        let header = request.header();
        let mut receipt = alloc::vec![0_u8; representation_wire::RECEIPT_BYTES_V2];
        let mut put = |offset: usize, value: &[u8]| {
            receipt
                .get_mut(offset..offset + value.len())
                .expect("receipt offset in bounds")
                .copy_from_slice(value);
        };
        put(
            representation_wire::RECEIPT_MAGIC_OFFSET,
            &representation_wire::RECEIPT_MAGIC_V2,
        );
        put(
            representation_wire::RECEIPT_VERSION_OFFSET,
            &representation_wire::PHYSICAL_ABI_VERSION_V3.to_le_bytes(),
        );
        put(
            representation_wire::RECEIPT_ACTION_OFFSET,
            &[representation_wire::ACTION_DENOMINATE],
        );
        put(
            representation_wire::RECEIPT_CALLER_ROLE_OFFSET,
            &[representation_wire::CALLER_ROLE_TRADING],
        );
        for (offset, value) in [
            (
                representation_wire::RECEIPT_RELEASE_SET_OFFSET,
                header.release_set,
            ),
            (representation_wire::RECEIPT_MARKET_OFFSET, header.market),
            (
                representation_wire::RECEIPT_GRAPH_ID_OFFSET,
                header.graph_id,
            ),
            (
                representation_wire::RECEIPT_DESCRIPTOR_ID_OFFSET,
                header.descriptor_id,
            ),
            (
                representation_wire::RECEIPT_PARENT_CONTEXT_OFFSET,
                header.parent_context,
            ),
            (
                representation_wire::RECEIPT_REQUEST_DIGEST_OFFSET,
                hash(request_bytes).to_bytes(),
            ),
            (representation_wire::RECEIPT_ACTOR_OFFSET, header.actor),
            (
                representation_wire::RECEIPT_REPRESENTATION_PROGRAM_OFFSET,
                claims,
            ),
            (representation_wire::RECEIPT_CLAIMS_PROGRAM_OFFSET, claims),
            (
                representation_wire::RECEIPT_TOKEN_PROGRAM_OFFSET,
                header.token_program,
            ),
            (
                representation_wire::RECEIPT_CLAIMS_PLAN_DIGEST_OFFSET,
                id(70),
            ),
            (
                representation_wire::RECEIPT_CLAIMS_RESOURCE_DIGEST_OFFSET,
                id(71),
            ),
            (
                representation_wire::RECEIPT_TOKEN_EFFECT_DIGEST_OFFSET,
                id(72),
            ),
            (
                representation_wire::RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
                id(73),
            ),
        ] {
            put(offset, &value);
        }
        for (offset, value) in [
            (
                representation_wire::RECEIPT_PRE_REPRESENTATION_REVISION_OFFSET,
                header.expected_representation_revision,
            ),
            (
                representation_wire::RECEIPT_POST_REPRESENTATION_REVISION_OFFSET,
                header.expected_representation_revision + 1,
            ),
            (
                representation_wire::RECEIPT_POST_CLAIMS_MARKET_REVISION_OFFSET,
                header.expected_claims_market_revision + 1,
            ),
            (
                representation_wire::RECEIPT_POST_ACTOR_POSITION_REVISION_OFFSET,
                header.expected_actor_position_revision + 1,
            ),
            (
                representation_wire::RECEIPT_POST_CUSTODY_POSITION_REVISION_OFFSET,
                header.expected_custody_position_revision + 1,
            ),
            (representation_wire::RECEIPT_POST_RECEIPT_SUPPLY_OFFSET, 0),
            (representation_wire::RECEIPT_PAYOUT_OFFSET, 0),
        ] {
            put(offset, &value.to_le_bytes());
        }
        put(
            representation_wire::RECEIPT_OUTCOME_COUNT_OFFSET,
            &header.outcome_count.to_le_bytes(),
        );
        RepresentationReceiptV2::decode(&receipt).expect("canonical representation receipt");
        receipt
    }

    fn founding(claims: [u8; 32], trading: [u8; 32]) -> ClaimsFoundingRequestV5 {
        ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            release_set: id(1),
            market: id(2),
            product_record_digest: id(30),
            product_instance_id: id(31),
            linked_basis_record_digest: id(32),
            semantic_basis_id: id(33),
            founder: id(34),
            founding_intent_digest: id(35),
            aggregate: id(36),
            position: id(37),
            admission: id(38),
            funding_source: id(39),
            hoard: id(40),
            custody_replay: id(41),
            rent_credit: id(42),
            rent_program: id(43),
            claims_program: claims,
            trading_program: trading,
            custody_request_digest: id(46),
            custody_receipt_digest: id(47),
            generation: 7,
            claim_count: 2,
            quantity: 2,
            basis_scale: 5,
            pre_source_amount: 10,
            post_source_amount: 0,
            pre_hoard_amount: 7,
            post_hoard_amount: 17,
            pre_custody_revision: 0,
            post_custody_revision: 1,
            aggregate_rent_principal: 100,
            position_rent_principal: 101,
            admission_rent_principal: 102,
            observed_aggregate_lamports: 100,
            observed_position_lamports: 101,
            observed_admission_lamports: 102,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        })
        .expect("founding request")
    }

    #[test]
    fn verifies_each_exact_claims_receipt_and_refuses_producer_substitution() {
        let claims = id(20);
        let trading = id(21);
        let admit = position(ProtocolPositionActionV2::Admit);
        let admit_bytes = admit.to_bytes().expect("admit bytes");
        let admission = ProtocolPositionAdmissionV2::new(
            admit,
            ProtocolPositionAdmissionEvidenceV2 {
                product_record_digest: id(30),
                semantic_basis_id: id(31),
                linked_basis_record_digest: id(32),
                request_digest: hash(&admit_bytes).to_bytes(),
                claims_program: claims,
                trading_program: trading,
                capability_descriptor: [0; 32],
                capability_outcome: 0,
                outcome_count: 258,
            },
        )
        .expect("admission");
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Admit,
                &admit_bytes,
                &admission.to_receipt_bytes().expect("receipt"),
                claims,
                trading,
                PostResourceEvidenceV3::None,
            ),
            Ok(ClaimsRouteReceiptV3::Admit(_))
        ));
        assert!(
            verify_route_receipt(
                ReceiptKindV3::Admit,
                &admit_bytes,
                &admission.to_receipt_bytes().expect("receipt"),
                id(99),
                trading,
                PostResourceEvidenceV3::None,
            )
            .is_err()
        );

        let affine_bytes = affine_bytes();
        let affine = AffineBatchPlanV2::decode(&affine_bytes).expect("plan");
        let affine_receipt = AffineBatchReceiptV2::new(
            affine,
            hash(&affine_bytes).to_bytes(),
            id(40),
            claims,
            id(41),
            9,
        )
        .expect("affine receipt")
        .to_bytes();
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Affine,
                &affine_bytes,
                &affine_receipt,
                claims,
                trading,
                PostResourceEvidenceV3::None,
            ),
            Ok(ClaimsRouteReceiptV3::Affine(_))
        ));

        let close = position(ProtocolPositionActionV2::Close);
        let close_bytes = close.to_bytes().expect("close bytes");
        let close_receipt = ProtocolPositionCloseReceiptV2::new(
            close,
            ProtocolPositionCloseEvidenceV2 {
                request_digest: hash(&close_bytes).to_bytes(),
                admission_digest: id(50),
                claims_program: claims,
                post_resource_digest: id(51),
                position_lamports: close.observed_position_lamports,
                admission_lamports: close.observed_admission_lamports,
                rent_credit_before: 100,
                rent_credit_after: 125,
            },
        )
        .expect("close receipt")
        .to_bytes()
        .expect("close bytes");
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Close,
                &close_bytes,
                &close_receipt,
                claims,
                trading,
                PostResourceEvidenceV3::None,
            ),
            Ok(ClaimsRouteReceiptV3::Close(_))
        ));
    }

    #[test]
    fn derives_nonaliasing_authority_contexts_from_exact_child_packets() {
        let admit = position(ProtocolPositionActionV2::Admit)
            .to_bytes()
            .expect("admit");
        let affine = affine_bytes();
        let representation = representation_bytes();
        let (admit_seeds, admit_kind) =
            route_authority(&admit, RouteKindV3::Once).expect("admit authority");
        let (affine_seeds, affine_kind) =
            route_authority(&affine, RouteKindV3::AffineOnce).expect("affine authority");
        let (representation_seeds, representation_kind) =
            route_authority(&representation, RouteKindV3::Once).expect("representation authority");
        assert_eq!(admit_kind, ReceiptKindV3::Admit);
        assert_eq!(affine_kind, ReceiptKindV3::Affine);
        assert_eq!(representation_kind, ReceiptKindV3::RationalRepresentation);
        let program = Pubkey::new_from_array(id(21));
        assert_ne!(
            Pubkey::find_program_address(&admit_seeds.as_slices(), &program).0,
            Pubkey::find_program_address(&affine_seeds.as_slices(), &program).0,
        );
        assert!(route_authority(&admit, RouteKindV3::AffineOnce).is_err());
        assert!(route_authority(&affine, RouteKindV3::Once).is_err());
        assert!(route_authority(&representation, RouteKindV3::AffineOnce).is_err());
        assert_ne!(
            Pubkey::find_program_address(&representation_seeds.as_slices(), &program).0,
            Pubkey::find_program_address(&affine_seeds.as_slices(), &program).0,
        );
    }

    #[test]
    fn fractional_authority_receipt_and_second_root_signer_are_exact() {
        let request = fractional_bytes();
        let decoded = FractionalExposureRequestV2::decode(&request).expect("request");
        let digest = hash(&request).to_bytes();
        let (seeds, kind) = route_authority(&request, RouteKindV3::Once).expect("authority");
        assert_eq!(kind, ReceiptKindV3::FractionalAtomic);
        assert_eq!(seeds.context(), decoded.input().terms);
        assert_eq!(seeds.role_request_digest(), digest);
        assert!(route_authority(&request, RouteKindV3::AffineOnce).is_err());

        let receipt = FractionalAtomicReceiptV3::new(
            decoded.action(),
            digest,
            id(40),
            id(41),
            id(42),
            id(43),
            id(44),
            5,
            6,
            7,
            200,
            200,
            200,
        )
        .expect("receipt");
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::FractionalAtomic,
                &request,
                &receipt.to_bytes(),
                id(20),
                id(21),
                PostResourceEvidenceV3::FractionalRoot(id(44)),
            ),
            Ok(ClaimsRouteReceiptV3::FractionalAtomic(_))
        ));
        let mut substituted = receipt.to_bytes();
        substituted[16] ^= 1;
        assert!(
            verify_route_receipt(
                ReceiptKindV3::FractionalAtomic,
                &request,
                &substituted,
                id(20),
                id(21),
                PostResourceEvidenceV3::FractionalRoot(id(44)),
            )
            .is_err()
        );

        let trading = Pubkey::new_from_array(id(21));
        let (root_info, bump) = fractional_root_info(decoded, trading);
        let mut accounts: Vec<_> = (0
            ..dclutch_fractional_claim_contract::FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3)
            .map(|_| account_info(false, false))
            .collect();
        accounts[FRACTIONAL_ATOMIC_ROOT_V3] = root_info;
        let mut metas: Vec<_> = accounts
            .iter()
            .map(|account| {
                if account.is_writable {
                    solana_program::instruction::AccountMeta::new(*account.key, false)
                } else {
                    solana_program::instruction::AccountMeta::new_readonly(*account.key, false)
                }
            })
            .collect();
        let signer = fractional_root_signer(
            &trading,
            ReceiptKindV3::FractionalAtomic,
            &request,
            &accounts,
            &mut metas,
        )
        .expect("root signer")
        .expect("present");
        assert_eq!(signer.bump, bump);
        assert!(metas[FRACTIONAL_ATOMIC_ROOT_V3].is_signer);
        metas[FRACTIONAL_ATOMIC_ROOT_V3].pubkey = Pubkey::new_unique();
        assert!(
            fractional_root_signer(
                &trading,
                ReceiptKindV3::FractionalAtomic,
                &request,
                &accounts,
                &mut metas,
            )
            .is_err()
        );
    }

    #[test]
    fn fractional_terminal_uses_distinct_receipt_frame_and_exact_root_signer() {
        for (action, payout) in [
            (FractionalExposureActionV2::TerminalRedeem, 9_u64),
            (FractionalExposureActionV2::TerminalZeroBurn, 0_u64),
        ] {
            let request = fractional_terminal_bytes(action);
            let decoded = FractionalExposureRequestV2::decode(&request).expect("request");
            let digest = hash(&request).to_bytes();
            let (seeds, kind) = route_authority(&request, RouteKindV3::Once).expect("authority");
            assert_eq!(kind, ReceiptKindV3::FractionalTerminalAtomic);
            assert_eq!(seeds.context(), decoded.input().terms);
            assert_eq!(seeds.role_request_digest(), digest);
            assert!(route_authority(&request, RouteKindV3::AffineOnce).is_err());

            let receipt = FractionalTerminalAtomicReceiptV3::new(
                action,
                digest,
                id(40),
                id(41),
                id(42),
                id(43),
                id(44),
                payout,
                6,
                7,
                2,
            )
            .expect("terminal receipt");
            assert!(matches!(
                verify_route_receipt(
                    ReceiptKindV3::FractionalTerminalAtomic,
                    &request,
                    &receipt.to_bytes(),
                    id(20),
                    id(21),
                    PostResourceEvidenceV3::FractionalRoot(id(44)),
                ),
                Ok(ClaimsRouteReceiptV3::FractionalTerminalAtomic(_))
            ));

            let trading = Pubkey::new_from_array(id(21));
            let (root_info, _bump) = fractional_root_info(decoded, trading);
            let mut accounts: Vec<_> = (0
                ..dclutch_fractional_claim_contract::FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3)
                .map(|_| account_info(false, false))
                .collect();
            accounts[FRACTIONAL_TERMINAL_ROOT_V3] = root_info;
            let mut metas: Vec<_> = accounts
                .iter()
                .map(|account| {
                    if account.is_writable {
                        solana_program::instruction::AccountMeta::new(*account.key, false)
                    } else {
                        solana_program::instruction::AccountMeta::new_readonly(*account.key, false)
                    }
                })
                .collect();
            assert!(
                fractional_root_signer(
                    &trading,
                    ReceiptKindV3::FractionalTerminalAtomic,
                    &request,
                    &accounts,
                    &mut metas,
                )
                .expect("root signer")
                .is_some()
            );
            assert!(metas[FRACTIONAL_TERMINAL_ROOT_V3].is_signer);
            assert!(!metas[FRACTIONAL_ATOMIC_ROOT_V3].is_signer);
        }
    }

    #[test]
    fn fractional_retirement_binds_exact_authority_receipt_and_root_signer() {
        let trading = Pubkey::new_from_array(id(21));
        let seed_request =
            FractionalExposureRequestV2::decode(&fractional_bytes()).expect("seed request");
        let (root_info, bump) = fractional_root_info(seed_request, trading);
        let request = fractional_retirement_bytes(root_info.key.to_bytes());
        let decoded = FractionalRetirementRequestV3::decode(&request).expect("request");
        let digest = hash(&request).to_bytes();
        let (seeds, kind) = route_authority(&request, RouteKindV3::Once).expect("authority");
        assert_eq!(kind, ReceiptKindV3::FractionalRetirementCoordinate);
        assert_eq!(seeds.context(), decoded.input().terms);
        assert_eq!(seeds.role_request_digest(), digest);
        assert!(route_authority(&request, RouteKindV3::AffineOnce).is_err());

        let receipt = FractionalRetirementCoordinateReceiptV3::new(
            decoded,
            digest,
            id(41),
            id(42),
            id(43),
            id(44),
            5,
        )
        .expect("retirement receipt");
        assert!(matches!(
            verify_route_receipt(
                kind,
                &request,
                &receipt.to_bytes(),
                id(20),
                trading.to_bytes(),
                PostResourceEvidenceV3::FractionalRoot(root_info.key.to_bytes()),
            ),
            Ok(ClaimsRouteReceiptV3::FractionalRetirementCoordinate(_))
        ));
        assert!(
            verify_route_receipt(
                kind,
                &request,
                &receipt.to_bytes(),
                id(20),
                trading.to_bytes(),
                PostResourceEvidenceV3::FractionalRoot(id(60)),
            )
            .is_err()
        );

        let mut accounts: Vec<_> = (0
            ..dclutch_fractional_claim_contract::FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3)
            .map(|_| account_info(false, false))
            .collect();
        accounts[FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3] = root_info;
        let mut metas: Vec<_> = accounts
            .iter()
            .map(|account| {
                if account.is_writable {
                    solana_program::instruction::AccountMeta::new(*account.key, false)
                } else {
                    solana_program::instruction::AccountMeta::new_readonly(*account.key, false)
                }
            })
            .collect();
        let signer = fractional_root_signer(&trading, kind, &request, &accounts, &mut metas)
            .expect("root signer")
            .expect("present");
        assert_eq!(signer.bump, bump);
        assert!(metas[FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3].is_signer);
        metas[FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3].pubkey = Pubkey::new_unique();
        assert!(fractional_root_signer(&trading, kind, &request, &accounts, &mut metas).is_err());
    }

    fn fractional_compaction_bytes(root: [u8; 32]) -> Vec<u8> {
        use dclutch_claims_svm::terminal_settlement_v3::{
            TerminalSettlementRequestInputV3, TerminalSettlementRequestV3,
        };
        let settlement = TerminalSettlementRequestV3::new(TerminalSettlementRequestInputV3 {
            caller_role: CallerRole::Claims,
            release_set: id(1),
            market: id(2),
            realm: id(3),
            parent_context: id(4),
            product_record_digest: id(5),
            exposure_id: id(6),
            exposure_digest: id(7),
            terminal_record_digest: id(8),
            // The reserve Position's owner IS the capability root; the request
            // reads it from here rather than carrying a second field.
            owner: root,
            position: id(10),
            recipient_owner: id(40),
            recipient_token_account: id(41),
            claims_program: id(20),
            custody_program: id(13),
            collateral_mint: id(14),
            token_program: id(15),
            semantic_basis_id: id(16),
            linked_basis_record_digest: id(17),
            generation: 9,
            expected_market_revision: 3,
            expected_position_revision: 4,
            expected_custody_revision: 5,
            quantity: 700,
            claim_index: 1,
            transfer_index: 0,
        })
        .expect("settlement");
        FractionalCompactToClaimCheckRequestV1::new(
            dclutch_claims_svm::fractional_claim_check_compaction_request_v1::FractionalCompactionCoordinatesV1 {
                terms: id(32),
                token_behavior: id(33),
                expected_root_revision: 11,
                representation_coordinate: 6,
                payout_per_claim: 4_000,
            },
            settlement,
        )
        .expect("fractional compaction request")
        .to_bytes()
        .expect("fractional compaction bytes")
        .to_vec()
    }

    #[test]
    fn fractional_compaction_selects_a_route_and_stops_at_its_missing_receipt() {
        // The layer FRACCHECK-2 found absent, from the execution side. Route
        // selection is what was missing -- a magic that reached
        // `route_authority` at all -- and this asserts it now resolves to its
        // own receipt kind with its own authority context.
        let trading = Pubkey::new_from_array(id(21));
        let seed_request =
            FractionalExposureRequestV2::decode(&fractional_bytes()).expect("seed request");
        let (root_info, _bump) = fractional_root_info(seed_request, trading);
        let request = fractional_compaction_bytes(root_info.key.to_bytes());
        let decoded =
            FractionalCompactToClaimCheckRequestV1::decode(&request).expect("compaction request");
        let digest = hash(&request).to_bytes();

        let (seeds, kind) = route_authority(&request, RouteKindV3::Once).expect("authority");
        assert_eq!(kind, ReceiptKindV3::FractionalClaimCheckCompaction);
        // The fourth seed is `terms`, as it is for both Fractional siblings --
        // and it comes from the compaction coordinates, because the terminal
        // header this request embeds carries no terms at all.
        assert_eq!(seeds.context(), decoded.coordinates().terms);
        assert_ne!(seeds.context(), decoded.input().parent_context);
        assert_eq!(seeds.role_request_digest(), digest);
        // One route kind, like every other Claims once-route.
        assert!(route_authority(&request, RouteKindV3::AffineOnce).is_err());

        // AND THE HONEST HALF. There is no receipt type yet, so the verifier
        // refuses rather than admitting an unchecked route. This test exists to
        // make that refusal a stated property: if a later lane adds a receipt
        // and forgets this arm, the assertion below fails and says so.
        assert!(
            verify_route_receipt(
                kind,
                &request,
                &[],
                id(20),
                trading.to_bytes(),
                PostResourceEvidenceV3::FractionalRoot(root_info.key.to_bytes()),
            )
            .is_err(),
            "a route with no receipt type must refuse, never verify vacuously"
        );

        // The delivery answer is the Fractional siblings', not the suffix
        // families': a suffix would change the packet digest the child derives
        // its caller authority from, so it is refused rather than ignored.
        assert_eq!(kind.delivery(), ReceiptDeliveryV3::VerifiedOnly);
        assert!(!carries_caller_bump_suffix_v4(kind));
    }

    /// The compaction frame, with its root at the index the declaration names.
    ///
    /// `writable` and `revision` are the caller's, because the two hostile
    /// witnesses below differ from the admitted one in exactly one of them.
    fn fractional_compaction_frame(
        trading: Pubkey,
        revision: u64,
        writable: bool,
    ) -> (
        Vec<u8>,
        Pubkey,
        u8,
        Vec<AccountInfo<'static>>,
        Vec<solana_program::instruction::AccountMeta>,
        usize,
    ) {
        // `fractional_compaction_bytes` pins release set 1, market 2 and terms
        // 32; the root has to be built at the same three or it is a different
        // root, which is a different refusal than the one under test.
        let (root_info, bump) =
            fractional_root_info_at(trading, id(1), id(2), id(32), revision, writable);
        let root_key = *root_info.key;
        let request = fractional_compaction_bytes(root_key.to_bytes());
        let root_index = FractionalCompactionRoleV1::FractionalCapabilityRoot
            .index()
            .expect("the root is admitted to its own frame");
        let mut accounts: Vec<_> = (0
            ..dclutch_claims_svm::fractional_claim_check_v1::FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1)
            .map(|_| account_info(false, false))
            .collect();
        accounts[root_index] = root_info;
        let metas: Vec<_> = accounts
            .iter()
            .map(|account| {
                if account.is_writable {
                    solana_program::instruction::AccountMeta::new(*account.key, false)
                } else {
                    solana_program::instruction::AccountMeta::new_readonly(*account.key, false)
                }
            })
            .collect();
        (request, root_key, bump, accounts, metas, root_index)
    }

    #[test]
    fn fractional_compaction_root_signs_read_only_and_refuses_a_writable_one() {
        // Design §17.8 ruling 1, witnesses w1-w4. The gate gains the compaction
        // kind so the root's signature can reach the ONE thing on this route
        // that can happen no other way: `SetAuthority(PermissionedBurn)`,
        // root -> escrow, which Token-2022 refuses without the current
        // authority's signature and which nothing but a Trading `invoke_signed`
        // can produce.
        let trading = Pubkey::new_from_array(id(21));
        let kind = ReceiptKindV3::FractionalClaimCheckCompaction;
        // 11 is `fractional_compaction_bytes`'s own `expected_root_revision`,
        // written here as the literal it is rather than read back off the
        // request under test.
        let revision = 11;

        // w1 -- THE ARM WORKS. A read-only root at the declared index is
        // authenticated, its meta is marked a signer, and the seeds and bump
        // handed back are the root's own.
        let (request, root_key, bump, accounts, mut metas, root_index) =
            fractional_compaction_frame(trading, revision, false);
        let signer = fractional_root_signer(&trading, kind, &request, &accounts, &mut metas)
            .expect("the compaction arm authenticates its root")
            .expect("and returns a signer rather than None");
        assert_eq!(signer.bump, bump);
        assert_eq!(
            Pubkey::find_program_address(&signer.seeds.as_slices(), &trading),
            (root_key, bump),
            "the seeds handed back must re-derive the very root that was read"
        );
        assert!(metas[root_index].is_signer, "w1: the meta is marked signer");
        // And the signature did NOT come with write access. This is the half
        // that makes the inversion worth having: a compaction revises nothing.
        assert!(
            !metas[root_index].is_writable,
            "w1: the root signs a compaction without becoming writable"
        );
        // The shared predicates still bite on this kind: a substituted meta is
        // refused exactly as it is for the three neighbouring kinds.
        metas[root_index].pubkey = Pubkey::new_unique();
        assert!(fractional_root_signer(&trading, kind, &request, &accounts, &mut metas).is_err());

        // w2 -- REVISION MISMATCH REFUSED. The root is otherwise perfect and
        // sits at the right index; only its recorded revision disagrees with
        // the request's `expected_root_revision`, which is that field's whole
        // purpose. Off by one, because a test that moves it far proves less.
        let (request, _, _, accounts, mut metas, _) =
            fractional_compaction_frame(trading, revision + 1, false);
        assert!(
            fractional_root_signer(&trading, kind, &request, &accounts, &mut metas).is_err(),
            "w2: a root at the wrong revision must not be handed a signature"
        );

        // w3 -- THE INVERSION, WITNESSED. A WRITABLE root is refused on this
        // kind. Its three neighbours require exactly the opposite, so without
        // this assertion a later reader "restoring consistency" with them would
        // silently widen what a compaction may touch, and every other test here
        // would stay green. That is the mutation this line exists to red.
        let (request, _, _, accounts, mut metas, _) =
            fractional_compaction_frame(trading, revision, true);
        assert!(
            fractional_root_signer(&trading, kind, &request, &accounts, &mut metas).is_err(),
            "w3: a compaction revises nothing, so a writable root is refused -- \
             this is the ONE predicate that inverts against the exposure arms"
        );

        // w4 -- THE KEPT CONTROL. The gate is a gate: a kind it does not name
        // gets `Ok(None)` and no signature, over these very accounts. Before
        // ruling 1 the compaction kind was in this class, which is why the route
        // was unreachable; deleting the arm from the `matches!` returns it here.
        // Verified by mutation, not by assertion: with the compaction kind
        // removed from the gate, w1 fails at `.expect("and returns a signer
        // rather than None")`.
        let (request, _, _, accounts, mut metas, root_index) =
            fractional_compaction_frame(trading, revision, false);
        assert!(
            fractional_root_signer(
                &trading,
                ReceiptKindV3::RationalRepresentation,
                &request,
                &accounts,
                &mut metas,
            )
            .expect("an unnamed kind is not an error, it is simply not signed")
            .is_none(),
            "w4: the gate signs only for the kinds it names"
        );
        assert!(
            !metas[root_index].is_signer,
            "w4: and it leaves the meta alone on the way out"
        );
    }

    #[test]
    fn verifies_rational_representation_receipt_and_refuses_every_program_substitution() {
        let request = representation_bytes();
        let claims = id(20);
        let receipt = representation_receipt_bytes(&request, claims);
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::RationalRepresentation,
                &request,
                &receipt,
                claims,
                id(21),
                PostResourceEvidenceV3::None,
            ),
            Ok(ClaimsRouteReceiptV3::RationalRepresentation(_))
        ));

        for hostile_claims in [id(98), id(99)] {
            assert!(
                verify_route_receipt(
                    ReceiptKindV3::RationalRepresentation,
                    &request,
                    &receipt,
                    hostile_claims,
                    id(21),
                    PostResourceEvidenceV3::None,
                )
                .is_err()
            );
        }
        assert!(
            verify_route_receipt(
                ReceiptKindV3::RationalRepresentation,
                &request,
                receipt
                    .get(..receipt.len() - 1)
                    .expect("receipt is non-empty"),
                claims,
                id(21),
                PostResourceEvidenceV3::None,
            )
            .is_err()
        );

        let mut substituted_receipt = receipt.clone();
        *substituted_receipt
            .get_mut(representation_wire::RECEIPT_TOKEN_PROGRAM_OFFSET)
            .expect("token program offset inside the receipt") ^= 1;
        assert!(
            verify_route_receipt(
                ReceiptKindV3::RationalRepresentation,
                &request,
                &substituted_receipt,
                claims,
                id(21),
                PostResourceEvidenceV3::None,
            )
            .is_err()
        );

        let mut substituted_request = request.clone();
        *substituted_request
            .get_mut(representation_wire::REQUEST_QUANTITY_OFFSET_V3)
            .expect("quantity offset inside the request") ^= 1;
        assert!(
            verify_route_receipt(
                ReceiptKindV3::RationalRepresentation,
                &substituted_request,
                &receipt,
                claims,
                id(21),
                PostResourceEvidenceV3::None,
            )
            .is_err()
        );
    }

    #[test]
    fn verifies_founding_authority_and_exact_post_resources() {
        let claims = id(20);
        let trading = id(21);
        let request = founding(claims, trading);
        let request_bytes = request.to_bytes();
        let mut accounts: Vec<_> = (0..33).map(|_| account_info(false, false)).collect();
        for slot in 2..=4 {
            *accounts
                .get_mut(slot)
                .expect("post-resource slot inside the frame") = account_info(false, true);
        }
        let post = founding_post_resource_digests(&accounts).expect("post resources");
        let receipt = ClaimsFoundingReceiptV5::new(
            request,
            hash(&request_bytes).to_bytes(),
            post.aggregate,
            post.position,
            post.admission,
            post.combined,
        )
        .expect("founding receipt")
        .to_bytes();

        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::Founding,
                &request_bytes,
                &receipt,
                claims,
                trading,
                PostResourceEvidenceV3::Founding(post),
            ),
            Ok(ClaimsRouteReceiptV3::Founding(_))
        ));
        for (producer, selected_trading, evidence) in [
            (id(99), trading, PostResourceEvidenceV3::Founding(post)),
            (claims, id(99), PostResourceEvidenceV3::Founding(post)),
            (
                claims,
                trading,
                PostResourceEvidenceV3::Founding(FoundingPostResourceDigestsV5 {
                    combined: id(99),
                    ..post
                }),
            ),
        ] {
            assert!(
                verify_route_receipt(
                    ReceiptKindV3::Founding,
                    &request_bytes,
                    &receipt,
                    producer,
                    selected_trading,
                    evidence,
                )
                .is_err()
            );
        }

        let (seeds, kind) =
            route_authority(&request_bytes, RouteKindV3::Once).expect("founding authority");
        assert_eq!(kind, ReceiptKindV3::Founding);
        assert_eq!(seeds.context(), request.founding_intent_digest());
        let permit = id(90);
        let transport = SeriesClaimsFoundingTransportV1::from_canonical_v5(permit, request)
            .expect("Series transport")
            .to_bytes();
        let (series_seeds, series_kind) =
            route_authority(&transport, RouteKindV3::Once).expect("Series authority");
        assert_eq!(series_kind, ReceiptKindV3::SeriesFounding);
        assert_eq!(series_seeds.context(), permit);
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::SeriesFounding,
                &transport,
                &receipt,
                claims,
                trading,
                PostResourceEvidenceV3::Founding(post),
            ),
            Ok(ClaimsRouteReceiptV3::Founding(_))
        ));
        let mut malformed_transport = transport;
        malformed_transport[0] ^= 1;
        assert!(route_authority(&malformed_transport, RouteKindV3::Once).is_err());
        assert!(
            verify_route_receipt(
                ReceiptKindV3::SeriesFounding,
                &malformed_transport,
                &receipt,
                claims,
                trading,
                PostResourceEvidenceV3::Founding(post),
            )
            .is_err()
        );
        assert!(route_authority(&request_bytes, RouteKindV3::AffineOnce).is_err());
        assert!(
            founding_post_resource_digests(accounts.get(..32).expect("frame holds 33 accounts"))
                .is_err()
        );
    }

    #[test]
    fn verifies_signed_delta_table_producer_and_post_resources() {
        let request = signed_bytes();
        let plan = SignedDeltaPlanV3::decode(&request).expect("plan");
        let (positions, aggregates, deltas) = plan.table_bytes();
        let table_digest = hashv(&[
            b"dclutch/claims/signed-delta-table/v3",
            positions,
            aggregates,
            deltas,
        ])
        .to_bytes();
        let claims = id(20);
        let post_resources = id(21);
        let receipt = SignedDeltaReceiptV3::new(
            plan,
            hash(&request).to_bytes(),
            table_digest,
            claims,
            post_resources,
            9,
        )
        .expect("receipt")
        .to_bytes();
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::SignedDelta,
                &request,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::Single(post_resources),
            ),
            Ok(ClaimsRouteReceiptV3::SignedDelta(_))
        ));
        assert!(
            verify_route_receipt(
                ReceiptKindV3::SignedDelta,
                &request,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::Single(id(99)),
            )
            .is_err()
        );
    }

    #[test]
    fn verifies_sparse_authority_producer_packet_and_post_resources() {
        let request = sparse();
        let request_bytes = request.to_bytes();
        let claims = id(20);
        let post_resources = id(21);
        let receipt = SparseNativeTransferReceiptV1::new(
            request,
            hash(&request_bytes).to_bytes(),
            claims,
            post_resources,
            9,
            5,
            7,
        )
        .expect("sparse receipt")
        .to_bytes();
        assert_eq!(receipt.len(), SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1);
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::SparseNativeTransfer,
                &request_bytes,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::Single(post_resources),
            ),
            Ok(ClaimsRouteReceiptV3::SparseNativeTransfer(_))
        ));
        // Direct's complete typed finalization compares this exact receipt in
        // the child transcript and hashes the three actual account bodies
        // itself. Only that explicit policy may defer the duplicate hash here;
        // packet and selected-producer authentication remain mandatory.
        assert!(matches!(
            verify_route_receipt(
                ReceiptKindV3::SparseNativeTransfer,
                &request_bytes,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::DeferredDirect,
            ),
            Ok(ClaimsRouteReceiptV3::SparseNativeTransfer(_))
        ));
        assert!(
            verify_route_receipt(
                ReceiptKindV3::SparseNativeTransfer,
                &request_bytes,
                &receipt,
                id(99),
                id(30),
                PostResourceEvidenceV3::DeferredDirect,
            )
            .is_err()
        );
        assert!(
            verify_route_receipt(
                ReceiptKindV3::SparseNativeTransfer,
                &request_bytes,
                &receipt,
                claims,
                id(30),
                PostResourceEvidenceV3::None,
            )
            .is_err()
        );
        assert!(
            verify_route_receipt(
                ReceiptKindV3::SparseNativeTransfer,
                &request_bytes,
                receipt
                    .get(..receipt.len() - 1)
                    .expect("receipt is non-empty"),
                claims,
                id(30),
                PostResourceEvidenceV3::Single(post_resources),
            )
            .is_err()
        );
        for (producer, poststate) in [(id(99), post_resources), (claims, id(99))] {
            assert!(
                verify_route_receipt(
                    ReceiptKindV3::SparseNativeTransfer,
                    &request_bytes,
                    &receipt,
                    producer,
                    id(30),
                    PostResourceEvidenceV3::Single(poststate),
                )
                .is_err()
            );
        }

        let program = Pubkey::new_from_array(id(30));
        let (authority, kind) =
            route_authority(&request_bytes, RouteKindV3::Once).expect("sparse authority");
        assert_eq!(kind, ReceiptKindV3::SparseNativeTransfer);
        assert!(route_authority(&request_bytes, RouteKindV3::AffineOnce).is_err());
        let mut changed = request.input();
        changed.quantity = 6;
        let changed = SparseNativeTransferV1::new(changed)
            .expect("changed request")
            .to_bytes();
        let (changed_authority, _) =
            route_authority(&changed, RouteKindV3::Once).expect("changed authority");
        assert_ne!(
            Pubkey::find_program_address(&authority.as_slices(), &program).0,
            Pubkey::find_program_address(&changed_authority.as_slices(), &program).0,
        );
    }

    #[test]
    fn sparse_resource_digest_binds_exact_aggregate_source_destination_poststates() {
        let mut accounts: Vec<_> = (0..23).map(|_| account_info(false, false)).collect();
        for slot in [1, 20, 21] {
            *accounts
                .get_mut(slot)
                .expect("post-resource slot inside the frame") = account_info(false, true);
        }
        let expected = hashv(&[b"dclutch/claims/sparse-native-post/v1", &[], &[], &[]]).to_bytes();
        assert_eq!(sparse_native_post_resource_digest(&accounts), Ok(expected));
        assert!(
            sparse_native_post_resource_digest(
                accounts.get(..22).expect("frame holds 23 accounts")
            )
            .is_err()
        );
    }
}
