//! Generic read-only Shadow-AOT execution for the common Trading V3 outer.
//!
//! Trading remains the sole interpreter, effect projector, child caller, and
//! state writer. The accelerator receives only a release-pinned caller PDA,
//! authenticated release/deployment evidence, exact read-only runtime
//! observations, and the complete family-neutral Shadow request. Its immediate
//! return data is comparison evidence, never mutation authority.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_execution_strategy_contract::shadow_v3::{
    SHADOW_ACK_BYTES_V3, SHADOW_RUNTIME_ACCOUNTS_START_V3, ShadowAckV3, ShadowDispositionV3,
    ShadowRequestV3,
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

use crate::TradingSbfError;

const SHADOW_ACK_TRANSCRIPT_DOMAIN_V3: &[u8] = b"dclutch:hot-shadow-ack:v3";

/// Exact fixed read-only evidence passed before the logical runtime slice.
#[derive(Clone, Copy)]
pub(crate) struct ShadowCpiFrameV3<'a, 'info> {
    pub(crate) caller_authority: &'a AccountInfo<'info>,
    pub(crate) activation: &'a AccountInfo<'info>,
    pub(crate) registry: &'a AccountInfo<'info>,
    pub(crate) trading_program: &'a AccountInfo<'info>,
    pub(crate) trading_programdata: &'a AccountInfo<'info>,
    pub(crate) accelerator_program: &'a AccountInfo<'info>,
    pub(crate) accelerator_programdata: &'a AccountInfo<'info>,
}

