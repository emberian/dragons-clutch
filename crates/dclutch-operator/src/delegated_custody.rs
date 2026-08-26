//! Chain-derived CPI construction for the delegated-allowance Custody successor.

use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CustodyReplaySeedsV1, DelegatedCustodyRequestV2,
};
use dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_release_set_contract::CallerAuthoritySeedsV1;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Host-side refusal while deriving one exact delegated Custody CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DelegatedCustodyOperatorErrorV2 {
    /// The successor request or one of its nested V1 facts refused.
    Request,
    /// A supplied program/account identity was zero or aliased unsafely.
    Identity,
}

/// External infrastructure identities not already owned by the exact request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelegatedCustodyInfrastructureV2 {
    /// Selected Custody program.
    pub custody_program: Pubkey,
    /// Immutable Registry program named by the live Core Market.
    pub registry_program: Pubkey,
    /// Loader-owned programdata for the selected caller program.
    pub caller_programdata: Pubkey,
    /// Canonical vacant staging cursor proving the Realm record final.
    pub realm_staging: Pubkey,
}

/// Build the exact 14-account Custody CPI for one delegated transfer.
///
/// The caller program must invoke this instruction with its derived account 0
/// PDA signer. This host builder signs and submits nothing.
pub fn delegated_custody_transfer_cpi_v2(
    request: DelegatedCustodyRequestV2,
    infrastructure: DelegatedCustodyInfrastructureV2,
) -> Result<Instruction, DelegatedCustodyOperatorErrorV2> {
    request
        .validate()
        .map_err(|_| DelegatedCustodyOperatorErrorV2::Request)?;
    let data = request
        .encode()
        .map_err(|_| DelegatedCustodyOperatorErrorV2::Request)?;
    let custody = request.custody;
    let caller_program = Pubkey::new_from_array(custody.caller_program);
    let market = Pubkey::new_from_array(custody.market);
    let realm = Pubkey::new_from_array(custody.realm);
    let source = Pubkey::new_from_array(custody.source);
    let destination = Pubkey::new_from_array(custody.destination);
    let mint = Pubkey::new_from_array(custody.mint);
    let token_program = Pubkey::new_from_array(custody.token_program);
    if [
        infrastructure.custody_program,
        infrastructure.registry_program,
        infrastructure.caller_programdata,
        infrastructure.realm_staging,
        caller_program,
        market,
        realm,
        source,
        destination,
        mint,
        token_program,
    ]
    .iter()
    .any(|key| key.to_bytes().iter().all(|byte| *byte == 0))
        || source == destination
        || caller_program == infrastructure.registry_program
        || infrastructure.custody_program == caller_program
    {
        return Err(DelegatedCustodyOperatorErrorV2::Identity);
    }
    let request_digest = hash(&data).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        custody.release_set,
        custody.market,
        custody.caller_role,
        custody.context,
        request_digest,
    )
    .map_err(|_| DelegatedCustodyOperatorErrorV2::Identity)?;
    let caller_authority =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &caller_program).0;
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &custody.release_set],
        &infrastructure.registry_program,
    )
    .0;
    let replay_seeds = CustodyReplaySeedsV1::from_request(custody);
    let replay =
        Pubkey::find_program_address(&replay_seeds.as_slices(), &infrastructure.custody_program).0;
    let authority = Pubkey::find_program_address(
        &[
            CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
            &custody.market,
            &custody.release_set,
        ],
        &infrastructure.custody_program,
    )
    .0;
    if request.delegate_before != authority.to_bytes() {
        return Err(DelegatedCustodyOperatorErrorV2::Identity);
    }
    Ok(Instruction {
        program_id: infrastructure.custody_program,
        accounts: vec![
            AccountMeta::new_readonly(caller_authority, true),
            AccountMeta::new_readonly(market, false),
            AccountMeta::new_readonly(activation, false),
            AccountMeta::new_readonly(infrastructure.registry_program, false),
            AccountMeta::new_readonly(caller_program, false),
            AccountMeta::new_readonly(infrastructure.caller_programdata, false),
            AccountMeta::new_readonly(realm, false),
            AccountMeta::new_readonly(infrastructure.realm_staging, false),
            AccountMeta::new(replay, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(source, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: data.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use dclutch_custody_contract::{
        CompartmentV1, ContextV1, CustodyRequestV1, DELEGATED_CUSTODY_REQUEST_BYTES_V2, OperationV1,
    };
    use dclutch_release_set_contract::ExecutionRoleV1;

    use super::*;

    fn key(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }

    fn base() -> CustodyRequestV1 {
        CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: ExecutionRoleV1::Trading,
            source_compartment: CompartmentV1::External,
            destination_compartment: CompartmentV1::HoardPrincipal,
            release_set: key(1).to_bytes(),
            market: key(2).to_bytes(),
            realm: key(3).to_bytes(),
            context: key(4).to_bytes(),
            caller_program: key(5).to_bytes(),
            semantic: ContextV1 {
                candidate: key(6).to_bytes(),
                source_owner: key(7).to_bytes(),
                destination_owner: [0; 32],
                order: key(8).to_bytes(),
                parent_request_digest: key(9).to_bytes(),
                order_nonce: 10,
                generation: 11,
                page_index: 12,
                execution_index: 13,
                transfer_index: 0,
            },
            source: key(14).to_bytes(),
            destination: key(15).to_bytes(),
            source_vault_context: [0; 32],
            destination_vault_context: key(4).to_bytes(),
            mint: key(16).to_bytes(),
            token_program: key(17).to_bytes(),
            payer: [0; 32],
            rent_refund: [0; 32],
            expected_revision: 2,
            resulting_revision: 3,
            amount: 100,
            rent_lamports: 0,
        }
    }

    #[test]
    fn operator_derives_every_authority_and_exact_successor_frame() {
        let infrastructure = DelegatedCustodyInfrastructureV2 {
            custody_program: key(30),
            registry_program: key(31),
            caller_programdata: key(32),
            realm_staging: key(33),
        };
        let custody = base();
        let authority = Pubkey::find_program_address(
            &[
                CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
                &custody.market,
                &custody.release_set,
            ],
            &infrastructure.custody_program,
        )
        .0;
        let request = DelegatedCustodyRequestV2 {
            custody,
            starts_atomic_debit: true,
            terminal: true,
            delegate_before: authority.to_bytes(),
            delegate_after: [0; 32],
            total_debit: 100,
            allowance_before: 100,
            allowance_after: 0,
        };
        let instruction = delegated_custody_transfer_cpi_v2(request, infrastructure)
            .expect("delegated instruction");
        assert_eq!(instruction.accounts.len(), 14);
        assert_eq!(instruction.data.len(), DELEGATED_CUSTODY_REQUEST_BYTES_V2);
        assert_eq!(instruction.accounts[12].pubkey, authority);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[8].is_writable);
        assert!(instruction.accounts[10].is_writable);
        assert!(instruction.accounts[11].is_writable);
    }
}
