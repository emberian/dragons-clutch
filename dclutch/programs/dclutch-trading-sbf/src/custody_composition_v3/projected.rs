//! The typed empty-projection cleanup child. Its caller namespace and receipt
//! are distinct from ordinary Custody; neither may be decoded as the other.

use super::*;
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1,
    PROJECTED_CUSTODY_RECEIPT_BYTES_V1, PROJECTED_CUSTODY_REQUEST_MAGIC_V1,
    ProjectedCustodyAbortFrameV1, ProjectedCustodyCallerSeedsV1, ProjectedCustodyLockReceiptV1,
    ProjectedCustodyOperationV1, ProjectedCustodyReceiptV1, ProjectedCustodyRequestV1,
    ProjectedCustodyTerminalFrameV1,
};

const CALLER: usize = ProjectedCustodyTerminalFrameV1::CALLER;
const STATE: usize = ProjectedCustodyTerminalFrameV1::STATE;
const RENT_CREDIT: usize = ProjectedCustodyTerminalFrameV1::RENT_CREDIT;
const ABORT_VAULT: usize = ProjectedCustodyAbortFrameV1::VAULT;

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
    let terminal = ProjectedCustodyTerminalFrameV1::new(request.operation)
        .map_err(|_| TradingSbfError::Content)?;
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
    project_terminal_privileges_v1(terminal, frame)?;
    if frame.len() != terminal.account_count() || frame.iter().any(|a| a.key == callee.key) {
        return Err(TradingSbfError::Content.into());
    }
    if frame[CALLER].key != &address {
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
    // The context is the persisted projection's context, not the current
    // family request digest. Lock, Found, then Realize share this parent root;
    // each terminal's poststate is checked after its own CPI below.
    if !matches!(
        request.operation,
        ProjectedCustodyOperationV1::AbortOpenAndClose
            | ProjectedCustodyOperationV1::LockHoardAndCloseSource
            | ProjectedCustodyOperationV1::RealizeAndClose
    ) || request.release_set != parent.release_set
        || request.market != parent.market
        || request.generation != parent.generation
        || request.caller_program != parent.trading_program
        || request.parent_capability_root != parent.capability_root
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

/// Lower a gathered physical union to Custody's exact terminal child frame.
/// A required write must already be present in the parent transaction; this
/// only removes inherited capabilities and never escalates one.
fn project_terminal_privileges_v1(
    frame: ProjectedCustodyTerminalFrameV1,
    accounts: &mut [AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if accounts.len() != frame.account_count() {
        return Err(TradingSbfError::Content.into());
    }
    for (coordinate, account) in accounts.iter_mut().enumerate() {
        let expected = frame
            .privileges(coordinate)
            .map_err(|_| TradingSbfError::Content)?;
        if account.executable != expected.executable()
            || (expected.writable() && !account.is_writable)
        {
            return Err(TradingSbfError::Content.into());
        }
        account.is_signer = expected.signer();
        account.is_writable = expected.writable();
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
    let rent_before = buffers.accounts[RENT_CREDIT].lamports();
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
    match prepared.request.operation {
        ProjectedCustodyOperationV1::AbortOpenAndClose => {
            verify_abort_receipt(&prepared.request, prepared.digest, &buffers.returned)?;
            let expected_rent = rent_before
                .checked_add(prepared.request.state_rent_lamports)
                .and_then(|value| value.checked_add(prepared.request.vault_rent_lamports))
                .ok_or(TradingSbfError::ChildReceipt)?;
            if buffers.accounts[STATE].lamports() != 0
                || buffers.accounts[ABORT_VAULT].lamports() != 0
                || buffers.accounts[RENT_CREDIT].lamports() != expected_rent
            {
                return Err(TradingSbfError::ChildReceipt.into());
            }
        }
        ProjectedCustodyOperationV1::LockHoardAndCloseSource => {
            verify_lock_receipt(
                &prepared.request,
                prepared.digest,
                &buffers.accounts,
                &buffers.returned,
            )?;
            if buffers.accounts[ProjectedCustodyTerminalFrameV1::LOCK_SOURCE].lamports() != 0
                || buffers.accounts[ProjectedCustodyTerminalFrameV1::LOCK_SOURCE_REPLAY].lamports()
                    != 0
            {
                return Err(TradingSbfError::ChildReceipt.into());
            }
        }
        ProjectedCustodyOperationV1::RealizeAndClose => {
            verify_realize_receipt(
                &prepared.request,
                prepared.digest,
                &buffers.accounts,
                &buffers.returned,
            )?;
            if buffers.accounts[STATE].data_len() != CUSTODY_REPLAY_BYTES_V1
                || buffers.accounts[STATE].owner != callee.key
            {
                return Err(TradingSbfError::ChildReceipt.into());
            }
        }
        _ => return Err(TradingSbfError::Content.into()),
    }
    Ok(delegated_custody_child_execution_digest_v3(
        route,
        ordinal,
        prepared.digest,
        &buffers.returned,
    ))
}

fn verify_lock_receipt(
    request: &ProjectedCustodyRequestV1,
    digest: [u8; 32],
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    if bytes.len() != PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1 {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    let receipt =
        ProjectedCustodyLockReceiptV1::decode(bytes).map_err(|_| TradingSbfError::ChildReceipt)?;
    if receipt.market != request.market
        || receipt.release_set != request.release_set
        || receipt.context_digest != request.context_digest
        || receipt.source_vault != request.funding_source_vault
        || receipt.source_replay
            != accounts
                .get(ProjectedCustodyTerminalFrameV1::LOCK_SOURCE_REPLAY)
                .ok_or(TradingSbfError::ChildReceipt)?
                .key
                .to_bytes()
        || receipt.hoard_vault != request.hoard_vault
        || receipt.rent_credit != request.rent_credit
        || receipt.request_digest != digest
        || receipt.amount != request.amount
        || receipt.source_vault_rent_lamports != request.funding_source_vault_rent_lamports
        || receipt.source_replay_rent_lamports != request.funding_source_state_rent_lamports
        || receipt.resulting_revision != request.resulting_revision
    {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    Ok(())
}

fn verify_realize_receipt(
    request: &ProjectedCustodyRequestV1,
    digest: [u8; 32],
    accounts: &[AccountInfo<'_>],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    if bytes.len() != PROJECTED_CUSTODY_RECEIPT_BYTES_V1 {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    let receipt =
        ProjectedCustodyReceiptV1::decode(bytes).map_err(|_| TradingSbfError::ChildReceipt)?;
    let market_data = accounts
        .get(ProjectedCustodyTerminalFrameV1::REALIZE_MARKET)
        .ok_or(TradingSbfError::ChildReceipt)?
        .try_borrow_data()
        .map_err(|_| TradingSbfError::ChildReceipt)?;
    let market_digest = hash(&market_data).to_bytes();
    if !receipt.realized
        || receipt.aborted_open
        || receipt.market != request.market
        || receipt.release_set != request.release_set
        || receipt.parent_capability_root != request.parent_capability_root
        || receipt.context_digest != request.context_digest
        || receipt.hoard_vault != request.hoard_vault
        || receipt.amount != request.amount
        || receipt.request_digest != digest
        || receipt.market_state_digest != market_digest
        || receipt.rent_credit != request.rent_credit
        || receipt.resulting_revision != request.resulting_revision
    {
        return Err(TradingSbfError::ChildReceipt.into());
    }
    Ok(())
}

fn verify_abort_receipt(
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
    use alloc::{boxed::Box, vec, vec::Vec};

    use super::*;
    use dclutch_custody::ProjectedCallerRoleV1;

    fn account_info_with_data(data: Vec<u8>) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let lamports = Box::leak(Box::new(0_u64));
        let data = Box::leak(data.into_boxed_slice());
        AccountInfo::new(key, false, false, lamports, data, owner, false)
    }

    fn account_info() -> AccountInfo<'static> {
        account_info_with_data(Vec::new())
    }

    fn terminal_invocation(count: usize) -> ResolvedInvocationV3 {
        ResolvedInvocationV3 {
            role: FixedRole::Custody,
            kind: RouteKindV3::Once,
            item: None,
            fixed_account_start: 0,
            fixed_account_count: u16::try_from(count).expect("terminal frame count fits u16"),
            item_account_start: 0,
            item_account_count: 0,
            item_account_stride: 0,
            repeated_item_count: 0,
            request_offset: 0,
            request_len: 0,
            borrowed_witness: None,
            receipt_dependencies: dclutch_vm::effect::v3::ResolvedReceiptDependenciesV3::empty(),
            receipt_dependency: None,
        }
    }

    #[test]
    fn terminal_privilege_projection_lowers_production_gathered_unions() {
        for operation in [
            ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            ProjectedCustodyOperationV1::RealizeAndClose,
        ] {
            let frame = ProjectedCustodyTerminalFrameV1::new(operation)
                .expect("terminal operation has a source-owned frame");
            let mut physical: Vec<_> = (0..frame.account_count()).map(|_| account_info()).collect();
            for (coordinate, account) in physical.iter_mut().enumerate() {
                account.executable = frame
                    .privileges(coordinate)
                    .expect("frame coordinate")
                    .executable();
            }
            let logical: Vec<_> = physical.iter().collect();
            let declared = vec![3_u8; frame.account_count()];
            let effect_accounts = crate::hot_v3::downgraded_effect_accounts_v3(&logical, &declared)
                .expect("production child views");
            let mut gathered = Vec::new();
            super::gather_invocation_accounts(
                &mut gathered,
                terminal_invocation(frame.account_count()),
                effect_accounts,
            )
            .expect("production terminal gather");
            project_terminal_privileges_v1(frame, &mut gathered)
                .expect("source-owned terminal projection");
            for (coordinate, account) in gathered.iter().enumerate() {
                let expected = frame.privileges(coordinate).expect("frame coordinate");
                assert_eq!(
                    account.is_signer,
                    expected.signer(),
                    "{operation:?} {coordinate}"
                );
                assert_eq!(
                    account.is_writable,
                    expected.writable(),
                    "{operation:?} {coordinate}"
                );
                assert_eq!(
                    account.executable,
                    expected.executable(),
                    "{operation:?} {coordinate}"
                );
            }
            let required_write = 1_usize;
            let mut hostile = gathered.clone();
            hostile[required_write].is_writable = false;
            assert_eq!(
                project_terminal_privileges_v1(frame, &mut hostile),
                Err(ProgramError::Custom(TradingSbfError::Content as u32))
            );
        }
    }

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
        for operation in [
            ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            ProjectedCustodyOperationV1::RealizeAndClose,
        ] {
            assert_eq!(
                require_parent(
                    &ProjectedCustodyRequestV1 {
                        operation,
                        ..request
                    },
                    parent,
                ),
                Ok(())
            );
        }
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
            verify_abort_receipt(&request, digest, &receipt.encode().unwrap()),
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
                verify_abort_receipt(&request, digest, &hostile.encode().unwrap()),
                Err(TradingSbfError::ChildReceipt.into())
            );
        }
    }

    #[test]
    fn projected_lock_receipt_binds_source_replay_and_closures() {
        let request = ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            amount: 500,
            ..request()
        };
        let digest = hash(&request.encode().expect("Lock wire")).to_bytes();
        let accounts: Vec<_> = (0..ProjectedCustodyTerminalFrameV1::new(request.operation)
            .expect("Lock frame")
            .account_count())
            .map(|_| account_info())
            .collect();
        let receipt = ProjectedCustodyLockReceiptV1 {
            market: request.market,
            release_set: request.release_set,
            context_digest: request.context_digest,
            source_vault: request.funding_source_vault,
            source_replay: accounts[ProjectedCustodyTerminalFrameV1::LOCK_SOURCE_REPLAY]
                .key
                .to_bytes(),
            hoard_vault: request.hoard_vault,
            rent_credit: request.rent_credit,
            request_digest: digest,
            amount: request.amount,
            source_vault_rent_lamports: request.funding_source_vault_rent_lamports,
            source_replay_rent_lamports: request.funding_source_state_rent_lamports,
            resulting_revision: request.resulting_revision,
        };
        assert_eq!(
            verify_lock_receipt(
                &request,
                digest,
                &accounts,
                &receipt.encode().expect("receipt")
            ),
            Ok(())
        );
        assert_eq!(
            verify_lock_receipt(
                &request,
                digest,
                &accounts,
                &ProjectedCustodyLockReceiptV1 {
                    source_replay: [23; 32],
                    ..receipt
                }
                .encode()
                .expect("hostile receipt"),
            ),
            Err(TradingSbfError::ChildReceipt.into())
        );
    }

    #[test]
    fn projected_realize_receipt_binds_founded_market_poststate() {
        let request = ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::RealizeAndClose,
            amount: 500,
            ..request()
        };
        let digest = hash(&request.encode().expect("Realize wire")).to_bytes();
        let market_data = vec![7; 3];
        let mut accounts: Vec<_> = (0..ProjectedCustodyTerminalFrameV1::new(request.operation)
            .expect("Realize frame")
            .account_count())
            .map(|_| account_info())
            .collect();
        accounts[ProjectedCustodyTerminalFrameV1::REALIZE_MARKET] =
            account_info_with_data(market_data.clone());
        let receipt = ProjectedCustodyReceiptV1 {
            realized: true,
            aborted_open: false,
            market: request.market,
            release_set: request.release_set,
            parent_capability_root: request.parent_capability_root,
            context_digest: request.context_digest,
            hoard_vault: request.hoard_vault,
            amount: request.amount,
            request_digest: digest,
            market_state_digest: hash(&market_data).to_bytes(),
            rent_credit: request.rent_credit,
            resulting_revision: request.resulting_revision,
        };
        assert_eq!(
            verify_realize_receipt(
                &request,
                digest,
                &accounts,
                &receipt.encode().expect("receipt")
            ),
            Ok(())
        );
        assert_eq!(
            verify_realize_receipt(
                &request,
                digest,
                &accounts,
                &ProjectedCustodyReceiptV1 {
                    market_state_digest: [23; 32],
                    ..receipt
                }
                .encode()
                .expect("hostile receipt"),
            ),
            Err(TradingSbfError::ChildReceipt.into())
        );
    }
}
