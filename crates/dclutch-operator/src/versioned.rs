//! Chain-derived address-table lifecycle and versioned-message construction.
//!
//! Address lookup tables are transaction-routing data, never protocol
//! authority. This module validates exact finalized table bytes before using
//! them and never signs, submits, or mutates an external system.

use crate::{Finality, Observation, ObservedAccount};
use solana_address_lookup_table_interface::{
    instruction::{
        close_lookup_table, create_lookup_table, deactivate_lookup_table, extend_lookup_table,
        freeze_lookup_table,
    },
    program,
    state::{AddressLookupTable, LOOKUP_TABLE_MAX_ADDRESSES, estimate_last_valid_slot},
};
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{instruction::Instruction, pubkey::Pubkey};

/// Solana's current serialized transaction packet limit.
pub const PACKET_DATA_BYTES: usize = 1_232;
/// Measured-profile maximum addresses in one table-extension transaction.
///
/// Twenty 32-byte addresses leave ample room for the instruction, account
/// keys, recent blockhash, and required signatures under `PACKET_DATA_BYTES`.
/// This is a transaction-packing bound, not a protocol or semantic bound.
pub const EXTEND_ADDRESSES_PER_TRANSACTION_V1: usize = 20;

/// Unsigned create-and-extend workflow for one canonically ordered table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LookupTableCreationPlanV1 {
    /// Address derived by the official lookup-table program.
    pub lookup_table: Pubkey,
    /// Sorted, duplicate-free addresses that the extensions will append.
    pub addresses: Vec<Pubkey>,
    /// Official create instruction.
    pub create: Instruction,
    /// Official bounded extension instructions, in execution order.
    pub extensions: Vec<Instruction>,
}

/// Exact versioned message and its packet geometry, without signatures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionedMessagePlanV0 {
    /// Unsigned v0 message selected from finalized table observations.
    pub message: VersionedMessage,
    /// Exact number of signature slots required by the message.
    pub required_signatures: u8,
    /// Serialized transaction bytes after those 64-byte signatures are added.
    pub wire_bytes: usize,
    /// Number of addresses actually loaded through tables.
    pub loaded_addresses: usize,
    /// Exact lookup-table accounts used by the compiled message.
    pub lookup_tables: Vec<Pubkey>,
}

/// Next safe unsigned retirement action for an authority-owned table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LookupTableRetirementV1 {
    /// The active table may now be deactivated.
    Deactivate(Instruction),
    /// The table remains in the chain-derived cooldown window.
    CoolingDown {
        /// Conservative first slot after which this planner will offer close.
        close_after_slot: u64,
    },
    /// The cooldown is conservatively complete and the table may be closed.
    Close(Instruction),
}

/// Refusal from malformed, stale, ambiguous, or non-packet-safe routing data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The requested table had no addresses.
    EmptyAddresses,
    /// The requested table exceeded the chain's 256-address limit.
    TooManyAddresses,
    /// A table address appeared more than once.
    DuplicateAddress,
    /// The same lookup-table account was supplied more than once.
    DuplicateTable,
    /// The observed account was not a valid lookup-table account.
    InvalidTable,
    /// The table observation was not finalized.
    ObservationNotFinalized,
    /// A table observation differed from the instruction observation.
    ObservationMismatch,
    /// Addresses appended in the observed slot are not active yet.
    TableNotActivated,
    /// A deactivating table is not admitted for a new message.
    TableDeactivating,
    /// No supplied lookup table contributed an address.
    NoLookupUsed,
    /// Message compilation failed.
    Compile,
    /// The fully signed packet would exceed Solana's packet limit.
    PacketTooLarge,
    /// The supplied authority cannot mutate or retire this table.
    AuthorityMismatch,
    /// Integer sizing overflowed.
    Arithmetic,
}

