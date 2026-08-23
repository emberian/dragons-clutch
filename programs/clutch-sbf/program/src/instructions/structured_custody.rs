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

use clutch_product_series::{
    CompiledProductSeriesBundleV2, CompiledProductSeriesBundleV2Id, ContentId,
    NativeClaimBasisV1, SeriesAttachmentPlanId,
};
use clutch_retirement::{PositionAccountV3, PositionPurposeV3, ReplayV3Envelope};
use clutch_retirement::{
    admit_deletable_rent, admit_initial_rent_split, Identity32V1, PositionLifecycleV3,
    PositionV3Fields, ReplayV3EnvelopeFields, ReplayV3EnvelopeHeader, ReplayV3ExtensionSchema,
    POSITION_TOMBSTONE_V3_BYTES, POSITION_V3_BYTES, PURPOSE_REPLAY_V3_PREFIX_BYTES,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::SeriesMarketLinkAccountV1;
use clutch_structured_claim::DeploymentBinding;
use clutch_structured_claim_adapter::runtime_contract::{
    authenticate_wrapper_recipe_membership_v1, structured_descriptor_admission_receipt_v1,
    structured_owner_release_id_v1,
    DescriptorBasisV1, PositionAssetTransferPayloadV1, StructuredClaimDescriptorV2,
    StructuredClaimPayloadV1, StructuredClaimReplayExtensionV1,
    StructuredClaimRuntimeAddressesV1, StructuredMarketRootBindingV1, StructuredMarketRootV1,
    StructuredProductLineageV1, WrapperRecipeHashV1, WrapperRecipeV1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1, STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
};
use clutch_structured_claim_adapter::{
    authenticate_structured_custody_call_v1, bind_descriptor_v1,
    canonical_native_claim_id_v1, canonical_series_scoped_wrapper_product_id_v2, AccountRoleV1,
    BasePositionPdaVerifierV1, Error as StructuredAdapterError, PdaVerifierV1, RawAccountV1,
    RuntimeDeploymentsV1, StructuredCustodyPdaVerifierV1, StructuredCustodyScratchV1,
    STRUCTURED_CUSTODY_ACCOUNT_COUNT, STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::loader_state::{decode_loader_pair_v1, LoaderAccountViewV1, UPGRADEABLE_LOADER_ID};
use crate::seeds;

use super::collateral_position_v3::{
    authenticate_general_market_liabilities_v1, RuntimeSha256,
};
use super::product_artifact::authenticate_product_artifact_v1;
use super::product_market::{
    admit_series_wrapper_obligation_v1, authenticate_series_market_link_v1,
    authenticate_series_wrapper_authorization_v1, AuthenticatedSeriesMarketLinkV1,
    AuthenticatedSeriesWrapperAuthorizationV1,
};
use super::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, transfer_data,
    SYSTEM_PROGRAM_ID,
};

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

const STRUCTURED_VAULT_CREATE_ACCOUNT_COUNT: usize = 28;
const STRUCTURED_ROOT_SEED_V1: &[u8] = b"dc:structured-root:v1";
const CV_VAULT_AUTHORITY: usize = 0;
const CV_PAYER: usize = 1;
const CV_SYSTEM: usize = 2;
const CV_RENT: usize = 3;
const CV_REALM: usize = 4;
const CV_PROFILE: usize = 5;
const CV_POLICY: usize = 6;
const CV_COLLATERAL_TOKEN_PROGRAM: usize = 7;
const CV_MARKET_BINDING: usize = 8;
const CV_MARKET_RUNTIME: usize = 9;
const CV_POSITION: usize = 10;
const CV_REPLAY: usize = 11;
const CV_DESCRIPTOR: usize = 12;
const CV_MINT: usize = 13;
const CV_WRAPPER_PROGRAM: usize = 14;
const CV_WRAPPER_PROGRAM_DATA: usize = 15;
const CV_BASE_PROGRAM: usize = 16;
const CV_BASE_PROGRAM_DATA: usize = 17;
const CV_TOKEN_PROGRAM: usize = 18;
const CV_TOKEN_PROGRAM_DATA: usize = 19;
const CV_BASIS: usize = 20;
const CV_MARKET_INSTANCE: usize = 21;
const CV_HOARD: usize = 22;
const CV_CLAIM_LEDGER: usize = 23;
const CV_STRUCTURED_ROOT: usize = 24;
const CV_SERIES_LINK: usize = 25;
const CV_COMPILER_BUNDLE: usize = 26;
const CV_ATTACHMENT: usize = 27;

/// Found one funded, empty Structured PositionV3 and SCV1 Replay pair.
///
/// This is the base-private half of Structured action 1. A direct transaction
/// cannot satisfy the vault-authority signature because only the separately
/// deployed wrapper can sign that PDA. The payer is charged both complete
/// principals atop any hostile prefund; those prefunds are persisted only as
/// donation floors.
pub fn process_create(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, STRUCTURED_VAULT_CREATE_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    validate_create_privileges(program_id, accounts)?;
    let create = match clutch_structured_claim_adapter::runtime_contract::decode_structured_claim_payload_v1(
        clutch_structured_claim_adapter::runtime_contract::StructuredClaimActionV1::CreateDescriptor.tag(),
        payload,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
    {
        StructuredClaimPayloadV1::CreateDescriptor(value) => value,
        _ => return Err(ClutchError::NonCanonical.into()),
    };

    let liabilities = authenticate_general_market_liabilities_v1(
        program_id,
        &accounts[CV_REALM],
        &accounts[CV_PROFILE],
        &accounts[CV_POLICY],
        &accounts[CV_COLLATERAL_TOKEN_PROGRAM],
        &accounts[CV_MARKET_BINDING],
        &accounts[CV_MARKET_RUNTIME],
        &accounts[CV_MARKET_INSTANCE],
        &accounts[CV_HOARD],
        &accounts[CV_CLAIM_LEDGER],
        false,
        false,
    )?;
    let basis_artifact = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[CV_BASIS],
        ContentId::from_bytes(liabilities.market_binding.native_claim_basis_id.bytes()),
    )?;
    let basis = *basis_artifact.value();
    require(
        basis.outcome_count == liabilities.market_binding.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let descriptor_data = accounts[CV_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let descriptor = StructuredClaimDescriptorV2::decode(&descriptor_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(descriptor_data);
    let deployments = authenticate_create_deployments(accounts, descriptor)?;
    let market_instance_id = liabilities
        .market_instance
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let descriptor_basis = DescriptorBasisV1 {
        market: market_instance_id,
        terms_digest: basis_artifact.semantic_id().bytes(),
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
    let product_id = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim_id,
        descriptor.structured_root_id,
        descriptor.wrapper_recipe_id,
    )
    .map_err(map_adapter_error)?;
    require(
        create.native_claim_id == native_claim_id
            && create.wrapper_product_id == product_id
            && create.structured_root_id == descriptor.structured_root_id
            && create.wrapper_recipe_id == descriptor.wrapper_recipe_id
            && create.primitive == descriptor.primitive,
        ClutchError::MismatchedState,
    )?;
    let addresses = derive_runtime_addresses(
        accounts[CV_WRAPPER_PROGRAM].key,
        product_id,
        descriptor,
    )?;
    require(
        accounts[CV_DESCRIPTOR].key.to_bytes() == addresses.descriptor
            && accounts[CV_MINT].key.to_bytes() == addresses.mint
            && accounts[CV_VAULT_AUTHORITY].key.to_bytes() == addresses.vault_owner
            && accounts[CV_DESCRIPTOR].owner == accounts[CV_WRAPPER_PROGRAM].key
            && accounts[CV_DESCRIPTOR].data_len()
                == clutch_structured_claim_adapter::runtime_contract::DESCRIPTOR_ACCOUNT_BYTES
            && accounts[CV_MINT].owner == accounts[CV_TOKEN_PROGRAM].key
            && accounts[CV_MINT].data_len()
                == clutch_structured_claim_adapter::runtime_contract::WRAPPER_MINT_ACCOUNT_BYTES,
        ClutchError::MismatchedState,
    )?;
    let verifier = RuntimeStructuredPdaVerifierV1;
    let _bound = bind_descriptor_v1(
        descriptor,
        descriptor_basis,
        deployments,
        native_claim_id,
        product_id,
        addresses,
        &verifier,
    )
    .map_err(map_adapter_error)?;

    admit_structured_descriptor_root_v1(
        program_id,
        accounts,
        liabilities,
        deployments,
        descriptor,
        native_claim_id,
        create.recipe_membership,
    )?;

    found_structured_vault(program_id, accounts, liabilities, product_id, addresses.descriptor)
}

fn validate_create_privileges(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let signer = [
        true, true, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false, false, false, false, false, false,
        false, false,
    ];
    let writable = [
        false, true, false, false, false, false, false, false, false, false, true, true, false,
        false, false, false, false, false, false, false, false, false, false, false, true, true,
        false, false,
    ];
    let executable = [
        false, false, true, false, false, false, false, true, false, false, false, false, false,
        false, true, false, true, false, true, false, false, false, false, false, false, false,
        false, false,
    ];
    let mut index = 0_usize;
    while index < accounts.len() {
        require(
            accounts[index].is_signer == signer[index]
                && accounts[index].is_writable == writable[index]
                && accounts[index].executable == executable[index],
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    require_system_program(&accounts[CV_SYSTEM])?;
    require(
        *accounts[CV_BASE_PROGRAM].key == *program_id
            && accounts[CV_PAYER].key != accounts[CV_VAULT_AUTHORITY].key
            && accounts[CV_POSITION].key != accounts[CV_REPLAY].key,
        ClutchError::MismatchedState,
    )?;
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let collateral_token_alias =
                left == CV_COLLATERAL_TOKEN_PROGRAM && right == CV_TOKEN_PROGRAM;
            require(
                accounts[left].key != accounts[right].key || collateral_token_alias,
                ClutchError::MismatchedState,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

#[derive(Debug)]
struct StructuredProductAuthorityV1 {
    link: Box<AuthenticatedSeriesMarketLinkV1>,
    authorization: AuthenticatedSeriesWrapperAuthorizationV1,
    compiler_release_id: ContentId,
}

fn authenticate_structured_product_authority_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    liabilities: super::collateral_position_v3::GeneralMarketLiabilityAuthorityV1,
) -> Outcome<StructuredProductAuthorityV1> {
    let link_data = accounts[CV_SERIES_LINK]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let untrusted_link = SeriesMarketLinkAccountV1::decode(&link_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(link_data);
    let untrusted_binding = untrusted_link.state.binding();
    require(
        untrusted_binding.market_instance_id.bytes()
            == liabilities.market_binding.market_instance_v2_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let link = authenticate_series_market_link_v1(
        program_id,
        &accounts[CV_SERIES_LINK],
        untrusted_binding.series_plan_id,
        untrusted_binding.ordinal,
        untrusted_binding.market_instance_id,
        untrusted_binding.generation,
        Pubkey::new_from_array(untrusted_binding.market_root_account_id.bytes()),
        true,
    )?;
    let authorization = authenticate_series_wrapper_authorization_v1(
        program_id,
        link,
        &accounts[CV_COMPILER_BUNDLE],
        &accounts[CV_ATTACHMENT],
    )?;
    require(
        authorization.market_instance_id().bytes()
                == liabilities.market_binding.market_instance_v2_id.bytes()
            && authorization.neutral_lamport_sink().bytes()
                == liabilities.market_binding.neutral_sink.bytes()
            && authorization.rent_refund_owner().bytes() == accounts[CV_PAYER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let compiler_bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV2>(
        program_id,
        &accounts[CV_COMPILER_BUNDLE],
        authorization.compiler_bundle_id(),
    )?;
    require(
        compiler_bundle.semantic_id() == authorization.compiler_bundle_id()
            && compiler_bundle.value().native_claim_basis_id.bytes()
                == liabilities.market_binding.native_claim_basis_id.bytes()
            && compiler_bundle.value().product_template_id
                == liabilities.market_instance.product_template_id
            && compiler_bundle.value().market_genesis_profile_id
                == liabilities.market_instance.market_genesis_profile_id
            && compiler_bundle.value().market_genesis_profile_id.bytes()
                == liabilities
                    .market_binding
                    .market_genesis_profile_v2_id
                    .bytes()
            && compiler_bundle.value().price_measure_policy_id.bytes()
                == liabilities.market_binding.price_measure_policy_v1_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(StructuredProductAuthorityV1 {
        link: Box::new(link),
        authorization,
        compiler_release_id: compiler_bundle.value().product_compiler_release_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn admit_structured_descriptor_root_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    liabilities: super::collateral_position_v3::GeneralMarketLiabilityAuthorityV1,
    deployments: RuntimeDeploymentsV1,
    descriptor: StructuredClaimDescriptorV2,
    native_claim_id: [u8; 32],
    recipe_membership: clutch_structured_claim_adapter::runtime_contract::WrapperRecipeMembershipV1,
) -> Outcome<()> {
    let product = authenticate_structured_product_authority_v1(program_id, accounts, liabilities)?;
    let root_binding = structured_root_binding_v1(
        accounts,
        deployments,
        product.authorization,
        product.compiler_release_id,
    )?;
    let root_id = root_binding
        .id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root_id.bytes() == descriptor.structured_root_id,
        ClutchError::MismatchedState,
    )?;
    let recipe = WrapperRecipeV1 {
        native_claim_id,
        outcome_count: liabilities.market_binding.outcome_count,
        primitive: descriptor.primitive,
    };
    authenticate_wrapper_recipe_membership_v1(
        recipe,
        descriptor.wrapper_recipe_id,
        recipe_membership,
        product.authorization.wrapper_recipe_set_id().bytes(),
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let descriptor_body = descriptor
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let descriptor_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
            &descriptor_body,
        ])
        .to_bytes(),
    );
    let recipe_id = ContentId::from_bytes(descriptor.wrapper_recipe_id);
    let root_pda = Pubkey::find_program_address(
        &[STRUCTURED_ROOT_SEED_V1, &root_id.bytes()],
        program_id,
    );
    require(
        *accounts[CV_STRUCTURED_ROOT].key == root_pda.0,
        ClutchError::WrongPda,
    )?;

    let root_is_uninitialized = accounts[CV_STRUCTURED_ROOT].owner == &SYSTEM_PROGRAM_ID
        && accounts[CV_STRUCTURED_ROOT].data_len() == 0;
    if root_is_uninitialized {
        require(
            product.authorization.requires_product_admission(),
            ClutchError::MismatchedState,
        )?;
        let first_admission_receipt = structured_descriptor_admission_receipt_v1(
            ContentId::ZERO,
            descriptor_id,
            recipe_id,
            1,
            &RuntimeSha256,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let rebound_link = Box::new(admit_series_wrapper_obligation_v1(
            program_id,
            &accounts[CV_SERIES_LINK],
            *product.link,
            product.authorization,
            first_admission_receipt,
        )?);
        let rebound_authorization = authenticate_series_wrapper_authorization_v1(
            program_id,
            *rebound_link,
            &accounts[CV_COMPILER_BUNDLE],
            &accounts[CV_ATTACHMENT],
        )?;
        require(
            !rebound_authorization.requires_product_admission()
                && rebound_authorization.wrapper_admission_receipt_id()
                    == first_admission_receipt
                && structured_root_binding_v1(
                    accounts,
                    deployments,
                    rebound_authorization,
                    product.compiler_release_id,
                )? == root_binding,
            ClutchError::MismatchedState,
        )?;
        let rent = read_rent(&accounts[CV_RENT])?;
        let root_principal = rent.minimum_balance(STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES)?;
        require(root_principal != 0, ClutchError::WrongRentSysvar)?;
        let root_admission = admit_deletable_rent(
            id(root_pda.0.to_bytes())?,
            id(accounts[CV_PAYER].key.to_bytes())?,
            root_principal,
            accounts[CV_STRUCTURED_ROOT].lamports(),
            accounts[CV_PAYER].lamports(),
            id(rebound_authorization.neutral_lamport_sink().bytes())?,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
        let root = Box::new(StructuredMarketRootV1::initialize(
            root_binding,
            structured_product_lineage_v1(rebound_authorization),
            descriptor_id,
            recipe_id,
            root_admission.rent().refundable_principal(),
            root_admission.rent().donation_floor(),
            root_pda.1,
            &RuntimeSha256,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?);
        require(
            root.admission_transcript_id == first_admission_receipt,
            ClutchError::MismatchedState,
        )?;
        let root_bump = [root_pda.1];
        let root_id_bytes = root_id.bytes();
        let root_seeds: [&[u8]; 3] = [STRUCTURED_ROOT_SEED_V1, &root_id_bytes, &root_bump];
        create_full_principal_account(
            program_id,
            &accounts[CV_PAYER],
            &accounts[CV_STRUCTURED_ROOT],
            &accounts[CV_SYSTEM],
            root_principal,
            root_admission.account_balance_after(),
            STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
            &root_seeds,
        )?;
        write_and_reauthenticate_structured_root_v1(
            &accounts[CV_STRUCTURED_ROOT],
            &root,
            root_id,
            root_pda.1,
            program_id,
        )
    } else {
        require(
            !product.authorization.requires_product_admission()
                && accounts[CV_STRUCTURED_ROOT].owner == program_id
                && accounts[CV_STRUCTURED_ROOT].data_len()
                    == STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
            ClutchError::MismatchedState,
        )?;
        let root_data = accounts[CV_STRUCTURED_ROOT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut root = Box::new(
            StructuredMarketRootV1::decode(&root_data)
                .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?,
        );
        drop(root_data);
        require(
            root.binding == root_binding
                && root.root_bump == root_pda.1
                && root.product_lineage.product_admission_receipt_id
                    == product.authorization.wrapper_admission_receipt_id(),
            ClutchError::MismatchedState,
        )?;
        let current_donation = accounts[CV_STRUCTURED_ROOT]
            .lamports()
            .checked_sub(root.rent_principal_lamports)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            current_donation >= root.donation_floor_lamports,
            ClutchError::MismatchedState,
        )?;
        root.current_donation_lamports = current_donation;
        root = Box::new((*root)
            .admit_descriptor(
                structured_product_lineage_v1(product.authorization),
                descriptor_id,
                recipe_id,
                &RuntimeSha256,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?);
        write_and_reauthenticate_structured_root_v1(
            &accounts[CV_STRUCTURED_ROOT],
            &root,
            root_id,
            root_pda.1,
            program_id,
        )
    }
}

fn structured_root_binding_v1(
    accounts: &[AccountInfo<'_>],
    deployments: RuntimeDeploymentsV1,
    authorization: AuthenticatedSeriesWrapperAuthorizationV1,
    compiler_release_id: ContentId,
) -> Outcome<StructuredMarketRootBindingV1> {
    Ok(StructuredMarketRootBindingV1 {
        link_account: accounts[CV_SERIES_LINK].key.to_bytes(),
        series_plan_id: authorization.series_plan_id(),
        ordinal: authorization.ordinal(),
        market_instance_id: authorization.market_instance_id(),
        generation: authorization.generation(),
        attachment_plan_id: SeriesAttachmentPlanId::from_bytes(
            authorization.attachment_plan_id().bytes(),
        ),
        compiler_output_id: CompiledProductSeriesBundleV2Id::from_bytes(
            authorization.compiler_bundle_id().bytes(),
        ),
        compiler_release_id,
        capability_profile_id: authorization.capability_profile_id(),
        wrapper_recipe_set_id: authorization.wrapper_recipe_set_id(),
        owner_release_id: structured_owner_release_id_v1(deployments.binding, &RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        rent_refund_owner: authorization.rent_refund_owner(),
        neutral_lamport_sink: authorization.neutral_lamport_sink(),
    })
}

fn structured_product_lineage_v1(
    authorization: AuthenticatedSeriesWrapperAuthorizationV1,
) -> StructuredProductLineageV1 {
    StructuredProductLineageV1 {
        link_authentication_id: authorization.link_authentication_id(),
        link_semantic_id: ContentId::from_bytes(authorization.link_semantic_id().bytes()),
        product_admission_receipt_id: authorization.wrapper_admission_receipt_id(),
        product_link_transition_sequence: authorization.link_transition_sequence(),
    }
}

fn write_and_reauthenticate_structured_root_v1(
    account: &AccountInfo<'_>,
    root: &StructuredMarketRootV1,
    expected_root_id: ContentId,
    expected_bump: u8,
    program_id: &Pubkey,
) -> Outcome<()> {
    let encoded = root
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&encoded);
    let observed_data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed = StructuredMarketRootV1::decode(&observed_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let expected_lamports = observed
        .rent_principal_lamports
        .checked_add(observed.current_donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        account.owner == program_id
            && account.data_len() == STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES
            && observed == *root
            && observed.root_bump == expected_bump
            && observed
                .binding
                .id(&RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == expected_root_id
            && account.lamports() == expected_lamports,
        ClutchError::AccountCreationFailed,
    )
}

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
    let descriptor = StructuredClaimDescriptorV2::decode(&descriptor_data)
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
    let canonical_product_id = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim_id,
        descriptor.structured_root_id,
        descriptor.wrapper_recipe_id,
    )
    .map_err(map_adapter_error)?;
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

fn authenticate_create_deployments(
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV2,
) -> Outcome<RuntimeDeploymentsV1> {
    let wrapper = loader_pair(
        &accounts[CV_WRAPPER_PROGRAM],
        &accounts[CV_WRAPPER_PROGRAM_DATA],
    )?;
    let base = loader_pair(&accounts[CV_BASE_PROGRAM], &accounts[CV_BASE_PROGRAM_DATA])?;
    let token = loader_pair(&accounts[CV_TOKEN_PROGRAM], &accounts[CV_TOKEN_PROGRAM_DATA])?;
    require(
        descriptor.wrapper_program_data == wrapper.state.linked_programdata
            && descriptor.wrapper_deployment_slot == wrapper.state.deployment_slot
            && descriptor.base_program == accounts[CV_BASE_PROGRAM].key.to_bytes()
            && descriptor.base_program_data == base.state.linked_programdata
            && descriptor.base_deployment_slot == base.state.deployment_slot
            && descriptor.token_2022_program == accounts[CV_TOKEN_PROGRAM].key.to_bytes()
            && descriptor.token_2022_program_data == token.state.linked_programdata
            && descriptor.token_2022_deployment_slot == token.state.deployment_slot,
        ClutchError::AuthorizationUnavailable,
    )?;
    Ok(RuntimeDeploymentsV1 {
        binding: DeploymentBinding {
            wrapper_program: accounts[CV_WRAPPER_PROGRAM].key.to_bytes(),
            wrapper_program_data: wrapper.state.linked_programdata,
            wrapper_deployment_slot: wrapper.state.deployment_slot,
            base_program: accounts[CV_BASE_PROGRAM].key.to_bytes(),
            base_program_data: base.state.linked_programdata,
            base_deployment_slot: base.state.deployment_slot,
            token_2022_program: accounts[CV_TOKEN_PROGRAM].key.to_bytes(),
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

fn id(bytes: [u8; 32]) -> Outcome<Identity32V1> {
    Identity32V1::new(bytes).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn found_structured_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    liabilities: super::collateral_position_v3::GeneralMarketLiabilityAuthorityV1,
    product_id: [u8; 32],
    descriptor_account: [u8; 32],
) -> Outcome<()> {
    let market = liabilities.market_binding.market_instance_v2_id.bytes();
    let vault = accounts[CV_VAULT_AUTHORITY].key.to_bytes();
    let position_pda = seeds::position_v3_pda(
        program_id,
        &market,
        &vault,
        PositionPurposeV3::StructuredClaim,
        &product_id,
    );
    let replay_pda = seeds::purpose_replay_v3_pda(
        program_id,
        &position_pda.0.to_bytes(),
        PositionPurposeV3::StructuredClaim,
        &product_id,
    );
    require(
        *accounts[CV_POSITION].key == position_pda.0
            && *accounts[CV_REPLAY].key == replay_pda.0
            && !accounts[CV_POSITION].executable
            && !accounts[CV_REPLAY].executable
            && accounts[CV_POSITION].data_len() == 0
            && accounts[CV_REPLAY].data_len() == 0
            && *accounts[CV_POSITION].owner == SYSTEM_PROGRAM_ID
            && *accounts[CV_REPLAY].owner == SYSTEM_PROGRAM_ID,
        ClutchError::AlreadyInitialized,
    )?;
    let rent = read_rent(&accounts[CV_RENT])?;
    let position_minimum = rent.minimum_balance(POSITION_V3_BYTES)?;
    let tombstone_principal = rent.minimum_balance(POSITION_TOMBSTONE_V3_BYTES)?;
    let refundable_principal = position_minimum
        .checked_sub(tombstone_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let replay_bytes = PURPOSE_REPLAY_V3_PREFIX_BYTES
        .checked_add(STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let replay_principal = rent.minimum_balance(replay_bytes)?;
    require(
        position_minimum != 0
            && tombstone_principal != 0
            && refundable_principal != 0
            && replay_principal != 0,
        ClutchError::WrongRentSysvar,
    )?;
    let neutral_sink = liabilities.market_binding.neutral_sink.bytes();
    let payer = accounts[CV_PAYER].key.to_bytes();
    let position_admission = admit_initial_rent_split(
        id(position_pda.0.to_bytes())?,
        id(payer)?,
        refundable_principal,
        tombstone_principal,
        accounts[CV_POSITION].lamports(),
        accounts[CV_PAYER].lamports(),
        id(neutral_sink)?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let replay_admission = admit_deletable_rent(
        id(replay_pda.0.to_bytes())?,
        id(payer)?,
        replay_principal,
        accounts[CV_REPLAY].lamports(),
        position_admission.payer_balance_after(),
        id(neutral_sink)?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let position = PositionAccountV3::new(PositionV3Fields {
        purpose: PositionPurposeV3::StructuredClaim,
        lifecycle: PositionLifecycleV3::Open,
        outcome_count: liabilities.market_binding.outcome_count,
        stored_bump: position_pda.1,
        generation: 1,
        market_instance_id: id(market)?,
        realm_id: id(liabilities.hoard.realm_id.bytes())?,
        collateral_policy_id: id(liabilities.hoard.collateral_policy_id.bytes())?,
        collateral_release_id: id(liabilities.hoard.collateral_release_id.bytes())?,
        owner: id(vault)?,
        controller: id(vault)?,
        replay_account: id(replay_pda.0.to_bytes())?,
        purpose_binding_id: id(product_id)?,
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        native_eggs: [0; clutch_retirement::MAX_OUTCOMES],
        outstanding_reservations: 0,
        rent: position_admission.rent(),
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_semantic_id = position
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .bytes();
    let extension = StructuredClaimReplayExtensionV1::founding(
        descriptor_account,
        product_id,
        vault,
        position_semantic_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
    .encode()
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let header = ReplayV3EnvelopeHeader::new_live(
        ReplayV3EnvelopeFields {
            position_account: id(position_pda.0.to_bytes())?,
            replay_account: id(replay_pda.0.to_bytes())?,
            purpose: PositionPurposeV3::StructuredClaim,
            purpose_binding_id: id(product_id)?,
            position_generation: 1,
            next_sequence: 0,
            stored_bump: replay_pda.1,
            rent: replay_admission.rent(),
        },
        ReplayV3ExtensionSchema::new(STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        &extension,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let envelope = ReplayV3Envelope::from_header(header, &extension, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_body = position
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut replay_body = [0_u8;
        PURPOSE_REPLAY_V3_PREFIX_BYTES + STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1];
    envelope
        .encode_into(&mut replay_body, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let purpose = [u8::from(PositionPurposeV3::StructuredClaim)];
    let position_bump = [position_pda.1];
    let position_seeds: [&[u8]; 6] = [
        clutch_retirement::POSITION_V3_PDA_PREFIX,
        &market,
        &vault,
        &purpose,
        &product_id,
        &position_bump,
    ];
    create_full_principal_account(
        program_id,
        &accounts[CV_PAYER],
        &accounts[CV_POSITION],
        &accounts[CV_SYSTEM],
        position_minimum,
        position_admission.account_balance_after(),
        POSITION_V3_BYTES,
        &position_seeds,
    )?;
    let position_key = position_pda.0.to_bytes();
    let replay_bump = [replay_pda.1];
    let replay_seeds: [&[u8]; 5] = [
        clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
        &position_key,
        &purpose,
        &product_id,
        &replay_bump,
    ];
    create_full_principal_account(
        program_id,
        &accounts[CV_PAYER],
        &accounts[CV_REPLAY],
        &accounts[CV_SYSTEM],
        replay_principal,
        replay_admission.account_balance_after(),
        replay_bytes,
        &replay_seeds,
    )?;
    accounts[CV_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&position_body);
    accounts[CV_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&replay_body);
    let position_after = accounts[CV_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let replay_after = accounts[CV_REPLAY]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed_position = PositionAccountV3::decode(&position_after)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let observed_replay = ReplayV3Envelope::decode(&replay_after, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        accounts[CV_PAYER].lamports() == replay_admission.payer_balance_after()
            && observed_position == position
            && observed_replay.header() == header
            && observed_replay.extension() == extension,
        ClutchError::AccountCreationFailed,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_full_principal_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    principal: u64,
    balance_after: u64,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let payer_before = payer.lamports();
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke(&transfer, &[payer.clone(), target.clone(), system_program.clone()])
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        payer.lamports()
            == payer_before
                .checked_sub(principal)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && target.lamports() == balance_after,
        ClutchError::AccountCreationFailed,
    )?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner == program_id
            && target.lamports() == balance_after,
        ClutchError::AccountCreationFailed,
    )
}

fn authenticate_deployments(
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV2,
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
    descriptor: StructuredClaimDescriptorV2,
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
            && mint_authority.1 == descriptor.mint_authority_bump
            && vault_owner.1 == descriptor.vault_owner_bump,
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

impl WrapperRecipeHashV1 for RuntimeSha256 {
    fn hashv(&self, slices: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(slices).to_bytes()
    }
}

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
