//! The family-neutral producer for Trading's validated-artifact seal.
//!
//! # Why this exists
//!
//! `process_capability_seal_v1` is PERMISSIONLESS: it reads six finalized
//! artifact records, mints one `SealedDescriptorClosureV1` over them, and writes
//! it at an address derived from the descriptor, the action, the Trading
//! semantic release and the Registry. Anybody willing to pay its rent may run
//! it, and no family owns it.
//!
//! Its only host builder was Direct's
//! (`direct_inline_route_v3::compile_direct_inline_capability_seal_plan_v3`),
//! which hard-codes `DirectExecutionActionV3::InlineOrdinary` and reads its
//! frame out of an authenticated Direct route. So on 2026-09-03
//! `devnet-general-session` reported cohort-14's General seal at fixed
//! coordinate 38 as **producible and unproduced** -- a route that exists, that
//! anybody may call, and that nothing in the tree could compose an instruction
//! for. That is the producer-missing shape: a reader, a schema and a refusal all
//! built, and only the failure path ever exercised.
//!
//! # What it derives and what it refuses
//!
//! The seal ADDRESS is derived here, never supplied, and the caller's frame must
//! already name it at coordinate 38. That single conjunct is the whole reason
//! this is a builder rather than a struct literal: `CapabilitySealRequestV1`
//! names only the descriptor and the action, both of which are seeds, so a
//! request naming the wrong one produces a truthful verdict at an address no hot
//! action derives -- never a false verdict at the right one. A caller that
//! assembles the account list by hand gets no warning at all.
//!
//! It does NOT re-derive the seal body. Six finalized record pairs and their
//! staging cursors are authenticated by the executing Program from accounts it
//! reads itself, and a host that reproduced that verdict would be a second
//! authority for it. Direct's builder computes an `expected_body` because it has
//! a further job -- deciding whether an existing seal is already materialized --
//! and that is a Direct concern, not a seal concern.

use dclutch_market::capability_program::hot_v3::{
    HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
};
use dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4;
use dclutch_vm::capability_seal::{CapabilitySealKeyV1, CapabilitySealRequestV1};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Stable refusal from composing one validated-artifact seal instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilitySealBuilderErrorV1 {
    /// The fixed frame is not exactly `HOT_FIXED_ACCOUNT_COUNT_V3` accounts.
    FixedFrame,
    /// The descriptor digest or the Trading semantic release is the reserved zero.
    ZeroIdentity,
    /// The frame's coordinate 38 is not the address these seeds derive.
    ///
    /// Named separately from `FixedFrame` because the remedy is different: the
    /// frame is the right shape and names the wrong seal, which is the one way
    /// a seal writes a truthful verdict nothing will ever read.
    SealCoordinate,
    /// The payer is one of the fixed frame's own accounts.
    AliasedPayer,
    /// `dclutch_vm::capability_seal` refused; the cause is its own.
    CapabilitySeal(dclutch_vm::capability_seal::Error),
}

/// Everything one seal instruction needs that cannot be derived from the rest.
#[derive(Clone, Copy, Debug)]
pub struct CapabilitySealInstructionInputV1<'a> {
    /// Current release-selected Trading program, which owns the seal PDA.
    pub trading_program: Pubkey,
    /// Immutable Registry program, a seed of the seal address.
    pub registry_program: Pubkey,
    /// The Trading role's semantic release identity, a seed of the seal address.
    pub trading_semantic_release: [u8; 32],
    /// SHA-256 of the exact selected `CapabilityProgramV4` descriptor.
    pub descriptor_digest: [u8; 32],
    /// The action selector this seal is filed under.
    ///
    /// FAMILY-NEUTRAL AND THEREFORE A PARAMETER. Direct's builder spells
    /// `DirectExecutionActionV3::InlineOrdinary` here; General's `OpenBatch`,
    /// Series's and Dealer's are different numbers under the same route.
    pub action: u32,
    /// The complete common Hot fixed frame, in `HOT_*_ACCOUNT_V3` order.
    pub fixed_frame: &'a [Pubkey],
    /// Whoever is paying the seal's rent, and the instruction's only signer.
    pub payer: Pubkey,
}

/// One composed seal instruction and the address it will write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilitySealInstructionV1 {
    /// The derived seal address; equal to the frame's coordinate 38.
    pub seal: Pubkey,
    /// Canonical bump for `seal`, which the executing Program re-derives.
    pub bump: u8,
    /// The instruction, ready to sign.
    pub instruction: Instruction,
}