/// Execute one exact Shadow comparison and return its transcript commitment.
pub(crate) fn execute_shadow_aot_v3<'info>(
    program_id: &Pubkey,
    frame: ShadowCpiFrameV3<'_, 'info>,
    runtime_accounts: &[&AccountInfo<'info>],
    request: ShadowRequestV3<'_>,
) -> Result<[u8; 32], ProgramError> {
    validate_frame(program_id, frame, runtime_accounts, request)?;
    let request_len = request
        .encoded_len()
        .map_err(|_| TradingSbfError::Content)?;
    let mut request_bytes = vec![0_u8; request_len];
    request
        .encode_into(&mut request_bytes)
        .map_err(|_| TradingSbfError::Content)?;
    let request_digest_bytes = hash(&request_bytes).to_bytes();
    let request_digest = dclutch_core_contract::ContentId::new(request_digest_bytes)
        .map_err(|_| TradingSbfError::Content)?;
    let authority_seeds = CallerAuthoritySeedsV1::new(
        request.release_set,
        request.market.to_bytes(),
        ExecutionRoleV1::Trading,
        request.root.to_bytes(),
        request_digest_bytes,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected_authority, bump) =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if frame.caller_authority.key != &expected_authority {
        return Err(TradingSbfError::Release.into());
    }

    let mut metas = Vec::with_capacity(
        SHADOW_RUNTIME_ACCOUNTS_START_V3
            .checked_add(runtime_accounts.len())
            .ok_or(TradingSbfError::Content)?,
    );
    metas.extend_from_slice(&[
        AccountMeta::new_readonly(*frame.caller_authority.key, true),
        AccountMeta::new_readonly(*frame.activation.key, false),
        AccountMeta::new_readonly(*frame.registry.key, false),
        AccountMeta::new_readonly(*frame.trading_program.key, false),
        AccountMeta::new_readonly(*frame.trading_programdata.key, false),
        AccountMeta::new_readonly(*frame.accelerator_programdata.key, false),
    ]);
    metas.extend(
        runtime_accounts
            .iter()
            .map(|account| AccountMeta::new_readonly(*account.key, false)),
    );
    let instruction = Instruction {
        program_id: *frame.accelerator_program.key,
        accounts: metas,
        data: request_bytes,
    };
    let mut infos = Vec::with_capacity(
        SHADOW_RUNTIME_ACCOUNTS_START_V3
            .checked_add(runtime_accounts.len())
            .and_then(|count| count.checked_add(1))
            .ok_or(TradingSbfError::Content)?,
    );
    infos.extend_from_slice(&[
        frame.caller_authority.clone(),
        frame.activation.clone(),
        frame.registry.clone(),
        frame.trading_program.clone(),
        frame.trading_programdata.clone(),
        frame.accelerator_programdata.clone(),
    ]);
    infos.extend(runtime_accounts.iter().map(|account| (*account).clone()));
    infos.push(frame.accelerator_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let (producer, ack_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *frame.accelerator_program.key || ack_bytes.len() != SHADOW_ACK_BYTES_V3 {
        return Err(TradingSbfError::Transition.into());
    }
    let ack = ShadowAckV3::decode(&ack_bytes).map_err(|_| TradingSbfError::Transition)?;
    ack.validate_for(request, request_digest, request.accelerator_program)
        .map_err(|_| TradingSbfError::Transition)?;
    if ack.disposition() != ShadowDispositionV3::Accepted {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(hashv(&[
        SHADOW_ACK_TRANSCRIPT_DOMAIN_V3,
        &request_digest_bytes,
        &ack_bytes,
    ])
    .to_bytes())
}

fn validate_frame(
    program_id: &Pubkey,
    frame: ShadowCpiFrameV3<'_, '_>,
    runtime_accounts: &[&AccountInfo<'_>],
    request: ShadowRequestV3<'_>,
) -> Result<(), ProgramError> {
    let account_count =
        u32::try_from(runtime_accounts.len()).map_err(|_| TradingSbfError::Content)?;
    let runtime_aliases_authority = runtime_keys_alias_authority(
        frame.caller_authority.key,
        runtime_accounts.iter().map(|account| account.key),
    );
    if request.trading_program.to_bytes() != program_id.to_bytes()
        || request.registry_program.to_bytes() != frame.registry.key.to_bytes()
        || request.accelerator_program.to_bytes() != frame.accelerator_program.key.to_bytes()
        || request.root.to_bytes()
            != runtime_accounts
                .first()
                .ok_or(TradingSbfError::Content)?
                .key
                .to_bytes()
        || request.shape.account_count != account_count
        || runtime_aliases_authority
        || frame.caller_authority.is_signer
        || frame.caller_authority.is_writable
        || frame.caller_authority.executable
        || frame.activation.is_signer
        || frame.activation.is_writable
        || frame.activation.executable
        || !frame.registry.executable
        || frame.registry.is_signer
        || frame.registry.is_writable
        || !frame.trading_program.executable
        || frame.trading_program.key != program_id
        || frame.trading_program.is_signer
        || frame.trading_program.is_writable
        || frame.trading_programdata.executable
        || frame.trading_programdata.is_signer
        || frame.trading_programdata.is_writable
        || !frame.accelerator_program.executable
        || frame.accelerator_program.is_signer
        || frame.accelerator_program.is_writable
        || frame.accelerator_programdata.executable
        || frame.accelerator_programdata.is_signer
        || frame.accelerator_programdata.is_writable
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(())
}

fn runtime_keys_alias_authority<'a>(
    authority: &Pubkey,
    runtime_keys: impl Iterator<Item = &'a Pubkey>,
) -> bool {
    runtime_keys.into_iter().any(|key| key == authority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_core_contract::ContentId;
    use dclutch_execution_strategy_contract::shadow_v3::{
        ShadowArtifactTupleV3, ShadowExecutionDigestsV3, ShadowRuntimeShapeV3,
    };

    fn id(tag: u8) -> ContentId {
        ContentId::new([tag; 32]).expect("nonzero")
    }

    #[test]
    fn request_digest_selects_one_release_pinned_trading_authority() {
        let family = [1_u8; 32];
        let request = ShadowRequestV3 {
            release_set: id(2),
            market: id(3),
            root: id(4),
            registry_program: id(5),
            trading_program: id(6),
            accelerator_program: id(7),
            artifacts: ShadowArtifactTupleV3 {
                capability_program: id(8),
                account_profile: id(9),
                request_profile: id(10),
                transition: id(11),
                effect: id(12),
                strategy: id(13),
                certificate: id(14),
            },
            invocation_context: id(15),
            digests: ShadowExecutionDigestsV3 {
                runtime_observations: id(16),
                family_request: id(17),
                interpreted_candidate: id(18),
                interpreted_effect: id(19),
            },
            shape: ShadowRuntimeShapeV3 {
                tail_count: 2,
                account_count: 3,
                scalar_count: 4,
                identity_count: 5,
            },
            family_request: &family,
        };
        let mut encoded = vec![0_u8; request.encoded_len().expect("width")];
        request.encode_into(&mut encoded).expect("request");
        let digest = hash(&encoded).to_bytes();
        let seeds = CallerAuthoritySeedsV1::new(
            request.release_set,
            request.market.to_bytes(),
            ExecutionRoleV1::Trading,
            request.root.to_bytes(),
            digest,
        )
        .expect("authority seeds");
        let program = Pubkey::new_from_array(request.trading_program.to_bytes());
        let (first, _) = Pubkey::find_program_address(&seeds.as_slices(), &program);

        encoded.push(0);
        let changed_digest = hash(&encoded).to_bytes();
        let changed = CallerAuthoritySeedsV1::new(
            request.release_set,
            request.market.to_bytes(),
            ExecutionRoleV1::Trading,
            request.root.to_bytes(),
            changed_digest,
        )
        .expect("changed seeds");
        let (second, _) = Pubkey::find_program_address(&changed.as_slices(), &program);
        assert_ne!(first, second);
    }

    #[test]
    fn caller_authority_cannot_reappear_in_readonly_runtime_slice() {
        let authority = Pubkey::new_unique();
        let other = Pubkey::new_unique();
        assert!(!runtime_keys_alias_authority(
            &authority,
            [&other].into_iter()
        ));
        assert!(runtime_keys_alias_authority(
            &authority,
            [&other, &authority].into_iter()
        ));
    }
}
