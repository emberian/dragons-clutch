//! Small authenticated boundary for external Shadow accelerator callbacks.
//!
//! This module is deliberately separate from the full Hot interpreter so an
//! accelerator linking the read-only verifier does not retain Trading's state
//! transition and child-CPI orchestration code.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    shadow_digest_v3::{
        ShadowRuntimeObservationV3, family_request_digest_v3, runtime_observations_digest_v3,
    },
    shadow_v3::{SHADOW_RUNTIME_ACCOUNTS_START_V3, ShadowRequestV3},
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey,
};

use crate::{TradingSbfError, execution_strategy_v2::authenticate_current_deployment};

/// Public read-only facts authenticated for one Shadow accelerator callback.
///
/// The caller authority proves that the current release-selected Trading
/// program constructed the exact [`ShadowRequestV3`]. Runtime accounts retain
/// the logical AccountProfile order but are admitted only with downgraded
/// read-only privileges. This view grants no signer, write, or child-CPI
/// authority.
pub struct AuthenticatedShadowAcceleratorInvocationV4<'request, 'accounts, 'info> {
    request: ShadowRequestV3<'request>,
    request_digest: ContentId,
    activation: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    accelerator_programdata: &'accounts AccountInfo<'info>,
    runtime_accounts: &'accounts [AccountInfo<'info>],
}

impl<'request, 'accounts, 'info>
    AuthenticatedShadowAcceleratorInvocationV4<'request, 'accounts, 'info>
{
    /// Exact hostile-decoded Shadow request supplied by Trading.
    pub const fn request(&self) -> ShadowRequestV3<'request> {
        self.request
    }

    /// SHA-256 content identity of the complete exact request bytes.
    pub const fn request_digest(&self) -> ContentId {
        self.request_digest
    }

    /// Current release activation cache used to select Trading.
    pub const fn activation(&self) -> &'accounts AccountInfo<'info> {
        self.activation
    }

    /// Current Registry executable.
    pub const fn registry(&self) -> &'accounts AccountInfo<'info> {
        self.registry
    }

    /// Current release-selected Trading executable.
    pub const fn trading_program(&self) -> &'accounts AccountInfo<'info> {
        self.trading_program
    }

    /// Current immutable Trading ProgramData observation.
    pub const fn trading_programdata(&self) -> &'accounts AccountInfo<'info> {
        self.trading_programdata
    }

    /// ProgramData observation authenticated by Trading for this accelerator.
    pub const fn accelerator_programdata(&self) -> &'accounts AccountInfo<'info> {
        self.accelerator_programdata
    }

    /// Exact logical runtime accounts, all downgraded read-only and nonsigner.
    pub const fn runtime_accounts(&self) -> &'accounts [AccountInfo<'info>] {
        self.runtime_accounts
    }

    /// Join an AccountProfile-normalized logical transcript to the exact
    /// physical callback observations and the digest committed by Trading.
    ///
    /// AccountProfile remains the semantic owner of each logical `key`.
    /// This method authenticates every other observed field against the CPI
    /// accounts and then authenticates the complete logical transcript digest.
    pub fn validate_runtime_transcript(
        &self,
        observations: &[ShadowRuntimeObservationV3<'_>],
    ) -> Result<(), ProgramError> {
        if observations.len() != self.runtime_accounts.len() {
            return Err(TradingSbfError::Content.into());
        }
        let runtime_data = self
            .runtime_accounts
            .iter()
            .map(|runtime| {
                runtime
                    .try_borrow_data()
                    .map_err(|_| TradingSbfError::Content)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if observations
            .iter()
            .zip(self.runtime_accounts)
            .zip(&runtime_data)
            .any(|((observation, runtime), data)| {
                observation.owner != runtime.owner.to_bytes()
                    || observation.lamports != runtime.lamports()
                    || observation.data != data.as_ref()
                    || observation.signer
                    || observation.writable
                    || observation.executable != runtime.executable
            })
        {
            return Err(TradingSbfError::Content.into());
        }
        let digest =
            runtime_observations_digest_v3(observations).map_err(|_| TradingSbfError::Content)?;
        if digest != self.request.digests.runtime_observations {
            Err(TradingSbfError::Content.into())
        } else {
            Ok(())
        }
    }
}

/// Authenticate one external Shadow accelerator invocation without lending
/// mutation or child-CPI authority.
#[inline(never)]
pub fn authenticate_shadow_accelerator_invocation_v4<'request, 'accounts, 'info>(
    accelerator_program: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    request_bytes: &'request [u8],
) -> Result<Box<AuthenticatedShadowAcceleratorInvocationV4<'request, 'accounts, 'info>>, ProgramError>
{
    let request = ShadowRequestV3::decode(request_bytes).map_err(|_| TradingSbfError::Content)?;
    let account_count =
        usize::try_from(request.shape.account_count).map_err(|_| TradingSbfError::Content)?;
    let expected_accounts = SHADOW_RUNTIME_ACCOUNTS_START_V3
        .checked_add(account_count)
        .ok_or(TradingSbfError::Content)?;
    if accounts.len() != expected_accounts {
        return Err(TradingSbfError::Content.into());
    }
    let caller_authority = account(accounts, 0)?;
    let activation = account(accounts, 1)?;
    let registry = account(accounts, 2)?;
    let trading_program = account(accounts, 3)?;
    let trading_programdata = account(accounts, 4)?;
    let accelerator_programdata = account(accounts, 5)?;
    let runtime_accounts = accounts
        .get(SHADOW_RUNTIME_ACCOUNTS_START_V3..)
        .ok_or(TradingSbfError::Content)?;
    if request.accelerator_program.to_bytes() != accelerator_program.to_bytes()
        || request.registry_program.to_bytes() != registry.key.to_bytes()
        || request.trading_program.to_bytes() != trading_program.key.to_bytes()
        || caller_authority.is_writable
        || caller_authority.executable
        || !caller_authority.is_signer
        || activation.is_signer
        || activation.is_writable
        || activation.executable
        || !registry.executable
        || registry.is_signer
        || registry.is_writable
        || !trading_program.executable
        || trading_program.is_signer
        || trading_program.is_writable
        || trading_programdata.executable
        || trading_programdata.is_signer
        || trading_programdata.is_writable
        || accelerator_programdata.executable
        || accelerator_programdata.is_signer
        || accelerator_programdata.is_writable
    {
        return Err(TradingSbfError::Release.into());
    }
    require_shadow_callback_runtime_v4(
        request.root.to_bytes(),
        account_count,
        caller_authority.key,
        runtime_accounts,
    )?;

    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, request.release_set.as_bytes()],
        registry.key,
    )
    .0;
    if activation.key != &expected_cache || activation.owner != registry.key {
        return Err(TradingSbfError::Release.into());
    }
    let activation_data = activation
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation_data)
        .map_err(|_| TradingSbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| TradingSbfError::Release)?
        != request.release_set
    {
        return Err(TradingSbfError::Release.into());
    }
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| TradingSbfError::Release)?;
    let trading_release = trading.release();
    drop(activation_data);
    authenticate_current_deployment(trading_release, trading_program, trading_programdata)
        .map_err(ProgramError::from)?;

    let request_digest =
        ContentId::new(hash(request_bytes).to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let authority_seeds = CallerAuthoritySeedsV1::new(
        request.release_set,
        request.market.to_bytes(),
        ExecutionRoleV1::Trading,
        request.root.to_bytes(),
        request_digest.to_bytes(),
    )
    .map_err(|_| TradingSbfError::Release)?;
    let expected_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), trading_program.key).0;
    if caller_authority.key != &expected_authority {
        return Err(TradingSbfError::Release.into());
    }

    if family_request_digest_v3(request.family_request).map_err(|_| TradingSbfError::Content)?
        != request.digests.family_request
    {
        return Err(TradingSbfError::Content.into());
    }

    Ok(Box::new(AuthenticatedShadowAcceleratorInvocationV4 {
        request,
        request_digest,
        activation,
        registry,
        trading_program,
        trading_programdata,
        accelerator_programdata,
        runtime_accounts,
    }))
}