/// Compose the permissionless validated-artifact seal instruction.
pub fn capability_seal_instruction_v1(
    input: CapabilitySealInstructionInputV1<'_>,
) -> Result<CapabilitySealInstructionV1, CapabilitySealBuilderErrorV1> {
    if input.fixed_frame.len() != HOT_FIXED_ACCOUNT_COUNT_V3 {
        return Err(CapabilitySealBuilderErrorV1::FixedFrame);
    }
    let key = CapabilitySealKeyV1::new(
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        input.descriptor_digest,
        input.action,
        input.trading_semantic_release,
        input.registry_program.to_bytes(),
    )
    .map_err(CapabilitySealBuilderErrorV1::CapabilitySeal)?;
    let (seal, bump) =
        Pubkey::find_program_address(&key.seeds().as_slices(), &input.trading_program);
    let stated = input
        .fixed_frame
        .get(HOT_CAPABILITY_SEAL_ACCOUNT_V3)
        .ok_or(CapabilitySealBuilderErrorV1::FixedFrame)?;
    if *stated != seal {
        return Err(CapabilitySealBuilderErrorV1::SealCoordinate);
    }
    // The payer signs and is writable; every fixed coordinate is read-only
    // except the seal. A payer that is also a fixed coordinate would silently
    // grant that coordinate signer and writable privileges through the
    // duplicate-account merge, which is the one way this frame can hand out an
    // authority nobody wrote down.
    if input.fixed_frame.contains(&input.payer) {
        return Err(CapabilitySealBuilderErrorV1::AliasedPayer);
    }
    let mut accounts = input
        .fixed_frame
        .iter()
        .map(|key| AccountMeta {
            pubkey: *key,
            is_signer: false,
            is_writable: *key == seal,
        })
        .collect::<Vec<_>>();
    accounts.push(AccountMeta::new(input.payer, true));
    accounts.push(AccountMeta::new_readonly(
        solana_sdk_ids::system_program::ID,
        false,
    ));
    let request = CapabilitySealRequestV1::new(input.action, input.descriptor_digest)
        .map_err(CapabilitySealBuilderErrorV1::CapabilitySeal)?;
    Ok(CapabilitySealInstructionV1 {
        seal,
        bump,
        instruction: Instruction {
            program_id: input.trading_program,
            accounts,
            data: request.to_bytes().to_vec(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(seal: Pubkey) -> Vec<Pubkey> {
        let mut frame = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|_| Pubkey::new_unique())
            .collect::<Vec<_>>();
        frame[HOT_CAPABILITY_SEAL_ACCOUNT_V3] = seal;
        frame
    }

    fn input<'a>(fixed: &'a [Pubkey], payer: Pubkey) -> CapabilitySealInstructionInputV1<'a> {
        CapabilitySealInstructionInputV1 {
            trading_program: Pubkey::new_from_array([0x11; 32]),
            registry_program: Pubkey::new_from_array([0x12; 32]),
            trading_semantic_release: [0x13; 32],
            descriptor_digest: [0x14; 32],
            action: 7,
            fixed_frame: fixed,
            payer,
        }
    }

    fn seal_for(action: u32, descriptor_digest: [u8; 32]) -> Pubkey {
        let key = CapabilitySealKeyV1::new(
            CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            descriptor_digest,
            action,
            [0x13; 32],
            [0x12; 32],
        )
        .expect("seal key");
        Pubkey::find_program_address(
            &key.seeds().as_slices(),
            &Pubkey::new_from_array([0x11; 32]),
        )
        .0
    }

    /// The frame is the fixed prefix, one writable coordinate, one signer.
    #[test]
    fn the_composed_frame_writes_only_the_seal_and_signs_only_the_payer() {
        let seal = seal_for(7, [0x14; 32]);
        let fixed = frame(seal);
        let payer = Pubkey::new_unique();
        let composed = capability_seal_instruction_v1(input(&fixed, payer)).expect("composed");
        assert_eq!(composed.seal, seal);
        assert_eq!(
            composed.instruction.accounts.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3 + 2
        );
        let writable = composed
            .instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_writable)
            .map(|meta| meta.pubkey)
            .collect::<Vec<_>>();
        assert_eq!(writable, vec![seal, payer]);
        let signers = composed
            .instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_signer)
            .map(|meta| meta.pubkey)
            .collect::<Vec<_>>();
        assert_eq!(signers, vec![payer]);
        assert_eq!(
            composed.instruction.accounts.last().expect("suffix").pubkey,
            solana_sdk_ids::system_program::ID
        );
        assert_eq!(
            CapabilitySealRequestV1::decode(&composed.instruction.data).expect("request"),
            CapabilitySealRequestV1::new(7, [0x14; 32]).expect("canonical")
        );
    }

    /// THE ADDRESS IS A FUNCTION OF THE ACTION, which is what makes this
    /// family-neutral rather than a Direct builder with a parameter added.
    #[test]
    fn a_second_action_over_one_descriptor_names_a_second_seal() {
        let first = seal_for(7, [0x14; 32]);
        let second = seal_for(8, [0x14; 32]);
        assert_ne!(first, second);
        let fixed = frame(second);
        let payer = Pubkey::new_unique();
        // The frame names action 8's seal and the request says 7: a caller that
        // assembled this by hand would write a truthful verdict at an address no
        // hot action derives, and the on-chain refusal would be about a seal
        // rather than about the action that asked for it.
        assert_eq!(
            capability_seal_instruction_v1(input(&fixed, payer)),
            Err(CapabilitySealBuilderErrorV1::SealCoordinate)
        );
        assert_eq!(
            capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
                action: 8,
                ..input(&fixed, payer)
            })
            .expect("action 8")
            .seal,
            second
        );
    }

    /// Every hostile the builder can name, named.
    #[test]
    fn a_short_frame_a_zero_identity_and_an_aliased_payer_each_refuse() {
        let seal = seal_for(7, [0x14; 32]);
        let fixed = frame(seal);
        let payer = Pubkey::new_unique();
        assert_eq!(
            capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
                fixed_frame: &fixed[..HOT_FIXED_ACCOUNT_COUNT_V3 - 1],
                ..input(&fixed, payer)
            }),
            Err(CapabilitySealBuilderErrorV1::FixedFrame)
        );
        assert_eq!(
            capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
                descriptor_digest: [0; 32],
                ..input(&fixed, payer)
            }),
            Err(CapabilitySealBuilderErrorV1::CapabilitySeal(
                dclutch_vm::capability_seal::Error::ZeroIdentity
            ))
        );
        assert_eq!(
            capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
                trading_semantic_release: [0; 32],
                ..input(&fixed, payer)
            }),
            Err(CapabilitySealBuilderErrorV1::CapabilitySeal(
                dclutch_vm::capability_seal::Error::ZeroIdentity
            ))
        );
        assert_eq!(
            capability_seal_instruction_v1(input(&fixed, fixed[0])),
            Err(CapabilitySealBuilderErrorV1::AliasedPayer)
        );
    }
}
