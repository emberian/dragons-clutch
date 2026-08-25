//! Chain-derived construction for the compiled Direct two-instruction batch.

use crate::{Finality, Observation, ObservedAccount};
use dclutch_direct_codec::{
    COMPACT_INTENT_BYTES, CompactIntentV1, ControllerInstructionV1, MarketProfileV1,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{ed25519_program, sysvar};

/// Global compiled-Direct controller authority seed.
pub const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
/// Compiled-Direct replay-root seed.
pub const REPLAY_SEED: &[u8] = b"dclutch/direct-replay/v3";
/// Compiled-Direct maker/outcome Position seed.
pub const POSITION_SEED: &[u8] = b"dclutch/position/v1";
/// Pinned experimental claim-child identity.
pub const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
/// Pinned real custody-child identity.
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);

const ED_DESCRIPTOR_BYTES: usize = 14;
const ED_PAYLOAD_OFFSET: usize = 2 + 2 * ED_DESCRIPTOR_BYTES;
const SELLER_MESSAGE_OFFSET: usize = 32;
const BUYER_MESSAGE_OFFSET: usize = 168;

/// Untrusted signature material paired with the exact intent it purports to sign.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedCompactIntentV1 {
    /// Native Ed25519 public key that owns maker identity.
    pub maker: Pubkey,
    /// Detached Ed25519 signature over the exact encoded compact intent.
    pub signature: [u8; 64],
    /// Exact reusable limit intent.
    pub intent: CompactIntentV1,
}

/// Same-finalized chain state required to construct one compiled Direct frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledDirectState {
    /// Controller PDA account.
    pub controller: ObservedAccount,
    /// Seller canonical replay root.
    pub seller_replay: ObservedAccount,
    /// Buyer canonical replay root.
    pub buyer_replay: ObservedAccount,
    /// Controller-owned transaction journal.
    pub journal: ObservedAccount,
    /// Seller canonical maker/outcome Position.
    pub seller_position: ObservedAccount,
    /// Buyer canonical maker/outcome Position.
    pub buyer_position: ObservedAccount,
    /// Pinned executable claim child.
    pub claim_program: ObservedAccount,
    /// Pinned executable custody child.
    pub custody_program: ObservedAccount,
    /// Read-only execution profile selected by both intents.
    pub execution_profile: ObservedAccount,
    /// Realm-selected collateral mint.
    pub mint: ObservedAccount,
    /// Buyer collateral source selected by the buyer intent.
    pub buyer_source: ObservedAccount,
    /// Seller collateral destination selected by the seller intent.
    pub seller_destination: ObservedAccount,
    /// Profile-selected fee destination.
    pub fee_destination: ObservedAccount,
    /// Profile-selected executable token program.
    pub token_program: ObservedAccount,
}

/// Matcher coordinates; admission remains solely onchain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchCoordinatesV1 {
    /// Proposed fill.
    pub fill: u64,
    /// Proposed execution price at the profile scale.
    pub execution_price: u64,
}

/// Exact native-Ed25519 plus controller transaction material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledDirectReport {
    /// Native Ed25519 verification followed by compiled controller execution.
    pub instructions: [Instruction; 2],
    /// Same finalized observation that selected all accounts.
    pub observation: Observation,
    /// Derived global controller PDA.
    pub controller: Pubkey,
    /// Derived seller replay root.
    pub seller_replay: Pubkey,
    /// Derived buyer replay root.
    pub buyer_replay: Pubkey,
    /// Derived seller Position.
    pub seller_position: Pubkey,
    /// Derived buyer Position.
    pub buyer_position: Pubkey,
}

/// Refusal from stale, inconsistent, or noncanonical chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// One input was not finalized.
    ObservationNotFinalized,
    /// Inputs did not share one exact observation.
    ObservationMismatch,
    /// Execution-profile bytes were not canonical.
    InvalidProfile,
    /// A program/state owner or executable bit was incompatible.
    InvalidAccount,
    /// Signed intent/profile bindings differed.
    IntentBinding,
    /// Maker keys or detached signatures were invalid at the structural layer.
    SignatureMaterial,
    /// An observed canonical PDA differed from its derivation.
    PdaMismatch,
    /// Fixed instruction encoding failed.
    Encoding,
}

