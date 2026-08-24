//! Executable base endpoints for current Structured custody.
//!
//! This module is intentionally only the base-owned half of the operation. A
//! separately deployed, descriptor-pinned wrapper program must call it with
//! its vault PDA as a signer and must atomically perform the matching
//! Token-2022 mint or burn. This program never claims that wrapper supply moved.
//!
//! Canonical routes write the two Position V3 bodies and their purpose-owned
//! Replay envelopes. Full-vector routes additionally write Hoard V2 and
//! ClaimLedger V3 complete-set reclassification successors. Position rent
//! owner, refundable principal, and donation floor are copied byte-exactly;
//! no lamports move and no prefunding becomes an economic asset.

use clutch_product_series::{
    CompiledProductSeriesBundleV5, CompiledProductSeriesBundleV5Id, ContentId, FixedCodec,
    MarketInstanceV2Id, NativeClaimBasisV1, RegistryProgramReleaseV2, RegistryReleaseLocusV2,
    SeriesAttachmentPlanV4Id, SeriesFundingTermsV2, SeriesLinkObligationStatusV1,
    SeriesLinkObligationV1, SeriesMarketLinkPhaseV1, SeriesPlanV5Id,
};
use clutch_collateral_adapter_v2::{
    accept_hoard_surplus_disposition_v1, admit_collateral_account_v2,
    admit_collateral_mint_v2, prepare_hoard_surplus_disposition_v1, BoundCollateralProfileV2,
    CpiAccountMetaV2, HoardSurplusDispositionRequestV1, Id as CollateralId,
    PreparedHoardSurplusDispositionV1, RuntimeAccountViewV2, TokenAccountRoleV2,
    TransferAuthorityKindV2, TransferAuthorityV2,
};
use clutch_retirement::{
    plan_position_v3_replay_v3_retirement_v1, PositionAccountV3, PositionPurposeV3,
    PositionV3ReplayV3AccountsV1, PositionV3ReplayV3RetirementPlanV1,
    PositionV3ReplayV3RetirementRequestV1, RecipientBalanceBookV1, RecipientBalanceV1,
    ReplayV3Envelope,
};
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
    structured_owner_release_id_v2, DescriptorBasisV1, PositionAssetTransferPayloadV1,
    StructuredClaimActionV1,
    StructuredClaimDescriptorV2, StructuredClaimPayloadV1, StructuredClaimReplayExtensionV1,
    StructuredClaimRuntimeAddressesV1, StructuredMarketRootBindingV1, StructuredMarketRootV1,
    StructuredProductLineageV1, WrapperQuantityPayloadV1, WrapperRecipeHashV1, WrapperRecipeV1,
    AuthenticatedVaultRetirementV1, StructuredClaimReplayTransitionV1,
    StructuredClaimTerminalReplayDeltaV1, StructuredProductWrapperTerminalProjectionV1,
    StructuredRootCloseDispositionV1,
    prepare_structured_descriptor_terminal_owner_v1, prepare_structured_descriptor_terminal_v1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_BYTES_V1,
    STRUCTURED_CLAIM_REPLAY_EXTENSION_SCHEMA_V1, STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
    STRUCTURED_CLAIM_TERMINAL_REPLAY_DELTA_DOMAIN_V1,
};
use clutch_structured_claim_adapter::{
    bind_descriptor_v1,
    canonical_native_claim_id_v1, canonical_series_scoped_wrapper_product_id_v2,
    decode_canonical_wrapper_mint_v1, decode_canonical_wrapper_token_v1,
    decode_retired_canonical_wrapper_mint_v1,
    finalize_current_compaction_disposition_v1,
    prepare_current_compact_donation_v1, prepare_current_redeem_terminal_v1,
    prepare_current_retire_descriptor_v1,
    prepare_current_structured_position_poststate_v1,
    prepare_current_structured_vault_poststate_v1, prepare_current_unwrap_full_v1,
    prepare_current_wrap_full_v1, AccountRoleV1,
    BasePositionPdaVerifierV1,
    CurrentStructuredLiabilitiesV1, CurrentStructuredQuantityAccountsV1,
    CurrentStructuredVaultAccountsV1,
    Error as StructuredAdapterError, PdaVerifierV1, RawAccountV1, RuntimeDeploymentsV1,
    StructuredCustodyPdaVerifierV1, STRUCTURED_CUSTODY_ACCOUNT_COUNT,
    STRUCTURED_CUSTODY_DESCRIPTOR_BODY_DOMAIN_V1,
    STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1,
    STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::accounts::{require, require_count, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::loader_state::{
    decode_loader_pair_v1, decode_synthesized_genesis_loader_pair_v1, LoaderAccountViewV1,
    UPGRADEABLE_LOADER_ID,
};
use crate::seeds;

use super::collateral_position_v3::{
    authenticate_general_market_value_authority_v2, authenticate_resolution_v5,
    GeneralMarketLiabilityAuthorityV2, RuntimeSha256,
};
use super::product_artifact::authenticate_product_artifact_v1;
use super::product_artifact::{
    authenticate_registry_capability_v3, authenticate_series_registry_capability_refs_v2,
};
use super::product_market::{
    admit_series_wrapper_obligation_v1, authenticate_series_market_link_v1,
    authenticate_series_wrapper_authorization_v1, terminalize_series_wrapper_obligation_v1,
    AuthenticatedSeriesWrapperAuthorizationV1, AuthenticatedSeriesWrapperTerminalOwnerV1,
    AuthenticatedSeriesWrapperTerminalV1,
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
const IX_COLLATERAL_TOKEN_PROGRAM_DATA: usize = 5;
const IX_MARKET_BINDING: usize = 6;
const IX_MARKET_RUNTIME: usize = 7;
const IX_SOURCE_POSITION: usize = 8;
const IX_SOURCE_REPLAY: usize = 9;
const IX_DESTINATION_POSITION: usize = 10;
const IX_DESTINATION_REPLAY: usize = 11;
const IX_ACTOR: usize = 12;
const IX_DESCRIPTOR: usize = 13;
const IX_WRAPPER_PROGRAM: usize = 14;
const IX_WRAPPER_PROGRAM_DATA: usize = 15;
const IX_BASE_PROGRAM: usize = 16;
const IX_BASE_PROGRAM_DATA: usize = 17;
const IX_TOKEN_2022_PROGRAM: usize = 18;
const IX_TOKEN_2022_PROGRAM_DATA: usize = 19;
const IX_NATIVE_CLAIM_BASIS: usize = 20;
const IX_MARKET_INSTANCE: usize = 21;
const IX_HOARD_V2: usize = 22;
const IX_CLAIM_LEDGER_V3: usize = 23;
const IX_WRAPPER_MINT: usize = 24;
const IX_WRAPPER_HOLDER: usize = 25;
const IX_WRAPPER_MINT_AUTHORITY: usize = 26;
const IX_COLLATERAL_MINT: usize = 27;
const IX_HOARD_TOKEN: usize = 28;
const IX_WRAPPER_RELEASE_V2: usize = 29;
const IX_BASE_RELEASE_V2: usize = 30;
const IX_TOKEN_RELEASE_V2: usize = 31;
const IX_RESOLUTION_V5: usize = 32;

const STRUCTURED_FULL_VECTOR_CORE_ACCOUNT_COUNT: usize = 29;
/// Exact account count for current full-vector wrap and unwind, including
/// three disjoint loader-release artifacts.
pub const STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT: usize = 32;
/// Exact account count for current terminal wrapper redemption.
pub const STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT: usize = 33;

/// Exact account count for beneficiary-free single-vault compaction. The five
/// Product/collateral authority roles are distinct from the original 27-frame:
/// Hoard authority, neutral token, Structured root, Product link, FundingTerms.
pub const STRUCTURED_COMPACTION_ACCOUNT_COUNT: usize = 32;
/// Exact action-8 frame. Hoard and ClaimLedger are current read-only semantic
/// owners; the descriptor, mint, Position/Replay, root, and optional Product
/// link terminal successor are mutated atomically.
pub const STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT: usize = 31;
const _: () = assert!(
    clutch_solana_layout::registry::STRUCTURED_MARKET_ROOT_ACCOUNT_TAG
        == clutch_structured_claim_adapter::runtime_contract::STRUCTURED_MARKET_ROOT_ACCOUNT_TAG
);
const _: () = assert!(
    clutch_solana_layout::registry::STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES
        == STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES
);
const CX_VAULT_AUTHORITY: usize = 0;
const CX_REALM: usize = 1;
const CX_PROFILE: usize = 2;
const CX_POLICY: usize = 3;
const CX_COLLATERAL_TOKEN_PROGRAM: usize = 4;
const CX_COLLATERAL_TOKEN_PROGRAM_DATA: usize = 5;
const CX_MARKET_BINDING: usize = 6;
const CX_MARKET_RUNTIME: usize = 7;
const CX_VAULT_POSITION: usize = 8;
const CX_VAULT_REPLAY: usize = 9;
const CX_DESCRIPTOR: usize = 10;
const CX_WRAPPER_PROGRAM: usize = 11;
const CX_WRAPPER_PROGRAM_DATA: usize = 12;
const CX_BASE_PROGRAM: usize = 13;
const CX_BASE_PROGRAM_DATA: usize = 14;
const CX_TOKEN_2022_PROGRAM: usize = 15;
const CX_TOKEN_2022_PROGRAM_DATA: usize = 16;
const CX_NATIVE_CLAIM_BASIS: usize = 17;
const CX_MARKET_INSTANCE: usize = 18;
const CX_HOARD_V2: usize = 19;
const CX_CLAIM_LEDGER_V3: usize = 20;
const CX_WRAPPER_MINT: usize = 21;
const CX_COLLATERAL_MINT: usize = 22;
const CX_HOARD_TOKEN: usize = 23;
const CX_HOARD_AUTHORITY: usize = 24;
const CX_NEUTRAL_TOKEN: usize = 25;
const CX_STRUCTURED_ROOT: usize = 26;
const CX_SERIES_LINK: usize = 27;
const CX_FUNDING_TERMS_V2: usize = 28;
const CX_WRAPPER_RELEASE_V2: usize = 29;
const CX_BASE_RELEASE_V2: usize = 30;
const CX_TOKEN_RELEASE_V2: usize = 31;
const _: () = assert!(CX_TOKEN_RELEASE_V2 + 1 == STRUCTURED_COMPACTION_ACCOUNT_COUNT);

const RT_VAULT_AUTHORITY: usize = 0;
const RT_REALM: usize = 1;
const RT_PROFILE: usize = 2;
const RT_POLICY: usize = 3;
const RT_COLLATERAL_TOKEN_PROGRAM: usize = 4;
const RT_COLLATERAL_TOKEN_PROGRAM_DATA: usize = 5;
const RT_MARKET_BINDING: usize = 6;
const RT_MARKET_RUNTIME: usize = 7;
const RT_POSITION: usize = 8;
const RT_REPLAY: usize = 9;
const RT_DESCRIPTOR: usize = 10;
const RT_WRAPPER_PROGRAM: usize = 11;
const RT_WRAPPER_PROGRAM_DATA: usize = 12;
const RT_BASE_PROGRAM: usize = 13;
const RT_BASE_PROGRAM_DATA: usize = 14;
const RT_TOKEN_PROGRAM: usize = 15;
const RT_TOKEN_PROGRAM_DATA: usize = 16;
const RT_BASIS: usize = 17;
const RT_MARKET_INSTANCE: usize = 18;
const RT_HOARD: usize = 19;
const RT_CLAIM_LEDGER: usize = 20;
const RT_MINT: usize = 21;
const RT_MINT_AUTHORITY: usize = 22;
const RT_STRUCTURED_ROOT: usize = 23;
const RT_SERIES_LINK: usize = 24;
const RT_RENT_REFUND_OWNER: usize = 25;
const RT_NEUTRAL_SINK: usize = 26;
const RT_WRAPPER_RELEASE_V2: usize = 27;
const RT_BASE_RELEASE_V2: usize = 28;
const RT_TOKEN_RELEASE_V2: usize = 29;
const RT_SYSTEM_PROGRAM: usize = 30;

const STRUCTURED_TERMINAL_POSITION_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/terminal-position-transition/v1\0";
const STRUCTURED_TERMINAL_RENT_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/terminal-rent-transition/v1\0";
const STRUCTURED_TERMINAL_VAULT_CLOSE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/terminal-vault-close/v1\0";
const STRUCTURED_TERMINAL_ROOT_SEMANTIC_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/terminal-root-semantic/v1\0";
const STRUCTURED_TERMINAL_ROOT_DATA_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/terminal-root-data/v1\0";

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

// ProgramData proves the selected collateral release but is not part of the
// historical Structured custody projection consumed by the adapter.
const ACCOUNT_INDICES: [usize; STRUCTURED_CUSTODY_ACCOUNT_COUNT] = [
    0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

const STRUCTURED_VAULT_CREATE_ACCOUNT_COUNT: usize = 34;
const STRUCTURED_ROOT_SEED_V1: &[u8] = b"dc:structured-root:v1";
const STRUCTURED_COMPACTION_PRODUCT_AUTHORITY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/structured-claim/compaction-product-authority/v1\0";
const CV_VAULT_AUTHORITY: usize = 0;
const CV_PAYER: usize = 1;
const CV_SYSTEM: usize = 2;
const CV_RENT: usize = 3;
const CV_REALM: usize = 4;
const CV_PROFILE: usize = 5;
const CV_POLICY: usize = 6;
const CV_COLLATERAL_TOKEN_PROGRAM: usize = 7;
const CV_COLLATERAL_TOKEN_PROGRAM_DATA: usize = 8;
const CV_MARKET_BINDING: usize = 9;
const CV_MARKET_RUNTIME: usize = 10;
const CV_POSITION: usize = 11;
const CV_REPLAY: usize = 12;
const CV_DESCRIPTOR: usize = 13;
const CV_MINT: usize = 14;
const CV_WRAPPER_PROGRAM: usize = 15;
const CV_WRAPPER_PROGRAM_DATA: usize = 16;
const CV_BASE_PROGRAM: usize = 17;
const CV_BASE_PROGRAM_DATA: usize = 18;
const CV_TOKEN_PROGRAM: usize = 19;
const CV_TOKEN_PROGRAM_DATA: usize = 20;
const CV_BASIS: usize = 21;
const CV_MARKET_INSTANCE: usize = 22;
const CV_HOARD: usize = 23;
const CV_CLAIM_LEDGER: usize = 24;
const CV_STRUCTURED_ROOT: usize = 25;
const CV_SERIES_LINK: usize = 26;
const CV_COMPILER_BUNDLE: usize = 27;
const CV_ATTACHMENT: usize = 28;
const CV_SERIES_REGISTRY_V2: usize = 29;
const CV_REGISTRY_RELEASE_V2: usize = 30;
const CV_CAPABILITY_PROFILE_V4: usize = 31;
const CV_WRAPPER_RELEASE_V2: usize = 32;
const CV_TOKEN_RELEASE_V2: usize = 33;

/// Private locus-aware deployment authority. Every field is derived from a
/// hostile-decoded release artifact plus the complete current ProgramData
/// bytes; callers cannot construct this receipt from descriptor slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedStructuredDeploymentsV2 {
    runtime: RuntimeDeploymentsV1,
    wrapper_release_id: ContentId,
    base_release_id: ContentId,
    token_release_id: ContentId,
    owner_release_id: ContentId,
}

/// Private action-6 capability. Every field is derived from the current
/// Structured root, exact Product link/FundingTerms, current collateral value
/// authority, and the family-local compaction plan. Public request structs are
/// never accepted as authority by the executable composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedStructuredCompactionV1 {
    plan: clutch_structured_claim_adapter::CurrentStructuredTransitionPlanV1,
    bound: BoundCollateralProfileV2,
    hoard_before: clutch_collateral_adapter_v2::HoardV2,
    claim_ledger_before: clutch_collateral_adapter_v2::ClaimLedgerV3,
    destination_token: CollateralId,
    destination_semantic_owner: CollateralId,
    collateral_value_receipt_id: CollateralId,
}

/// Private postwrite authority proving the exact terminal Structured root and
/// every immutable Product join consumed by the Product-owned Wrapper latch.
/// This type is constructible only after hostile reauthentication of the
/// descriptor, mint, Position tombstone, deleted Replay, and root postimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedStructuredDescriptorTerminalV1 {
    id: ContentId,
    root_account: Pubkey,
    root_semantic_id: ContentId,
    root_data_id: ContentId,
    link_account: Pubkey,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    wrapper_admission_receipt_id: ContentId,
}

impl AuthenticatedSeriesWrapperTerminalOwnerV1
    for AuthenticatedStructuredDescriptorTerminalV1
{
    fn owner_terminal_receipt_id(&self) -> Outcome<ContentId> {
        Ok(self.id)
    }

    fn structured_root_account(&self) -> Outcome<Pubkey> {
        Ok(self.root_account)
    }

    fn structured_root_semantic_id(&self) -> Outcome<ContentId> {
        Ok(self.root_semantic_id)
    }

    fn structured_root_data_id(&self) -> Outcome<ContentId> {
        Ok(self.root_data_id)
    }

    fn authenticate_series_wrapper_terminal_owner_v1(
        &self,
        link_account: Pubkey,
        series_plan_id: SeriesPlanV5Id,
        ordinal: u32,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        wrapper_admission_receipt_id: ContentId,
        owner_terminal_receipt_id: ContentId,
        structured_root_account: Pubkey,
        structured_root_semantic_id: ContentId,
        structured_root_data_id: ContentId,
    ) -> Outcome<()> {
        require(
            link_account == self.link_account
                && series_plan_id == self.series_plan_id
                && ordinal == self.ordinal
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && wrapper_admission_receipt_id == self.wrapper_admission_receipt_id
                && owner_terminal_receipt_id == self.id
                && structured_root_account == self.root_account
                && structured_root_semantic_id == self.root_semantic_id
                && structured_root_data_id == self.root_data_id,
            ClutchError::MismatchedState,
        )
    }
}

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

    let value_authority = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[CV_REALM],
        &accounts[CV_PROFILE],
        &accounts[CV_POLICY],
        &accounts[CV_COLLATERAL_TOKEN_PROGRAM],
        &accounts[CV_COLLATERAL_TOKEN_PROGRAM_DATA],
        &accounts[CV_MARKET_BINDING],
        &accounts[CV_MARKET_RUNTIME],
        &accounts[CV_MARKET_INSTANCE],
        &accounts[CV_HOARD],
        &accounts[CV_CLAIM_LEDGER],
        false,
        false,
    )?;
    let liabilities = value_authority.liabilities;
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
        deployments.runtime.binding,
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
        deployments.runtime,
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
        false, false, false, false, false, false, false, false,
    ];
    let mut writable = [
        false, true, false, false, false, false, false, false, false, false, false, true, true,
        false, false, false, false, false, false, false, false, false, false, false, false, true,
        false, false, false, false, false, false, false, false,
    ];
    writable[CV_SERIES_LINK] = structured_root_requires_product_write_v1(
        accounts[CV_STRUCTURED_ROOT].owner,
        accounts[CV_STRUCTURED_ROOT].data_len(),
    );
    let executable = [
        false, false, true, false, false, false, false, true, false, false, false, false, false,
        false, false, true, false, true, false, true, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false,
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
            let same_token_release = accounts[CV_COLLATERAL_TOKEN_PROGRAM].key
                == accounts[CV_TOKEN_PROGRAM].key;
            let collateral_token_alias = (left == CV_COLLATERAL_TOKEN_PROGRAM
                && right == CV_TOKEN_PROGRAM)
                || (same_token_release
                    && left == CV_COLLATERAL_TOKEN_PROGRAM_DATA
                    && right == CV_TOKEN_PROGRAM_DATA);
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

#[allow(clippy::too_many_arguments)]
fn admit_structured_descriptor_root_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    liabilities: GeneralMarketLiabilityAuthorityV2,
    deployments: AuthenticatedStructuredDeploymentsV2,
    descriptor: StructuredClaimDescriptorV2,
    native_claim_id: [u8; 32],
    recipe_membership: clutch_structured_claim_adapter::runtime_contract::WrapperRecipeMembershipV1,
) -> Outcome<()> {
    // Product owns the framed `0xad/1` decoder and authentication formula. The
    // fixed output buffers are adapter scratch, not persisted authority and not
    // kernel evidence. Keeping both receipts in this lexical scope avoids a
    // self-referential owner/borrowed-receipt DTO.
    let mut link_output = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let link_data = accounts[CV_SERIES_LINK]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&link_data, &mut link_output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(link_data);
    let untrusted_binding = link_output.state.binding();
    require(
        untrusted_binding.market_instance_id.bytes()
            == liabilities.market_binding.market_instance_v2_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let root_is_uninitialized = structured_root_requires_product_write_v1(
        accounts[CV_STRUCTURED_ROOT].owner,
        accounts[CV_STRUCTURED_ROOT].data_len(),
    );
    let link = authenticate_series_market_link_v1(
        program_id,
        &accounts[CV_SERIES_LINK],
        untrusted_binding.series_plan_id,
        untrusted_binding.ordinal,
        untrusted_binding.market_instance_id,
        untrusted_binding.generation,
        Pubkey::new_from_array(untrusted_binding.market_root_account_id.bytes()),
        root_is_uninitialized,
        &mut link_output,
    )?;
    let authorization = authenticate_series_wrapper_authorization_v1(
        program_id,
        link,
        &accounts[CV_COMPILER_BUNDLE],
        &accounts[CV_ATTACHMENT],
    )?;
    let registry_refs = authenticate_series_registry_capability_refs_v2(
        program_id,
        &accounts[CV_SERIES_REGISTRY_V2],
        authorization.series_plan_id(),
    )?;
    require(
        registry_refs.compiler_bundle_id() == authorization.compiler_bundle_id(),
        ClutchError::MismatchedState,
    )?;
    let registry = authenticate_registry_capability_v3(
        program_id,
        registry_refs,
        &accounts[CV_BASE_PROGRAM],
        &accounts[CV_BASE_PROGRAM_DATA],
        &accounts[CV_REGISTRY_RELEASE_V2],
        &accounts[CV_CAPABILITY_PROFILE_V4],
    )?;
    require(
        authorization.market_instance_id().bytes()
                == liabilities.market_binding.market_instance_v2_id.bytes()
            && authorization.neutral_lamport_sink().bytes()
                == liabilities.market_binding.neutral_sink.bytes()
            && authorization.rent_refund_owner().bytes() == accounts[CV_PAYER].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let compiler_bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        &accounts[CV_COMPILER_BUNDLE],
        authorization.compiler_bundle_id(),
    )?;
    require(
        compiler_bundle.semantic_id() == authorization.compiler_bundle_id()
            && compiler_bundle.value().registry_release_id == registry.registry_release_id()
            && compiler_bundle.value().capability_profile_id.content_id()
                == registry.capability_profile_id()
            && registry.series_plan_id() == authorization.series_plan_id()
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
    let compiler_release_id = compiler_bundle.value().product_compiler_release_id;
    let root_binding = structured_root_binding_v1(
        accounts,
        deployments,
        authorization,
        compiler_release_id,
        registry.registry_release_id(),
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
        authorization.wrapper_recipe_set_id().bytes(),
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

    if root_is_uninitialized {
        require(
            authorization.requires_product_admission(),
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
        let mut rebound_link_output = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
        let rebound_link = admit_series_wrapper_obligation_v1(
            program_id,
            &accounts[CV_SERIES_LINK],
            link,
            authorization,
            first_admission_receipt,
            &mut rebound_link_output,
        )?;
        let rebound_authorization = authenticate_series_wrapper_authorization_v1(
            program_id,
            rebound_link,
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
                    compiler_release_id,
                    registry.registry_release_id(),
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
            !authorization.requires_product_admission()
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
                    == authorization.wrapper_admission_receipt_id(),
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
                structured_product_lineage_v1(authorization),
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
    deployments: AuthenticatedStructuredDeploymentsV2,
    authorization: AuthenticatedSeriesWrapperAuthorizationV1,
    compiler_release_id: ContentId,
    registry_release_id: ContentId,
) -> Outcome<StructuredMarketRootBindingV1> {
    Ok(StructuredMarketRootBindingV1 {
        link_account: accounts[CV_SERIES_LINK].key.to_bytes(),
        series_plan_id: authorization.series_plan_id(),
        ordinal: authorization.ordinal(),
        market_instance_id: authorization.market_instance_id(),
        generation: authorization.generation(),
        attachment_plan_id: SeriesAttachmentPlanV4Id::from_bytes(
            authorization.attachment_plan_id().bytes(),
        ),
        compiler_output_id: CompiledProductSeriesBundleV5Id::from_bytes(
            authorization.compiler_bundle_id().bytes(),
        ),
        compiler_release_id,
        registry_release_id,
        capability_profile_id: authorization.capability_profile_id(),
        wrapper_recipe_set_id: authorization.wrapper_recipe_set_id(),
        owner_release_id: deployments.owner_release_id,
        rent_refund_owner: authorization.rent_refund_owner(),
        neutral_lamport_sink: authorization.neutral_lamport_sink(),
    })
}

fn structured_product_lineage_v1(
    authorization: AuthenticatedSeriesWrapperAuthorizationV1,
) -> StructuredProductLineageV1 {
    StructuredProductLineageV1 {
        link_binding_id: authorization.link_binding_id(),
        wrapper_obligation_configuration_id: authorization
            .wrapper_obligation_configuration_id(),
        product_admission_receipt_id: authorization.wrapper_admission_receipt_id(),
        last_observed_link_transition_sequence: authorization.link_transition_sequence(),
    }
}

fn structured_root_requires_product_write_v1(owner: &Pubkey, data_len: usize) -> bool {
    owner == &SYSTEM_PROGRAM_ID && data_len == 0
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

/// Execute current full-vector wrap, unwind, or terminal redemption under the
/// wrapper-only vault signer.
///
/// This is compiled only into the explicitly selected Structured laboratory
/// artifact by `instructions::mod`; central capability admission remains a
/// separate release decision. The call mutates both Position/Replay V3 pairs,
/// Hoard V2, and ClaimLedger V3 as one SVM transaction. It never moves the
/// Realm collateral token: immutable mint and Hoard-token observations prove
/// that the reclassification remains fully covered.
pub fn process_full_vector(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: StructuredClaimActionV1,
    payload: &[u8],
) -> Outcome<()> {
    let expected_count = if action == StructuredClaimActionV1::RedeemTerminal {
        STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT
    } else {
        STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT
    };
    require_count(accounts, expected_count)?;
    require(sequence == 0, ClutchError::Replay)?;
    require(
        matches!(
            action,
            StructuredClaimActionV1::WrapFull
                | StructuredClaimActionV1::UnwrapFull
                | StructuredClaimActionV1::RedeemTerminal
        ),
        ClutchError::UnsupportedInstruction,
    )?;
    validate_full_vector_privileges(program_id, accounts)?;
    let request = match clutch_structured_claim_adapter::runtime_contract::decode_structured_claim_payload_v1(
        action.tag(),
        payload,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
    {
        StructuredClaimPayloadV1::WrapFull(value)
        | StructuredClaimPayloadV1::UnwrapFull(value)
        | StructuredClaimPayloadV1::RedeemTerminal(value) => value,
        _ => return Err(ClutchError::NonCanonical.into()),
    };

    let value_authority = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[IX_REALM],
        &accounts[IX_PROFILE],
        &accounts[IX_COLLATERAL_POLICY],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM],
        &accounts[IX_COLLATERAL_TOKEN_PROGRAM_DATA],
        &accounts[IX_MARKET_BINDING],
        &accounts[IX_MARKET_RUNTIME],
        &accounts[IX_MARKET_INSTANCE],
        &accounts[IX_HOARD_V2],
        &accounts[IX_CLAIM_LEDGER_V3],
        true,
        true,
    )?;
    let liabilities = value_authority.liabilities;
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
    let deployments = authenticate_deployments(
        program_id,
        accounts,
        descriptor,
        [
            IX_WRAPPER_RELEASE_V2,
            IX_BASE_RELEASE_V2,
            IX_TOKEN_RELEASE_V2,
        ],
    )?;
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
        deployments.runtime.binding,
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
        product_id == request.wrapper_product_id
            && structured_replay_product(accounts)? == product_id,
        ClutchError::MismatchedState,
    )?;
    let verifier = RuntimeStructuredPdaVerifierV1;
    let addresses = derive_runtime_addresses(
        accounts[IX_WRAPPER_PROGRAM].key,
        product_id,
        descriptor,
    )?;
    require(
        addresses.descriptor == accounts[IX_DESCRIPTOR].key.to_bytes()
            && addresses.mint == accounts[IX_WRAPPER_MINT].key.to_bytes()
            && addresses.mint_authority == accounts[IX_WRAPPER_MINT_AUTHORITY].key.to_bytes()
            && addresses.vault_owner == accounts[IX_VAULT_AUTHORITY].key.to_bytes(),
        ClutchError::WrongPda,
    )?;
    let bound_descriptor = bind_descriptor_v1(
        descriptor,
        descriptor_basis,
        deployments.runtime,
        native_claim_id,
        product_id,
        addresses,
        &verifier,
    )
    .map_err(map_adapter_error)?;

    let (plan, poststate) = {
        let borrowed = accounts
            .iter()
            .map(|account| {
                account
                    .try_borrow_data()
                    .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
            })
            .collect::<Outcome<Vec<_>>>()?;
        let mut raw = Vec::with_capacity(STRUCTURED_CUSTODY_ACCOUNT_COUNT);
        let mut index = 0usize;
        while index < STRUCTURED_CUSTODY_ACCOUNT_COUNT {
            let account_index = ACCOUNT_INDICES[index];
            raw.push(RawAccountV1 {
                role: ACCOUNT_ROLES[index],
                key: accounts[account_index].key.to_bytes(),
                owner: accounts[account_index].owner.to_bytes(),
                lamports: accounts[account_index].lamports(),
                data: &borrowed[account_index],
                signer: accounts[account_index].is_signer,
                writable: accounts[account_index].is_writable,
                executable: accounts[account_index].executable,
            });
            index += 1;
        }
        let source = PositionAccountV3::decode(&borrowed[IX_SOURCE_POSITION])
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        let source_replay = ReplayV3Envelope::decode(&borrowed[IX_SOURCE_REPLAY], &RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        let destination = PositionAccountV3::decode(&borrowed[IX_DESTINATION_POSITION])
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        let destination_replay = ReplayV3Envelope::decode(
            &borrowed[IX_DESTINATION_REPLAY],
            &RuntimeSha256,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        let (user, user_replay, vault, vault_replay, user_position, user_replay_account, vault_position, vault_replay_account) =
            match action {
                StructuredClaimActionV1::WrapFull => (
                    source,
                    source_replay,
                    destination,
                    destination_replay,
                    accounts[IX_SOURCE_POSITION].key.to_bytes(),
                    accounts[IX_SOURCE_REPLAY].key.to_bytes(),
                    accounts[IX_DESTINATION_POSITION].key.to_bytes(),
                    accounts[IX_DESTINATION_REPLAY].key.to_bytes(),
                ),
                StructuredClaimActionV1::UnwrapFull
                | StructuredClaimActionV1::RedeemTerminal => (
                    destination,
                    destination_replay,
                    source,
                    source_replay,
                    accounts[IX_DESTINATION_POSITION].key.to_bytes(),
                    accounts[IX_DESTINATION_REPLAY].key.to_bytes(),
                    accounts[IX_SOURCE_POSITION].key.to_bytes(),
                    accounts[IX_SOURCE_REPLAY].key.to_bytes(),
                ),
                _ => return Err(ClutchError::UnsupportedInstruction.into()),
            };
        let mint_observed = decode_canonical_wrapper_mint_v1(
            accounts[IX_TOKEN_2022_PROGRAM].key.to_bytes(),
            accounts[IX_WRAPPER_MINT].key.to_bytes(),
            accounts[IX_WRAPPER_MINT_AUTHORITY].key.to_bytes(),
            &borrowed[IX_WRAPPER_MINT],
        )
        .map_err(map_adapter_error)?;
        let holder_observed = decode_canonical_wrapper_token_v1(
            accounts[IX_TOKEN_2022_PROGRAM].key.to_bytes(),
            accounts[IX_WRAPPER_MINT].key.to_bytes(),
            accounts[IX_WRAPPER_HOLDER].key.to_bytes(),
            accounts[IX_ACTOR].key.to_bytes(),
            &borrowed[IX_WRAPPER_HOLDER],
        )
        .map_err(map_adapter_error)?;
        let (mint_before, holder_before) = match action {
            StructuredClaimActionV1::WrapFull => (mint_observed, holder_observed),
            StructuredClaimActionV1::UnwrapFull | StructuredClaimActionV1::RedeemTerminal => {
                let mut mint_before = mint_observed;
                mint_before.supply = mint_before
                    .supply
                    .checked_add(request.quantity)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
                let mut holder_before = holder_observed;
                holder_before.amount = holder_before
                    .amount
                    .checked_add(request.quantity)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
                (mint_before, holder_before)
            }
            _ => return Err(ClutchError::UnsupportedInstruction.into()),
        };
        let route_accounts = CurrentStructuredQuantityAccountsV1 {
            descriptor: accounts[IX_DESCRIPTOR].key.to_bytes(),
            wrapper_product_id: product_id,
            user_position,
            user_replay: user_replay_account,
            vault_position,
            vault_replay: vault_replay_account,
            mint: accounts[IX_WRAPPER_MINT].key.to_bytes(),
            holder: accounts[IX_WRAPPER_HOLDER].key.to_bytes(),
            actor: accounts[IX_ACTOR].key.to_bytes(),
        };
        let liability_prestate = CurrentStructuredLiabilitiesV1 {
            hoard: liabilities.hoard,
            claim_ledger: liabilities.claim_ledger,
        };
        let collateral_value_receipt_id = value_authority.receipt_id.bytes();
        let plan = match action {
            StructuredClaimActionV1::WrapFull => prepare_current_wrap_full_v1(
                &bound_descriptor,
                liabilities.bound,
                route_accounts,
                liability_prestate,
                collateral_value_receipt_id,
                mint_before,
                holder_before,
                current_position_projection(user, &user_replay),
                current_position_projection(vault, &vault_replay),
                request,
                &RuntimeSha256,
            ),
            StructuredClaimActionV1::UnwrapFull => prepare_current_unwrap_full_v1(
                &bound_descriptor,
                liabilities.bound,
                route_accounts,
                liability_prestate,
                collateral_value_receipt_id,
                mint_before,
                holder_before,
                current_position_projection(user, &user_replay),
                current_position_projection(vault, &vault_replay),
                request,
                &RuntimeSha256,
            ),
            StructuredClaimActionV1::RedeemTerminal => {
                let resolution = authenticate_resolution_v5(
                    program_id,
                    &accounts[IX_RESOLUTION_V5],
                    liabilities,
                )?;
                prepare_current_redeem_terminal_v1(
                    &bound_descriptor,
                    liabilities.bound,
                    route_accounts,
                    liability_prestate,
                    collateral_value_receipt_id,
                    resolution.account_id.bytes(),
                    resolution.resolution,
                    mint_before,
                    holder_before,
                    current_position_projection(user, &user_replay),
                    current_position_projection(vault, &vault_replay),
                    request,
                    &RuntimeSha256,
                )
            }
            _ => return Err(ClutchError::UnsupportedInstruction.into()),
        }
        .map_err(map_adapter_error)?;
        require(
            plan.mint_supply_after == mint_observed.supply
                || action == StructuredClaimActionV1::WrapFull,
            ClutchError::MismatchedState,
        )?;
        require(
            plan.holder_after == holder_observed.amount
                || action == StructuredClaimActionV1::WrapFull,
            ClutchError::MismatchedState,
        )?;
        let poststate = prepare_current_structured_position_poststate_v1(
            &raw,
            &bound_descriptor,
            plan,
            &verifier,
        )
        .map_err(map_adapter_error)?;
        (plan, poststate)
    };

    authenticate_full_vector_collateral_observations(accounts, liabilities.bound, plan)?;
    write_full_vector_poststate(accounts, poststate, plan)
}

/// Erase all vault surplus without a beneficiary while preserving exact
/// wrapper backing. Donated cash is atomically transferred from the Hoard to
/// the Product/Realm-selected neutral account; Egg-only compaction emits no
/// token CPI. Replay commits the accepted exact collateral disposition.
pub fn process_compact_donation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, STRUCTURED_COMPACTION_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    validate_compaction_privileges(program_id, accounts)?;
    let request = match clutch_structured_claim_adapter::runtime_contract::decode_structured_claim_payload_v1(
        StructuredClaimActionV1::CompactDonation.tag(),
        payload,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
    {
        StructuredClaimPayloadV1::CompactDonation(value) => value,
        _ => return Err(ClutchError::NonCanonical.into()),
    };

    let value_authority = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[CX_REALM],
        &accounts[CX_PROFILE],
        &accounts[CX_POLICY],
        &accounts[CX_COLLATERAL_TOKEN_PROGRAM],
        &accounts[CX_COLLATERAL_TOKEN_PROGRAM_DATA],
        &accounts[CX_MARKET_BINDING],
        &accounts[CX_MARKET_RUNTIME],
        &accounts[CX_MARKET_INSTANCE],
        &accounts[CX_HOARD_V2],
        &accounts[CX_CLAIM_LEDGER_V3],
        true,
        true,
    )?;
    let liabilities = value_authority.liabilities;
    let basis_artifact = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[CX_NATIVE_CLAIM_BASIS],
        ContentId::from_bytes(liabilities.market_binding.native_claim_basis_id.bytes()),
    )?;
    let basis = *basis_artifact.value();
    require(
        basis.outcome_count == liabilities.market_binding.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let descriptor_data = accounts[CX_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let descriptor = StructuredClaimDescriptorV2::decode(&descriptor_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(descriptor_data);
    let deployments = authenticate_structured_release_set_v2(
        program_id,
        descriptor,
        [
            (
                &accounts[CX_WRAPPER_PROGRAM],
                &accounts[CX_WRAPPER_PROGRAM_DATA],
            ),
            (&accounts[CX_BASE_PROGRAM], &accounts[CX_BASE_PROGRAM_DATA]),
            (
                &accounts[CX_TOKEN_2022_PROGRAM],
                &accounts[CX_TOKEN_2022_PROGRAM_DATA],
            ),
        ],
        [
            &accounts[CX_WRAPPER_RELEASE_V2],
            &accounts[CX_BASE_RELEASE_V2],
            &accounts[CX_TOKEN_RELEASE_V2],
        ],
    )?;
    let descriptor_basis = DescriptorBasisV1 {
        market: liabilities
            .market_instance
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
        terms_digest: basis_artifact.semantic_id().bytes(),
        basis_degree: basis.basis_degree,
        denominator: basis.denominator,
        outcome_count: basis.outcome_count,
    };
    let identity = clutch_structured_claim_adapter::runtime_contract::reconstruct_descriptor_identity_v1(
        &descriptor,
        descriptor_basis,
        deployments.runtime.binding,
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
        request.wrapper_product_id == product_id,
        ClutchError::MismatchedState,
    )?;
    let addresses = derive_runtime_addresses(
        accounts[CX_WRAPPER_PROGRAM].key,
        product_id,
        descriptor,
    )?;
    require(
        addresses.descriptor == accounts[CX_DESCRIPTOR].key.to_bytes()
            && addresses.mint == accounts[CX_WRAPPER_MINT].key.to_bytes()
            && addresses.vault_owner == accounts[CX_VAULT_AUTHORITY].key.to_bytes(),
        ClutchError::WrongPda,
    )?;
    let verifier = RuntimeStructuredPdaVerifierV1;
    let bound_descriptor = bind_descriptor_v1(
        descriptor,
        descriptor_basis,
        deployments.runtime,
        native_claim_id,
        product_id,
        addresses,
        &verifier,
    )
    .map_err(map_adapter_error)?;

    let plan = {
        let vault_position_data = accounts[CX_VAULT_POSITION]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let vault_replay_data = accounts[CX_VAULT_REPLAY]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mint_data = accounts[CX_WRAPPER_MINT]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let vault = PositionAccountV3::decode(&vault_position_data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        let replay = ReplayV3Envelope::decode(&vault_replay_data, &RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        let extension = StructuredClaimReplayExtensionV1::decode(replay.extension())
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
        require(
            vault.purpose() == PositionPurposeV3::StructuredClaim
                && extension.descriptor_account == accounts[CX_DESCRIPTOR].key.to_bytes()
                && extension.wrapper_product_id == product_id
                && extension.vault_authority == accounts[CX_VAULT_AUTHORITY].key.to_bytes()
                && request.vault_generation == vault.generation()
                && request.vault_replay_sequence == replay.header().next_sequence(),
            ClutchError::MismatchedState,
        )?;
        let mint = decode_canonical_wrapper_mint_v1(
            accounts[CX_TOKEN_2022_PROGRAM].key.to_bytes(),
            accounts[CX_WRAPPER_MINT].key.to_bytes(),
            addresses.mint_authority,
            &mint_data,
        )
        .map_err(map_adapter_error)?;
        prepare_current_compact_donation_v1(
            &bound_descriptor,
            liabilities.bound,
            CurrentStructuredVaultAccountsV1 {
                descriptor: accounts[CX_DESCRIPTOR].key.to_bytes(),
                wrapper_product_id: product_id,
                vault_position: accounts[CX_VAULT_POSITION].key.to_bytes(),
                vault_replay: accounts[CX_VAULT_REPLAY].key.to_bytes(),
                mint: accounts[CX_WRAPPER_MINT].key.to_bytes(),
            },
            CurrentStructuredLiabilitiesV1 {
                hoard: liabilities.hoard,
                claim_ledger: liabilities.claim_ledger,
            },
            value_authority.receipt_id.bytes(),
            mint,
            current_position_projection(vault, &replay),
            &RuntimeSha256,
        )
        .map_err(map_adapter_error)?
    };
    let capability = authenticate_structured_compaction_v1(
        program_id,
        accounts,
        descriptor,
        liabilities.bound,
        liabilities.hoard,
        liabilities.claim_ledger,
        value_authority.receipt_id,
        plan,
    )?;
    let plan = capability.plan;
    let accepted = execute_structured_compaction_disposition(accounts, capability)?;
    let plan = finalize_current_compaction_disposition_v1(plan, accepted, &RuntimeSha256)
        .map_err(map_adapter_error)?;
    let poststate = prepare_compaction_poststate(accounts, &bound_descriptor, plan, &verifier)?;
    write_compaction_poststate(accounts, poststate, plan)
}

/// Retire one exact descriptor after the wrapper has atomically revoked its
/// zero-supply mint authority and persisted the descriptor tombstone. This is
/// the sole current action-8 base route: it consumes current owners plus
/// Product's private Wrapper terminal writer only for the final live
/// descriptor in the Structured root.
pub fn process_retire_descriptor(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    let request = match clutch_structured_claim_adapter::runtime_contract::decode_structured_claim_payload_v1(
        StructuredClaimActionV1::RetireDescriptor.tag(),
        payload,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
    {
        StructuredClaimPayloadV1::RetireDescriptor(value) => value,
        _ => return Err(ClutchError::NonCanonical.into()),
    };

    let root_data = accounts[RT_STRUCTURED_ROOT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let root_before = StructuredMarketRootV1::decode(&root_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(root_data);
    let family_terminal = root_before.live_descriptor_count == 1;
    validate_retirement_privileges(program_id, accounts, family_terminal)?;
    let current_product_lineage = authenticate_current_structured_product_lineage(
        program_id,
        accounts,
        root_before,
        family_terminal,
    )?;
    let root_before = root_before
        .observe_current_product_lineage(current_product_lineage)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let value_authority = authenticate_general_market_value_authority_v2(
        program_id,
        &accounts[RT_REALM],
        &accounts[RT_PROFILE],
        &accounts[RT_POLICY],
        &accounts[RT_COLLATERAL_TOKEN_PROGRAM],
        &accounts[RT_COLLATERAL_TOKEN_PROGRAM_DATA],
        &accounts[RT_MARKET_BINDING],
        &accounts[RT_MARKET_RUNTIME],
        &accounts[RT_MARKET_INSTANCE],
        &accounts[RT_HOARD],
        &accounts[RT_CLAIM_LEDGER],
        false,
        false,
    )?;
    let liabilities = value_authority.liabilities;
    let basis_artifact = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        &accounts[RT_BASIS],
        ContentId::from_bytes(liabilities.market_binding.native_claim_basis_id.bytes()),
    )?;
    let basis = *basis_artifact.value();
    require(
        basis.outcome_count == liabilities.market_binding.outcome_count,
        ClutchError::MismatchedState,
    )?;

    let descriptor_data = accounts[RT_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let retired_descriptor = StructuredClaimDescriptorV2::decode(&descriptor_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(descriptor_data);
    require(
        retired_descriptor.state
            == clutch_structured_claim_adapter::runtime_contract::DescriptorStateV1::Retired,
        ClutchError::MismatchedState,
    )?;
    let mut active_descriptor = retired_descriptor;
    active_descriptor.state =
        clutch_structured_claim_adapter::runtime_contract::DescriptorStateV1::Active;
    active_descriptor
        .validate_persisted()
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let deployments = authenticate_terminal_deployments(accounts, active_descriptor)?;
    let market_instance_id = liabilities
        .market_instance
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let descriptor_basis = DescriptorBasisV1 {
        market: market_instance_id.bytes(),
        terms_digest: basis_artifact.semantic_id().bytes(),
        basis_degree: basis.basis_degree,
        denominator: basis.denominator,
        outcome_count: basis.outcome_count,
    };
    let identity = clutch_structured_claim_adapter::runtime_contract::reconstruct_descriptor_identity_v1(
        &active_descriptor,
        descriptor_basis,
        deployments.runtime.binding,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let native_claim_id = canonical_native_claim_id_v1(&identity).map_err(map_adapter_error)?;
    let product_id = canonical_series_scoped_wrapper_product_id_v2(
        &identity,
        native_claim_id,
        active_descriptor.structured_root_id,
        active_descriptor.wrapper_recipe_id,
    )
    .map_err(map_adapter_error)?;
    require(
        request.wrapper_product_id == product_id,
        ClutchError::MismatchedState,
    )?;
    let addresses = derive_runtime_addresses(
        accounts[RT_WRAPPER_PROGRAM].key,
        product_id,
        active_descriptor,
    )?;
    require(
        addresses.descriptor == accounts[RT_DESCRIPTOR].key.to_bytes()
            && addresses.mint == accounts[RT_MINT].key.to_bytes()
            && addresses.mint_authority == accounts[RT_MINT_AUTHORITY].key.to_bytes()
            && addresses.vault_owner == accounts[RT_VAULT_AUTHORITY].key.to_bytes(),
        ClutchError::WrongPda,
    )?;
    let bound_descriptor = bind_descriptor_v1(
        active_descriptor,
        descriptor_basis,
        deployments.runtime,
        native_claim_id,
        product_id,
        addresses,
        &RuntimeStructuredPdaVerifierV1,
    )
    .map_err(map_adapter_error)?;

    let root_id = root_before
        .binding
        .id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_pda = Pubkey::find_program_address(
        &[STRUCTURED_ROOT_SEED_V1, &root_id.bytes()],
        program_id,
    );
    require(
        *accounts[RT_STRUCTURED_ROOT].key == root_pda.0
            && root_before.root_bump == root_pda.1
            && root_id.bytes() == active_descriptor.structured_root_id
            && root_before.binding.owner_release_id == deployments.owner_release_id
            && root_before.binding.market_instance_id == market_instance_id
            && root_before.binding.link_account == accounts[RT_SERIES_LINK].key.to_bytes()
            && root_before.binding.rent_refund_owner.bytes()
                == accounts[RT_RENT_REFUND_OWNER].key.to_bytes()
            && root_before.binding.neutral_lamport_sink.bytes()
                == accounts[RT_NEUTRAL_SINK].key.to_bytes()
            && liabilities.market_binding.neutral_sink.bytes()
                == accounts[RT_NEUTRAL_SINK].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;

    let position_data = accounts[RT_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let replay_data = accounts[RT_REPLAY]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mint_data = accounts[RT_MINT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let position = PositionAccountV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let replay = ReplayV3Envelope::decode(&replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let extension = StructuredClaimReplayExtensionV1::decode(replay.extension())
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let retired_mint = decode_retired_canonical_wrapper_mint_v1(
        accounts[RT_TOKEN_PROGRAM].key.to_bytes(),
        accounts[RT_MINT].key.to_bytes(),
        &mint_data,
    )
    .map_err(map_adapter_error)?;
    require(
        position.purpose() == PositionPurposeV3::StructuredClaim
            && position.lifecycle() == PositionLifecycleV3::Open
            && position.outstanding_reservations() == 0
            && position.cash_atoms() == 0
            && position.reserved_cash_atoms() == 0
            && position.native_eggs() == [0; clutch_retirement::MAX_OUTCOMES]
            && position.owner().bytes() == addresses.vault_owner
            && position.controller().bytes() == addresses.vault_owner
            && position.purpose_binding_id().bytes() == product_id
            && position.replay_account().bytes() == accounts[RT_REPLAY].key.to_bytes()
            && replay.header().position_account().bytes() == accounts[RT_POSITION].key.to_bytes()
            && replay.header().replay_account().bytes() == accounts[RT_REPLAY].key.to_bytes()
            && replay.header().purpose() == PositionPurposeV3::StructuredClaim
            && replay.header().purpose_binding_id().bytes() == product_id
            && replay.header().position_generation() == position.generation()
            && replay.header().next_sequence() == request.vault_replay_sequence
            && request.vault_generation == position.generation()
            && extension.descriptor_account == accounts[RT_DESCRIPTOR].key.to_bytes()
            && extension.wrapper_product_id == product_id
            && extension.vault_authority == addresses.vault_owner
            && extension.current_position_semantic_id
                == position
                    .semantic_id(&RuntimeSha256)
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .bytes(),
        ClutchError::MismatchedState,
    )?;

    let position_semantic_before = position
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_position = PositionAccountV3::new(PositionV3Fields {
        lifecycle: PositionLifecycleV3::CloseRequested,
        ..position.fields()
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_position_semantic = terminal_position
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_transition_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            STRUCTURED_TERMINAL_POSITION_TRANSITION_DOMAIN_V1,
            &value_authority.receipt_id.bytes(),
            &root_id.bytes(),
            accounts[RT_DESCRIPTOR].key.as_ref(),
            &product_id,
            accounts[RT_MINT].key.as_ref(),
            accounts[RT_POSITION].key.as_ref(),
            accounts[RT_REPLAY].key.as_ref(),
            &position_semantic_before.bytes(),
            &terminal_position_semantic.bytes(),
            &request.vault_generation.to_le_bytes(),
            &request.vault_replay_sequence.to_le_bytes(),
            &accounts[RT_POSITION].lamports().to_le_bytes(),
            &accounts[RT_REPLAY].lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!terminal_transition_id.is_zero(), ClutchError::MismatchedState)?;
    let terminal_delta = StructuredClaimTerminalReplayDeltaV1 {
        action: StructuredClaimActionV1::RetireDescriptor,
        sequence: request.vault_replay_sequence,
        transition_id: terminal_transition_id.bytes(),
        position_account: accounts[RT_POSITION].key.to_bytes(),
        position_pre_semantic_id: position_semantic_before.bytes(),
        position_terminal_semantic_id: terminal_position_semantic.bytes(),
    };
    let terminal_delta_body = terminal_delta
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_delta_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            STRUCTURED_CLAIM_TERMINAL_REPLAY_DELTA_DOMAIN_V1,
            &terminal_delta_body,
        ])
        .to_bytes(),
    );
    let terminal_extension = extension
        .terminalized(StructuredClaimReplayTransitionV1 {
            descriptor_account: accounts[RT_DESCRIPTOR].key.to_bytes(),
            wrapper_product_id: product_id,
            vault_authority: addresses.vault_owner,
            action: StructuredClaimActionV1::RetireDescriptor,
            transition_id: terminal_transition_id.bytes(),
            delta_id: terminal_delta_id.bytes(),
            position_pre_semantic_id: position_semantic_before.bytes(),
            position_post_semantic_id: terminal_position_semantic.bytes(),
        })
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_replay_header = replay
        .header()
        .terminalized(position.generation(), &terminal_extension, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let terminal_replay = ReplayV3Envelope::from_header(
        terminal_replay_header,
        &terminal_extension,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let payer = position.rent().payer;
    let neutral_sink = id(accounts[RT_NEUTRAL_SINK].key.to_bytes())?;
    require(
        payer.bytes() == accounts[RT_RENT_REFUND_OWNER].key.to_bytes()
            && replay.header().rent().payer() == payer
            && payer != neutral_sink,
        ClutchError::MismatchedState,
    )?;
    let retirement = plan_position_v3_replay_v3_retirement_v1(
        PositionV3ReplayV3RetirementRequestV1 {
            position: terminal_position
                .terminal_projection()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            replay: terminal_replay
                .terminal_projection()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            position_balance: accounts[RT_POSITION].lamports(),
            replay_balance: accounts[RT_REPLAY].lamports(),
            neutral_sink,
            accounts: PositionV3ReplayV3AccountsV1 {
                position: id(accounts[RT_POSITION].key.to_bytes())?,
                replay: id(accounts[RT_REPLAY].key.to_bytes())?,
            },
            recipient_balances: RecipientBalanceBookV1 {
                entries: [
                    Some(RecipientBalanceV1 {
                        recipient: payer,
                        balance_before: accounts[RT_RENT_REFUND_OWNER].lamports(),
                    }),
                    Some(RecipientBalanceV1 {
                        recipient: neutral_sink,
                        balance_before: accounts[RT_NEUTRAL_SINK].lamports(),
                    }),
                    None,
                    None,
                ],
            },
            signed_sequence: request
                .vault_replay_sequence
                .checked_add(1)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        },
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let tombstone_semantic_id = retirement
        .position_tombstone
        .semantic_id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let position_rent_body = position
        .rent()
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_rent_body = replay
        .header()
        .rent()
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent_transition_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            STRUCTURED_TERMINAL_RENT_TRANSITION_DOMAIN_V1,
            accounts[RT_POSITION].key.as_ref(),
            &position_rent_body,
            accounts[RT_REPLAY].key.as_ref(),
            &replay_rent_body,
            accounts[RT_RENT_REFUND_OWNER].key.as_ref(),
            accounts[RT_NEUTRAL_SINK].key.as_ref(),
        ])
        .to_bytes(),
    );
    let position_principal = position
        .rent()
        .refundable_live_principal
        .checked_add(position.rent().permanent_tombstone_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let position_donation = accounts[RT_POSITION]
        .lamports()
        .checked_sub(position_principal)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_donation = accounts[RT_REPLAY]
        .lamports()
        .checked_sub(replay.header().rent().refundable_principal())
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let payer_credit = retirement
        .recipient_credits
        .get(payer)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let sink_credit = retirement
        .recipient_credits
        .get(neutral_sink)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let vault_close_receipt = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            STRUCTURED_TERMINAL_VAULT_CLOSE_DOMAIN_V1,
            &terminal_transition_id.bytes(),
            &terminal_delta_id.bytes(),
            &retirement.terminal_replay_semantic_id.bytes(),
            &tombstone_semantic_id.bytes(),
            &rent_transition_id.bytes(),
            &value_authority.receipt_id.bytes(),
            &payer_credit.credit_lamports.to_le_bytes(),
            &sink_credit.credit_lamports.to_le_bytes(),
            &retirement.position_balance_after.to_le_bytes(),
            &retirement.replay_balance_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    let descriptor_retirement = prepare_current_retire_descriptor_v1(
        &bound_descriptor,
        liabilities.bound,
        CurrentStructuredVaultAccountsV1 {
            descriptor: accounts[RT_DESCRIPTOR].key.to_bytes(),
            wrapper_product_id: product_id,
            vault_position: accounts[RT_POSITION].key.to_bytes(),
            vault_replay: accounts[RT_REPLAY].key.to_bytes(),
            mint: accounts[RT_MINT].key.to_bytes(),
        },
        CurrentStructuredLiabilitiesV1 {
            hoard: liabilities.hoard,
            claim_ledger: liabilities.claim_ledger,
        },
        value_authority.receipt_id.bytes(),
        retired_mint,
        current_position_projection(position, &replay),
        request,
        AuthenticatedVaultRetirementV1 {
            close_receipt: vault_close_receipt.bytes(),
            market: market_instance_id.bytes(),
            vault_owner: addresses.vault_owner,
            position_account: accounts[RT_POSITION].key.to_bytes(),
            replay_account: accounts[RT_REPLAY].key.to_bytes(),
            generation: request.vault_generation,
            replay_sequence: request.vault_replay_sequence,
            tombstone: tombstone_semantic_id.bytes(),
            terminal_replay_semantic_id: retirement.terminal_replay_semantic_id.bytes(),
            rent_transition_id: rent_transition_id.bytes(),
            rent_refund_owner: accounts[RT_RENT_REFUND_OWNER].key.to_bytes(),
            neutral_lamport_sink: accounts[RT_NEUTRAL_SINK].key.to_bytes(),
            position_tombstone_principal_lamports: retirement.position_balance_after,
            position_refund_lamports: position.rent().refundable_live_principal,
            replay_refund_lamports: replay.header().rent().refundable_principal(),
            position_donation_lamports: position_donation,
            replay_donation_lamports: replay_donation,
        },
    )
    .map_err(map_adapter_error)?;
    let owner_plan = prepare_structured_descriptor_terminal_owner_v1(
        root_before,
        accounts[RT_STRUCTURED_ROOT].lamports(),
        accounts[RT_STRUCTURED_ROOT].key.to_bytes(),
        product_id,
        accounts[RT_MINT].key.to_bytes(),
        accounts[RT_DESCRIPTOR].key.to_bytes(),
        active_descriptor,
        descriptor_retirement,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        (owner_plan.root_after.live_descriptor_count == 0) == family_terminal,
        ClutchError::MismatchedState,
    )?;

    preflight_retirement_balances(accounts, retirement, owner_plan.root_after, family_terminal)?;
    apply_vault_retirement(accounts, retirement)?;
    write_and_reauthenticate_structured_root_v1(
        &accounts[RT_STRUCTURED_ROOT],
        &owner_plan.root_after,
        root_id,
        root_pda.1,
        program_id,
    )?;
    reauthenticate_retired_descriptor_vault(
        accounts,
        retired_descriptor,
        retired_mint,
        retirement,
    )?;

    let product_terminal = if family_terminal {
        let structured_terminal = authenticate_structured_terminal_postwrite(
            accounts,
            owner_plan.root_after,
            root_id,
            retirement,
            retired_descriptor,
            retired_mint,
        )?;
        Some(terminalize_product_wrapper(
            program_id,
            accounts,
            root_before,
            structured_terminal,
        )?)
    } else {
        None
    };
    let product_projection = match product_terminal {
        Some(terminal) => {
            let projection = terminal.product_terminal_projection();
            let obligation_terminal_receipt_id = projection
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            Some(StructuredProductWrapperTerminalProjectionV1 {
                link_account: terminal.link_account().to_bytes(),
                market_instance_id: root_before.binding.market_instance_id.bytes(),
                generation: root_before.binding.generation,
                previous_link_authentication_id: terminal.link_authentication_before(),
                previous_link_semantic_id: terminal.link_semantic_before(),
                previous_link_transition_sequence: projection
                    .link_transition_sequence
                    .checked_sub(1)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
                product_admission_receipt_id: terminal.wrapper_admission_receipt_id(),
                owner_terminal_receipt_id: terminal.owner_terminal_receipt_id(),
                obligation_terminal_receipt_id,
                successor_link_authentication_id: terminal.link_authentication_after(),
                successor_link_semantic_id: terminal.link_semantic_after(),
                successor_link_transition_sequence: projection.link_transition_sequence,
            })
        }
        None => None,
    };
    let complete = prepare_structured_descriptor_terminal_v1(
        root_before,
        accounts[RT_STRUCTURED_ROOT].lamports(),
        accounts[RT_STRUCTURED_ROOT].key.to_bytes(),
        product_id,
        accounts[RT_MINT].key.to_bytes(),
        accounts[RT_DESCRIPTOR].key.to_bytes(),
        active_descriptor,
        descriptor_retirement,
        product_projection,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        complete.root_after == owner_plan.root_after
            && complete.descriptor_terminal_receipt_id
                == owner_plan.descriptor_terminal_receipt_id,
        ClutchError::MismatchedState,
    )?;
    if let Some(close) = complete.root_close {
        close_structured_root(accounts, close)?;
    }
    Ok(())
}

fn validate_retirement_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    family_terminal: bool,
) -> Outcome<()> {
    let mut index = 0usize;
    while index < accounts.len() {
        let signer = index == RT_VAULT_AUTHORITY;
        let writable = structured_retirement_account_writable(index, family_terminal);
        let executable = matches!(
            index,
            RT_COLLATERAL_TOKEN_PROGRAM
                | RT_WRAPPER_PROGRAM
                | RT_BASE_PROGRAM
                | RT_TOKEN_PROGRAM
                | RT_SYSTEM_PROGRAM
        );
        require(
            accounts[index].is_signer == signer
                && accounts[index].is_writable == writable
                && accounts[index].executable == executable,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    require_system_program(&accounts[RT_SYSTEM_PROGRAM])?;
    require(
        *accounts[RT_BASE_PROGRAM].key == *program_id
            && accounts[RT_POSITION].owner == program_id
            && accounts[RT_REPLAY].owner == program_id
            && accounts[RT_STRUCTURED_ROOT].owner == program_id
            && accounts[RT_SERIES_LINK].owner == program_id
            && accounts[RT_DESCRIPTOR].owner == accounts[RT_WRAPPER_PROGRAM].key
            && accounts[RT_MINT].owner == accounts[RT_TOKEN_PROGRAM].key
            && accounts[RT_MINT_AUTHORITY].owner == &SYSTEM_PROGRAM_ID
            && accounts[RT_MINT_AUTHORITY].data_len() == 0
            && accounts[RT_RENT_REFUND_OWNER].owner == &SYSTEM_PROGRAM_ID
            && accounts[RT_RENT_REFUND_OWNER].data_len() == 0
            && accounts[RT_NEUTRAL_SINK].owner == &SYSTEM_PROGRAM_ID
            && accounts[RT_NEUTRAL_SINK].data_len() == 0,
        ClutchError::MismatchedState,
    )?;
    for release in [
        RT_WRAPPER_RELEASE_V2,
        RT_BASE_RELEASE_V2,
        RT_TOKEN_RELEASE_V2,
    ] {
        require(
            accounts[release].owner == program_id,
            ClutchError::MismatchedState,
        )?;
    }
    let same_token_release =
        accounts[RT_COLLATERAL_TOKEN_PROGRAM].key == accounts[RT_TOKEN_PROGRAM].key;
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let token_alias = (left == RT_COLLATERAL_TOKEN_PROGRAM
                && right == RT_TOKEN_PROGRAM)
                || (same_token_release
                    && left == RT_COLLATERAL_TOKEN_PROGRAM_DATA
                    && right == RT_TOKEN_PROGRAM_DATA);
            require(
                accounts[left].key != accounts[right].key || token_alias,
                ClutchError::MismatchedState,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

const fn structured_retirement_account_writable(
    index: usize,
    family_terminal: bool,
) -> bool {
    matches!(
        index,
        RT_POSITION
            | RT_REPLAY
            | RT_DESCRIPTOR
            | RT_MINT
            | RT_STRUCTURED_ROOT
            | RT_RENT_REFUND_OWNER
            | RT_NEUTRAL_SINK
    ) || (index == RT_SERIES_LINK && family_terminal)
}

fn authenticate_terminal_deployments(
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV2,
) -> Outcome<AuthenticatedStructuredDeploymentsV2> {
    authenticate_structured_release_set_v2(
        accounts[RT_BASE_PROGRAM].key,
        descriptor,
        [
            (
                &accounts[RT_WRAPPER_PROGRAM],
                &accounts[RT_WRAPPER_PROGRAM_DATA],
            ),
            (&accounts[RT_BASE_PROGRAM], &accounts[RT_BASE_PROGRAM_DATA]),
            (&accounts[RT_TOKEN_PROGRAM], &accounts[RT_TOKEN_PROGRAM_DATA]),
        ],
        [
            &accounts[RT_WRAPPER_RELEASE_V2],
            &accounts[RT_BASE_RELEASE_V2],
            &accounts[RT_TOKEN_RELEASE_V2],
        ],
    )
}

fn preflight_retirement_balances(
    accounts: &[AccountInfo<'_>],
    retirement: PositionV3ReplayV3RetirementPlanV1,
    root_after: StructuredMarketRootV1,
    family_terminal: bool,
) -> Outcome<()> {
    let payer = id(accounts[RT_RENT_REFUND_OWNER].key.to_bytes())?;
    let sink = id(accounts[RT_NEUTRAL_SINK].key.to_bytes())?;
    let payer_credit = retirement
        .recipient_credits
        .get(payer)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    let sink_credit = retirement
        .recipient_credits
        .get(sink)
        .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        payer_credit.balance_after
            == accounts[RT_RENT_REFUND_OWNER]
                .lamports()
                .checked_add(payer_credit.credit_lamports)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && sink_credit.balance_after
                == accounts[RT_NEUTRAL_SINK]
                    .lamports()
                    .checked_add(sink_credit.credit_lamports)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && retirement.replay_balance_after == 0
            && root_after
                .rent_principal_lamports
                .checked_add(root_after.current_donation_lamports)
                == Some(accounts[RT_STRUCTURED_ROOT].lamports()),
        ClutchError::MismatchedState,
    )?;
    if family_terminal {
        payer_credit
            .balance_after
            .checked_add(root_after.rent_principal_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        sink_credit
            .balance_after
            .checked_add(root_after.current_donation_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    Ok(())
}

fn apply_vault_retirement(
    accounts: &[AccountInfo<'_>],
    retirement: PositionV3ReplayV3RetirementPlanV1,
) -> Outcome<()> {
    let tombstone_body = retirement
        .position_tombstone
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    set_structured_lamports(&accounts[RT_POSITION], retirement.position_balance_after)?;
    accounts[RT_POSITION]
        .resize(POSITION_TOMBSTONE_V3_BYTES)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    accounts[RT_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&tombstone_body);
    set_structured_lamports(&accounts[RT_REPLAY], retirement.replay_balance_after)?;
    accounts[RT_REPLAY]
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    accounts[RT_REPLAY].assign(&SYSTEM_PROGRAM_ID);
    for (account_index, recipient) in [
        (RT_RENT_REFUND_OWNER, id(accounts[RT_RENT_REFUND_OWNER].key.to_bytes())?),
        (RT_NEUTRAL_SINK, id(accounts[RT_NEUTRAL_SINK].key.to_bytes())?),
    ] {
        let credit = retirement
            .recipient_credits
            .get(recipient)
            .ok_or(Refusal::Adapter(ClutchError::MismatchedState))?;
        set_structured_lamports(&accounts[account_index], credit.balance_after)?;
    }
    Ok(())
}

fn reauthenticate_retired_descriptor_vault(
    accounts: &[AccountInfo<'_>],
    expected_descriptor: StructuredClaimDescriptorV2,
    expected_mint: clutch_structured_claim_adapter::runtime_contract::WrapperMintProjectionV1,
    retirement: PositionV3ReplayV3RetirementPlanV1,
) -> Outcome<()> {
    let descriptor_data = accounts[RT_DESCRIPTOR]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed_descriptor = StructuredClaimDescriptorV2::decode(&descriptor_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(descriptor_data);
    let mint_data = accounts[RT_MINT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed_mint = decode_retired_canonical_wrapper_mint_v1(
        accounts[RT_TOKEN_PROGRAM].key.to_bytes(),
        accounts[RT_MINT].key.to_bytes(),
        &mint_data,
    )
    .map_err(map_adapter_error)?;
    drop(mint_data);
    let position_data = accounts[RT_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed_tombstone = clutch_retirement::PositionTombstoneV3::decode(&position_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    require(
        observed_descriptor == expected_descriptor
            && observed_mint == expected_mint
            && observed_tombstone == retirement.position_tombstone
            && accounts[RT_POSITION].owner == accounts[RT_BASE_PROGRAM].key
            && accounts[RT_POSITION].data_len() == POSITION_TOMBSTONE_V3_BYTES
            && accounts[RT_POSITION].lamports() == retirement.position_balance_after
            && accounts[RT_REPLAY].owner == &SYSTEM_PROGRAM_ID
            && accounts[RT_REPLAY].data_len() == 0
            && accounts[RT_REPLAY].lamports() == 0,
        ClutchError::MismatchedState,
    )
}

fn authenticate_structured_terminal_postwrite(
    accounts: &[AccountInfo<'_>],
    root: StructuredMarketRootV1,
    root_id: ContentId,
    retirement: PositionV3ReplayV3RetirementPlanV1,
    descriptor: StructuredClaimDescriptorV2,
    mint: clutch_structured_claim_adapter::runtime_contract::WrapperMintProjectionV1,
) -> Outcome<AuthenticatedStructuredDescriptorTerminalV1> {
    let root_body = root
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_data = accounts[RT_STRUCTURED_ROOT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        root_data.as_ref() == &root_body[..],
        ClutchError::MismatchedState,
    )?;
    drop(root_data);
    let descriptor_body = descriptor
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let data_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[STRUCTURED_TERMINAL_ROOT_DATA_DOMAIN_V1, &root_body])
            .to_bytes(),
    );
    let semantic_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            STRUCTURED_TERMINAL_ROOT_SEMANTIC_DOMAIN_V1,
            &root_id.bytes(),
            &root.aggregate_terminal_receipt_id.bytes(),
            &root.transition_sequence.to_le_bytes(),
            &root.admitted_descriptor_count.to_le_bytes(),
            &root.terminal_descriptor_count.to_le_bytes(),
            &data_id.bytes(),
            accounts[RT_DESCRIPTOR].key.as_ref(),
            &descriptor_body,
            accounts[RT_MINT].key.as_ref(),
            &mint.supply.to_le_bytes(),
            &retirement
                .position_tombstone
                .semantic_id(&RuntimeSha256)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes(),
            &retirement.terminal_replay_semantic_id.bytes(),
        ])
        .to_bytes(),
    );
    require(
        root.live_descriptor_count == 0
            && root.terminal_descriptor_count == root.admitted_descriptor_count
            && !root.aggregate_terminal_receipt_id.is_zero()
            && !data_id.is_zero()
            && !semantic_id.is_zero()
            && data_id != semantic_id,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedStructuredDescriptorTerminalV1 {
        id: root.aggregate_terminal_receipt_id,
        root_account: *accounts[RT_STRUCTURED_ROOT].key,
        root_semantic_id: semantic_id,
        root_data_id: data_id,
        link_account: *accounts[RT_SERIES_LINK].key,
        series_plan_id: root.binding.series_plan_id,
        ordinal: root.binding.ordinal,
        market_instance_id: root.binding.market_instance_id,
        generation: root.binding.generation,
        wrapper_admission_receipt_id: root.product_lineage.product_admission_receipt_id,
    })
}

fn terminalize_product_wrapper(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    root_before: StructuredMarketRootV1,
    owner: AuthenticatedStructuredDescriptorTerminalV1,
) -> Outcome<AuthenticatedSeriesWrapperTerminalV1> {
    let mut link_output = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let link_data = accounts[RT_SERIES_LINK]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&link_data, &mut link_output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(link_data);
    let link_binding = link_output.state.binding();
    let link = authenticate_series_market_link_v1(
        program_id,
        &accounts[RT_SERIES_LINK],
        root_before.binding.series_plan_id,
        root_before.binding.ordinal,
        root_before.binding.market_instance_id,
        root_before.binding.generation,
        Pubkey::new_from_array(link_binding.market_root_account_id.bytes()),
        true,
        &mut link_output,
    )?;
    let current_binding = link.state().binding();
    require(
        link.account() == *accounts[RT_SERIES_LINK].key
            && current_binding
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == root_before.product_lineage.link_binding_id
            && current_binding.obligation_configuration_id.content_id()
                == root_before
                    .product_lineage
                    .wrapper_obligation_configuration_id
            && link.state().transition_sequence()
                >= root_before
                    .product_lineage
                    .last_observed_link_transition_sequence
            && link.state().obligation_admission_receipt_id(
                clutch_product_series::SeriesLinkObligationV1::Wrapper,
            ) == root_before.product_lineage.product_admission_receipt_id,
        ClutchError::MismatchedState,
    )?;
    let mut rebound = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    terminalize_series_wrapper_obligation_v1(
        program_id,
        &accounts[RT_SERIES_LINK],
        link,
        &owner,
        &mut rebound,
    )
}

fn authenticate_current_structured_product_lineage(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    root: StructuredMarketRootV1,
    require_writable: bool,
) -> Outcome<StructuredProductLineageV1> {
    let mut link_output = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let link_data = accounts[RT_SERIES_LINK]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&link_data, &mut link_output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(link_data);
    let untrusted = link_output.state.binding();
    let link = authenticate_series_market_link_v1(
        program_id,
        &accounts[RT_SERIES_LINK],
        root.binding.series_plan_id,
        root.binding.ordinal,
        root.binding.market_instance_id,
        root.binding.generation,
        Pubkey::new_from_array(untrusted.market_root_account_id.bytes()),
        require_writable,
        &mut link_output,
    )?;
    let binding = link.state().binding();
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let sequence = link.state().transition_sequence();
    require(
        binding_id == root.product_lineage.link_binding_id
            && binding.obligation_configuration_id.content_id()
                == root.product_lineage.wrapper_obligation_configuration_id
            && link.state().phase() == SeriesMarketLinkPhaseV1::Active
            && link.state().obligation_status(SeriesLinkObligationV1::Wrapper)
                == SeriesLinkObligationStatusV1::Live
            && link.state().obligation_admission_receipt_id(SeriesLinkObligationV1::Wrapper)
                == root.product_lineage.product_admission_receipt_id
            && sequence >= root.product_lineage.last_observed_link_transition_sequence,
        ClutchError::MismatchedState,
    )?;
    Ok(StructuredProductLineageV1 {
        link_binding_id: binding_id,
        wrapper_obligation_configuration_id: binding
            .obligation_configuration_id
            .content_id(),
        product_admission_receipt_id: root.product_lineage.product_admission_receipt_id,
        last_observed_link_transition_sequence: sequence,
    })
}

fn close_structured_root(
    accounts: &[AccountInfo<'_>],
    close: StructuredRootCloseDispositionV1,
) -> Outcome<()> {
    require(
        close.root_account == accounts[RT_STRUCTURED_ROOT].key.to_bytes()
            && close.rent_refund_owner == accounts[RT_RENT_REFUND_OWNER].key.to_bytes()
            && close.neutral_lamport_sink == accounts[RT_NEUTRAL_SINK].key.to_bytes()
            && close.balance_before_lamports == accounts[RT_STRUCTURED_ROOT].lamports()
            && close.balance_after_lamports == 0
            && close.refund_lamports.checked_add(close.donation_lamports)
                == Some(close.balance_before_lamports),
        ClutchError::MismatchedState,
    )?;
    let refund_after = accounts[RT_RENT_REFUND_OWNER]
        .lamports()
        .checked_add(close.refund_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let sink_after = accounts[RT_NEUTRAL_SINK]
        .lamports()
        .checked_add(close.donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    set_structured_lamports(&accounts[RT_STRUCTURED_ROOT], 0)?;
    accounts[RT_STRUCTURED_ROOT]
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    accounts[RT_STRUCTURED_ROOT].assign(&SYSTEM_PROGRAM_ID);
    set_structured_lamports(&accounts[RT_RENT_REFUND_OWNER], refund_after)?;
    set_structured_lamports(&accounts[RT_NEUTRAL_SINK], sink_after)?;
    require(
        accounts[RT_STRUCTURED_ROOT].lamports() == 0
            && accounts[RT_STRUCTURED_ROOT].data_len() == 0
            && accounts[RT_STRUCTURED_ROOT].owner == &SYSTEM_PROGRAM_ID
            && accounts[RT_RENT_REFUND_OWNER].lamports() == refund_after
            && accounts[RT_NEUTRAL_SINK].lamports() == sink_after,
        ClutchError::MismatchedState,
    )
}

fn set_structured_lamports(account: &AccountInfo<'_>, value: u64) -> Outcome<()> {
    **account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = value;
    Ok(())
}

fn validate_compaction_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    let signer = [
        true, false, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false, false, false, false,
    ];
    let writable = [
        false, false, false, false, false, false, false, false, true, true, false, false, false,
        false, false, false, false, false, false, true, true, false, false, true, false, true,
        false, false, false, false, false, false,
    ];
    let executable = [
        false, false, false, false, true, false, false, false, false, false, false, true, false,
        true, false, true, false, false, false, false, false, false, false, false, false, false,
        false, false, false, false, false, false,
    ];
    let mut index = 0usize;
    while index < accounts.len() {
        require(
            accounts[index].is_signer == signer[index]
                && accounts[index].is_writable == writable[index]
                && accounts[index].executable == executable[index],
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    require(
        *accounts[CX_BASE_PROGRAM].key == *program_id
            && accounts[CX_DESCRIPTOR].owner == accounts[CX_WRAPPER_PROGRAM].key
            && accounts[CX_WRAPPER_MINT].owner == accounts[CX_TOKEN_2022_PROGRAM].key
            && accounts[CX_COLLATERAL_MINT].owner == accounts[CX_COLLATERAL_TOKEN_PROGRAM].key
            && accounts[CX_HOARD_TOKEN].owner == accounts[CX_COLLATERAL_TOKEN_PROGRAM].key
            && accounts[CX_NEUTRAL_TOKEN].owner == accounts[CX_COLLATERAL_TOKEN_PROGRAM].key
            && accounts[CX_STRUCTURED_ROOT].owner == program_id
            && accounts[CX_SERIES_LINK].owner == program_id
            && accounts[CX_FUNDING_TERMS_V2].owner == program_id,
        ClutchError::MismatchedState,
    )?;
    for release in [
        CX_WRAPPER_RELEASE_V2,
        CX_BASE_RELEASE_V2,
        CX_TOKEN_RELEASE_V2,
    ] {
        require(
            accounts[release].owner == program_id,
            ClutchError::MismatchedState,
        )?;
    }
    let same_token_release =
        accounts[CX_COLLATERAL_TOKEN_PROGRAM].key == accounts[CX_TOKEN_2022_PROGRAM].key;
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let token_alias = (left == CX_COLLATERAL_TOKEN_PROGRAM
                && right == CX_TOKEN_2022_PROGRAM)
                || (same_token_release
                    && left == CX_COLLATERAL_TOKEN_PROGRAM_DATA
                    && right == CX_TOKEN_2022_PROGRAM_DATA);
            require(
                accounts[left].key != accounts[right].key || token_alias,
                ClutchError::MismatchedState,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn validate_full_vector_privileges(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    let signer = [
        true, false, false, false, false, false, false, false, false, false, false, false, true,
        false, false, false, false, false, false, false, false, false, false, false, false, false,
        false, false, false,
    ];
    let writable = [
        false, false, false, false, false, false, false, false, true, true, true, true, false,
        false, false, false, false, false, false, false, false, false, true, true, true, true,
        false, false, false,
    ];
    let executable = [
        false, false, false, false, true, false, false, false, false, false, false, false, false,
        false, true, false, true, false, true, false, false, false, false, false, false, false,
        false, false, false,
    ];
    let mut index = 0usize;
    while index < STRUCTURED_FULL_VECTOR_CORE_ACCOUNT_COUNT {
        require(
            accounts[index].is_signer == signer[index]
                && accounts[index].is_writable == writable[index]
                && accounts[index].executable == executable[index],
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    for release_index in [
        IX_WRAPPER_RELEASE_V2,
        IX_BASE_RELEASE_V2,
        IX_TOKEN_RELEASE_V2,
    ] {
        require(
            !accounts[release_index].is_signer
                && !accounts[release_index].is_writable
                && !accounts[release_index].executable
                && accounts[release_index].owner == program_id,
            ClutchError::MismatchedState,
        )?;
    }
    if accounts.len() == STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT {
        require(
            !accounts[IX_RESOLUTION_V5].is_signer
                && !accounts[IX_RESOLUTION_V5].is_writable
                && !accounts[IX_RESOLUTION_V5].executable
                && accounts[IX_RESOLUTION_V5].owner == program_id,
            ClutchError::MismatchedState,
        )?;
    }
    require(
        *accounts[IX_BASE_PROGRAM].key == *program_id
            && accounts[IX_DESCRIPTOR].owner == accounts[IX_WRAPPER_PROGRAM].key
            && accounts[IX_WRAPPER_MINT].owner == accounts[IX_TOKEN_2022_PROGRAM].key
            && accounts[IX_WRAPPER_HOLDER].owner == accounts[IX_TOKEN_2022_PROGRAM].key
            && accounts[IX_COLLATERAL_MINT].owner == accounts[IX_COLLATERAL_TOKEN_PROGRAM].key
            && accounts[IX_HOARD_TOKEN].owner == accounts[IX_COLLATERAL_TOKEN_PROGRAM].key,
        ClutchError::MismatchedState,
    )?;
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let same_token_release = accounts[IX_COLLATERAL_TOKEN_PROGRAM].key
                == accounts[IX_TOKEN_2022_PROGRAM].key;
            let token_program_alias = (left == IX_COLLATERAL_TOKEN_PROGRAM
                && right == IX_TOKEN_2022_PROGRAM)
                || (same_token_release
                    && left == IX_COLLATERAL_TOKEN_PROGRAM_DATA
                    && right == IX_TOKEN_2022_PROGRAM_DATA);
            require(
                accounts[left].key != accounts[right].key || token_program_alias,
                ClutchError::MismatchedState,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn current_position_projection(
    position: PositionAccountV3,
    replay: &ReplayV3Envelope<'_>,
) -> clutch_structured_claim_adapter::runtime_contract::PositionProjectionV1 {
    clutch_structured_claim_adapter::runtime_contract::PositionProjectionV1 {
        market: position.market_instance_id().bytes(),
        owner: position.owner().bytes(),
        generation: position.generation(),
        replay_sequence: replay.header().next_sequence(),
        cash_atoms: position.cash_atoms(),
        reserved_cash_atoms: position.reserved_cash_atoms(),
        internal: position.native_eggs(),
        closed: position.lifecycle() != PositionLifecycleV3::Open,
    }
}

fn collateral_view<'a>(account: &AccountInfo<'_>, data: &'a [u8]) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: clutch_collateral_adapter_v2::Id::from_bytes(account.key.to_bytes()),
        owner_program: clutch_collateral_adapter_v2::Id::from_bytes(account.owner.to_bytes()),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

fn authenticate_full_vector_collateral_observations(
    accounts: &[AccountInfo<'_>],
    bound: clutch_collateral_adapter_v2::BoundCollateralProfileV2,
    plan: clutch_structured_claim_adapter::CurrentStructuredTransitionPlanV1,
) -> Outcome<()> {
    let mint_data = accounts[IX_COLLATERAL_MINT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_token_data = accounts[IX_HOARD_TOKEN]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let _mint = admit_collateral_mint_v2(
        bound,
        collateral_view(&accounts[IX_COLLATERAL_MINT], &mint_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let hoard_token = admit_collateral_account_v2(
        bound,
        collateral_view(&accounts[IX_HOARD_TOKEN], &hoard_token_data),
        TokenAccountRoleV2::Hoard,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let required_before = plan
        .hoard_after
        .cash_liability_atoms
        .checked_add(plan.hoard_after.locked_claim_principal_atoms)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        hoard_token.address.bytes() == plan.hoard_after.token_account.bytes()
            && hoard_token.amount_atoms >= required_before,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_structured_compaction_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV2,
    bound: BoundCollateralProfileV2,
    hoard_before: clutch_collateral_adapter_v2::HoardV2,
    claim_ledger_before: clutch_collateral_adapter_v2::ClaimLedgerV3,
    collateral_value_receipt_id: CollateralId,
    mut plan: clutch_structured_claim_adapter::CurrentStructuredTransitionPlanV1,
) -> Outcome<AuthenticatedStructuredCompactionV1> {
    require(
        plan.action == StructuredClaimActionV1::CompactDonation
            && descriptor.state == clutch_structured_claim_adapter::runtime_contract::DescriptorStateV1::Active
            && accounts[CX_STRUCTURED_ROOT].owner == program_id
            && !accounts[CX_STRUCTURED_ROOT].is_signer
            && !accounts[CX_STRUCTURED_ROOT].is_writable
            && !accounts[CX_STRUCTURED_ROOT].executable
            && accounts[CX_STRUCTURED_ROOT].data_len() == STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
        ClutchError::MismatchedState,
    )?;
    let root_data = accounts[CX_STRUCTURED_ROOT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let root = StructuredMarketRootV1::decode(&root_data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    let root_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&root_data[..]]).to_bytes());
    drop(root_data);
    let root_id = root
        .binding
        .id(&RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_pda = Pubkey::find_program_address(
        &[STRUCTURED_ROOT_SEED_V1, &descriptor.structured_root_id],
        program_id,
    );
    let root_accounted_lamports = root
        .rent_principal_lamports
        .checked_add(root.current_donation_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let observed_root_donation_lamports = accounts[CX_STRUCTURED_ROOT]
        .lamports()
        .checked_sub(root.rent_principal_lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        root_id.bytes() == descriptor.structured_root_id
            && *accounts[CX_STRUCTURED_ROOT].key == root_pda.0
            && root.root_bump == root_pda.1
            && accounts[CX_STRUCTURED_ROOT].lamports() >= root_accounted_lamports
            && observed_root_donation_lamports >= root.donation_floor_lamports
            && root.live_descriptor_count != 0
            && root.binding.link_account == accounts[CX_SERIES_LINK].key.to_bytes()
            && root.binding.market_instance_id.bytes() == bound.market().market.bytes(),
        ClutchError::MismatchedState,
    )?;

    let mut link_output = Box::new(SeriesMarketLinkAccountV1::decode_buffer());
    let link_data = accounts[CX_SERIES_LINK]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&link_data, &mut link_output)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    drop(link_data);
    let untrusted_link_binding = link_output.state.binding();
    let link = authenticate_series_market_link_v1(
        program_id,
        &accounts[CX_SERIES_LINK],
        root.binding.series_plan_id,
        root.binding.ordinal,
        root.binding.market_instance_id,
        root.binding.generation,
        Pubkey::new_from_array(untrusted_link_binding.market_root_account_id.bytes()),
        false,
        &mut link_output,
    )?;
    let link_binding = link.state().binding();
    let link_binding_id = link_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        link.state().phase() == SeriesMarketLinkPhaseV1::Active
            && link.state().obligation_status(SeriesLinkObligationV1::Wrapper)
                == SeriesLinkObligationStatusV1::Live
            && link_binding_id == root.product_lineage.link_binding_id
            && link_binding.obligation_configuration_id.content_id()
                == root.product_lineage.wrapper_obligation_configuration_id
            && link.state().transition_sequence()
                >= root.product_lineage.last_observed_link_transition_sequence
            && link.state().obligation_admission_receipt_id(SeriesLinkObligationV1::Wrapper)
                == root.product_lineage.product_admission_receipt_id
            && link_binding.capability_profile_id == root.binding.capability_profile_id,
        ClutchError::MismatchedState,
    )?;

    let funding_terms = authenticate_product_artifact_v1::<SeriesFundingTermsV2>(
        program_id,
        &accounts[CX_FUNDING_TERMS_V2],
        link_binding.funding_terms_id.content_id(),
    )?;
    let terms = *funding_terms.value();
    let terms_id = terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        terms_id == link_binding.funding_terms_id
            && terms.series_plan_id == root.binding.series_plan_id
            && terms.collateral_mint.bytes() == bound.policy().mint.bytes()
            && terms.token_program.bytes() == bound.policy().token_program.bytes()
            && terms.token_program.bytes() == accounts[CX_COLLATERAL_TOKEN_PROGRAM].key.to_bytes()
            && terms.neutral_collateral_disposition_token_account.bytes()
                == accounts[CX_NEUTRAL_TOKEN].key.to_bytes()
            && accounts[CX_NEUTRAL_TOKEN].key != accounts[CX_HOARD_TOKEN].key,
        ClutchError::MismatchedState,
    )?;

    let market_bytes = bound.market().market.bytes();
    let expected_hoard_authority = seeds::hoard_authority_v2_pda(program_id, &market_bytes);
    require(
        *accounts[CX_HOARD_AUTHORITY].key == expected_hoard_authority.0
            && accounts[CX_HOARD_AUTHORITY].key.to_bytes() == hoard_before.authority.bytes()
            && accounts[CX_HOARD_AUTHORITY].owner == &SYSTEM_PROGRAM_ID
            && accounts[CX_HOARD_AUTHORITY].data_is_empty()
            && !accounts[CX_HOARD_AUTHORITY].is_signer
            && !accounts[CX_HOARD_AUTHORITY].is_writable
            && !accounts[CX_HOARD_AUTHORITY].executable,
        ClutchError::WrongPda,
    )?;

    let product_authority_receipt = solana_sha256_hasher::hashv(&[
        STRUCTURED_COMPACTION_PRODUCT_AUTHORITY_DOMAIN_V1,
        &plan.transition_id,
        &collateral_value_receipt_id.bytes(),
        accounts[CX_STRUCTURED_ROOT].key.as_ref(),
        &root_id.bytes(),
        &root_data_id.bytes(),
        &root.transition_sequence.to_le_bytes(),
        &accounts[CX_STRUCTURED_ROOT].lamports().to_le_bytes(),
        &observed_root_donation_lamports.to_le_bytes(),
        accounts[CX_SERIES_LINK].key.as_ref(),
        &link.authentication_id().bytes(),
        &link_semantic_id.bytes(),
        &link_binding_id.bytes(),
        &link_binding.obligation_configuration_id.bytes(),
        &link.state().transition_sequence().to_le_bytes(),
        &root.product_lineage.product_admission_receipt_id.bytes(),
        accounts[CX_FUNDING_TERMS_V2].key.as_ref(),
        &terms_id.bytes(),
        accounts[CX_NEUTRAL_TOKEN].key.as_ref(),
    ])
    .to_bytes();
    require(
        product_authority_receipt != [0; 32] && product_authority_receipt != plan.transition_id,
        ClutchError::MismatchedState,
    )?;
    plan.transition_id = product_authority_receipt;
    Ok(AuthenticatedStructuredCompactionV1 {
        plan,
        bound,
        hoard_before,
        claim_ledger_before,
        destination_token: CollateralId::from_bytes(accounts[CX_NEUTRAL_TOKEN].key.to_bytes()),
        destination_semantic_owner: CollateralId::from_bytes(terms_id.bytes()),
        collateral_value_receipt_id,
    })
}

fn execute_structured_compaction_disposition(
    accounts: &[AccountInfo<'_>],
    capability: AuthenticatedStructuredCompactionV1,
) -> Outcome<clutch_collateral_adapter_v2::AcceptedHoardSurplusDispositionV1> {
    let mint_data = accounts[CX_COLLATERAL_MINT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_data = accounts[CX_HOARD_TOKEN]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_data = accounts[CX_NEUTRAL_TOKEN]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared = prepare_hoard_surplus_disposition_v1(
        capability.bound,
        HoardSurplusDispositionRequestV1 {
            transition_id: CollateralId::from_bytes(capability.plan.transition_id),
            collateral_value_receipt_id: capability.collateral_value_receipt_id,
            destination_token_account: capability.destination_token,
            destination_semantic_owner: capability.destination_semantic_owner,
            donated_cash_atoms: capability.plan.donated_cash_atoms,
            donated_internal: capability.plan.donated_internal,
            hoard_before: capability.hoard_before,
            hoard_after: capability.plan.hoard_after,
            claim_ledger_before: capability.claim_ledger_before,
            claim_ledger_after: capability.plan.claim_ledger_after,
        },
        TransferAuthorityV2 {
            address: CollateralId::from_bytes(accounts[CX_HOARD_AUTHORITY].key.to_bytes()),
            kind: TransferAuthorityKindV2::ProgramDerived,
            is_transaction_signer: false,
            program_address_authenticated: true,
            is_writable: false,
            executable: false,
            data_is_empty: true,
        },
        collateral_view(&accounts[CX_COLLATERAL_MINT], &mint_data),
        collateral_view(&accounts[CX_HOARD_TOKEN], &hoard_data),
        collateral_view(&accounts[CX_NEUTRAL_TOKEN], &destination_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    drop((mint_data, hoard_data, destination_data));
    invoke_and_accept_structured_compaction_disposition(accounts, prepared)
}

fn cpi_account_meta_v2(value: CpiAccountMetaV2) -> AccountMeta {
    AccountMeta {
        pubkey: Pubkey::new_from_array(value.address.bytes()),
        is_signer: value.signer,
        is_writable: value.writable,
    }
}

fn invoke_and_accept_structured_compaction_disposition(
    accounts: &[AccountInfo<'_>],
    prepared: PreparedHoardSurplusDispositionV1,
) -> Outcome<clutch_collateral_adapter_v2::AcceptedHoardSurplusDispositionV1> {
    if let Some(cpi) = prepared.cpi() {
        require(
            cpi.program_signed
                && cpi.token_program
                    == CollateralId::from_bytes(accounts[CX_COLLATERAL_TOKEN_PROGRAM].key.to_bytes())
                && cpi.accounts[0].address
                    == CollateralId::from_bytes(accounts[CX_HOARD_TOKEN].key.to_bytes())
                && cpi.accounts[1].address
                    == CollateralId::from_bytes(accounts[CX_COLLATERAL_MINT].key.to_bytes())
                && cpi.accounts[2].address
                    == CollateralId::from_bytes(accounts[CX_NEUTRAL_TOKEN].key.to_bytes())
                && cpi.accounts[3].address
                    == CollateralId::from_bytes(accounts[CX_HOARD_AUTHORITY].key.to_bytes()),
            ClutchError::MismatchedState,
        )?;
        let instruction = Instruction::new_with_bytes(
            *accounts[CX_COLLATERAL_TOKEN_PROGRAM].key,
            &cpi.data,
            cpi.accounts.into_iter().map(cpi_account_meta_v2).collect(),
        );
        let infos = [
            accounts[CX_HOARD_TOKEN].clone(),
            accounts[CX_COLLATERAL_MINT].clone(),
            accounts[CX_NEUTRAL_TOKEN].clone(),
            accounts[CX_HOARD_AUTHORITY].clone(),
            accounts[CX_COLLATERAL_TOKEN_PROGRAM].clone(),
        ];
        let market_bytes = prepared.hoard_after().market_instance_id.bytes();
        let bump = [seeds::hoard_authority_v2_pda(
            accounts[CX_BASE_PROGRAM].key,
            &market_bytes,
        )
        .1];
        let signer: [&[u8]; 3] = [seeds::SEED_HOARD_AUTHORITY_V2, &market_bytes, &bump];
        invoke_signed(&instruction, &infos, &[&signer])
            .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
    }
    let mint_after = accounts[CX_COLLATERAL_MINT]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_after = accounts[CX_HOARD_TOKEN]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_after = accounts[CX_NEUTRAL_TOKEN]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    accept_hoard_surplus_disposition_v1(
        prepared,
        collateral_view(&accounts[CX_COLLATERAL_MINT], &mint_after),
        collateral_view(&accounts[CX_HOARD_TOKEN], &hoard_after),
        collateral_view(&accounts[CX_NEUTRAL_TOKEN], &destination_after),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

fn prepare_compaction_poststate(
    accounts: &[AccountInfo<'_>],
    descriptor: &clutch_structured_claim_adapter::BoundDescriptorV1,
    plan: clutch_structured_claim_adapter::CurrentStructuredTransitionPlanV1,
    verifier: &RuntimeStructuredPdaVerifierV1,
) -> Outcome<clutch_structured_claim_adapter::StructuredVaultPoststateV1> {
    let position_data = accounts[CX_VAULT_POSITION]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let replay_data = accounts[CX_VAULT_REPLAY]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let hoard_data = accounts[CX_HOARD_V2]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let claim_data = accounts[CX_CLAIM_LEDGER_V3]
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let position = RawAccountV1 {
        role: AccountRoleV1::SourcePositionV3,
        key: accounts[CX_VAULT_POSITION].key.to_bytes(),
        owner: accounts[CX_VAULT_POSITION].owner.to_bytes(),
        lamports: accounts[CX_VAULT_POSITION].lamports(),
        data: &position_data,
        signer: false,
        writable: true,
        executable: false,
    };
    let replay = RawAccountV1 {
        role: AccountRoleV1::SourceReplayV3,
        key: accounts[CX_VAULT_REPLAY].key.to_bytes(),
        owner: accounts[CX_VAULT_REPLAY].owner.to_bytes(),
        lamports: accounts[CX_VAULT_REPLAY].lamports(),
        data: &replay_data,
        signer: false,
        writable: true,
        executable: false,
    };
    let hoard = RawAccountV1 {
        role: AccountRoleV1::HoardV2,
        key: accounts[CX_HOARD_V2].key.to_bytes(),
        owner: accounts[CX_HOARD_V2].owner.to_bytes(),
        lamports: accounts[CX_HOARD_V2].lamports(),
        data: &hoard_data,
        signer: false,
        writable: true,
        executable: false,
    };
    let claim = RawAccountV1 {
        role: AccountRoleV1::ClaimLedgerV3,
        key: accounts[CX_CLAIM_LEDGER_V3].key.to_bytes(),
        owner: accounts[CX_CLAIM_LEDGER_V3].owner.to_bytes(),
        lamports: accounts[CX_CLAIM_LEDGER_V3].lamports(),
        data: &claim_data,
        signer: false,
        writable: true,
        executable: false,
    };
    prepare_current_structured_vault_poststate_v1(
        &position, &replay, &hoard, &claim, descriptor, plan, verifier,
    )
    .map_err(map_adapter_error)
}

fn write_compaction_poststate(
    accounts: &[AccountInfo<'_>],
    poststate: clutch_structured_claim_adapter::StructuredVaultPoststateV1,
    plan: clutch_structured_claim_adapter::CurrentStructuredTransitionPlanV1,
) -> Outcome<()> {
    require(
        poststate.vault_position.address == accounts[CX_VAULT_POSITION].key.to_bytes()
            && poststate.vault_replay.address == accounts[CX_VAULT_REPLAY].key.to_bytes()
            && plan.action == StructuredClaimActionV1::CompactDonation
            && plan.user_after.is_none(),
        ClutchError::MismatchedState,
    )?;
    let mut position = accounts[CX_VAULT_POSITION]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut replay = accounts[CX_VAULT_REPLAY]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut hoard = accounts[CX_HOARD_V2]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut claim_ledger = accounts[CX_CLAIM_LEDGER_V3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        position.len() == poststate.vault_position.body.len()
            && replay.len() == usize::from(poststate.vault_replay.body_len)
            && hoard.len() == clutch_collateral_adapter_v2::HOARD_V2_BYTES
            && claim_ledger.len() == clutch_collateral_adapter_v2::CLAIM_LEDGER_V3_BYTES,
        ClutchError::WrongDataLength,
    )?;
    position.copy_from_slice(&poststate.vault_position.body);
    replay.copy_from_slice(
        &poststate.vault_replay.body[..usize::from(poststate.vault_replay.body_len)],
    );
    plan.hoard_after
        .encode(&mut hoard)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.claim_ledger_after
        .encode(&mut claim_ledger)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn write_full_vector_poststate(
    accounts: &[AccountInfo<'_>],
    poststate: clutch_structured_claim_adapter::StructuredCustodyPoststateV1,
    plan: clutch_structured_claim_adapter::CurrentStructuredTransitionPlanV1,
) -> Outcome<()> {
    require(
        poststate.source_position.address == accounts[IX_SOURCE_POSITION].key.to_bytes()
            && poststate.source_replay.address == accounts[IX_SOURCE_REPLAY].key.to_bytes()
            && poststate.destination_position.address
                == accounts[IX_DESTINATION_POSITION].key.to_bytes()
            && poststate.destination_replay.address
                == accounts[IX_DESTINATION_REPLAY].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
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
    let mut hoard = accounts[IX_HOARD_V2]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut claim_ledger = accounts[IX_CLAIM_LEDGER_V3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        source_position.len() == poststate.source_position.body.len()
            && source_replay.len() == usize::from(poststate.source_replay.body_len)
            && destination_position.len() == poststate.destination_position.body.len()
            && destination_replay.len()
                == usize::from(poststate.destination_replay.body_len)
            && hoard.len() == clutch_collateral_adapter_v2::HOARD_V2_BYTES
            && claim_ledger.len() == clutch_collateral_adapter_v2::CLAIM_LEDGER_V3_BYTES,
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
    plan.hoard_after
        .encode(&mut hoard)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    plan.claim_ledger_after
        .encode(&mut claim_ledger)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
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
) -> Outcome<AuthenticatedStructuredDeploymentsV2> {
    authenticate_structured_release_set_v2(
        accounts[CV_BASE_PROGRAM].key,
        descriptor,
        [
            (&accounts[CV_WRAPPER_PROGRAM], &accounts[CV_WRAPPER_PROGRAM_DATA]),
            (&accounts[CV_BASE_PROGRAM], &accounts[CV_BASE_PROGRAM_DATA]),
            (&accounts[CV_TOKEN_PROGRAM], &accounts[CV_TOKEN_PROGRAM_DATA]),
        ],
        [
            &accounts[CV_WRAPPER_RELEASE_V2],
            &accounts[CV_REGISTRY_RELEASE_V2],
            &accounts[CV_TOKEN_RELEASE_V2],
        ],
    )
}

fn id(bytes: [u8; 32]) -> Outcome<Identity32V1> {
    Identity32V1::new(bytes).map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn found_structured_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    liabilities: GeneralMarketLiabilityAuthorityV2,
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
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredClaimDescriptorV2,
    release_indices: [usize; 3],
) -> Outcome<AuthenticatedStructuredDeploymentsV2> {
    authenticate_structured_release_set_v2(
        program_id,
        descriptor,
        [
            (&accounts[IX_WRAPPER_PROGRAM], &accounts[IX_WRAPPER_PROGRAM_DATA]),
            (&accounts[IX_BASE_PROGRAM], &accounts[IX_BASE_PROGRAM_DATA]),
            (&accounts[IX_TOKEN_2022_PROGRAM], &accounts[IX_TOKEN_2022_PROGRAM_DATA]),
        ],
        [
            &accounts[release_indices[0]],
            &accounts[release_indices[1]],
            &accounts[release_indices[2]],
        ],
    )
}

fn authenticate_structured_release_set_v2(
    artifact_owner: &Pubkey,
    descriptor: StructuredClaimDescriptorV2,
    programs: [(&AccountInfo<'_>, &AccountInfo<'_>); 3],
    release_artifacts: [&AccountInfo<'_>; 3],
) -> Outcome<AuthenticatedStructuredDeploymentsV2> {
    require(
        STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1 == crate::capabilities::PROFILE_ID,
        ClutchError::AuthorizationUnavailable,
    )?;
    let manifests = [
        ContentId::from_bytes(STRUCTURED_WRAPPER_CAPABILITY_MANIFEST_ID_V1),
        ContentId::from_bytes(STRUCTURED_BASE_CAPABILITY_MANIFEST_ID_V1),
        ContentId::from_bytes(STRUCTURED_TOKEN_2022_CAPABILITY_MANIFEST_ID_V1),
    ];
    let wrapper = authenticate_structured_program_release_v2(
        artifact_owner,
        programs[0].0,
        programs[0].1,
        release_artifacts[0],
        manifests[0],
    )?;
    let base = authenticate_structured_program_release_v2(
        artifact_owner,
        programs[1].0,
        programs[1].1,
        release_artifacts[1],
        manifests[1],
    )?;
    let token = authenticate_structured_program_release_v2(
        artifact_owner,
        programs[2].0,
        programs[2].1,
        release_artifacts[2],
        manifests[2],
    )?;
    require(
        descriptor.wrapper_program_data == programs[0].1.key.to_bytes()
            && descriptor.wrapper_deployment_slot == wrapper.1.deployment_slot
            && descriptor.base_program == programs[1].0.key.to_bytes()
            && descriptor.base_program_data == programs[1].1.key.to_bytes()
            && descriptor.base_deployment_slot == base.1.deployment_slot
            && descriptor.token_2022_program == programs[2].0.key.to_bytes()
            && descriptor.token_2022_program_data == programs[2].1.key.to_bytes()
            && descriptor.token_2022_deployment_slot == token.1.deployment_slot,
        ClutchError::AuthorizationUnavailable,
    )?;
    let binding = DeploymentBinding {
        wrapper_program: programs[0].0.key.to_bytes(),
        wrapper_program_data: programs[0].1.key.to_bytes(),
        wrapper_deployment_slot: wrapper.1.deployment_slot,
        base_program: programs[1].0.key.to_bytes(),
        base_program_data: programs[1].1.key.to_bytes(),
        base_deployment_slot: base.1.deployment_slot,
        token_2022_program: programs[2].0.key.to_bytes(),
        token_2022_program_data: programs[2].1.key.to_bytes(),
        token_2022_deployment_slot: token.1.deployment_slot,
    };
    let owner_release_id = structured_owner_release_id_v2(
        binding,
        wrapper.0,
        base.0,
        token.0,
        &RuntimeSha256,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    Ok(AuthenticatedStructuredDeploymentsV2 {
        runtime: RuntimeDeploymentsV1 {
            binding,
            upgradeable_loader: UPGRADEABLE_LOADER_ID,
            program_owners: [UPGRADEABLE_LOADER_ID; 3],
            program_data_owners: [UPGRADEABLE_LOADER_ID; 3],
            linked_program_data: [
                programs[0].1.key.to_bytes(),
                programs[1].1.key.to_bytes(),
                programs[2].1.key.to_bytes(),
            ],
            executable_mask: 0b111,
        },
        wrapper_release_id: wrapper.0,
        base_release_id: base.0,
        token_release_id: token.0,
        owner_release_id,
    })
}

fn authenticate_structured_program_release_v2(
    artifact_owner: &Pubkey,
    program: &AccountInfo<'_>,
    program_data: &AccountInfo<'_>,
    release_artifact: &AccountInfo<'_>,
    expected_manifest_id: ContentId,
) -> Outcome<(ContentId, RegistryProgramReleaseV2)> {
    require(
        program.key != program_data.key
            && program.key != release_artifact.key
            && program_data.key != release_artifact.key
            && !program.is_signer
            && !program.is_writable
            && program.executable
            && !program_data.is_signer
            && !program_data.is_writable
            && !program_data.executable,
        ClutchError::MismatchedState,
    )?;
    let release_data = release_artifact
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let untrusted_release = RegistryProgramReleaseV2::decode(&release_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(release_data);
    let release_id = untrusted_release
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let authenticated = authenticate_product_artifact_v1::<RegistryProgramReleaseV2>(
        artifact_owner,
        release_artifact,
        release_id,
    )?;
    let release = *authenticated.value();
    let program_body = program
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let program_data_body = program_data
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let program_view = LoaderAccountViewV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        &program_body,
    );
    let programdata_view = LoaderAccountViewV1::new(
        program_data.key.to_bytes(),
        program_data.owner.to_bytes(),
        program_data.executable,
        &program_data_body,
    );
    let deployment_slot = match release.locus {
        RegistryReleaseLocusV2::SynthesizedGenesisZero => {
            decode_synthesized_genesis_loader_pair_v1(program_view, programdata_view)
                .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
                .deployment_slot
        }
        RegistryReleaseLocusV2::ObservedPositive => decode_loader_pair_v1(
            program_view,
            programdata_view,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?
        .state
        .deployment_slot,
    };
    require(
        authenticated.semantic_id() == release_id
            && release.program.bytes() == program.key.to_bytes()
            && release.programdata.bytes() == program_data.key.to_bytes()
            && release.programdata_sha256.bytes()
                == solana_sha256_hasher::hashv(&[&program_data_body]).to_bytes()
            && release.capability_manifest_id == expected_manifest_id
            && release.deployment_slot == deployment_slot
            && (matches!(
                (release.locus, deployment_slot),
                (RegistryReleaseLocusV2::SynthesizedGenesisZero, 0)
            ) || matches!(
                (release.locus, deployment_slot),
                (RegistryReleaseLocusV2::ObservedPositive, slot) if slot != 0
            )),
        ClutchError::AuthorizationUnavailable,
    )?;
    Ok((release_id, release))
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
    fn staged_profile_refuses_every_structured_action() {
        assert!(!crate::capabilities::extension_intent_action_enabled(74, 1, 35));
        for action in 1..=8 {
            assert!(!crate::capabilities::extension_intent_action_enabled(
                75, 1, action
            ));
        }
    }

    #[test]
    fn structured_root_registry_and_owner_codec_are_exactly_identical() {
        assert_eq!(
            clutch_solana_layout::registry::STRUCTURED_MARKET_ROOT_ACCOUNT_TAG,
            clutch_structured_claim_adapter::runtime_contract::STRUCTURED_MARKET_ROOT_ACCOUNT_TAG,
        );
        assert_eq!(
            clutch_solana_layout::registry::STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
            STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES,
        );
        assert_eq!(STRUCTURED_MARKET_ROOT_ACCOUNT_BYTES, 656);
    }

    #[test]
    fn descriptor_retirement_frame_keeps_product_link_read_only_until_final() {
        assert_eq!(STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT, 31);
        assert!(!structured_retirement_account_writable(
            RT_SERIES_LINK,
            false,
        ));
        assert!(structured_retirement_account_writable(
            RT_SERIES_LINK,
            true,
        ));
        assert!(!structured_retirement_account_writable(
            RT_MINT_AUTHORITY,
            true,
        ));
    }

    #[test]
    fn product_link_is_writable_only_for_an_empty_system_root() {
        assert!(structured_root_requires_product_write_v1(
            &SYSTEM_PROGRAM_ID,
            0,
        ));
        assert!(!structured_root_requires_product_write_v1(
            &SYSTEM_PROGRAM_ID,
            1,
        ));
        assert!(!structured_root_requires_product_write_v1(
            &Pubkey::new_from_array([91; 32]),
            0,
        ));
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
