//! Chain-derived unsigned foundation plans.
//!
//! These builders consume caller-supplied observations; they perform no RPC,
//! key access, signing, submission, or account mutation. Every semantic content
//! identity is recomputed from a hostile-decoded canonical record.

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingAssetClassV1, FundingQuoteV1, MARKET_OPENING_READINESS_BYTES,
    MARKET_OPENING_READINESS_PDA_DOMAIN, MarketOpeningReadinessV1, RequiredFoundingEntryV1,
};
use dclutch_collateral_contract::{
    COLLATERAL_CUSTODY_BYTES, COLLATERAL_CUSTODY_PDA_DOMAIN, COLLATERAL_VAULT_PDA_DOMAIN,
    CREATE_REALM_BYTES, CreateRealmV1, FOUND_MARKET_AND_FUND_BYTES, FoundMarketAndFundV1,
    OPEN_COLLATERAL_VAULT_BYTES, OpenCollateralVaultV1,
    frame::{
        AccountRole, CREATE_REALM_FRAME, FOUND_MARKET_AND_FUND_FRAME, OPEN_COLLATERAL_VAULT_FRAME,
        Role,
    },
};
use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity, MarketRoot};
use dclutch_market_contract::market::{
    CategoricalMarketV1, CategoricalSettlementSummaryV1, decode_market_outcome_count,
};
use dclutch_product_contract::{
    ContentId as ProductContentId, capacity::CapacityProfileV1, claim::CategoricalUnitV1,
    product::InstanceV1, result_domain::FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
};
use dclutch_pyth_contract::funding::FUNDING_BYTES;
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, REALM_PDA_DOMAIN, RealmV1,
    RealmV1Input,
};
use dclutch_record_contract::{
    ContentDigest, RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1, SchemaReleaseId,
};
use dclutch_source_contract::{SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SourceMaterialViewV1};
use dclutch_token_svm::{ACCOUNT_BYTES, CollateralAdapterReleaseV1, PRODUCTION_ADAPTER_RELEASES};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar::{SysvarSerialize, rent::Rent},
};
use solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable, native_loader, system_program, sysvar};

use crate::{Finality, MARKET_SEED, Observation, ObservedAccount, authenticate_rent_credit};

mod creation;

pub use creation::*;

pub(crate) const REALM_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] = b"dclutch/schema/realm-v1";
pub(crate) const PRODUCT_INSTANCE_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/product-instance-v1";
pub(crate) const CATEGORICAL_CLAIM_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/categorical-unit-claim-v1";
pub(crate) const PRODUCT_CAPACITY_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/product-capacity-profile-v1";
pub(crate) const CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/capability-manifest-profile-1-v1";

/// Initial Market generation created by this foundation workflow.
///
/// Successor/reopen workflows must derive their next generation from terminal
/// chain state and are deliberately outside this initial-foundation builder.
pub const FOUNDATION_GENERATION: u64 = 0;
/// Exact account count of [`build_create_realm_v1`].
pub const CREATE_REALM_ACCOUNT_COUNT: usize = CREATE_REALM_FRAME.len();
/// Exact account count of [`build_found_market_and_fund_v1`].
pub const FOUND_MARKET_ACCOUNT_COUNT: usize = FOUND_MARKET_AND_FUND_FRAME.len();
/// Exact account count of [`build_open_collateral_vault_v1`].
pub const OPEN_COLLATERAL_VAULT_ACCOUNT_COUNT: usize = OPEN_COLLATERAL_VAULT_FRAME.len();

/// One finalized observation that a derived destination did not exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedVacancy {
    /// Exact address checked for absence.
    pub key: Pubkey,
    /// Observation at which absence was reported.
    pub observation: Observation,
}

/// Chain-observed finalization proof paired with one immutable raw record.
///
/// The schema/release identifier and content digest derive both the raw record
/// and its now-vacant staging cursor.  A builder never treats a decoded record
/// at an arbitrary program-owned address as finalized evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedRecordProof {
    /// Schema/release identity used in the raw and cursor PDA derivations.
    pub schema_release_id: [u8; 32],
    /// Full finalized observation of the paired, vacant staging cursor.
    pub staging_cursor: ObservedAccount,
}

/// Same-observation records needed to create an immutable Realm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRealmState {
    /// System-owned signing sponsor and rent payer.
    pub sponsor: ObservedAccount,
    /// Proved-vacant derived Realm address.
    pub realm_destination: ObservedVacancy,
    /// Exact collateral Mint state.
    pub collateral_mint: ObservedAccount,
    /// Executable token program owning the Mint.
    pub token_program: ObservedAccount,
    /// Canonical executable System Program account.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar account.
    pub rent_sysvar: ObservedAccount,
}

/// Explicit caller-selected Realm authority-risk policy.
///
/// This is semantic risk consent only: it contains no address, content ID, or
/// account meta. [`Self::STRICT`] is the conservative default choice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmAuthorityPolicy {
    /// Whether continuing mint authority is admitted.
    pub mint_authority: MintAuthorityPolicy,
    /// Whether continuing freeze authority is admitted.
    pub freeze_authority: FreezeAuthorityPolicy,
}

impl RealmAuthorityPolicy {
    /// Require both issuer authorities to be absent in the observed Mint.
    pub const STRICT: Self = Self {
        mint_authority: MintAuthorityPolicy::RequireAbsent,
        freeze_authority: FreezeAuthorityPolicy::RequireAbsent,
    };
}

impl Default for RealmAuthorityPolicy {
    fn default() -> Self {
        Self::STRICT
    }
}

/// Exact observed issuer-control risk and affirmative selected policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmAuthorityReport {
    /// Caller-selected authority-risk policy embedded in the Realm.
    pub selected_policy: RealmAuthorityPolicy,
    /// Exact observed Mint authority, when present.
    pub observed_mint_authority: Option<[u8; 32]>,
    /// Exact observed freeze authority, when present.
    pub observed_freeze_authority: Option<[u8; 32]>,
}