/// Build, but never sign or submit, the exact compiled Direct instruction pair.
///
/// Detached signatures remain untrusted until the native Ed25519 instruction
/// executes. This builder validates structural material and derives every PDA;
/// economic admission remains exclusively in the compiled onchain transition.
pub fn build_compiled_direct(
    controller_program: Pubkey,
    state: &CompiledDirectState,
    seller: SignedCompactIntentV1,
    buyer: SignedCompactIntentV1,
    coordinates: MatchCoordinatesV1,
) -> Result<CompiledDirectReport, Error> {
    let observation = same_finalized_observation(state)?;
    let profile = MarketProfileV1::decode(&state.execution_profile.data)
        .map_err(|_| Error::InvalidProfile)?;
    validate_program_accounts(controller_program, state, profile)?;
    if seller.maker == Pubkey::default()
        || buyer.maker == Pubkey::default()
        || seller.maker == buyer.maker
        || seller.signature.iter().all(|byte| *byte == 0)
        || buyer.signature.iter().all(|byte| *byte == 0)
    {
        return Err(Error::SignatureMaterial);
    }
    let profile_key = state.execution_profile.key.to_bytes();
    if seller.intent.execution_profile != profile_key
        || buyer.intent.execution_profile != profile_key
        || seller.intent.generation != profile.generation
        || buyer.intent.generation != profile.generation
        || seller.intent.fee_basis_points != profile.fee_basis_points
        || buyer.intent.fee_basis_points != profile.fee_basis_points
        || seller.intent.collateral_account != state.seller_destination.key.to_bytes()
        || buyer.intent.collateral_account != state.buyer_source.key.to_bytes()
    {
        return Err(Error::IntentBinding);
    }

    let (controller, controller_bump) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &controller_program);
    let generation = profile.generation.to_le_bytes();
    let (seller_replay, seller_replay_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            state.execution_profile.key.as_ref(),
            &generation,
            seller.maker.as_ref(),
        ],
        &controller_program,
    );
    let (buyer_replay, buyer_replay_bump) = Pubkey::find_program_address(
        &[
            REPLAY_SEED,
            state.execution_profile.key.as_ref(),
            &generation,
            buyer.maker.as_ref(),
        ],
        &controller_program,
    );
    let seller_outcome = [seller.intent.outcome];
    let buyer_outcome = [buyer.intent.outcome];
    let (seller_position, seller_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            state.execution_profile.key.as_ref(),
            seller.maker.as_ref(),
            &seller_outcome,
        ],
        &controller_program,
    );
    let (buyer_position, buyer_position_bump) = Pubkey::find_program_address(
        &[
            POSITION_SEED,
            state.execution_profile.key.as_ref(),
            buyer.maker.as_ref(),
            &buyer_outcome,
        ],
        &controller_program,
    );
    if state.controller.key != controller
        || state.seller_replay.key != seller_replay
        || state.buyer_replay.key != buyer_replay
        || state.seller_position.key != seller_position
        || state.buyer_position.key != buyer_position
    {
        return Err(Error::PdaMismatch);
    }

    let controller_data = ControllerInstructionV1 {
        controller_bump,
        seller_replay_bump,
        buyer_replay_bump,
        seller_position_bump,
        buyer_position_bump,
        fill: coordinates.fill,
        execution_price: coordinates.execution_price,
        seller: seller.intent,
        buyer: buyer.intent,
    }
    .encode()
    .map_err(|_| Error::Encoding)?;
    let controller_instruction = Instruction {
        program_id: controller_program,
        accounts: vec![
            AccountMeta::new_readonly(controller, false),
            AccountMeta::new(seller_replay, false),
            AccountMeta::new(buyer_replay, false),
            AccountMeta::new(state.journal.key, false),
            AccountMeta::new(seller_position, false),
            AccountMeta::new(buyer_position, false),
            AccountMeta::new_readonly(CLAIM_PROGRAM_ID, false),
            AccountMeta::new_readonly(CUSTODY_PROGRAM_ID, false),
            AccountMeta::new_readonly(state.execution_profile.key, false),
            AccountMeta::new_readonly(state.mint.key, false),
            AccountMeta::new(state.buyer_source.key, false),
            AccountMeta::new(state.seller_destination.key, false),
            AccountMeta::new(state.fee_destination.key, false),
            AccountMeta::new_readonly(state.token_program.key, false),
            AccountMeta::new_readonly(sysvar::instructions::ID, false),
        ],
        data: controller_data.to_vec(),
    };
    let signature_instruction = ed25519_batch(seller, buyer, &controller_data)?;

    Ok(CompiledDirectReport {
        instructions: [signature_instruction, controller_instruction],
        observation,
        controller,
        seller_replay,
        buyer_replay,
        seller_position,
        buyer_position,
    })
}

