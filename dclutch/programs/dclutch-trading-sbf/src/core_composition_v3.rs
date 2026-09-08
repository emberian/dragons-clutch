//! Family-neutral Core CPI execution for EffectProgram V3 routes.
//!
//! The selected EffectProgram owns the exact request/account frame. This
//! adapter currently admits the canonical recurring-Series Core request ABI;
//! it appends only the EffectProgram-authenticated borrowed witness, signs the
//! release-pinned Trading authority, and accepts only the immediate current
//! Core producer's typed acknowledgment.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_market::{
    FOUND_ACCOUNT_COUNT_V3, Identity, SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1,
    SERIES_CONSUME_FOUND_SUFFIX_START_V1, SERIES_CORE_REQUEST_MAGIC_V1,
    SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1, SERIES_OPEN_ACCOUNT_COUNT_V1,
    SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1, SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1,
    SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1, SeriesCoreAckV1, SeriesCoreActionV1,
    SeriesCoreFoundAckV2, SeriesCoreRequestV1, SeriesPermitExpiryRequestV1,
    SeriesUnallocatedPermitExpiryRequestV1,
};
#[cfg(test)]
use dclutch_market::{
    SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1, SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::series::request::{
    SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3,
};
use dclutch_trading::series::{
    AccountKeyV3, TemplateV3, funding_list_id,
    replay::{
        SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SeriesStateV3, TicketPhaseV3,
        TicketStateV3,
    },
    ticket_content_id,
};
use dclutch_vm::effect::{
    v2::FixedRole,
    v3::{ProgramV3 as EffectProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::child_authority_v4::{PreflightedCallerBumpV4, child_caller_authority_v4};
use crate::{
    TradingSbfError,
    child_receipt_v3::{ReceiptDeliveryV3, deliver_receipt_dependency_v3},
    child_refused_v1,
    hot_v3::{BorrowedRouteRangesV4, ChildInvocationBuffersV3, DowngradedEffectAccountsV3},
};

const CORE_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-core-receipt:v3";
const CORE_RECEIPTLESS_EXPIRY_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-core-permit-expiry:v1";
const CORE_PRECOMMIT_EXPIRY_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch:hot-core-series-expiry-precommit:v1";
// Chain-facing frame width owned by Core's Series permit-expiry adapter. The
// optional funded-crank successor is not part of the selected Series V4 route.
const SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1: u16 = 25;
const SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1: u16 = 26;
const SERIES_PRECOMMIT_REPLAY_COORDINATES_V1: [usize; 2] = [14, 15];

/// The authenticated precommit child observes replay state that Trading alone
/// commits after the child returns. AccountProfile aliases inherit their
/// physical representative's writability; the typed Core boundary must remove
/// that privilege explicitly before making the observation CPI.
pub(crate) fn series_precommit_replay_observation_v1<'info>(
    mut account: AccountInfo<'info>,
) -> Result<AccountInfo<'info>, ProgramError> {
    if account.is_signer || account.executable {
        return Err(TradingSbfError::Content.into());
    }
    account.is_writable = false;
    Ok(account)
}
/// Recognize the one Core invocation which intentionally observes the two
/// Trading replay prestates before Trading commits their Expire candidates.
///
/// This grants no execution authority. The ordinary Core preflight repeats
/// the complete check and derives the signed caller before CPI; Hot uses this
/// closed predicate only to admit the exact root+Ticket post-CPI local-write
/// pair through its otherwise strict child/local separation rule.
pub(crate) fn is_series_permit_expiry_precommit_observation_v1(
    effect: EffectProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    invocation: ResolvedInvocationV3,
    request_bank: &[u8],
    family_request: &[u8],
    parent: CoreCompositionParentV3,
) -> Result<bool, ProgramError> {
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let request = request_bank
        .get(invocation.request_offset..request_end)
        .ok_or(TradingSbfError::Content)?;
    let family = match SeriesActionRequestV3::decode(family_request) {
        Ok(request) if request.action() == SeriesActionV3::Expire => request,
        Ok(_) | Err(_) => return Ok(false),
    };
    let precommit = match authenticate_core_request(request, parent, family_request) {
        Ok(AuthenticatedCoreRequestV3::UnallocatedPermitExpiry(request, ticket)) => {
            ticket == family.ticket().ok_or(TradingSbfError::Content)?.to_bytes()
                && request.expected_series_revision() == family.expected_series_revision()
                && request.expected_ticket_revision() == family.expected_ticket_revision()
        }
        Ok(_) | Err(_) => false,
    };
    Ok(invocation.role == FixedRole::Core
        && invocation.kind == RouteKindV3::Once
        && invocation.item.is_none()
        && invocation.repeated_item_count == 0
        && invocation_index == 0
        && route_index.checked_add(1) == Some(effect.route_count())
        && invocation.fixed_account_count == SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1
        && invocation.borrowed_witness.is_none()
        && invocation.receipt_dependencies.is_empty()
        && !has_receipt_dependent(effect, route_index, invocation_index)?
        && precommit)
}

/// Result shape of one authenticated Core route.
///
/// Ordinary Core routes return and authenticate a typed receipt. The one
/// permissionless permit-expiry route has an intentionally empty Core return
/// channel; its success is committed only as a dedicated transcript digest and
/// cannot satisfy another Effect route's receipt dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreRouteExecutionV3 {
    /// Typed immediate Core receipt was authenticated and remains in buffers.
    ReturnedReceipt([u8; 32]),
    /// Typed permit expiry succeeded with an empty return channel.
    ReceiptlessPermitExpiry([u8; 32]),
}

impl CoreRouteExecutionV3 {
    /// Digest committed into the common child transcript.
    pub(crate) const fn digest(self) -> [u8; 32] {
        match self {
            Self::ReturnedReceipt(digest) | Self::ReceiptlessPermitExpiry(digest) => digest,
        }
    }

    /// Whether the common receipt bank must omit this successful invocation.
    pub(crate) const fn receiptless(self) -> bool {
        matches!(self, Self::ReceiptlessPermitExpiry(_))
    }
}

/// Immutable parent facts every projected Core request must reproduce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreCompositionParentV3 {
    /// Current immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Current Market generation.
    pub generation: u64,
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
}

/// Preflight one exact active Core invocation without external mutation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn preflight_core_route_v3<'info>(
    program_id: &Pubkey,
    effect: EffectProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    invocation: ResolvedInvocationV3,
    successor_account_count: usize,
    borrowed_ranges: BorrowedRouteRangesV4<'_, '_, '_>,
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    frame: &mut Vec<AccountInfo<'info>>,
    wire: &mut Vec<u8>,
    core_program: &AccountInfo<'_>,
    parent: CoreCompositionParentV3,
    // The caller's mined bump for this invocation's Trading caller authority.
    // `None` searches, exactly as this walk always did; `Some` reproduces and
    // refuses at the coordinate-0 equality `prepare` already runs. See
    // `HotBumpHintsV1`.
    hint: PreflightedCallerBumpV4,
) -> Result<u8, ProgramError> {
    let prepared = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        invocation,
        successor_account_count,
        borrowed_ranges,
        effect_accounts,
        request_bank,
        frame,
        wire,
        core_program,
        parent,
        hint,
    )?;
    Ok(prepared.bump())
}

