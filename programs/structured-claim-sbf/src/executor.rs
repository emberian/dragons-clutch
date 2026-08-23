//! Exact account loading, CPI execution, and authoritative post-reconciliation.

use clutch_collateral_adapter_v2::{
    ClaimLedgerV3, HoardV2, MarketLiabilityLifecycleV1, ResolutionStateV5, ResolutionV5,
};
use clutch_product_series::{
    CompiledProductSeriesBundleV5, ContentId, FixedCodec, MarketInstancePreimageV2,
    NativeClaimBasisV1, RegistryCapabilityProfileV4,
    SeriesAttachmentPlanV4, SeriesLinkObligationStatusV1, SeriesLinkObligationV1,
    SeriesMarketLinkPhaseV1,
};
use clutch_retirement::{
    PositionAccountV3, PositionPurposeV3, PositionV3Sha256Backend, ReplayV3Envelope,
    ReplayV3HashBackend,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{
    series_market_link_authentication_id_v1, SeriesMarketLinkAccountV1,
    SeriesRegistryAccountV2, SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
    SERIES_REGISTRY_ACCOUNT_BYTES_V2,
};
use clutch_solana_layout::registry::{
    ExtensionAction, ExtensionFamily, GeneralV2Action, StructuredClaimAction,
};
use clutch_solana_reference::{ExtensionEnvelope, ExtensionRequest};
use clutch_structured_claim::{ClaimVector, DeploymentBinding};
use clutch_structured_claim_adapter::runtime_contract::{
    AssetTransferPhasePolicyV1, CreateDescriptorPayloadV1, DescriptorBasisV1,
    DescriptorStateV1, PositionAssetTransferAuthorityKindV1,
    PositionAssetTransferPayloadV1, StructuredClaimActionV1, StructuredClaimDescriptorV2,
    StructuredClaimPayloadV1, StructuredClaimRuntimeAddressesV1,
    StructuredClaimReplayExtensionStateV1, StructuredClaimReplayExtensionV1,
    StructuredCustodyCallProjectionV1, StructuredMarketRootV1, WrapperQuantityPayloadV1,
    WrapperRecipeHashV1, DESCRIPTOR_ACCOUNT_BYTES, DESCRIPTOR_ACCOUNT_TAG,
    DESCRIPTOR_ACCOUNT_VERSION, STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES,
    STRUCTURED_CUSTODY_CALL_V1_DOMAIN, STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
    WRAPPER_MINT_ACCOUNT_BYTES, structured_descriptor_admission_receipt_v1,
    structured_owner_release_id_v2,
};
use clutch_structured_claim_adapter::{
    admit_runtime_envelope_v1, bind_descriptor_v1, canonical_native_claim_id_v1,
    canonical_series_scoped_wrapper_product_id_v2, decode_canonical_wrapper_mint_v1,
    decode_canonical_wrapper_token_v1, plan_token_2022_cpi_v1,
    PdaVerifierV1, RuntimeDeploymentsV1, Token2022CpiV1, Token2022InstructionPlanV1,
    DESCRIPTOR_SEED, MINT_AUTHORITY_SEED, MINT_SEED, VAULT_OWNER_SEED,
    STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_CUSTODY_CLAIM_LEDGER_BODY_DOMAIN_V1,
    STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1, STRUCTURED_CUSTODY_HOARD_BODY_DOMAIN_V1,
    STRUCTURED_CUSTODY_MARKET_BINDING_BODY_DOMAIN_V1,
    STRUCTURED_CUSTODY_MARKET_RUNTIME_BODY_DOMAIN_V1,
    STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;
use solana_sha256_hasher::hashv;

use crate::error::{Result, WrapperError};
use crate::loader::{authenticate_release_v2, AuthenticatedReleaseV2, UPGRADEABLE_LOADER_ID};
use crate::system::{create_permanent_pda, rent};

const CREATE_ACCOUNT_COUNT: usize = 33;
const CANONICAL_ACCOUNT_COUNT: usize = 29;
const FULL_VECTOR_CORE_ACCOUNT_COUNT: usize = 28;
const FULL_VECTOR_ACCOUNT_COUNT: usize = 31;
const TERMINAL_REDEMPTION_ACCOUNT_COUNT: usize = 32;

const VAULT_AUTHORITY: usize = 0;
const PAYER: usize = 1;
const SYSTEM: usize = 2;
const RENT: usize = 3;
const CREATE_REALM: usize = 4;
const CREATE_PROFILE: usize = 5;
const CREATE_POLICY: usize = 6;
const CREATE_COLLATERAL_TOKEN: usize = 7;
const CREATE_BINDING: usize = 8;
const CREATE_RUNTIME: usize = 9;
const CREATE_POSITION: usize = 10;
const CREATE_REPLAY: usize = 11;
const CREATE_DESCRIPTOR: usize = 12;
const CREATE_MINT: usize = 13;
const CREATE_WRAPPER_PROGRAM: usize = 14;
const CREATE_WRAPPER_DATA: usize = 15;
const CREATE_BASE_PROGRAM: usize = 16;
const CREATE_BASE_DATA: usize = 17;
const CREATE_TOKEN_PROGRAM: usize = 18;
const CREATE_TOKEN_DATA: usize = 19;
const CREATE_BASIS: usize = 20;
const CREATE_MARKET: usize = 21;
const CREATE_STRUCTURED_ROOT: usize = 24;
const CREATE_SERIES_LINK: usize = 25;
const CREATE_COMPILER_BUNDLE: usize = 26;
const CREATE_ATTACHMENT: usize = 27;
const CREATE_SERIES_REGISTRY_V2: usize = 28;
const CREATE_REGISTRY_RELEASE_V2: usize = 29;
const CREATE_CAPABILITY_PROFILE_V4: usize = 30;
const CREATE_WRAPPER_RELEASE_V2: usize = 31;
const CREATE_TOKEN_RELEASE_V2: usize = 32;
const STRUCTURED_ROOT_SEED_V1: &[u8] = b"dc:structured-root:v1";

const C_DESCRIPTOR: usize = 12;
const C_WRAPPER_PROGRAM: usize = 13;
const C_WRAPPER_DATA: usize = 14;
const C_BASE_PROGRAM: usize = 15;
const C_BASE_DATA: usize = 16;
const C_TOKEN_PROGRAM: usize = 17;
const C_TOKEN_DATA: usize = 18;
const C_BASIS: usize = 19;
const C_MARKET: usize = 20;
const C_HOARD: usize = 21;
const C_LEDGER: usize = 22;
const C_MINT: usize = 23;
const C_HOLDER: usize = 24;
const C_MINT_AUTHORITY: usize = 25;
const CANONICAL_WRAPPER_RELEASE_V2: usize = 23;
const CANONICAL_BASE_RELEASE_V2: usize = 24;
const CANONICAL_TOKEN_RELEASE_V2: usize = 25;
const CANONICAL_MINT: usize = 26;
const CANONICAL_HOLDER: usize = 27;
const CANONICAL_MINT_AUTHORITY: usize = 28;
const C_ACTOR: usize = 11;
const C_COLLATERAL_MINT: usize = 26;
const C_HOARD_TOKEN: usize = 27;
const C_WRAPPER_RELEASE_V2: usize = 28;
const C_BASE_RELEASE_V2: usize = 29;
const C_TOKEN_RELEASE_V2: usize = 30;
const C_RESOLUTION_V5: usize = 31;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedStructuredDeploymentsV2 {
    runtime: RuntimeDeploymentsV1,
    wrapper_release_id: ContentId,
    base_release_id: ContentId,
    token_release_id: ContentId,
    owner_release_id: ContentId,
}

/// Process the exact enabled wrapper profile.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo<'_>], input: &[u8]) -> Result<()> {
    let envelope = admit_runtime_envelope_v1(input).map_err(|_| WrapperError::Instruction)?;
    let payload = envelope
        .decode_payload()
        .map_err(|_| WrapperError::Instruction)?;
    match payload {
        StructuredClaimPayloadV1::CreateDescriptor(value) => {
            create(program_id, accounts, value)
        }
        StructuredClaimPayloadV1::WrapCanonical(value) => {
            canonical(program_id, accounts, StructuredClaimActionV1::WrapCanonical, value)
        }
        StructuredClaimPayloadV1::UnwrapCanonical(value) => canonical(
            program_id,
            accounts,
            StructuredClaimActionV1::UnwrapCanonical,
            value,
        ),
        StructuredClaimPayloadV1::WrapFull(value) => {
            full_vector(program_id, accounts, StructuredClaimActionV1::WrapFull, value)
        }
        StructuredClaimPayloadV1::UnwrapFull(value) => full_vector(
            program_id,
            accounts,
            StructuredClaimActionV1::UnwrapFull,
            value,
        ),
        StructuredClaimPayloadV1::RedeemTerminal(value) => full_vector(
            program_id,
            accounts,
            StructuredClaimActionV1::RedeemTerminal,
            value,
        ),
        _ => Err(WrapperError::Instruction),
    }
}

fn create(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    payload: CreateDescriptorPayloadV1,
) -> Result<()> {
    validate_create_accounts(program_id, accounts)?;
    let deployments = create_deployments(program_id, accounts)?;
    let basis_data = accounts[CREATE_BASIS]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let mut basis = Box::new(NativeClaimBasisV1::ZEROED);
    NativeClaimBasisV1::decode_into(&basis_data, &mut basis)
        .map_err(|_| WrapperError::Identity)?;
    let basis_id = hashv(&[
        clutch_product_series::NATIVE_CLAIM_BASIS_DOMAIN,
        &basis_data,
    ])
    .to_bytes();
    drop(basis_data);
    let market_data = accounts[CREATE_MARKET]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let market = MarketInstancePreimageV2::decode(&market_data).map_err(|_| WrapperError::Identity)?;
    drop(market_data);
    let market_id = market.id().map_err(|_| WrapperError::Identity)?.bytes();
    let binding = DescriptorBasisV1 {
        market: market_id,
        terms_digest: basis_id,
        basis_degree: basis.basis_degree,
        denominator: basis.denominator,
        outcome_count: basis.outcome_count,
    };
    let mut descriptor = StructuredClaimDescriptorV2 {
        tag: DESCRIPTOR_ACCOUNT_TAG,
        version: DESCRIPTOR_ACCOUNT_VERSION,
        flags: 0,
        base_program: deployments.runtime.binding.base_program,
        base_program_data: deployments.runtime.binding.base_program_data,
        base_deployment_slot: deployments.runtime.binding.base_deployment_slot,
        wrapper_program_data: deployments.runtime.binding.wrapper_program_data,
        wrapper_deployment_slot: deployments.runtime.binding.wrapper_deployment_slot,
        token_2022_program: deployments.runtime.binding.token_2022_program,
        token_2022_program_data: deployments.runtime.binding.token_2022_program_data,
        token_2022_deployment_slot: deployments.runtime.binding.token_2022_deployment_slot,
        market: market_id,
        terms_digest: basis_id,
        structured_root_id: payload.structured_root_id,
        wrapper_recipe_id: payload.wrapper_recipe_id,
        primitive: payload.primitive,
        state: DescriptorStateV1::Active,
        descriptor_bump: 0,
        mint_bump: 0,
        mint_authority_bump: 0,
        vault_owner_bump: 0,
    };
    let identity = clutch_structured_claim_adapter::runtime_contract::reconstruct_descriptor_identity_v1(
        &descriptor,
        binding,
        deployments.runtime.binding,
    )
    .map_err(|_| WrapperError::Identity)?;
    let native_claim_id = canonical_native_claim_id_v1(&identity)
        .map_err(|_| WrapperError::Identity)?;
    let product_id = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim_id,
        descriptor.structured_root_id,
        descriptor.wrapper_recipe_id,
    )
    .map_err(|_| WrapperError::Identity)?;
    if native_claim_id != payload.native_claim_id || product_id != payload.wrapper_product_id {
        return Err(WrapperError::Identity);
    }
    let addresses = derive_addresses(program_id, product_id);
    descriptor.descriptor_bump = addresses.descriptor.1;
    descriptor.mint_bump = addresses.mint.1;
    descriptor.mint_authority_bump = addresses.mint_authority.1;
    descriptor.vault_owner_bump = addresses.vault_owner.1;
    require_key(&accounts[CREATE_DESCRIPTOR], addresses.descriptor.0)?;
    require_key(&accounts[CREATE_MINT], addresses.mint.0)?;
    require_key(&accounts[VAULT_AUTHORITY], addresses.vault_owner.0)?;
    require_key(&accounts[CREATE_POSITION], position_pda(
        accounts[CREATE_BASE_PROGRAM].key,
        market_id,
        addresses.vault_owner.0.to_bytes(),
        product_id,
    ).0)?;
    let expected_replay = replay_pda(
        accounts[CREATE_BASE_PROGRAM].key,
        accounts[CREATE_POSITION].key.to_bytes(),
        product_id,
    );
    require_key(&accounts[CREATE_REPLAY], expected_replay.0)?;
    let bound = bind_descriptor_v1(
        descriptor,
        binding,
        deployments.runtime,
        native_claim_id,
        product_id,
        StructuredClaimRuntimeAddressesV1 {
            descriptor: addresses.descriptor.0.to_bytes(),
            mint: addresses.mint.0.to_bytes(),
            mint_authority: addresses.mint_authority.0.to_bytes(),
            vault_owner: addresses.vault_owner.0.to_bytes(),
        },
        &RuntimePdaVerifier,
    )
    .map_err(|_| WrapperError::Identity)?;
    let descriptor_body = descriptor.encode().map_err(|_| WrapperError::Identity)?;
    let current_rent = rent(&accounts[RENT])?;
    let descriptor_minimum = current_rent.minimum_balance(DESCRIPTOR_ACCOUNT_BYTES);
    let mint_minimum = current_rent.minimum_balance(WRAPPER_MINT_ACCOUNT_BYTES);
    let descriptor_bump = [descriptor.descriptor_bump];
    let descriptor_seeds: [&[u8]; 3] = [DESCRIPTOR_SEED, &product_id, &descriptor_bump];
    create_permanent_pda(
        &accounts[PAYER],
        &accounts[CREATE_DESCRIPTOR],
        &accounts[SYSTEM],
        program_id,
        DESCRIPTOR_ACCOUNT_BYTES,
        descriptor_minimum,
        &descriptor_seeds,
    )?;
    let mint_bump = [descriptor.mint_bump];
    let mint_seeds: [&[u8]; 3] = [MINT_SEED, &product_id, &mint_bump];
    create_permanent_pda(
        &accounts[PAYER],
        &accounts[CREATE_MINT],
        &accounts[SYSTEM],
        accounts[CREATE_TOKEN_PROGRAM].key,
        WRAPPER_MINT_ACCOUNT_BYTES,
        mint_minimum,
        &mint_seeds,
    )?;
    let initialize = plan_token_2022_cpi_v1(
        accounts[CREATE_TOKEN_PROGRAM].key.to_bytes(),
        Token2022CpiV1::InitializeMint {
            token_program: accounts[CREATE_TOKEN_PROGRAM].key.to_bytes(),
            mint: accounts[CREATE_MINT].key.to_bytes(),
            mint_authority: addresses.mint_authority.0.to_bytes(),
        },
    )
    .map_err(|_| WrapperError::Token2022)?;
    invoke_token_plan(&initialize, accounts, &[])?;
    accounts[CREATE_DESCRIPTOR]
        .try_borrow_mut_data()
        .map_err(|_| WrapperError::Borrow)?
        .copy_from_slice(&descriptor_body);
    let root_before = structured_root_prestate(accounts, descriptor)?;
    invoke_base_create(accounts, &payload)?;
    reconcile_create(
        accounts,
        &bound,
        product_id,
        market,
        root_before,
        deployments,
    )?;
    Ok(())
}