fn same_finalized_observation(state: &CompiledDirectState) -> Result<Observation, Error> {
    let accounts = [
        &state.controller,
        &state.seller_replay,
        &state.buyer_replay,
        &state.journal,
        &state.seller_position,
        &state.buyer_position,
        &state.claim_program,
        &state.custody_program,
        &state.execution_profile,
        &state.mint,
        &state.buyer_source,
        &state.seller_destination,
        &state.fee_destination,
        &state.token_program,
    ];
    let observation = state.execution_profile.observation;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
    {
        return Err(Error::ObservationNotFinalized);
    }
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(observation)
}

fn validate_program_accounts(
    controller_program: Pubkey,
    state: &CompiledDirectState,
    profile: MarketProfileV1,
) -> Result<(), Error> {
    if state.controller.executable
        || state.seller_replay.owner != CLAIM_PROGRAM_ID
        || state.seller_replay.executable
        || state.buyer_replay.owner != CLAIM_PROGRAM_ID
        || state.buyer_replay.executable
        || state.seller_position.owner != CLAIM_PROGRAM_ID
        || state.seller_position.executable
        || state.buyer_position.owner != CLAIM_PROGRAM_ID
        || state.buyer_position.executable
        || state.journal.owner != controller_program
        || state.journal.executable
        || state.execution_profile.owner != controller_program
        || state.execution_profile.executable
        || state.claim_program.key != CLAIM_PROGRAM_ID
        || !state.claim_program.executable
        || state.custody_program.key != CUSTODY_PROGRAM_ID
        || !state.custody_program.executable
        || state.token_program.key.to_bytes() != profile.token_program
        || !state.token_program.executable
        || state.mint.key.to_bytes() != profile.collateral_mint
        || state.mint.owner != state.token_program.key
        || state.mint.executable
        || state.buyer_source.owner != state.token_program.key
        || state.buyer_source.executable
        || state.seller_destination.owner != state.token_program.key
        || state.seller_destination.executable
        || state.fee_destination.key.to_bytes() != profile.fee_recipient
        || state.fee_destination.owner != state.token_program.key
        || state.fee_destination.executable
    {
        return Err(Error::InvalidAccount);
    }
    Ok(())
}

