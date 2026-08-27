//! Family-neutral Custody CPI execution for EffectProgram V3 routes.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, CustodyReceiptV1, CustodyRequestV1,
    DELEGATED_CUSTODY_REQUEST_MAGIC_V2, DelegatedCustodyReceiptV2, DelegatedCustodyRequestV2,
};
use dclutch_effect_kernel::{
    v2::FixedRole,
    v3::{ProgramV3, ResolvedInvocationV3, RouteKindV3},
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::{TradingSbfError, child_receipt_v3::append_receipt_dependency_v3};

const CUSTODY_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-custody-receipt:v3";
const CUSTODY_REPLAY_FRAME_COORDINATE_V1: usize = 8;

/// Immutable parent facts every projected Custody request must reproduce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyCompositionParentV3 {
    /// Current immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Current Market generation.
    pub generation: u64,
    /// SHA-256 of the complete exact family request.
    pub parent_request_digest: [u8; 32],
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
}

/// Preflight one exact active Custody invocation without external mutation.
#[allow(clippy::too_many_arguments)]
pub fn preflight_custody_route_v3(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'_>],
    request_bank: &[u8],
    custody_program: &AccountInfo<'_>,
    parent: CustodyCompositionParentV3,
) -> Result<(), ProgramError> {
    let prepared = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        custody_program,
        parent,
    )?;
    let _ = prepared;
    Ok(())
}

/// Execute one preflighted Custody invocation and verify its immediate receipt.
#[allow(clippy::too_many_arguments)]
pub fn execute_custody_route_v3<'info>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'info>],
    request_bank: &[u8],
    prior_receipt: Option<&[u8]>,
    custody_program: &AccountInfo<'info>,
    parent: CustodyCompositionParentV3,
) -> Result<[u8; 32], ProgramError> {
    let prepared = prepare(
        program_id,
        effect,
        route_index,
        invocation_index,
        tail_count,
        scalars,
        identities,
        effect_accounts,
        request_bank,
        custody_program,
        parent,
    )?;
    let mut child_accounts = invocation_accounts(prepared.invocation, effect_accounts)?;
    let mut metas = Vec::with_capacity(child_accounts.len());
    for (index, account) in child_accounts.iter().enumerate() {
        let signer = index == 0 || account.is_signer;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let mut child_data = prepared.request_bytes.to_vec();
    append_receipt_dependency_v3(prepared.invocation, &mut child_data, prior_receipt)?;
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: child_data,
    };
    child_accounts.push(custody_program.clone());
    let bump_seed = [prepared.bump];
    let [domain, release, market, role, context, digest] = prepared.authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &child_accounts,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *custody_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    let replay = child_accounts
        .get(CUSTODY_REPLAY_FRAME_COORDINATE_V1)
        .ok_or(TradingSbfError::Transition)?;
    let replay_digest = {
        let bytes = replay
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Transition)?;
        hash(&bytes).to_bytes()
    };
    verify_custody_receipt_v3(
        prepared.request,
        &receipt_bytes,
        prepared.request_digest,
        replay_digest,
    )?;
    Ok(hashv(&[
        CUSTODY_EXECUTION_DIGEST_DOMAIN_V3,
        &route_index.to_le_bytes(),
        &invocation_index.to_le_bytes(),
        &prepared.request_digest,
        &receipt_bytes,
    ])
    .to_bytes())
}

struct PreparedCustodyInvocationV3<'a> {
    invocation: ResolvedInvocationV3,
    request: CustodyRequestKindV3,
    request_bytes: &'a [u8],
    request_digest: [u8; 32],
    authority_seeds: CallerAuthoritySeedsV1,
    bump: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CustodyRequestKindV3 {
    V1(CustodyRequestV1),
    DelegatedV2(DelegatedCustodyRequestV2),
}