/// Build official create and bounded extension instructions.
///
/// The output is deterministic for an address set: addresses are sorted by
/// bytes and duplicates refuse. The table remains authority-owned so it may be
/// rotated and its rent recovered. Call [`build_lookup_table_freeze`] only when
/// permanent immutability and permanent rent are intentional.
pub fn build_lookup_table_creation_v1(
    authority: Pubkey,
    payer: Pubkey,
    recent_slot: u64,
    addresses: &[Pubkey],
) -> Result<LookupTableCreationPlanV1, Error> {
    let addresses = canonical_addresses(addresses)?;
    let (create, lookup_table) = create_lookup_table(authority, payer, recent_slot);
    let extensions = addresses
        .chunks(EXTEND_ADDRESSES_PER_TRANSACTION_V1)
        .map(|chunk| extend_lookup_table(lookup_table, authority, Some(payer), chunk.to_vec()))
        .collect();
    Ok(LookupTableCreationPlanV1 {
        lookup_table,
        addresses,
        create,
        extensions,
    })
}

/// Build the official irreversible freeze instruction.
pub fn build_lookup_table_freeze(lookup_table: Pubkey, authority: Pubkey) -> Instruction {
    freeze_lookup_table(lookup_table, authority)
}

/// Select the next safe retirement action from exact finalized table bytes.
///
/// Cooldown uses the official conservative slot estimate. A close transaction
/// can still refuse if the live SlotHashes state differs; no transaction is
/// signed or submitted here.
pub fn plan_lookup_table_retirement_v1(
    table_account: &ObservedAccount,
    authority: Pubkey,
    recipient: Pubkey,
    current_slot: u64,
) -> Result<LookupTableRetirementV1, Error> {
    if table_account.observation.finality != Finality::Finalized {
        return Err(Error::ObservationNotFinalized);
    }
    if current_slot < table_account.observation.slot {
        return Err(Error::ObservationMismatch);
    }
    let table = decode_table(table_account)?;
    if table.meta.authority != Some(authority) {
        return Err(Error::AuthorityMismatch);
    }
    if table.meta.deactivation_slot == u64::MAX {
        return Ok(LookupTableRetirementV1::Deactivate(
            deactivate_lookup_table(table_account.key, authority),
        ));
    }
    let close_after_slot = estimate_last_valid_slot(table.meta.deactivation_slot);
    if current_slot <= close_after_slot {
        return Ok(LookupTableRetirementV1::CoolingDown { close_after_slot });
    }
    Ok(LookupTableRetirementV1::Close(close_lookup_table(
        table_account.key,
        authority,
        recipient,
    )))
}

/// Compile an unsigned packet-safe v0 message from finalized lookup tables.
///
/// Tables must share the instruction builder's exact finalized observation,
/// must not be deactivating, and must have been extended before the observed
/// slot. Existing lookup-table entries are append-only, but duplicate entries
/// are refused to keep index selection unambiguous and economical.
pub fn compile_v0_message(
    payer: Pubkey,
    instructions: &[Instruction],
    recent_blockhash: Hash,
    observation: Observation,
    table_accounts: &[ObservedAccount],
) -> Result<VersionedMessagePlanV0, Error> {
    let mut tables = Vec::with_capacity(table_accounts.len());
    let mut table_keys = Vec::with_capacity(table_accounts.len());
    let mut seen_addresses = Vec::new();
    for account in table_accounts {
        if account.observation.finality != Finality::Finalized {
            return Err(Error::ObservationNotFinalized);
        }
        if account.observation != observation {
            return Err(Error::ObservationMismatch);
        }
        if table_keys.contains(&account.key) {
            return Err(Error::DuplicateTable);
        }
        table_keys.push(account.key);
        let table = decode_table(account)?;
        if table.meta.deactivation_slot != u64::MAX {
            return Err(Error::TableDeactivating);
        }
        if table.meta.last_extended_slot >= observation.slot {
            return Err(Error::TableNotActivated);
        }
        let addresses = canonical_observed_addresses(&table.addresses)?;
        for address in &addresses {
            if seen_addresses.contains(address) {
                return Err(Error::DuplicateAddress);
            }
            seen_addresses.push(*address);
        }
        tables.push(AddressLookupTableAccount {
            key: account.key,
            addresses,
        });
    }
    let message = v0::Message::try_compile(&payer, instructions, &tables, recent_blockhash)
        .map_err(|_| Error::Compile)?;
    let loaded_addresses = message
        .address_table_lookups
        .iter()
        .try_fold(0_usize, |total, lookup| {
            total
                .checked_add(lookup.writable_indexes.len())
                .and_then(|value| value.checked_add(lookup.readonly_indexes.len()))
        })
        .ok_or(Error::Arithmetic)?;
    if loaded_addresses == 0 {
        return Err(Error::NoLookupUsed);
    }
    let required_signatures = message.header.num_required_signatures;
    let message_bytes = message.serialize().len();
    let message = VersionedMessage::V0(message);
    let signature_count = usize::from(required_signatures);
    let wire_bytes = short_vec_prefix_bytes(signature_count)
        .checked_add(signature_count.checked_mul(64).ok_or(Error::Arithmetic)?)
        .and_then(|value| value.checked_add(message_bytes))
        .ok_or(Error::Arithmetic)?;
    if wire_bytes > PACKET_DATA_BYTES {
        return Err(Error::PacketTooLarge);
    }
    let lookup_tables = message
        .address_table_lookups()
        .ok_or(Error::Compile)?
        .iter()
        .map(|lookup| lookup.account_key)
        .collect();
    Ok(VersionedMessagePlanV0 {
        message,
        required_signatures,
        wire_bytes,
        loaded_addresses,
        lookup_tables,
    })
}

