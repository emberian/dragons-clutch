//! Transaction construction for the two signature-carrying relay routes.
//!
//! **Code complete, execution gated.** Nothing in this module reaches a
//! cluster; [`crate::submit`] decides whether the result may be sent, and in
//! this lane the only admitted destination is a local endpoint the operator
//! configured.
//!
//! The shape is fixed by §4.4 and is not a choice:
//!
//! - The native Ed25519 precompile is verified against the transaction's
//!   **top-level** instruction list during transaction verification and is not
//!   reachable by CPI, so a post-then-consume transport is unavailable and
//!   **adjacency is the only carriage**.  The precompile instruction therefore
//!   sits *immediately* before the relay instruction.
//! - Adjacency is not the authority (O-018).  The authority is the pair
//!   (release-pinned public key, byte-exact message equality against the
//!   current instruction's own data).  The signed message lives at a **fixed**
//!   offset in the relay instruction's data, and the descriptor's
//!   `message_data_offset` is that constant, which is what lets the on-chain
//!   adapter compare it rather than trust it.
//! - One signature per precompile instruction.  An m-of-n key set signing the
//!   same message would produce m identical message slices, which the adapter's
//!   parser refuses by construction; the family uses one short seal message per
//!   signer instead.

use dclutch_relay_contract::frame::{RelayAccountNameV1, RelayFrameKindV1, relay_frame_roles_v1};
use dclutch_relay_contract::instruction::{
    APPEND_OBSERVATION_PREFIX_BYTES, AppendObservationInstructionV1, SEAL_RECORD_PREFIX_BYTES,
    SealRecordInstructionV1,
};
use dclutch_relay_contract::signature::{
    ED25519_CURRENT_INSTRUCTION_INDEX, ED25519_ONE_SIGNATURE_INSTRUCTION_BYTES,
    ED25519_PROGRAM_ID_3_0,
};
use dclutch_relay_contract::{
    ED25519_DESCRIPTOR_BYTES_V1, ED25519_DESCRIPTOR_START_V1, ED25519_PUBLIC_KEY_BYTES_V1,
    ED25519_SIGNATURE_BYTES_V1, RELAYED_SEAL_BYTES,
};
use solana_address::Address;
use solana_hash::Hash;
use solana_instruction::{AccountMeta, Instruction};
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;

use crate::chain::{
    CLOCK_SYSVAR_ID, COMPUTE_BUDGET_PROGRAM_ID, COMPUTE_BUDGET_SET_UNIT_LIMIT_TAG,
    COMPUTE_BUDGET_SET_UNIT_PRICE_TAG, INSTRUCTIONS_SYSVAR_ID, RENT_SYSVAR_ID,
};
use crate::error::{RelayerError, Result};
use crate::id32::ID_BYTES;

/// Offset of the public key inside a one-signature precompile instruction.
pub const ED25519_PUBLIC_KEY_OFFSET: usize =
    ED25519_DESCRIPTOR_START_V1 + ED25519_DESCRIPTOR_BYTES_V1;
/// Offset of the signature inside a one-signature precompile instruction.
pub const ED25519_SIGNATURE_OFFSET: usize = ED25519_PUBLIC_KEY_OFFSET + ED25519_PUBLIC_KEY_BYTES_V1;

/// The seven `u16` descriptor fields, in wire order at offset 2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ed25519Descriptor {
    /// Offset of the 64 signature bytes.
    pub signature_offset: u16,
    /// Instruction holding the signature; `u16::MAX` means "this one".
    pub signature_instruction_index: u16,
    /// Offset of the 32 public-key bytes.
    pub public_key_offset: u16,
    /// Instruction holding the public key; `u16::MAX` means "this one".
    pub public_key_instruction_index: u16,
    /// Offset of the signed message inside the relay instruction's data.
    pub message_data_offset: u16,
    /// Exact length of the signed message.
    pub message_data_size: u16,
    /// Index of the relay instruction that carries the message.
    pub message_instruction_index: u16,
}

