//! The typed empty-projection cleanup child. Its caller namespace and receipt
//! are distinct from ordinary Custody; neither may be decoded as the other.

use super::*;
use dclutch_custody::{
    PROJECTED_CUSTODY_REQUEST_MAGIC_V1, ProjectedCustodyAbortFrameV1 as Frame,
    ProjectedCustodyCallerSeedsV1, ProjectedCustodyOperationV1, ProjectedCustodyReceiptV1,
    ProjectedCustodyRequestV1,
};

pub(super) fn selected(bytes: &[u8]) -> bool {
    bytes.get(..8) == Some(PROJECTED_CUSTODY_REQUEST_MAGIC_V1.as_slice())
}

struct Prepared<'a> {
    request: ProjectedCustodyRequestV1,
    bytes: &'a [u8],
    digest: [u8; 32],
    seeds: ProjectedCustodyCallerSeedsV1,
    bump: u8,
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare<'a, 'info>(
    program: &Pubkey,
    count: usize,
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    bank: &'a [u8],
    frame: &mut Vec<AccountInfo<'info>>,
    callee: &AccountInfo<'_>,
    parent: CustodyCompositionParentV3,
    hint: PreflightedCallerBumpV4,
) -> Result<Prepared<'a>, ProgramError> {
    validate_parent(program, parent)?;
    if count != accounts.len()
        || !callee.executable
        || callee.is_signer
        || callee.is_writable
        || invocation.role != FixedRole::Custody
        || invocation.borrowed_witness.is_some()
    {
        return Err(TradingSbfError::Content.into());
    }
    let bytes = invocation_request(invocation, bank)?;
    let request = ProjectedCustodyRequestV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    require_parent(&request, parent)?;
    let digest = hash(bytes).to_bytes();
    let seeds = ProjectedCustodyCallerSeedsV1::new(request, digest);
    let (address, bump) = match hint {
        None => Pubkey::find_program_address(&seeds.as_slices(), program),
        Some(bump) => {
            let [domain, release, market, root, context, digest] = seeds.as_slices();
            let address = Pubkey::create_program_address(
                &[domain, release, market, root, context, digest, &[bump]],
                program,
            )
            .map_err(|_| TradingSbfError::Release)?;
            (address, bump)
        }
    };
    gather_invocation_accounts(frame, invocation, accounts)?;
    if frame.len() != Frame::ACCOUNT_COUNT || frame.iter().any(|a| a.key == callee.key) {
        return Err(TradingSbfError::Content.into());
    }
    if frame[Frame::CALLER].key != &address {
        return Err(TradingSbfError::Release.into());
    }
    Ok(Prepared {
        request,
        bytes,
        digest,
        seeds,
        bump,
    })
}

