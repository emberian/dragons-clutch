//! Finalized devnet planning for one wallet-authorized Claims Position admission.
//!
//! This module performs no RPC, wallet access, signing, or submission. It
//! reauthenticates one caller-supplied finalized snapshot, computes exact rent
//! deficits for the two vacant Claims PDAs, and returns an ordered unsigned
//! instruction plan. The two funding transfers and the Trading outer belong in
//! one transaction so any Claims refusal rolls the entire admission back.

use dclutch_claims::position_admission::{
    USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1, UserPositionAdmissionFrameV1,
    UserPositionAdmissionRequestV1,
};
use dclutch_claims::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketViewV2,
    },
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2,
        ProtocolPositionAdmissionEvidenceV2, ProtocolPositionAdmissionSeedsV2,
        ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
    },
};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase as CorePhase};
use dclutch_product::admission::{
    AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_product::payoff::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, semantic_basis_preimage_v3},
};
use dclutch_product::{ContentId as ProductContentId, ResultDomainV2};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_source::relay::SOLANA_DEVNET_GENESIS_HASH_V1;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program};
use solana_system_interface::instruction::transfer;

use crate::{
    Finality, Observation, ObservedAccount,
    observation::{FinalizedRecordProof, authenticate_finalized_record, decode_rent},
};

/// Domain separating the wallet's parent admission context from every child ABI.
pub const USER_POSITION_ADMISSION_PARENT_REQUEST_DOMAIN_V1: &[u8] =
    b"dclutch/operator/user-position-admission/v1";

/// One complete finalized snapshot required to plan a User Position admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPositionAdmissionSnapshotV1 {
    /// Cluster genesis hash reported with this snapshot.
    pub genesis_hash: [u8; 32],
    /// Existing Claims-owned LiabilityBasisV2 aggregate.
    pub claims_market: ObservedAccount,
    /// Vacant canonical Claims Position PDA.
    pub position: ObservedAccount,
    /// Vacant canonical Claims admission-record PDA.
    pub admission: ObservedAccount,
    /// Finalized ProductBasisV3 raw record.
    pub linked_basis_raw: ObservedAccount,
    /// Vacant ProductBasisV3 staging cursor.
    pub linked_basis_staging: ObservedAccount,
    /// Finalized Product Runtime V2 graph root.
    pub product_raw: ObservedAccount,
    /// Vacant Product staging cursor.
    pub product_staging: ObservedAccount,
    /// Finalized Product-selected ResultDomain.
    pub result_domain_raw: ObservedAccount,
    /// Vacant ResultDomain staging cursor.
    pub result_domain_staging: ObservedAccount,
    /// Finalized Product-selected Portfolio.
    pub portfolio_raw: ObservedAccount,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Existing canonical Core Market PDA.
    pub core_market: ObservedAccount,
    /// Registry-owned activated execution-release-set cache.
    pub activation_cache: ObservedAccount,
    /// Market-selected executable Registry program.
    pub registry_program: ObservedAccount,
    /// Activated executable Trading program.
    pub trading_program: ObservedAccount,
    /// Current Trading ProgramData account and complete ELF tail.
    pub trading_programdata: ObservedAccount,
    /// Activated executable Claims program.
    pub claims_program: ObservedAccount,
    /// Current Claims ProgramData account and complete ELF tail.
    pub claims_programdata: ObservedAccount,
    /// Activated executable Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData account and complete ELF tail.
    pub core_programdata: ObservedAccount,
    /// Wallet account that must sign the final transaction.
    pub owner: ObservedAccount,
    /// Existing lifecycle-scoped RentCredit selected by Core.
    pub rent_credit: ObservedAccount,
    /// Executable program owning the RentCredit.
    pub rent_program: ObservedAccount,
}

/// Exact unsigned admission plan and independently predicted Claims receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserPositionAdmissionPlanV1 {
    /// Ordered atomic instructions: zero to two rent transfers, then Trading.
    pub instructions: Vec<Instruction>,
    /// Sole required transaction signer.
    pub required_signer: Pubkey,
    /// Finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical Claims child request embedded in the Trading outer.
    pub claims_request: ProtocolPositionRequestV2,
    /// SHA-256 of the exact child request bytes.
    pub claims_request_digest: [u8; 32],
    /// Request-bound Trading caller-authority PDA supplied to Claims.
    pub caller_authority: Pubkey,
    /// Canonical Claims Position PDA.
    pub position: Pubkey,
    /// Canonical Claims admission-record PDA.
    pub admission: Pubkey,
    /// Current Rent minimum committed as Position rent principal.
    pub position_rent_principal: u64,
    /// Current Rent minimum committed as admission-record rent principal.
    pub admission_rent_principal: u64,
    /// Exact owner debit needed to bring the Position to its rent minimum.
    pub position_top_up_lamports: u64,
    /// Exact owner debit needed to bring the admission record to its rent minimum.
    pub admission_top_up_lamports: u64,
    /// Exact program expected to produce the immediate return data.
    pub expected_receipt_producer: Pubkey,
    /// Exact Claims receipt body predicted from authenticated inputs.
    pub expected_receipt_body: [u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2],
}