fn decode_table(account: &ObservedAccount) -> Result<AddressLookupTable<'_>, Error> {
    if account.owner != program::id() || account.executable {
        return Err(Error::InvalidTable);
    }
    AddressLookupTable::deserialize(&account.data).map_err(|_| Error::InvalidTable)
}

fn canonical_addresses(addresses: &[Pubkey]) -> Result<Vec<Pubkey>, Error> {
    if addresses.is_empty() {
        return Err(Error::EmptyAddresses);
    }
    if addresses.len() > LOOKUP_TABLE_MAX_ADDRESSES {
        return Err(Error::TooManyAddresses);
    }
    let mut canonical = addresses.to_vec();
    canonical.sort_unstable_by_key(Pubkey::to_bytes);
    if canonical.windows(2).any(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(left, right)| left == right)
    }) {
        return Err(Error::DuplicateAddress);
    }
    Ok(canonical)
}

fn canonical_observed_addresses(addresses: &[Pubkey]) -> Result<Vec<Pubkey>, Error> {
    if addresses.len() > LOOKUP_TABLE_MAX_ADDRESSES {
        return Err(Error::TooManyAddresses);
    }
    if addresses.windows(2).any(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(left, right)| left == right)
    }) {
        return Err(Error::DuplicateAddress);
    }
    let mut seen = Vec::with_capacity(addresses.len());
    for address in addresses {
        if seen.contains(address) {
            return Err(Error::DuplicateAddress);
        }
        seen.push(*address);
    }
    Ok(addresses.to_vec())
}