fn ed25519_batch(
    seller: SignedCompactIntentV1,
    buyer: SignedCompactIntentV1,
    controller_data: &[u8],
) -> Result<Instruction, Error> {
    let mut data = vec![0_u8; ED_PAYLOAD_OFFSET + 2 * 96];
    put_u16(&mut data, 0, 2)?;
    for (index, material, message_offset) in [
        (0_usize, seller, SELLER_MESSAGE_OFFSET),
        (1_usize, buyer, BUYER_MESSAGE_OFFSET),
    ] {
        let descriptor = 2 + index * ED_DESCRIPTOR_BYTES;
        let public_key_offset = ED_PAYLOAD_OFFSET + index * 96;
        let signature_offset = public_key_offset + 32;
        put_u16(&mut data, descriptor, to_u16(signature_offset)?)?;
        put_u16(&mut data, descriptor + 2, u16::MAX)?;
        put_u16(&mut data, descriptor + 4, to_u16(public_key_offset)?)?;
        put_u16(&mut data, descriptor + 6, u16::MAX)?;
        put_u16(&mut data, descriptor + 8, to_u16(message_offset)?)?;
        put_u16(&mut data, descriptor + 10, to_u16(COMPACT_INTENT_BYTES)?)?;
        put_u16(&mut data, descriptor + 12, 1)?;
        put(&mut data, public_key_offset, material.maker.as_ref())?;
        put(&mut data, signature_offset, &material.signature)?;
        let end = message_offset
            .checked_add(COMPACT_INTENT_BYTES)
            .ok_or(Error::Encoding)?;
        if controller_data.get(message_offset..end).is_none() {
            return Err(Error::Encoding);
        }
    }
    Ok(Instruction {
        program_id: ed25519_program::ID,
        accounts: vec![],
        data,
    })
}

