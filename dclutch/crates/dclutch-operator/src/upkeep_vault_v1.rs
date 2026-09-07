//! Host-only construction for the upkeep vault's two permissionless routes.
//!
//! Decision 0024 item 4. `dclutch_custody::upkeep_vault_v1` owns the record and
//! the wire; `programs/dclutch-custody-sbf/src/upkeep_vault_v1.rs` owns the
//! accounts; this owns neither — it is the only thing off chain that can put
//! either of them in a transaction, and it exists because the runbook's
//! `upkeep-found` row had nothing to call.
//!
//! # Why there are exactly two builders here and not three
//!
//! The vault has two operations and three account frames: found, a signer's
//! deposit, and a protocol credit. The third is deliberately absent. A protocol
//! credit is signed by a release-set role's caller-authority PDA, which only
//! that program can sign for, so its frame can only ever be assembled INSIDE a
//! program — the Direct close-maker assembles it at
//! `programs/dclutch-trading-sbf/src/direct_close_maker_v1.rs`. A host builder
//! for it would be a frame nobody can submit, which is worse than no builder.

use dclutch_custody::upkeep_vault_v1::{
    Error as UpkeepError, UPKEEP_DEPOSIT_CREDIT_ACCOUNT_COUNT_V1, UPKEEP_FOUND_ACCOUNT_COUNT_V1,
    UpkeepCreditV1, UpkeepOperationV1, UpkeepRequestV1, UpkeepSourceClassV1, UpkeepVaultSeedsV1,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

/// Stable refusal from host-side upkeep-vault construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpkeepVaultBuildErrorV1 {
    /// The Custody program or the payer was the zero key.
    ///
    /// Never a claim about which key is right — a host cannot know that — only
    /// that a builder asked to address the zero key is a builder given nothing.
    InvalidIdentity,
    /// The request the caller asked for is not one this wire admits; the cause
    /// is the vault contract's own.
    Upkeep(UpkeepError),
}

/// The one vault address under one Custody deployment.
///
/// Ruling R1: the seeds are the domain alone, so there is exactly one vault per
/// Custody program and no market appears in it. Derived here rather than
/// carried, because a caller that could pass an address could pass the wrong
/// one and the chain would refuse it after the fee.
#[must_use]
pub fn upkeep_vault_address_v1(custody_program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&UpkeepVaultSeedsV1.as_slices(), &custody_program).0
}

/// `[vault (w), payer (signer, w), system_program, rent_sysvar]`.
///
/// Ruling R2: anybody founds it, once, and gains no authority by doing so —
/// the record has none to gain. The payer funds it to exactly its own rent
/// minimum, which is not an inflow: the vault's legibility subtracts it.
pub fn found_upkeep_vault_instruction_v1(
    custody_program: Pubkey,
    payer: Pubkey,
) -> Result<Instruction, UpkeepVaultBuildErrorV1> {
    if custody_program == Pubkey::default() || payer == Pubkey::default() {
        return Err(UpkeepVaultBuildErrorV1::InvalidIdentity);
    }
    let vault = upkeep_vault_address_v1(custody_program);
    let data = UpkeepRequestV1::FOUND
        .to_bytes()
        .map_err(UpkeepVaultBuildErrorV1::Upkeep)?;
    let accounts = vec![
        AccountMeta::new(vault, false),
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];
    debug_assert_eq!(accounts.len(), UPKEEP_FOUND_ACCOUNT_COUNT_V1);
    Ok(Instruction {
        program_id: custody_program,
        accounts,
        data: data.to_vec(),
    })
}