/// Stable refusal from hostile, stale, non-devnet, or underfunded observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserPositionAdmissionPlanErrorV1 {
    /// The supplied cluster identity was not Solana devnet.
    DevnetOnly,
    /// Accounts were not all from one nonzero finalized observation.
    InvalidObservation,
    /// A canonical sysvar, System program, or executable shell refused.
    InvalidInfrastructure,
    /// Activated release cache or current role deployment refused.
    InvalidRelease,
    /// Claims aggregate bytes, PDA, revision, or immutable links refused.
    InvalidClaimsMarket,
    /// Product graph, ProductBasisV3, or semantic joins refused.
    InvalidProductGraph,
    /// Core Market bytes, PDA, phase, or identity joins refused.
    InvalidCoreMarket,
    /// RentCredit bytes, PDA, owner, or lifecycle joins refused.
    InvalidRentCredit,
    /// Position or admission account was not the exact vacant PDA.
    InvalidVacancy,
    /// Exact width, balance, or rent arithmetic overflowed.
    ArithmeticOverflow,
    /// The wallet did not hold the exact lamports needed for both top-ups.
    InsufficientOwnerLamports,
    /// Canonical child/outer request or caller-authority construction refused.
    InvalidRequest,
    /// `dclutch_operator` refused; the cause is its own.
    Observation(crate::observation::ObservationError),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_registry::svm` refused; the cause is its own.
    RegistrySvm(dclutch_registry::svm::Error),
    /// `dclutch_claims` refused; the cause is its own.
    LiabilityBasisState(dclutch_claims::liability_basis_state_v2::LiabilityBasisStateErrorV2),
    /// `dclutch_product::admission` refused; the cause is its own.
    ProductRuntimeAdmission(dclutch_product::admission::Error),
    /// `dclutch_product` refused; the cause is its own.
    ProductRuntime(dclutch_product::Error),
    /// `dclutch_product::payoff` refused; the cause is its own.
    ProductBasis(dclutch_product::payoff::runtime_v3::Error),
    /// `dclutch_market` refused; the cause is its own.
    MarketCore(dclutch_market::Error),
    /// `dclutch_market::rent` refused; the cause is its own.
    LifecycleRent(dclutch_market::rent::lifecycle_v2::LifecycleRentErrorV2),
    /// `dclutch_claims` refused; the cause is its own.
    ProtocolPosition(dclutch_claims::protocol_position_v2::ProtocolPositionErrorV2),
    /// `dclutch_claims::position_admission` refused; the cause is its own.
    UserPositionAdmission(dclutch_claims::position_admission::UserPositionAdmissionErrorV1),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
}

#[derive(Clone, Copy)]
struct ProductFactsV1 {
    product_record_digest: [u8; 32],
    product_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    linked_basis_record_digest: [u8; 32],
    outcome_count: u32,
}

/// Reauthenticate one exact finalized devnet snapshot and build its unsigned plan.
pub fn plan_user_position_admission_v1(
    snapshot: &UserPositionAdmissionSnapshotV1,
) -> Result<UserPositionAdmissionPlanV1, UserPositionAdmissionPlanErrorV1> {
    if snapshot.genesis_hash != SOLANA_DEVNET_GENESIS_HASH_V1 {
        return Err(UserPositionAdmissionPlanErrorV1::DevnetOnly);
    }
    let observation = same_finalized_observation(snapshot)?;
    let rent = decode_rent(&snapshot.rent_sysvar)
        .map_err(UserPositionAdmissionPlanErrorV1::Observation)?;
    authenticate_infrastructure(snapshot)?;
    let activated = authenticate_release_cache(snapshot)?;
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Trading,
            &snapshot.trading_program,
            &snapshot.trading_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            &snapshot.claims_program,
            &snapshot.claims_programdata,
        ),
        (
            ExecutionRoleV1::Core,
            &snapshot.core_program,
            &snapshot.core_programdata,
        ),
    ] {
        authenticate_role_deployment(activated, role, program, programdata)?;
    }

    let market = authenticate_claims_market(snapshot, activated)?;
    let product = authenticate_product_graph(snapshot)?;
    authenticate_core_market(snapshot, market, product)?;
    authenticate_rent_credit(snapshot, market)?;
    authenticate_vacancy(snapshot)?;
    assemble_plan(snapshot, observation, rent, market, product)
}

fn same_finalized_observation(
    snapshot: &UserPositionAdmissionSnapshotV1,
) -> Result<Observation, UserPositionAdmissionPlanErrorV1> {
    let accounts = [
        &snapshot.claims_market,
        &snapshot.position,
        &snapshot.admission,
        &snapshot.linked_basis_raw,
        &snapshot.linked_basis_staging,
        &snapshot.product_raw,
        &snapshot.product_staging,
        &snapshot.result_domain_raw,
        &snapshot.result_domain_staging,
        &snapshot.portfolio_raw,
        &snapshot.portfolio_staging,
        &snapshot.rent_sysvar,
        &snapshot.system_program,
        &snapshot.core_market,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.trading_program,
        &snapshot.trading_programdata,
        &snapshot.claims_program,
        &snapshot.claims_programdata,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.owner,
        &snapshot.rent_credit,
        &snapshot.rent_program,
    ];
    let observation = accounts[0].observation;
    if observation.slot == 0
        || observation.finality != Finality::Finalized
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidObservation);
    }
    Ok(observation)
}

fn authenticate_infrastructure(
    snapshot: &UserPositionAdmissionSnapshotV1,
) -> Result<(), UserPositionAdmissionPlanErrorV1> {
    if snapshot.system_program.key != system_program::ID
        || snapshot.system_program.owner != native_loader::ID
        || !snapshot.system_program.executable
        || snapshot.owner.owner != system_program::ID
        || snapshot.owner.executable
        || !snapshot.owner.data.is_empty()
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidInfrastructure);
    }
    for program in [&snapshot.registry_program, &snapshot.rent_program] {
        if program.owner != bpf_loader_upgradeable::ID
            || !program.executable
            || ProgramV3View::parse(&program.data).is_err()
        {
            return Err(UserPositionAdmissionPlanErrorV1::InvalidInfrastructure);
        }
    }
    if snapshot.rent_credit.owner != snapshot.rent_program.key
        || snapshot.rent_credit.executable
        || !funded_rent_persists_v1(snapshot.rent_credit.lamports)
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRentCredit);
    }
    Ok(())
}

fn authenticate_release_cache<'a>(
    snapshot: &'a UserPositionAdmissionSnapshotV1,
) -> Result<ActivatedExecutionReleaseSetViewV1<'a>, UserPositionAdmissionPlanErrorV1> {
    let cache = &snapshot.activation_cache;
    if cache.owner != snapshot.registry_program.key
        || cache.executable
        || cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !funded_rent_persists_v1(cache.lamports)
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRelease);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache.data)
        .map_err(UserPositionAdmissionPlanErrorV1::Registry)?;
    let release_set = activated
        .execution_release_set_id()
        .map_err(UserPositionAdmissionPlanErrorV1::Registry)?;
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
        &snapshot.registry_program.key,
    )
    .0;
    if cache.key != expected {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRelease);
    }
    Ok(activated)
}

pub(crate) fn authenticate_role_deployment(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), UserPositionAdmissionPlanErrorV1> {
    let selected = activated
        .role(role)
        .map_err(UserPositionAdmissionPlanErrorV1::Registry)?;
    let observation = deployment_observation(program, programdata, selected.release())?;
    selected
        .authenticate_current_deployment(observation)
        .map_err(UserPositionAdmissionPlanErrorV1::Registry)
}

pub(crate) fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, UserPositionAdmissionPlanErrorV1> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRelease);
    }
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(UserPositionAdmissionPlanErrorV1::RegistrySvm)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != derived {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRelease);
    }
    let data = ProgramDataV3View::parse(&programdata.data)
        .map_err(UserPositionAdmissionPlanErrorV1::RegistrySvm)?;
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        data.deployment_slot(),
        hash(data.elf()).to_bytes(),
        data.upgrade_authority(),
    )
    .map_err(UserPositionAdmissionPlanErrorV1::Registry)
}

fn authenticate_claims_market(
    snapshot: &UserPositionAdmissionSnapshotV1,
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<LiabilityBasisMarketViewV2, UserPositionAdmissionPlanErrorV1> {
    let account = &snapshot.claims_market;
    if account.owner != snapshot.claims_program.key
        || account.executable
        || !funded_rent_persists_v1(account.lamports)
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidClaimsMarket);
    }
    let market = LiabilityBasisMarketViewV2::decode(&account.data)
        .map_err(UserPositionAdmissionPlanErrorV1::LiabilityBasisState)?;
    let expected = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            market.logical_market.as_slice(),
        ],
        &snapshot.claims_program.key,
    )
    .0;
    let release_set = activated
        .execution_release_set_id()
        .map_err(UserPositionAdmissionPlanErrorV1::Registry)?;
    if account.key != expected
        || market.registry_program != snapshot.registry_program.key.to_bytes()
        || market.release_set != release_set.to_bytes()
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidClaimsMarket);
    }
    Ok(market)
}

fn authenticate_product_graph(
    snapshot: &UserPositionAdmissionSnapshotV1,
) -> Result<ProductFactsV1, UserPositionAdmissionPlanErrorV1> {
    for (raw, staging, schema) in [
        (
            &snapshot.product_raw,
            &snapshot.product_staging,
            PRODUCT_RECORD_SCHEMA_ID_V2,
        ),
        (
            &snapshot.result_domain_raw,
            &snapshot.result_domain_staging,
            RESULT_DOMAIN_SCHEMA_ID_V2,
        ),
        (
            &snapshot.portfolio_raw,
            &snapshot.portfolio_staging,
            PORTFOLIO_SCHEMA_ID_V2,
        ),
        (
            &snapshot.linked_basis_raw,
            &snapshot.linked_basis_staging,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        ),
    ] {
        authenticate_finalized_record(
            snapshot.registry_program.key,
            raw,
            &FinalizedRecordProof {
                schema_release_id: schema,
                staging_cursor: staging.clone(),
            },
        )
        .map_err(UserPositionAdmissionPlanErrorV1::Observation)?;
    }

    let receipt = AdmissionReceiptV2 {
        product: finalized_coordinate(PRODUCT_RECORD_SCHEMA_ID_V2, &snapshot.product_raw)?,
        result_domain: finalized_coordinate(
            RESULT_DOMAIN_SCHEMA_ID_V2,
            &snapshot.result_domain_raw,
        )?,
        portfolio: finalized_coordinate(PORTFOLIO_SCHEMA_ID_V2, &snapshot.portfolio_raw)?,
    };
    let admitted = admit_authenticated_records_v2(
        receipt,
        &snapshot.product_raw.data,
        &snapshot.result_domain_raw.data,
        &snapshot.portfolio_raw.data,
    )
    .map_err(UserPositionAdmissionPlanErrorV1::ProductRuntimeAdmission)?;
    let domain = ResultDomainV2::decode(&snapshot.result_domain_raw.data)
        .map_err(UserPositionAdmissionPlanErrorV1::ProductRuntime)?;
    let basis = ProductBasisV3::decode(&snapshot.linked_basis_raw.data)
        .map_err(UserPositionAdmissionPlanErrorV1::ProductBasis)?;
    let semantic = semantic_basis_preimage_v3(&snapshot.linked_basis_raw.data)
        .map_err(UserPositionAdmissionPlanErrorV1::ProductBasis)?;
    let semantic_basis_id = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes();
    if semantic_basis_id != admitted.join.liability_basis_id.to_bytes()
        || basis.product_id() != admitted.join.product_id.to_bytes()
        || basis.result_domain_id() != admitted.join.result_domain_id.to_bytes()
        || basis.coordinate_domain_id() != domain.coordinate_domain_id().to_bytes()
        || basis.result_unit_id() != domain.result_unit_id().to_bytes()
        || basis.basis_width() != admitted.join.outcome_count
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidProductGraph);
    }
    Ok(ProductFactsV1 {
        product_record_digest: hash(&snapshot.product_raw.data).to_bytes(),
        product_id: admitted.join.product_id.to_bytes(),
        semantic_basis_id,
        linked_basis_record_digest: hash(&snapshot.linked_basis_raw.data).to_bytes(),
        outcome_count: admitted.join.outcome_count,
    })
}

fn finalized_coordinate(
    schema: [u8; 32],
    raw: &ObservedAccount,
) -> Result<FinalizedRecordCoordinateV2, UserPositionAdmissionPlanErrorV1> {
    Ok(FinalizedRecordCoordinateV2 {
        schema_id: ProductContentId::new(schema)
            .map_err(|_| UserPositionAdmissionPlanErrorV1::InvalidProductGraph)?,
        content_digest: ProductContentId::new(hash(&raw.data).to_bytes())
            .map_err(|_| UserPositionAdmissionPlanErrorV1::InvalidProductGraph)?,
        raw_account: ProductContentId::new(raw.key.to_bytes())
            .map_err(|_| UserPositionAdmissionPlanErrorV1::InvalidProductGraph)?,
        staging_account: ProductContentId::new(
            Pubkey::find_program_address(
                &[
                    dclutch_registry::record::STAGING_CURSOR_PDA_SEED_V1,
                    &schema,
                    &hash(&raw.data).to_bytes(),
                ],
                &raw.owner,
            )
            .0
            .to_bytes(),
        )
        .map_err(UserPositionAdmissionPlanErrorV1::ProductRuntime)?,
    })
}

fn authenticate_core_market(
    snapshot: &UserPositionAdmissionSnapshotV1,
    market: LiabilityBasisMarketViewV2,
    product: ProductFactsV1,
) -> Result<(), UserPositionAdmissionPlanErrorV1> {
    let account = &snapshot.core_market;
    if account.owner != snapshot.core_program.key
        || account.executable
        || !funded_rent_persists_v1(account.lamports)
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidCoreMarket);
    }
    let core =
        CoreState::decode(&account.data).map_err(UserPositionAdmissionPlanErrorV1::MarketCore)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &snapshot.core_program.key,
    )
    .0;
    if account.key != expected
        || account.key.to_bytes() != market.logical_market
        || core.phase != CorePhase::Open
        || core.identity.market_id.to_bytes() != market.logical_market
        || core.identity.realm_id.to_bytes() != market.realm_id
        || core.identity.product_record.to_bytes() != product.product_record_digest
        || core.identity.product_id.to_bytes() != product.product_id
        || core.identity.selected_release_set.to_bytes() != market.release_set
        || core.identity.registry_program.to_bytes() != market.registry_program
        || core.identity.generation != market.generation
        || core.rent_beneficiary.to_bytes() != snapshot.rent_credit.key.to_bytes()
        || market.product_instance_id != product.product_id
        || market.basis_id != product.semantic_basis_id
        || market.claim_count != product.outcome_count
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidCoreMarket);
    }
    Ok(())
}

fn authenticate_rent_credit(
    snapshot: &UserPositionAdmissionSnapshotV1,
    market: LiabilityBasisMarketViewV2,
) -> Result<(), UserPositionAdmissionPlanErrorV1> {
    let credit = LifecycleRentCreditV2::decode(&snapshot.rent_credit.data)
        .map_err(UserPositionAdmissionPlanErrorV1::LifecycleRent)?;
    if snapshot.rent_credit.owner != snapshot.rent_program.key
        || snapshot.rent_credit.executable
        || !funded_rent_persists_v1(snapshot.rent_credit.lamports)
        || credit.market().to_bytes() != market.logical_market
        || credit.release_set().to_bytes() != market.release_set
        || credit.generation() != market.generation
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRentCredit);
    }
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let market_seed = seeds.market().to_bytes();
    let generation = seeds.generation();
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market_seed.as_slice(),
            generation.as_slice(),
            &bump,
        ],
        &snapshot.rent_program.key,
    )
    .map_err(|_| UserPositionAdmissionPlanErrorV1::InvalidRentCredit)?;
    if snapshot.rent_credit.key != expected {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRentCredit);
    }
    Ok(())
}

fn authenticate_vacancy(
    snapshot: &UserPositionAdmissionSnapshotV1,
) -> Result<(), UserPositionAdmissionPlanErrorV1> {
    let position_seeds = ProtocolPositionSeedsV2::new(
        snapshot.claims_market.key.to_bytes(),
        snapshot.owner.key.to_bytes(),
    )
    .map_err(UserPositionAdmissionPlanErrorV1::ProtocolPosition)?;
    let admission_seeds = ProtocolPositionAdmissionSeedsV2::new(
        snapshot.claims_market.key.to_bytes(),
        snapshot.owner.key.to_bytes(),
    )
    .map_err(UserPositionAdmissionPlanErrorV1::ProtocolPosition)?;
    let expected_position =
        Pubkey::find_program_address(&position_seeds.as_slices(), &snapshot.claims_program.key).0;
    let expected_admission =
        Pubkey::find_program_address(&admission_seeds.as_slices(), &snapshot.claims_program.key).0;
    if snapshot.position.key != expected_position
        || snapshot.admission.key != expected_admission
        || snapshot.position.owner != system_program::ID
        || snapshot.admission.owner != system_program::ID
        || snapshot.position.executable
        || snapshot.admission.executable
        || !snapshot.position.data.is_empty()
        || !snapshot.admission.data.is_empty()
    {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidVacancy);
    }
    Ok(())
}

fn assemble_plan(
    snapshot: &UserPositionAdmissionSnapshotV1,
    observation: Observation,
    rent: Rent,
    market: LiabilityBasisMarketViewV2,
    product: ProductFactsV1,
) -> Result<UserPositionAdmissionPlanV1, UserPositionAdmissionPlanErrorV1> {
    let claim_count = usize::try_from(market.claim_count)
        .map_err(|_| UserPositionAdmissionPlanErrorV1::ArithmeticOverflow)?;
    let position_width = claim_count
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|tail| LIABILITY_BASIS_POSITION_HEADER_BYTES_V2.checked_add(tail))
        .ok_or(UserPositionAdmissionPlanErrorV1::ArithmeticOverflow)?;
    let position_rent_principal = rent.minimum_balance(position_width);
    let admission_rent_principal = rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    let position_top_up_lamports =
        position_rent_principal.saturating_sub(snapshot.position.lamports);
    let admission_top_up_lamports =
        admission_rent_principal.saturating_sub(snapshot.admission.lamports);
    let total_top_up = position_top_up_lamports
        .checked_add(admission_top_up_lamports)
        .ok_or(UserPositionAdmissionPlanErrorV1::ArithmeticOverflow)?;
    if snapshot.owner.lamports < total_top_up {
        return Err(UserPositionAdmissionPlanErrorV1::InsufficientOwnerLamports);
    }
    let observed_position_lamports = snapshot
        .position
        .lamports
        .checked_add(position_top_up_lamports)
        .ok_or(UserPositionAdmissionPlanErrorV1::ArithmeticOverflow)?;
    let observed_admission_lamports = snapshot
        .admission
        .lamports
        .checked_add(admission_top_up_lamports)
        .ok_or(UserPositionAdmissionPlanErrorV1::ArithmeticOverflow)?;
    let generation = market.generation.to_le_bytes();
    let revision = market.revision.to_le_bytes();
    let slot = observation.slot.to_le_bytes();
    let parent_request_digest = hashv(&[
        USER_POSITION_ADMISSION_PARENT_REQUEST_DOMAIN_V1,
        &market.release_set,
        &market.logical_market,
        snapshot.claims_market.key.as_ref(),
        snapshot.owner.key.as_ref(),
        &generation,
        &revision,
        &slot,
    ])
    .to_bytes();
    let claims_request = ProtocolPositionRequestV2::new(ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind: ProtocolPositionOwnerKindV2::User,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: market.release_set,
        market: market.logical_market,
        position_owner: snapshot.owner.key.to_bytes(),
        parent_request_digest,
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        rent_program: snapshot.rent_program.key.to_bytes(),
        generation: market.generation,
        expected_market_revision: market.revision,
        expected_position_revision: 0,
        observed_position_lamports,
        observed_admission_lamports,
        position_rent_principal,
        admission_rent_principal,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    })
    .map_err(UserPositionAdmissionPlanErrorV1::ProtocolPosition)?;
    let outer = UserPositionAdmissionRequestV1::new(claims_request)
        .map_err(UserPositionAdmissionPlanErrorV1::UserPositionAdmission)?;
    let child = outer
        .claims_request_bytes()
        .map_err(UserPositionAdmissionPlanErrorV1::UserPositionAdmission)?;
    let claims_request_digest = hash(&child).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        market.release_set,
        market.logical_market,
        ExecutionRoleV1::Trading,
        snapshot.owner.key.to_bytes(),
        claims_request_digest,
    )
    .map_err(UserPositionAdmissionPlanErrorV1::ReleaseSet)?;
    let caller_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), &snapshot.trading_program.key).0;
    let frame_keys = [
        snapshot.claims_program.key,
        caller_authority,
        snapshot.claims_market.key,
        snapshot.position.key,
        snapshot.admission.key,
        snapshot.linked_basis_raw.key,
        snapshot.linked_basis_staging.key,
        snapshot.product_raw.key,
        snapshot.product_staging.key,
        snapshot.result_domain_raw.key,
        snapshot.result_domain_staging.key,
        snapshot.portfolio_raw.key,
        snapshot.portfolio_staging.key,
        snapshot.rent_sysvar.key,
        snapshot.system_program.key,
        snapshot.core_market.key,
        snapshot.activation_cache.key,
        snapshot.registry_program.key,
        snapshot.trading_program.key,
        snapshot.trading_programdata.key,
        snapshot.claims_program.key,
        snapshot.claims_programdata.key,
        snapshot.core_program.key,
        snapshot.core_programdata.key,
        snapshot.owner.key,
        snapshot.rent_credit.key,
        snapshot.rent_program.key,
    ];
    let frame = UserPositionAdmissionFrameV1;
    if frame_keys.len() != USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1 {
        return Err(UserPositionAdmissionPlanErrorV1::InvalidRequest);
    }
    let mut accounts = Vec::with_capacity(USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1);
    for (index, key) in frame_keys.into_iter().enumerate() {
        let privileges = frame
            .privileges(index)
            .map_err(UserPositionAdmissionPlanErrorV1::UserPositionAdmission)?;
        accounts.push(if privileges.writable() {
            AccountMeta::new(key, privileges.signer())
        } else {
            AccountMeta::new_readonly(key, privileges.signer())
        });
    }
    let outer_data = outer
        .to_bytes()
        .map_err(UserPositionAdmissionPlanErrorV1::UserPositionAdmission)?;
    let trading_instruction = Instruction {
        program_id: snapshot.trading_program.key,
        accounts,
        data: outer_data.to_vec(),
    };
    let mut instructions = Vec::with_capacity(3);
    if position_top_up_lamports != 0 {
        instructions.push(transfer(
            &snapshot.owner.key,
            &snapshot.position.key,
            position_top_up_lamports,
        ));
    }
    if admission_top_up_lamports != 0 {
        instructions.push(transfer(
            &snapshot.owner.key,
            &snapshot.admission.key,
            admission_top_up_lamports,
        ));
    }
    instructions.push(trading_instruction);
    let expected_receipt_body = ProtocolPositionAdmissionV2::new(
        claims_request,
        ProtocolPositionAdmissionEvidenceV2 {
            product_record_digest: product.product_record_digest,
            semantic_basis_id: product.semantic_basis_id,
            linked_basis_record_digest: product.linked_basis_record_digest,
            request_digest: claims_request_digest,
            claims_program: snapshot.claims_program.key.to_bytes(),
            trading_program: snapshot.trading_program.key.to_bytes(),
            capability_descriptor: [0; 32],
            capability_outcome: 0,
            outcome_count: product.outcome_count,
        },
    )
    .and_then(ProtocolPositionAdmissionV2::to_receipt_bytes)
    .map_err(UserPositionAdmissionPlanErrorV1::ProtocolPosition)?;
    Ok(UserPositionAdmissionPlanV1 {
        instructions,
        required_signer: snapshot.owner.key,
        observation,
        claims_request,
        claims_request_digest,
        caller_authority,
        position: snapshot.position.key,
        admission: snapshot.admission.key,
        position_rent_principal,
        admission_rent_principal,
        position_top_up_lamports,
        admission_top_up_lamports,
        expected_receipt_producer: snapshot.claims_program.key,
        expected_receipt_body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::position_admission::{
        USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1,
        USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1,
        USER_POSITION_ADMISSION_CLAIMS_PROGRAM_ACCOUNT_V1,
        USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
    };

    const OBSERVATION: Observation = Observation {
        slot: 91,
        unix_timestamp: 1_788_000_000,
        finality: Finality::Finalized,
    };

    fn key(tag: u8) -> Pubkey {
        Pubkey::new_from_array([tag; 32])
    }

    fn observed(key: Pubkey, owner: Pubkey, lamports: u64) -> ObservedAccount {
        ObservedAccount {
            observation: OBSERVATION,
            key,
            owner,
            lamports,
            executable: false,
            data: Vec::new(),
        }
    }

    fn fixture(
        position_lamports: u64,
        admission_lamports: u64,
        owner_lamports: u64,
    ) -> (
        UserPositionAdmissionSnapshotV1,
        LiabilityBasisMarketViewV2,
        ProductFactsV1,
    ) {
        let claims_market = key(2);
        let logical_market = key(3);
        let release_set = key(4);
        let claims_program = key(5);
        let registry_program = key(6);
        let trading_program = key(7);
        let core_program = key(8);
        let owner = key(9);
        let rent_program = key(10);
        let rent_credit = key(11);
        let position_seeds =
            ProtocolPositionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
                .expect("position seeds");
        let admission_seeds =
            ProtocolPositionAdmissionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
                .expect("admission seeds");
        let position = Pubkey::find_program_address(&position_seeds.as_slices(), &claims_program).0;
        let admission =
            Pubkey::find_program_address(&admission_seeds.as_slices(), &claims_program).0;
        let market = LiabilityBasisMarketViewV2 {
            claim_count: 3,
            revision: 17,
            logical_market: logical_market.to_bytes(),
            release_set: release_set.to_bytes(),
            registry_program: registry_program.to_bytes(),
            product_instance_id: [21; 32],
            basis_id: [22; 32],
            realm_id: [23; 32],
            custody_context: [24; 32],
            generation: 25,
        };
        let product = ProductFactsV1 {
            product_record_digest: [31; 32],
            product_id: market.product_instance_id,
            semantic_basis_id: market.basis_id,
            linked_basis_record_digest: [32; 32],
            outcome_count: market.claim_count,
        };
        let mut registry = observed(registry_program, bpf_loader_upgradeable::ID, 1);
        registry.executable = true;
        let mut trading = observed(trading_program, bpf_loader_upgradeable::ID, 1);
        trading.executable = true;
        let mut claims = observed(claims_program, bpf_loader_upgradeable::ID, 1);
        claims.executable = true;
        let mut core = observed(core_program, bpf_loader_upgradeable::ID, 1);
        core.executable = true;
        let mut rent_program_account = observed(rent_program, bpf_loader_upgradeable::ID, 1);
        rent_program_account.executable = true;
        let mut system = observed(system_program::ID, native_loader::ID, 1);
        system.executable = true;
        let snapshot = UserPositionAdmissionSnapshotV1 {
            genesis_hash: SOLANA_DEVNET_GENESIS_HASH_V1,
            claims_market: observed(claims_market, claims_program, 1),
            position: observed(position, system_program::ID, position_lamports),
            admission: observed(admission, system_program::ID, admission_lamports),
            linked_basis_raw: observed(key(33), registry_program, 1),
            linked_basis_staging: observed(key(34), system_program::ID, 0),
            product_raw: observed(key(35), registry_program, 1),
            product_staging: observed(key(36), system_program::ID, 0),
            result_domain_raw: observed(key(37), registry_program, 1),
            result_domain_staging: observed(key(38), system_program::ID, 0),
            portfolio_raw: observed(key(39), registry_program, 1),
            portfolio_staging: observed(key(40), system_program::ID, 0),
            rent_sysvar: observed(key(41), key(42), 1),
            system_program: system,
            core_market: observed(logical_market, core_program, 1),
            activation_cache: observed(key(43), registry_program, 1),
            registry_program: registry,
            trading_program: trading,
            trading_programdata: observed(key(44), bpf_loader_upgradeable::ID, 1),
            claims_program: claims,
            claims_programdata: observed(key(45), bpf_loader_upgradeable::ID, 1),
            core_program: core,
            core_programdata: observed(key(46), bpf_loader_upgradeable::ID, 1),
            owner: observed(owner, system_program::ID, owner_lamports),
            rent_credit: observed(rent_credit, rent_program, 1),
            rent_program: rent_program_account,
        };
        (snapshot, market, product)
    }

    #[test]
    fn exact_atomic_plan_funds_both_vacancies_then_calls_trading() {
        let (snapshot, market, product) = fixture(1, 0, u64::MAX);
        let plan =
            assemble_plan(&snapshot, OBSERVATION, Rent::default(), market, product).expect("plan");
        assert_eq!(plan.instructions.len(), 3);
        assert_eq!(plan.instructions[0].program_id, system_program::ID);
        assert_eq!(plan.instructions[1].program_id, system_program::ID);
        let trading = &plan.instructions[2];
        assert_eq!(trading.program_id, snapshot.trading_program.key);
        assert_eq!(
            trading.accounts.len(),
            USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1
        );
        assert_eq!(
            trading.accounts[USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1].pubkey,
            snapshot.claims_program.key
        );
        assert_eq!(
            trading.accounts[USER_POSITION_ADMISSION_CLAIMS_PROGRAM_ACCOUNT_V1].pubkey,
            snapshot.claims_program.key
        );
        assert_eq!(
            trading.accounts[USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1].pubkey,
            plan.caller_authority
        );
        assert!(trading.accounts[USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1].is_signer);
        assert!(!trading.accounts[USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1].is_writable);
        let outer = UserPositionAdmissionRequestV1::decode(&trading.data).expect("outer");
        assert_eq!(outer.claims_request(), plan.claims_request);
        assert_eq!(
            plan.claims_request.observed_position_lamports,
            plan.position_rent_principal
        );
        assert_eq!(
            plan.claims_request.observed_admission_lamports,
            plan.admission_rent_principal
        );
        outer
            .validate_claims_receipt(
                &plan.expected_receipt_body,
                plan.claims_request_digest,
                snapshot.claims_program.key.to_bytes(),
                snapshot.trading_program.key.to_bytes(),
            )
            .expect("predicted receipt");
    }

    #[test]
    fn surplus_vacancies_are_preserved_without_spurious_transfers() {
        let rent = Rent::default();
        let position_principal = rent.minimum_balance(
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 3 * core::mem::size_of::<u64>(),
        );
        let admission_principal = rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
        let (snapshot, market, product) =
            fixture(position_principal + 7, admission_principal + 11, u64::MAX);
        let plan = assemble_plan(&snapshot, OBSERVATION, rent, market, product).expect("plan");
        assert_eq!(plan.instructions.len(), 1);
        assert_eq!(plan.position_top_up_lamports, 0);
        assert_eq!(plan.admission_top_up_lamports, 0);
        assert_eq!(
            plan.claims_request.observed_position_lamports,
            position_principal + 7
        );
        assert_eq!(
            plan.claims_request.observed_admission_lamports,
            admission_principal + 11
        );
    }

    #[test]
    fn owner_underfunding_refuses_before_any_plan_is_returned() {
        let (snapshot, market, product) = fixture(0, 0, 1);
        assert_eq!(
            assemble_plan(&snapshot, OBSERVATION, Rent::default(), market, product),
            Err(UserPositionAdmissionPlanErrorV1::InsufficientOwnerLamports)
        );
    }

    #[test]
    fn mainnet_or_mixed_snapshot_refuses_before_hostile_data_is_decoded() {
        let (mut snapshot, _, _) = fixture(0, 0, u64::MAX);
        snapshot.genesis_hash = [99; 32];
        assert_eq!(
            plan_user_position_admission_v1(&snapshot),
            Err(UserPositionAdmissionPlanErrorV1::DevnetOnly)
        );
        snapshot.genesis_hash = SOLANA_DEVNET_GENESIS_HASH_V1;
        snapshot.portfolio_staging.observation.slot += 1;
        assert_eq!(
            plan_user_position_admission_v1(&snapshot),
            Err(UserPositionAdmissionPlanErrorV1::InvalidObservation)
        );
    }

    #[test]
    fn vacancy_substitution_and_stale_parent_context_change_refuse_or_rebind() {
        let (mut snapshot, _market, _product) = fixture(0, 0, u64::MAX);
        snapshot.position.key = key(100);
        assert_eq!(
            authenticate_vacancy(&snapshot),
            Err(UserPositionAdmissionPlanErrorV1::InvalidVacancy)
        );
        let (snapshot, market, product) = fixture(0, 0, u64::MAX);
        let first =
            assemble_plan(&snapshot, OBSERVATION, Rent::default(), market, product).expect("first");
        let mut later = OBSERVATION;
        later.slot += 1;
        let second =
            assemble_plan(&snapshot, later, Rent::default(), market, product).expect("second");
        assert_ne!(
            first.claims_request.parent_request_digest,
            second.claims_request.parent_request_digest
        );
        assert_ne!(first.claims_request_digest, second.claims_request_digest);
        assert_ne!(first.caller_authority, second.caller_authority);
    }
}
