//! Packet-safe unsigned Rational terminal Bearer redemption construction.

use dclutch_bearer_v2_operator::RationalTerminalHotInstructionV3;
use solana_hash::Hash;
use solana_program::pubkey::Pubkey;

use crate::{
    Finality, Observation, ObservedAccount,
    versioned::{VersionedMessagePlanV0, compile_v0_message},
};

/// Unsigned packet plus the exact wallet identities which must sign it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalTerminalTransactionPlanV3 {
    /// Packet-safe v0 message compiled from a finalized lookup table.
    pub message: VersionedMessagePlanV0,
    /// Exact wallet signer set; terminal redemption requires only the actor.
    pub required_signers: Vec<Pubkey>,
    /// Checked release-manifest identity carried from instruction construction.
    pub checked_manifest_digest: [u8; 32],
    /// Exact family request digest carried from instruction construction.
    pub family_digest: [u8; 32],
}

/// Stable packet-construction refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RationalTerminalTransactionErrorV3 {
    /// The instruction and lookup observations did not share one finalized floor.
    Snapshot,
    /// The instruction did not expose exactly one actor wallet signer.
    Signer,
    /// Lookup-table or packet geometry refused.
    Routing(crate::versioned::Error),
}

/// Compile one checked Hot redemption into an unsigned packet-safe v0 message.
///
/// The actor is also the fee payer, so the message has exactly one wallet
/// signature slot. The caller must supply an already finalized active lookup
/// table; this function never creates, signs, or submits a transaction.
pub fn compile_rational_terminal_hot_v0(
    report: &RationalTerminalHotInstructionV3,
    observation: Observation,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
) -> Result<RationalTerminalTransactionPlanV3, RationalTerminalTransactionErrorV3> {
    if observation.finality != Finality::Finalized
        || observation.slot == 0
        || observation.slot != report.finalized_slot
    {
        return Err(RationalTerminalTransactionErrorV3::Snapshot);
    }
    let actor = match report.required_wallet_signers.as_slice() {
        [actor] => *actor,
        _ => return Err(RationalTerminalTransactionErrorV3::Signer),
    };
    let message = compile_v0_message(
        actor,
        core::slice::from_ref(&report.instruction),
        recent_blockhash,
        observation,
        lookup_tables,
    )
    .map_err(RationalTerminalTransactionErrorV3::Routing)?;
    if message.required_signatures != 1 {
        return Err(RationalTerminalTransactionErrorV3::Signer);
    }
    Ok(RationalTerminalTransactionPlanV3 {
        message,
        required_signers: vec![actor],
        checked_manifest_digest: report.checked_manifest_digest,
        family_digest: report.family_digest,
    })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use solana_address_lookup_table_interface::{
        program,
        state::{AddressLookupTable, LookupTableMeta},
    };
    use solana_program::instruction::{AccountMeta, Instruction};

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn observation() -> Observation {
        Observation {
            slot: 99,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn report() -> RationalTerminalHotInstructionV3 {
        let actor = key(1);
        let mut accounts = vec![AccountMeta::new_readonly(actor, true)];
        accounts.extend((2_u8..90).map(|value| AccountMeta::new_readonly(key(value), false)));
        RationalTerminalHotInstructionV3 {
            instruction: Instruction {
                program_id: key(200),
                accounts,
                data: vec![7; 776],
            },
            required_wallet_signers: vec![actor],
            family_digest: [8; 32],
            checked_manifest_digest: [9; 32],
            finalized_slot: 99,
        }
    }

    fn table(report: &RationalTerminalHotInstructionV3) -> ObservedAccount {
        let addresses = report
            .instruction
            .accounts
            .iter()
            .filter(|account| !account.is_signer)
            .map(|account| account.pubkey)
            .collect::<Vec<_>>();
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(key(201)),
                last_extended_slot: 98,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: observation(),
            key: key(202),
            owner: program::id(),
            lamports: 1_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("lookup table bytes"),
        }
    }

    #[test]
    fn compiles_one_actor_unsigned_v0_through_finalized_table() {
        let report = report();
        let table = table(&report);
        let plan = compile_rational_terminal_hot_v0(
            &report,
            observation(),
            Hash::new_from_array([10; 32]),
            core::slice::from_ref(&table),
        )
        .expect("packet-safe redemption");
        assert_eq!(plan.required_signers, vec![key(1)]);
        assert_eq!(plan.message.required_signatures, 1);
        assert!(plan.message.loaded_addresses > 80);
        assert!(plan.message.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
    }

    #[test]
    fn stale_floor_extra_signer_and_missing_table_refuse() {
        let canonical = report();
        let mut stale = observation();
        stale.slot += 1;
        assert_eq!(
            compile_rational_terminal_hot_v0(
                &canonical,
                stale,
                Hash::new_from_array([10; 32]),
                &[]
            ),
            Err(RationalTerminalTransactionErrorV3::Snapshot)
        );

        let mut extra = canonical.clone();
        extra.required_wallet_signers.push(key(2));
        assert_eq!(
            compile_rational_terminal_hot_v0(
                &extra,
                observation(),
                Hash::new_from_array([10; 32]),
                &[]
            ),
            Err(RationalTerminalTransactionErrorV3::Signer)
        );
        assert_eq!(
            compile_rational_terminal_hot_v0(
                &canonical,
                observation(),
                Hash::new_from_array([10; 32]),
                &[]
            ),
            Err(RationalTerminalTransactionErrorV3::Routing(
                crate::versioned::Error::NoLookupUsed
            ))
        );
    }
}
