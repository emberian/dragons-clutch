//! Executable base endpoint for one Structured canonical wrap/unwind transfer.
//!
//! This module is intentionally only the base-owned half of the operation. A
//! separately deployed, descriptor-pinned wrapper program must call it with
//! its vault PDA as a signer and must atomically perform the matching
//! Token-2022 mint or burn. This program never claims that wrapper supply moved.
//!
//! The transition writes only the two canonical Position V3 bodies and their
//! purpose-owned Replay envelopes. Position rent owner, refundable principal,
//! and donation floor are copied byte-exactly; no lamports move and no
//! prefunding becomes an economic asset.

use clutch_product_series::{ContentId, NativeClaimBasisV1};
use clutch_retirement::{PositionAccountV3, PositionPurposeV3, ReplayV3Envelope};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_structured_claim::DeploymentBinding;
use clutch_structured_claim_adapter::runtime_contract::{
    DescriptorBasisV1, PositionAssetTransferPayloadV1, StructuredClaimDescriptorV1,
    StructuredClaimReplayExtensionV1, StructuredClaimRuntimeAddressesV1,
};
use clutch_structured_claim_adapter::{
    authenticate_structured_custody_call_v1, bind_descriptor_v1,
    canonical_native_claim_id_v1, canonical_wrapper_product_id_v1, AccountRoleV1,
    BasePositionPdaVerifierV1, Error as StructuredAdapterError, PdaVerifierV1, RawAccountV1,
    RuntimeDeploymentsV1, StructuredCustodyPdaVerifierV1, StructuredCustodyScratchV1,
    STRUCTURED_CUSTODY_ACCOUNT_COUNT,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::loader_state::{decode_loader_pair_v1, LoaderAccountViewV1, UPGRADEABLE_LOADER_ID};
use crate::seeds;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, RuntimeSha256,
};
use super::product_artifact::authenticate_product_artifact_v1;

const IX_VAULT_AUTHORITY: usize = 0;
const IX_REALM: usize = 1;
const IX_PROFILE: usize = 2;
const IX_COLLATERAL_POLICY: usize = 3;
const IX_COLLATERAL_TOKEN_PROGRAM: usize = 4;
const IX_MARKET_BINDING: usize = 5;
const IX_MARKET_RUNTIME: usize = 6;
const IX_SOURCE_POSITION: usize = 7;
const IX_SOURCE_REPLAY: usize = 8;
const IX_DESTINATION_POSITION: usize = 9;
const IX_DESTINATION_REPLAY: usize = 10;
const IX_DESCRIPTOR: usize = 12;
const IX_WRAPPER_PROGRAM: usize = 13;
const IX_WRAPPER_PROGRAM_DATA: usize = 14;
const IX_BASE_PROGRAM: usize = 15;
const IX_BASE_PROGRAM_DATA: usize = 16;
const IX_TOKEN_2022_PROGRAM: usize = 17;
const IX_TOKEN_2022_PROGRAM_DATA: usize = 18;
const IX_NATIVE_CLAIM_BASIS: usize = 19;
const IX_MARKET_INSTANCE: usize = 20;
const IX_HOARD_V2: usize = 21;
const IX_CLAIM_LEDGER_V3: usize = 22;

const ACCOUNT_ROLES: [AccountRoleV1; STRUCTURED_CUSTODY_ACCOUNT_COUNT] = [
    AccountRoleV1::VaultAuthority,
    AccountRoleV1::Realm,
    AccountRoleV1::Profile,
    AccountRoleV1::CollateralPolicy,
    AccountRoleV1::CollateralTokenProgram,
    AccountRoleV1::MarketBinding,
    AccountRoleV1::MarketRuntime,
    AccountRoleV1::SourcePositionV3,
    AccountRoleV1::SourceReplayV3,
    AccountRoleV1::DestinationPositionV3,
    AccountRoleV1::DestinationReplayV3,
    AccountRoleV1::Actor,
    AccountRoleV1::Descriptor,
    AccountRoleV1::WrapperProgram,
    AccountRoleV1::WrapperProgramData,
    AccountRoleV1::BaseProgram,
    AccountRoleV1::BaseProgramData,
    AccountRoleV1::Token2022Program,
    AccountRoleV1::Token2022ProgramData,
    AccountRoleV1::NativeClaimBasisArtifact,
    AccountRoleV1::MarketInstanceArtifact,
    AccountRoleV1::HoardV2,
    AccountRoleV1::ClaimLedgerV3,
];