fn to_u16(value: usize) -> Result<u16, Error> {
    u16::try_from(value).map_err(|_| Error::Encoding)
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) -> Result<(), Error> {
    put(output, offset, &value.to_le_bytes())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::Encoding)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Encoding)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk_ids::system_program;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn observed(
        observation: Observation,
        key: Pubkey,
        owner: Pubkey,
        executable: bool,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation,
            key,
            owner,
            lamports: 1_000_000,
            executable,
            data,
        }
    }

    fn intent(profile: Pubkey, collateral: Pubkey, side: u8) -> CompactIntentV1 {
        CompactIntentV1 {
            side,
            outcome: 1,
            lifecycle: 0,
            execution_profile: profile.to_bytes(),
            generation: 3,
            nonce: 0,
            valid_from: 0,
            valid_through: u64::MAX,
            maximum_fill: 2_000,
            limit_price: if side == 0 { 400_000 } else { 600_000 },
            fee_basis_points: 25,
            collateral_account: collateral.to_bytes(),
        }
    }

    fn fixture() -> (
        Pubkey,
        CompiledDirectState,
        SignedCompactIntentV1,
        SignedCompactIntentV1,
    ) {
        let observation = Observation {
            slot: 55,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let program = key(67);
        let seller = key(1);
        let buyer = key(2);
        let profile_key = key(4);
        let seller_destination = key(5);
        let buyer_source = key(6);
        let token_program = key(7);
        let mint = key(8);
        let fee_destination = key(9);
        let generation = 3_u64.to_le_bytes();
        let outcome = [1_u8];
        let (controller, _) = Pubkey::find_program_address(&[CONTROLLER_SEED], &program);
        let (seller_replay, _) = Pubkey::find_program_address(
            &[
                REPLAY_SEED,
                profile_key.as_ref(),
                &generation,
                seller.as_ref(),
            ],
            &program,
        );
        let (buyer_replay, _) = Pubkey::find_program_address(
            &[
                REPLAY_SEED,
                profile_key.as_ref(),
                &generation,
                buyer.as_ref(),
            ],
            &program,
        );
        let (seller_position, _) = Pubkey::find_program_address(
            &[
                POSITION_SEED,
                profile_key.as_ref(),
                seller.as_ref(),
                &outcome,
            ],
            &program,
        );
        let (buyer_position, _) = Pubkey::find_program_address(
            &[
                POSITION_SEED,
                profile_key.as_ref(),
                buyer.as_ref(),
                &outcome,
            ],
            &program,
        );
        let profile = MarketProfileV1 {
            phase: 1,
            outcome_count: 2,
            generation: 3,
            price_scale: 1_000_000,
            fee_basis_points: 25,
            token_program: token_program.to_bytes(),
            collateral_mint: mint.to_bytes(),
            fee_recipient: fee_destination.to_bytes(),
        }
        .encode()
        .expect("profile");
        let state = CompiledDirectState {
            controller: observed(observation, controller, system_program::ID, false, vec![]),
            seller_replay: observed(observation, seller_replay, CLAIM_PROGRAM_ID, false, vec![]),
            buyer_replay: observed(observation, buyer_replay, CLAIM_PROGRAM_ID, false, vec![]),
            journal: observed(observation, key(10), program, false, vec![]),
            seller_position: observed(
                observation,
                seller_position,
                CLAIM_PROGRAM_ID,
                false,
                vec![],
            ),
            buyer_position: observed(observation, buyer_position, CLAIM_PROGRAM_ID, false, vec![]),
            claim_program: observed(observation, CLAIM_PROGRAM_ID, key(99), true, vec![]),
            custody_program: observed(observation, CUSTODY_PROGRAM_ID, key(99), true, vec![]),
            execution_profile: observed(observation, profile_key, program, false, profile.to_vec()),
            mint: observed(observation, mint, token_program, false, vec![]),
            buyer_source: observed(observation, buyer_source, token_program, false, vec![]),
            seller_destination: observed(
                observation,
                seller_destination,
                token_program,
                false,
                vec![],
            ),
            fee_destination: observed(observation, fee_destination, token_program, false, vec![]),
            token_program: observed(observation, token_program, key(99), true, vec![]),
        };
        (
            program,
            state,
            SignedCompactIntentV1 {
                maker: seller,
                signature: [11; 64],
                intent: intent(profile_key, seller_destination, 0),
            },
            SignedCompactIntentV1 {
                maker: buyer,
                signature: [12; 64],
                intent: intent(profile_key, buyer_source, 1),
            },
        )
    }

    #[test]
    fn derives_exact_batch_from_one_finalized_snapshot() {
        let (program, state, seller, buyer) = fixture();
        let report = build_compiled_direct(
            program,
            &state,
            seller,
            buyer,
            MatchCoordinatesV1 {
                fill: 2_000,
                execution_price: 500_000,
            },
        )
        .expect("canonical batch");
        assert_eq!(report.instructions[0].program_id, ed25519_program::ID);
        assert_eq!(report.instructions[1].program_id, program);
        assert_eq!(report.instructions[1].accounts.len(), 15);
        let decoded =
            ControllerInstructionV1::decode(&report.instructions[1].data).expect("controller data");
        assert_eq!(decoded.seller, seller.intent);
        assert_eq!(decoded.buyer, buyer.intent);
        assert_eq!(decoded.fill, 2_000);
        assert_eq!(report.instructions[0].data.len(), 222);
    }

    #[test]
    fn refuses_stale_authority_and_structural_signature_material() {
        let (program, mut state, seller, buyer) = fixture();
        state.buyer_replay.key = Pubkey::new_unique();
        assert_eq!(
            build_compiled_direct(
                program,
                &state,
                seller,
                buyer,
                MatchCoordinatesV1 {
                    fill: 2_000,
                    execution_price: 500_000,
                },
            ),
            Err(Error::PdaMismatch)
        );
        let (program, mut state, seller, mut buyer) = fixture();
        state.execution_profile.observation.slot += 1;
        assert_eq!(
            build_compiled_direct(
                program,
                &state,
                seller,
                buyer,
                MatchCoordinatesV1 {
                    fill: 2_000,
                    execution_price: 500_000,
                },
            ),
            Err(Error::ObservationMismatch)
        );
        let (program, state, seller, _) = fixture();
        buyer.signature = [0; 64];
        assert_eq!(
            build_compiled_direct(
                program,
                &state,
                seller,
                buyer,
                MatchCoordinatesV1 {
                    fill: 2_000,
                    execution_price: 500_000,
                },
            ),
            Err(Error::SignatureMaterial)
        );
    }
}