impl Ed25519Descriptor {
    /// The descriptor for one self-contained signature over a message that
    /// lives in the relay instruction at `message_instruction_index`.
    pub fn new(
        message_data_offset: u16,
        message_data_size: u16,
        message_instruction_index: u16,
    ) -> Result<Self> {
        Ok(Self {
            signature_offset: u16_from(ED25519_SIGNATURE_OFFSET)?,
            signature_instruction_index: ED25519_CURRENT_INSTRUCTION_INDEX,
            public_key_offset: u16_from(ED25519_PUBLIC_KEY_OFFSET)?,
            public_key_instruction_index: ED25519_CURRENT_INSTRUCTION_INDEX,
            message_data_offset,
            message_data_size,
            message_instruction_index,
        })
    }

    fn fields(self) -> [u16; 7] {
        [
            self.signature_offset,
            self.signature_instruction_index,
            self.public_key_offset,
            self.public_key_instruction_index,
            self.message_data_offset,
            self.message_data_size,
            self.message_instruction_index,
        ]
    }
}

fn u16_from(value: usize) -> Result<u16> {
    u16::try_from(value)
        .map_err(|_| RelayerError::config("a wire offset did not fit in a u16".to_owned()))
}

/// Build the exact data of a one-signature Ed25519 precompile instruction.
///
/// `2 + 110` bytes of layout, `112` in total: a `u16` count of one, the 14-byte
/// descriptor at offset 2, the 32-byte public key at 16, the 64-byte signature
/// at 48.  Every offset here comes from the wire crate's pinned constants.
pub fn build_ed25519_precompile_data(
    signer: &[u8; ID_BYTES],
    signature: &[u8; ED25519_SIGNATURE_BYTES_V1],
    descriptor: Ed25519Descriptor,
) -> Result<Vec<u8>> {
    let mut data = vec![0u8; ED25519_ONE_SIGNATURE_INSTRUCTION_BYTES];
    write_at(&mut data, 0, &1u16.to_le_bytes())?;
    let mut cursor = ED25519_DESCRIPTOR_START_V1;
    for field in descriptor.fields() {
        write_at(&mut data, cursor, &field.to_le_bytes())?;
        cursor = cursor
            .checked_add(2)
            .ok_or_else(|| RelayerError::config("descriptor cursor overflowed"))?;
    }
    write_at(&mut data, ED25519_PUBLIC_KEY_OFFSET, signer)?;
    write_at(&mut data, ED25519_SIGNATURE_OFFSET, signature)?;
    Ok(data)
}

fn write_at(output: &mut [u8], offset: usize, input: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(input.len())
        .ok_or_else(|| RelayerError::config("instruction write overflowed"))?;
    let destination = output
        .get_mut(offset..end)
        .ok_or_else(|| RelayerError::config("instruction write ran past the buffer"))?;
    destination.copy_from_slice(input);
    Ok(())
}

/// The addresses that fill the append and seal frames.
///
/// `Rent`, `Instructions` and `Clock` are pinned sysvars rather than config, so
/// a misconfigured sysvar is not a failure mode this daemon has.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayFrameAddresses {
    /// The permissionless worker paying for and signing the transaction.
    pub worker: [u8; ID_BYTES],
    /// The owning Market root.
    pub market: [u8; ID_BYTES],
    /// The observation record for this `(market, generation, set, slot)`.
    pub record: [u8; ID_BYTES],
    /// The raw immutable `RelayerKeySetV1` record.
    pub relayer_key_set: [u8; ID_BYTES],
    /// The finalized staging vacancy proving that record is immutable.
    pub relayer_key_set_staging_vacancy: [u8; ID_BYTES],
}