/// Execute General V2 action 35 after the central profile admitted its tuple.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, STRUCTURED_CUSTODY_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require(
        *accounts[IX_BASE_PROGRAM].key == *program_id,
        ClutchError::MismatchedState,
    )?;
    let transfer = clutch_structured_claim_adapter::runtime_contract::decode_position_asset_transfer_payload_v1(
        payload,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;

    // Authenticate the single Realm-selected collateral closure first. The
    // private receipt, rather than caller-authored IDs, enters Structured's
    // independent reconstruction below.
    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_COLLATERAL_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD_V2],
        &accounts[IX_CLAIM_LEDGER_V3],
        false,
        false,
    )?;
    let basis_artifact = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[IX_NATIVE_CLAIM_BASIS],
        ContentId::from_bytes(liabilities.market_binding.native_claim_basis_id.bytes()),
    )?;
    let basis = *basis_artifact.value();
    require(
        basis.outcome_count == liabilities.market_binding.outcome_count,
        ClutchError::MismatchedState,
    )?;

    let descriptor_data = accounts[IX_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let descriptor = StructuredClaimDescriptorV1::decode(&descriptor_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(descriptor_data);

    let deployments = authenticate_deployments(accounts, descriptor)?;
    let product_id = structured_replay_product(accounts)?;
    let market_instance_id = liabilities
        .market_instance
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let basis_id = basis_artifact.semantic_id().bytes();
    let descriptor_basis = DescriptorBasisV1 {
        market: market_instance_id,
        terms_digest: basis_id,
        basis_degree: basis.basis_degree,
        denominator: basis.denominator,
        outcome_count: basis.outcome_count,
    };
    let identity = clutch_structured_claim_adapter::runtime_contract::reconstruct_descriptor_identity_v1(
        &descriptor,
        descriptor_basis,
        deployments.binding,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let native_claim_id = canonical_native_claim_id_v1(&identity).map_err(map_adapter_error)?;
    let canonical_product_id =
        canonical_wrapper_product_id_v1(&identity, native_claim_id).map_err(map_adapter_error)?;
    require(product_id == canonical_product_id, ClutchError::MismatchedState)?;

    let verifier = RuntimeStructuredPdaVerifierV1;
    let addresses = derive_runtime_addresses(
        accounts[IX_WRAPPER_PROGRAM].key,
        canonical_product_id,
        descriptor,
    )?;
    require(
        addresses.descriptor == accounts[IX_DESCRIPTOR].key.to_bytes()
            && addresses.vault_owner == accounts[IX_VAULT_AUTHORITY].key.to_bytes(),
        ClutchError::WrongPda,
    )?;
    let bound_descriptor = bind_descriptor_v1(
        descriptor,
        descriptor_basis,
        deployments,
        native_claim_id,
        canonical_product_id,
        addresses,
        &verifier,
    )
    .map_err(map_adapter_error)?;

    let poststate = {
        let borrowed = accounts
            .iter()
            .map(|account| {
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
            })
            .collect::<Outcome<Vec<_>>>()?;
        let mut raw = Vec::with_capacity(STRUCTURED_CUSTODY_ACCOUNT_COUNT);
        let mut index = 0_usize;
        while index < accounts.len() {
            raw.push(RawAccountV1 {
                role: ACCOUNT_ROLES[index],
                key: accounts[index].key.to_bytes(),
                owner: accounts[index].owner.to_bytes(),
                lamports: accounts[index].lamports(),
                data: &borrowed[index],
                signer: accounts[index].is_signer,
                writable: accounts[index].is_writable,
                executable: accounts[index].executable,
            });
            index += 1;
        }
        let mut scratch = Box::new(StructuredCustodyScratchV1::ZEROED);
        authenticate_structured_custody_call_v1(
            &raw,
            &bound_descriptor,
            deployments,
            liabilities.bound,
            transfer,
            &mut scratch,
            &verifier,
        )
        .map_err(map_adapter_error)?
        .poststate()
    };

    require(
        poststate.source_position.address == accounts[IX_SOURCE_POSITION].key.to_bytes()
            && poststate.source_replay.address == accounts[IX_SOURCE_REPLAY].key.to_bytes()
            && poststate.destination_position.address
                == accounts[IX_DESTINATION_POSITION].key.to_bytes()
            && poststate.destination_replay.address
                == accounts[IX_DESTINATION_REPLAY].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    verify_rent_and_exact_transfer(accounts, poststate, transfer)?;

    // Acquire every mutable borrow before the first write. Any borrow or width
    // refusal therefore leaves all four accounts unchanged without relying on
    // a partial-write cleanup path.
    let mut source_position = accounts[IX_SOURCE_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut source_replay = accounts[IX_SOURCE_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut destination_position = accounts[IX_DESTINATION_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut destination_replay = accounts[IX_DESTINATION_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        source_position.len() == poststate.source_position.body.len()
            && source_replay.len() == usize::from(poststate.source_replay.body_len)
            && destination_position.len() == poststate.destination_position.body.len()
            && destination_replay.len()
                == usize::from(poststate.destination_replay.body_len),
        ClutchError::WrongDataLength,
    )?;
    source_position.copy_from_slice(&poststate.source_position.body);
    source_replay.copy_from_slice(
        &poststate.source_replay.body[..usize::from(poststate.source_replay.body_len)],
    );
    destination_position.copy_from_slice(&poststate.destination_position.body);
    destination_replay.copy_from_slice(
        &poststate.destination_replay.body
            [..usize::from(poststate.destination_replay.body_len)],
    );
    Ok(())
}

fn structured_replay_product(accounts: &[AccountInfo<'_>]) -> Outcome<[u8; 32]> {
    let source_position_data = accounts[IX_SOURCE_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_position_data = accounts[IX_DESTINATION_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_position = PositionAccountV3::decode(&source_position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let destination_position = PositionAccountV3::decode(&destination_position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(source_position_data);
    drop(destination_position_data);
    let replay_index = match (source_position.purpose(), destination_position.purpose()) {
        (PositionPurposeV3::StructuredClaim, PositionPurposeV3::General) => IX_SOURCE_REPLAY,
        (PositionPurposeV3::General, PositionPurposeV3::StructuredClaim) => IX_DESTINATION_REPLAY,
        _ => return Err(ClutchError::MismatchedState.into()),
    };
    let replay_data = accounts[replay_index]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let replay = ReplayV3Envelope::decode(&replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let extension = StructuredClaimReplayExtensionV1::decode(replay.extension())
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        extension.descriptor_account == accounts[IX_DESCRIPTOR].key.to_bytes()
            && extension.vault_authority == accounts[IX_VAULT_AUTHORITY].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(extension.wrapper_product_id)
}

fn authenticate_deployments(
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV1,
) -> Outcome<RuntimeDeploymentsV1> {
    let wrapper = loader_pair(
        &accounts[IX_WRAPPER_PROGRAM],
        &accounts[IX_WRAPPER_PROGRAM_DATA],
    )?;
    let base = loader_pair(
        &accounts[IX_BASE_PROGRAM],
        &accounts[IX_BASE_PROGRAM_DATA],
    )?;
    let token = loader_pair(
        &accounts[IX_TOKEN_2022_PROGRAM],
        &accounts[IX_TOKEN_2022_PROGRAM_DATA],
    )?;
    require(
        descriptor.wrapper_program_data == wrapper.state.linked_programdata
            && descriptor.wrapper_deployment_slot == wrapper.state.deployment_slot
            && descriptor.base_program == accounts[IX_BASE_PROGRAM].key.to_bytes()
            && descriptor.base_program_data == base.state.linked_programdata
            && descriptor.base_deployment_slot == base.state.deployment_slot
            && descriptor.token_2022_program == accounts[IX_TOKEN_2022_PROGRAM].key.to_bytes()
            && descriptor.token_2022_program_data == token.state.linked_programdata
            && descriptor.token_2022_deployment_slot == token.state.deployment_slot,
        ClutchError::AuthorizationUnavailable,
    )?;
    Ok(RuntimeDeploymentsV1 {
        binding: DeploymentBinding {
            wrapper_program: accounts[IX_WRAPPER_PROGRAM].key.to_bytes(),
            wrapper_program_data: wrapper.state.linked_programdata,
            wrapper_deployment_slot: wrapper.state.deployment_slot,
            base_program: accounts[IX_BASE_PROGRAM].key.to_bytes(),
            base_program_data: base.state.linked_programdata,
            base_deployment_slot: base.state.deployment_slot,
            token_2022_program: accounts[IX_TOKEN_2022_PROGRAM].key.to_bytes(),
            token_2022_program_data: token.state.linked_programdata,
            token_2022_deployment_slot: token.state.deployment_slot,
        },
        upgradeable_loader: UPGRADEABLE_LOADER_ID,
        program_owners: [UPGRADEABLE_LOADER_ID; 3],
        program_data_owners: [UPGRADEABLE_LOADER_ID; 3],
        linked_program_data: [
            wrapper.state.linked_programdata,
            base.state.linked_programdata,
            token.state.linked_programdata,
        ],
        executable_mask: 0b111,
    })
}

fn loader_pair(
    program: &AccountInfo<'_>,
    program_data: &AccountInfo<'_>,
) -> Outcome<crate::loader_state::DecodedLoaderPairV1> {
    require(
        !program.is_writable
            && !program.is_signer
            && !program_data.is_writable
            && !program_data.is_signer,
        ClutchError::MismatchedState,
    )?;
    let program_body = program
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let program_data_body = program_data
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    decode_loader_pair_v1(
        LoaderAccountViewV1::new(
            program.key.to_bytes(),
            program.owner.to_bytes(),
            program.executable,
            &program_body,
        ),
        LoaderAccountViewV1::new(
            program_data.key.to_bytes(),
            program_data.owner.to_bytes(),
            program_data.executable,
            &program_data_body,
        ),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn derive_runtime_addresses(
    wrapper_program: &Pubkey,
    product_id: [u8; 32],
    descriptor: StructuredClaimDescriptorV1,
) -> Outcome<StructuredClaimRuntimeAddressesV1> {
    let descriptor_pda = Pubkey::find_program_address(
        &[clutch_structured_claim_adapter::DESCRIPTOR_SEED, &product_id],
        wrapper_program,
    );
    let mint = Pubkey::find_program_address(
        &[clutch_structured_claim_adapter::MINT_SEED, &product_id],
        wrapper_program,
    );
    let mint_authority = Pubkey::find_program_address(
        &[clutch_structured_claim_adapter::MINT_AUTHORITY_SEED, &product_id],
        wrapper_program,
    );
    let vault_owner = Pubkey::find_program_address(
        &[clutch_structured_claim_adapter::VAULT_OWNER_SEED, &product_id],
        wrapper_program,
    );
    require(
        descriptor_pda.1 == descriptor.descriptor_bump
            && mint.1 == descriptor.mint_bump
            && mint_authority.1 == descriptor.vault_bump
            && vault_owner.1 == descriptor.vault_bump,
        ClutchError::WrongBump,
    )?;
    Ok(StructuredClaimRuntimeAddressesV1 {
        descriptor: descriptor_pda.0.to_bytes(),
        mint: mint.0.to_bytes(),
        mint_authority: mint_authority.0.to_bytes(),
        vault_owner: vault_owner.0.to_bytes(),
    })
}

fn verify_rent_and_exact_transfer(
    accounts: &[AccountInfo<'_>],
    poststate: clutch_structured_claim_adapter::StructuredCustodyPoststateV1,
    transfer: PositionAssetTransferPayloadV1,
) -> Outcome<()> {
    let source_data = accounts[IX_SOURCE_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_data = accounts[IX_DESTINATION_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_before = PositionAccountV3::decode(&source_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let destination_before = PositionAccountV3::decode(&destination_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let source_after = PositionAccountV3::decode(&poststate.source_position.body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let destination_after = PositionAccountV3::decode(&poststate.destination_position.body)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    verify_exact_transfer_closure(
        source_before,
        destination_before,
        source_after,
        destination_after,
        transfer,
    )
}

fn verify_exact_transfer_closure(
    source_before: PositionAccountV3,
    destination_before: PositionAccountV3,
    source_after: PositionAccountV3,
    destination_after: PositionAccountV3,
    transfer: PositionAssetTransferPayloadV1,
) -> Outcome<()> {
    require(
        source_before.rent() == source_after.rent()
            && destination_before.rent() == destination_after.rent()
            && source_before.cash_atoms().checked_sub(source_after.cash_atoms())
                == Some(transfer.cash_atoms)
            && destination_after
                .cash_atoms()
                .checked_sub(destination_before.cash_atoms())
                == Some(transfer.cash_atoms),
        ClutchError::AggregateClosureMismatch,
    )?;
    let source_eggs = source_before.native_eggs();
    let source_eggs_after = source_after.native_eggs();
    let destination_eggs = destination_before.native_eggs();
    let destination_eggs_after = destination_after.native_eggs();
    let mut outcome = 0_usize;
    while outcome < source_eggs.len() {
        require(
            source_eggs[outcome].checked_sub(source_eggs_after[outcome])
                == Some(transfer.internal[outcome])
                && destination_eggs_after[outcome].checked_sub(destination_eggs[outcome])
                    == Some(transfer.internal[outcome]),
            ClutchError::AggregateClosureMismatch,
        )?;
        outcome += 1;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default)]
struct RuntimeStructuredPdaVerifierV1;

impl PdaVerifierV1 for RuntimeStructuredPdaVerifierV1 {
    fn verify(
        &self,
        program: &[u8; 32],
        address: &[u8; 32],
        prefix: &[u8],
        product_id: &[u8; 32],
        bump: u8,
    ) -> bool {
        let derived = Pubkey::find_program_address(&[prefix, product_id], &Pubkey::new_from_array(*program));
        derived.0.to_bytes() == *address && derived.1 == bump
    }
}

impl BasePositionPdaVerifierV1 for RuntimeStructuredPdaVerifierV1 {
    fn verify_position_v3(
        &self,
        program: [u8; 32],
        address: [u8; 32],
        projection: clutch_retirement::PositionV3PdaSeeds,
    ) -> bool {
        let derived = seeds::position_v3_pda(
            &Pubkey::new_from_array(program),
            &projection.market_instance_id().bytes(),
            &projection.owner().bytes(),
            projection.purpose(),
            &projection.purpose_binding_id().bytes(),
        );
        derived.0.to_bytes() == address && derived.1 == projection.stored_bump()
    }

    fn verify_replay_v3(
        &self,
        program: [u8; 32],
        address: [u8; 32],
        position_account: [u8; 32],
        purpose: PositionPurposeV3,
        purpose_binding_id: [u8; 32],
        stored_bump: u8,
    ) -> bool {
        let derived = seeds::purpose_replay_v3_pda(
            &Pubkey::new_from_array(program),
            &position_account,
            purpose,
            &purpose_binding_id,
        );
        derived.0.to_bytes() == address && derived.1 == stored_bump
    }
}

impl StructuredCustodyPdaVerifierV1 for RuntimeStructuredPdaVerifierV1 {
    fn verify_realm(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        realm_id: [u8; 32],
        stored_bump: u8,
    ) -> bool {
        let derived = seeds::realm_pda(&Pubkey::new_from_array(base_program), &realm_id);
        derived.0.to_bytes() == address && derived.1 == stored_bump
    }

    fn verify_profile(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        realm_id: [u8; 32],
        profile_id: [u8; 32],
    ) -> bool {
        seeds::profile_pda(
            &Pubkey::new_from_array(base_program),
            &realm_id,
            &profile_id,
        )
        .0
        .to_bytes()
            == address
    }

    fn verify_collateral_policy(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        profile_id: [u8; 32],
        policy_id: [u8; 32],
    ) -> bool {
        seeds::policy_pda(
            &Pubkey::new_from_array(base_program),
            &profile_id,
            &policy_id,
        )
        .0
        .to_bytes()
            == address
    }

    fn verify_upgradeable_deployment(
        &self,
        upgradeable_loader: [u8; 32],
        program: &RawAccountV1<'_>,
        program_data: &RawAccountV1<'_>,
        expected_deployment_slot: u64,
    ) -> bool {
        if upgradeable_loader != UPGRADEABLE_LOADER_ID {
            return false;
        }
        decode_loader_pair_v1(
            LoaderAccountViewV1::new(program.key, program.owner, program.executable, program.data),
            LoaderAccountViewV1::new(
                program_data.key,
                program_data.owner,
                program_data.executable,
                program_data.data,
            ),
        )
        .map(|pair| pair.state.deployment_slot == expected_deployment_slot)
        .unwrap_or(false)
    }

    fn verify_market_binding(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        market_instance_id: [u8; 32],
        stored_bump: u8,
    ) -> bool {
        let derived = seeds::general_v2_market_binding_pda(
            &Pubkey::new_from_array(base_program),
            &market_instance_id,
        );
        derived.0.to_bytes() == address && derived.1 == stored_bump
    }

    fn verify_market_runtime(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        market_binding: [u8; 32],
        stored_bump: u8,
    ) -> bool {
        let derived = seeds::general_v2_market_runtime_pda(
            &Pubkey::new_from_array(base_program),
            &market_binding,
        );
        derived.0.to_bytes() == address && derived.1 == stored_bump
    }

    fn verify_hoard_v2(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        market_instance_id: [u8; 32],
        stored_bump: u8,
    ) -> bool {
        let derived =
            seeds::hoard_v2_pda(&Pubkey::new_from_array(base_program), &market_instance_id);
        derived.0.to_bytes() == address && derived.1 == stored_bump
    }

    fn verify_claim_ledger_v3(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        market_instance_id: [u8; 32],
        stored_bump: u8,
    ) -> bool {
        let derived = seeds::claim_ledger_v3_pda(
            &Pubkey::new_from_array(base_program),
            &market_instance_id,
        );
        derived.0.to_bytes() == address && derived.1 == stored_bump
    }

    fn verify_product_artifact(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        artifact_kind: u8,
        content_id: [u8; 32],
    ) -> bool {
        seeds::product_artifact_pda(
            &Pubkey::new_from_array(base_program),
            artifact_kind,
            &content_id,
        )
        .0
        .to_bytes()
            == address
    }

    fn verify_market_instance_artifact(
        &self,
        base_program: [u8; 32],
        address: [u8; 32],
        market_instance_id: [u8; 32],
    ) -> bool {
        seeds::product_artifact_pda(
            &Pubkey::new_from_array(base_program),
            ArtifactKind::MarketInstancePreimageV2.byte(),
            &market_instance_id,
        )
        .0
        .to_bytes()
            == address
    }
}

fn map_adapter_error(error: StructuredAdapterError) -> Refusal {
    let mapped = match error {
        StructuredAdapterError::InvalidInstruction
        | StructuredAdapterError::InvalidAccountData
        | StructuredAdapterError::Runtime(
            clutch_structured_claim_adapter::runtime_contract::Error::InvalidLength
            | clutch_structured_claim_adapter::runtime_contract::Error::InvalidHeader
            | clutch_structured_claim_adapter::runtime_contract::Error::NonCanonicalPadding
            | clutch_structured_claim_adapter::runtime_contract::Error::InvalidState,
        ) => ClutchError::NonCanonical,
        StructuredAdapterError::PdaMismatch => ClutchError::WrongPda,
        StructuredAdapterError::Arithmetic
        | StructuredAdapterError::Runtime(
            clutch_structured_claim_adapter::runtime_contract::Error::ArithmeticOverflow
            | clutch_structured_claim_adapter::runtime_contract::Error::ArithmeticUnderflow,
        ) => ClutchError::Arithmetic,
        StructuredAdapterError::Runtime(
            clutch_structured_claim_adapter::runtime_contract::Error::ReplayExhausted
            | clutch_structured_claim_adapter::runtime_contract::Error::InvalidReplayExtension,
        ) => ClutchError::Replay,
        StructuredAdapterError::BaseClosureMismatch
        | StructuredAdapterError::PostStateMismatch
        | StructuredAdapterError::ReceiptMismatch => ClutchError::AggregateClosureMismatch,
        StructuredAdapterError::InvalidDeployment
        | StructuredAdapterError::BaseCapabilityUnavailable
        | StructuredAdapterError::CapabilityDisabled => ClutchError::AuthorizationUnavailable,
        StructuredAdapterError::WrongFamily
        | StructuredAdapterError::WrongFamilyVersion
        | StructuredAdapterError::UnknownAction => ClutchError::UnsupportedInstruction,
        StructuredAdapterError::InvalidAccounts
        | StructuredAdapterError::DigestMismatch
        | StructuredAdapterError::Token2022Boundary
        | StructuredAdapterError::ProductBoundary
        | StructuredAdapterError::CustodyAuthorityMismatch
        | StructuredAdapterError::Runtime(_) => ClutchError::MismatchedState,
    };
    Refusal::Adapter(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_retirement::{
        Identity32V1, PositionLifecycleV3, PositionV3Fields, RentSplitV2, MAX_OUTCOMES,
    };

    fn identity(byte: u8) -> Identity32V1 {
        Identity32V1::new([byte; 32]).unwrap()
    }

    fn position(cash_atoms: u64, egg_atoms: u64, rent: RentSplitV2) -> PositionAccountV3 {
        let mut native_eggs = [0; MAX_OUTCOMES];
        native_eggs[0] = egg_atoms;
        PositionAccountV3::new(PositionV3Fields {
            purpose: PositionPurposeV3::General,
            lifecycle: PositionLifecycleV3::Open,
            outcome_count: 1,
            stored_bump: 1,
            generation: 1,
            market_instance_id: identity(1),
            realm_id: identity(2),
            collateral_policy_id: identity(3),
            collateral_release_id: identity(4),
            owner: identity(5),
            controller: identity(6),
            replay_account: identity(7),
            purpose_binding_id: identity(8),
            cash_atoms,
            reserved_cash_atoms: 0,
            native_eggs,
            outstanding_reservations: 0,
            rent,
        })
        .unwrap()
    }

    fn exact_transfer(cash_atoms: u64, egg_atoms: u64) -> PositionAssetTransferPayloadV1 {
        let mut internal = [0; clutch_structured_claim_adapter::runtime_contract::MAX_OUTCOMES];
        internal[0] = egg_atoms;
        PositionAssetTransferPayloadV1 {
            market: [1; 32],
            source_owner: [2; 32],
            destination_owner: [3; 32],
            source_generation: 1,
            destination_generation: 1,
            source_replay_sequence: 0,
            destination_replay_sequence: 0,
            cash_atoms,
            internal,
            phase_policy: clutch_structured_claim_adapter::runtime_contract::AssetTransferPhasePolicyV1::ActiveOnly,
            authority_kind: clutch_structured_claim_adapter::runtime_contract::PositionAssetTransferAuthorityKindV1::StructuredCustody,
            authority_id: [9; 32],
        }
    }

    #[test]
    fn exact_profile_action_is_not_a_structured_family_mint_claim() {
        assert!(crate::capabilities::extension_intent_action_enabled(74, 1, 35));
        for action in 1..=8 {
            assert!(!crate::capabilities::extension_intent_action_enabled(75, 1, action));
        }
    }

    #[test]
    fn wrapper_pda_verifier_rejects_wrong_address_and_bump() {
        let verifier = RuntimeStructuredPdaVerifierV1;
        let program = Pubkey::new_from_array([11; 32]);
        let product = [7; 32];
        let (address, bump) = Pubkey::find_program_address(
            &[clutch_structured_claim_adapter::DESCRIPTOR_SEED, &product],
            &program,
        );
        assert!(verifier.verify(
            &program.to_bytes(),
            &address.to_bytes(),
            clutch_structured_claim_adapter::DESCRIPTOR_SEED,
            &product,
            bump,
        ));
        assert!(!verifier.verify(
            &program.to_bytes(),
            &[9; 32],
            clutch_structured_claim_adapter::DESCRIPTOR_SEED,
            &product,
            bump,
        ));
        assert!(!verifier.verify(
            &program.to_bytes(),
            &address.to_bytes(),
            clutch_structured_claim_adapter::DESCRIPTOR_SEED,
            &product,
            bump.wrapping_sub(1),
        ));
    }

    #[test]
    fn exact_transfer_closure_refuses_off_by_one_and_rent_compartment_changes() {
        let rent = RentSplitV2 {
            payer: identity(10),
            refundable_live_principal: 100,
            permanent_tombstone_principal: 20,
            donation_floor: 7,
        };
        let transfer = exact_transfer(5, 3);
        let source_before = position(20, 10, rent);
        let destination_before = position(4, 1, rent);
        let source_after = position(15, 7, rent);
        let destination_after = position(9, 4, rent);
        assert!(verify_exact_transfer_closure(
            source_before,
            destination_before,
            source_after,
            destination_after,
            transfer,
        )
        .is_ok());

        assert!(verify_exact_transfer_closure(
            source_before,
            destination_before,
            position(14, 7, rent),
            destination_after,
            transfer,
        )
        .is_err());

        let changed_rent = RentSplitV2 {
            donation_floor: 8,
            ..rent
        };
        assert!(verify_exact_transfer_closure(
            source_before,
            destination_before,
            position(15, 7, changed_rent),
            destination_after,
            transfer,
        )
        .is_err());
    }
}