/// `[vault (w), depositor (signer, w), system_program, rent_sysvar]`.
///
/// The only voluntary inflow. The depositor's own lamports move inside the
/// route, by a System transfer the depositor signs, and are receipted under
/// [`UpkeepSourceClassV1::Deposit`] — the one class a wallet may name, because
/// it is the only one whose provenance a signature can vouch for.
pub fn deposit_upkeep_instruction_v1(
    custody_program: Pubkey,
    depositor: Pubkey,
    amount: u64,
) -> Result<Instruction, UpkeepVaultBuildErrorV1> {
    if custody_program == Pubkey::default() || depositor == Pubkey::default() {
        return Err(UpkeepVaultBuildErrorV1::InvalidIdentity);
    }
    let vault = upkeep_vault_address_v1(custody_program);
    // A zero deposit refuses HERE, in the contract's own words, rather than
    // after a fee: `UpkeepRequestV1::validate` is the one author of what the
    // wire admits and this builder does not restate it.
    let data = UpkeepRequestV1 {
        operation: UpkeepOperationV1::Credit,
        credit: Some(UpkeepCreditV1 {
            source_class: UpkeepSourceClassV1::Deposit,
            caller: None,
            receipt_digest: [0; 32],
            amount,
        }),
    }
    .to_bytes()
    .map_err(UpkeepVaultBuildErrorV1::Upkeep)?;
    let accounts = vec![
        AccountMeta::new(vault, false),
        AccountMeta::new(depositor, true),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];
    debug_assert_eq!(accounts.len(), UPKEEP_DEPOSIT_CREDIT_ACCOUNT_COUNT_V1);
    Ok(Instruction {
        program_id: custody_program,
        accounts,
        data: data.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_custody::upkeep_vault_v1::{
        UPKEEP_VAULT_REQUEST_BYTES_V1, UPKEEP_VAULT_REQUEST_MAGIC_V1,
    };

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    /// The founding frame is the adapter's, coordinate by coordinate: the vault
    /// is the derived PDA and writable and not a signer, the payer signs and is
    /// writable, and the last two are the canonical System program and Rent.
    #[test]
    fn the_founding_frame_is_the_adapters_frame() {
        let custody = key(0xc0);
        let payer = key(0x91);
        let instruction =
            found_upkeep_vault_instruction_v1(custody, payer).expect("founding builds");
        assert_eq!(instruction.program_id, custody);
        assert_eq!(
            instruction.accounts.len(),
            UPKEEP_FOUND_ACCOUNT_COUNT_V1,
            "four accounts, and the count is the contract's"
        );
        let vault = upkeep_vault_address_v1(custody);
        assert_eq!(instruction.accounts[0].pubkey, vault);
        assert!(instruction.accounts[0].is_writable && !instruction.accounts[0].is_signer);
        assert_eq!(instruction.accounts[1].pubkey, payer);
        assert!(instruction.accounts[1].is_signer && instruction.accounts[1].is_writable);
        assert_eq!(instruction.accounts[2].pubkey, system_program::ID);
        assert_eq!(instruction.accounts[3].pubkey, sysvar::rent::ID);
        assert!(!instruction.accounts[2].is_writable && !instruction.accounts[3].is_writable);
        assert_eq!(instruction.data.len(), UPKEEP_VAULT_REQUEST_BYTES_V1);
        assert_eq!(
            instruction.data.get(..8),
            Some(UPKEEP_VAULT_REQUEST_MAGIC_V1.as_slice())
        );
        // And the bytes are the ones the adapter's selector admits, decoded
        // back through the contract rather than compared to a literal.
        assert_eq!(
            UpkeepRequestV1::decode(&instruction.data),
            Ok(UpkeepRequestV1::FOUND)
        );
    }

    /// The one class a wallet may name, and the exact amount, round-tripped
    /// through the contract's own decoder.
    #[test]
    fn a_deposit_carries_its_class_and_its_amount_and_no_caller() {
        let custody = key(0xc0);
        let depositor = key(0x77);
        let instruction =
            deposit_upkeep_instruction_v1(custody, depositor, 2_786_520).expect("deposit builds");
        assert_eq!(
            instruction.accounts.len(),
            UPKEEP_DEPOSIT_CREDIT_ACCOUNT_COUNT_V1
        );
        let decoded = UpkeepRequestV1::decode(&instruction.data).expect("canonical deposit");
        let credit = decoded.credit.expect("a credit");
        assert_eq!(decoded.operation, UpkeepOperationV1::Credit);
        assert_eq!(credit.source_class, UpkeepSourceClassV1::Deposit);
        assert_eq!(credit.amount, 2_786_520);
        assert!(
            credit.caller.is_none(),
            "a deposit's source is its signer, never a release-set role"
        );
        assert_eq!(credit.receipt_digest, [0; 32]);
    }

    /// The builder does not restate what the wire admits: a zero deposit
    /// refuses with the CONTRACT's name for it, before a fee is spent.
    #[test]
    fn a_zero_deposit_refuses_with_the_contracts_own_name() {
        assert_eq!(
            deposit_upkeep_instruction_v1(key(0xc0), key(0x77), 0),
            Err(UpkeepVaultBuildErrorV1::Upkeep(UpkeepError::ZeroAmount))
        );
    }

    /// A builder addressed to nothing is refused as an identity, never as a
    /// wire defect: the bytes would have been perfectly canonical.
    #[test]
    fn the_zero_key_refuses_as_an_identity() {
        assert_eq!(
            found_upkeep_vault_instruction_v1(Pubkey::default(), key(0x91)),
            Err(UpkeepVaultBuildErrorV1::InvalidIdentity)
        );
        assert_eq!(
            deposit_upkeep_instruction_v1(key(0xc0), Pubkey::default(), 5),
            Err(UpkeepVaultBuildErrorV1::InvalidIdentity)
        );
    }
}