/// Build the exact ordered account metas for one relay route.
///
/// The order, count and privileges come from
/// `dclutch_relay_contract::frame`, not from a table copied into this file, so
/// this daemon cannot drift from the frame the program validates.
pub fn frame_metas(
    kind: RelayFrameKindV1,
    addresses: &RelayFrameAddresses,
) -> Result<Vec<AccountMeta>> {
    let roles = relay_frame_roles_v1(kind);
    let mut metas = Vec::with_capacity(roles.len());
    for role in roles {
        let key = match role.name() {
            RelayAccountNameV1::Worker => addresses.worker,
            RelayAccountNameV1::Market => addresses.market,
            RelayAccountNameV1::Record => addresses.record,
            RelayAccountNameV1::RelayerKeySet => addresses.relayer_key_set,
            RelayAccountNameV1::RelayerKeySetStagingVacancy => {
                addresses.relayer_key_set_staging_vacancy
            }
            RelayAccountNameV1::RentSysvar => RENT_SYSVAR_ID,
            RelayAccountNameV1::ClockSysvar => CLOCK_SYSVAR_ID,
            RelayAccountNameV1::InstructionsSysvar => INSTRUCTIONS_SYSVAR_ID,
            other => {
                return Err(RelayerError::MissingCapability(format!(
                    "the relayer builds only the append and seal routes; the {other:?} position \
                     belongs to record creation or retirement, which this daemon does not \
                     construct"
                )));
            }
        };
        metas.push(AccountMeta {
            pubkey: Address::from(key),
            is_signer: role.is_signer(),
            is_writable: role.is_writable(),
        });
    }
    Ok(metas)
}

/// One relay instruction and the exact span of it the precompile must name.
#[derive(Clone, Debug)]
pub struct RelayInstructionPlan {
    /// The relay instruction itself.
    pub instruction: Instruction,
    /// Fixed offset of the signed message in the instruction data.
    pub message_data_offset: u16,
    /// Exact length of the signed message.
    pub message_data_size: u16,
}

/// Build an `AppendObservation` instruction carrying one signed attestation.
pub fn append_observation_instruction(
    relay_program_id: [u8; ID_BYTES],
    addresses: &RelayFrameAddresses,
    generation: u64,
    observed_slot: u64,
    attestation_message: &[u8],
) -> Result<RelayInstructionPlan> {
    let prefix = AppendObservationInstructionV1::new(generation, observed_slot)
        .to_prefix_bytes()
        .map_err(|error| RelayerError::wire("append instruction prefix", error))?;
    let mut data = Vec::with_capacity(prefix.len().saturating_add(attestation_message.len()));
    data.extend_from_slice(&prefix);
    data.extend_from_slice(attestation_message);
    Ok(RelayInstructionPlan {
        instruction: Instruction {
            program_id: Address::from(relay_program_id),
            accounts: frame_metas(RelayFrameKindV1::AppendObservation, addresses)?,
            data,
        },
        message_data_offset: u16_from(APPEND_OBSERVATION_PREFIX_BYTES)?,
        message_data_size: u16_from(attestation_message.len())?,
    })
}

/// Build a `SealRecord` instruction carrying one signed seal.
pub fn seal_record_instruction(
    relay_program_id: [u8; ID_BYTES],
    addresses: &RelayFrameAddresses,
    generation: u64,
    observed_slot: u64,
    seal_message: &[u8; RELAYED_SEAL_BYTES],
) -> Result<RelayInstructionPlan> {
    let prefix = SealRecordInstructionV1::new(generation, observed_slot)
        .to_prefix_bytes()
        .map_err(|error| RelayerError::wire("seal instruction prefix", error))?;
    let mut data = Vec::with_capacity(prefix.len().saturating_add(seal_message.len()));
    data.extend_from_slice(&prefix);
    data.extend_from_slice(seal_message);
    Ok(RelayInstructionPlan {
        instruction: Instruction {
            program_id: Address::from(relay_program_id),
            accounts: frame_metas(RelayFrameKindV1::SealRecord, addresses)?,
            data,
        },
        message_data_offset: u16_from(SEAL_RECORD_PREFIX_BYTES)?,
        message_data_size: u16_from(seal_message.len())?,
    })
}