/// Same-observation records needed to atomically found a Market and Fund.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundMarketState {
    /// System-owned signing sponsor and rent payer.
    pub sponsor: ObservedAccount,
    /// Proved-vacant derived Market address.
    pub market_destination: ObservedVacancy,
    /// Proved-vacant derived generic capability-FundingState address.
    pub fund_destination: ObservedVacancy,
    /// Pre-existing permanent RentCredit bound to the Market beneficiary.
    pub rent_credit: ObservedAccount,
    /// Canonical immutable Realm record.
    pub realm: ObservedAccount,
    /// Canonical finalized-record proof for the Realm raw bytes.
    pub realm_finalization: FinalizedRecordProof,
    /// Canonical occurrence-specific Product Instance record.
    pub product_instance: ObservedAccount,
    /// Canonical finalized-record proof for the Product Instance raw bytes.
    pub product_instance_finalization: FinalizedRecordProof,
    /// Canonical categorical ClaimBasis record.
    pub claim_basis: ObservedAccount,
    /// Canonical finalized-record proof for the ClaimBasis raw bytes.
    pub claim_basis_finalization: FinalizedRecordProof,
    /// Canonical Product capacity profile record.
    pub capacity_profile: ObservedAccount,
    /// Canonical finalized-record proof for the CapacityProfile raw bytes.
    pub capacity_profile_finalization: FinalizedRecordProof,
    /// Canonical provider-neutral SourceMaterial record.
    pub resolution_material: ObservedAccount,
    /// Canonical finalized-record proof for the resolution-material raw bytes.
    pub resolution_material_finalization: FinalizedRecordProof,
    /// Canonical capability manifest record.
    pub capability_manifest: ObservedAccount,
    /// Canonical finalized-record proof for the capability-manifest raw bytes.
    pub capability_manifest_finalization: FinalizedRecordProof,
    /// Canonical executable System Program account.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar account.
    pub rent_sysvar: ObservedAccount,
}

/// Same-observation records needed to create custody and open a founded Market.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCollateralVaultState {
    /// System-owned signer paying only the new custody and Vault rent.
    pub sponsor: ObservedAccount,
    /// Authenticated Founding Market.
    pub market: ObservedAccount,
    /// Ready direct Market child consumed and closed during Open.
    pub readiness: ObservedAccount,
    /// Pre-existing permanent credit bound to the immutable Market beneficiary.
    pub rent_credit: ObservedAccount,
    /// Finalized capability manifest raw record committed by the Market.
    pub capability_manifest: ObservedAccount,
    /// Raw-record and vacant-cursor proof for the manifest.
    pub capability_manifest_finalization: FinalizedRecordProof,
    /// Finalized Realm raw record committed by the Market.
    pub realm: ObservedAccount,
    /// Raw-record and vacant-cursor proof for the Realm.
    pub realm_finalization: FinalizedRecordProof,
    /// System-owned empty derived custody destination.
    pub custody_destination: ObservedAccount,
    /// System-owned empty derived token-Vault destination.
    pub vault_destination: ObservedAccount,
    /// Mint bound by the Realm.
    pub collateral_mint: ObservedAccount,
    /// Executable token program bound by the Realm.
    pub token_program: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Exact signer, rent, prepayment, and total sponsor debit report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundationDebitReport {
    /// The one required transaction signer and debit authority.
    pub sponsor: Pubkey,
    /// Rent principal for an immutable Realm, or zero for Market founding.
    pub realm_rent: u64,
    /// Rent principal for a Market, or zero for Realm creation.
    pub market_rent: u64,
    /// Rent principal retained by a resolution Fund.
    pub fund_rent: u64,
    /// Segregated present provider reimbursement.
    pub provider_fee_reimbursement: u64,
    /// Segregated present resolution bounty.
    pub resolution_success_bounty: u64,
    /// Exact sponsor debit if the unsigned instruction succeeds.
    pub total_sponsor_debit: u64,
}

/// Complete unsigned Realm-creation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRealmReport {
    /// Exact unsigned instruction with canonical ordered privileges.
    pub instruction: Instruction,
    /// One finalized observation shared by every input.
    pub observation: Observation,
    /// Canonical derived Realm record embedded in the instruction.
    pub realm: RealmV1,
    /// SHA-256 content identity of the canonical Realm record.
    pub realm_content_id: [u8; 32],
    /// Canonical Realm PDA derived from record content and program ID.
    pub realm_address: Pubkey,
    /// Exact observed issuer-control risk and selected policy.
    pub authority: RealmAuthorityReport,
    /// Exact signer and debit report.
    pub debit: FoundationDebitReport,
}

/// Complete unsigned Market-and-Fund founding result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundMarketReport {
    /// Exact unsigned instruction with canonical ordered privileges.
    pub instruction: Instruction,
    /// One finalized observation shared by every input.
    pub observation: Observation,
    /// Market identity rebuilt only from authenticated record content.
    pub identity: MarketIdentity,
    /// Canonical Market PDA.
    pub market_address: Pubkey,
    /// Canonical generic capability-FundingState PDA.
    pub fund_address: Pubkey,
    /// Exact exhaustive categorical outcome count.
    pub outcome_count: u8,
    /// Unique manifest entry authorizing and funding the resolution adapter.
    ///
    /// This canonical value identifies its manifest index, release, config,
    /// capacity, child schema/derivation, activation policy, and exact funding
    /// quote without creating a second operator-owned semantic representation.
    pub resolution_funding: RequiredFoundingEntryV1,
    /// Exact signer and debit report.
    pub debit: FoundationDebitReport,
}

/// Complete unsigned custody creation and Market-open result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCollateralVaultReport {
    /// Exact unsigned 14-account Open instruction.
    pub instruction: Instruction,
    /// One finalized observation shared by every state input.
    pub observation: Observation,
    /// Exact immutable Market generation.
    pub generation: u64,
    /// Required pre-open direct-child replay count.
    pub child_count: u64,
    /// Derived custody PDA to be created.
    pub custody_address: Pubkey,
    /// Derived token Vault PDA to be created.
    pub vault_address: Pubkey,
    /// Exact signer and debit report.
    pub debit: FoundationDebitReport,
}