fn short_vec_prefix_bytes(mut value: usize) -> usize {
    let mut bytes = 1_usize;
    while value >= 0x80 {
        value >>= 7;
        bytes = bytes.saturating_add(1);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_address_lookup_table_interface::state::LookupTableMeta;
    use solana_message::legacy;
    use solana_program::instruction::AccountMeta;
    use std::borrow::Cow;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn observation(slot: u64) -> Observation {
        Observation {
            slot,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn observed_table(
        observation: Observation,
        table_key: Pubkey,
        authority: Pubkey,
        last_extended_slot: u64,
        deactivation_slot: u64,
        addresses: Vec<Pubkey>,
    ) -> ObservedAccount {
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(authority),
                last_extended_slot,
                deactivation_slot,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation,
            key: table_key,
            owner: program::id(),
            lamports: 1_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("table bytes"),
        }
    }

    #[test]
    fn creation_is_canonical_bounded_and_retirable() {
        let authority = key(1);
        let payer = key(2);
        let mut addresses = (10_u8..55).rev().map(key).collect::<Vec<_>>();
        let plan = build_lookup_table_creation_v1(authority, payer, 77, &addresses)
            .expect("creation plan");
        addresses.sort_unstable_by_key(Pubkey::to_bytes);
        assert_eq!(plan.addresses, addresses);
        assert_eq!(plan.extensions.len(), 3);
        assert!(plan.extensions.iter().all(|instruction| {
            instruction.program_id == program::id()
                && instruction
                    .accounts
                    .get(1)
                    .is_some_and(|meta| meta.is_signer)
        }));
        for instruction in std::iter::once(&plan.create).chain(plan.extensions.iter()) {
            let message = legacy::Message::new_with_blockhash(
                std::slice::from_ref(instruction),
                Some(&payer),
                &Hash::new_from_array([3; 32]),
            );
            let signatures = usize::from(message.header.num_required_signatures);
            let wire_bytes =
                short_vec_prefix_bytes(signatures) + signatures * 64 + message.serialize().len();
            assert!(wire_bytes <= PACKET_DATA_BYTES);
        }

        let active = observed_table(
            observation(90),
            plan.lookup_table,
            authority,
            78,
            u64::MAX,
            plan.addresses,
        );
        assert!(matches!(
            plan_lookup_table_retirement_v1(&active, authority, payer, 90),
            Ok(LookupTableRetirementV1::Deactivate(_))
        ));

        let deactivation_slot = 91;
        let cooling = observed_table(
            observation(92),
            active.key,
            authority,
            78,
            deactivation_slot,
            addresses.clone(),
        );
        let close_after_slot = estimate_last_valid_slot(deactivation_slot);
        assert_eq!(
            plan_lookup_table_retirement_v1(&cooling, authority, payer, 92),
            Ok(LookupTableRetirementV1::CoolingDown { close_after_slot })
        );
        assert!(matches!(
            plan_lookup_table_retirement_v1(&cooling, authority, payer, close_after_slot + 1),
            Ok(LookupTableRetirementV1::Close(_))
        ));

        let duplicate = vec![key(10), key(10)];
        assert_eq!(
            build_lookup_table_creation_v1(authority, payer, 77, &duplicate),
            Err(Error::DuplicateAddress)
        );
    }

    #[test]
    fn finalized_active_table_compiles_packet_and_stale_table_refuses() {
        let observed = observation(90);
        let payer = key(1);
        let program_id = key(2);
        let writable = key(3);
        let readonly = key(4);
        let instruction = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(writable, false),
                AccountMeta::new_readonly(readonly, false),
            ],
            data: vec![5; 700],
        };
        let table = observed_table(
            observed,
            key(8),
            key(9),
            89,
            u64::MAX,
            vec![writable, readonly],
        );
        let plan = compile_v0_message(
            payer,
            std::slice::from_ref(&instruction),
            Hash::new_from_array([7; 32]),
            observed,
            std::slice::from_ref(&table),
        )
        .expect("packet-safe v0");
        assert_eq!(plan.required_signatures, 1);
        assert_eq!(plan.loaded_addresses, 2);
        assert!(plan.wire_bytes <= PACKET_DATA_BYTES);
        assert_eq!(plan.lookup_tables, vec![table.key]);

        let mut same_slot = table.clone();
        same_slot.observation.slot = 89;
        assert_eq!(
            compile_v0_message(
                payer,
                std::slice::from_ref(&instruction),
                Hash::new_from_array([7; 32]),
                same_slot.observation,
                std::slice::from_ref(&same_slot),
            ),
            Err(Error::TableNotActivated)
        );

        let deactivating =
            observed_table(observed, key(10), key(9), 89, 88, vec![writable, readonly]);
        assert_eq!(
            compile_v0_message(
                payer,
                &[instruction],
                Hash::new_from_array([7; 32]),
                observed,
                &[deactivating],
            ),
            Err(Error::TableDeactivating)
        );
        assert_eq!(
            compile_v0_message(
                payer,
                &[],
                Hash::new_from_array([7; 32]),
                observed,
                &[table.clone(), table],
            ),
            Err(Error::DuplicateTable)
        );
    }
}