/// Optional ComputeBudget preamble.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ComputeBudget {
    /// `SetComputeUnitLimit`.
    pub unit_limit: Option<u32>,
    /// `SetComputeUnitPrice`, in micro-lamports per compute unit.
    pub unit_price_micro_lamports: Option<u64>,
}

impl ComputeBudget {
    fn instructions(self) -> Vec<Instruction> {
        let program = Address::from(COMPUTE_BUDGET_PROGRAM_ID);
        let mut out = Vec::new();
        if let Some(limit) = self.unit_limit {
            let mut data = vec![COMPUTE_BUDGET_SET_UNIT_LIMIT_TAG];
            data.extend_from_slice(&limit.to_le_bytes());
            out.push(Instruction {
                program_id: program,
                accounts: Vec::new(),
                data,
            });
        }
        if let Some(price) = self.unit_price_micro_lamports {
            let mut data = vec![COMPUTE_BUDGET_SET_UNIT_PRICE_TAG];
            data.extend_from_slice(&price.to_le_bytes());
            out.push(Instruction {
                program_id: program,
                accounts: Vec::new(),
                data,
            });
        }
        out
    }
}

/// A built, unsigned v0 message plus the indices the descriptor committed to.
#[derive(Clone, Debug)]
pub struct RelayTransactionPlan {
    /// The compiled v0 message.
    pub message: VersionedMessage,
    /// Index of the Ed25519 precompile instruction.
    pub ed25519_instruction_index: u16,
    /// Index of the relay instruction, which the descriptor names.
    pub relay_instruction_index: u16,
}

/// Compile the v0 message for one relay route.
///
/// Instruction order is `[compute budget…] · Ed25519 precompile · relay`, and
/// the precompile is immediately before the relay instruction by construction
/// rather than by convention.
#[allow(clippy::too_many_arguments)]
pub fn build_relay_transaction_plan(
    fee_payer: [u8; ID_BYTES],
    plan: &RelayInstructionPlan,
    attestation_signer: &[u8; ID_BYTES],
    attestation_signature: &[u8; ED25519_SIGNATURE_BYTES_V1],
    compute_budget: ComputeBudget,
    lookup_tables: &[AddressLookupTableAccount],
    recent_blockhash: [u8; ID_BYTES],
) -> Result<RelayTransactionPlan> {
    let mut instructions = compute_budget.instructions();
    let ed25519_index = u16_from(instructions.len())?;
    let relay_index = ed25519_index
        .checked_add(1)
        .ok_or_else(|| RelayerError::config("instruction index overflowed"))?;

    let descriptor = Ed25519Descriptor::new(
        plan.message_data_offset,
        plan.message_data_size,
        relay_index,
    )?;
    instructions.push(Instruction {
        program_id: Address::from(ED25519_PROGRAM_ID_3_0),
        accounts: Vec::new(),
        data: build_ed25519_precompile_data(attestation_signer, attestation_signature, descriptor)?,
    });
    instructions.push(plan.instruction.clone());

    let message = v0::Message::try_compile(
        &Address::from(fee_payer),
        &instructions,
        lookup_tables,
        Hash::new_from_array(recent_blockhash),
    )
    .map_err(|error| RelayerError::config(format!("could not compile the v0 message: {error}")))?;

    Ok(RelayTransactionPlan {
        message: VersionedMessage::V0(message),
        ed25519_instruction_index: ed25519_index,
        relay_instruction_index: relay_index,
    })
}

/// The exact bytes the fee payer signs.
pub fn message_bytes(message: &VersionedMessage) -> Vec<u8> {
    message.serialize()
}