fn canonical(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: StructuredClaimActionV1,
    payload: WrapperQuantityPayloadV1,
) -> Result<()> {
    validate_canonical_accounts(program_id, accounts)?;
    let (mint_index, holder_index, mint_authority_index) = wrapper_token_indices(accounts)?;
    let (bound, descriptor) = load_bound_descriptor(program_id, accounts, payload.wrapper_product_id)?;
    let source_before = decode_position(&accounts[7])?;
    let destination_before = decode_position(&accounts[9])?;
    let source_replay_before = decode_replay(&accounts[8])?;
    let destination_replay_before = decode_replay(&accounts[10])?;
    let (user, vault) = match action {
        StructuredClaimActionV1::WrapCanonical => (source_before, destination_before),
        StructuredClaimActionV1::UnwrapCanonical => (destination_before, source_before),
        _ => return Err(WrapperError::Instruction),
    };
    if user.purpose() != PositionPurposeV3::General
        || vault.purpose() != PositionPurposeV3::StructuredClaim
        || user.owner().bytes() != accounts[C_ACTOR].key.to_bytes()
        || vault.owner().bytes() != accounts[VAULT_AUTHORITY].key.to_bytes()
        || vault.purpose_binding_id().bytes() != payload.wrapper_product_id
        || user.market_instance_id().bytes() != descriptor.market
        || vault.market_instance_id().bytes() != descriptor.market
        || user.generation() != payload.user_generation
        || vault.generation() != payload.vault_generation
        || source_replay_before.header.next_sequence()
            != match action {
                StructuredClaimActionV1::WrapCanonical => payload.user_replay_sequence,
                StructuredClaimActionV1::UnwrapCanonical => payload.vault_replay_sequence,
                _ => return Err(WrapperError::Instruction),
            }
        || destination_replay_before.header.next_sequence()
            != match action {
                StructuredClaimActionV1::WrapCanonical => payload.vault_replay_sequence,
                StructuredClaimActionV1::UnwrapCanonical => payload.user_replay_sequence,
                _ => return Err(WrapperError::Instruction),
            }
    {
        return Err(WrapperError::Identity);
    }
    let backing = ClaimVector {
        outcome_count: user.outcome_count(),
        coefficients: descriptor.primitive,
    }
    .backing_plan()
    .map_err(|_| WrapperError::Identity)?;
    let cash = payload
        .quantity
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(WrapperError::Arithmetic)?;
    let mut internal = [0_u64; clutch_structured_claim::MAX_OUTCOMES];
    let mut outcome = 0_usize;
    while outcome < internal.len() {
        internal[outcome] = payload
            .quantity
            .checked_mul(backing.residual_eggs_per_wrapper[outcome])
            .ok_or(WrapperError::Arithmetic)?;
        outcome += 1;
    }
    let neutral_transfer = PositionAssetTransferPayloadV1 {
        market: descriptor.market,
        source_owner: source_before.owner().bytes(),
        destination_owner: destination_before.owner().bytes(),
        source_generation: source_before.generation(),
        destination_generation: destination_before.generation(),
        source_replay_sequence: source_replay_before.header.next_sequence(),
        destination_replay_sequence: destination_replay_before.header.next_sequence(),
        cash_atoms: cash,
        internal,
        phase_policy: AssetTransferPhasePolicyV1::ActiveOrResolved,
        authority_kind: PositionAssetTransferAuthorityKindV1::StructuredCustody,
        authority_id: [0; 32],
    };
    let final_transfer = custody_authority(
        accounts,
        &bound,
        action,
        neutral_transfer,
        source_before,
        source_replay_before,
        destination_before,
        destination_replay_before,
    )?;
    let mint_before = decode_mint(accounts, &bound)?;
    let holder_before = decode_holder(accounts, &bound)?;
    let supply_after = match action {
        StructuredClaimActionV1::WrapCanonical => mint_before
            .supply
            .checked_add(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        StructuredClaimActionV1::UnwrapCanonical => mint_before
            .supply
            .checked_sub(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        _ => return Err(WrapperError::Instruction),
    };
    let holder_after = match action {
        StructuredClaimActionV1::WrapCanonical => holder_before
            .amount
            .checked_add(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        StructuredClaimActionV1::UnwrapCanonical => holder_before
            .amount
            .checked_sub(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        _ => return Err(WrapperError::Instruction),
    };
    let token_operation = match action {
        StructuredClaimActionV1::WrapCanonical => Token2022CpiV1::MintChecked {
            mint: accounts[mint_index].key.to_bytes(),
            token: accounts[holder_index].key.to_bytes(),
            authority: accounts[mint_authority_index].key.to_bytes(),
            quantity: payload.quantity,
            supply_before: mint_before.supply,
            supply_after,
            holder_before: holder_before.amount,
            holder_after,
        },
        StructuredClaimActionV1::UnwrapCanonical => Token2022CpiV1::BurnChecked {
            mint: accounts[mint_index].key.to_bytes(),
            token: accounts[holder_index].key.to_bytes(),
            authority: accounts[C_ACTOR].key.to_bytes(),
            quantity: payload.quantity,
            supply_before: mint_before.supply,
            supply_after,
            holder_before: holder_before.amount,
            holder_after,
        },
        _ => return Err(WrapperError::Instruction),
    };
    let token_plan = plan_token_2022_cpi_v1(
        accounts[C_TOKEN_PROGRAM].key.to_bytes(),
        token_operation,
    )
    .map_err(|_| WrapperError::Token2022)?;
    match action {
        StructuredClaimActionV1::WrapCanonical => {
            invoke_base_transfer(accounts, final_transfer, payload.wrapper_product_id)?;
            reconcile_base_delta(
                accounts,
                source_before,
                source_replay_before,
                destination_before,
                destination_replay_before,
                final_transfer,
            )?;
            let bump = [descriptor.mint_authority_bump];
            let signer: [&[u8]; 3] = [MINT_AUTHORITY_SEED, &payload.wrapper_product_id, &bump];
            invoke_token_plan(&token_plan, accounts, &[&signer])?;
        }
        StructuredClaimActionV1::UnwrapCanonical => {
            invoke_token_plan(&token_plan, accounts, &[])?;
            invoke_base_transfer(accounts, final_transfer, payload.wrapper_product_id)?;
            reconcile_base_delta(
                accounts,
                source_before,
                source_replay_before,
                destination_before,
                destination_replay_before,
                final_transfer,
            )?;
        }
        _ => return Err(WrapperError::Instruction),
    }
    let mint_observed = decode_mint(accounts, &bound)?;
    let holder_observed = decode_holder(accounts, &bound)?;
    if mint_observed.supply != supply_after
        || holder_observed.amount != holder_after
        || mint_observed.mint_authority != mint_before.mint_authority
        || holder_observed.mint != holder_before.mint
        || holder_observed.owner != holder_before.owner
    {
        return Err(WrapperError::Token2022);
    }
    Ok(())
}

fn full_vector(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: StructuredClaimActionV1,
    payload: WrapperQuantityPayloadV1,
) -> Result<()> {
    validate_full_vector_accounts(program_id, accounts, action)?;
    if !matches!(
        action,
        StructuredClaimActionV1::WrapFull
            | StructuredClaimActionV1::UnwrapFull
            | StructuredClaimActionV1::RedeemTerminal
    ) {
        return Err(WrapperError::Instruction);
    }
    let (bound, descriptor) =
        load_bound_descriptor(program_id, accounts, payload.wrapper_product_id)?;
    let source_before = decode_position(&accounts[7])?;
    let destination_before = decode_position(&accounts[9])?;
    let source_replay_before = decode_replay(&accounts[8])?;
    let destination_replay_before = decode_replay(&accounts[10])?;
    let (user_before, user_replay_before, vault_before, vault_replay_before) = match action {
        StructuredClaimActionV1::WrapFull => (
            source_before,
            source_replay_before,
            destination_before,
            destination_replay_before,
        ),
        StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => (
            destination_before,
            destination_replay_before,
            source_before,
            source_replay_before,
        ),
        _ => return Err(WrapperError::Instruction),
    };
    if user_before.purpose() != PositionPurposeV3::General
        || vault_before.purpose() != PositionPurposeV3::StructuredClaim
        || user_before.owner().bytes() != accounts[C_ACTOR].key.to_bytes()
        || vault_before.owner().bytes() != accounts[VAULT_AUTHORITY].key.to_bytes()
        || vault_before.purpose_binding_id().bytes() != payload.wrapper_product_id
        || user_before.market_instance_id().bytes() != descriptor.market
        || vault_before.market_instance_id().bytes() != descriptor.market
        || user_before.generation() != payload.user_generation
        || vault_before.generation() != payload.vault_generation
        || user_replay_before.header.next_sequence() != payload.user_replay_sequence
        || vault_replay_before.header.next_sequence() != payload.vault_replay_sequence
    {
        return Err(WrapperError::Identity);
    }
    let backing = ClaimVector {
        outcome_count: user_before.outcome_count(),
        coefficients: descriptor.primitive,
    }
    .backing_plan()
    .map_err(|_| WrapperError::Identity)?;
    let complete_set_atoms = payload
        .quantity
        .checked_mul(backing.cash_per_wrapper)
        .ok_or(WrapperError::Arithmetic)?;
    let mut full = [0_u64; clutch_structured_claim::MAX_OUTCOMES];
    let mut residual = [0_u64; clutch_structured_claim::MAX_OUTCOMES];
    let mut index = 0usize;
    while index < usize::from(backing.outcome_count) {
        full[index] = payload
            .quantity
            .checked_mul(descriptor.primitive[index])
            .ok_or(WrapperError::Arithmetic)?;
        residual[index] = payload
            .quantity
            .checked_mul(backing.residual_eggs_per_wrapper[index])
            .ok_or(WrapperError::Arithmetic)?;
        index += 1;
    }
    let hoard_before = decode_hoard(accounts)?;
    let claim_ledger_before = decode_claim_ledger(accounts)?;
    let resolution = if action == StructuredClaimActionV1::RedeemTerminal {
        Some(decode_resolution(accounts, hoard_before, claim_ledger_before)?)
    } else {
        None
    };
    let (expected_user, expected_vault, expected_hoard, expected_claim_ledger) =
        expected_full_vector_successors(
            action,
            user_before,
            vault_before,
            hoard_before,
            claim_ledger_before,
            complete_set_atoms,
            full,
            residual,
            resolution,
        )?;
    let mint_before = decode_mint(accounts, &bound)?;
    let holder_before = decode_holder(accounts, &bound)?;
    let supply_after = match action {
        StructuredClaimActionV1::WrapFull => mint_before
            .supply
            .checked_add(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => mint_before
            .supply
            .checked_sub(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        _ => return Err(WrapperError::Instruction),
    };
    let holder_after = match action {
        StructuredClaimActionV1::WrapFull => holder_before
            .amount
            .checked_add(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => holder_before
            .amount
            .checked_sub(payload.quantity)
            .ok_or(WrapperError::Arithmetic)?,
        _ => return Err(WrapperError::Instruction),
    };
    let token_operation = match action {
        StructuredClaimActionV1::WrapFull => Token2022CpiV1::MintChecked {
            mint: accounts[C_MINT].key.to_bytes(),
            token: accounts[C_HOLDER].key.to_bytes(),
            authority: accounts[C_MINT_AUTHORITY].key.to_bytes(),
            quantity: payload.quantity,
            supply_before: mint_before.supply,
            supply_after,
            holder_before: holder_before.amount,
            holder_after,
        },
        StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => Token2022CpiV1::BurnChecked {
            mint: accounts[C_MINT].key.to_bytes(),
            token: accounts[C_HOLDER].key.to_bytes(),
            authority: accounts[C_ACTOR].key.to_bytes(),
            quantity: payload.quantity,
            supply_before: mint_before.supply,
            supply_after,
            holder_before: holder_before.amount,
            holder_after,
        },
        _ => return Err(WrapperError::Instruction),
    };
    let token_plan = plan_token_2022_cpi_v1(
        accounts[C_TOKEN_PROGRAM].key.to_bytes(),
        token_operation,
    )
    .map_err(|_| WrapperError::Token2022)?;
    match action {
        StructuredClaimActionV1::WrapFull => {
            invoke_base_full_vector(accounts, action, payload)?;
            reconcile_full_vector_base(
                accounts,
                action,
                source_before,
                source_replay_before,
                destination_before,
                destination_replay_before,
                expected_user,
                expected_vault,
                expected_hoard,
                expected_claim_ledger,
            )?;
            let bump = [descriptor.mint_authority_bump];
            let signer: [&[u8]; 3] = [MINT_AUTHORITY_SEED, &payload.wrapper_product_id, &bump];
            invoke_token_plan(&token_plan, accounts, &[&signer])?;
        }
        StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => {
            invoke_token_plan(&token_plan, accounts, &[])?;
            invoke_base_full_vector(accounts, action, payload)?;
            reconcile_full_vector_base(
                accounts,
                action,
                source_before,
                source_replay_before,
                destination_before,
                destination_replay_before,
                expected_user,
                expected_vault,
                expected_hoard,
                expected_claim_ledger,
            )?;
        }
        _ => return Err(WrapperError::Instruction),
    }
    let mint_observed = decode_mint(accounts, &bound)?;
    let holder_observed = decode_holder(accounts, &bound)?;
    if mint_observed.supply != supply_after
        || holder_observed.amount != holder_after
        || mint_observed.mint_authority != mint_before.mint_authority
        || holder_observed.mint != holder_before.mint
        || holder_observed.owner != holder_before.owner
    {
        return Err(WrapperError::Token2022);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn expected_full_vector_successors(
    action: StructuredClaimActionV1,
    user_before: PositionAccountV3,
    vault_before: PositionAccountV3,
    hoard_before: HoardV2,
    claim_ledger_before: ClaimLedgerV3,
    complete_set_atoms: u64,
    full: [u64; clutch_structured_claim::MAX_OUTCOMES],
    residual: [u64; clutch_structured_claim::MAX_OUTCOMES],
    resolution: Option<ResolutionV5>,
) -> Result<(PositionAccountV3, PositionAccountV3, HoardV2, ClaimLedgerV3)> {
    let mut user_fields = user_before.fields();
    let mut vault_fields = vault_before.fields();
    let mut aggregate_internal_supply = claim_ledger_before.aggregate_internal_supply;
    let mut index = 0usize;
    while index < usize::from(user_before.outcome_count()) {
        match action {
            StructuredClaimActionV1::WrapFull => {
                user_fields.native_eggs[index] = user_fields.native_eggs[index]
                    .checked_sub(full[index])
                    .ok_or(WrapperError::BaseCustody)?;
                vault_fields.native_eggs[index] = vault_fields.native_eggs[index]
                    .checked_add(residual[index])
                    .ok_or(WrapperError::Arithmetic)?;
                aggregate_internal_supply[index] = aggregate_internal_supply[index]
                    .checked_sub(complete_set_atoms)
                    .ok_or(WrapperError::BaseCustody)?;
            }
            StructuredClaimActionV1::UnwrapFull => {
                user_fields.native_eggs[index] = user_fields.native_eggs[index]
                    .checked_add(full[index])
                    .ok_or(WrapperError::Arithmetic)?;
                vault_fields.native_eggs[index] = vault_fields.native_eggs[index]
                    .checked_sub(residual[index])
                    .ok_or(WrapperError::BaseCustody)?;
                aggregate_internal_supply[index] = aggregate_internal_supply[index]
                    .checked_add(complete_set_atoms)
                    .ok_or(WrapperError::Arithmetic)?;
            }
            StructuredClaimActionV1::RedeemTerminal => {
                vault_fields.native_eggs[index] = vault_fields.native_eggs[index]
                    .checked_sub(residual[index])
                    .ok_or(WrapperError::BaseCustody)?;
                aggregate_internal_supply[index] = aggregate_internal_supply[index]
                    .checked_sub(residual[index])
                    .ok_or(WrapperError::BaseCustody)?;
            }
            _ => return Err(WrapperError::Instruction),
        }
        index += 1;
    }
    let (cash_liability_atoms, locked_claim_principal_atoms) = match action {
        StructuredClaimActionV1::WrapFull => {
            vault_fields.cash_atoms = vault_fields
                .cash_atoms
                .checked_add(complete_set_atoms)
                .ok_or(WrapperError::Arithmetic)?;
            (
                hoard_before
                    .cash_liability_atoms
                    .checked_add(complete_set_atoms)
                    .ok_or(WrapperError::Arithmetic)?,
                hoard_before
                    .locked_claim_principal_atoms
                    .checked_sub(complete_set_atoms)
                    .ok_or(WrapperError::BaseCustody)?,
            )
        }
        StructuredClaimActionV1::UnwrapFull => {
            vault_fields.cash_atoms = vault_fields
                .cash_atoms
                .checked_sub(complete_set_atoms)
                .ok_or(WrapperError::BaseCustody)?;
            (
                hoard_before
                    .cash_liability_atoms
                    .checked_sub(complete_set_atoms)
                    .ok_or(WrapperError::BaseCustody)?,
                hoard_before
                    .locked_claim_principal_atoms
                    .checked_add(complete_set_atoms)
                    .ok_or(WrapperError::Arithmetic)?,
            )
        }
        StructuredClaimActionV1::RedeemTerminal => {
            let resolution = resolution.ok_or(WrapperError::BaseCustody)?;
            let mut numerator = 0_u128;
            let mut outcome = 0usize;
            while outcome < usize::from(resolution.facts.outcome_count) {
                numerator = numerator
                    .checked_add(
                        u128::from(residual[outcome])
                            .checked_mul(u128::from(resolution.facts.payout_weights[outcome]))
                            .ok_or(WrapperError::Arithmetic)?,
                    )
                    .ok_or(WrapperError::Arithmetic)?;
                outcome += 1;
            }
            let denominator = u128::from(resolution.facts.payout_denominator);
            if numerator % denominator != 0 {
                return Err(WrapperError::BaseCustody);
            }
            let residual_payout = u64::try_from(numerator / denominator)
                .map_err(|_| WrapperError::Arithmetic)?;
            vault_fields.cash_atoms = vault_fields
                .cash_atoms
                .checked_sub(complete_set_atoms)
                .ok_or(WrapperError::BaseCustody)?;
            user_fields.cash_atoms = user_fields
                .cash_atoms
                .checked_add(
                    complete_set_atoms
                        .checked_add(residual_payout)
                        .ok_or(WrapperError::Arithmetic)?,
                )
                .ok_or(WrapperError::Arithmetic)?;
            (
                hoard_before
                    .cash_liability_atoms
                    .checked_add(residual_payout)
                    .ok_or(WrapperError::Arithmetic)?,
                hoard_before
                    .locked_claim_principal_atoms
                    .checked_sub(residual_payout)
                    .ok_or(WrapperError::BaseCustody)?,
            )
        }
        _ => return Err(WrapperError::Instruction),
    };
    let user_after = PositionAccountV3::new(user_fields).map_err(|_| WrapperError::BaseCustody)?;
    let vault_after =
        PositionAccountV3::new(vault_fields).map_err(|_| WrapperError::BaseCustody)?;
    let hoard_after = HoardV2 {
        cash_liability_atoms,
        locked_claim_principal_atoms,
        ..hoard_before
    };
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_internal_supply,
        ..claim_ledger_before
    };
    hoard_after
        .validate()
        .map_err(|_| WrapperError::BaseCustody)?;
    claim_ledger_after
        .validate()
        .map_err(|_| WrapperError::BaseCustody)?;
    Ok((user_after, vault_after, hoard_after, claim_ledger_after))
}

fn validate_create_accounts(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> Result<()> {
    if accounts.len() != CREATE_ACCOUNT_COUNT {
        return Err(WrapperError::Accounts);
    }
    let signer = [
        false, true, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false,
    ];
    let mut writable = [
        false, true, false, false, false, false, false, false, false, false, true, true, true,
        true, false, false, false, false, false, false, false, false, false, false, true, false,
        false, false, false, false, false, false, false,
    ];
    writable[CREATE_SERIES_LINK] = structured_root_requires_product_write(
        accounts[CREATE_STRUCTURED_ROOT].owner,
        accounts[CREATE_STRUCTURED_ROOT].data_len(),
    );
    let executable = [
        false, false, true, false, false, false, false, true, false, false, false, false, false,
        false, true, false, true, false, true, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false,
    ];
    validate_privileges(accounts, &signer, &writable, &executable)?;
    if *accounts[CREATE_WRAPPER_PROGRAM].key != *program_id
        || accounts[CREATE_DESCRIPTOR].key == accounts[CREATE_MINT].key
        || accounts[CREATE_POSITION].key == accounts[CREATE_REPLAY].key
        || accounts[PAYER].key == accounts[VAULT_AUTHORITY].key
    {
        return Err(WrapperError::Accounts);
    }
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let collateral_token_alias =
                left == CREATE_COLLATERAL_TOKEN && right == CREATE_TOKEN_PROGRAM;
            if accounts[left].key == accounts[right].key && !collateral_token_alias {
                return Err(WrapperError::Accounts);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn validate_canonical_accounts(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> Result<()> {
    if accounts.len() != CANONICAL_ACCOUNT_COUNT {
        return Err(WrapperError::Accounts);
    }
    let signer = [
        false, false, false, false, false, false, false, false, false, false, false, true, false,
        false, false, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false,
    ];
    let writable = [
        false, false, false, false, false, false, false, true, true, true, true, false, false,
        false, false, false, false, false, false, false, false, false, false, false, false, false,
        true, true, false,
    ];
    let executable = [
        false, false, false, false, true, false, false, false, false, false, false, false, false,
        true, false, true, false, true, false, false, false, false, false, false, false, false,
        false, false, false,
    ];
    validate_privileges(accounts, &signer, &writable, &executable)?;
    if *accounts[C_WRAPPER_PROGRAM].key != *program_id
        || accounts[CANONICAL_MINT].owner != accounts[C_TOKEN_PROGRAM].key
        || accounts[CANONICAL_HOLDER].owner != accounts[C_TOKEN_PROGRAM].key
        || accounts[CANONICAL_MINT].key == accounts[CANONICAL_HOLDER].key
        || accounts[VAULT_AUTHORITY].key == accounts[C_ACTOR].key
        || accounts[CANONICAL_MINT_AUTHORITY].key == accounts[VAULT_AUTHORITY].key
    {
        return Err(WrapperError::Accounts);
    }
    let mut left = 0_usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let token_program_alias = left == 4 && right == C_TOKEN_PROGRAM;
            if accounts[left].key == accounts[right].key && !token_program_alias {
                return Err(WrapperError::Accounts);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn validate_full_vector_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    action: StructuredClaimActionV1,
) -> Result<()> {
    let expected_count = if action == StructuredClaimActionV1::RedeemTerminal {
        TERMINAL_REDEMPTION_ACCOUNT_COUNT
    } else {
        FULL_VECTOR_ACCOUNT_COUNT
    };
    if accounts.len() != expected_count {
        return Err(WrapperError::Accounts);
    }
    let signer = [
        false, false, false, false, false, false, false, false, false, false, false, true, false,
        false, false, false, false, false, false, false, false, false, false, false, false, false,
        false, false,
    ];
    let writable = [
        false, false, false, false, false, false, false, true, true, true, true, false, false,
        false, false, false, false, false, false, false, false, true, true, true, true, false,
        false, false,
    ];
    let executable = [
        false, false, false, false, true, false, false, false, false, false, false, false, false,
        true, false, true, false, true, false, false, false, false, false, false, false, false,
        false, false,
    ];
    let mut privilege_index = 0usize;
    while privilege_index < FULL_VECTOR_CORE_ACCOUNT_COUNT {
        if accounts[privilege_index].is_signer != signer[privilege_index]
            || accounts[privilege_index].is_writable != writable[privilege_index]
            || accounts[privilege_index].executable != executable[privilege_index]
        {
            return Err(WrapperError::Accounts);
        }
        privilege_index += 1;
    }
    for release_index in [C_WRAPPER_RELEASE_V2, C_BASE_RELEASE_V2, C_TOKEN_RELEASE_V2] {
        if accounts[release_index].is_signer
            || accounts[release_index].is_writable
            || accounts[release_index].executable
            || accounts[release_index].owner != accounts[C_BASE_PROGRAM].key
        {
            return Err(WrapperError::Accounts);
        }
    }
    if action == StructuredClaimActionV1::RedeemTerminal
        && (accounts[C_RESOLUTION_V5].is_signer
            || accounts[C_RESOLUTION_V5].is_writable
            || accounts[C_RESOLUTION_V5].executable
            || accounts[C_RESOLUTION_V5].owner != accounts[C_BASE_PROGRAM].key)
    {
        return Err(WrapperError::Accounts);
    }
    if *accounts[C_WRAPPER_PROGRAM].key != *program_id
        || accounts[C_MINT].owner != accounts[C_TOKEN_PROGRAM].key
        || accounts[C_HOLDER].owner != accounts[C_TOKEN_PROGRAM].key
        || accounts[C_COLLATERAL_MINT].owner != accounts[4].key
        || accounts[C_HOARD_TOKEN].owner != accounts[4].key
        || accounts[C_MINT].key == accounts[C_HOLDER].key
        || accounts[VAULT_AUTHORITY].key == accounts[C_ACTOR].key
    {
        return Err(WrapperError::Accounts);
    }
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let token_program_alias = left == 4 && right == C_TOKEN_PROGRAM;
            if accounts[left].key == accounts[right].key && !token_program_alias {
                return Err(WrapperError::Accounts);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn validate_privileges<const N: usize>(
    accounts: &[AccountInfo<'_>],
    signer: &[bool; N],
    writable: &[bool; N],
    executable: &[bool; N],
) -> Result<()> {
    if accounts.len() != N {
        return Err(WrapperError::Accounts);
    }
    let mut index = 0_usize;
    while index < N {
        if accounts[index].is_signer != signer[index]
            || accounts[index].is_writable != writable[index]
            || accounts[index].executable != executable[index]
        {
            return Err(WrapperError::Accounts);
        }
        index += 1;
    }
    Ok(())
}

fn create_deployments(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedStructuredDeploymentsV2> {
    let wrapper = authenticate_release_v2(
        accounts[CREATE_BASE_PROGRAM].key,
        &accounts[CREATE_WRAPPER_PROGRAM],
        &accounts[CREATE_WRAPPER_DATA],
        &accounts[CREATE_WRAPPER_RELEASE_V2],
        ContentId::from_bytes(STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1),
    )?;
    let base = authenticate_release_v2(
        accounts[CREATE_BASE_PROGRAM].key,
        &accounts[CREATE_BASE_PROGRAM],
        &accounts[CREATE_BASE_DATA],
        &accounts[CREATE_REGISTRY_RELEASE_V2],
        ContentId::from_bytes(STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1),
    )?;
    let token = authenticate_release_v2(
        accounts[CREATE_BASE_PROGRAM].key,
        &accounts[CREATE_TOKEN_PROGRAM],
        &accounts[CREATE_TOKEN_DATA],
        &accounts[CREATE_TOKEN_RELEASE_V2],
        ContentId::from_bytes(STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1),
    )?;
    if *accounts[CREATE_WRAPPER_PROGRAM].key != *program_id {
        return Err(WrapperError::Deployment);
    }
    runtime_deployments(
        accounts[CREATE_WRAPPER_PROGRAM].key,
        wrapper,
        accounts[CREATE_BASE_PROGRAM].key,
        base,
        accounts[CREATE_TOKEN_PROGRAM].key,
        token,
    )
}

fn canonical_deployments(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<AuthenticatedStructuredDeploymentsV2> {
    let releases = if accounts.len() == CANONICAL_ACCOUNT_COUNT {
        [
            CANONICAL_WRAPPER_RELEASE_V2,
            CANONICAL_BASE_RELEASE_V2,
            CANONICAL_TOKEN_RELEASE_V2,
        ]
    } else if matches!(
        accounts.len(),
        FULL_VECTOR_ACCOUNT_COUNT | TERMINAL_REDEMPTION_ACCOUNT_COUNT
    ) {
        [C_WRAPPER_RELEASE_V2, C_BASE_RELEASE_V2, C_TOKEN_RELEASE_V2]
    } else {
        return Err(WrapperError::Accounts);
    };
    let wrapper = authenticate_release_v2(
        accounts[C_BASE_PROGRAM].key,
        &accounts[C_WRAPPER_PROGRAM],
        &accounts[C_WRAPPER_DATA],
        &accounts[releases[0]],
        ContentId::from_bytes(STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1),
    )?;
    let base = authenticate_release_v2(
        accounts[C_BASE_PROGRAM].key,
        &accounts[C_BASE_PROGRAM],
        &accounts[C_BASE_DATA],
        &accounts[releases[1]],
        ContentId::from_bytes(STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1),
    )?;
    let token = authenticate_release_v2(
        accounts[C_BASE_PROGRAM].key,
        &accounts[C_TOKEN_PROGRAM],
        &accounts[C_TOKEN_DATA],
        &accounts[releases[2]],
        ContentId::from_bytes(STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1),
    )?;
    if *accounts[C_WRAPPER_PROGRAM].key != *program_id {
        return Err(WrapperError::Deployment);
    }
    runtime_deployments(
        accounts[C_WRAPPER_PROGRAM].key,
        wrapper,
        accounts[C_BASE_PROGRAM].key,
        base,
        accounts[C_TOKEN_PROGRAM].key,
        token,
    )
}

fn runtime_deployments(
    wrapper_program: &Pubkey,
    wrapper: AuthenticatedReleaseV2,
    base_program: &Pubkey,
    base: AuthenticatedReleaseV2,
    token_program: &Pubkey,
    token: AuthenticatedReleaseV2,
) -> Result<AuthenticatedStructuredDeploymentsV2> {
    let binding = DeploymentBinding {
            wrapper_program: wrapper_program.to_bytes(),
            wrapper_program_data: wrapper.program_data,
            wrapper_deployment_slot: wrapper.slot,
            base_program: base_program.to_bytes(),
            base_program_data: base.program_data,
            base_deployment_slot: base.slot,
            token_2022_program: token_program.to_bytes(),
            token_2022_program_data: token.program_data,
            token_2022_deployment_slot: token.slot,
        };
    let value = RuntimeDeploymentsV1 {
        binding,
        upgradeable_loader: UPGRADEABLE_LOADER_ID,
        program_owners: [UPGRADEABLE_LOADER_ID; 3],
        program_data_owners: [UPGRADEABLE_LOADER_ID; 3],
        linked_program_data: [wrapper.program_data, base.program_data, token.program_data],
        executable_mask: 0b111,
    };
    value.validate().map_err(|_| WrapperError::Deployment)?;
    let owner_release_id = structured_owner_release_id_v2(
        binding,
        wrapper.release_id,
        base.release_id,
        token.release_id,
        &RuntimeSha,
    )
    .map_err(|_| WrapperError::Deployment)?;
    Ok(AuthenticatedStructuredDeploymentsV2 {
        runtime: value,
        wrapper_release_id: wrapper.release_id,
        base_release_id: base.release_id,
        token_release_id: token.release_id,
        owner_release_id,
    })
}

fn wrapper_token_indices(accounts: &[AccountInfo<'_>]) -> Result<(usize, usize, usize)> {
    if accounts.len() == CANONICAL_ACCOUNT_COUNT {
        Ok((CANONICAL_MINT, CANONICAL_HOLDER, CANONICAL_MINT_AUTHORITY))
    } else if matches!(
        accounts.len(),
        FULL_VECTOR_ACCOUNT_COUNT | TERMINAL_REDEMPTION_ACCOUNT_COUNT
    ) {
        Ok((C_MINT, C_HOLDER, C_MINT_AUTHORITY))
    } else {
        Err(WrapperError::Accounts)
    }
}

fn load_bound_descriptor(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_product: [u8; 32],
) -> Result<(
    clutch_structured_claim_adapter::BoundDescriptorV1,
    StructuredClaimDescriptorV2,
)> {
    let deployments = canonical_deployments(program_id, accounts)?;
    if accounts[C_DESCRIPTOR].owner != program_id
        || accounts[C_DESCRIPTOR].executable
        || accounts[C_DESCRIPTOR].is_writable
    {
        return Err(WrapperError::Accounts);
    }
    let descriptor_data = accounts[C_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let descriptor = StructuredClaimDescriptorV2::decode(&descriptor_data)
        .map_err(|_| WrapperError::Identity)?;
    drop(descriptor_data);
    let basis_data = accounts[C_BASIS]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let mut basis = Box::new(NativeClaimBasisV1::ZEROED);
    NativeClaimBasisV1::decode_into(&basis_data, &mut basis)
        .map_err(|_| WrapperError::Identity)?;
    let basis_id = hashv(&[
        clutch_product_series::NATIVE_CLAIM_BASIS_DOMAIN,
        &basis_data,
    ])
    .to_bytes();
    drop(basis_data);
    let market_data = accounts[C_MARKET]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let market = MarketInstancePreimageV2::decode(&market_data).map_err(|_| WrapperError::Identity)?;
    drop(market_data);
    let market_id = market.id().map_err(|_| WrapperError::Identity)?.bytes();
    let basis_projection = DescriptorBasisV1 {
        market: market_id,
        terms_digest: basis_id,
        basis_degree: basis.basis_degree,
        denominator: basis.denominator,
        outcome_count: basis.outcome_count,
    };
    let identity = clutch_structured_claim_adapter::runtime_contract::reconstruct_descriptor_identity_v1(
        &descriptor,
        basis_projection,
        deployments.runtime.binding,
    )
    .map_err(|_| WrapperError::Identity)?;
    let native_claim = canonical_native_claim_id_v1(&identity).map_err(|_| WrapperError::Identity)?;
    let product = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim,
        descriptor.structured_root_id,
        descriptor.wrapper_recipe_id,
    )
    .map_err(|_| WrapperError::Identity)?;
    if product != expected_product {
        return Err(WrapperError::Identity);
    }
    let addresses = derive_addresses(program_id, product);
    if addresses.descriptor.0 != *accounts[C_DESCRIPTOR].key
        || addresses.mint.0 != *accounts[wrapper_token_indices(accounts)?.0].key
        || addresses.mint_authority.0 != *accounts[wrapper_token_indices(accounts)?.2].key
        || addresses.vault_owner.0 != *accounts[VAULT_AUTHORITY].key
    {
        return Err(WrapperError::Identity);
    }
    let bound = bind_descriptor_v1(
        descriptor,
        basis_projection,
        deployments.runtime,
        native_claim,
        product,
        StructuredClaimRuntimeAddressesV1 {
            descriptor: addresses.descriptor.0.to_bytes(),
            mint: addresses.mint.0.to_bytes(),
            mint_authority: addresses.mint_authority.0.to_bytes(),
            vault_owner: addresses.vault_owner.0.to_bytes(),
        },
        &RuntimePdaVerifier,
    )
    .map_err(|_| WrapperError::Identity)?;
    Ok((bound, descriptor))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedAddresses {
    descriptor: (Pubkey, u8),
    mint: (Pubkey, u8),
    mint_authority: (Pubkey, u8),
    vault_owner: (Pubkey, u8),
}

fn derive_addresses(program_id: &Pubkey, product: [u8; 32]) -> DerivedAddresses {
    DerivedAddresses {
        descriptor: Pubkey::find_program_address(&[DESCRIPTOR_SEED, &product], program_id),
        mint: Pubkey::find_program_address(&[MINT_SEED, &product], program_id),
        mint_authority: Pubkey::find_program_address(
            &[MINT_AUTHORITY_SEED, &product],
            program_id,
        ),
        vault_owner: Pubkey::find_program_address(&[VAULT_OWNER_SEED, &product], program_id),
    }
}

fn position_pda(
    base_program: &Pubkey,
    market: [u8; 32],
    owner: [u8; 32],
    product: [u8; 32],
) -> (Pubkey, u8) {
    let purpose = [u8::from(PositionPurposeV3::StructuredClaim)];
    Pubkey::find_program_address(
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            &market,
            &owner,
            &purpose,
            &product,
        ],
        base_program,
    )
}

fn replay_pda(
    base_program: &Pubkey,
    position: [u8; 32],
    product: [u8; 32],
) -> (Pubkey, u8) {
    let purpose = [u8::from(PositionPurposeV3::StructuredClaim)];
    Pubkey::find_program_address(
        &[
            clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
            &position,
            &purpose,
            &product,
        ],
        base_program,
    )
}

fn require_key(account: &AccountInfo<'_>, expected: Pubkey) -> Result<()> {
    if *account.key != expected {
        Err(WrapperError::Identity)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimePdaVerifier;

impl PdaVerifierV1 for RuntimePdaVerifier {
    fn verify(
        &self,
        program: &[u8; 32],
        address: &[u8; 32],
        prefix: &[u8],
        product_id: &[u8; 32],
        bump: u8,
    ) -> bool {
        let bump_seed = [bump];
        Pubkey::create_program_address(
            &[prefix, product_id, &bump_seed],
            &Pubkey::new_from_array(*program),
        )
        .map(|candidate| candidate.to_bytes() == *address)
        .unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha;

impl PositionV3Sha256Backend for RuntimeSha {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        hashv(parts).to_bytes()
    }
}

impl WrapperRecipeHashV1 for RuntimeSha {
    fn hashv(&self, slices: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(slices).to_bytes()
    }
}

fn domain_hash(domain: &[u8], body: &[u8]) -> [u8; 32] {
    hashv(&[domain, body]).to_bytes()
}

fn decode_position(account: &AccountInfo<'_>) -> Result<PositionAccountV3> {
    let data = account.try_borrow_data().map_err(|_| WrapperError::Borrow)?;
    PositionAccountV3::decode(&data).map_err(|_| WrapperError::Identity)
}

fn decode_hoard(accounts: &[AccountInfo<'_>]) -> Result<HoardV2> {
    let data = accounts[C_HOARD]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    HoardV2::decode(&data).map_err(|_| WrapperError::BaseCustody)
}

fn decode_claim_ledger(accounts: &[AccountInfo<'_>]) -> Result<ClaimLedgerV3> {
    let data = accounts[C_LEDGER]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    ClaimLedgerV3::decode(&data).map_err(|_| WrapperError::BaseCustody)
}

fn decode_resolution(
    accounts: &[AccountInfo<'_>],
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
) -> Result<ResolutionV5> {
    if accounts[C_RESOLUTION_V5].owner != accounts[C_BASE_PROGRAM].key {
        return Err(WrapperError::BaseCustody);
    }
    let data = accounts[C_RESOLUTION_V5]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let resolution = ResolutionV5::decode(&data).map_err(|_| WrapperError::BaseCustody)?;
    drop(data);
    let expected = Pubkey::find_program_address(
        &[b"dc:resolution:v5", &hoard.market_instance_id.bytes()],
        accounts[C_BASE_PROGRAM].key,
    );
    if resolution.validate().is_err()
        || resolution.state != ResolutionStateV5::Finalized
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.resolution_account.bytes() != accounts[C_RESOLUTION_V5].key.to_bytes()
        || *accounts[C_RESOLUTION_V5].key != expected.0
        || resolution.stored_bump != expected.1
        || resolution.facts.market_instance_id != hoard.market_instance_id
        || resolution.facts.native_claim_basis_id != claim_ledger.native_claim_basis_id
        || resolution.facts.outcome_count != hoard.outcome_count
    {
        return Err(WrapperError::BaseCustody);
    }
    Ok(resolution)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReplaySnapshot {
    header: clutch_retirement::ReplayV3EnvelopeHeader,
    semantic_id: [u8; 32],
}

fn decode_replay(account: &AccountInfo<'_>) -> Result<ReplaySnapshot> {
    let data = account.try_borrow_data().map_err(|_| WrapperError::Borrow)?;
    let envelope = ReplayV3Envelope::decode(&data, &RuntimeSha)
        .map_err(|_| WrapperError::Identity)?;
    Ok(ReplaySnapshot {
        header: envelope.header(),
        semantic_id: envelope
            .semantic_id(&RuntimeSha)
            .map_err(|_| WrapperError::Identity)?
            .bytes(),
    })
}

fn decode_mint(
    accounts: &[AccountInfo<'_>],
    bound: &clutch_structured_claim_adapter::BoundDescriptorV1,
) -> Result<clutch_structured_claim_adapter::runtime_contract::WrapperMintProjectionV1> {
    let (mint_index, _, _) = wrapper_token_indices(accounts)?;
    if accounts[mint_index].owner != accounts[C_TOKEN_PROGRAM].key {
        return Err(WrapperError::Token2022);
    }
    let data = accounts[mint_index]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    decode_canonical_wrapper_mint_v1(
        accounts[C_TOKEN_PROGRAM].key.to_bytes(),
        accounts[mint_index].key.to_bytes(),
        bound.addresses().mint_authority,
        &data,
    )
    .map_err(|_| WrapperError::Token2022)
}

fn decode_holder(
    accounts: &[AccountInfo<'_>],
    bound: &clutch_structured_claim_adapter::BoundDescriptorV1,
) -> Result<clutch_structured_claim_adapter::runtime_contract::WrapperTokenProjectionV1> {
    let (_, holder_index, _) = wrapper_token_indices(accounts)?;
    if accounts[holder_index].owner != accounts[C_TOKEN_PROGRAM].key {
        return Err(WrapperError::Token2022);
    }
    let data = accounts[holder_index]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    decode_canonical_wrapper_token_v1(
        accounts[C_TOKEN_PROGRAM].key.to_bytes(),
        bound.addresses().mint,
        accounts[holder_index].key.to_bytes(),
        accounts[C_ACTOR].key.to_bytes(),
        &data,
    )
    .map_err(|_| WrapperError::Token2022)
}

#[allow(clippy::too_many_arguments)]
fn custody_authority(
    accounts: &[AccountInfo<'_>],
    bound: &clutch_structured_claim_adapter::BoundDescriptorV1,
    action: StructuredClaimActionV1,
    transfer: PositionAssetTransferPayloadV1,
    source: PositionAccountV3,
    source_replay: ReplaySnapshot,
    destination: PositionAccountV3,
    destination_replay: ReplaySnapshot,
) -> Result<PositionAssetTransferPayloadV1> {
    let descriptor_data = accounts[C_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let hoard_data = accounts[C_HOARD]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let binding_data = accounts[5]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let runtime_data = accounts[6]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let ledger_data = accounts[C_LEDGER]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let source_id = source
        .semantic_id(&RuntimeSha)
        .map_err(|_| WrapperError::Identity)?
        .bytes();
    let destination_id = destination
        .semantic_id(&RuntimeSha)
        .map_err(|_| WrapperError::Identity)?
        .bytes();
    let vault = if source.purpose() == PositionPurposeV3::StructuredClaim {
        source
    } else if destination.purpose() == PositionPurposeV3::StructuredClaim {
        destination
    } else {
        return Err(WrapperError::Identity);
    };
    let projection = StructuredCustodyCallProjectionV1 {
        target_base_program: accounts[C_BASE_PROGRAM].key.to_bytes(),
        wrapper_local_action: action,
        descriptor_account: accounts[C_DESCRIPTOR].key.to_bytes(),
        descriptor_body_digest: domain_hash(
            STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
            &descriptor_data,
        ),
        native_claim_id: bound.native_claim_id(),
        wrapper_product_id: bound.wrapper_product_id(),
        deployment: bound.identity().deployment,
        hoard_account: accounts[C_HOARD].key.to_bytes(),
        hoard_body_digest: domain_hash(STRUCTURED_CUSTODY_HOARD_BODY_DOMAIN_V1, &hoard_data),
        market_binding_account: accounts[5].key.to_bytes(),
        market_binding_body_digest: domain_hash(
            STRUCTURED_CUSTODY_MARKET_BINDING_BODY_DOMAIN_V1,
            &binding_data,
        ),
        market_runtime_account: accounts[6].key.to_bytes(),
        market_runtime_body_digest: domain_hash(
            STRUCTURED_CUSTODY_MARKET_RUNTIME_BODY_DOMAIN_V1,
            &runtime_data,
        ),
        native_claim_basis_account: accounts[C_BASIS].key.to_bytes(),
        native_claim_basis_id: bound.descriptor().terms_digest,
        market_instance_account: accounts[C_MARKET].key.to_bytes(),
        market_instance_id: bound.descriptor().market,
        claim_ledger_account: accounts[C_LEDGER].key.to_bytes(),
        claim_ledger_body_digest: domain_hash(
            STRUCTURED_CUSTODY_CLAIM_LEDGER_BODY_DOMAIN_V1,
            &ledger_data,
        ),
        realm_id: vault.realm_id().bytes(),
        collateral_policy_id: vault.collateral_policy_id().bytes(),
        collateral_release_id: vault.collateral_release_id().bytes(),
        vault_authority: accounts[VAULT_AUTHORITY].key.to_bytes(),
        user_actor: accounts[C_ACTOR].key.to_bytes(),
        source_position_account: accounts[7].key.to_bytes(),
        source_position_body_digest: source_id,
        source_replay_account: accounts[8].key.to_bytes(),
        source_replay_body_digest: source_replay.semantic_id,
        destination_position_account: accounts[9].key.to_bytes(),
        destination_position_body_digest: destination_id,
        destination_replay_account: accounts[10].key.to_bytes(),
        destination_replay_body_digest: destination_replay.semantic_id,
        transfer,
    };
    let mut preimage = Box::new([0_u8; STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES]);
    projection
        .encode_preimage_into(&mut preimage)
        .map_err(|_| WrapperError::Identity)?;
    let authority = hashv(&[STRUCTURED_CUSTODY_CALL_V1_DOMAIN, &preimage]).to_bytes();
    drop(descriptor_data);
    drop(hoard_data);
    drop(binding_data);
    drop(runtime_data);
    drop(ledger_data);
    transfer
        .with_custody_authority(authority)
        .map_err(|_| WrapperError::Identity)
}

fn reconcile_base_delta(
    accounts: &[AccountInfo<'_>],
    source_before: PositionAccountV3,
    source_replay_before: ReplaySnapshot,
    destination_before: PositionAccountV3,
    destination_replay_before: ReplaySnapshot,
    transfer: PositionAssetTransferPayloadV1,
) -> Result<()> {
    let source_after = decode_position(&accounts[7])?;
    let destination_after = decode_position(&accounts[9])?;
    let source_replay_after = decode_replay(&accounts[8])?;
    let destination_replay_after = decode_replay(&accounts[10])?;
    if immutable_position_fields(source_before, source_after).is_err()
        || immutable_position_fields(destination_before, destination_after).is_err()
        || source_before.rent() != source_after.rent()
        || destination_before.rent() != destination_after.rent()
        || source_before
            .cash_atoms()
            .checked_sub(source_after.cash_atoms())
            != Some(transfer.cash_atoms)
        || destination_after
            .cash_atoms()
            .checked_sub(destination_before.cash_atoms())
            != Some(transfer.cash_atoms)
        || source_replay_after.header.next_sequence()
            != source_replay_before.header.next_sequence().checked_add(1)
                .ok_or(WrapperError::Arithmetic)?
        || destination_replay_after.header.next_sequence()
            != destination_replay_before.header.next_sequence().checked_add(1)
                .ok_or(WrapperError::Arithmetic)?
        || source_replay_after.header.position_generation()
            != source_replay_before.header.position_generation()
        || destination_replay_after.header.position_generation()
            != destination_replay_before.header.position_generation()
        || !immutable_replay_fields(source_replay_before, source_replay_after)
        || !immutable_replay_fields(destination_replay_before, destination_replay_after)
        || source_replay_after.header.rent() != source_replay_before.header.rent()
        || destination_replay_after.header.rent() != destination_replay_before.header.rent()
        || source_replay_after.semantic_id == source_replay_before.semantic_id
        || destination_replay_after.semantic_id == destination_replay_before.semantic_id
    {
        return Err(WrapperError::BaseCustody);
    }
    let source_eggs_before = source_before.native_eggs();
    let source_eggs_after = source_after.native_eggs();
    let destination_eggs_before = destination_before.native_eggs();
    let destination_eggs_after = destination_after.native_eggs();
    let mut outcome = 0_usize;
    while outcome < transfer.internal.len() {
        if source_eggs_before[outcome].checked_sub(source_eggs_after[outcome])
            != Some(transfer.internal[outcome])
            || destination_eggs_after[outcome].checked_sub(destination_eggs_before[outcome])
                != Some(transfer.internal[outcome])
        {
            return Err(WrapperError::BaseCustody);
        }
        outcome += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reconcile_full_vector_base(
    accounts: &[AccountInfo<'_>],
    action: StructuredClaimActionV1,
    source_before: PositionAccountV3,
    source_replay_before: ReplaySnapshot,
    destination_before: PositionAccountV3,
    destination_replay_before: ReplaySnapshot,
    expected_user: PositionAccountV3,
    expected_vault: PositionAccountV3,
    expected_hoard: HoardV2,
    expected_claim_ledger: ClaimLedgerV3,
) -> Result<()> {
    let source_after = decode_position(&accounts[7])?;
    let destination_after = decode_position(&accounts[9])?;
    let source_replay_after = decode_replay(&accounts[8])?;
    let destination_replay_after = decode_replay(&accounts[10])?;
    let hoard_after = decode_hoard(accounts)?;
    let claim_ledger_after = decode_claim_ledger(accounts)?;
    let (expected_source, expected_destination) = match action {
        StructuredClaimActionV1::WrapFull => (expected_user, expected_vault),
        StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => {
            (expected_vault, expected_user)
        }
        _ => return Err(WrapperError::Instruction),
    };
    if source_after != expected_source
        || destination_after != expected_destination
        || hoard_after != expected_hoard
        || claim_ledger_after != expected_claim_ledger
        || immutable_position_fields(source_before, source_after).is_err()
        || immutable_position_fields(destination_before, destination_after).is_err()
        || source_before.rent() != source_after.rent()
        || destination_before.rent() != destination_after.rent()
        || source_replay_after.header.next_sequence()
            != source_replay_before
                .header
                .next_sequence()
                .checked_add(1)
                .ok_or(WrapperError::Arithmetic)?
        || destination_replay_after.header.next_sequence()
            != destination_replay_before
                .header
                .next_sequence()
                .checked_add(1)
                .ok_or(WrapperError::Arithmetic)?
        || !immutable_replay_fields(source_replay_before, source_replay_after)
        || !immutable_replay_fields(destination_replay_before, destination_replay_after)
        || source_replay_after.header.rent() != source_replay_before.header.rent()
        || destination_replay_after.header.rent() != destination_replay_before.header.rent()
        || source_replay_after.semantic_id == source_replay_before.semantic_id
        || destination_replay_after.semantic_id == destination_replay_before.semantic_id
    {
        return Err(WrapperError::BaseCustody);
    }
    Ok(())
}

fn immutable_replay_fields(before: ReplaySnapshot, after: ReplaySnapshot) -> bool {
    before.header.position_account() == after.header.position_account()
        && before.header.replay_account() == after.header.replay_account()
        && before.header.purpose() == after.header.purpose()
        && before.header.purpose_binding_id() == after.header.purpose_binding_id()
        && before.header.stored_bump() == after.header.stored_bump()
        && before.header.extension_schema() == after.header.extension_schema()
        && before.header.extension_len() == after.header.extension_len()
}

fn immutable_position_fields(before: PositionAccountV3, after: PositionAccountV3) -> Result<()> {
    if before.purpose() != after.purpose()
        || before.lifecycle() != after.lifecycle()
        || before.market_instance_id() != after.market_instance_id()
        || before.realm_id() != after.realm_id()
        || before.collateral_policy_id() != after.collateral_policy_id()
        || before.collateral_release_id() != after.collateral_release_id()
        || before.owner() != after.owner()
        || before.controller() != after.controller()
        || before.replay_account() != after.replay_account()
        || before.purpose_binding_id() != after.purpose_binding_id()
        || before.outcome_count() != after.outcome_count()
        || before.generation() != after.generation()
        || before.stored_bump() != after.stored_bump()
        || before.reserved_cash_atoms() != after.reserved_cash_atoms()
        || before.outstanding_reservations() != after.outstanding_reservations()
    {
        Err(WrapperError::BaseCustody)
    } else {
        Ok(())
    }
}

fn invoke_base_create(
    accounts: &[AccountInfo<'_>],
    payload: &CreateDescriptorPayloadV1,
) -> Result<()> {
    let payload_body = payload.encode().map_err(|_| WrapperError::Instruction)?;
    let request = ExtensionRequest {
        sequence: 0,
        envelope: ExtensionEnvelope {
            family: ExtensionFamily::StructuredClaim,
            action: ExtensionAction::StructuredClaim(StructuredClaimAction::CreateDescriptor),
            payload: &payload_body,
        },
    };
    let mut data = vec![0_u8; 13 + 3 + payload_body.len()];
    let written = request
        .encode(&mut data)
        .map_err(|_| WrapperError::Instruction)?;
    if written != data.len() {
        return Err(WrapperError::Instruction);
    }
    let mut metas = Vec::with_capacity(CREATE_ACCOUNT_COUNT);
    let mut infos = Vec::with_capacity(CREATE_ACCOUNT_COUNT + 1);
    let mut index = 0_usize;
    while index < CREATE_ACCOUNT_COUNT {
        metas.push(AccountMeta {
            pubkey: *accounts[index].key,
            is_signer: index == VAULT_AUTHORITY || index == PAYER,
            is_writable: matches!(
                index,
                PAYER | CREATE_POSITION | CREATE_REPLAY | CREATE_STRUCTURED_ROOT
            ) || (index == CREATE_SERIES_LINK && accounts[index].is_writable),
        });
        infos.push(accounts[index].clone());
        index += 1;
    }
    infos.push(accounts[CREATE_BASE_PROGRAM].clone());
    let instruction = Instruction::new_with_bytes(*accounts[CREATE_BASE_PROGRAM].key, &data, metas);
    let product = payload.wrapper_product_id;
    let vault_bump = Pubkey::find_program_address(
        &[VAULT_OWNER_SEED, &product],
        accounts[CREATE_WRAPPER_PROGRAM].key,
    )
    .1;
    let bump = [vault_bump];
    let signer: [&[u8]; 3] = [VAULT_OWNER_SEED, &product, &bump];
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| WrapperError::BaseCustody)
}

fn invoke_base_transfer(
    accounts: &[AccountInfo<'_>],
    transfer: PositionAssetTransferPayloadV1,
    product: [u8; 32],
) -> Result<()> {
    let payload = transfer.encode().map_err(|_| WrapperError::Instruction)?;
    let request = ExtensionRequest {
        sequence: 0,
        envelope: ExtensionEnvelope {
            family: ExtensionFamily::GeneralV2,
            action: ExtensionAction::GeneralV2(GeneralV2Action::TransferPositionAssets),
            payload: &payload,
        },
    };
    let mut data = vec![0_u8; 13 + 3 + payload.len()];
    let written = request
        .encode(&mut data)
        .map_err(|_| WrapperError::Instruction)?;
    if written != data.len() {
        return Err(WrapperError::Instruction);
    }
    let mut metas = Vec::with_capacity(26);
    let mut infos = Vec::with_capacity(27);
    let mut index = 0_usize;
    while index < 26 {
        metas.push(AccountMeta {
            pubkey: *accounts[index].key,
            is_signer: index == VAULT_AUTHORITY || index == C_ACTOR,
            is_writable: (7..=10).contains(&index),
        });
        infos.push(accounts[index].clone());
        index += 1;
    }
    infos.push(accounts[C_BASE_PROGRAM].clone());
    let instruction = Instruction::new_with_bytes(*accounts[C_BASE_PROGRAM].key, &data, metas);
    let bump = [Pubkey::find_program_address(
        &[VAULT_OWNER_SEED, &product],
        accounts[C_WRAPPER_PROGRAM].key,
    )
    .1];
    let signer: [&[u8]; 3] = [VAULT_OWNER_SEED, &product, &bump];
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| WrapperError::BaseCustody)
}

fn invoke_base_full_vector(
    accounts: &[AccountInfo<'_>],
    action: StructuredClaimActionV1,
    payload: WrapperQuantityPayloadV1,
) -> Result<()> {
    let payload_body = payload.encode().map_err(|_| WrapperError::Instruction)?;
    let base_action = match action {
        StructuredClaimActionV1::WrapFull => StructuredClaimAction::WrapFull,
        StructuredClaimActionV1::UnwrapFull => StructuredClaimAction::UnwrapFull,
        StructuredClaimActionV1::RedeemTerminal => StructuredClaimAction::RedeemTerminal,
        _ => return Err(WrapperError::Instruction),
    };
    let request = ExtensionRequest {
        sequence: 0,
        envelope: ExtensionEnvelope {
            family: ExtensionFamily::StructuredClaim,
            action: ExtensionAction::StructuredClaim(base_action),
            payload: &payload_body,
        },
    };
    let mut data = vec![0_u8; 13 + 3 + payload_body.len()];
    let written = request
        .encode(&mut data)
        .map_err(|_| WrapperError::Instruction)?;
    if written != data.len() {
        return Err(WrapperError::Instruction);
    }
    let account_count = if action == StructuredClaimActionV1::RedeemTerminal {
        TERMINAL_REDEMPTION_ACCOUNT_COUNT
    } else {
        FULL_VECTOR_ACCOUNT_COUNT
    };
    let mut metas = Vec::with_capacity(account_count);
    let mut infos = Vec::with_capacity(account_count + 1);
    let mut index = 0usize;
    while index < account_count {
        metas.push(AccountMeta {
            pubkey: *accounts[index].key,
            is_signer: index == VAULT_AUTHORITY || index == C_ACTOR,
            is_writable: matches!(index, 7..=10 | C_HOARD | C_LEDGER | C_MINT | C_HOLDER),
        });
        infos.push(accounts[index].clone());
        index += 1;
    }
    infos.push(accounts[C_BASE_PROGRAM].clone());
    let instruction = Instruction::new_with_bytes(*accounts[C_BASE_PROGRAM].key, &data, metas);
    let bump = [Pubkey::find_program_address(
        &[VAULT_OWNER_SEED, &payload.wrapper_product_id],
        accounts[C_WRAPPER_PROGRAM].key,
    )
    .1];
    let signer: [&[u8]; 3] = [VAULT_OWNER_SEED, &payload.wrapper_product_id, &bump];
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| WrapperError::BaseCustody)
}

fn invoke_token_plan<'a>(
    plan: &Token2022InstructionPlanV1,
    accounts: &[AccountInfo<'a>],
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let program = accounts
        .iter()
        .find(|account| account.key.to_bytes() == plan.program_id && account.executable)
        .ok_or(WrapperError::Token2022)?;
    let active = usize::from(plan.account_count);
    let mut metas = Vec::with_capacity(active);
    let mut infos = Vec::with_capacity(active + 1);
    let mut index = 0_usize;
    while index < active {
        let planned = plan.accounts[index];
        let account = accounts
            .iter()
            .find(|candidate| candidate.key.to_bytes() == planned.address)
            .ok_or(WrapperError::Token2022)?;
        metas.push(AccountMeta {
            pubkey: *account.key,
            is_signer: planned.signer,
            is_writable: planned.writable,
        });
        infos.push(account.clone());
        index += 1;
    }
    infos.push(program.clone());
    let instruction = Instruction::new_with_bytes(
        Pubkey::new_from_array(plan.program_id),
        &plan.data[..usize::from(plan.data_len)],
        metas,
    );
    if signer_seeds.is_empty() {
        invoke(&instruction, &infos).map_err(|_| WrapperError::Token2022)
    } else {
        invoke_signed(&instruction, &infos, signer_seeds).map_err(|_| WrapperError::Token2022)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StructuredRootPrestateV1 {
    Empty {
        hostile_prefund_lamports: u64,
    },
    Live {
        root: Box<StructuredMarketRootV1>,
        observed_donation_lamports: u64,
    },
}

fn structured_root_prestate(
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV2,
) -> Result<StructuredRootPrestateV1> {
    let expected = Pubkey::find_program_address(
        &[STRUCTURED_ROOT_SEED_V1, &descriptor.structured_root_id],
        accounts[CREATE_BASE_PROGRAM].key,
    );
    if *accounts[CREATE_STRUCTURED_ROOT].key != expected.0 {
        return Err(WrapperError::Identity);
    }
    if accounts[CREATE_STRUCTURED_ROOT].owner == &system_program::ID
        && accounts[CREATE_STRUCTURED_ROOT].data_len() == 0
    {
        return Ok(StructuredRootPrestateV1::Empty {
            hostile_prefund_lamports: accounts[CREATE_STRUCTURED_ROOT].lamports(),
        });
    }
    if accounts[CREATE_STRUCTURED_ROOT].owner != accounts[CREATE_BASE_PROGRAM].key
        || accounts[CREATE_STRUCTURED_ROOT].data_len() != STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES
    {
        return Err(WrapperError::BaseCustody);
    }
    let data = accounts[CREATE_STRUCTURED_ROOT]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let root = Box::new(
        StructuredMarketRootV1::decode(&data).map_err(|_| WrapperError::Identity)?,
    );
    drop(data);
    let observed_donation_lamports = accounts[CREATE_STRUCTURED_ROOT]
        .lamports()
        .checked_sub(root.rent_principal_lamports)
        .ok_or(WrapperError::BaseCustody)?;
    if root.root_bump != expected.1
        || root
            .binding
            .id(&RuntimeSha)
            .map_err(|_| WrapperError::Identity)?
            .bytes()
            != descriptor.structured_root_id
        || observed_donation_lamports < root.current_donation_lamports
        || observed_donation_lamports < root.donation_floor_lamports
    {
        return Err(WrapperError::BaseCustody);
    }
    Ok(StructuredRootPrestateV1::Live {
        root,
        observed_donation_lamports,
    })
}

fn reconcile_create(
    accounts: &[AccountInfo<'_>],
    bound: &clutch_structured_claim_adapter::BoundDescriptorV1,
    product: [u8; 32],
    market: MarketInstancePreimageV2,
    root_before: StructuredRootPrestateV1,
    deployments: AuthenticatedStructuredDeploymentsV2,
) -> Result<()> {
    let descriptor_data = accounts[CREATE_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let observed_descriptor = StructuredClaimDescriptorV2::decode(&descriptor_data)
        .map_err(|_| WrapperError::Identity)?;
    if observed_descriptor != *bound.descriptor() {
        return Err(WrapperError::Identity);
    }
    drop(descriptor_data);
    reconcile_structured_root(
        accounts,
        observed_descriptor,
        market,
        root_before,
        deployments,
    )?;
    let position = decode_position(&accounts[CREATE_POSITION])?;
    let replay = decode_replay(&accounts[CREATE_REPLAY])?;
    let replay_data = accounts[CREATE_REPLAY]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let replay_envelope = ReplayV3Envelope::decode(&replay_data, &RuntimeSha)
        .map_err(|_| WrapperError::Identity)?;
    let structured_extension = StructuredClaimReplayExtensionV1::decode(
        replay_envelope.extension(),
    )
    .map_err(|_| WrapperError::Identity)?;
    if position.purpose() != PositionPurposeV3::StructuredClaim
        || position.lifecycle() != clutch_retirement::PositionLifecycleV3::Open
        || position.owner().bytes() != accounts[VAULT_AUTHORITY].key.to_bytes()
        || position.controller().bytes() != accounts[VAULT_AUTHORITY].key.to_bytes()
        || position.purpose_binding_id().bytes() != product
        || position.generation() != 1
        || position.cash_atoms() != 0
        || position.reserved_cash_atoms() != 0
        || position.native_eggs() != [0; clutch_retirement::MAX_OUTCOMES]
        || position.outstanding_reservations() != 0
        || replay.header.position_account().bytes() != accounts[CREATE_POSITION].key.to_bytes()
        || replay.header.replay_account().bytes() != accounts[CREATE_REPLAY].key.to_bytes()
        || replay.header.purpose() != PositionPurposeV3::StructuredClaim
        || replay.header.purpose_binding_id().bytes() != product
        || replay.header.position_generation() != 1
        || replay.header.next_sequence() != 0
        || structured_extension.descriptor_account
            != accounts[CREATE_DESCRIPTOR].key.to_bytes()
        || structured_extension.wrapper_product_id != product
        || structured_extension.vault_authority != accounts[VAULT_AUTHORITY].key.to_bytes()
        || structured_extension.current_position_semantic_id
            != position
                .semantic_id(&RuntimeSha)
                .map_err(|_| WrapperError::Identity)?
                .bytes()
        || structured_extension.state != StructuredClaimReplayExtensionStateV1::Founding
        || structured_extension.last_action != 0
    {
        return Err(WrapperError::BaseCustody);
    }
    let mint_data = accounts[CREATE_MINT]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let mint = decode_canonical_wrapper_mint_v1(
        accounts[CREATE_TOKEN_PROGRAM].key.to_bytes(),
        accounts[CREATE_MINT].key.to_bytes(),
        bound.addresses().mint_authority,
        &mint_data,
    )
    .map_err(|_| WrapperError::Token2022)?;
    if mint.supply != 0 {
        return Err(WrapperError::Token2022);
    }
    Ok(())
}

fn reconcile_structured_root(
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV2,
    market: MarketInstancePreimageV2,
    before: StructuredRootPrestateV1,
    deployments: AuthenticatedStructuredDeploymentsV2,
) -> Result<()> {
    if accounts[CREATE_STRUCTURED_ROOT].owner != accounts[CREATE_BASE_PROGRAM].key
        || accounts[CREATE_STRUCTURED_ROOT].data_len() != STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES
    {
        return Err(WrapperError::BaseCustody);
    }
    let root_data = accounts[CREATE_STRUCTURED_ROOT]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let root = Box::new(
        StructuredMarketRootV1::decode(&root_data).map_err(|_| WrapperError::Identity)?,
    );
    drop(root_data);
    let expected_pda = Pubkey::find_program_address(
        &[STRUCTURED_ROOT_SEED_V1, &descriptor.structured_root_id],
        accounts[CREATE_BASE_PROGRAM].key,
    );
    let descriptor_body = descriptor.encode().map_err(|_| WrapperError::Identity)?;
    let descriptor_id = ContentId::from_bytes(
        hashv(&[
            STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
            &descriptor_body,
        ])
        .to_bytes(),
    );
    let recipe_id = ContentId::from_bytes(descriptor.wrapper_recipe_id);
    let (bundle, attachment, registry, profile) =
        decode_structured_product_artifacts(accounts, deployments)?;
    if *accounts[CREATE_STRUCTURED_ROOT].key != expected_pda.0
        || root.root_bump != expected_pda.1
        || root
            .binding
            .id(&RuntimeSha)
            .map_err(|_| WrapperError::Identity)?
            .bytes()
            != descriptor.structured_root_id
        || root.binding.link_account != accounts[CREATE_SERIES_LINK].key.to_bytes()
        || root.binding.market_instance_id.bytes() != descriptor.market
        || root.binding.rent_refund_owner.bytes() != accounts[PAYER].key.to_bytes()
        || root.binding.owner_release_id
            != deployments.owner_release_id
        || root.binding.registry_release_id != deployments.base_release_id
        || root.binding.compiler_output_id
            != bundle.id().map_err(|_| WrapperError::Identity)?
        || root.binding.attachment_plan_id
            != attachment.id().map_err(|_| WrapperError::Identity)?
        || root.binding.series_plan_id != bundle.series_plan_id
        || root.binding.attachment_plan_id != bundle.attachment_plan_id
        || root.binding.capability_profile_id != bundle.capability_profile_id.content_id()
        || root.binding.registry_release_id != bundle.registry_release_id
        || root.binding.registry_release_id != registry.registry_release_id
        || root.binding.capability_profile_id != registry.capability_profile_id
        || root.binding.capability_profile_id
            != profile.id().map_err(|_| WrapperError::Identity)?.content_id()
        || profile.registry_release_id().content_id() != root.binding.registry_release_id
        || registry.compiler_bundle_id != bundle.id().map_err(|_| WrapperError::Identity)?
        || registry.series_plan_id != bundle.series_plan_id
        || root.binding.compiler_release_id != bundle.product_compiler_release_id
        || root.binding.wrapper_recipe_set_id != attachment.wrapper_recipe_set_id
        || attachment.funding_quote_id != bundle.funding_quote_id
        || bundle.native_claim_basis_id.bytes() != descriptor.terms_digest
        || bundle.product_template_id != market.product_template_id
        || bundle.market_genesis_profile_id != market.market_genesis_profile_id
        || root.live_descriptor_count == 0
        || accounts[CREATE_STRUCTURED_ROOT].lamports()
            != root
                .rent_principal_lamports
                .checked_add(root.current_donation_lamports)
                .ok_or(WrapperError::Arithmetic)?
    {
        return Err(WrapperError::BaseCustody);
    }
    let link_should_be_writable = matches!(&before, StructuredRootPrestateV1::Empty { .. });
    reconcile_product_series_link(accounts, &root, link_should_be_writable)?;
    match before {
        StructuredRootPrestateV1::Empty {
            hostile_prefund_lamports,
        } => {
            let expected_receipt = structured_descriptor_admission_receipt_v1(
                ContentId::ZERO,
                descriptor_id,
                recipe_id,
                1,
                &RuntimeSha,
            )
            .map_err(|_| WrapperError::Identity)?;
            let minimum = rent(&accounts[RENT])?
                .minimum_balance(STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES);
            if minimum == 0
                || root.transition_sequence != 1
                || root.admitted_descriptor_count != 1
                || root.live_descriptor_count != 1
                || root.terminal_descriptor_count != 0
                || root.admission_transcript_id != expected_receipt
                || !root.terminal_transcript_id.is_zero()
                || !root.aggregate_terminal_receipt_id.is_zero()
                || root.rent_principal_lamports != minimum
                || root.donation_floor_lamports != hostile_prefund_lamports
                || root.current_donation_lamports != hostile_prefund_lamports
                || root.product_lineage.product_admission_receipt_id != expected_receipt
            {
                return Err(WrapperError::BaseCustody);
            }
        }
        StructuredRootPrestateV1::Live {
            root: previous,
            observed_donation_lamports,
        } => {
            let next_sequence = previous
                .transition_sequence
                .checked_add(1)
                .ok_or(WrapperError::Arithmetic)?;
            let expected_receipt = structured_descriptor_admission_receipt_v1(
                previous.admission_transcript_id,
                descriptor_id,
                recipe_id,
                next_sequence,
                &RuntimeSha,
            )
            .map_err(|_| WrapperError::Identity)?;
            if root.binding != previous.binding
                || root.transition_sequence != next_sequence
                || root.admitted_descriptor_count
                    != previous
                        .admitted_descriptor_count
                        .checked_add(1)
                        .ok_or(WrapperError::Arithmetic)?
                || root.live_descriptor_count
                    != previous
                        .live_descriptor_count
                        .checked_add(1)
                        .ok_or(WrapperError::Arithmetic)?
                || root.terminal_descriptor_count != previous.terminal_descriptor_count
                || root.admission_transcript_id != expected_receipt
                || root.terminal_transcript_id != previous.terminal_transcript_id
                || root.aggregate_terminal_receipt_id
                    != previous.aggregate_terminal_receipt_id
                || root.rent_principal_lamports != previous.rent_principal_lamports
                || root.donation_floor_lamports != previous.donation_floor_lamports
                || root.current_donation_lamports != observed_donation_lamports
                || root.product_lineage.product_admission_receipt_id
                    != previous.product_lineage.product_admission_receipt_id
            {
                return Err(WrapperError::BaseCustody);
            }
        }
    }
    Ok(())
}

fn reconcile_product_series_link(
    accounts: &[AccountInfo<'_>],
    root: &StructuredMarketRootV1,
    expected_writable: bool,
) -> Result<()> {
    let account = &accounts[CREATE_SERIES_LINK];
    if account.owner != accounts[CREATE_BASE_PROGRAM].key
        || account.data_len() != SERIES_MARKET_LINK_ACCOUNT_BYTES_V1
        || account.is_signer
        || account.is_writable != expected_writable
        || account.executable
        || account.key.to_bytes() != root.binding.link_account
    {
        return Err(WrapperError::BaseCustody);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let framed_data_id = hashv(&[&data[..]]).to_bytes();
    let mut link = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    SeriesMarketLinkAccountV1::decode_into(&data, &mut link)
        .map_err(|_| WrapperError::Identity)?;
    drop(data);
    let binding = link.state.binding();
    let semantic_id = link
        .state
        .semantic_id()
        .map_err(|_| WrapperError::Identity)?;
    let ordinal = binding.ordinal.to_le_bytes();
    let expected_pda = Pubkey::find_program_address(
        &[
            b"dc:series-market-link:v1",
            &binding.series_plan_id.bytes(),
            &ordinal,
        ],
        accounts[CREATE_BASE_PROGRAM].key,
    );
    let observed_lamports = account.lamports();
    let accounted_lamports = link
        .state
        .rent_principal_lamports()
        .checked_add(link.state.current_donation_lamports())
        .ok_or(WrapperError::Arithmetic)?;
    let authentication_id = ContentId::from_bytes(
        series_market_link_authentication_id_v1(
            account.key.to_bytes(),
            accounts[CREATE_BASE_PROGRAM].key.to_bytes(),
            framed_data_id,
            semantic_id.bytes(),
            binding.market_root_account_id.bytes(),
            observed_lamports,
        )
        .0,
    );
    if *account.key != expected_pda.0
        || link.stored_bump != expected_pda.1
        || observed_lamports < accounted_lamports
        || binding.series_plan_id != root.binding.series_plan_id
        || binding.ordinal != root.binding.ordinal
        || binding.market_instance_id != root.binding.market_instance_id
        || binding.generation != root.binding.generation
        || binding.attachment_plan_id != root.binding.attachment_plan_id.content_id()
        || binding.compiler_output_id != root.binding.compiler_output_id.content_id()
        || binding.capability_profile_id != root.binding.capability_profile_id
        || binding.rent_refund_owner != root.binding.rent_refund_owner
        || binding.neutral_lamport_sink != root.binding.neutral_lamport_sink
        || link.state.phase() != SeriesMarketLinkPhaseV1::Active
        || link
            .state
            .obligation_status(SeriesLinkObligationV1::Wrapper)
            != SeriesLinkObligationStatusV1::Live
        || link
            .state
            .obligation_admission_receipt_id(SeriesLinkObligationV1::Wrapper)
            != root.product_lineage.product_admission_receipt_id
        || link.state.transition_sequence()
            != root.product_lineage.product_link_transition_sequence
        || ContentId::from_bytes(semantic_id.bytes()) != root.product_lineage.link_semantic_id
        || authentication_id != root.product_lineage.link_authentication_id
    {
        return Err(WrapperError::BaseCustody);
    }
    Ok(())
}

fn decode_structured_product_artifacts(
    accounts: &[AccountInfo<'_>],
    deployments: AuthenticatedStructuredDeploymentsV2,
) -> Result<(
    Box<CompiledProductSeriesBundleV5>,
    Box<SeriesAttachmentPlanV4>,
    SeriesRegistryAccountV2,
    Box<RegistryCapabilityProfileV4>,
)> {
    if accounts[CREATE_COMPILER_BUNDLE].owner != accounts[CREATE_BASE_PROGRAM].key
        || accounts[CREATE_ATTACHMENT].owner != accounts[CREATE_BASE_PROGRAM].key
        || accounts[CREATE_SERIES_REGISTRY_V2].owner != accounts[CREATE_BASE_PROGRAM].key
        || accounts[CREATE_CAPABILITY_PROFILE_V4].owner != accounts[CREATE_BASE_PROGRAM].key
        || accounts[CREATE_SERIES_REGISTRY_V2].data_len() != SERIES_REGISTRY_ACCOUNT_BYTES_V2
    {
        return Err(WrapperError::Identity);
    }
    let bundle_data = accounts[CREATE_COMPILER_BUNDLE]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let bundle = Box::new(
        CompiledProductSeriesBundleV5::decode(&bundle_data)
            .map_err(|_| WrapperError::Identity)?,
    );
    drop(bundle_data);
    let attachment_data = accounts[CREATE_ATTACHMENT]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let attachment = Box::new(
        SeriesAttachmentPlanV4::decode(&attachment_data)
            .map_err(|_| WrapperError::Identity)?,
    );
    drop(attachment_data);
    let registry_data = accounts[CREATE_SERIES_REGISTRY_V2]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let registry = SeriesRegistryAccountV2::decode(&registry_data)
        .map_err(|_| WrapperError::Identity)?;
    drop(registry_data);
    let profile_data = accounts[CREATE_CAPABILITY_PROFILE_V4]
        .try_borrow_data()
        .map_err(|_| WrapperError::Borrow)?;
    let profile = Box::new(
        RegistryCapabilityProfileV4::decode(&profile_data)
            .map_err(|_| WrapperError::Identity)?,
    );
    drop(profile_data);
    let bundle_id = bundle.id().map_err(|_| WrapperError::Identity)?.bytes();
    let attachment_id = attachment
        .id()
        .map_err(|_| WrapperError::Identity)?
        .bytes();
    if *accounts[CREATE_COMPILER_BUNDLE].key
        != product_artifact_pda(
            accounts[CREATE_BASE_PROGRAM].key,
            ArtifactKind::CompiledProductSeriesBundleV5.byte(),
            bundle_id,
        )
        || *accounts[CREATE_ATTACHMENT].key
            != product_artifact_pda(
                accounts[CREATE_BASE_PROGRAM].key,
                ArtifactKind::SeriesAttachmentPlanV4.byte(),
                attachment_id,
            )
        || *accounts[CREATE_CAPABILITY_PROFILE_V4].key
            != product_artifact_pda(
                accounts[CREATE_BASE_PROGRAM].key,
                ArtifactKind::RegistryCapabilityProfileV4.byte(),
                profile
                    .id()
                    .map_err(|_| WrapperError::Identity)?
                    .bytes(),
            )
        || {
            let expected_registry = Pubkey::find_program_address(
                &[b"dc:series-registry:v1", &registry.series_plan_id.bytes()],
                accounts[CREATE_BASE_PROGRAM].key,
            );
            *accounts[CREATE_SERIES_REGISTRY_V2].key != expected_registry.0
                || registry.stored_bump != expected_registry.1
        }
        || accounts[CREATE_SERIES_REGISTRY_V2].lamports() < registry.rent_principal_lamports
        || registry.registry_release_id != deployments.base_release_id
        || registry.capability_profile_id
            != profile.id().map_err(|_| WrapperError::Identity)?.content_id()
        || profile.registry_release_id().content_id() != deployments.base_release_id
        || registry.compiler_bundle_id
            != bundle.id().map_err(|_| WrapperError::Identity)?
    {
        return Err(WrapperError::Identity);
    }
    Ok((bundle, attachment, registry, profile))
}

fn product_artifact_pda(program_id: &Pubkey, kind: u8, id: [u8; 32]) -> Pubkey {
    let kind_seed = [kind];
    Pubkey::find_program_address(
        &[b"dc:product-artifact:v1", &kind_seed, &id],
        program_id,
    )
    .0
}

fn structured_root_requires_product_write(owner: &Pubkey, data_len: usize) -> bool {
    owner == &system_program::ID && data_len == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_root_refuses_product_link_write_privilege() {
        assert!(structured_root_requires_product_write(&system_program::ID, 0));
        assert!(!structured_root_requires_product_write(&system_program::ID, 1));
        assert!(!structured_root_requires_product_write(
            &Pubkey::new_from_array([92; 32]),
            0,
        ));
    }
}