/// Execute one preflighted Core invocation and verify its immediate receipt.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_core_route_v3<'info>(
    program_id: &Pubkey,
    effect: EffectProgramV3<'_>,
    invocation: ResolvedInvocationV3,
    route_index: u16,
    invocation_index: u32,
    successor_account_count: usize,
    borrowed_ranges: BorrowedRouteRangesV4<'_, '_, '_>,
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    prior_receipt: Option<&[u8]>,
    buffers: &mut ChildInvocationBuffersV3<'info>,
    core_program: &AccountInfo<'info>,
    parent: CoreCompositionParentV3,
    // The bump `preflight_core_route_v3` already derived over byte-identical
    // seeds; see `crate::child_authority_v4`.
    preflighted_bump: PreflightedCallerBumpV4,
) -> Result<CoreRouteExecutionV3, ProgramError> {
    // `prepare` leaves the authenticated frame and the child wire IN the walk's
    // buffers. It used to build both for its own authority check and then be
    // handed a second copy of each here.
    let prepared = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        invocation,
        successor_account_count,
        borrowed_ranges,
        effect_accounts,
        request_bank,
        &mut buffers.accounts,
        &mut buffers.data,
        core_program,
        parent,
        preflighted_bump,
    )?;
    match prepared {
        PreparedCoreInvocationV3::Series {
            invocation,
            request,
            request_digest,
            authority_seeds,
            bump,
        } => {
            // Core's Series Consume ABI genuinely READS the producer receipt:
            // Core splits the typed Custody/Claims suffix itself. The suffix
            // is that ABI, not a Trading convenience.
            deliver_receipt_dependency_v3(
                invocation,
                &mut buffers.data,
                prior_receipt,
                ReceiptDeliveryV3::ExactSuffix,
            )?;
            buffers.fill_metas()?;
            buffers.push_callee(core_program)?;
            let bump_seed = [bump];
            let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
            buffers
                .invoke(
                    core_program.key,
                    &[&[domain, release, market, role, context, digest, &bump_seed]],
                )
                .map_err(child_refused_v1)?;
            buffers.capture_return()?;
            if buffers.producer != *core_program.key {
                return Err(TradingSbfError::Transition.into());
            }
            validate_series_core_receipt_v2(
                request,
                request_digest,
                core_program.key,
                &buffers.accounts,
                &buffers.returned,
                prior_receipt,
            )?;
            Ok(CoreRouteExecutionV3::ReturnedReceipt(
                hashv(&[
                    CORE_EXECUTION_DIGEST_DOMAIN_V3,
                    &route_index.to_le_bytes(),
                    &invocation_index.to_le_bytes(),
                    &hash(&buffers.data).to_bytes(),
                    &buffers.returned,
                ])
                .to_bytes(),
            ))
        }
        PreparedCoreInvocationV3::PermitExpiry {
            invocation,
            request_digest,
        } => {
            deliver_receipt_dependency_v3(
                invocation,
                &mut buffers.data,
                prior_receipt,
                ReceiptDeliveryV3::VerifiedOnly,
            )?;
            buffers.fill_metas()?;
            // `fill_metas` grants coordinate zero a signer bit for the ordinary
            // child-authority convention. Permit expiry is permissionless and
            // coordinate zero is the System-owned permit candidate, so the
            // typed branch removes precisely that synthetic signer bit.
            let first = buffers.metas.first_mut().ok_or(TradingSbfError::Content)?;
            first.is_signer = false;
            buffers.push_callee(core_program)?;
            buffers
                .invoke(core_program.key, &[])
                .map_err(child_refused_v1)?;
            // Core permit expiry deliberately returns no DTO. A nonempty OR
            // merely present return channel is a substituted callee outcome;
            // it cannot be laundered into the common receipt bank.
            if solana_program::program::get_return_data().is_some() {
                return Err(TradingSbfError::Transition.into());
            }
            buffers.producer = *core_program.key;
            buffers.returned.clear();
            Ok(CoreRouteExecutionV3::ReceiptlessPermitExpiry(
                hashv(&[
                    CORE_RECEIPTLESS_EXPIRY_DIGEST_DOMAIN_V3,
                    core_program.key.as_ref(),
                    &route_index.to_le_bytes(),
                    &invocation_index.to_le_bytes(),
                    &request_digest,
                    &hash(&buffers.data).to_bytes(),
                ])
                .to_bytes(),
            ))
        }
        PreparedCoreInvocationV3::UnallocatedPermitExpiry {
            invocation,
            request_digest,
            authority_seeds,
            bump,
        } => {
            deliver_receipt_dependency_v3(
                invocation,
                &mut buffers.data,
                prior_receipt,
                ReceiptDeliveryV3::VerifiedOnly,
            )?;
            buffers.fill_metas()?;
            // `fill_metas` reserves coordinate zero for the ordinary caller
            // convention. This selected precommit route instead carries its
            // authenticated Trading caller as Core-local coordinate 25.
            buffers
                .metas
                .first_mut()
                .ok_or(TradingSbfError::Content)?
                .is_signer = false;
            buffers
                .metas
                .get_mut(usize::from(SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1))
                .ok_or(TradingSbfError::Content)?
                .is_signer = true;
            buffers.push_callee(core_program)?;
            let bump_seed = [bump];
            let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
            buffers
                .invoke(
                    core_program.key,
                    &[&[domain, release, market, role, context, digest, &bump_seed]],
                )
                .map_err(child_refused_v1)?;
            if solana_program::program::get_return_data().is_some() {
                return Err(TradingSbfError::Transition.into());
            }
            buffers.producer = *core_program.key;
            buffers.returned.clear();
            Ok(CoreRouteExecutionV3::ReceiptlessPermitExpiry(
                hashv(&[
                    CORE_PRECOMMIT_EXPIRY_DIGEST_DOMAIN_V1,
                    core_program.key.as_ref(),
                    &route_index.to_le_bytes(),
                    &invocation_index.to_le_bytes(),
                    &request_digest,
                    &hash(&buffers.data).to_bytes(),
                ])
                .to_bytes(),
            ))
        }
    }
}