/// Replace the blockhash of an already built message.
///
/// **On blockhash expiry, re-sign the transaction; never re-observe.** The
/// attestation is bound to `observed_slot`, and taking a fresh read because a
/// blockhash aged out would silently change the fact being attested — the
/// message would name a different slot and different bytes while the operator
/// believes they retried the same submission.  Re-signing costs one fee-payer
/// signature and changes nothing that was observed.
pub fn set_recent_blockhash(message: &mut VersionedMessage, recent_blockhash: [u8; ID_BYTES]) {
    match message {
        VersionedMessage::Legacy(legacy) => {
            legacy.recent_blockhash = Hash::new_from_array(recent_blockhash);
        }
        VersionedMessage::V0(v0_message) => {
            v0_message.recent_blockhash = Hash::new_from_array(recent_blockhash);
        }
        other => {
            other.set_recent_blockhash(Hash::new_from_array(recent_blockhash));
        }
    }
}

/// Attach the fee payer's signature and produce the wire transaction.
pub fn sign_transaction(
    message: VersionedMessage,
    fee_payer_signature: [u8; ED25519_SIGNATURE_BYTES_V1],
) -> VersionedTransaction {
    VersionedTransaction {
        signatures: vec![Signature::from(fee_payer_signature)],
        message,
    }
}

/// Serialize a transaction to its wire bytes.
pub fn serialize_transaction(transaction: &VersionedTransaction) -> Result<Vec<u8>> {
    bincode::serialize(transaction)
        .map_err(|error| RelayerError::Serialization(format!("transaction wire: {error}")))
}