fn require_parent(
    request: &ProjectedCustodyRequestV1,
    parent: CustodyCompositionParentV3,
) -> Result<(), ProgramError> {
    // Only terminal empty-projection cleanup is a composed route today. The
    // remaining operations need their own poststate/receipt contract before
    // they can be admitted here. The context is the persisted projection's
    // context, not the current family request's digest; Custody checks it.
    if request.operation != ProjectedCustodyOperationV1::AbortOpenAndClose
        || request.release_set != parent.release_set
        || request.market != parent.market
        || request.generation != parent.generation
        || request.caller_program != parent.trading_program
        || request.parent_capability_root != parent.capability_root
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn preflight<'info>(
    program: &Pubkey,
    count: usize,
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    bank: &[u8],
    frame: &mut Vec<AccountInfo<'info>>,
    callee: &AccountInfo<'_>,
    parent: CustodyCompositionParentV3,
    hint: PreflightedCallerBumpV4,
) -> Result<u8, ProgramError> {
    Ok(prepare(
        program, count, invocation, accounts, bank, frame, callee, parent, hint,
    )?
    .bump)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn execute<'info>(
    program: &Pubkey,
    count: usize,
    route: u16,
    ordinal: u32,
    invocation: ResolvedInvocationV3,
    accounts: DowngradedEffectAccountsV3<'_, '_, 'info>,
    bank: &[u8],
    prior_receipt: Option<&[u8]>,
    buffers: &mut ChildInvocationBuffersV3<'info>,
    callee: &AccountInfo<'info>,
    parent: CustodyCompositionParentV3,
    hint: PreflightedCallerBumpV4,
) -> Result<[u8; 32], ProgramError> {
    let prepared = prepare(
        program,
        count,
        invocation,
        accounts,
        bank,
        &mut buffers.accounts,
        callee,
        parent,
        hint,
    )?;
    let rent_before = buffers.accounts[Frame::RENT_CREDIT].lamports();
    buffers.fill_metas()?;
    buffers.set_wire(prepared.bytes)?;
    deliver_receipt_dependency_v3(
        invocation,
        &mut buffers.data,
        prior_receipt,
        ReceiptDeliveryV3::VerifiedOnly,
    )?;
    // Projected Custody has its own caller namespace and exact 768-byte wire.
    // It derives its caller itself and accepts no ordinary Custody bump tail.
    buffers.push_callee(callee)?;
    let [domain, release, market, root, context, digest] = prepared.seeds.as_slices();
    buffers
        .invoke(
            callee.key,
            &[&[
                domain,
                release,
                market,
                root,
                context,
                digest,
                &[prepared.bump],
            ]],
        )
        .map_err(child_refused_v1)?;
    buffers.capture_return()?;
    if buffers.producer != *callee.key {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    verify_receipt(&prepared.request, prepared.digest, &buffers.returned)?;
    let expected_rent = rent_before
        .checked_add(prepared.request.state_rent_lamports)
        .and_then(|value| value.checked_add(prepared.request.vault_rent_lamports))
        .ok_or(TradingSbfError::ChildReceipt)?;
    if buffers.accounts[Frame::STATE].lamports() != 0
        || buffers.accounts[Frame::VAULT].lamports() != 0
        || buffers.accounts[Frame::RENT_CREDIT].lamports() != expected_rent
    {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    Ok(delegated_custody_child_execution_digest_v3(
        route,
        ordinal,
        prepared.digest,
        &buffers.returned,
    ))
}

fn verify_receipt(
    request: &ProjectedCustodyRequestV1,
    digest: [u8; 32],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let receipt =
        ProjectedCustodyReceiptV1::decode(bytes).map_err(|_| TradingSbfError::ChildReceipt)?;
    let expected = ProjectedCustodyReceiptV1 {
        realized: false,
        aborted_open: true,
        market: request.market,
        release_set: request.release_set,
        parent_capability_root: request.parent_capability_root,
        context_digest: request.context_digest,
        hoard_vault: request.hoard_vault,
        amount: 0,
        request_digest: digest,
        market_state_digest: [0; 32],
        rent_credit: request.rent_credit,
        resulting_revision: request.resulting_revision,
    };
    if receipt != expected {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_custody::ProjectedCallerRoleV1;

    fn request() -> ProjectedCustodyRequestV1 {
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::AbortOpenAndClose,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: [1; 32],
            generation: 7,
            realm: [2; 32],
            product_record: [3; 32],
            product: [4; 32],
            source: [5; 32],
            release_set: [6; 32],
            projection_receipt_digest: [7; 32],
            parent_capability_root: [8; 32],
            context_digest: [9; 32],
            caller_program: [10; 32],
            payer: [11; 32],
            core_program: [12; 32],
            rent_program: [13; 32],
            refund_owner: [14; 32],
            rent_credit: [15; 32],
            hoard_vault: [16; 32],
            funding_source_vault: [17; 32],
            funding_source_context: [18; 32],
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: [19; 32],
            token_program: [20; 32],
            collateral_release: [21; 32],
            expiry_slot: 100,
            expected_revision: 2,
            resulting_revision: 3,
            amount: 0,
            state_rent_lamports: 1000,
            vault_rent_lamports: 2000,
            funding_source_replay_revision: 3,
            funding_source_state_rent_lamports: 3000,
            funding_source_vault_rent_lamports: 4000,
        }
    }

    #[test]
    fn projected_cleanup_keeps_its_typed_wire_and_selected_parent() {
        let request = request();
        let bytes = request.encode().unwrap();
        assert!(selected(&bytes));
        assert_eq!(ProjectedCustodyRequestV1::decode(&bytes), Ok(request));
        assert_eq!(
            decode_custody_request_v3(&bytes),
            Err(TradingSbfError::Content.into())
        );
        let parent = CustodyCompositionParentV3 {
            capability_root: request.parent_capability_root,
            market: request.market,
            generation: request.generation,
            release_set: request.release_set,
            parent_request_digest: [22; 32],
            trading_program: request.caller_program,
            child_relay: [0; 2],
        };
        assert_eq!(require_parent(&request, parent), Ok(()));
        for hostile in [
            ProjectedCustodyRequestV1 {
                market: [23; 32],
                ..request
            },
            ProjectedCustodyRequestV1 {
                generation: 8,
                ..request
            },
            ProjectedCustodyRequestV1 {
                release_set: [23; 32],
                ..request
            },
            ProjectedCustodyRequestV1 {
                parent_capability_root: [23; 32],
                ..request
            },
            ProjectedCustodyRequestV1 {
                caller_program: [23; 32],
                ..request
            },
            ProjectedCustodyRequestV1 {
                operation: ProjectedCustodyOperationV1::OpenHoard,
                ..request
            },
        ] {
            assert_eq!(
                require_parent(&hostile, parent),
                Err(TradingSbfError::Content.into())
            );
        }
    }

    #[test]
    fn projected_cleanup_receipt_binds_every_terminal_fact() {
        let request = request();
        let digest = hash(&request.encode().unwrap()).to_bytes();
        let receipt = ProjectedCustodyReceiptV1 {
            realized: false,
            aborted_open: true,
            market: request.market,
            release_set: request.release_set,
            parent_capability_root: request.parent_capability_root,
            context_digest: request.context_digest,
            hoard_vault: request.hoard_vault,
            amount: 0,
            request_digest: digest,
            market_state_digest: [0; 32],
            rent_credit: request.rent_credit,
            resulting_revision: 3,
        };
        assert_eq!(
            verify_receipt(&request, digest, &receipt.encode().unwrap()),
            Ok(())
        );
        for hostile in [
            ProjectedCustodyReceiptV1 {
                market: [23; 32],
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                release_set: [23; 32],
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                parent_capability_root: [23; 32],
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                context_digest: [23; 32],
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                hoard_vault: [23; 32],
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                rent_credit: [23; 32],
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                request_digest: [23; 32],
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                resulting_revision: 4,
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                aborted_open: false,
                amount: 1,
                ..receipt
            },
            ProjectedCustodyReceiptV1 {
                aborted_open: false,
                realized: true,
                amount: 1,
                market_state_digest: [23; 32],
                ..receipt
            },
        ] {
            assert_eq!(
                verify_receipt(&request, digest, &hostile.encode().unwrap()),
                Err(TradingSbfError::ChildReceipt.into())
            );
        }
    }
}