enum PreparedCoreInvocationV3 {
    Series {
        invocation: ResolvedInvocationV3,
        request: SeriesCoreRequestV1,
        request_digest: [u8; 32],
        authority_seeds: CallerAuthoritySeedsV1,
        bump: u8,
    },
    PermitExpiry {
        invocation: ResolvedInvocationV3,
        request_digest: [u8; 32],
    },
    UnallocatedPermitExpiry {
        invocation: ResolvedInvocationV3,
        request_digest: [u8; 32],
        authority_seeds: CallerAuthoritySeedsV1,
        bump: u8,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthenticatedCoreRequestV3 {
    Series(SeriesCoreRequestV1),
    PermitExpiry(SeriesPermitExpiryRequestV1),
    UnallocatedPermitExpiry(SeriesUnallocatedPermitExpiryRequestV1, [u8; 32]),
}

impl PreparedCoreInvocationV3 {
    const fn bump(&self) -> u8 {
        match self {
            Self::Series { bump, .. } => *bump,
            // Receiptless permissionless routes consume a bump-list slot so
            // the two generic walks retain identical invocation ordinals, but
            // this byte is never used as a seed or authority.
            Self::PermitExpiry { .. } => 0,
            Self::UnallocatedPermitExpiry { bump, .. } => *bump,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare<'info>(
    program_id: &Pubkey,
    effect: EffectProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    invocation: ResolvedInvocationV3,
    successor_account_count: usize,
    borrowed_ranges: BorrowedRouteRangesV4<'_, '_, '_>,
    effect_accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    request_bank: &[u8],
    frame: &mut Vec<AccountInfo<'info>>,
    wire: &mut Vec<u8>,
    core_program: &AccountInfo<'_>,
    parent: CoreCompositionParentV3,
    preflighted_bump: PreflightedCallerBumpV4,
) -> Result<PreparedCoreInvocationV3, ProgramError> {
    if parent.release_set == [0; 32]
        || parent.market == [0; 32]
        || parent.trading_program != program_id.to_bytes()
        || !core_program.executable
        || core_program.is_signer
        || core_program.is_writable
        || successor_account_count != effect_accounts.len()
    {
        return Err(TradingSbfError::Content.into());
    }
    if invocation.role != FixedRole::Core
        || invocation.kind != RouteKindV3::Once
        || invocation.item.is_some()
        || invocation.repeated_item_count != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let request_bytes = request_bank
        .get(invocation.request_offset..request_end)
        .ok_or(TradingSbfError::Content)?;
    let authenticated_request =
        authenticate_core_request(request_bytes, parent, borrowed_ranges.family_request())?;
    let is_expiry = matches!(
        authenticated_request,
        AuthenticatedCoreRequestV3::PermitExpiry(_)
            | AuthenticatedCoreRequestV3::UnallocatedPermitExpiry(_, _)
    );
    let range_count = borrowed_ranges.count()?;
    let legacy_witness = if range_count == 0 {
        invocation
            .borrowed_witness
            .map(|witness| {
                witness
                    .slice(borrowed_ranges.family_request())
                    .map_err(|_| ProgramError::from(TradingSbfError::Content))
            })
            .transpose()?
    } else {
        if invocation.borrowed_witness.is_some() {
            return Err(TradingSbfError::Content.into());
        }
        None
    };
    if is_expiry {
        let expected_accounts = if matches!(
            authenticated_request,
            AuthenticatedCoreRequestV3::UnallocatedPermitExpiry(_, _)
        ) {
            SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1
        } else {
            SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1
        };
        // THE RANGE COUNT IS A PER-TEMPLATE CONSTANT, NOT THE LITERAL `1`.
        //
        // The fifth author of the fact `97ce7a748` moved four authors of, and
        // the one its sweep missed. An expiry route borrows the family
        // request's canonical proof exactly once -- and a `BorrowedRangeV4` is
        // canonically nonempty, so the honest declaration for a Template whose
        // `series_proof_count_v3` is ZERO is an EMPTY range table, which is
        // what `series_expire_borrowed_range_count_v5` emits. Pinning this to
        // `1` therefore refused every single-occurrence Series -- the only
        // shape this tree has ever founded -- with `Content`.
        //
        // The width the request arrives at is already authenticated against
        // the RequestProfile, which pins `series_action_request_bytes_v3` for
        // the Template this release serves, so "does this Template have a
        // proof" is readable here without a second copy of the rule and
        // without decoding the request again.
        let expected_ranges =
            u16::from(borrowed_ranges.family_request().len() > SERIES_ACTION_HEADER_BYTES_V3);
        if range_count != expected_ranges
            || legacy_witness.is_some()
            || invocation_index != 0
            || route_index.checked_add(1) != Some(effect.route_count())
            || invocation.fixed_account_count != expected_accounts
            || !invocation.receipt_dependencies.is_empty()
            || has_receipt_dependent(effect, route_index, invocation_index)?
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    let borrowed_bytes = if range_count == 0 {
        legacy_witness.map_or(0, <[u8]>::len)
    } else {
        borrowed_ranges.byte_len()?
    };
    wire.clear();
    wire.try_reserve_exact(
        request_bytes
            .len()
            .checked_add(borrowed_bytes)
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::HeapExhausted)?;
    wire.extend_from_slice(request_bytes);
    if range_count == 0 {
        wire.extend_from_slice(legacy_witness.unwrap_or(&[]));
    } else {
        borrowed_ranges.append_to(wire)?;
    }
    gather_invocation_accounts(frame, invocation, effect_accounts)?;
    let request_digest = hash(request_bytes).to_bytes();
    if matches!(
        authenticated_request,
        AuthenticatedCoreRequestV3::PermitExpiry(_)
    ) {
        if frame.len() != usize::from(SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1)
            || frame.iter().any(|account| account.is_signer)
        {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(PreparedCoreInvocationV3::PermitExpiry {
            invocation,
            request_digest,
        });
    }

    if let AuthenticatedCoreRequestV3::UnallocatedPermitExpiry(_, ticket_context) =
        authenticated_request
    {
        if frame.len() != usize::from(SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1)
            || frame.iter().any(|account| account.is_signer)
        {
            return Err(TradingSbfError::Content.into());
        }
        for coordinate in SERIES_PRECOMMIT_REPLAY_COORDINATES_V1 {
            let account = frame.get_mut(coordinate).ok_or(TradingSbfError::Content)?;
            *account = series_precommit_replay_observation_v1(account.clone())?;
        }
        let caller = frame
            .get(usize::from(SERIES_PERMIT_EXPIRY_ACCOUNT_COUNT_V1))
            .ok_or(TradingSbfError::Content)?;
        let authority_seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(parent.release_set).map_err(|_| TradingSbfError::Content)?,
            parent.market,
            ExecutionRoleV1::Trading,
            ticket_context,
            request_digest,
        )
        .map_err(|_| TradingSbfError::Content)?;
        let (expected_authority, bump) =
            child_caller_authority_v4(&authority_seeds, program_id, preflighted_bump)?;
        authenticate_precommit_caller_v1(caller, &expected_authority)?;
        return Ok(PreparedCoreInvocationV3::UnallocatedPermitExpiry {
            invocation,
            request_digest,
            authority_seeds,
            bump,
        });
    }

    let AuthenticatedCoreRequestV3::Series(request) = authenticated_request else {
        return Err(TradingSbfError::Content.into());
    };
    project_series_core_child_privileges_v1(frame)?;
    let ticket = ticket_content_id_from_core_frame_v2(frame)?;
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(parent.release_set).map_err(|_| TradingSbfError::Content)?,
        parent.market,
        ExecutionRoleV1::Trading,
        ticket,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected_authority, bump) =
        child_caller_authority_v4(&authority_seeds, program_id, preflighted_bump)?;
    if frame
        .first()
        .is_none_or(|account| account.key != &expected_authority)
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(PreparedCoreInvocationV3::Series {
        invocation,
        request,
        request_digest,
        authority_seeds,
        bump,
    })
}

/// Replace the authenticated physical-union view with Core's exact child view.
///
/// The outer AccountProfile cannot encode per-route privilege reductions for
/// an alias: it authenticates one physical representative carrying the union
/// needed across Lock, Claims, Found, and Open. Core's native parsers reject
/// that union for their readonly suffixes, so the Core-owned frame contract is
/// applied after gathering and before child metas are emitted. This can only
/// remove writable/signer bits; a native writable bit absent from the parent
/// still refuses before CPI.
fn project_series_core_child_privileges_v1(
    frame: &mut [AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let width = frame.len();
    for (local, account) in frame.iter_mut().enumerate() {
        let writable = dclutch_market::series_core_child_writable_v1(width, local)
            .ok_or(TradingSbfError::Content)?;
        if writable && !account.is_writable {
            return Err(TradingSbfError::Content.into());
        }
        account.is_writable = writable;
        account.is_signer = false;
    }
    Ok(())
}

/// Derive the caller-PDA context from the finalized Ticket record Core reads.
///
/// The request's `ticket` coordinate is the mutable replay PDA.  Core's
/// `SeriesConsumeAccounts::authenticate_trading_caller` instead derives the
/// same context from the distinct finalized Ticket raw record at local 45.
fn ticket_content_id_from_core_frame_v2(
    frame: &[AccountInfo<'_>],
) -> Result<[u8; 32], ProgramError> {
    let ticket_raw_local = if frame.len() == SERIES_OPEN_ACCOUNT_COUNT_V1 {
        21
    } else {
        FOUND_ACCOUNT_COUNT_V3 + 8
    };
    let ticket_raw = frame
        .get(ticket_raw_local)
        .ok_or(TradingSbfError::Content)?;
    let bytes = ticket_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    Ok(ticket_content_id(&bytes)
        .map_err(|_| TradingSbfError::Content)?
        .to_bytes())
}

/// Authenticate the action-specific Core return DTO against the child frame.
///
/// Consume Found returns the 328-byte V2 receipt.  Its funding span, permit,
/// and post-resource digest are reconstructed from the post-CPI frame rather
/// than accepted as receipt mirrors.  Open remains V1 while its native
/// post-resource preimage is moved into this composition (tracked separately
/// below); retaining the V1 shape here avoids accepting a Found V2 as Open.
fn validate_series_core_receipt_v2(
    request: SeriesCoreRequestV1,
    request_digest: [u8; 32],
    core_program: &Pubkey,
    accounts_with_callee: &[AccountInfo<'_>],
    returned: &[u8],
    prior_receipt: Option<&[u8]>,
) -> Result<(), ProgramError> {
    let width = accounts_with_callee
        .len()
        .checked_sub(1)
        .ok_or(TradingSbfError::ChildReceipt)?;
    let accounts = accounts_with_callee
        .get(..width)
        .ok_or(TradingSbfError::ChildReceipt)?;
    let expected_core =
        Identity::new(core_program.to_bytes()).map_err(|_| TradingSbfError::Transition)?;
    let expected_request =
        Identity::new(request_digest).map_err(|_| TradingSbfError::Transition)?;
    if width == SERIES_OPEN_ACCOUNT_COUNT_V1 {
        let claims_receipt = prior_receipt.ok_or(TradingSbfError::ChildReceipt)?;
        let market = accounts.get(1).ok_or(TradingSbfError::ChildReceipt)?;
        let (root_candidate, ticket_candidate) = predicted_open_replay_bytes_v1(accounts, request)?;
        let market_bytes = market
            .try_borrow_data()
            .map_err(|_| TradingSbfError::ChildReceipt)?;
        let post = hashv(&[
            SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1,
            &market_bytes,
            claims_receipt,
            &root_candidate,
            &ticket_candidate,
        ])
        .to_bytes();
        drop(market_bytes);
        let receipt =
            SeriesCoreAckV1::decode(returned).map_err(|_| TradingSbfError::ChildReceipt)?;
        receipt
            .validate_for(
                request,
                expected_core,
                expected_request,
                Identity::new(post).map_err(|_| TradingSbfError::ChildReceipt)?,
            )
            .map_err(|_| TradingSbfError::ChildReceipt)?;
        return Ok(());
    }

    let fixed = SERIES_CONSUME_FOUND_SUFFIX_START_V1
        .checked_add(SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1)
        .ok_or(TradingSbfError::ChildReceipt)?;
    let funding_count = width
        .checked_sub(fixed)
        .ok_or(TradingSbfError::ChildReceipt)?;
    if !(dclutch_market::SERIES_CONSUME_FOUND_MINIMUM_FUNDING_COUNT_V1
        ..=dclutch_market::SERIES_CONSUME_FOUND_MAXIMUM_FUNDING_COUNT_V1)
        .contains(&funding_count)
    {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    let permit = accounts
        .get(SERIES_CONSUME_FOUND_SUFFIX_START_V1 + funding_count)
        .ok_or(TradingSbfError::ChildReceipt)?;
    let market = accounts.get(1).ok_or(TradingSbfError::ChildReceipt)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(funding_count)
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    for account in accounts
        .get(
            SERIES_CONSUME_FOUND_SUFFIX_START_V1
                ..SERIES_CONSUME_FOUND_SUFFIX_START_V1 + funding_count,
        )
        .ok_or(TradingSbfError::ChildReceipt)?
    {
        keys.push(
            AccountKeyV3::new(account.key.to_bytes()).map_err(|_| TradingSbfError::ChildReceipt)?,
        );
    }
    let list = funding_list_id(&keys).map_err(|_| TradingSbfError::ChildReceipt)?;
    let market_bytes = market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let permit_bytes = permit
        .try_borrow_data()
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let post = hashv(&[
        SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &market_bytes,
        &permit_bytes,
    ])
    .to_bytes();
    drop(market_bytes);
    drop(permit_bytes);
    let receipt =
        SeriesCoreFoundAckV2::decode(returned).map_err(|_| TradingSbfError::ChildReceipt)?;
    receipt
        .validate_for(
            request,
            expected_core,
            Identity::new(permit.key.to_bytes()).map_err(|_| TradingSbfError::ChildReceipt)?,
            expected_request,
            u8::try_from(funding_count).map_err(|_| TradingSbfError::ChildReceipt)?,
            Identity::new(list.to_bytes()).map_err(|_| TradingSbfError::ChildReceipt)?,
            Identity::new(post).map_err(|_| TradingSbfError::ChildReceipt)?,
        )
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    Ok(())
}

fn predicted_open_replay_bytes_v1(
    accounts: &[AccountInfo<'_>],
    request: SeriesCoreRequestV1,
) -> Result<
    (
        [u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3],
        [u8; SERIES_TICKET_STATE_BYTES_V3],
    ),
    ProgramError,
> {
    let root = accounts.get(15).ok_or(TradingSbfError::ChildReceipt)?;
    let ticket_state = accounts.get(16).ok_or(TradingSbfError::ChildReceipt)?;
    let template_raw = accounts.get(17).ok_or(TradingSbfError::ChildReceipt)?;
    let template_bytes = template_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let occurrence_count = TemplateV3::decode(&template_bytes)
        .map_err(|_| TradingSbfError::ChildReceipt)?
        .occurrence_count();
    drop(template_bytes);
    let root_bytes = root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let current_root = SeriesStateV3::decode(
        root_bytes
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(TradingSbfError::ChildReceipt)?,
        occurrence_count,
    )
    .map_err(|_| TradingSbfError::ChildReceipt)?;
    let next_root = current_root
        .settle_current(request.expected_series_revision(), occurrence_count)
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let mut root_candidate = [0; CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3];
    root_candidate.copy_from_slice(&root_bytes);
    root_candidate[CAPABILITY_ROOT_HEADER_BYTES_V1..].copy_from_slice(
        &next_root
            .encode(occurrence_count)
            .map_err(|_| TradingSbfError::ChildReceipt)?,
    );
    drop(root_bytes);
    let ticket_bytes = ticket_state
        .try_borrow_data()
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let current_ticket =
        TicketStateV3::decode(&ticket_bytes).map_err(|_| TradingSbfError::ChildReceipt)?;
    let next_ticket = current_ticket
        .settle(request.expected_ticket_revision(), TicketPhaseV3::Consumed)
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    Ok((root_candidate, next_ticket.encode()))
}

fn authenticate_precommit_caller_v1(
    caller: &AccountInfo<'_>,
    expected_authority: &Pubkey,
) -> Result<(), ProgramError> {
    if caller.key != expected_authority {
        return Err(TradingSbfError::SeriesPrecommitCallerKey.into());
    }
    require_series_precommit_caller_shape_v1(caller)
}

/// Validate the precommit caller's shape before common profile projection can
/// flatten these distinct account defects into a generic content refusal.
pub(crate) fn require_series_precommit_caller_shape_v1(
    caller: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if caller.is_signer || caller.is_writable || caller.executable {
        return Err(TradingSbfError::SeriesPrecommitCallerPrivileges.into());
    }
    if caller.owner != &system_program::ID {
        return Err(TradingSbfError::SeriesPrecommitCallerOwner.into());
    }
    if !caller.data_is_empty() {
        return Err(TradingSbfError::SeriesPrecommitCallerData.into());
    }
    Ok(())
}

fn authenticate_core_request(
    request_bytes: &[u8],
    parent: CoreCompositionParentV3,
    family_request: &[u8],
) -> Result<AuthenticatedCoreRequestV3, ProgramError> {
    if request_bytes.get(..SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1.len())
        == Some(SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1.as_slice())
    {
        let request = SeriesUnallocatedPermitExpiryRequestV1::decode(request_bytes)
            .map_err(|_| TradingSbfError::Content)?;
        let family =
            SeriesActionRequestV3::decode(family_request).map_err(|_| TradingSbfError::Content)?;
        let ticket = family.ticket().ok_or(TradingSbfError::Content)?.to_bytes();
        if family.action() != SeriesActionV3::Expire
            || request.expected_series_revision() != family.expected_series_revision()
            || request.expected_ticket_revision() != family.expected_ticket_revision()
            || parent.release_set == [0; 32]
            || parent.market == [0; 32]
            || parent.generation == 0
            || parent.trading_program == [0; 32]
        {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(AuthenticatedCoreRequestV3::UnallocatedPermitExpiry(
            request, ticket,
        ));
    }
    if request_bytes.get(..8) == Some(SERIES_CORE_REQUEST_MAGIC_V1.as_slice()) {
        let request =
            SeriesCoreRequestV1::decode(request_bytes).map_err(|_| TradingSbfError::Content)?;
        if request.action() != SeriesCoreActionV1::Consume
            || request.release_set().to_bytes() != parent.release_set
            || request
                .market()
                .is_none_or(|market| market.to_bytes() != parent.market)
            || request.market_generation() != Some(parent.generation)
        {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(AuthenticatedCoreRequestV3::Series(request));
    }
    if request_bytes.get(..8) == Some(SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1.as_slice()) {
        let request = SeriesPermitExpiryRequestV1::decode(request_bytes)
            .map_err(|_| TradingSbfError::Content)?;
        let intent = request.permit().intent();
        if intent.release_set().to_bytes() != parent.release_set
            || intent.market().to_bytes() != parent.market
            || intent.generation() != parent.generation
            || intent.trading_program().to_bytes() != parent.trading_program
        {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(AuthenticatedCoreRequestV3::PermitExpiry(request));
    }
    Err(TradingSbfError::UnsupportedContent.into())
}

fn has_receipt_dependent(
    effect: EffectProgramV3<'_>,
    producer_route: u16,
    producer_invocation: u32,
) -> Result<bool, ProgramError> {
    let mut route = 0_u16;
    while route < effect.route_count() {
        let selected = effect.route(route).map_err(|_| TradingSbfError::Content)?;
        let mut ordinal = 0_u16;
        while ordinal < selected.receipt_dependency_count() {
            let dependency = effect
                .route_receipt_dependency(route, ordinal)
                .map_err(|_| TradingSbfError::Content)?;
            // A descriptor dependency names a route. `Each` producers expand
            // to item-matching invocation indices; permit expiry is a `Once`
            // route, so any reference to its route necessarily depends on
            // invocation zero. Keep the explicit argument in the conjunction
            // so a future non-Once receiptless route cannot inherit this rule.
            if dependency.producer_role() == FixedRole::Core
                && dependency.producer_route() == producer_route
                && producer_invocation == 0
            {
                return Ok(true);
            }
            ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(false)
}

/// Gather this invocation's account window into a caller-owned buffer.
fn gather_invocation_accounts<'info>(
    output: &mut Vec<AccountInfo<'info>>,
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
) -> Result<(), ProgramError> {
    let start = usize::from(invocation.fixed_account_start);
    let end = start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    accounts.reserve_invocation_frame(output, invocation)?;
    accounts.extend_window(
        output,
        start,
        end.checked_sub(start).ok_or(TradingSbfError::Content)?,
    )
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::{boxed::Box, vec, vec::Vec};

    use dclutch_trading::series::request::encode_series_action_header_v3;
    use dclutch_vm::effect::v3::{
        RouteReceiptDependencyV3,
        encode::{
            AccountCoordinateV3, EffectGeometryV3, EffectInstructionV3, RouteInputV3,
            ScalarCoordinateV3, encode_effect_program_v4_atomic,
        },
    };

    use super::*;

    /// A physical representative arrives from the authenticated outer profile
    /// with its cross-route union privileges.  Series Core must narrow that
    /// view before `fill_metas` turns it into a child instruction.
    fn union_account() -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(Vec::<u8>::new().into_boxed_slice());
        AccountInfo::new(key, true, true, lamports, data, owner, false)
    }

    fn union_frame(width: usize) -> Vec<AccountInfo<'static>> {
        (0..width).map(|_| union_account()).collect()
    }

    fn id(value: u8) -> Identity {
        Identity::new([value; 32]).expect("nonzero identity")
    }

    fn receipt_account(data: impl Into<Vec<u8>>) -> AccountInfo<'static> {
        receipt_account_at(Pubkey::new_unique(), data)
    }

    fn receipt_account_at(key: Pubkey, data: impl Into<Vec<u8>>) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(key));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(data.into().into_boxed_slice());
        AccountInfo::new(key, false, false, lamports, data, owner, false)
    }

    fn consume_request(ticket_replay: Pubkey) -> SeriesCoreRequestV1 {
        SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Consume,
            id(1),
            id(2),
            Identity::new(ticket_replay.to_bytes()).expect("replay PDA"),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            9,
            1,
            0,
            10,
            11,
            12,
            13,
        )
        .expect("Series Consume request")
    }

    fn child_receipt() -> ProgramError {
        TradingSbfError::ChildReceipt.into()
    }

    fn found_frame(funding_count: usize) -> Vec<AccountInfo<'static>> {
        let width = SERIES_CONSUME_FOUND_SUFFIX_START_V1
            + SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1
            + funding_count;
        let mut frame = (0..width)
            .map(|local| receipt_account(vec![local as u8, 0xA5]))
            .collect::<Vec<_>>();
        let ticket_raw = FOUND_ACCOUNT_COUNT_V3 + 8;
        frame[ticket_raw] =
            receipt_account(dclutch_trading::series::generated::SERIES_EXAMPLE_TICKET_V3.to_vec());
        frame.push(receipt_account(Vec::new()));
        frame
    }

    fn found_return(
        request: SeriesCoreRequestV1,
        request_digest: [u8; 32],
        core_program: Pubkey,
        frame: &[AccountInfo<'_>],
    ) -> Vec<u8> {
        let accounts = frame
            .get(..frame.len() - 1)
            .expect("callee appended to Found frame");
        let funding_count = accounts.len()
            - SERIES_CONSUME_FOUND_SUFFIX_START_V1
            - SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1;
        let permit = &accounts[SERIES_CONSUME_FOUND_SUFFIX_START_V1 + funding_count];
        let funding_keys = accounts[SERIES_CONSUME_FOUND_SUFFIX_START_V1
            ..SERIES_CONSUME_FOUND_SUFFIX_START_V1 + funding_count]
            .iter()
            .map(|account| AccountKeyV3::new(account.key.to_bytes()).expect("funding key"))
            .collect::<Vec<_>>();
        let funding_list = funding_list_id(&funding_keys).expect("canonical funding list");
        let market = &accounts[1];
        let market_data = market.try_borrow_data().expect("market data");
        let permit_data = permit.try_borrow_data().expect("permit data");
        let post = hashv(&[
            SERIES_FOUND_POST_RESOURCE_DIGEST_DOMAIN_V1,
            &market_data,
            &permit_data,
        ])
        .to_bytes();
        SeriesCoreFoundAckV2::new(
            request,
            Identity::new(core_program.to_bytes()).expect("Core program"),
            Identity::new(permit.key.to_bytes()).expect("permit"),
            Identity::new(request_digest).expect("request digest"),
            u8::try_from(funding_count).expect("bounded funding count"),
            Identity::new(funding_list.to_bytes()).expect("funding list"),
            Identity::new(post).expect("post digest"),
        )
        .expect("Found V2 receipt")
        .encode()
        .expect("Found V2 bytes")
        .to_vec()
    }

    fn open_frame(ticket_record: &[u8], ticket_replay: Pubkey) -> Vec<AccountInfo<'static>> {
        let mut frame = (0..SERIES_OPEN_ACCOUNT_COUNT_V1)
            .map(|local| receipt_account(vec![0xC0, local as u8]))
            .collect::<Vec<_>>();
        let template = dclutch_trading::series::generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let template_value = TemplateV3::decode(&template).expect("canonical Template");
        let ticket_id = ticket_content_id(ticket_record).expect("canonical Ticket record");
        let current_root = SeriesStateV3::new(0)
            .prepare_ticket(0)
            .expect("canonical prepared SeriesState");
        let mut root = vec![0_u8; CAPABILITY_ROOT_HEADER_BYTES_V1];
        root.extend_from_slice(
            &current_root
                .encode(template_value.occurrence_count())
                .expect("canonical SeriesState bytes"),
        );
        frame[15] = receipt_account(root);
        frame[17] = receipt_account(template.to_vec());
        frame[21] = receipt_account(ticket_record.to_vec());
        frame[16] = receipt_account_at(
            ticket_replay,
            TicketStateV3::prepared(ticket_id).encode().to_vec(),
        );
        frame.push(receipt_account(Vec::new()));
        frame
    }

    /// Build an Open receipt from the native replay kernels rather than the
    /// Trading-side verifier's candidate helper.  This keeps the test capable
    /// of detecting a self-consistent but wrong verifier prediction.
    fn native_open_replay_candidates(
        accounts: &[AccountInfo<'_>],
        request: SeriesCoreRequestV1,
    ) -> (
        [u8; CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3],
        [u8; SERIES_TICKET_STATE_BYTES_V3],
    ) {
        let occurrence_count = TemplateV3::decode(
            &accounts[17]
                .try_borrow_data()
                .expect("canonical Template bytes"),
        )
        .expect("canonical Template")
        .occurrence_count();
        let root_data = accounts[15].try_borrow_data().expect("root data");
        let current_root = SeriesStateV3::decode(
            &root_data[CAPABILITY_ROOT_HEADER_BYTES_V1..],
            occurrence_count,
        )
        .expect("canonical SeriesState");
        let next_root = current_root
            .settle_current(request.expected_series_revision(), occurrence_count)
            .expect("next SeriesState");
        let mut root_candidate = [0; CAPABILITY_ROOT_HEADER_BYTES_V1 + SERIES_STATE_BYTES_V3];
        root_candidate.copy_from_slice(&root_data);
        root_candidate[CAPABILITY_ROOT_HEADER_BYTES_V1..].copy_from_slice(
            &next_root
                .encode(occurrence_count)
                .expect("next SeriesState bytes"),
        );
        drop(root_data);
        let current_ticket =
            TicketStateV3::decode(&accounts[16].try_borrow_data().expect("ticket replay data"))
                .expect("canonical TicketState");
        let next_ticket = current_ticket
            .settle(request.expected_ticket_revision(), TicketPhaseV3::Consumed)
            .expect("next TicketState");
        (root_candidate, next_ticket.encode())
    }

    fn open_return_from_native_poststate(
        request: SeriesCoreRequestV1,
        request_digest: [u8; 32],
        core_program: Pubkey,
        frame: &[AccountInfo<'_>],
        claims_receipt: &[u8],
        root_candidate: &[u8],
        ticket_candidate: &[u8],
    ) -> Vec<u8> {
        let accounts = frame
            .get(..frame.len() - 1)
            .expect("callee appended to Open frame");
        let market = accounts[1].try_borrow_data().expect("market data");
        let post = hashv(&[
            SERIES_OPEN_POST_RESOURCE_DIGEST_DOMAIN_V1,
            &market,
            claims_receipt,
            root_candidate,
            ticket_candidate,
        ])
        .to_bytes();
        SeriesCoreAckV1::new(
            request,
            Identity::new(core_program.to_bytes()).expect("Core program"),
            Identity::new(request_digest).expect("request digest"),
            Identity::new(post).expect("post digest"),
        )
        .encode()
        .expect("Open V1 bytes")
        .to_vec()
    }

    fn expiry_request() -> SeriesPermitExpiryRequestV1 {
        let intent = dclutch_market::FoundingIntentV5::new(
            255,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            id(11),
            id(12),
            id(13),
            id(14),
            id(15),
            8,
            1,
            1,
            100,
            4,
            1,
        )
        .expect("founding intent");
        SeriesPermitExpiryRequestV1::new(
            dclutch_market::SeriesFoundingPermitV1::new(intent, id(16), id(17)).expect("permit"),
        )
    }

    fn parent() -> CoreCompositionParentV3 {
        let intent = expiry_request().permit().intent();
        CoreCompositionParentV3 {
            release_set: intent.release_set().to_bytes(),
            market: intent.market().to_bytes(),
            generation: intent.generation(),
            trading_program: intent.trading_program().to_bytes(),
        }
    }

    fn precommit_request() -> (SeriesUnallocatedPermitExpiryRequestV1, Vec<u8>, Vec<u8>) {
        let request = SeriesUnallocatedPermitExpiryRequestV1::new(3, 1);
        let mut family = encode_series_action_header_v3(
            SeriesActionV3::Expire,
            ContentId::new(id(18).to_bytes()).expect("template"),
            Some(ContentId::new([0x51; 32]).expect("occurrence")),
            Some(ContentId::new(id(6).to_bytes()).expect("Ticket record")),
            request.expected_series_revision(),
            request.expected_ticket_revision(),
            1,
        )
        .expect("Series family")
        .to_vec();
        family.extend_from_slice(&[0x52; 32]);
        (request, request.encode().to_vec(), family)
    }

    #[test]
    fn core_packets_are_explicitly_typed() {
        assert_eq!(SERIES_CORE_REQUEST_MAGIC_V1, *b"DCLTCSR1");
        assert_eq!(SERIES_PERMIT_EXPIRY_REQUEST_MAGIC_V1, *b"DCLTSFX1");
        assert_eq!(
            SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_MAGIC_V1,
            *b"DCLSUPE1"
        );
        assert_ne!(
            CORE_EXECUTION_DIGEST_DOMAIN_V3,
            b"dclutch:hot-custody-receipt:v3"
        );
        assert_ne!(
            CORE_RECEIPTLESS_EXPIRY_DIGEST_DOMAIN_V3,
            CORE_EXECUTION_DIGEST_DOMAIN_V3
        );
    }

    #[test]
    fn series_found_v2_receipts_bind_one_and_sixteen_canonical_funding_states() {
        let core_program = Pubkey::new_unique();
        for funding_count in [
            dclutch_market::SERIES_CONSUME_FOUND_MINIMUM_FUNDING_COUNT_V1,
            dclutch_market::SERIES_CONSUME_FOUND_MAXIMUM_FUNDING_COUNT_V1,
        ] {
            let ticket_replay = Pubkey::new_unique();
            let request = consume_request(ticket_replay);
            let request_digest = hash(&request.encode().expect("request bytes")).to_bytes();
            let frame = found_frame(funding_count);
            let returned = found_return(request, request_digest, core_program, &frame);
            assert_eq!(
                validate_series_core_receipt_v2(
                    request,
                    request_digest,
                    &core_program,
                    &frame,
                    &returned,
                    None,
                ),
                Ok(())
            );
        }
    }

    #[test]
    fn series_found_v2_receipt_refuses_substituted_market_permit_list_count_and_request() {
        let core_program = Pubkey::new_unique();
        let request = consume_request(Pubkey::new_unique());
        let request_digest = hash(&request.encode().expect("request bytes")).to_bytes();

        let frame = found_frame(2);
        let returned = found_return(request, request_digest, core_program, &frame);
        let mut market = frame[1].try_borrow_mut_data().expect("market data");
        market[0] ^= 0xEE;
        drop(market);
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &returned,
                None,
            ),
            Err(child_receipt())
        );

        let frame = found_frame(2);
        let returned = found_return(request, request_digest, core_program, &frame);
        let permit = SERIES_CONSUME_FOUND_SUFFIX_START_V1 + 2;
        let mut permit_data = frame[permit].try_borrow_mut_data().expect("permit data");
        permit_data[0] ^= 0xEF;
        drop(permit_data);
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &returned,
                None,
            ),
            Err(child_receipt())
        );

        let mut frame = found_frame(2);
        let returned = found_return(request, request_digest, core_program, &frame);
        frame.swap(
            SERIES_CONSUME_FOUND_SUFFIX_START_V1,
            SERIES_CONSUME_FOUND_SUFFIX_START_V1 + 1,
        );
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &returned,
                None,
            ),
            Err(child_receipt())
        );

        let frame = found_frame(1);
        let mut count_substitution = found_return(request, request_digest, core_program, &frame);
        let count_offset = dclutch_market::SERIES_CORE_FOUND_ACK_FUNDING_COUNT_OFFSET_V2;
        assert_eq!(count_substitution[count_offset], 1);
        count_substitution[count_offset] = 2;
        assert_eq!(
            SeriesCoreFoundAckV2::decode(&count_substitution)
                .expect("count-only substituted Found V2")
                .funding_count(),
            2
        );
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &count_substitution,
                None,
            ),
            Err(child_receipt())
        );

        let frame = found_frame(1);
        let returned = found_return(request, request_digest, core_program, &frame);
        let changed_request = SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Consume,
            id(1),
            id(2),
            request.ticket().expect("ticket"),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            9,
            3,
            0,
            10,
            11,
            12,
            13,
        )
        .expect("substituted request");
        assert_eq!(
            validate_series_core_receipt_v2(
                changed_request,
                request_digest,
                &core_program,
                &frame,
                &returned,
                None,
            ),
            Err(child_receipt())
        );
    }

    #[test]
    fn series_found_v1_receipt_cannot_cross_the_v2_found_boundary() {
        let core_program = Pubkey::new_unique();
        let request = consume_request(Pubkey::new_unique());
        let request_digest = hash(&request.encode().expect("request bytes")).to_bytes();
        let frame = found_frame(1);
        let obsolete = SeriesCoreAckV1::new(
            request,
            Identity::new(core_program.to_bytes()).expect("Core program"),
            Identity::new(request_digest).expect("request digest"),
            id(9),
        )
        .encode()
        .expect("legacy V1 bytes");
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &obsolete,
                None,
            ),
            Err(child_receipt())
        );
    }

    #[test]
    fn series_open_v1_receipt_binds_claims_and_predicted_future_replay() {
        let ticket_record = dclutch_trading::series::generated::SERIES_EXAMPLE_TICKET_V3;
        let ticket_replay = Pubkey::new_unique();
        let request = consume_request(ticket_replay);
        let request_digest = hash(&request.encode().expect("request bytes")).to_bytes();
        let core_program = Pubkey::new_unique();
        let claims_receipt = [0x51, 0x52, 0x53];
        let frame = open_frame(&ticket_record, ticket_replay);
        let accounts = frame.get(..frame.len() - 1).expect("Open accounts");
        let (root_future, ticket_future) = native_open_replay_candidates(accounts, request);
        let current_root = accounts[15].try_borrow_data().expect("root data").to_vec();
        let current_ticket = accounts[16]
            .try_borrow_data()
            .expect("ticket replay data")
            .to_vec();
        assert_ne!(current_root.as_slice(), root_future.as_slice());
        assert_ne!(current_ticket.as_slice(), ticket_future.as_slice());
        let returned = open_return_from_native_poststate(
            request,
            request_digest,
            core_program,
            &frame,
            &claims_receipt,
            &root_future,
            &ticket_future,
        );
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &returned,
                Some(&claims_receipt),
            ),
            Ok(())
        );
        let current_return = open_return_from_native_poststate(
            request,
            request_digest,
            core_program,
            &frame,
            &claims_receipt,
            &current_root,
            &current_ticket,
        );
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &current_return,
                Some(&claims_receipt),
            ),
            Err(child_receipt())
        );
        assert_eq!(
            validate_series_core_receipt_v2(
                request,
                request_digest,
                &core_program,
                &frame,
                &returned,
                Some(&[0x51, 0x52, 0x54]),
            ),
            Err(child_receipt())
        );
    }

    #[test]
    fn ticket_context_comes_from_ticket_record_not_replay_pda_at_native_offsets() {
        let ticket_record = dclutch_trading::series::generated::SERIES_EXAMPLE_TICKET_V3;
        let expected = ticket_content_id(&ticket_record)
            .expect("canonical Ticket record")
            .to_bytes();
        for (mut frame, replay_local) in [
            (found_frame(1), FOUND_ACCOUNT_COUNT_V3 + 3),
            (open_frame(&ticket_record, Pubkey::new_unique()), 16),
        ] {
            let replay = Pubkey::new_unique();
            let replay_data = frame[replay_local]
                .try_borrow_data()
                .expect("replay data")
                .to_vec();
            frame[replay_local] = receipt_account_at(replay, replay_data);
            assert_ne!(replay.to_bytes(), expected);
            assert_eq!(
                consume_request(replay)
                    .ticket()
                    .expect("replay PDA")
                    .to_bytes(),
                replay.to_bytes()
            );
            assert_eq!(
                ticket_content_id_from_core_frame_v2(
                    frame.get(..frame.len() - 1).expect("callee appended"),
                ),
                Ok(expected)
            );
        }
    }

    #[test]
    fn series_found_child_projection_uses_the_native_dynamic_suffix() {
        let funding_count = dclutch_market::SERIES_CONSUME_FOUND_MINIMUM_FUNDING_COUNT_V1;
        let width = dclutch_market::SERIES_CONSUME_FOUND_SUFFIX_START_V1
            + dclutch_market::SERIES_CONSUME_FOUND_SUFFIX_ACCOUNT_COUNT_V1
            + funding_count;
        let permit = dclutch_market::SERIES_CONSUME_FOUND_SUFFIX_START_V1 + funding_count;
        let mut frame = union_frame(width);

        // Aggregate, position, and admission are writable physical
        // representatives for Claims Founding, but Core Found only observes
        // them in its suffix.
        for suffix_local in 9..=11 {
            assert!(frame[permit + suffix_local].is_writable);
        }
        assert_eq!(project_series_core_child_privileges_v1(&mut frame), Ok(()));

        assert!(frame[permit].is_writable);
        for local in dclutch_market::SERIES_CONSUME_FOUND_SUFFIX_START_V1..permit {
            assert!(!frame[local].is_writable, "FundingState local {local}");
        }
        for suffix_local in 9..=11 {
            assert!(!frame[permit + suffix_local].is_writable);
        }
        assert!(!frame[0].is_signer);

        let mut buffers = ChildInvocationBuffersV3::new();
        buffers.accounts = frame;
        buffers.fill_metas().expect("Core child metas");
        assert!(buffers.metas[0].is_signer);
        assert!(buffers.metas[permit].is_writable);
        for suffix_local in 9..=11 {
            assert!(!buffers.metas[permit + suffix_local].is_writable);
        }
    }

    #[test]
    fn series_open_child_projection_refuses_missing_parent_write() {
        let mut accepted = union_frame(dclutch_market::SERIES_OPEN_ACCOUNT_COUNT_V1);
        assert_eq!(
            project_series_core_child_privileges_v1(&mut accepted),
            Ok(())
        );
        for (local, account) in accepted.iter().enumerate() {
            assert_eq!(account.is_writable, matches!(local, 1..=3));
            assert!(!account.is_signer);
        }
        let mut buffers = ChildInvocationBuffersV3::new();
        buffers.accounts = accepted;
        buffers.fill_metas().expect("Core child metas");
        assert!(buffers.metas[0].is_signer);
        for (local, meta) in buffers.metas.iter().enumerate() {
            assert_eq!(meta.is_writable, matches!(local, 1..=3));
        }

        let mut hostile = union_frame(dclutch_market::SERIES_OPEN_ACCOUNT_COUNT_V1);
        hostile[1].is_writable = false;
        assert_eq!(
            project_series_core_child_privileges_v1(&mut hostile),
            Err(ProgramError::from(TradingSbfError::Content))
        );
    }

    #[test]
    fn permit_expiry_authenticates_every_parent_coordinate_before_buffers() {
        let bytes = expiry_request().encode().expect("expiry bytes");
        assert!(matches!(
            authenticate_core_request(&bytes, parent(), &[]),
            Ok(AuthenticatedCoreRequestV3::PermitExpiry(_))
        ));
        for hostile in [
            CoreCompositionParentV3 {
                release_set: [0x81; 32],
                ..parent()
            },
            CoreCompositionParentV3 {
                market: [0x82; 32],
                ..parent()
            },
            CoreCompositionParentV3 {
                generation: parent().generation + 1,
                ..parent()
            },
            CoreCompositionParentV3 {
                trading_program: [0x83; 32],
                ..parent()
            },
        ] {
            assert_eq!(
                authenticate_core_request(&bytes, hostile, &[]),
                Err(ProgramError::from(TradingSbfError::Content))
            );
        }

        let mut substituted = bytes;
        substituted[0] ^= 1;
        assert_eq!(
            authenticate_core_request(&substituted, parent(), &[]),
            Err(ProgramError::from(TradingSbfError::UnsupportedContent))
        );
        assert_eq!(
            authenticate_core_request(&bytes[..bytes.len() - 1], parent(), &[]),
            Err(ProgramError::from(TradingSbfError::Content))
        );
    }

    #[test]
    fn precommit_expiry_requires_exact_transport_and_family() {
        let (request, bytes, family) = precommit_request();
        assert_eq!(
            authenticate_core_request(&bytes, parent(), &family),
            Ok(AuthenticatedCoreRequestV3::UnallocatedPermitExpiry(
                request,
                id(6).to_bytes(),
            ))
        );

        let mut wrong_magic = bytes.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            authenticate_core_request(&wrong_magic, parent(), &family),
            Err(ProgramError::from(TradingSbfError::UnsupportedContent))
        );
        assert_eq!(
            authenticate_core_request(&bytes[..bytes.len() - 1], parent(), &family),
            Err(ProgramError::from(TradingSbfError::Content))
        );

        let mut wrong_action = family.clone();
        wrong_action[12] = SeriesActionV3::Consume as u8;
        assert_eq!(
            authenticate_core_request(&bytes, parent(), &wrong_action),
            Err(ProgramError::from(TradingSbfError::Content))
        );
    }

    #[test]
    fn precommit_overlap_classifier_is_closed_over_exact_core_once_shape() {
        fn classify(
            role: FixedRole,
            kind: RouteKindV3,
            account_count: u16,
            request: &[u8],
            family_request: &[u8],
            dependencies: &[RouteReceiptDependencyV3],
        ) -> Result<bool, ProgramError> {
            let route = [RouteInputV3 {
                role,
                kind,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: account_count,
                item_account_start: 0,
                item_account_count: if kind == RouteKindV3::Each { 1 } else { 0 },
                fixed_request: request,
                item_request: &[],
            }];
            let route_dependencies = [dependencies];
            let width = dclutch_vm::effect::v3::HEADER_BYTES
                + dclutch_vm::effect::v3::ROUTE_BYTES
                + dependencies.len() * dclutch_vm::effect::v3::RECEIPT_DEPENDENCY_BYTES
                + dclutch_vm::effect::v3::OPERATION_BYTES
                + request.len();
            let mut scratch = vec![0_u8; width];
            let mut output = vec![0_u8; width];
            encode_effect_program_v4_atomic(
                EffectGeometryV3 {
                    fixed_accounts: account_count,
                    item_account_stride: 0,
                    common_scalars: 1,
                    item_scalar_stride: 0,
                    common_identities: 0,
                    item_identity_stride: 0,
                },
                &route,
                &route_dependencies,
                &[EffectInstructionV3::require_lamports_eq(
                    AccountCoordinateV3::fixed(0),
                    ScalarCoordinateV3::common(0),
                )],
                &[],
                &mut scratch,
                &mut output,
            )
            .expect("effect");
            let effect = EffectProgramV3::decode(&output).expect("effect decode");
            let invocation = effect
                .resolved_invocation(0, 0, 0, &[0], &[])
                .expect("resolved invocation");
            is_series_permit_expiry_precommit_observation_v1(
                effect,
                0,
                0,
                invocation,
                request,
                family_request,
                parent(),
            )
        }

        let (_, exact, family) = precommit_request();
        assert_eq!(
            classify(
                FixedRole::Core,
                RouteKindV3::Once,
                SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1,
                &exact,
                &family,
                &[],
            ),
            Ok(true)
        );
        assert_eq!(
            classify(
                FixedRole::Custody,
                RouteKindV3::Once,
                SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1,
                &exact,
                &family,
                &[],
            ),
            Ok(false)
        );
        assert_eq!(
            classify(
                FixedRole::Core,
                RouteKindV3::Once,
                SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1 - 1,
                &exact,
                &family,
                &[],
            ),
            Ok(false)
        );
        let mut substituted = exact.clone();
        substituted[16] ^= 1;
        assert_eq!(
            classify(
                FixedRole::Core,
                RouteKindV3::Once,
                SERIES_PERMIT_EXPIRY_PRECOMMIT_ACCOUNT_COUNT_V1,
                &substituted,
                &family,
                &[],
            ),
            Ok(false)
        );
    }

    #[test]
    fn ordinary_and_precommit_expiry_packets_remain_disjoint() {
        let ordinary = expiry_request().encode().expect("ordinary expiry");
        assert!(matches!(
            authenticate_core_request(&ordinary, parent(), &[]),
            Ok(AuthenticatedCoreRequestV3::PermitExpiry(_))
        ));
        let (_, precommit, family) = precommit_request();
        assert!(matches!(
            authenticate_core_request(&precommit, parent(), &family),
            Ok(AuthenticatedCoreRequestV3::UnallocatedPermitExpiry(_, _))
        ));
        assert_eq!(ordinary.len(), SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1);
        assert_eq!(
            precommit.len(),
            SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1
        );
    }

    #[test]
    fn precommit_replay_is_readonly_only_in_the_child_view() {
        let key = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mut lamports = 7;
        let mut data = [19];
        let parent = AccountInfo::new(&key, false, true, &mut lamports, &mut data, &owner, false);
        let child = series_precommit_replay_observation_v1(parent.clone()).expect("observation");
        assert!(parent.is_writable);
        assert!(!child.is_writable);
        assert!(!child.is_signer);
        assert_eq!(child.key, parent.key);
        assert_eq!(child.owner, parent.owner);
        assert_eq!(&**child.try_borrow_data().expect("data"), &[19]);
        assert_eq!(child.lamports(), 7);
        for (signer, executable) in [(true, false), (false, true), (true, true)] {
            let mut hostile = parent.clone();
            hostile.is_signer = signer;
            hostile.executable = executable;
            assert_eq!(
                series_precommit_replay_observation_v1(hostile).err(),
                Some(TradingSbfError::Content.into())
            );
        }
    }

    #[test]
    fn precommit_caller_refuses_pda_privilege_owner_and_data_substitution() {
        let expected = Pubkey::new_unique();
        let wrong = Pubkey::new_unique();
        let system = system_program::ID;
        let stranger = Pubkey::new_unique();

        let mut lamports = 1_u64;
        let mut empty = [];
        let exact = AccountInfo::new(
            &expected,
            false,
            false,
            &mut lamports,
            &mut empty,
            &system,
            false,
        );
        assert_eq!(authenticate_precommit_caller_v1(&exact, &expected), Ok(()));

        let mut hostile_lamports = 1_u64;
        let mut hostile_empty = [];
        let wrong_key = AccountInfo::new(
            &wrong,
            false,
            false,
            &mut hostile_lamports,
            &mut hostile_empty,
            &system,
            false,
        );
        assert_eq!(
            authenticate_precommit_caller_v1(&wrong_key, &expected),
            Err(ProgramError::from(
                TradingSbfError::SeriesPrecommitCallerKey
            ))
        );

        for (signer, writable, executable, owner) in [
            (true, false, false, &system),
            (false, true, false, &system),
            (false, false, true, &system),
            (false, false, false, &stranger),
        ] {
            let mut hostile_lamports = 1_u64;
            let mut hostile_empty = [];
            let account = AccountInfo::new(
                &expected,
                signer,
                writable,
                &mut hostile_lamports,
                &mut hostile_empty,
                owner,
                executable,
            );
            assert_eq!(
                authenticate_precommit_caller_v1(&account, &expected),
                Err(ProgramError::from(if owner == &stranger {
                    TradingSbfError::SeriesPrecommitCallerOwner
                } else {
                    TradingSbfError::SeriesPrecommitCallerPrivileges
                }))
            );
        }

        let mut hostile_lamports = 1_u64;
        let mut nonempty = [1_u8];
        let account = AccountInfo::new(
            &expected,
            false,
            false,
            &mut hostile_lamports,
            &mut nonempty,
            &system,
            false,
        );
        assert_eq!(
            authenticate_precommit_caller_v1(&account, &expected),
            Err(ProgramError::from(
                TradingSbfError::SeriesPrecommitCallerData
            ))
        );
    }

    #[test]
    fn legacy_series_consume_packet_remains_the_typed_signed_branch() {
        let request = SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Consume,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            9,
            10,
            11,
            12,
            13,
            14,
            15,
        )
        .expect("Series Consume request");
        let bytes = request.encode().expect("request bytes");
        let parent = CoreCompositionParentV3 {
            release_set: id(1).to_bytes(),
            market: id(4).to_bytes(),
            generation: 10,
            trading_program: [0x91; 32],
        };
        assert_eq!(
            authenticate_core_request(&bytes, parent, &[]),
            Ok(AuthenticatedCoreRequestV3::Series(request))
        );
    }

    #[test]
    fn receiptless_expiry_cannot_be_named_as_a_dependency() {
        let request = [0x55_u8; 8];
        let routes = [
            RouteInputV3 {
                role: FixedRole::Core,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 0,
                fixed_account_count: 25,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &request,
                item_request: &[],
            },
            RouteInputV3 {
                role: FixedRole::Custody,
                kind: RouteKindV3::Once,
                enable_common_scalar: None,
                witness_range_common_scalar: None,
                receipt_dependency: None,
                fixed_account_start: 25,
                fixed_account_count: 1,
                item_account_start: 0,
                item_account_count: 0,
                fixed_request: &request,
                item_request: &[],
            },
        ];
        let dependent = [RouteReceiptDependencyV3::new(FixedRole::Core, 0, 8)];
        let dependencies = [&[][..], &dependent[..]];
        let width = dclutch_vm::effect::v3::HEADER_BYTES
            + 2 * dclutch_vm::effect::v3::ROUTE_BYTES
            + dclutch_vm::effect::v3::RECEIPT_DEPENDENCY_BYTES
            + 2 * request.len();
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_effect_program_v4_atomic(
            EffectGeometryV3 {
                fixed_accounts: 26,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &routes,
            &dependencies,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("effect");
        let effect = EffectProgramV3::decode(&output).expect("effect decode");
        assert_eq!(has_receipt_dependent(effect, 0, 0), Ok(true));
        assert_eq!(has_receipt_dependent(effect, 1, 0), Ok(false));
    }
}