/// Derive the observation record PDA for one `(market, generation, set, slot)`.
///
/// Seeding by `observed_slot` is the equivocation bound: at most one record
/// exists per set per slot, so a second contradictory observation of the same
/// set at the same slot has nowhere to live.
pub fn derive_record_address(
    relay_program_id: [u8; ID_BYTES],
    market: [u8; ID_BYTES],
    generation: u64,
    account_set_id: [u8; ID_BYTES],
    observed_slot: u64,
) -> ([u8; ID_BYTES], u8) {
    let generation_le = generation.to_le_bytes();
    let observed_slot_le = observed_slot.to_le_bytes();
    let seeds: [&[u8]; 5] = [
        dclutch_relay_contract::RELAYED_RECORD_PDA_DOMAIN_V1,
        &market,
        &generation_le,
        &account_set_id,
        &observed_slot_le,
    ];
    let (address, bump) = Address::find_program_address(&seeds, &Address::from(relay_program_id));
    (address.to_bytes(), bump)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_relay_contract::instruction::RelayInstructionV1;
    use dclutch_relay_contract::signature::{
        Ed25519InstructionViewV1, inspect_preceding_relay_signature_v1,
    };

    fn addresses() -> RelayFrameAddresses {
        RelayFrameAddresses {
            worker: [1; ID_BYTES],
            market: [2; ID_BYTES],
            record: [3; ID_BYTES],
            relayer_key_set: [4; ID_BYTES],
            relayer_key_set_staging_vacancy: [5; ID_BYTES],
        }
    }

    #[test]
    fn the_precompile_instruction_is_exactly_112_bytes_with_the_pinned_descriptor() {
        let descriptor = Ed25519Descriptor::new(40, 268, 3).expect("descriptor");
        assert_eq!(descriptor.signature_offset, 48);
        assert_eq!(descriptor.public_key_offset, 16);
        assert_eq!(descriptor.signature_instruction_index, u16::MAX);
        assert_eq!(descriptor.public_key_instruction_index, u16::MAX);

        let data = build_ed25519_precompile_data(&[0x55; ID_BYTES], &[0x77; 64], descriptor)
            .expect("data");
        assert_eq!(data.len(), 112);
        assert_eq!(&data[0..2], &1u16.to_le_bytes());
        assert_eq!(&data[2..4], &48u16.to_le_bytes());
        assert_eq!(&data[4..6], &u16::MAX.to_le_bytes());
        assert_eq!(&data[6..8], &16u16.to_le_bytes());
        assert_eq!(&data[8..10], &u16::MAX.to_le_bytes());
        assert_eq!(&data[10..12], &40u16.to_le_bytes());
        assert_eq!(&data[12..14], &268u16.to_le_bytes());
        assert_eq!(&data[14..16], &3u16.to_le_bytes());
        assert_eq!(&data[16..48], &[0x55; 32]);
        assert_eq!(&data[48..112], &[0x77; 64]);
    }

    /// The strongest available offline check: the precompile instruction this
    /// daemon builds is accepted by the *on-chain adapter's own parser*, at the
    /// exact message span the relay instruction places the message at.
    #[test]
    fn the_adapters_own_parser_accepts_what_this_daemon_builds() {
        let attestation = vec![0xAB; 268];
        let plan = append_observation_instruction(
            [9; ID_BYTES],
            &addresses(),
            7,
            423_941_138,
            &attestation,
        )
        .expect("plan");
        assert_eq!(plan.message_data_offset, 40);
        assert_eq!(plan.message_data_size, 268);

        let descriptor =
            Ed25519Descriptor::new(plan.message_data_offset, plan.message_data_size, 1)
                .expect("descriptor");
        let precompile = build_ed25519_precompile_data(&[0x55; ID_BYTES], &[0x77; 64], descriptor)
            .expect("data");

        let view = Ed25519InstructionViewV1 {
            program_id: ED25519_PROGRAM_ID_3_0,
            ed25519_data: &precompile,
            preceding_index: 0,
            current_index: 1,
            current_data: &plan.instruction.data,
        };
        let accepted = inspect_preceding_relay_signature_v1(
            view,
            plan.message_data_offset,
            plan.message_data_size,
        )
        .expect("the adapter parser accepts the daemon's precompile");
        assert_eq!(accepted.signer(), [0x55; ID_BYTES]);
        assert_eq!(accepted.message(), attestation.as_slice());
    }

    #[test]
    fn a_seal_instruction_places_its_message_at_the_fixed_seal_offset() {
        let seal = [0xCD; RELAYED_SEAL_BYTES];
        let plan = seal_record_instruction([9; ID_BYTES], &addresses(), 7, 1, &seal).expect("plan");
        assert_eq!(plan.message_data_offset, 32);
        assert_eq!(
            plan.message_data_size,
            u16::try_from(RELAYED_SEAL_BYTES).unwrap()
        );
        match RelayInstructionV1::decode(&plan.instruction.data).expect("decodes") {
            RelayInstructionV1::SealRecord(_, message) => assert_eq!(message, seal.as_slice()),
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn the_frame_comes_from_the_contract_crate_and_keeps_its_privileges() {
        let metas = frame_metas(RelayFrameKindV1::AppendObservation, &addresses()).expect("metas");
        let roles = relay_frame_roles_v1(RelayFrameKindV1::AppendObservation);
        assert_eq!(metas.len(), roles.len());
        for (meta, role) in metas.iter().zip(roles) {
            assert_eq!(meta.is_signer, role.is_signer());
            assert_eq!(meta.is_writable, role.is_writable());
        }
        let worker = metas.first().expect("worker");
        assert!(worker.is_signer && worker.is_writable);
    }

    #[test]
    fn the_create_route_is_refused_rather_than_half_built() {
        let error = frame_metas(RelayFrameKindV1::CreateRecord, &addresses()).unwrap_err();
        assert!(
            matches!(error, RelayerError::MissingCapability(_)),
            "{error:?}"
        );
    }

    #[test]
    fn the_precompile_is_immediately_before_the_relay_instruction() {
        let attestation = vec![0xAB; 268];
        let plan = append_observation_instruction([9; ID_BYTES], &addresses(), 7, 1, &attestation)
            .expect("plan");
        let built = build_relay_transaction_plan(
            [1; ID_BYTES],
            &plan,
            &[0x55; ID_BYTES],
            &[0x77; 64],
            ComputeBudget {
                unit_limit: Some(200_000),
                unit_price_micro_lamports: Some(1),
            },
            &[],
            [0x42; ID_BYTES],
        )
        .expect("plan");
        assert_eq!(built.ed25519_instruction_index, 2);
        assert_eq!(built.relay_instruction_index, 3);
        let instructions = built.message.instructions();
        assert_eq!(instructions.len(), 4);
        let keys = built.message.static_account_keys();
        let ed25519_program = Address::from(ED25519_PROGRAM_ID_3_0);
        let precompile = &instructions[2];
        let relay = &instructions[3];
        assert_eq!(
            keys[usize::from(precompile.program_id_index)],
            ed25519_program
        );
        assert_eq!(
            keys[usize::from(relay.program_id_index)],
            Address::from([9u8; ID_BYTES])
        );
        // The descriptor's message_instruction_index must be the relay
        // instruction's real index in the compiled message.
        let named = u16::from_le_bytes([precompile.data[14], precompile.data[15]]);
        assert_eq!(usize::from(named), 3);
    }

    #[test]
    fn a_new_blockhash_changes_the_message_but_not_the_attestation_signature() {
        let attestation = vec![0xAB; 268];
        let plan = append_observation_instruction([9; ID_BYTES], &addresses(), 7, 1, &attestation)
            .expect("plan");
        let mut built = build_relay_transaction_plan(
            [1; ID_BYTES],
            &plan,
            &[0x55; ID_BYTES],
            &[0x77; 64],
            ComputeBudget::default(),
            &[],
            [0x42; ID_BYTES],
        )
        .expect("plan");
        let before = message_bytes(&built.message);
        let precompile_before = built.message.instructions()[0].data.clone();

        set_recent_blockhash(&mut built.message, [0x43; ID_BYTES]);
        let after = message_bytes(&built.message);
        assert_ne!(before, after, "the blockhash did not change the message");
        assert_eq!(
            built.message.instructions()[0].data,
            precompile_before,
            "re-blockhashing must not disturb the attestation signature"
        );
    }

    #[test]
    fn the_wire_transaction_serializes_with_exactly_one_signature() {
        let attestation = vec![0xAB; 268];
        let plan = append_observation_instruction([9; ID_BYTES], &addresses(), 7, 1, &attestation)
            .expect("plan");
        let built = build_relay_transaction_plan(
            [1; ID_BYTES],
            &plan,
            &[0x55; ID_BYTES],
            &[0x77; 64],
            ComputeBudget::default(),
            &[],
            [0x42; ID_BYTES],
        )
        .expect("plan");
        let message = message_bytes(&built.message);
        let transaction = sign_transaction(built.message, [0x11; 64]);
        assert_eq!(transaction.signatures.len(), 1);
        let wire = serialize_transaction(&transaction).expect("wire");
        // compact-u16 signature count (1 byte) + one 64-byte signature, then
        // the message bytes verbatim.
        assert_eq!(wire.len(), 1 + 64 + message.len());
        assert_eq!(&wire[0..1], &[1u8]);
        assert_eq!(&wire[1..65], &[0x11u8; 64]);
        assert_eq!(&wire[65..], message.as_slice());
    }

    #[test]
    fn the_record_address_is_slot_seeded_so_one_slot_admits_one_record() {
        let (first, _) = derive_record_address([9; ID_BYTES], [2; ID_BYTES], 7, [3; ID_BYTES], 100);
        let (again, _) = derive_record_address([9; ID_BYTES], [2; ID_BYTES], 7, [3; ID_BYTES], 100);
        let (later, _) = derive_record_address([9; ID_BYTES], [2; ID_BYTES], 7, [3; ID_BYTES], 101);
        assert_eq!(first, again);
        assert_ne!(first, later);
    }
}