/// Refusal from a chain-derived foundation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoundationError {
    /// At least one record was not observed at finalized commitment.
    ObservationNotFinalized,
    /// Records or vacancy proofs came from different observations.
    ObservationMismatch,
    /// A protocol PDA or canonical program/sysvar key differed.
    AddressMismatch,
    /// An account owner or executable bit was incompatible with its role.
    InvalidOwner,
    /// A system sponsor was executable or carried state data.
    InvalidSponsor,
    /// A destination was not the expected proved-vacant PDA.
    DestinationNotVacant,
    /// Rent sysvar bytes or identity were invalid.
    InvalidRent,
    /// An existing immutable or Mint account was not rent exempt.
    AccountNotRentExempt,
    /// The token program was not one exact supported production profile.
    UnsupportedTokenProgram,
    /// The collateral Mint failed exact hostile decoding.
    InvalidMint,
    /// A present Mint or freeze authority lacked affirmative issuer-risk consent.
    IssuerAuthorityConsentRequired,
    /// A Realm or immutable Product/Capability/Source record did not decode.
    InvalidRecord,
    /// Canonical decoded bytes differed from observed bytes.
    NonCanonicalRecord,
    /// A content digest or cross-record semantic link differed.
    ContentLinkMismatch,
    /// Outcome width could not be represented by the categorical V1 path.
    InvalidOutcomeCount,
    /// The unique required-at-founding resolution entry or quote was invalid.
    InvalidFundingAuthority,
    /// Exact rent, funding, or space arithmetic overflowed.
    ArithmeticOverflow,
    /// Sponsor principal could not cover the exact present debit.
    SponsorUnderfunded,
    /// Two ordered account roles aliased the same address.
    AccountAlias,
    /// Exact collateral instruction construction unexpectedly failed.
    InstructionEncoding,
}

/// Construct an unsigned `CreateRealmV1` only from one finalized observation.
///
/// The exact token program and Mint select the raw collateral atom. The adapter
/// release identity is selected from the compiled production catalog and
/// hashed locally. `authority_policy` is the only caller-selected semantic
/// input. The strict default requires both authorities absent; a present
/// authority is admitted only by an affirmative `AdmitIssuerControl` choice
/// and is returned verbatim in the report.
pub fn build_create_realm_v1(
    program_id: Pubkey,
    state: &CreateRealmState,
    authority_policy: RealmAuthorityPolicy,
) -> Result<CreateRealmReport, FoundationError> {
    let observation = require_observation(&[
        state.sponsor.observation,
        state.realm_destination.observation,
        state.collateral_mint.observation,
        state.token_program.observation,
        state.system_program.observation,
        state.rent_sysvar.observation,
    ])?;
    authenticate_system_program(&state.system_program)?;
    let rent = decode_rent(&state.rent_sysvar)?;
    authenticate_sponsor(&state.sponsor)?;
    authenticate_distinct(&[
        state.sponsor.key,
        state.realm_destination.key,
        state.collateral_mint.key,
        state.token_program.key,
        state.system_program.key,
        state.rent_sysvar.key,
    ])?;
    authenticate_token_program(&state.token_program)?;
    if state.collateral_mint.owner != state.token_program.key || state.collateral_mint.executable {
        return Err(FoundationError::InvalidOwner);
    }
    require_rent_exempt(&rent, &state.collateral_mint)?;
    let release = select_token_release(state.token_program.key)?;
    let mint = release
        .profile()
        .check_mint(
            state.token_program.key.to_bytes(),
            &state.collateral_mint.data,
        )
        .map_err(|_| FoundationError::InvalidMint)?;
    if (!mint.mint_authority.is_none()
        && authority_policy.mint_authority != MintAuthorityPolicy::AdmitIssuerControl)
        || (!mint.freeze_authority.is_none()
            && authority_policy.freeze_authority != FreezeAuthorityPolicy::AdmitIssuerControl)
    {
        return Err(FoundationError::IssuerAuthorityConsentRequired);
    }
    let observed_mint_authority = mint.mint_authority.as_ref().copied();
    let observed_freeze_authority = mint.freeze_authority.as_ref().copied();
    let realm = RealmV1::new(RealmV1Input {
        token_program: state.token_program.key.to_bytes(),
        collateral_mint: state.collateral_mint.key.to_bytes(),
        collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
        mint_authority_policy: authority_policy.mint_authority,
        freeze_authority_policy: authority_policy.freeze_authority,
    })
    .map_err(|_| FoundationError::InvalidRecord)?;
    let realm_content_id = hash(&realm.to_bytes()).to_bytes();
    let (realm_address, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_content_id], &program_id);
    authenticate_vacancy(state.realm_destination, realm_address)?;
    let realm_rent = rent.minimum_balance(REALM_BYTES);
    require_sponsor_balance(state.sponsor.lamports, realm_rent)?;
    let mut data = vec![0u8; CREATE_REALM_BYTES];
    CreateRealmV1::new(realm)
        .encode(&mut data)
        .map_err(|_| FoundationError::InstructionEncoding)?;
    let accounts = exact_create_realm_metas(state, realm_address)?;
    let instruction = Instruction {
        program_id,
        accounts,
        data,
    };
    Ok(CreateRealmReport {
        instruction,
        observation,
        realm,
        realm_content_id,
        realm_address,
        authority: RealmAuthorityReport {
            selected_policy: authority_policy,
            observed_mint_authority,
            observed_freeze_authority,
        },
        debit: FoundationDebitReport {
            sponsor: state.sponsor.key,
            realm_rent,
            market_rent: 0,
            fund_rent: 0,
            provider_fee_reimbursement: 0,
            resolution_success_bounty: 0,
            total_sponsor_debit: realm_rent,
        },
    })
}