impl CustodyRequestKindV3 {
    const fn base(self) -> CustodyRequestV1 {
        match self {
            Self::V1(request) => request,
            Self::DelegatedV2(request) => request.custody,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare<'a>(
    program_id: &Pubkey,
    effect: ProgramV3<'_>,
    route_index: u16,
    invocation_index: u32,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'_>],
    request_bank: &'a [u8],
    custody_program: &AccountInfo<'_>,
    parent: CustodyCompositionParentV3,
) -> Result<PreparedCustodyInvocationV3<'a>, ProgramError> {
    validate_parent(program_id, parent)?;
    if !custody_program.executable
        || custody_program.is_signer
        || custody_program.is_writable
        || effect
            .account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
            != effect_accounts.len()
    {
        return Err(TradingSbfError::Content.into());
    }
    let invocation = effect
        .resolved_invocation(
            route_index,
            invocation_index,
            tail_count,
            scalars,
            identities,
        )
        .map_err(|_| TradingSbfError::Content)?;
    if invocation.role != FixedRole::Custody || invocation.borrowed_witness.is_some() {
        return Err(TradingSbfError::Content.into());
    }
    let request_bytes = invocation_request(invocation, request_bank)?;
    let request = decode_custody_request_v3(request_bytes)?;
    let custody = request.base();
    if custody.caller_role != CallerRoleV1::Trading
        || custody.release_set != parent.release_set
        || custody.market != parent.market
        || custody.semantic.generation != parent.generation
        || custody.semantic.parent_request_digest != parent.parent_request_digest
        || custody.caller_program != parent.trading_program
    {
        return Err(TradingSbfError::Content.into());
    }
    let request_digest = hash(request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(custody.release_set).map_err(|_| TradingSbfError::Content)?,
        custody.market,
        ExecutionRoleV1::Trading,
        custody.context,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    let child_accounts = invocation_accounts(invocation, effect_accounts)?;
    require_custody_frame_shape_v3(&child_accounts, custody_program.key, &expected_authority)?;
    Ok(PreparedCustodyInvocationV3 {
        invocation,
        request,
        request_bytes,
        request_digest,
        authority_seeds,
        bump,
    })
}

/// Refuse any Custody frame that is not the exact shape the child CPI needs.
///
/// **The callee is not a member of the frame.** A Custody `Transfer` FrameSpec
/// declares `CallerProgram`/`CallerProgramData` -- Trading's -- and never names
/// the Custody program itself, so the topology carries the callee at a
/// family-owned coordinate past every route range (Direct's is 90) and
/// [`execute_custody_route_v3`] appends it after the frame's metas.
///
/// This guard previously demanded the opposite: that the callee appear exactly
/// ONCE INSIDE the fourteen-account frame. That condition was copied from the
/// Claims check, where it holds only because a Claims sparse-native-transfer
/// frame does declare `ClaimsProgram` at frame coordinate 16. No Custody frame
/// has such a coordinate at any index, so the condition was unsatisfiable by
/// construction and every Custody invocation refused here -- which is why no
/// Custody child CPI had ever executed on this path.
///
/// Requiring absence is not a weakened refusal; it is the same fact stated for
/// the topology that actually exists. The callee is authenticated against the
/// activated release set by `selected_role_program_v3` before it reaches this
/// module, and requiring it to be absent from the frame keeps the appended
/// callee the only account in the invocation that can be it. A frame that
/// smuggled the Custody program into a coordinate its FrameSpec declares as
/// something else would hand the child a duplicate account under a role it does
/// not hold; that is refused here rather than left to the child.
///
/// The three facts are checked separately and refuse distinctly. Fusing them
/// into one `Release` is what made this cost a measurement run to diagnose: the
/// refusal named no conjunct, so a frame carrying no callee at all and a frame
/// whose caller authority was derived from the wrong seeds were the same error
/// code.
fn require_custody_frame_shape_v3(
    child_accounts: &[AccountInfo<'_>],
    custody_program: &Pubkey,
    expected_authority: &Pubkey,
) -> Result<(), ProgramError> {
    if child_accounts
        .iter()
        .any(|account| account.key == custody_program)
    {
        return Err(TradingSbfError::Content.into());
    }
    if child_accounts
        .get(CUSTODY_REPLAY_FRAME_COORDINATE_V1)
        .is_none()
    {
        return Err(TradingSbfError::Content.into());
    }
    if child_accounts
        .first()
        .is_none_or(|account| account.key != expected_authority)
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(())
}

fn decode_custody_request_v3(bytes: &[u8]) -> Result<CustodyRequestKindV3, ProgramError> {
    if bytes.get(..8) == Some(DELEGATED_CUSTODY_REQUEST_MAGIC_V2.as_slice()) {
        DelegatedCustodyRequestV2::decode(bytes)
            .map(CustodyRequestKindV3::DelegatedV2)
            .map_err(|_| TradingSbfError::Content.into())
    } else {
        let request = CustodyRequestV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
        if request.source_compartment == CompartmentV1::External {
            return Err(TradingSbfError::Content.into());
        }
        Ok(CustodyRequestKindV3::V1(request))
    }
}

fn verify_custody_receipt_v3(
    request: CustodyRequestKindV3,
    receipt_bytes: &[u8],
    request_digest: [u8; 32],
    replay_digest: [u8; 32],
) -> Result<(), ProgramError> {
    match request {
        CustodyRequestKindV3::V1(request) => {
            let receipt =
                CustodyReceiptV1::decode(receipt_bytes).map_err(|_| TradingSbfError::Transition)?;
            receipt
                .verify_for(request, request_digest, replay_digest)
                .map_err(|_| TradingSbfError::Transition.into())
        }
        CustodyRequestKindV3::DelegatedV2(request) => {
            let receipt = DelegatedCustodyReceiptV2::decode(receipt_bytes)
                .map_err(|_| TradingSbfError::Transition)?;
            receipt
                .custody
                .verify_for(request.custody, request_digest, replay_digest)
                .map_err(|_| TradingSbfError::Transition)?;
            if receipt.starts_atomic_debit != request.starts_atomic_debit
                || receipt.terminal != request.terminal
                || receipt.delegate_before != request.delegate_before
                || receipt.delegate_after != request.delegate_after
                || receipt.total_debit != request.total_debit
                || receipt.allowance_before != request.allowance_before
                || receipt.allowance_after != request.allowance_after
            {
                return Err(TradingSbfError::Transition.into());
            }
            Ok(())
        }
    }
}

fn validate_parent(
    program_id: &Pubkey,
    parent: CustodyCompositionParentV3,
) -> Result<(), ProgramError> {
    if parent.release_set == [0; 32]
        || parent.market == [0; 32]
        || parent.parent_request_digest == [0; 32]
        || parent.trading_program != program_id.to_bytes()
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn invocation_request(
    invocation: ResolvedInvocationV3,
    request_bank: &[u8],
) -> Result<&[u8], ProgramError> {
    let end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    request_bank
        .get(invocation.request_offset..end)
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn invocation_accounts<'info>(
    invocation: ResolvedInvocationV3,
    accounts: &[AccountInfo<'info>],
) -> Result<Vec<AccountInfo<'info>>, ProgramError> {
    let mut output = Vec::new();
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    output.extend_from_slice(
        accounts
            .get(fixed_start..fixed_end)
            .ok_or(TradingSbfError::Content)?,
    );
    let item_count = usize::from(invocation.item_account_count);
    match invocation.kind {
        RouteKindV3::Once => {
            if item_count != 0 || invocation.repeated_item_count != 0 {
                return Err(TradingSbfError::Content.into());
            }
        }
        RouteKindV3::Each => {
            if invocation.item.is_none() || invocation.repeated_item_count != 1 {
                return Err(TradingSbfError::Content.into());
            }
            let end = invocation
                .item_account_start
                .checked_add(item_count)
                .ok_or(TradingSbfError::Content)?;
            output.extend_from_slice(
                accounts
                    .get(invocation.item_account_start..end)
                    .ok_or(TradingSbfError::Content)?,
            );
        }
        RouteKindV3::AffineOnce => {
            let stride = usize::from(invocation.item_account_stride);
            let mut item = 0_u32;
            while item < invocation.repeated_item_count {
                let start = invocation
                    .item_account_start
                    .checked_add(
                        usize::try_from(item)
                            .map_err(|_| TradingSbfError::Content)?
                            .checked_mul(stride)
                            .ok_or(TradingSbfError::Content)?,
                    )
                    .ok_or(TradingSbfError::Content)?;
                let end = start
                    .checked_add(item_count)
                    .ok_or(TradingSbfError::Content)?;
                output.extend_from_slice(accounts.get(start..end).ok_or(TradingSbfError::Content)?);
                item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use dclutch_custody_contract::{
        ContextV1, DelegatedAllowanceObservationV2, OperationV1, ReceiptEvidenceV1,
        TRANSFER_ACCOUNT_COUNT_V1,
    };

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn delegated_request() -> DelegatedCustodyRequestV2 {
        DelegatedCustodyRequestV2 {
            custody: CustodyRequestV1 {
                operation: OperationV1::Transfer,
                caller_role: CallerRoleV1::Trading,
                source_compartment: CompartmentV1::External,
                destination_compartment: CompartmentV1::HoardPrincipal,
                release_set: id(1),
                market: id(2),
                realm: id(3),
                context: id(4),
                caller_program: id(5),
                semantic: ContextV1 {
                    candidate: id(6),
                    source_owner: id(7),
                    destination_owner: [0; 32],
                    order: id(8),
                    parent_request_digest: id(9),
                    order_nonce: 10,
                    generation: 11,
                    page_index: 12,
                    execution_index: 13,
                    transfer_index: 0,
                },
                source: id(14),
                destination: id(15),
                source_vault_context: [0; 32],
                destination_vault_context: id(4),
                mint: id(16),
                token_program: id(17),
                payer: [0; 32],
                rent_refund: [0; 32],
                expected_revision: 3,
                resulting_revision: 4,
                amount: 40,
                rent_lamports: 0,
            },
            starts_atomic_debit: true,
            terminal: false,
            delegate_before: id(18),
            delegate_after: id(18),
            total_debit: 100,
            allowance_before: 100,
            allowance_after: 60,
        }
    }

    #[test]
    fn delegated_v2_is_distinct_and_v1_external_debit_is_fail_closed() {
        let request = delegated_request();
        let bytes = request.encode().expect("delegated request");
        assert_eq!(
            decode_custody_request_v3(&bytes),
            Ok(CustodyRequestKindV3::DelegatedV2(request))
        );
        let base = request.custody.to_bytes().expect("base request");
        assert_eq!(
            decode_custody_request_v3(&base),
            Err(TradingSbfError::Content.into())
        );

        let request_digest = hash(&bytes).to_bytes();
        let replay_digest = id(20);
        let receipt = DelegatedCustodyReceiptV2::new(
            request,
            request_digest,
            ReceiptEvidenceV1 {
                source_before: 500,
                source_after: 460,
                destination_before: 10,
                destination_after: 50,
                poststate_commitment: id(19),
                replay_state_digest: replay_digest,
            },
            DelegatedAllowanceObservationV2 {
                delegate_before: id(18),
                allowance_before: 100,
                delegate_after: id(18),
                allowance_after: 60,
            },
        )
        .expect("delegated receipt")
        .encode()
        .expect("receipt bytes");
        assert_eq!(
            verify_custody_receipt_v3(
                CustodyRequestKindV3::DelegatedV2(request),
                &receipt,
                request_digest,
                replay_digest,
            ),
            Ok(())
        );
        assert_eq!(
            verify_custody_receipt_v3(
                CustodyRequestKindV3::DelegatedV2(request),
                &receipt,
                request_digest,
                id(21),
            ),
            Err(TradingSbfError::Transition.into())
        );
    }

    #[test]
    fn parent_binding_refuses_program_or_request_substitution() {
        let program = Pubkey::new_from_array([5; 32]);
        let canonical = CustodyCompositionParentV3 {
            release_set: [1; 32],
            market: [2; 32],
            generation: 3,
            parent_request_digest: [4; 32],
            trading_program: [5; 32],
        };
        assert_eq!(validate_parent(&program, canonical), Ok(()));

        for hostile in [
            CustodyCompositionParentV3 {
                trading_program: [6; 32],
                ..canonical
            },
            CustodyCompositionParentV3 {
                parent_request_digest: [0; 32],
                ..canonical
            },
            CustodyCompositionParentV3 {
                release_set: [0; 32],
                ..canonical
            },
            CustodyCompositionParentV3 {
                market: [0; 32],
                ..canonical
            },
        ] {
            assert_eq!(
                validate_parent(&program, hostile),
                Err(TradingSbfError::Content.into())
            );
        }
    }

    /// Build one frame of `keys` as writable non-signer accounts.
    ///
    /// The banks are supplied by the caller so each `AccountInfo` can hold a
    /// distinct `&mut` into them for the frame's whole lifetime.
    fn frame<'a>(
        keys: &'a [Pubkey],
        owner: &'a Pubkey,
        lamports: &'a mut [u64],
        data: &'a mut [[u8; 1]],
    ) -> Vec<AccountInfo<'a>> {
        keys.iter()
            .zip(lamports.iter_mut())
            .zip(data.iter_mut())
            .map(|((key, lamport), bytes)| {
                AccountInfo::new(
                    key,
                    false,
                    true,
                    lamport,
                    bytes.as_mut_slice(),
                    owner,
                    false,
                )
            })
            .collect()
    }

    const FRAME: usize = TRANSFER_ACCOUNT_COUNT_V1 as usize;

    #[test]
    fn a_custody_frame_carrying_its_own_callee_is_refused() {
        let owner = Pubkey::new_from_array(id(0x20));
        let authority = Pubkey::new_from_array(id(0x21));
        let callee = Pubkey::new_from_array(id(0x22));
        let filler = Pubkey::new_from_array(id(0x23));
        // The canonical fourteen-account Transfer frame: caller authority at
        // coordinate 0, replay reachable at coordinate 8, and no coordinate
        // anywhere holding the callee.
        let mut keys = [filler; FRAME];
        keys[0] = authority;
        let (mut lamports, mut data) = ([0_u64; FRAME], [[0_u8; 1]; FRAME]);
        assert_eq!(
            require_custody_frame_shape_v3(
                &frame(&keys, &owner, &mut lamports, &mut data),
                &callee,
                &authority
            ),
            Ok(()),
            "the shape the topology actually produces was refused"
        );

        // The callee smuggled into a coordinate whose FrameSpec role is
        // something else. Before this, the OPPOSITE was required -- the callee
        // had to be INSIDE the frame -- so every canonical frame refused and no
        // Custody child CPI could ever run.
        for coordinate in [1_usize, 8, FRAME - 1] {
            let mut hostile = keys;
            hostile[coordinate] = callee;
            let (mut hl, mut hd) = ([0_u64; FRAME], [[0_u8; 1]; FRAME]);
            assert_eq!(
                require_custody_frame_shape_v3(
                    &frame(&hostile, &owner, &mut hl, &mut hd),
                    &callee,
                    &authority
                ),
                Err(TradingSbfError::Content.into()),
                "a frame carrying the callee at coordinate {coordinate} was admitted"
            );
        }
    }

    #[test]
    fn a_short_frame_and_a_wrong_caller_authority_refuse_distinctly() {
        let owner = Pubkey::new_from_array(id(0x30));
        let authority = Pubkey::new_from_array(id(0x31));
        let callee = Pubkey::new_from_array(id(0x32));
        let filler = Pubkey::new_from_array(id(0x33));

        // Too short to reach the replay coordinate: Content, never Release.
        const SHORT: usize = CUSTODY_REPLAY_FRAME_COORDINATE_V1;
        let mut short = [filler; SHORT];
        short[0] = authority;
        let (mut sl, mut sd) = ([0_u64; SHORT], [[0_u8; 1]; SHORT]);
        assert_eq!(
            require_custody_frame_shape_v3(
                &frame(&short, &owner, &mut sl, &mut sd),
                &callee,
                &authority
            ),
            Err(TradingSbfError::Content.into())
        );

        // Wide enough, no callee inside, but coordinate 0 is not the
        // release-pinned caller authority: Release, and only Release.
        let wrong = [filler; FRAME];
        let (mut wl, mut wd) = ([0_u64; FRAME], [[0_u8; 1]; FRAME]);
        assert_eq!(
            require_custody_frame_shape_v3(
                &frame(&wrong, &owner, &mut wl, &mut wd),
                &callee,
                &authority
            ),
            Err(TradingSbfError::Release.into())
        );
    }
}
