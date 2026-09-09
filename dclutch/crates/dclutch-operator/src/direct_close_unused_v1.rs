//! Canonical instruction projection for a never-activated Direct funding close.

use dclutch_trading::retirement_v1::DirectCloseUnusedRequestV1;
use solana_sdk::{instruction::AccountMeta, instruction::Instruction, pubkey::Pubkey};
use solana_system_interface::program as system_program;

/// Exact ordered coordinates authenticated by the Trading entrypoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCloseUnusedAccountsV1 {
    /// Retiring Core Market.
    pub market: Pubkey,
    /// Trading-owned singleton Pending ledger.
    pub ledger: Pubkey,
    /// Canonical absent Direct root coordinate.
    pub vacant_root: Pubkey,
    /// Market-selected lifecycle RentCredit.
    pub rent_credit: Pubkey,
    /// Finalized manifest raw record.
    pub manifest_raw: Pubkey,
    /// Vacant manifest staging coordinate.
    pub manifest_staging: Pubkey,
    /// Registry activation cache.
    pub activation_cache: Pubkey,
    /// Current Core program.
    pub core_program: Pubkey,
    /// Current Core ProgramData.
    pub core_programdata: Pubkey,
    /// Current Trading program.
    pub trading_program: Pubkey,
    /// Current Trading ProgramData.
    pub trading_programdata: Pubkey,
    /// Registry program.
    pub registry: Pubkey,
    /// Lifecycle Rent program.
    pub rent_program: Pubkey,
}

/// Operator-side structural refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCloseUnusedOperatorErrorV1 {
    /// Two semantic coordinates named one account.
    DuplicateAccount,
    /// A required identity was the all-zero pubkey.
    ZeroIdentity,
}

/// Build the sole canonical top-level Trading instruction.
pub fn plan_direct_close_unused_v1(
    accounts: DirectCloseUnusedAccountsV1,
    entry_index: u16,
) -> Result<Instruction, DirectCloseUnusedOperatorErrorV1> {
    let keys = [
        accounts.market,
        accounts.ledger,
        accounts.vacant_root,
        accounts.rent_credit,
        accounts.manifest_raw,
        accounts.manifest_staging,
        accounts.activation_cache,
        accounts.core_program,
        accounts.core_programdata,
        accounts.trading_program,
        accounts.trading_programdata,
        accounts.registry,
        accounts.rent_program,
        system_program::ID,
    ];
    if keys[..13].iter().any(|key| *key == Pubkey::default()) {
        return Err(DirectCloseUnusedOperatorErrorV1::ZeroIdentity);
    }
    for (index, key) in keys.iter().enumerate() {
        if keys.iter().skip(index + 1).any(|other| other == key) {
            return Err(DirectCloseUnusedOperatorErrorV1::DuplicateAccount);
        }
    }
    Ok(Instruction {
        program_id: accounts.trading_program,
        accounts: vec![
            AccountMeta::new_readonly(accounts.market, false),
            AccountMeta::new(accounts.ledger, false),
            AccountMeta::new_readonly(accounts.vacant_root, false),
            AccountMeta::new(accounts.rent_credit, false),
            AccountMeta::new_readonly(accounts.manifest_raw, false),
            AccountMeta::new_readonly(accounts.manifest_staging, false),
            AccountMeta::new_readonly(accounts.activation_cache, false),
            AccountMeta::new_readonly(accounts.core_program, false),
            AccountMeta::new_readonly(accounts.core_programdata, false),
            AccountMeta::new_readonly(accounts.trading_program, false),
            AccountMeta::new_readonly(accounts.trading_programdata, false),
            AccountMeta::new_readonly(accounts.registry, false),
            AccountMeta::new_readonly(accounts.rent_program, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: DirectCloseUnusedRequestV1 { entry_index }
            .to_bytes()
            .to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_trading::retirement_v1::DirectCloseUnusedRequestV1;

    fn accounts() -> DirectCloseUnusedAccountsV1 {
        let mut keys = (1_u8..=13).map(|byte| Pubkey::new_from_array([byte; 32]));
        DirectCloseUnusedAccountsV1 {
            market: keys.next().unwrap(),
            ledger: keys.next().unwrap(),
            vacant_root: keys.next().unwrap(),
            rent_credit: keys.next().unwrap(),
            manifest_raw: keys.next().unwrap(),
            manifest_staging: keys.next().unwrap(),
            activation_cache: keys.next().unwrap(),
            core_program: keys.next().unwrap(),
            core_programdata: keys.next().unwrap(),
            trading_program: keys.next().unwrap(),
            trading_programdata: keys.next().unwrap(),
            registry: keys.next().unwrap(),
            rent_program: keys.next().unwrap(),
        }
    }

    #[test]
    fn projection_has_exact_wire_order_and_privileges() {
        let input = accounts();
        let instruction = plan_direct_close_unused_v1(input, 7).expect("plan");
        assert_eq!(instruction.program_id, input.trading_program);
        assert_eq!(instruction.accounts.len(), 14);
        assert_eq!(
            instruction
                .accounts
                .iter()
                .filter(|meta| meta.is_writable)
                .count(),
            2
        );
        assert!(instruction.accounts.iter().all(|meta| !meta.is_signer));
        assert_eq!(
            DirectCloseUnusedRequestV1::decode(&instruction.data),
            Ok(DirectCloseUnusedRequestV1 { entry_index: 7 })
        );
    }

    #[test]
    fn substituted_coordinate_refuses_before_construction() {
        let mut input = accounts();
        input.ledger = input.rent_credit;
        assert_eq!(
            plan_direct_close_unused_v1(input, 7),
            Err(DirectCloseUnusedOperatorErrorV1::DuplicateAccount)
        );
    }
}