#[inline(never)]
fn require_shadow_callback_runtime_v4(
    expected_root: [u8; 32],
    expected_count: usize,
    caller_authority: &Pubkey,
    runtime_accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if runtime_accounts.len() != expected_count
        || runtime_accounts
            .first()
            .is_none_or(|runtime| runtime.key.to_bytes() != expected_root)
        || runtime_accounts.iter().any(|runtime| {
            runtime.is_signer || runtime.is_writable || runtime.key == caller_authority
        })
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec, vec::Vec};

    use super::*;

    fn readonly_test_account(key: Pubkey) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            false,
            Box::leak(Box::new(1_u64)),
            Box::leak(Vec::new().into_boxed_slice()),
            Box::leak(Box::new(Pubkey::new_unique())),
            false,
        )
    }

    #[test]
    fn runtime_is_exact_readonly_and_authority_disjoint() {
        let root = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let mut runtime = vec![
            readonly_test_account(root),
            readonly_test_account(Pubkey::new_unique()),
        ];
        require_shadow_callback_runtime_v4(root.to_bytes(), 2, &authority, &runtime)
            .expect("exact readonly runtime");
        assert!(
            require_shadow_callback_runtime_v4(root.to_bytes(), 1, &authority, &runtime).is_err()
        );
        runtime[1].is_writable = true;
        assert!(
            require_shadow_callback_runtime_v4(root.to_bytes(), 2, &authority, &runtime).is_err()
        );
        runtime[1].is_writable = false;
        let aliased_authority = *runtime[1].key;
        assert!(
            require_shadow_callback_runtime_v4(root.to_bytes(), 2, &aliased_authority, &runtime,)
                .is_err()
        );
    }
}