/// Construct an unsigned `FoundMarketAndFundV1` from authenticated records.
///
/// This function accepts no Market, Realm, Product, ClaimBasis, policy,
/// capability-manifest, or capacity-profile content identity. Every one is
/// recomputed from canonical observed bytes, cross-linked, then used to derive
/// the initial-generation Market and Fund PDAs.
pub fn build_found_market_and_fund_v1(
    program_id: Pubkey,
    state: &FoundMarketState,
) -> Result<FoundMarketReport, FoundationError> {
    let observation = require_observation(&[
        state.sponsor.observation,
        state.market_destination.observation,
        state.fund_destination.observation,
        state.rent_credit.observation,
        state.realm.observation,
        state.realm_finalization.staging_cursor.observation,
        state.product_instance.observation,
        state
            .product_instance_finalization
            .staging_cursor
            .observation,
        state.claim_basis.observation,
        state.claim_basis_finalization.staging_cursor.observation,
        state.capacity_profile.observation,
        state
            .capacity_profile_finalization
            .staging_cursor
            .observation,
        state.resolution_material.observation,
        state
            .resolution_material_finalization
            .staging_cursor
            .observation,
        state.capability_manifest.observation,
        state
            .capability_manifest_finalization
            .staging_cursor
            .observation,
        state.system_program.observation,
        state.rent_sysvar.observation,
    ])?;
    authenticate_system_program(&state.system_program)?;
    let rent = decode_rent(&state.rent_sysvar)?;
    authenticate_sponsor(&state.sponsor)?;
    authenticate_distinct(&[
        state.sponsor.key,
        state.market_destination.key,
        state.fund_destination.key,
        state.rent_credit.key,
        state.realm.key,
        state.realm_finalization.staging_cursor.key,
        state.product_instance.key,
        state.product_instance_finalization.staging_cursor.key,
        state.claim_basis.key,
        state.claim_basis_finalization.staging_cursor.key,
        state.capacity_profile.key,
        state.capacity_profile_finalization.staging_cursor.key,
        state.resolution_material.key,
        state.resolution_material_finalization.staging_cursor.key,
        state.capability_manifest.key,
        state.capability_manifest_finalization.staging_cursor.key,
        state.system_program.key,
        state.rent_sysvar.key,
    ])?;
    for (record, proof, schema_release_id) in [
        (
            &state.realm,
            &state.realm_finalization,
            hash(REALM_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        ),
        (
            &state.product_instance,
            &state.product_instance_finalization,
            hash(PRODUCT_INSTANCE_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        ),
        (
            &state.claim_basis,
            &state.claim_basis_finalization,
            hash(CATEGORICAL_CLAIM_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        ),
        (
            &state.capacity_profile,
            &state.capacity_profile_finalization,
            hash(PRODUCT_CAPACITY_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        ),
        (
            &state.resolution_material,
            &state.resolution_material_finalization,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        ),
        (
            &state.capability_manifest,
            &state.capability_manifest_finalization,
            hash(CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        ),
    ] {
        if proof.schema_release_id != schema_release_id {
            return Err(FoundationError::AddressMismatch);
        }
        authenticate_finalized_record(program_id, &rent, record, proof)?;
    }

    let realm = RealmV1::decode(&state.realm.data).map_err(|_| FoundationError::InvalidRecord)?;
    require_canonical(&state.realm.data, &realm.to_bytes())?;
    let realm_id = hash(&state.realm.data).to_bytes();
    let capacity = CapacityProfileV1::decode(&state.capacity_profile.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    require_canonical(&state.capacity_profile.data, &capacity.to_bytes())?;
    let capacity_id_bytes = hash(&state.capacity_profile.data).to_bytes();
    let capacity_id = product_id(capacity_id_bytes)?;

    let claim = CategoricalUnitV1::decode(&state.claim_basis.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    require_canonical(&state.claim_basis.data, &claim.to_bytes())?;
    if claim.capacity_profile_id().content_id() != capacity_id {
        return Err(FoundationError::ContentLinkMismatch);
    }
    claim
        .validate_capacity(capacity)
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    let outcome_count =
        u8::try_from(claim.outcome_count()).map_err(|_| FoundationError::InvalidOutcomeCount)?;
    if !(2..=16).contains(&outcome_count) {
        return Err(FoundationError::InvalidOutcomeCount);
    }
    let claim_id_bytes = hash(&state.claim_basis.data).to_bytes();
    let claim_id = product_id(claim_id_bytes)?;

    let instance = InstanceV1::decode(&state.product_instance.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    require_canonical(&state.product_instance.data, &instance.to_bytes())?;
    instance
        .validate_claim_basis(claim_id, claim)
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    if instance.partition_cell_count() != u32::from(outcome_count) {
        return Err(FoundationError::InvalidOutcomeCount);
    }
    let instance_id = hash(&state.product_instance.data).to_bytes();

    let source_material = SourceMaterialViewV1::decode(&state.resolution_material.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let material_instance_id = source_material
        .product_instance_id()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let policy = source_material
        .policy()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let result_domain = source_material
        .result_domain()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let result_domain_bytes = result_domain.to_bytes();
    let result_domain_id = hashv(&[
        FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        &[0],
        result_domain_bytes.as_slice(),
    ])
    .to_bytes();
    if material_instance_id.to_bytes() != instance_id
        || policy.product_instance_id().to_bytes() != instance_id
        || policy.result_domain_id().to_bytes() != result_domain_id
        || instance.result_domain_id().to_bytes() != result_domain_id
        || result_domain.outcome_count() != outcome_count
        || instance.partition_cell_count() != u32::from(result_domain.outcome_count())
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    let (_, provider_release) = source_material
        .primary_provider_release()
        .map_err(|_| FoundationError::InvalidRecord)?;
    let material_id = hash(source_material.as_bytes()).to_bytes();

    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let manifest_id = hash(manifest.as_bytes()).to_bytes();
    let fund_rent = rent.minimum_balance(FUNDING_BYTES);
    let material_capability_id =
        CapabilityContentId::new(material_id).map_err(|_| FoundationError::ContentLinkMismatch)?;
    let resolution_funding = manifest
        .required_founding_entry_for_config(material_capability_id)
        .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    let funding_entry = resolution_funding.entry();
    if funding_entry.config_id() != material_capability_id
        || funding_entry.release_id().to_bytes() != provider_release.adapter_release_id().to_bytes()
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    let funding_quote = resolution_funding
        .validate_one_shot_resolution_fund_quote(fund_rent)
        .map_err(|_| FoundationError::InvalidFundingAuthority)?;

    let identity = MarketIdentity::new(
        core_id(realm_id)?,
        core_id(instance_id)?,
        core_id(claim_id_bytes)?,
        core_id(material_id)?,
        core_id(manifest_id)?,
        FOUNDATION_GENERATION,
    );
    let identity_id = hash(&identity.to_bytes()).to_bytes();
    let (market_address, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_id], &program_id);
    authenticate_vacancy(state.market_destination, market_address)?;
    let funding = dclutch_pyth_contract::funding::construct_required_resolution_funding(
        core_id(manifest_id).map_err(|_| FoundationError::ContentLinkMismatch)?,
        manifest,
        resolution_funding,
        fund_rent,
        observation.slot,
    )
    .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market_address.to_bytes(),
        FOUNDATION_GENERATION,
        core_id(manifest_id).map_err(|_| FoundationError::ContentLinkMismatch)?,
        manifest,
        funding,
    )
    .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    let (fund_address, _) =
        Pubkey::find_program_address(&derivation.seed_components(), &program_id);
    authenticate_vacancy(state.fund_destination, fund_address)?;

    authenticate_rent_credit(program_id, &state.rent_credit, state.sponsor.key)
        .map_err(|_| FoundationError::InvalidOwner)?;

    let mut root = MarketRoot::founding(identity, state.sponsor.key.to_bytes())
        .map_err(|_| FoundationError::InvalidRecord)?;
    root.register_child(FOUNDATION_GENERATION, 0)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let market_space = validate_market_space(outcome_count, root)?;
    let market_rent = rent.minimum_balance(market_space);
    let native_funding = resolution_native_funding(funding_quote)?;
    let total_sponsor_debit = market_rent
        .checked_add(native_funding.total_lamports)
        .ok_or(FoundationError::ArithmeticOverflow)?;
    require_sponsor_balance(state.sponsor.lamports, total_sponsor_debit)?;

    let wire = FoundMarketAndFundV1::new(identity, outcome_count)
        .map_err(|_| FoundationError::InstructionEncoding)?;
    let mut data = vec![0u8; FOUND_MARKET_AND_FUND_BYTES];
    wire.encode(&mut data)
        .map_err(|_| FoundationError::InstructionEncoding)?;
    let accounts = exact_found_market_metas(state, market_address, fund_address)?;
    let instruction = Instruction {
        program_id,
        accounts,
        data,
    };
    Ok(FoundMarketReport {
        instruction,
        observation,
        identity,
        market_address,
        fund_address,
        outcome_count,
        resolution_funding,
        debit: FoundationDebitReport {
            sponsor: state.sponsor.key,
            realm_rent: 0,
            market_rent,
            fund_rent: native_funding.rent_lamports,
            provider_fee_reimbursement: native_funding.provider_lamports,
            resolution_success_bounty: native_funding.bounty_lamports,
            total_sponsor_debit,
        },
    })
}

#[derive(Clone, Copy)]
struct ResolutionNativeFunding {
    rent_lamports: u64,
    provider_lamports: u64,
    bounty_lamports: u64,
    total_lamports: u64,
}

fn resolution_native_funding(
    quote: FundingQuoteV1,
) -> Result<ResolutionNativeFunding, FoundationError> {
    let amounts = quote.amounts();
    let provider = amounts.provider();
    if quote.realm_collateral().is_some()
        || amounts.realm_collateral_total() != 0
        || amounts.rent().asset_class() != FundingAssetClassV1::NativeLamports
        || !matches!(
            provider.asset_class(),
            FundingAssetClassV1::NotApplicable | FundingAssetClassV1::NativeLamports
        )
        || amounts.bounty().asset_class() != FundingAssetClassV1::NativeLamports
        || amounts.creation().amount() != 0
        || amounts.work().amount() != 0
        || amounts.liquidity().amount() != 0
        || amounts.service().amount() != 0
    {
        return Err(FoundationError::InvalidFundingAuthority);
    }
    let total_lamports = amounts
        .rent()
        .amount()
        .checked_add(provider.amount())
        .and_then(|value| value.checked_add(amounts.bounty().amount()))
        .ok_or(FoundationError::ArithmeticOverflow)?;
    if amounts.native_lamports_total() != total_lamports {
        return Err(FoundationError::InvalidFundingAuthority);
    }
    Ok(ResolutionNativeFunding {
        rent_lamports: amounts.rent().amount(),
        provider_lamports: provider.amount(),
        bounty_lamports: amounts.bounty().amount(),
        total_lamports,
    })
}

/// Construct an unsigned Open14 instruction from finalized chain observations.
pub fn build_open_collateral_vault_v1(
    program_id: Pubkey,
    state: &OpenCollateralVaultState,
) -> Result<OpenCollateralVaultReport, FoundationError> {
    let observation = require_observation(&[
        state.sponsor.observation,
        state.market.observation,
        state.readiness.observation,
        state.rent_credit.observation,
        state.capability_manifest.observation,
        state
            .capability_manifest_finalization
            .staging_cursor
            .observation,
        state.realm.observation,
        state.realm_finalization.staging_cursor.observation,
        state.custody_destination.observation,
        state.vault_destination.observation,
        state.collateral_mint.observation,
        state.token_program.observation,
        state.system_program.observation,
        state.rent_sysvar.observation,
    ])?;
    authenticate_system_program(&state.system_program)?;
    let rent = decode_rent(&state.rent_sysvar)?;
    authenticate_sponsor(&state.sponsor)?;
    authenticate_distinct(&[
        state.sponsor.key,
        state.market.key,
        state.readiness.key,
        state.rent_credit.key,
        state.capability_manifest.key,
        state.capability_manifest_finalization.staging_cursor.key,
        state.realm.key,
        state.realm_finalization.staging_cursor.key,
        state.custody_destination.key,
        state.vault_destination.key,
        state.collateral_mint.key,
        state.token_program.key,
        state.system_program.key,
        state.rent_sysvar.key,
    ])?;
    if state.capability_manifest_finalization.schema_release_id
        != hash(CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes()
        || state.realm_finalization.schema_release_id
            != hash(REALM_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes()
    {
        return Err(FoundationError::AddressMismatch);
    }
    authenticate_finalized_record(
        program_id,
        &rent,
        &state.capability_manifest,
        &state.capability_manifest_finalization,
    )?;
    authenticate_finalized_record(program_id, &rent, &state.realm, &state.realm_finalization)?;
    require_rent_exempt(&rent, &state.market)?;
    let manifest = CapabilityManifestV1::decode(&state.capability_manifest.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    require_canonical(&state.capability_manifest.data, manifest.as_bytes())?;
    let manifest_id = CapabilityContentId::new(hash(manifest.as_bytes()).to_bytes())
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    let realm = RealmV1::decode(&state.realm.data).map_err(|_| FoundationError::InvalidRecord)?;
    require_canonical(&state.realm.data, &realm.to_bytes())?;
    authenticate_token_program(&state.token_program)?;
    if realm.token_program() != &state.token_program.key.to_bytes()
        || realm.collateral_mint() != state.collateral_mint.key.as_ref()
        || state.collateral_mint.owner != state.token_program.key
        || state.collateral_mint.executable
    {
        return Err(FoundationError::InvalidOwner);
    }
    require_rent_exempt(&rent, &state.collateral_mint)?;
    let release = select_token_release(state.token_program.key)?;
    let mint = release
        .profile()
        .check_mint(
            state.token_program.key.to_bytes(),
            &state.collateral_mint.data,
        )
        .map_err(|_| FoundationError::InvalidMint)?;
    require_realm_authorities(
        realm,
        !mint.mint_authority.is_none(),
        !mint.freeze_authority.is_none(),
    )?;
    let (generation, child_count, beneficiary) = open_market_facts(
        program_id,
        &state.market,
        hash(&state.realm.data).to_bytes(),
        manifest_id.to_bytes(),
    )?;
    let readiness = MarketOpeningReadinessV1::decode(&state.readiness.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    if readiness.to_bytes().as_slice() != state.readiness.data.as_slice()
        || readiness.sponsor_rent_refund() != state.sponsor.key.as_ref()
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    readiness
        .require_ready_for_open(
            state.market.key.to_bytes(),
            generation,
            manifest_id,
            manifest,
        )
        .map_err(|_| FoundationError::InvalidFundingAuthority)?;
    let generation_seed = generation.to_le_bytes();
    let (expected_readiness, _) = Pubkey::find_program_address(
        &[
            MARKET_OPENING_READINESS_PDA_DOMAIN,
            state.market.key.as_ref(),
            generation_seed.as_slice(),
        ],
        &program_id,
    );
    if state.readiness.key != expected_readiness
        || state.readiness.owner != program_id
        || state.readiness.executable
        || state.readiness.lamports != rent.minimum_balance(MARKET_OPENING_READINESS_BYTES)
    {
        return Err(FoundationError::AddressMismatch);
    }
    authenticate_rent_credit(
        program_id,
        &state.rent_credit,
        Pubkey::new_from_array(beneficiary),
    )
    .map_err(|_| FoundationError::InvalidOwner)?;
    let (custody_address, _) = Pubkey::find_program_address(
        &[COLLATERAL_CUSTODY_PDA_DOMAIN, state.market.key.as_ref()],
        &program_id,
    );
    let (vault_address, _) = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, state.market.key.as_ref()],
        &program_id,
    );
    authenticate_empty_system_destination(&state.custody_destination, custody_address)?;
    authenticate_empty_system_destination(&state.vault_destination, vault_address)?;
    let custody_rent = rent.minimum_balance(COLLATERAL_CUSTODY_BYTES);
    let vault_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let total = custody_rent
        .checked_add(vault_rent)
        .ok_or(FoundationError::ArithmeticOverflow)?;
    require_sponsor_balance(state.sponsor.lamports, total)?;
    let mut data = vec![0; OPEN_COLLATERAL_VAULT_BYTES];
    OpenCollateralVaultV1::new(generation, child_count)
        .encode(&mut data)
        .map_err(|_| FoundationError::InstructionEncoding)?;
    let accounts = exact_open_collateral_vault_metas(state, custody_address, vault_address)?;
    Ok(OpenCollateralVaultReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation,
        generation,
        child_count,
        custody_address,
        vault_address,
        debit: FoundationDebitReport {
            sponsor: state.sponsor.key,
            realm_rent: 0,
            market_rent: 0,
            fund_rent: 0,
            provider_fee_reimbursement: 0,
            resolution_success_bounty: 0,
            total_sponsor_debit: total,
        },
    })
}

fn exact_create_realm_metas(
    state: &CreateRealmState,
    realm_address: Pubkey,
) -> Result<Vec<AccountMeta>, FoundationError> {
    CREATE_REALM_FRAME
        .iter()
        .map(|role| {
            let key = match role.role() {
                Role::Sponsor => state.sponsor.key,
                Role::Realm => realm_address,
                Role::CollateralMint => state.collateral_mint.key,
                Role::TokenProgram => state.token_program.key,
                Role::SystemProgram => system_program::ID,
                Role::RentSysvar => sysvar::rent::ID,
                _ => return Err(FoundationError::InstructionEncoding),
            };
            Ok(exact_meta(key, *role))
        })
        .collect()
}

fn exact_found_market_metas(
    state: &FoundMarketState,
    market_address: Pubkey,
    fund_address: Pubkey,
) -> Result<Vec<AccountMeta>, FoundationError> {
    FOUND_MARKET_AND_FUND_FRAME
        .iter()
        .map(|role| {
            let key = match role.role() {
                Role::Sponsor => state.sponsor.key,
                Role::Market => market_address,
                Role::FundingState => fund_address,
                Role::RentCredit => state.rent_credit.key,
                Role::Realm => state.realm.key,
                Role::ProductInstance => state.product_instance.key,
                Role::ClaimBasis => state.claim_basis.key,
                Role::CapacityProfile => state.capacity_profile.key,
                Role::ResolutionPolicy => state.resolution_material.key,
                Role::CapabilityManifest => state.capability_manifest.key,
                Role::RealmStagingCursor => state.realm_finalization.staging_cursor.key,
                Role::ProductInstanceStagingCursor => {
                    state.product_instance_finalization.staging_cursor.key
                }
                Role::ClaimBasisStagingCursor => state.claim_basis_finalization.staging_cursor.key,
                Role::CapacityProfileStagingCursor => {
                    state.capacity_profile_finalization.staging_cursor.key
                }
                Role::ResolutionPolicyStagingCursor => {
                    state.resolution_material_finalization.staging_cursor.key
                }
                Role::CapabilityManifestStagingCursor => {
                    state.capability_manifest_finalization.staging_cursor.key
                }
                Role::SystemProgram => system_program::ID,
                Role::RentSysvar => sysvar::rent::ID,
                _ => return Err(FoundationError::InstructionEncoding),
            };
            Ok(exact_meta(key, *role))
        })
        .collect()
}

fn exact_open_collateral_vault_metas(
    state: &OpenCollateralVaultState,
    custody_address: Pubkey,
    vault_address: Pubkey,
) -> Result<Vec<AccountMeta>, FoundationError> {
    OPEN_COLLATERAL_VAULT_FRAME
        .iter()
        .map(|role| {
            let key = match role.role() {
                Role::Sponsor => state.sponsor.key,
                Role::Market => state.market.key,
                Role::CapabilityReadiness => state.readiness.key,
                Role::RentCredit => state.rent_credit.key,
                Role::CapabilityManifest => state.capability_manifest.key,
                Role::Realm => state.realm.key,
                Role::CollateralCustody => custody_address,
                Role::CollateralVault => vault_address,
                Role::CollateralMint => state.collateral_mint.key,
                Role::CapabilityManifestStagingCursor => {
                    state.capability_manifest_finalization.staging_cursor.key
                }
                Role::RealmStagingCursor => state.realm_finalization.staging_cursor.key,
                Role::TokenProgram => state.token_program.key,
                Role::SystemProgram => system_program::ID,
                Role::RentSysvar => sysvar::rent::ID,
                _ => return Err(FoundationError::InstructionEncoding),
            };
            Ok(exact_meta(key, *role))
        })
        .collect()
}

fn exact_meta(key: Pubkey, role: AccountRole) -> AccountMeta {
    AccountMeta {
        pubkey: key,
        is_signer: role.is_signer(),
        is_writable: role.is_writable(),
    }
}

fn require_observation(observations: &[Observation]) -> Result<Observation, FoundationError> {
    let observation = observations
        .first()
        .copied()
        .ok_or(FoundationError::ObservationMismatch)?;
    if observation.finality != Finality::Finalized {
        return Err(FoundationError::ObservationNotFinalized);
    }
    if observations
        .iter()
        .any(|candidate| *candidate != observation)
    {
        return Err(FoundationError::ObservationMismatch);
    }
    Ok(observation)
}

fn authenticate_sponsor(account: &ObservedAccount) -> Result<(), FoundationError> {
    if account.owner != system_program::ID || account.executable || !account.data.is_empty() {
        return Err(FoundationError::InvalidSponsor);
    }
    Ok(())
}

fn authenticate_empty_system_destination(
    account: &ObservedAccount,
    expected: Pubkey,
) -> Result<(), FoundationError> {
    if account.key != expected
        || account.owner != system_program::ID
        || account.executable
        || account.lamports != 0
        || !account.data.is_empty()
    {
        return Err(FoundationError::DestinationNotVacant);
    }
    Ok(())
}

fn authenticate_system_program(account: &ObservedAccount) -> Result<(), FoundationError> {
    if account.key != system_program::ID
        || account.owner != native_loader::ID
        || !account.executable
        || !account.data.is_empty()
    {
        return Err(FoundationError::InvalidOwner);
    }
    Ok(())
}

fn authenticate_token_program(account: &ObservedAccount) -> Result<(), FoundationError> {
    if !account.executable
        || (account.owner != bpf_loader::ID && account.owner != bpf_loader_upgradeable::ID)
    {
        return Err(FoundationError::InvalidOwner);
    }
    Ok(())
}

pub(crate) fn authenticate_finalized_record(
    program_id: Pubkey,
    rent: &Rent,
    account: &ObservedAccount,
    proof: &FinalizedRecordProof,
) -> Result<(), FoundationError> {
    if account.owner != program_id || account.executable {
        return Err(FoundationError::InvalidOwner);
    }
    require_rent_exempt(rent, account)?;
    let schema = SchemaReleaseId::new(proof.schema_release_id)
        .map_err(|_| FoundationError::AddressMismatch)?;
    let digest = ContentDigest::new(hash(&account.data).to_bytes())
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    let schema_bytes = schema.to_bytes();
    let digest_bytes = digest.to_bytes();
    let (expected_raw, _) = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            schema_bytes.as_slice(),
            digest_bytes.as_slice(),
        ],
        &program_id,
    );
    let (expected_cursor, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema_bytes.as_slice(),
            digest_bytes.as_slice(),
        ],
        &program_id,
    );
    let cursor = &proof.staging_cursor;
    if account.key != expected_raw
        || cursor.key != expected_cursor
        || cursor.owner != system_program::ID
        || cursor.executable
        || !cursor.data.is_empty()
    {
        return Err(FoundationError::AddressMismatch);
    }
    if cursor.observation != account.observation {
        return Err(FoundationError::ObservationMismatch);
    }
    Ok(())
}

fn authenticate_vacancy(vacancy: ObservedVacancy, expected: Pubkey) -> Result<(), FoundationError> {
    if vacancy.key != expected {
        return Err(FoundationError::DestinationNotVacant);
    }
    Ok(())
}

fn authenticate_distinct(keys: &[Pubkey]) -> Result<(), FoundationError> {
    for (index, key) in keys.iter().enumerate() {
        if keys
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other == key)
        {
            return Err(FoundationError::AccountAlias);
        }
    }
    Ok(())
}

pub(crate) fn decode_rent(account: &ObservedAccount) -> Result<Rent, FoundationError> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(FoundationError::InvalidRent);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        account.executable,
    );
    Rent::from_account_info(&info).map_err(|_| FoundationError::InvalidRent)
}

fn require_rent_exempt(rent: &Rent, account: &ObservedAccount) -> Result<(), FoundationError> {
    if !rent.is_exempt(account.lamports, account.data.len()) {
        return Err(FoundationError::AccountNotRentExempt);
    }
    Ok(())
}

fn select_token_release(
    token_program: Pubkey,
) -> Result<CollateralAdapterReleaseV1, FoundationError> {
    PRODUCTION_ADAPTER_RELEASES
        .iter()
        .copied()
        .find(|release| release.token_program() == token_program.to_bytes())
        .ok_or(FoundationError::UnsupportedTokenProgram)
}

fn require_sponsor_balance(actual: u64, debit: u64) -> Result<(), FoundationError> {
    if actual < debit {
        return Err(FoundationError::SponsorUnderfunded);
    }
    Ok(())
}

fn require_canonical(observed: &[u8], canonical: &[u8]) -> Result<(), FoundationError> {
    if observed != canonical {
        return Err(FoundationError::NonCanonicalRecord);
    }
    Ok(())
}

fn product_id(bytes: [u8; 32]) -> Result<ProductContentId, FoundationError> {
    ProductContentId::new(bytes).map_err(|_| FoundationError::ContentLinkMismatch)
}

fn core_id(bytes: [u8; 32]) -> Result<CoreContentId, FoundationError> {
    CoreContentId::new(bytes).map_err(|_| FoundationError::ContentLinkMismatch)
}

fn require_realm_authorities(
    realm: RealmV1,
    mint_authority_present: bool,
    freeze_authority_present: bool,
) -> Result<(), FoundationError> {
    if (realm.mint_authority_policy() == MintAuthorityPolicy::RequireAbsent
        && mint_authority_present)
        || (realm.freeze_authority_policy() == FreezeAuthorityPolicy::RequireAbsent
            && freeze_authority_present)
    {
        return Err(FoundationError::IssuerAuthorityConsentRequired);
    }
    Ok(())
}

fn open_market_facts(
    program_id: Pubkey,
    market: &ObservedAccount,
    realm_id: [u8; 32],
    manifest_id: [u8; 32],
) -> Result<(u64, u64, [u8; 32]), FoundationError> {
    match decode_market_outcome_count(&market.data).map_err(|_| FoundationError::InvalidRecord)? {
        2 => typed_open_market_facts::<2>(program_id, market, realm_id, manifest_id),
        3 => typed_open_market_facts::<3>(program_id, market, realm_id, manifest_id),
        4 => typed_open_market_facts::<4>(program_id, market, realm_id, manifest_id),
        5 => typed_open_market_facts::<5>(program_id, market, realm_id, manifest_id),
        6 => typed_open_market_facts::<6>(program_id, market, realm_id, manifest_id),
        7 => typed_open_market_facts::<7>(program_id, market, realm_id, manifest_id),
        8 => typed_open_market_facts::<8>(program_id, market, realm_id, manifest_id),
        9 => typed_open_market_facts::<9>(program_id, market, realm_id, manifest_id),
        10 => typed_open_market_facts::<10>(program_id, market, realm_id, manifest_id),
        11 => typed_open_market_facts::<11>(program_id, market, realm_id, manifest_id),
        12 => typed_open_market_facts::<12>(program_id, market, realm_id, manifest_id),
        13 => typed_open_market_facts::<13>(program_id, market, realm_id, manifest_id),
        14 => typed_open_market_facts::<14>(program_id, market, realm_id, manifest_id),
        15 => typed_open_market_facts::<15>(program_id, market, realm_id, manifest_id),
        16 => typed_open_market_facts::<16>(program_id, market, realm_id, manifest_id),
        _ => Err(FoundationError::InvalidOutcomeCount),
    }
}

fn typed_open_market_facts<const N: usize>(
    program_id: Pubkey,
    observed: &ObservedAccount,
    realm_id: [u8; 32],
    manifest_id: [u8; 32],
) -> Result<(u64, u64, [u8; 32]), FoundationError> {
    if observed.owner != program_id || observed.executable {
        return Err(FoundationError::InvalidOwner);
    }
    let market = CategoricalMarketV1::<N>::decode(&observed.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let root = market.root();
    if root.phase() != dclutch_core_contract::Phase::Founding
        || root.outstanding_children() != 2
        || root.identity().realm_id().to_bytes() != realm_id
        || root.identity().capability_manifest_id().to_bytes() != manifest_id
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &program_id);
    if observed.key != expected {
        return Err(FoundationError::AddressMismatch);
    }
    Ok((
        root.identity().generation(),
        root.outstanding_children(),
        root.rent_refund(),
    ))
}

fn validate_market_space(outcome_count: u8, root: MarketRoot) -> Result<usize, FoundationError> {
    match outcome_count {
        2 => typed_market_space::<2>(root),
        3 => typed_market_space::<3>(root),
        4 => typed_market_space::<4>(root),
        5 => typed_market_space::<5>(root),
        6 => typed_market_space::<6>(root),
        7 => typed_market_space::<7>(root),
        8 => typed_market_space::<8>(root),
        9 => typed_market_space::<9>(root),
        10 => typed_market_space::<10>(root),
        11 => typed_market_space::<11>(root),
        12 => typed_market_space::<12>(root),
        13 => typed_market_space::<13>(root),
        14 => typed_market_space::<14>(root),
        15 => typed_market_space::<15>(root),
        16 => typed_market_space::<16>(root),
        _ => Err(FoundationError::InvalidOutcomeCount),
    }
}

fn typed_market_space<const N: usize>(root: MarketRoot) -> Result<usize, FoundationError> {
    CategoricalMarketV1::<N>::new(root, 0, [0; N], CategoricalSettlementSummaryV1::empty())
        .map_err(|_| FoundationError::ContentLinkMismatch)?;
    CategoricalMarketV1::<N>::encoded_len().map_err(|_| FoundationError::ArithmeticOverflow)
}

#[cfg(test)]
mod tests;
